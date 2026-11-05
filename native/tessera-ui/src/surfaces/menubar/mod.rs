//! The menu bar (rust.surfaces: menubar): one Bar per output, the system
//! menu, and the data (Hyprland, battery, Wi-Fi, the clock, settings) flowing
//! into them. Compare shell/modules/menubar/MenuBar.qml.

pub mod settings;

use crate::services::hypr::{self, Glass, Snapshot};
use crate::services::status::{Battery, Wifi};
use crate::shell::Part;
use crate::theme;
use crate::ui;
use crate::util::{desktop_name, now};
use crate::wayland::{Spec, State, Surface};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use smithay_client_toolkit::reexports::client::protocol::wl_output::WlOutput;
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::rc::Rc;

pub const BAR_NS: &str = "tessera:bar";
pub const MENU_NS: &str = "tessera:barmenu";
const NAME: &str = "menubar";
const BAR_H: u32 = 30;
const SHADOW: f32 = 16.0; // the menu window's room around the panel

/// Its own messages (Msg::Part("menubar", …))
enum M {
    Tick,
    ToggleMenu(String),
    MenuActivated(i32, String),
    MenuDismissed,
    MenuClosed,
}

struct BarEntry {
    output: String,
    surface: Rc<Surface>,
    ui: ui::Bar,
    a11y: Option<crate::a11y::A11y>,
    glass_sent: String,
}

struct MenuEntry {
    output: String,
    surface: Rc<Surface>,
    ui: ui::Menu,
    a11y: Option<crate::a11y::A11y>,
    open: bool,
    glass_sent: String,
}

#[derive(Default)]
pub struct Menubar {
    bars: Vec<BarEntry>,
    menu: Option<MenuEntry>,
    names: HashMap<String, String>,
    clock: slint::Timer,
}

fn rows() -> ModelRc<ui::MenuRow> {
    let r = |icon: &str, text: &str, shortcut: &str| ui::MenuRow {
        icon: icon.into(),
        text: text.into(),
        shortcut: shortcut.into(),
        separator: false,
    };
    ModelRc::new(VecModel::from(vec![
        r("\u{e2ce}", "About this computer", ""),
        r("\u{e270}", "System settings…", "Super ,"),
        ui::MenuRow { separator: true, ..Default::default() },
        r("\u{e2fa}", "Lock screen", "Super L"),
        r("\u{e58e}", "Sleep", ""),
    ]))
}

impl Menubar {
    fn spec<'a>(output: &'a WlOutput, name: &'a str) -> Spec<'a> {
        Spec {
            layer: Layer::Top,
            namespace: BAR_NS,
            output: Some((output, name)),
            anchor: Anchor::TOP | Anchor::LEFT | Anchor::RIGHT,
            size: (0, BAR_H),
            exclusive: BAR_H as i32,
            keyboard: KeyboardInteractivity::None,
            margin_left: 0,
        }
    }

    /// Push the data into every bar
    fn refresh(&mut self, st: &mut State) {
        let snapshot = st.shell.get::<Snapshot>().cloned().unwrap_or_default();
        let bat = st.shell.get::<Battery>().cloned().unwrap_or_default();
        let w = st.shell.get::<Wifi>().cloned().unwrap_or_default();
        let cfg = &st.shell.cfg;
        let class = snapshot.class.clone();
        let app_name = if class.is_empty() {
            "Desktop".to_string()
        } else {
            self.names
                .entry(class.clone())
                .or_insert_with(|| {
                    desktop_name(&class).unwrap_or_else(|| {
                        let mut c = class.chars();
                        c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
                    })
                })
                .clone()
        };
        let (short, long) = settings::clock_formats(cfg);
        let (clock, clock_long) = (now(&short), now(&long));
        let logo = settings::menu_icon(cfg);
        let strip = settings::bar_background(cfg);
        let level = bat.percent.round() as i32;
        let battery_icon = if bat.charging {
            "\u{e0ba}"
        } else {
            match level {
                90.. => "\u{e0c0}",
                60..=89 => "\u{e0c2}",
                35..=59 => "\u{e0c6}",
                10..=34 => "\u{e0c4}",
                _ => "\u{e0be}",
            }
        };
        let (wifi_icon, wifi_label) = if !w.enabled {
            ("\u{e4f2}", "Wi-Fi off".to_string())
        } else if w.ssid.is_empty() {
            ("\u{e4f0}", "Wi-Fi on, not connected".to_string())
        } else {
            let (i, q) = match w.strength {
                76.. => ("\u{e4ea}", "excellent"),
                51..=75 => ("\u{e4ee}", "good"),
                31..=50 => ("\u{e4ec}", "fair"),
                _ => ("\u{e4f0}", "weak"),
            };
            (i, format!("Wi-Fi {}, {q} signal", w.ssid))
        };
        for b in &self.bars {
            let ui = &b.ui;
            ui.set_app_name(app_name.as_str().into());
            ui.set_logo(logo.into());
            ui.set_strip(strip);
            ui.set_clock(clock.as_str().into());
            ui.set_clock_long(clock_long.as_str().into());
            ui.set_battery(if bat.present { level } else { -1 });
            ui.set_charging(bat.charging);
            ui.set_battery_icon(battery_icon.into());
            ui.set_wifi_shown(w.present);
            ui.set_wifi_icon(wifi_icon.into());
            ui.set_wifi_label(wifi_label.as_str().into());
            ui.set_menu_open(self.menu.as_ref().is_some_and(|m| m.open && m.output == b.output));
            let (spaces, current) = spaces_for(&snapshot, &b.output);
            let spaces: Vec<ui::Space> = spaces.into_iter().map(|(id, occupied)| ui::Space { id, occupied }).collect();
            if !ui.get_spaces().iter().eq(spaces.iter().cloned()) {
                ui.set_spaces(ModelRc::new(VecModel::from(spaces)));
            }
            ui.set_current_space(current);
        }
    }

    /// Re-arm the clock: on the minute, or every second when it shows seconds
    fn tick(&mut self, st: &mut State) {
        self.refresh(st);
        let every_second = st.shell.cfg["menubar"]["showSeconds"].as_bool().unwrap_or(false);
        let secs = now("%S").parse::<u64>().unwrap_or(0);
        let wait = if every_second { 1000 } else { (60 - secs.min(59)) * 1000 + 50 };
        let tx = st.tx.clone();
        self.clock.start(slint::TimerMode::SingleShot, std::time::Duration::from_millis(wait), move || {
            let _ = tx.send(crate::Msg::Part(NAME, Box::new(M::Tick)));
        });
    }

    fn apply_theme(&mut self, st: &State) {
        for b in &mut self.bars {
            st.shell.apply_theme(&b.ui.global::<ui::Theme>());
            b.glass_sent.clear();
        }
        if let Some(m) = &mut self.menu {
            st.shell.apply_theme(&m.ui.global::<ui::Theme>());
            m.glass_sent.clear();
        }
    }

    /// `menubar.screens` changed: bars come and go to match
    fn screens_changed(&mut self, st: &mut State) {
        let list = st.shell.cfg["menubar"]["screens"].clone();
        let gone: Vec<String> = self.bars.iter().filter(|b| !crate::surfaces::on_screen(&list, &b.output)).map(|b| b.output.clone()).collect();
        for g in gone {
            self.output_removed(st, &g);
        }
        for (o, n) in st.outputs_now() {
            self.output_added(st, &o, &n);
        }
    }

    // ── The system menu ──────────────────────────────────────────────────

    fn toggle_menu(&mut self, st: &mut State, output: String) {
        if self.menu.as_ref().is_some_and(|m| m.open) {
            self.close_menu(st);
            return;
        }
        let Some(bar) = self.bars.iter().find(|b| b.output == output) else { return };
        let left = (bar.ui.get_logo_x() - SHADOW + 4.0).max(0.0) as i32;
        // (re)made on the output it opens on
        if self.menu.as_ref().is_some_and(|m| m.output != output) {
            let m = self.menu.take().unwrap();
            st.drop_surface(&m.surface);
        }
        if self.menu.is_none() {
            let Some(wl_output) = st.output_named(&output) else { return };
            let surface = st.prepare_surface(Spec {
                layer: Layer::Top,
                namespace: MENU_NS,
                output: Some((&wl_output, &output)),
                anchor: Anchor::TOP | Anchor::LEFT,
                size: (292, 1),
                exclusive: 0,
                keyboard: KeyboardInteractivity::OnDemand,
                margin_left: left,
            });
            let ui = ui::Menu::new().expect("Menu");
            st.shell.apply_theme(&ui.global::<ui::Theme>());
            ui.set_rows(rows());
            let to = st.shell.sender(NAME);
            let (to2, to3) = (to.clone(), to.clone());
            ui.on_activated(move |i, text| to(Box::new(M::MenuActivated(i, text.to_string()))));
            ui.on_dismissed(move || to2(Box::new(M::MenuDismissed)));
            ui.on_closed(move || to3(Box::new(M::MenuClosed)));
            ui.show().expect("show menu");
            let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
            self.menu = Some(MenuEntry { output: output.clone(), surface, ui, a11y, open: false, glass_sent: String::new() });
        }
        let m = self.menu.as_mut().unwrap();
        m.surface.layer.set_margin(0, 0, 0, left);
        let want = (m.ui.get_panel_height() + 2.0 * SHADOW).ceil() as u32;
        m.surface.show(292, want);
        m.open = true;
        m.ui.set_shown(true);
        m.ui.invoke_open();
        self.refresh(st);
    }

    fn close_menu(&mut self, st: &mut State) {
        if let Some(m) = &mut self.menu {
            if m.open {
                m.open = false;
                m.ui.set_shown(false); // the close animation, then MenuClosed hides it
            }
        }
        self.refresh(st);
    }

    fn menu_closed(&mut self, st: &mut State) {
        if let Some(m) = &mut self.menu {
            if !m.open {
                m.surface.hide();
                st.shell.glass.set(&m.output, MENU_NS, &[]);
                m.glass_sent.clear();
            }
        }
    }

    fn menu_activated(&mut self, st: &mut State, row: i32, search: String) {
        let what = match row {
            0 => format!("search {search:?}"),
            1 => "about".into(),
            2 => "settings".into(),
            4 => "lock".into(), // (a preview: nothing is really locked or put to sleep yet)
            5 => "sleep".into(),
            _ => return,
        };
        eprintln!("tessera-ui: menu chose {what}");
        self.close_menu(st);
    }
}

/// This output's spaces: its own, and at least five from its base (as
/// Workspaces.qml), each with whether it has windows, and the current one's index
pub fn spaces_for(h: &Snapshot, output: &str) -> (Vec<(i32, bool)>, i32) {
    let active = h.active.get(output).copied().unwrap_or(0);
    let mut ids: Vec<i32> = h.workspaces.iter().filter(|w| w.monitor == output && w.id > 0).map(|w| w.id).collect();
    let base = if active > 0 { (active - 1) / 10 * 10 } else { 0 };
    let top = ids.iter().copied().max().unwrap_or(0).max(base + 5);
    ids.extend(base + 1..=top);
    ids.sort_unstable();
    ids.dedup();
    let spaces = ids.iter().map(|&id| (id, h.workspaces.iter().any(|w| w.id == id && w.windows > 0))).collect();
    let current = ids.iter().position(|&i| i == active).map_or(-1, |i| i as i32);
    (spaces, current)
}

impl Part for Menubar {
    fn name(&self) -> &'static str {
        NAME
    }

    fn start(&mut self, st: &mut State) {
        st.shell.want("hypr");
        st.shell.want("status");
        self.tick(st);
    }

    fn output_added(&mut self, st: &mut State, output: &WlOutput, name: &str) {
        if self.bars.iter().any(|b| b.output == name) || !crate::surfaces::on_screen(&st.shell.cfg["menubar"]["screens"], name) {
            return;
        }
        let surface = st.prepare_surface(Self::spec(output, name));
        let ui = ui::Bar::new().expect("Bar");
        st.shell.apply_theme(&ui.global::<ui::Theme>());
        let (to, out) = (st.shell.sender(NAME), name.to_string());
        ui.on_logo_clicked(move || to(Box::new(M::ToggleMenu(out.clone()))));
        ui.on_go(hypr::focus_workspace);
        ui.show().expect("show bar");
        let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
        self.bars.push(BarEntry { output: name.to_string(), surface, ui, a11y, glass_sent: String::new() });
        self.refresh(st);
    }

    fn output_removed(&mut self, st: &mut State, name: &str) {
        if let Some(i) = self.bars.iter().position(|b| b.output == name) {
            let b = self.bars.remove(i);
            st.drop_surface(&b.surface);
        }
        if self.menu.as_ref().is_some_and(|m| m.output == name) {
            let m = self.menu.take().unwrap();
            st.drop_surface(&m.surface);
        }
    }

    fn config_changed(&mut self, st: &mut State, look: bool) {
        self.screens_changed(st);
        self.tick(st); // (the clock's format, the menu icon, the background)
        if look {
            self.apply_theme(st);
        }
    }

    fn plugin_changed(&mut self, st: &mut State) {
        self.apply_theme(st);
    }

    fn data(&mut self, st: &mut State, what: TypeId) {
        if what == TypeId::of::<Snapshot>() || what == TypeId::of::<Battery>() || what == TypeId::of::<Wifi>() {
            self.refresh(st);
        }
    }

    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Tick => self.tick(st),
            M::ToggleMenu(o) => self.toggle_menu(st, o),
            M::MenuActivated(i, t) => self.menu_activated(st, i, t),
            M::MenuDismissed => self.close_menu(st),
            M::MenuClosed => self.menu_closed(st),
        }
    }

    fn keyboard_left(&mut self, st: &mut State, s: &Rc<Surface>) {
        // Clicking another window takes the keyboard: the menu closes, as on macOS
        if self.menu.as_ref().is_some_and(|m| m.open && Rc::ptr_eq(&m.surface, s)) {
            self.close_menu(st);
        }
    }

    fn surface_closed(&mut self, st: &mut State, s: &Rc<Surface>) {
        let name = s.output.clone();
        self.output_removed(st, &name);
    }

    /// The glass shapes for the plugin (the bar's strip when Settings gives it
    /// a background; the menu's panel as it springs open), sent when they
    /// change; then the accessibility trees
    fn after_frame(&mut self, st: &mut State) {
        let kbd = st.keyboard_surface();
        for b in &mut self.bars {
            let f = kbd.as_ref().is_some_and(|k| Rc::ptr_eq(k, &b.surface));
            if let Some(a) = b.a11y.as_mut() {
                a.update(false, f);
            }
        }
        if let Some(m) = self.menu.as_mut() {
            let f = kbd.as_ref().is_some_and(|k| Rc::ptr_eq(k, &m.surface));
            if let Some(a) = m.a11y.as_mut() {
                a.update(false, f);
            }
        }
        if !st.shell.plugin || st.shell.look.material {
            return;
        }
        let look = st.shell.look.clone();
        let strip = settings::bar_background(&st.shell.cfg);
        for b in &mut self.bars {
            if !b.surface.mapped.get() {
                continue;
            }
            let (w, h) = b.surface.logical_size();
            let shapes: Vec<[f32; 19]> = if strip { vec![theme::glass_shape(&look, 0.0, 0.0, w as f32, h as f32, 0.0, 1.0, 1.0)] } else { Vec::new() };
            let key = format!("{shapes:?}");
            if key != b.glass_sent {
                st.shell.glass.set(&b.output, BAR_NS, &shapes);
                b.glass_sent = key;
            }
        }
        if let Some(m) = &mut self.menu {
            if !m.surface.mapped.get() {
                return;
            }
            let ui = &m.ui;
            let open = ui.get_openness();
            let s = ui.get_panel_scale();
            let (px, py, pw, ph) = (ui.get_panel_x(), ui.get_panel_y(), ui.get_panel_width(), ui.get_panel_height());
            // scaled about (18, 0) in the panel, as in menubar.slint
            let x = px + 18.0 * (1.0 - s);
            let opacity = (open * 1.6).min(1.0);
            let shape = theme::glass_shape(&look, x, py, pw * s, ph * s, 12.0 * s, opacity, 0.35);
            let key = format!("{shape:?}");
            if key != m.glass_sent {
                st.shell.glass.set(&m.output, MENU_NS, if opacity > 0.002 { std::slice::from_ref(&shape) } else { &[] });
                m.glass_sent = key;
            }
        }
    }

    fn a11y(&mut self) -> Vec<&mut crate::a11y::A11y> {
        let mut v: Vec<&mut crate::a11y::A11y> = self.bars.iter_mut().filter_map(|b| b.a11y.as_mut()).collect();
        if let Some(m) = self.menu.as_mut().and_then(|m| m.a11y.as_mut()) {
            v.push(m);
        }
        v
    }

    fn stats(&self) {
        for b in &self.bars {
            eprintln!("tessera-ui: stats {}", b.surface.stats.borrow_mut().report(&format!("bar:{}", b.output)));
        }
        if let Some(m) = &self.menu {
            eprintln!("tessera-ui: stats {}", m.surface.stats.borrow_mut().report("menu"));
        }
    }

    /// On the way out: no glass left behind
    fn quit(&mut self, _: &mut State) {
        for b in &self.bars {
            Glass::clear_now(&b.output, BAR_NS);
        }
        if let Some(m) = &self.menu {
            Glass::clear_now(&m.output, MENU_NS);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::hypr::Workspace;

    fn ws(id: i32, monitor: &str, windows: u32) -> Workspace {
        Workspace { id, monitor: monitor.into(), windows, ..Default::default() }
    }

    #[test]
    fn at_least_five_spaces_and_the_used_ones() {
        let mut h = Snapshot::default();
        h.workspaces = vec![ws(1, "DP-1", 2), ws(7, "DP-1", 1), ws(2, "HDMI-A-1", 1)];
        h.active.insert("DP-1".into(), 7);
        let (spaces, current) = spaces_for(&h, "DP-1");
        let ids: Vec<i32> = spaces.iter().map(|s| s.0).collect();
        assert_eq!(ids, vec![1, 2, 3, 4, 5, 6, 7]);
        assert_eq!(current, 6);
        // (occupied wherever its windows are: 2 lives on the other monitor)
        assert!(spaces[0].1 && spaces[1].1 && !spaces[2].1 && spaces[6].1);
    }

    #[test]
    fn a_second_ten_starts_from_its_base() {
        let mut h = Snapshot::default();
        h.workspaces = vec![ws(12, "DP-1", 1)];
        h.active.insert("DP-1".into(), 12);
        let ids: Vec<i32> = spaces_for(&h, "DP-1").0.iter().map(|s| s.0).collect();
        assert_eq!(ids, vec![11, 12, 13, 14, 15]);
    }
}

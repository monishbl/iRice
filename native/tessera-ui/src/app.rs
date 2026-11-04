//! The bar's behaviour: one Bar per output, the system menu, and the data
//! (Hyprland, battery, Wi-Fi, clock, config) flowing into them.

use crate::hypr::{self, Glass};
use crate::status::{Battery, Wifi};
use crate::theme::{self, Look};
use crate::ui;
use crate::wayland::{Spec, State, Surface};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use smithay_client_toolkit::reexports::client::protocol::wl_output::WlOutput;
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use std::collections::HashMap;
use std::rc::Rc;

pub const BAR_NS: &str = "tessera:bar";
pub const MENU_NS: &str = "tessera:barmenu";
const BAR_H: u32 = 30;
const SHADOW: f32 = 16.0; // the menu window's room around the panel

pub struct BarEntry {
    pub output: String,
    pub surface: Rc<Surface>,
    pub ui: ui::Bar,
    pub a11y: Option<crate::a11y::A11y>,
    glass_sent: String,
}

pub struct MenuEntry {
    pub output: String,
    pub surface: Rc<Surface>,
    pub ui: ui::Menu,
    pub a11y: Option<crate::a11y::A11y>,
    open: bool,
    glass_sent: String,
}

pub struct App {
    pub look: Look,
    pub glass: Glass,
    pub plugin: bool,
    pub bars: Vec<BarEntry>,
    pub menu: Option<MenuEntry>,
    pub hypr: hypr::Snapshot,
    pub battery: Battery,
    pub wifi: Wifi,
    names: HashMap<String, String>,
    pub clock: slint::Timer,
    /// what this process draws (--surfaces)
    pub surfaces: crate::surfaces::Surfaces,
    /// config.json, as last read
    pub cfg: serde_json::Value,
}

impl App {
    pub fn new(glass: Glass, surfaces: crate::surfaces::Surfaces) -> Self {
        Self {
            surfaces,
            cfg: theme::read_json(),
            look: theme::read(),
            glass,
            plugin: false,
            bars: Vec::new(),
            menu: None,
            hypr: Default::default(),
            battery: Default::default(),
            wifi: Default::default(),
            names: HashMap::new(),
            clock: slint::Timer::default(),
        }
    }
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

/// strftime in the local time zone
fn now(fmt: &str) -> String {
    let fmt = std::ffi::CString::new(fmt).unwrap();
    let mut buf = [0u8; 128];
    unsafe {
        let t = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        let n = libc::strftime(buf.as_mut_ptr() as *mut libc::c_char, buf.len(), fmt.as_ptr(), &tm);
        String::from_utf8_lossy(&buf[..n]).into_owned()
    }
}

/// An app's name from its desktop entry (by file name or StartupWMClass)
fn desktop_name(class: &str) -> Option<String> {
    let home = std::env::var("XDG_DATA_HOME").unwrap_or_else(|_| format!("{}/.local/share", std::env::var("HOME").unwrap_or_default()));
    let dirs = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    let lc = class.to_lowercase();
    let name_of = |text: &str| {
        let mut in_entry = false;
        for l in text.lines() {
            if l.starts_with('[') {
                in_entry = l == "[Desktop Entry]";
            } else if in_entry {
                if let Some(n) = l.strip_prefix("Name=") {
                    return Some(n.to_string());
                }
            }
        }
        None
    };
    let mut by_wmclass = None;
    for d in std::iter::once(home.as_str()).chain(dirs.split(':')) {
        let Ok(rd) = std::fs::read_dir(format!("{d}/applications")) else { continue };
        for e in rd.flatten() {
            let f = e.file_name().to_string_lossy().to_lowercase();
            let Some(stem) = f.strip_suffix(".desktop") else { continue };
            let wanted = stem == lc || stem.rsplit('.').next() == Some(lc.as_str());
            if !wanted && by_wmclass.is_some() {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(e.path()) else { continue };
            if wanted {
                return name_of(&text);
            }
            if text.lines().any(|l| l.strip_prefix("StartupWMClass=").is_some_and(|c| c.eq_ignore_ascii_case(class))) {
                by_wmclass = name_of(&text);
            }
        }
    }
    by_wmclass
}

impl State {
    fn spec<'a>(&self, output: &'a WlOutput, name: &'a str) -> Spec<'a> {
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

    pub fn app_output_added(&mut self, output: WlOutput, name: String) {
        if self.app.bars.iter().any(|b| b.output == name) || !self.app.surfaces.has("menubar") || !crate::surfaces::on_screen(&theme::read_json(), &name) {
            return;
        }
        let surface = self.prepare_surface(self.spec(&output, &name));
        let ui = ui::Bar::new().expect("Bar");
        theme::apply(&ui.global::<ui::Theme>(), &self.app.look, self.app.plugin);
        let (tx, out) = (self.tx.clone(), name.clone());
        ui.on_logo_clicked(move || {
            let _ = tx.send(crate::Msg::ToggleMenu(out.clone()));
        });
        ui.on_go(hypr::focus_workspace);
        ui.show().expect("show bar");
        let a11y = crate::a11y::A11y::new(&surface, self.tx.clone());
        self.app.bars.push(BarEntry { output: name, surface, ui, a11y, glass_sent: String::new() });
        self.app_refresh();
    }

    pub fn app_output_removed(&mut self, name: &str) {
        if let Some(i) = self.app.bars.iter().position(|b| b.output == name) {
            let b = self.app.bars.remove(i);
            self.drop_surface(&b.surface);
        }
        if self.app.menu.as_ref().is_some_and(|m| m.output == name) {
            let m = self.app.menu.take().unwrap();
            self.drop_surface(&m.surface);
        }
    }

    pub fn app_surface_closed(&mut self, s: &Rc<Surface>) {
        let name = s.output.clone();
        self.app_output_removed(&name);
    }

    pub fn app_keyboard_left(&mut self, s: &Rc<Surface>) {
        // Clicking another window takes the keyboard: the menu closes, as on macOS
        if self.app.menu.as_ref().is_some_and(|m| m.open && Rc::ptr_eq(&m.surface, s)) {
            self.app_close_menu();
        }
    }


    /// Push the data into every bar
    pub fn app_refresh(&mut self) {
        let class = self.app.hypr.class.clone();
        let app_name = if class.is_empty() {
            "Desktop".to_string()
        } else {
            self.app
                .names
                .entry(class.clone())
                .or_insert_with(|| {
                    desktop_name(&class).unwrap_or_else(|| {
                        let mut c = class.chars();
                        c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
                    })
                })
                .clone()
        };
        let (short, long) = crate::settings::clock_formats(&self.app.cfg);
        let (clock, clock_long) = (now(&short), now(&long));
        let logo = crate::settings::menu_icon(&self.app.cfg);
        let bat = &self.app.battery;
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
        let w = &self.app.wifi;
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
        for b in &self.app.bars {
            let ui = &b.ui;
            ui.set_app_name(app_name.as_str().into());
            ui.set_logo(logo.into());
            ui.set_strip(crate::settings::bar_background(&self.app.cfg));
            ui.set_clock(clock.as_str().into());
            ui.set_clock_long(clock_long.as_str().into());
            ui.set_battery(if bat.present { level } else { -1 });
            ui.set_charging(bat.charging);
            ui.set_battery_icon(battery_icon.into());
            ui.set_wifi_shown(w.present);
            ui.set_wifi_icon(wifi_icon.into());
            ui.set_wifi_label(wifi_label.as_str().into());
            ui.set_menu_open(self.app.menu.as_ref().is_some_and(|m| m.open && m.output == b.output));
            // This output's spaces: its own, at least five from its base
            let h = &self.app.hypr;
            let active = h.active.get(&b.output).copied().unwrap_or(0);
            let mut ids: Vec<i32> = h.workspaces.iter().filter(|w| w.monitor == b.output && w.id > 0).map(|w| w.id).collect();
            let base = if active > 0 { (active - 1) / 10 * 10 } else { 0 };
            let top = ids.iter().copied().max().unwrap_or(0).max(base + 5);
            ids.extend(base + 1..=top);
            ids.sort_unstable();
            ids.dedup();
            let spaces: Vec<ui::Space> = ids
                .iter()
                .map(|&id| ui::Space { id, occupied: h.workspaces.iter().any(|w| w.id == id && w.windows > 0) })
                .collect();
            let current = ids.iter().position(|&i| i == active).map_or(-1, |i| i as i32);
            let same = ui.get_spaces().iter().eq(spaces.iter().cloned());
            if !same {
                ui.set_spaces(ModelRc::new(VecModel::from(spaces)));
            }
            ui.set_current_space(current);
        }
    }

    /// Re-arm the clock for the next minute
    pub fn app_tick(&mut self) {
        self.app_refresh();
        // (on the minute, or every second when the clock shows seconds)
        let every_second = self.app.cfg["menubar"]["showSeconds"].as_bool().unwrap_or(false);
        let secs = now("%S").parse::<u64>().unwrap_or(0);
        let wait = if every_second { 1000 } else { (60 - secs.min(59)) * 1000 + 50 };
        let tx = self.tx.clone();
        self.app.clock.start(slint::TimerMode::SingleShot, std::time::Duration::from_millis(wait), move || {
            let _ = tx.send(crate::Msg::Tick);
        });
    }

    pub fn app_look_changed(&mut self) {
        self.app.cfg = theme::read_json();
        self.app_screens_changed();
        self.app_tick(); // (the clock's format and the menu icon may have changed)
        let look = theme::read();
        if look == self.app.look {
            return;
        }
        self.app.look = look;
        self.app_apply_theme();
    }

    /// `menubar.screens` changed: bars come and go to match
    fn app_screens_changed(&mut self) {
        let cfg = theme::read_json();
        let gone: Vec<String> = self.app.bars.iter().filter(|b| !crate::surfaces::on_screen(&cfg, &b.output)).map(|b| b.output.clone()).collect();
        for g in gone {
            self.app_output_removed(&g);
        }
        let outputs: Vec<(WlOutput, String)> = self.outputs.outputs().filter_map(|o| self.outputs.info(&o).and_then(|i| i.name).map(|n| (o, n))).collect();
        for (o, n) in outputs {
            self.app_output_added(o, n);
        }
    }

    pub fn app_apply_theme(&mut self) {
        for b in &mut self.app.bars {
            theme::apply(&b.ui.global::<ui::Theme>(), &self.app.look, self.app.plugin);
            b.glass_sent.clear();
        }
        if let Some(m) = &mut self.app.menu {
            theme::apply(&m.ui.global::<ui::Theme>(), &self.app.look, self.app.plugin);
            m.glass_sent.clear();
        }
    }

    // ── The system menu ──────────────────────────────────────────────────

    pub fn app_toggle_menu(&mut self, output: String) {
        if self.app.menu.as_ref().is_some_and(|m| m.open) {
            self.app_close_menu();
            return;
        }
        let Some(bar) = self.app.bars.iter().find(|b| b.output == output) else { return };
        let left = (bar.ui.get_logo_x() - SHADOW + 4.0).max(0.0) as i32;
        // (re)made on the output it opens on
        if self.app.menu.as_ref().is_some_and(|m| m.output != output) {
            let m = self.app.menu.take().unwrap();
            self.drop_surface(&m.surface);
        }
        if self.app.menu.is_none() {
            let wl_output = self.outputs.outputs().find(|o| self.outputs.info(o).and_then(|i| i.name).as_deref() == Some(output.as_str()));
            let Some(wl_output) = wl_output else { return };
            let surface = self.prepare_surface(Spec {
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
            theme::apply(&ui.global::<ui::Theme>(), &self.app.look, self.app.plugin);
            ui.set_rows(rows());
            let tx = self.tx.clone();
            ui.on_activated(move |i, text| {
                let _ = tx.send(crate::Msg::MenuActivated(i, text.to_string()));
            });
            let tx = self.tx.clone();
            ui.on_dismissed(move || {
                let _ = tx.send(crate::Msg::MenuDismissed);
            });
            let tx = self.tx.clone();
            ui.on_closed(move || {
                let _ = tx.send(crate::Msg::MenuClosed);
            });
            ui.show().expect("show menu");
            let a11y = crate::a11y::A11y::new(&surface, self.tx.clone());
            self.app.menu = Some(MenuEntry { output: output.clone(), surface, ui, a11y, open: false, glass_sent: String::new() });
        }
        let m = self.app.menu.as_mut().unwrap();
        m.surface.layer.set_margin(0, 0, 0, left);
        let want = (m.ui.get_panel_height() + 2.0 * SHADOW).ceil() as u32;
        m.surface.show(292, want);
        m.open = true;
        m.ui.set_shown(true);
        m.ui.invoke_open();
        self.app_refresh();
    }

    pub fn app_close_menu(&mut self) {
        if let Some(m) = &mut self.app.menu {
            if m.open {
                m.open = false;
                m.ui.set_shown(false); // the close animation, then MenuClosed hides it
            }
        }
        self.app_refresh();
    }

    pub fn app_menu_closed(&mut self) {
        if let Some(m) = &mut self.app.menu {
            if !m.open {
                m.surface.hide();
                self.app.glass.set(&m.output, MENU_NS, &[]);
                m.glass_sent.clear();
            }
        }
    }

    pub fn app_menu_activated(&mut self, row: i32, search: String) {
        let what = match row {
            0 => format!("search {search:?}"),
            1 => "about".into(),
            2 => "settings".into(),
            4 => "lock".into(), // (a prototype: nothing is really locked or put to sleep)
            5 => "sleep".into(),
            _ => return,
        };
        eprintln!("tessera-ui: menu chose {what}");
        self.app_close_menu();
    }

    /// The glass shapes for the plugin: the bar's strip, the menu's panel as
    /// it springs open (sent only when they change)
    pub fn app_sync_glass(&mut self) {
        if !self.app.plugin || self.app.look.material {
            return;
        }
        let look = self.app.look.clone();
        // (no background in Settings: no glass either, the bar is clear over
        // the wallpaper, as MenuBar.qml)
        let strip = crate::settings::bar_background(&self.app.cfg);
        for b in &mut self.app.bars {
            if !b.surface.mapped.get() {
                continue;
            }
            let (w, h) = b.surface.logical_size();
            let shapes: Vec<[f32; 19]> = if strip { vec![theme::glass_shape(&look, 0.0, 0.0, w as f32, h as f32, 0.0, 1.0, 1.0)] } else { Vec::new() };
            let key = format!("{shapes:?}");
            if key != b.glass_sent {
                self.app.glass.set(&b.output, BAR_NS, &shapes);
                b.glass_sent = key;
            }
        }
        if let Some(m) = &mut self.app.menu {
            if !m.surface.mapped.get() {
                return;
            }
            let ui = &m.ui;
            let open = ui.get_openness();
            let s = ui.get_panel_scale();
            let (px, py, pw, ph) = (ui.get_panel_x(), ui.get_panel_y(), ui.get_panel_width(), ui.get_panel_height());
            // scaled about (18, 0) in the panel, as in bar.slint
            let x = px + 18.0 * (1.0 - s);
            let opacity = (open * 1.6).min(1.0);
            let shape = theme::glass_shape(&look, x, py, pw * s, ph * s, 12.0 * s, opacity, 0.35);
            let key = format!("{shape:?}");
            if key != m.glass_sent {
                self.app.glass.set(&m.output, MENU_NS, if opacity > 0.002 { std::slice::from_ref(&shape) } else { &[] });
                m.glass_sent = key;
            }
        }
    }

    pub fn app_glass_plugin(&mut self, ok: bool) {
        eprintln!("tessera-ui: glass plugin {}", if ok { "found" } else { "missing (Hyprland's blur instead)" });
        self.app.plugin = ok;
        self.app_apply_theme();
    }

    /// On the way out: no glass left behind
    pub fn app_quit(&mut self) {
        for b in &self.app.bars {
            Glass::clear_now(&b.output, BAR_NS);
        }
        if let Some(m) = &self.app.menu {
            Glass::clear_now(&m.output, MENU_NS);
        }
    }

    pub fn app_dump_stats(&mut self) {
        for b in &self.app.bars {
            eprintln!("tessera-ui: stats {}", b.surface.stats.borrow_mut().report(&format!("bar:{}", b.output)));
        }
        if let Some(m) = &self.app.menu {
            eprintln!("tessera-ui: stats {}", m.surface.stats.borrow_mut().report("menu"));
        }
    }
}

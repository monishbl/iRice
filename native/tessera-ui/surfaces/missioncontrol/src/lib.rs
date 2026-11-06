//! Mission Control (rust.surfaces: missioncontrol), both looks:
//! shell/modules/missioncontrol/MissionControl.qml. The spaces along the top
//! (plain containers with a tile per window; drag one sideways to put it
//! elsewhere, click to go there), the current space's windows spread out
//! below as pictures, in the grid that keeps them largest; click one to
//! focus it, anywhere else (or Escape) to close. App Exposé (Panels'
//! exposeApp, the dock's "Show All Windows"): only one app's windows from
//! every space (and minimized ones), its name on top instead of the spaces.
//!
//! It opens through Panels ("missionControl", its panel name; RustUi.qml
//! maps it); RustWindows.qml tells it the app for App Exposé before it opens
//! (`{"to":"missioncontrol","expose":…}`) and the spaces' blocks.
//!
//! Window pictures: services::capture, once as it opens (Settings →
//! Overview → Window previews: Snapshot), or about 30 times a second while
//! it's open (Live and Hybrid: all of them are on the current space).
//!
//! Not like the QML yet: the glass is the plugin's full-screen glass at the
//! user's Glass settings only as far as theme::glass_shape follows them (see
//! docs/rust-ui-glass-compare.md); a space's drag doesn't lift it out of the
//! bar toward another display (the QML's doesn't either).

pub mod ui {
    slint::include_modules!();
}
tessera_core::theme_global!(ui);

// tessera-core's modules (shell, wayland, theme, services, util…) under the
// names they had when everything was one crate: `crate::shell::Part`
#[allow(unused_imports)]
use tessera_core::*;

pub mod layout;

use tessera_core::services::apps::Apps;
use tessera_core::services::capture::Shot;
use tessera_core::services::clients::{Client, Windows};
use tessera_core::services::hypr::{Glass, Snapshot};
use tessera_core::services::icons::Icons;
use tessera_core::shell::{Command, Part};
use tessera_core::wm::{self, spaces, AppIcons, IconsDone, LiveTick, OverviewCfg, Thumbs, Wall, WallPicture};
use tessera_core::theme;
use tessera_core::wayland::{Spec, State, Surface};
use layout::Rect;
use slint::platform::Key;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use smithay_client_toolkit::shell::WaylandSurface;
use std::any::Any;
use std::collections::HashMap;
use std::rc::Rc;

const NAME: &str = "missioncontrol";
pub const NS: &str = "tessera:missioncontrol";

enum M {
    Key(String),
    Pointer(String, f32, f32),
    Press(String, f32, f32, i32),
    Release(String, f32, f32, i32),
    Closed(String),
}

#[derive(Clone, Debug, Default)]
enum Press {
    #[default]
    None,
    Space { id: i32, from: f32, moved: bool },
    Window(String),
    Background,
}

struct Win {
    output: String,
    surface: Rc<Surface>,
    ui: ui::MissionControlPanel,
    a11y: Option<crate::a11y::A11y>,
    glass_sent: String,
    size: (f32, f32),
    cards: Vec<(i32, Rect)>,
    tiles: Vec<(String, Rect)>,
    press: Press,
    drag_space: i32,
    drop_on: i32,
}

#[derive(Default)]
pub struct MissionControl {
    wins: Vec<Win>,
    open: Option<String>,
    /// App Exposé: the app it shows (kept while it animates out)
    expose: String,
    /// what the shell said last (Panels.exposeApp)
    told: String,
    info: spaces::Info,
    icons: AppIcons,
    thumbs: Thumbs,
    wall: Wall,
}

fn send(st: &State) -> impl Fn(Box<dyn Any + Send>) + Clone + 'static {
    st.shell.sender(NAME)
}

/// A window's app is the one App Exposé is for (its class, or its entry's id)
pub fn of_app(apps: Option<&Apps>, class: &str, app: &str) -> bool {
    class == app || apps.and_then(|a| a.entry_for(class)).is_some_and(|e| e.id == app)
}

/// The windows it shows: one app's from every space (and set aside), or the
/// current space's
pub fn windows_for<'a>(wins: &'a Windows, apps: Option<&Apps>, expose: &str, active: i32) -> Vec<&'a Client> {
    wins.clients
        .iter()
        .filter(|c| c.size.0 > 0 && c.size.1 > 0)
        .filter(|c| if expose.is_empty() { c.workspace.id == active } else { (c.workspace.id > 0 || c.aside()) && of_app(apps, &c.class, expose) })
        .collect()
}

impl MissionControl {
    fn data(st: &State) -> (Windows, Snapshot) {
        (st.shell.get::<Windows>().cloned().unwrap_or_default(), st.shell.get::<Snapshot>().cloned().unwrap_or_default())
    }

    fn window(&mut self, st: &mut State, output: &str) -> Option<usize> {
        if let Some(i) = self.wins.iter().position(|w| w.output == output) {
            return Some(i);
        }
        let wl_output = st.output_named(output)?;
        let surface = st.prepare_surface(Spec {
            layer: Layer::Overlay,
            namespace: NS,
            output: Some((&wl_output, output)),
            anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
            size: (0, 0),
            exclusive: -1,
            keyboard: KeyboardInteractivity::None,
            margin_left: 0,
        });
        let ui = ui::MissionControlPanel::new().expect("MissionControlPanel");
        st.shell.apply_theme(&ui.global::<ui::Theme>());
        let to = send(st);
        {
            let to = to.clone();
            ui.on_key(move |t| {
                let esc = t.as_str() == SharedString::from(Key::Escape).as_str();
                if esc {
                    to(Box::new(M::Key(t.to_string())));
                }
                esc
            });
        }
        let o = output.to_string();
        {
            let (to, o) = (to.clone(), o.clone());
            ui.on_pointer(move |x, y, _| to(Box::new(M::Pointer(o.clone(), x, y))));
        }
        {
            let (to, o) = (to.clone(), o.clone());
            ui.on_press(move |x, y, b| to(Box::new(M::Press(o.clone(), x, y, b))));
        }
        {
            let (to, o) = (to.clone(), o.clone());
            ui.on_release(move |x, y, b| to(Box::new(M::Release(o.clone(), x, y, b))));
        }
        {
            let (to, o) = (to.clone(), o.clone());
            ui.on_closed(move || to(Box::new(M::Closed(o.clone()))));
        }
        ui.show().expect("show mission control");
        let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
        self.wins.push(Win { output: output.into(), surface, ui, a11y, glass_sent: String::new(), size: (1920.0, 1080.0), cards: Vec::new(), tiles: Vec::new(), press: Press::None, drag_space: 0, drop_on: 0 });
        let i = self.wins.len() - 1;
        self.look(st, i);
        Some(i)
    }

    fn look(&mut self, st: &State, i: usize) {
        let w = &self.wins[i];
        w.ui.set_glass_fill(crate::wm::glass_fill(st.shell.plugin, st.shell.look.dark));
        let t = w.ui.global::<ui::Theme>();
        self.icons.restyle(st, t.get_accent(), t.get_m3_primary_container(), t.get_m3_on_primary_container());
    }

    fn open(&mut self, st: &mut State, screen: Option<String>) {
        let (wins, _) = Self::data(st);
        let Some(out) = screen.or_else(|| wins.focused_monitor().map(|m| m.name.clone())).or_else(|| st.outputs_now().first().map(|o| o.1.clone())) else { return };
        if let Some(prev) = self.open.clone().filter(|p| *p != out) {
            self.hide_now(st, &prev);
        }
        let Some(i) = self.window(st, &out) else { return };
        self.open = Some(out);
        self.expose = self.told.clone();
        self.thumbs.retake();
        if OverviewCfg::read(&st.shell.cfg).wallpapers {
            self.wall.want(st, NAME, (320, 200));
        }
        let w = &mut self.wins[i];
        w.press = Press::None;
        w.drag_space = 0;
        w.drop_on = 0;
        w.ui.set_shown(true);
        w.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        st.set_input_region(&w.surface, None);
        w.surface.show(0, 0);
        w.ui.invoke_focus_keys();
        self.rebuild(st);
    }

    fn close(&mut self, st: &mut State, tell: bool) {
        let Some(out) = self.open.take() else { return };
        if let Some(w) = self.wins.iter().find(|w| w.output == out) {
            w.ui.set_shown(false);
            w.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::None);
            st.set_input_region(&w.surface, Some(&[]));
            w.surface.layer.commit();
        }
        if tell {
            crate::shell::closed(NAME);
        }
    }

    fn hide_now(&mut self, st: &mut State, out: &str) {
        if let Some(w) = self.wins.iter_mut().find(|w| w.output == out) {
            w.ui.set_shown(false);
            w.surface.hide();
            st.shell.glass.set(&w.output, NS, &[]);
            w.glass_sent.clear();
        }
    }

    fn closed(&mut self, st: &mut State, out: &str) {
        if self.open.as_deref() == Some(out) {
            return;
        }
        self.hide_now(st, out);
        if self.open.is_none() {
            self.thumbs.clear();
        }
    }

    fn rebuild(&mut self, st: &mut State) {
        let Some(out) = self.open.clone() else { return };
        let Some(i) = self.wins.iter().position(|w| w.output == out) else { return };
        let (wins, snap) = Self::data(st);
        let cfg = OverviewCfg::read(&st.shell.cfg);
        let apps = st.shell.get::<Apps>();
        let size = {
            let s = self.wins[i].surface.logical_size();
            if s.0 > 0 { (s.0 as f32, s.1 as f32) } else { wins.monitor(&out).map(|m| m.logical()).unwrap_or((1920.0, 1080.0)) }
        };
        let scale = self.wins[i].ui.window().scale_factor().max(1.0);
        let active = snap.active.get(&out).copied().unwrap_or(1);
        let mon = wins.monitor(&out).cloned().unwrap_or_default();
        // The spaces that exist, in this display's order
        let order = self.info.ids_for(&out, &snap);
        let ids: Vec<i32> = order.iter().copied().filter(|id| snap.workspaces.iter().any(|w| w.id == *id)).collect();
        let cards = layout::cards(ids.len(), size);
        let s = layout::CARD_W / size.0.max(1.0);
        let card_ui: Vec<ui::McCard> = ids
            .iter()
            .zip(&cards)
            .map(|(&id, r)| {
                let minis: Vec<ui::McMini> = wins
                    .clients
                    .iter()
                    .filter(|c| c.workspace.id == id && c.size.0 > 0)
                    .map(|c| {
                        let h = c.size.1 as f32 * s;
                        let icon = self.icons.get(st, &c.class, 20f32.min(h * 0.6), scale, false);
                        ui::McMini { x: (c.at.0 - mon.x) as f32 * s, y: (c.at.1 - mon.y) as f32 * s, w: c.size.0 as f32 * s, h, has_icon: icon.is_some(), icon: icon.unwrap_or_default() }
                    })
                    .collect();
                ui::McCard { id, x: r.x, w: r.w, h: r.h, title: self.info.title(id, &out, "Desktop").into(), current: id == active, minis: ModelRc::new(VecModel::from(minis)) }
            })
            .collect();
        let list = windows_for(&wins, apps, &self.expose, active);
        let stage = layout::stage(size);
        let sizes: Vec<(f32, f32)> = list.iter().map(|c| (c.size.0 as f32, c.size.1 as f32)).collect();
        let rects = layout::tiles(&sizes, stage);
        let live = self.open.is_some() && (cfg.previews == "live" || cfg.previews == "hybrid");
        let mut live_set = HashMap::new();
        let tiles: Vec<ui::McTile> = list
            .iter()
            .zip(&rects)
            .map(|(c, r)| {
                let max = ((r.w * scale).ceil() as u32, (r.h * scale).ceil() as u32);
                self.thumbs.ask(st, NAME, &c.address, max);
                if live {
                    live_set.insert(c.address.clone(), max);
                }
                let pic = self.thumbs.images.get(&c.address).cloned();
                let icon = self.icons.get(st, &c.class, 40.0, scale, false);
                let chip = self.icons.get(st, &c.class, 20.0, scale, false);
                ui::McTile {
                    address: c.address.as_str().into(),
                    x: r.x,
                    y: r.y,
                    w: r.w,
                    h: r.h,
                    has_picture: pic.is_some(),
                    picture: pic.unwrap_or_default(),
                    has_icon: icon.is_some(),
                    icon: icon.unwrap_or_default(),
                    chip_icon: chip.unwrap_or_default(),
                    title: if c.title.is_empty() { c.class.as_str().into() } else { c.title.as_str().into() },
                }
            })
            .collect();
        self.thumbs.set_live(st, NAME, live_set);
        // App Exposé's header
        let entry = apps.and_then(|a| a.by_id(&self.expose).or_else(|| a.entry_for(&self.expose))).cloned();
        let expose_icon = if self.expose.is_empty() { None } else { self.icons.get(st, entry.as_ref().map(|e| e.id.as_str()).unwrap_or(&self.expose), 40.0, scale, true) };
        self.icons.flush(send(st));
        let w = &mut self.wins[i];
        w.size = size;
        w.cards = ids.iter().copied().zip(cards.iter().copied()).collect();
        w.tiles = list.iter().map(|c| c.address.clone()).zip(rects.iter().copied()).collect();
        let ui = &w.ui;
        ui.set_cards(ModelRc::new(VecModel::from(card_ui)));
        ui.set_tiles(ModelRc::new(VecModel::from(tiles)));
        ui.set_stage_x(stage.x);
        ui.set_stage_y(stage.y);
        ui.set_stage_w(stage.w);
        ui.set_stage_h(stage.h);
        ui.set_current_ws(active);
        ui.set_expose(!self.expose.is_empty());
        ui.set_expose_name(entry.map(|e| e.name).filter(|n| !n.is_empty()).unwrap_or_else(|| self.expose.clone()).into());
        let n = list.len();
        ui.set_expose_count(format!("{n} {}", if n == 1 { "window" } else { "windows" }).into());
        ui.set_expose_icon(expose_icon.unwrap_or_default());
        ui.set_wallpapers(cfg.wallpapers);
        ui.set_wallpaper(self.wall.image.clone().unwrap_or_default());
    }

    fn push(w: &Win, hover: &str) {
        w.ui.set_hover_win(hover.into());
        w.ui.set_drag_space(w.drag_space);
        w.ui.set_drop_on(w.drop_on);
        w.ui.set_cursor(if w.drag_space > 0 { "grabbing".into() } else if !hover.is_empty() { "pointer".into() } else { "default".into() });
    }

    fn card_at(w: &Win, x: f32, y: f32, expose: bool) -> Option<i32> {
        if expose {
            return None;
        }
        w.cards.iter().find(|(_, r)| Rect { h: r.h + 6.0 + layout::TITLE_H, ..*r }.has(x, y)).map(|(id, _)| *id)
    }

    fn tile_at(w: &Win, x: f32, y: f32) -> Option<String> {
        w.tiles.iter().rev().find(|(_, r)| r.has(x, y)).map(|(a, _)| a.clone())
    }

    fn pointer(&mut self, out: &str, x: f32, y: f32) {
        let expose = !self.expose.is_empty();
        let Some(w) = self.wins.iter_mut().find(|w| w.output == out) else { return };
        if self.open.as_deref() != Some(out) {
            return;
        }
        if let Press::Space { id, from, moved } = w.press.clone() {
            let dx = x - from;
            if moved || dx.abs() > 8.0 {
                w.press = Press::Space { id, from, moved: true };
                w.drag_space = id;
                w.ui.set_drag_dx(dx);
                let cx = w.cards.iter().find(|(c, _)| *c == id).map(|(_, r)| r.x + r.w / 2.0 + dx).unwrap_or(x);
                w.drop_on = w.cards.iter().min_by(|a, b| (a.1.x + a.1.w / 2.0 - cx).abs().total_cmp(&(b.1.x + b.1.w / 2.0 - cx).abs())).map(|(c, _)| *c).unwrap_or(0);
            }
            Self::push(w, "");
            return;
        }
        let hover = Self::tile_at(w, x, y).unwrap_or_default();
        let over_card = Self::card_at(w, x, y, expose).is_some();
        Self::push(w, &hover);
        if over_card {
            w.ui.set_cursor("pointer".into());
        }
    }

    fn press(&mut self, out: &str, x: f32, y: f32, b: i32) {
        let expose = !self.expose.is_empty();
        let Some(w) = self.wins.iter_mut().find(|w| w.output == out) else { return };
        if b != 0 {
            return;
        }
        w.press = if let Some(id) = Self::card_at(w, x, y, expose) {
            Press::Space { id, from: x, moved: false }
        } else if let Some(a) = Self::tile_at(w, x, y) {
            Press::Window(a)
        } else {
            Press::Background
        };
    }

    fn release(&mut self, st: &mut State, out: &str, x: f32, y: f32, b: i32) {
        let Some(i) = self.wins.iter().position(|w| w.output == out) else { return };
        if b == 1 || b == 2 {
            return;
        }
        let press = std::mem::take(&mut self.wins[i].press);
        let cancel = b == 3;
        match press {
            Press::Space { id, moved, .. } => {
                let w = &mut self.wins[i];
                if moved {
                    if !cancel && w.drop_on > 0 && w.drop_on != id {
                        let order: Vec<i32> = w.cards.iter().map(|(c, _)| *c).collect();
                        wm::ask("spaces.move", serde_json::json!([order, id, w.drop_on]));
                    }
                    w.drag_space = 0;
                    w.drop_on = 0;
                    w.ui.set_drag_dx(0.0);
                    Self::push(w, "");
                } else if !cancel {
                    crate::services::hypr::focus_workspace(id);
                }
            }
            Press::Window(a) => {
                if !cancel && Self::tile_at(&self.wins[i], x, y).as_deref() == Some(a.as_str()) {
                    self.focus_window(st, &a);
                }
            }
            Press::Background => {
                if !cancel {
                    self.close(st, true);
                }
            }
            Press::None => {}
        }
    }

    /// MissionControl.focusWindow: a minimized (or set aside) one comes
    /// back; one on another space (App Exposé) takes you there
    fn focus_window(&mut self, st: &mut State, address: &str) {
        self.close(st, true);
        let (wins, _) = Self::data(st);
        match wins.clients.iter().find(|c| c.address == address) {
            Some(c) if c.aside() => wm::ask("apps.show", serde_json::json!([address])),
            _ => crate::services::clients::focus(address),
        }
    }

    fn glass(&mut self, st: &mut State) {
        let look = st.shell.look.clone();
        for w in &mut self.wins {
            if !w.surface.mapped.get() || !st.shell.plugin || look.material {
                continue;
            }
            let p = w.ui.get_progress().clamp(0.0, 1.0);
            // borderless: no lens, no rim (MissionControl.qml's Glass)
            let mut s = theme::glass_shape(&look, 0.0, 0.0, w.size.0, w.size.1, 0.0, p, 0.0);
            s[11] = 0.0;
            let shapes = [s];
            let key = format!("{shapes:?}");
            if key != w.glass_sent {
                st.shell.glass.set(&w.output, NS, &shapes);
                w.glass_sent = key;
            }
        }
    }
}

impl Part for MissionControl {
    fn name(&self) -> &'static str {
        NAME
    }

    fn start(&mut self, st: &mut State) {
        for s in ["clients", "hypr", "apps", "icons"] {
            st.shell.want(s);
        }
        self.info.count = wm::workspace_count(&st.shell.cfg);
    }

    fn command(&mut self, st: &mut State, cmd: &Command) {
        match cmd.cmd.as_str() {
            "open" => self.open(st, cmd.screen.clone()),
            "close" => self.close(st, false),
            _ => {}
        }
    }

    fn config_changed(&mut self, st: &mut State, look: bool) {
        self.info.count = wm::workspace_count(&st.shell.cfg);
        for i in 0..self.wins.len() {
            if look {
                st.shell.apply_theme(&self.wins[i].ui.global::<ui::Theme>());
                self.wins[i].glass_sent.clear();
            }
            self.look(st, i);
        }
        self.rebuild(st);
    }

    fn plugin_changed(&mut self, st: &mut State) {
        for i in 0..self.wins.len() {
            st.shell.apply_theme(&self.wins[i].ui.global::<ui::Theme>());
            self.wins[i].glass_sent.clear();
            self.look(st, i);
        }
    }

    fn data(&mut self, st: &mut State, what: std::any::TypeId) {
        let busy = self.wins.iter().any(|w| !matches!(w.press, Press::None));
        if self.open.is_some() && !busy && (what == std::any::TypeId::of::<Windows>() || what == std::any::TypeId::of::<Snapshot>() || what == std::any::TypeId::of::<Icons>() || what == std::any::TypeId::of::<Apps>()) {
            self.rebuild(st);
        }
    }

    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        let msg = match msg.downcast::<serde_json::Value>() {
            Ok(v) => {
                if v["spaces"].is_object() {
                    self.info = spaces::Info::from_json(&v["spaces"], wm::workspace_count(&st.shell.cfg));
                    self.rebuild(st);
                }
                if let Some(app) = v["expose"].as_str() {
                    self.told = app.to_string();
                    // (opened already: it switches to that app's windows)
                    if !app.is_empty() && self.open.is_some() {
                        self.expose = app.to_string();
                        self.rebuild(st);
                    }
                }
                return;
            }
            Err(m) => m,
        };
        let msg = match msg.downcast::<Shot>() {
            Ok(s) => {
                if self.thumbs.got(*s) {
                    self.rebuild(st);
                }
                return;
            }
            Err(m) => m,
        };
        let msg = match msg.downcast::<IconsDone>() {
            Ok(d) => {
                self.icons.done(*d);
                self.rebuild(st);
                return;
            }
            Err(m) => m,
        };
        let msg = match msg.downcast::<LiveTick>() {
            Ok(_) => {
                self.thumbs.tick(st, NAME);
                return;
            }
            Err(m) => m,
        };
        let msg = match msg.downcast::<WallPicture>() {
            Ok(p) => {
                self.wall.got(*p);
                self.rebuild(st);
                return;
            }
            Err(m) => m,
        };
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Key(_) => self.close(st, true),
            M::Pointer(o, x, y) => self.pointer(&o, x, y),
            M::Press(o, x, y, b) => self.press(&o, x, y, b),
            M::Release(o, x, y, b) => self.release(st, &o, x, y, b),
            M::Closed(o) => self.closed(st, &o),
        }
    }

    fn keyboard_left(&mut self, st: &mut State, s: &Rc<Surface>) {
        if self.wins.iter().any(|w| Rc::ptr_eq(&w.surface, s) && self.open.as_deref() == Some(w.output.as_str())) {
            self.close(st, true);
        }
    }

    fn output_removed(&mut self, st: &mut State, name: &str) {
        if let Some(i) = self.wins.iter().position(|w| w.output == name) {
            if self.open.as_deref() == Some(name) {
                self.close(st, true);
            }
            let w = self.wins.remove(i);
            Glass::clear_now(&w.output, NS);
            st.drop_surface(&w.surface);
        }
    }

    fn surface_closed(&mut self, st: &mut State, s: &Rc<Surface>) {
        if let Some(n) = self.wins.iter().find(|w| Rc::ptr_eq(&w.surface, s)).map(|w| w.output.clone()) {
            self.output_removed(st, &n);
        }
    }

    fn after_frame(&mut self, st: &mut State) {
        let kbd = st.keyboard_surface();
        for w in &mut self.wins {
            let f = kbd.as_ref().is_some_and(|k| Rc::ptr_eq(k, &w.surface));
            if let Some(a) = w.a11y.as_mut() {
                a.update(false, f);
            }
        }
        self.glass(st);
    }

    fn a11y(&mut self) -> Vec<&mut crate::a11y::A11y> {
        self.wins.iter_mut().filter_map(|w| w.a11y.as_mut()).collect()
    }

    fn quit(&mut self, _st: &mut State) {
        for w in &self.wins {
            Glass::clear_now(&w.output, NS);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::apps::Entry;
    use crate::services::clients::WsRef;

    fn c(addr: &str, class: &str, ws: (i32, &str)) -> Client {
        Client { address: addr.into(), class: class.into(), size: (800, 600), workspace: WsRef { id: ws.0, name: ws.1.into() }, ..Default::default() }
    }

    #[test]
    fn the_current_space_or_one_apps_windows() {
        let w = Windows { clients: vec![c("1", "foot", (1, "1")), c("2", "firefox", (1, "1")), c("3", "foot", (2, "2")), c("4", "foot", (-99, "special:minimized")), c("5", "foot", (-98, "special:scratch"))], ..Default::default() };
        let addrs = |l: Vec<&Client>| l.iter().map(|c| c.address.clone()).collect::<Vec<_>>();
        assert_eq!(addrs(windows_for(&w, None, "", 1)), vec!["1", "2"]);
        // App Exposé: from every space, minimized too, not a scratchpad's
        assert_eq!(addrs(windows_for(&w, None, "foot", 1)), vec!["1", "3", "4"]);
        // by the app's desktop id too
        let apps = Apps::from_entries(vec![Entry { id: "org.codeberg.dnkl.foot".into(), name: "Foot".into(), wm_class: "foot".into(), ..Default::default() }]);
        assert_eq!(addrs(windows_for(&w, Some(&apps), "org.codeberg.dnkl.foot", 1)), vec!["1", "3", "4"]);
    }
}

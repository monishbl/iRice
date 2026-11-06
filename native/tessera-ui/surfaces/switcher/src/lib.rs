//! The window switcher (rust.surfaces: switcher; hold Alt, press Tab), both
//! looks: shell/modules/switcher/Switcher.qml. A centred glass strip of
//! window pictures, most recently focused first, from every space (and
//! minimized ones). Tab / Shift+Tab (or the arrows) move the selection;
//! releasing Alt focuses it, Escape cancels, Enter or a click focuses. A
//! quick tap switches straight to the previous window: the strip shows only
//! once Alt is held 150 ms (or on a second Tab), so nothing flashes.
//!
//! Tab presses still arrive as the shell's global shortcuts
//! (tessera:switcher, switcherBack, switcherCommit: Hyprland eats the bound
//! keys); Switcher.qml forwards them here as commands (`step` with `d`,
//! `commit`) while this draws it. The first press maps the layer at once,
//! transparent and with no input region, holding the keyboard, so the
//! compositor sends Alt's release here wherever it went down. If the
//! keyboard arrives only after Alt is already up (a very quick tap), the
//! strip doesn't wait: it switches when its 150 ms are up.
//! Registers with Panels (`wm panels.show switcher`) once it shows; another
//! panel opening (a `close` command), or the screen locking, cancels it.
//! Pictures are taken once, when the strip appears, never live.
//!
//! Not like the QML yet: nothing found (the QML's GridView momentum aside).

pub mod ui {
    slint::include_modules!();
}
tessera_core::theme_global!(ui);

// tessera-core's modules (shell, wayland, theme, services, util…) under the
// names they had when everything was one crate: `crate::shell::Part`
#[allow(unused_imports)]
use tessera_core::*;

use tessera_core::services::apps::Apps;
use tessera_core::services::capture::Shot;
use tessera_core::services::clients::{Client, Windows};
use tessera_core::services::hypr::{Glass, Snapshot};
use tessera_core::shell::{Command, Part};
use tessera_core::wm::{self, spaces, AppIcons, IconsDone, Thumbs};
use tessera_core::theme;
use tessera_core::wayland::{Spec, State, Surface};
use slint::platform::Key;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use smithay_client_toolkit::shell::WaylandSurface;
use std::any::Any;
use std::rc::Rc;
use std::time::Duration;

const NAME: &str = "switcher";
pub const NS: &str = "tessera:switcher";
const HOLD_MS: u64 = 150;
const MARGIN: f32 = 24.0;
const PAD: f32 = 12.0;
const GAP: f32 = 8.0;
const CELL_W: f32 = 208.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Pending,
    Shown,
}

enum M {
    Key(String),
    Released(String),
    Picked(i32),
    Faded,
    Hold,
    Focus(String),
}

/// Every window on a regular space, plus minimized (or set aside) ones; not
/// other special workspaces (scratchpads), most recently focused first
pub fn candidates(w: &Windows) -> Vec<Client> {
    let mut list: Vec<(usize, Client)> = w.clients.iter().filter(|c| c.workspace.id > 0 || c.aside()).cloned().enumerate().collect();
    list.sort_by_key(|(i, c)| (c.focus_history_id, *i));
    list.into_iter().map(|(_, c)| c).collect()
}

/// Where the selection starts: forward, the window behind the focused one;
/// backward, the last
pub fn first_selected(list: &[Client], active: &str, dir: i32) -> usize {
    if list.is_empty() {
        return 0;
    }
    if dir > 0 {
        if list[0].address == active {
            1.min(list.len() - 1)
        } else {
            0
        }
    } else {
        list.len() - 1
    }
}

pub fn step(sel: usize, d: i32, n: usize) -> usize {
    if n == 0 {
        return 0;
    }
    ((sel as i32 + d).rem_euclid(n as i32)) as usize
}

/// Cells per row and rows in view for this many windows on this screen
pub fn grid(count: usize, screen: (f32, f32), material: bool) -> (usize, usize, usize) {
    let cell_h = cell_h(material);
    let per_row = ((screen.0 - 96.0 - 2.0 * MARGIN - 2.0 * PAD + GAP) / (CELL_W + GAP)).floor().max(1.0) as usize;
    let per_row = per_row.min(count).max(1);
    let rows = count.div_ceil(per_row);
    let fits = ((screen.1 - 160.0 - 2.0 * MARGIN - 2.0 * PAD + GAP) / (cell_h + GAP)).floor().max(1.0) as usize;
    (per_row, rows, rows.min(fits).max(1))
}

pub fn cell_h(material: bool) -> f32 {
    if material {
        8.0 + 28.0 + 8.0 + 120.0 + 12.0
    } else {
        8.0 + 120.0 + 8.0 + 20.0 + 10.0
    }
}

/// The body: an empty one is 240×72
pub fn body(count: usize, screen: (f32, f32), material: bool) -> (f32, f32) {
    if count == 0 {
        return (240.0, 72.0);
    }
    let (per_row, _, visible) = grid(count, screen, material);
    (per_row as f32 * (CELL_W + GAP) - GAP + 2.0 * PAD, visible as f32 * (cell_h(material) + GAP) - GAP + 2.0 * PAD)
}

/// A window fitted into the 192×120 picture area
pub fn card(size: (i32, i32)) -> (f32, f32) {
    let (w, h) = (if size.0 > 0 { size.0 as f32 } else { 16.0 }, if size.1 > 0 { size.1 as f32 } else { 10.0 });
    let k = ((CELL_W - 16.0) / w).min(120.0 / h);
    ((w * k).round().max(32.0), (h * k).round().max(24.0))
}

struct Win {
    output: String,
    surface: Rc<Surface>,
    ui: ui::SwitcherStrip,
    a11y: Option<crate::a11y::A11y>,
    glass_sent: String,
    size: (f32, f32),
}

pub struct Switcher {
    win: Option<Win>,
    phase: Phase,
    items: Vec<Client>,
    selected: usize,
    output: String,
    /// Alt went down while it had the keyboard
    alt_down: bool,
    registered: bool,
    locked: bool,
    info: spaces::Info,
    icons: AppIcons,
    thumbs: Thumbs,
    hold: slint::Timer,
    later: slint::Timer,
}

impl Default for Switcher {
    fn default() -> Self {
        Self {
            win: None,
            phase: Phase::Idle,
            items: Vec::new(),
            selected: 0,
            output: String::new(),
            alt_down: false,
            registered: false,
            locked: false,
            info: spaces::Info::default(),
            icons: AppIcons::default(),
            thumbs: Thumbs::default(),
            hold: slint::Timer::default(),
            later: slint::Timer::default(),
        }
    }
}

fn send(st: &State) -> impl Fn(Box<dyn Any + Send>) + Clone + 'static {
    st.shell.sender(NAME)
}

fn is(text: &str, k: Key) -> bool {
    text == SharedString::from(k).as_str()
}

impl Switcher {
    fn enabled(st: &State) -> bool {
        st.shell.cfg["switcher"]["enabled"].as_bool().unwrap_or(true)
    }

    fn window(&mut self, st: &mut State, output: &str) -> bool {
        if self.win.as_ref().is_some_and(|w| w.output != output) {
            let w = self.win.take().unwrap();
            Glass::clear_now(&w.output, NS);
            st.drop_surface(&w.surface);
        }
        if self.win.is_some() {
            return true;
        }
        let Some(wl_output) = st.output_named(output) else { return false };
        let surface = st.prepare_surface(Spec {
            layer: Layer::Overlay,
            namespace: NS,
            output: Some((&wl_output, output)),
            anchor: Anchor::empty(),
            size: (288, 120),
            exclusive: 0,
            keyboard: KeyboardInteractivity::None,
            margin_left: 0,
        });
        let ui = ui::SwitcherStrip::new().expect("SwitcherStrip");
        st.shell.apply_theme(&ui.global::<ui::Theme>());
        let to = send(st);
        {
            let to = to.clone();
            ui.on_key(move |t| {
                let ok = [Key::Escape, Key::Return, Key::Tab, Key::Backtab, Key::LeftArrow, Key::RightArrow, Key::UpArrow, Key::DownArrow, Key::Alt, Key::Meta].iter().any(|k| is(&t, *k));
                if ok {
                    to(Box::new(M::Key(t.to_string())));
                }
                ok
            });
        }
        {
            let to = to.clone();
            ui.on_released(move |t| to(Box::new(M::Released(t.to_string()))));
        }
        {
            let to = to.clone();
            ui.on_picked(move |i| to(Box::new(M::Picked(i))));
        }
        ui.on_faded(move || to(Box::new(M::Faded)));
        ui.show().expect("show switcher");
        let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
        self.win = Some(Win { output: output.into(), surface, ui, a11y, glass_sent: String::new(), size: (288.0, 120.0) });
        self.look(st);
        true
    }

    fn look(&mut self, st: &State) {
        let Some(w) = &self.win else { return };
        w.ui.set_glass_fill(crate::wm::glass_fill(st.shell.plugin, st.shell.look.dark));
        let t = w.ui.global::<ui::Theme>();
        self.icons.restyle(st, t.get_accent(), t.get_m3_primary_container(), t.get_m3_on_primary_container());
    }

    /// The first press: the windows, the selection, the layer mapped at once
    fn begin(&mut self, st: &mut State, dir: i32) {
        if !Self::enabled(st) || self.locked {
            return;
        }
        let wins = st.shell.get::<Windows>().cloned().unwrap_or_default();
        self.items = candidates(&wins);
        self.selected = first_selected(&self.items, &wins.active, dir);
        let Some(out) = wins.focused_monitor().map(|m| m.name.clone()).or_else(|| st.outputs_now().first().map(|o| o.1.clone())) else { return };
        // (a popover may hold the keyboard: it closes)
        wm::ask("panels.closeOpen", serde_json::json!([]));
        if !self.window(st, &out) {
            return;
        }
        self.output = out;
        self.phase = Phase::Pending;
        self.alt_down = false;
        self.thumbs.clear();
        self.layout(st);
        let w = self.win.as_ref().unwrap();
        w.ui.set_shown(false);
        w.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        st.set_input_region(&w.surface, Some(&[]));
        w.surface.show(w.size.0 as u32, w.size.1 as u32);
        w.ui.invoke_grab_keys();
        let to = send(st);
        self.hold.start(slint::TimerMode::SingleShot, Duration::from_millis(HOLD_MS), move || to(Box::new(M::Hold)));
    }

    fn step(&mut self, st: &mut State, d: i32) {
        if self.phase == Phase::Idle {
            return self.begin(st, d);
        }
        self.selected = step(self.selected, d, self.items.len());
        self.reveal(st);
        self.push(st);
    }

    /// The strip shows (held long enough, or a second Tab)
    fn reveal(&mut self, st: &mut State) {
        self.hold.stop();
        if self.phase != Phase::Pending {
            return;
        }
        self.phase = Phase::Shown;
        self.layout(st);
        if let Some(w) = &self.win {
            w.ui.set_shown(true);
            let (bw, bh) = (w.ui.get_body_w(), w.ui.get_body_h());
            st.set_input_region(&w.surface, Some(&[(MARGIN as i32, MARGIN as i32, bw.ceil() as i32, bh.ceil() as i32)]));
        }
        wm::ask("panels.show", serde_json::json!(["switcher", self.output]));
        self.registered = true;
        self.push(st);
    }

    fn commit(&mut self, st: &mut State) {
        if self.phase == Phase::Idle {
            return;
        }
        let t = self.items.get(self.selected).cloned();
        self.finish(st);
        if let Some(t) = t {
            // after the strip has given the keyboard back (Hyprland ignores a
            // focus change while a layer holds it)
            let to = send(st);
            let a = t.address.clone();
            self.later.start(slint::TimerMode::SingleShot, Duration::from_millis(50), move || to(Box::new(M::Focus(a.clone()))));
        }
    }

    fn finish(&mut self, st: &mut State) {
        self.hold.stop();
        self.phase = Phase::Idle;
        if let Some(w) = &self.win {
            w.ui.set_shown(false);
            w.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::None);
            st.set_input_region(&w.surface, Some(&[]));
            w.surface.layer.commit();
            // (nothing was showing: no fade to wait for)
            if w.ui.get_progress() <= 0.001 {
                w.surface.hide();
            }
        }
        if self.registered {
            self.registered = false;
            crate::shell::closed(NAME);
        }
    }

    fn focus(&mut self, st: &State, address: &str) {
        let wins = st.shell.get::<Windows>().cloned().unwrap_or_default();
        match wins.clients.iter().find(|c| c.address == address) {
            Some(c) if c.aside() => wm::ask("apps.show", serde_json::json!([address])),
            _ => crate::services::clients::focus(address),
        }
    }

    /// The window's size for this many windows
    fn layout(&mut self, st: &State) {
        let m = st.shell.look.material;
        let wins = st.shell.get::<Windows>().cloned().unwrap_or_default();
        let screen = wins.monitor(&self.output).map(|mo| mo.logical()).unwrap_or((1920.0, 1080.0));
        let (bw, bh) = body(self.items.len(), screen, m);
        let (per_row, _, _) = grid(self.items.len(), screen, m);
        let Some(w) = &mut self.win else { return };
        w.ui.set_body_w(bw);
        w.ui.set_body_h(bh);
        w.ui.set_per_row(per_row as i32);
        let size = (bw + 2.0 * MARGIN, bh + 2.0 * MARGIN);
        if size != w.size && w.surface.mapped.get() {
            w.surface.layer.set_size(size.0 as u32, size.1 as u32);
            w.surface.layer.commit();
        }
        w.size = size;
    }

    /// The tiles (pictures once it shows) and the selection into the strip
    fn push(&mut self, st: &mut State) {
        if self.win.is_none() {
            return;
        }
        let shown = self.phase == Phase::Shown || self.win.as_ref().is_some_and(|w| w.ui.get_progress() > 0.0);
        let scale = self.win.as_ref().unwrap().ui.window().scale_factor().max(1.0);
        let snap = st.shell.get::<Snapshot>().cloned().unwrap_or_default();
        let here = snap.active.get(&self.output).copied().unwrap_or(0);
        let tiles: Vec<ui::SwTile> = if !shown {
            Vec::new()
        } else {
            self.items
                .iter()
                .map(|c| {
                    let (cw, chh) = card(c.size);
                    if self.phase == Phase::Shown {
                        self.thumbs.ask(st, NAME, &c.address, ((cw * scale).ceil() as u32, (chh * scale).ceil() as u32));
                    }
                    let entry = st.shell.get::<Apps>().and_then(|a| a.entry_for(&c.class));
                    let key = entry.map(|e| e.id.clone()).unwrap_or_else(|| c.class.clone());
                    let icon = self.icons.get(st, &key, 48.0, scale, true);
                    let small = self.icons.get(st, &key, 16.0, scale, true);
                    let pic = self.thumbs.images.get(&c.address).cloned();
                    let title = [c.title.clone(), entry.map(|e| e.name.clone()).unwrap_or_default(), c.class.clone(), "Window".into()].into_iter().find(|s| !s.is_empty()).unwrap();
                    let place = if c.minimized() {
                        "Minimized".to_string()
                    } else if c.workspace.id > 0 && c.workspace.id != here {
                        self.info.title(c.workspace.id, &self.output, "Space")
                    } else {
                        String::new()
                    };
                    ui::SwTile {
                        address: c.address.as_str().into(),
                        title: title.into(),
                        place: place.into(),
                        has_picture: pic.is_some(),
                        picture: pic.unwrap_or_default(),
                        has_icon: icon.is_some(),
                        icon: icon.unwrap_or_default(),
                        small: small.unwrap_or_default(),
                        card_w: cw,
                        card_h: chh,
                    }
                })
                .collect()
        };
        self.icons.flush(send(st));
        let m = st.shell.look.material;
        let w = self.win.as_ref().unwrap();
        w.ui.set_tiles(ModelRc::new(VecModel::from(tiles)));
        w.ui.set_selected(self.selected as i32);
        // keep the selection's row in view
        let per_row = w.ui.get_per_row().max(1) as usize;
        let row_h = cell_h(m) + GAP;
        let top = (self.selected / per_row) as f32 * row_h;
        let view = w.ui.get_body_h() - 2.0 * PAD;
        let cur = -w.ui.get_scroll_y();
        if top < cur {
            w.ui.set_scroll_y(-top);
        } else if top + row_h - GAP > cur + view {
            w.ui.set_scroll_y(-(top + row_h - GAP - view));
        }
    }

    fn key(&mut self, st: &mut State, t: &str) {
        if is(t, Key::Alt) || is(t, Key::Meta) {
            self.alt_down = true;
            return;
        }
        if self.phase == Phase::Idle {
            return;
        }
        let per_row = self.win.as_ref().map(|w| w.ui.get_per_row().max(1)).unwrap_or(1);
        if is(t, Key::Escape) {
            self.finish(st);
        } else if is(t, Key::Return) {
            self.commit(st);
        } else if is(t, Key::Tab) || is(t, Key::RightArrow) {
            self.step(st, 1);
        } else if is(t, Key::Backtab) || is(t, Key::LeftArrow) {
            self.step(st, -1);
        } else if is(t, Key::DownArrow) {
            self.step(st, per_row);
        } else if is(t, Key::UpArrow) {
            self.step(st, -per_row);
        }
    }

    fn glass(&mut self, st: &mut State) {
        let Some(w) = &mut self.win else { return };
        if !w.surface.mapped.get() || !st.shell.plugin || st.shell.look.material {
            return;
        }
        let p = w.ui.get_progress().clamp(0.0, 1.0);
        let (bw, bh) = (w.ui.get_body_w(), w.ui.get_body_h());
        let k = 0.97 + 0.03 * p;
        let (cx, cy) = (MARGIN + bw / 2.0, MARGIN + bh / 2.0);
        let shapes = if p > 0.0 { vec![theme::glass_shape(&st.shell.look, cx - bw * k / 2.0, cy - bh * k / 2.0, bw * k, bh * k, 26.0 * k, p, 1.0)] } else { Vec::new() };
        let key = format!("{shapes:?}");
        if key != w.glass_sent {
            st.shell.glass.set(&w.output, NS, &shapes);
            w.glass_sent = key;
        }
    }
}

impl Part for Switcher {
    fn name(&self) -> &'static str {
        NAME
    }

    fn start(&mut self, st: &mut State) {
        for s in ["clients", "hypr", "apps", "icons"] {
            st.shell.want(s);
        }
    }

    fn command(&mut self, st: &mut State, cmd: &Command) {
        match cmd.cmd.as_str() {
            "step" => {
                let d = cmd.args["d"].as_i64().unwrap_or(1) as i32;
                self.step(st, d);
            }
            "commit" => self.commit(st),
            "cancel" => self.finish(st),
            // Panels went to another panel (or Escape-all): it's gone without switching
            "close" => {
                if self.phase == Phase::Shown {
                    self.registered = false;
                    self.finish(st);
                }
            }
            _ => {}
        }
    }

    fn config_changed(&mut self, st: &mut State, look: bool) {
        if let Some(w) = &mut self.win {
            if look {
                st.shell.apply_theme(&w.ui.global::<ui::Theme>());
                w.glass_sent.clear();
            }
        }
        self.look(st);
        self.info.count = wm::workspace_count(&st.shell.cfg);
    }

    fn plugin_changed(&mut self, st: &mut State) {
        if let Some(w) = &mut self.win {
            st.shell.apply_theme(&w.ui.global::<ui::Theme>());
            w.glass_sent.clear();
        }
        self.look(st);
    }

    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        let msg = match msg.downcast::<serde_json::Value>() {
            Ok(v) => {
                if v["spaces"].is_object() {
                    self.info = spaces::Info::from_json(&v["spaces"], wm::workspace_count(&st.shell.cfg));
                }
                if let Some(l) = v["locked"].as_bool() {
                    self.locked = l;
                    if l && self.phase != Phase::Idle {
                        self.finish(st);
                    }
                }
                return;
            }
            Err(m) => m,
        };
        let msg = match msg.downcast::<Shot>() {
            Ok(s) => {
                if self.thumbs.got(*s) && self.phase == Phase::Shown {
                    self.push(st);
                }
                return;
            }
            Err(m) => m,
        };
        let msg = match msg.downcast::<IconsDone>() {
            Ok(d) => {
                self.icons.done(*d);
                if self.phase == Phase::Shown {
                    self.push(st);
                }
                return;
            }
            Err(m) => m,
        };
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Key(t) => self.key(st, &t),
            M::Released(t) => {
                if is(&t, Key::Alt) || is(&t, Key::Meta) {
                    self.alt_down = false;
                    self.commit(st);
                }
            }
            M::Picked(i) => {
                self.selected = i.max(0) as usize;
                self.commit(st);
            }
            M::Hold => {
                // the keyboard came, but Alt was already up: a quick tap
                let ours = st.keyboard_surface().is_some_and(|k| self.win.as_ref().is_some_and(|w| Rc::ptr_eq(&k, &w.surface)));
                if self.phase == Phase::Pending && ours && !self.alt_down {
                    self.commit(st);
                } else {
                    self.reveal(st);
                }
            }
            M::Faded => {
                if self.phase == Phase::Idle {
                    if let Some(w) = &mut self.win {
                        w.surface.hide();
                        st.shell.glass.set(&w.output, NS, &[]);
                        w.glass_sent.clear();
                        w.ui.set_tiles(ModelRc::default());
                    }
                    self.thumbs.clear();
                }
            }
            M::Focus(a) => self.focus(st, &a),
        }
    }

    fn keyboard_left(&mut self, st: &mut State, s: &Rc<Surface>) {
        // (Super+Escape, the lock…: the keyboard went elsewhere)
        if self.phase == Phase::Shown && self.win.as_ref().is_some_and(|w| Rc::ptr_eq(&w.surface, s)) {
            self.finish(st);
        }
    }

    fn output_removed(&mut self, st: &mut State, name: &str) {
        if self.win.as_ref().is_some_and(|w| w.output == name) {
            if self.phase != Phase::Idle {
                self.finish(st);
            }
            let w = self.win.take().unwrap();
            st.drop_surface(&w.surface);
        }
    }

    fn surface_closed(&mut self, st: &mut State, s: &Rc<Surface>) {
        if let Some(n) = self.win.as_ref().filter(|w| Rc::ptr_eq(&w.surface, s)).map(|w| w.output.clone()) {
            self.output_removed(st, &n);
        }
    }

    fn after_frame(&mut self, st: &mut State) {
        let kbd = st.keyboard_surface();
        if let Some(w) = &mut self.win {
            let f = kbd.as_ref().is_some_and(|k| Rc::ptr_eq(k, &w.surface));
            if let Some(a) = w.a11y.as_mut() {
                a.update(false, f);
            }
        }
        self.glass(st);
    }

    fn a11y(&mut self) -> Vec<&mut crate::a11y::A11y> {
        self.win.iter_mut().filter_map(|w| w.a11y.as_mut()).collect()
    }

    fn quit(&mut self, _st: &mut State) {
        if let Some(w) = &self.win {
            Glass::clear_now(&w.output, NS);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::clients::WsRef;

    fn c(addr: &str, ws: (i32, &str), recent: i32) -> Client {
        Client { address: addr.into(), workspace: WsRef { id: ws.0, name: ws.1.into() }, focus_history_id: recent, ..Default::default() }
    }

    #[test]
    fn most_recent_first_minimized_in_scratchpads_out() {
        let w = Windows { clients: vec![c("a", (1, "1"), 2), c("b", (2, "2"), 0), c("m", (-99, "special:minimized"), 1), c("s", (-98, "special:scratch"), 3)], ..Default::default() };
        let l = candidates(&w);
        assert_eq!(l.iter().map(|c| c.address.as_str()).collect::<Vec<_>>(), vec!["b", "m", "a"]);
        // forward: the one behind the focused window; backward: the last
        assert_eq!(first_selected(&l, "b", 1), 1);
        assert_eq!(first_selected(&l, "x", 1), 0);
        assert_eq!(first_selected(&l, "b", -1), 2);
        assert_eq!(first_selected(&l[..1], "b", 1), 0);
        assert_eq!(first_selected(&[], "b", 1), 0);
    }

    #[test]
    fn tab_wraps_round() {
        assert_eq!(step(2, 1, 3), 0);
        assert_eq!(step(0, -1, 3), 2);
        assert_eq!(step(1, 4, 3), 2);
        assert_eq!(step(0, 1, 0), 0);
    }

    #[test]
    fn as_many_per_row_as_fit() {
        // 1920 wide: (1920 - 96 - 48 - 24 + 8) / 216 = 8
        assert_eq!(grid(20, (1920.0, 1080.0), false).0, 8);
        assert_eq!(grid(3, (1920.0, 1080.0), false), (3, 1, 1));
        assert_eq!(body(3, (1920.0, 1080.0), false), (3.0 * 216.0 - 8.0 + 24.0, 166.0 + 24.0));
        assert_eq!(body(0, (1920.0, 1080.0), true), (240.0, 72.0));
        // rows past the screen's height scroll
        let (_, rows, visible) = grid(60, (1920.0, 1080.0), false);
        assert!(rows > visible);
        // a wide window fills the width, a tall one the height
        assert_eq!(card((1920, 1080)), (192.0, 108.0));
        assert_eq!(card((600, 1200)), (60.0, 120.0));
        assert_eq!(card((0, 0)), (192.0, 120.0));
    }
}

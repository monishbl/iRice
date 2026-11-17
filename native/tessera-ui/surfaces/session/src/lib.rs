//! The power and session menu, and "About This Computer" (rust.surfaces:
//! session), as shell/modules/session/SessionDialog.qml and AboutDialog.qml
//! in the shell's dialog (shell/glass/Dialog.qml): a centred dialog over a
//! light scrim, on the plugin's glass (Glass) or a Material dialog's tonal
//! container; Lock, Sleep, Log Out, Restart, Shut Down. Arrow keys move,
//! Enter confirms (not a key still repeating from Search), Esc or a click
//! outside cancels; a click on an option chooses it. Material: each a big
//! tonal shape in its role's colours (Shut Down in the error colours), the
//! chosen one filled; each keeps its own shape and a press squares it
//! (tessera_core::surfaces::polar mixes the outlines).
//!
//! ── Roles ─────────────────────────────────────────────────────────────────
//! None of its own: what an option does stays the shell's System.power
//! (which only notifies in a sandbox). This part says
//! `{"src":"ui","kind":"session","power":"<id>"}` and closes; SessionDialog.qml
//! runs it. `--demo` changes nothing here: choosing still goes to the shell.
//!
//! ── With the shell ────────────────────────────────────────────────────────
//!   Panels' "session" opens and closes it as any panel (RustUi.follow);
//!   SessionDialog.qml says `{"cmd":"action","surface":"session","action":…}`
//!   (Panels.sessionAction: asked for from Search, so Enter confirms it);
//!   AboutDialog.qml says `{"cmd":"about","surface":"session","open":…,
//!   "screen":…,"info":{osName,hostname,cpu,gpu,memory,kernel,logo}}`
//!   (SystemInfo stays the shell's), and hears `{"kind":"session","close":"about"}`.
//! Not like the QML: no elastic pull when the dialog is dragged.

pub mod ui {
    slint::include_modules!();
}
tessera_core::theme_global!(ui);

// tessera-core's modules (shell, wayland, theme, services, util…) under the
// names they had when everything was one crate: `crate::shell::Part`
#[allow(unused_imports)]
use tessera_core::*;

use tessera_core::services::hypr::Glass;
use tessera_core::shell::{Command, Part};
use tessera_core::surfaces::polar as shape;
use tessera_core::theme;
use tessera_core::wayland::{Spec, State, Surface};
use slint::{ComponentHandle};
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use smithay_client_toolkit::shell::WaylandSurface;
use std::any::Any;
use std::rc::Rc;
use std::time::Duration;

const NAME: &str = "session";
const NS: &str = "tessera:session";

/// (id, text, Phosphor glyph, Material shape, role)
pub const OPTIONS: [(&str, &str, &str, &str, &str); 5] = [
    ("lock", "Lock", "\u{e2fa}", "cookie4", "primary"),
    ("sleep", "Sleep", "\u{e330}", "cookie9", "secondary"),
    ("logout", "Log Out", "\u{e42a}", "clover4", "secondary"),
    ("restart", "Restart", "\u{e036}", "cookie12", "tertiary"),
    ("shutdown", "Shut Down", "\u{e3da}", "softBurst", "error"),
];

/// Material's labels are sentence case ("Log out")
pub fn label(text: &str, material: bool) -> String {
    if !material {
        return text.to_string();
    }
    let mut out = String::new();
    for (i, c) in text.chars().enumerate() {
        out.extend(if i == 0 { c.to_uppercase().collect::<Vec<_>>() } else { c.to_lowercase().collect() });
    }
    out
}

/// Where a key moves the choice (None: not a moving key)
pub fn moved(current: usize, key: &str) -> Option<usize> {
    let last = OPTIONS.len() - 1;
    match key {
        "\u{f702}" | "\u{f700}" => Some(current.saturating_sub(1)),
        "\u{f703}" | "\u{f701}" => Some((current + 1).min(last)),
        _ => None,
    }
}

/// The option asked for (Panels.sessionAction), else Shut Down
pub fn start_at(action: &str) -> usize {
    OPTIONS.iter().position(|o| o.0 == action).unwrap_or(4)
}

enum M {
    Hover(i32),
    Choose(i32),
    Press(i32, bool),
    Key(String, bool),
    Dismissed,
    /// the close animation ended (Dialog.qml: unmapped once progress is 0)
    Closed,
    Frame,
}

struct Win {
    output: String,
    surface: Rc<Surface>,
    ui: ui::SessionDialog,
    a11y: Option<crate::a11y::A11y>,
    glass_sent: String,
    /// the choices, changed in place (they were made again on every hover,
    /// which restarted a press under the pointer)
    opts: tessera_core::rows::Rows<ui::SOpt>,
    /// About's rows, changed in place (rows.rs)
    rows: tessera_core::rows::Rows<ui::SRow>,
}

pub struct Session {
    win: Option<Win>,
    /// what shows: "session", "about", or "" (closed)
    open: String,
    action: String,
    current: usize,
    pressed: Option<usize>,
    /// Material: each option's morph, 0 a rounded square .. 1 its shape
    morph: [f64; 5],
    frames: tessera_core::pacing::FrameTimer,
    hide: slint::Timer,
}

impl Default for Session {
    fn default() -> Self {
        Self { win: None, open: String::new(), action: String::new(), current: 4, pressed: None, morph: [0.0; 5], frames: tessera_core::pacing::FrameTimer::default(), hide: slint::Timer::default() }
    }
}

fn hex(c: &str) -> slint::Color {
    let v = u32::from_str_radix(c.trim_start_matches('#'), 16).unwrap_or(0);
    slint::Color::from_rgb_u8((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

impl Session {
    fn send(st: &State) -> impl Fn(Box<dyn Any + Send>) + Clone + 'static {
        st.shell.sender(NAME)
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
            anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
            size: (0, 0),
            exclusive: -1,
            keyboard: KeyboardInteractivity::None,
            margin_left: 0,
        });
        let ui = ui::SessionDialog::new().expect("SessionDialog");
        st.shell.apply_theme(&ui.global::<ui::Theme>());
        ui.global::<ui::Symbols>().on_of(|g| crate::icons::symbol(&g).into());
        ui.set_cookie12(shape::outline("cookie12", "cookie12", 0.0, 0.0).into());
        let to = Self::send(st);
        {
            let to = to.clone();
            ui.on_key(move |k, repeat| {
                let ok = moved(0, &k).is_some() || k == "\u{a}" || k == "\u{1b}";
                if ok {
                    to(Box::new(M::Key(k.to_string(), repeat)));
                }
                ok
            });
        }
        macro_rules! on {
            ($set:ident, |$($a:ident),*| $m:expr) => {{
                let to = to.clone();
                ui.$set(move |$($a),*| to(Box::new($m)));
            }};
        }
        on!(on_hover, |i| M::Hover(i));
        on!(on_choose, |i| M::Choose(i));
        on!(on_pressing, |i, p| M::Press(i, p));
        on!(on_dismissed, | | M::Dismissed);
        on!(on_closed, | | M::Closed);
        let opts = tessera_core::rows::Rows::new();
        ui.set_options(opts.model());
        let rows = tessera_core::rows::Rows::new();
        ui.set_rows(rows.model());
        ui.show().expect("show session");
        let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
        surface.hide();
        self.win = Some(Win { output: output.into(), surface, ui, a11y, glass_sent: String::new(), opts, rows });
        true
    }

    fn options(&self, st: &State) {
        let Some(w) = &self.win else { return };
        let look = &st.shell.look;
        let roles: std::collections::HashMap<String, String> = if look.material { theme::look_roles(&look).into_iter().collect() } else { Default::default() };
        let r = |k: &str| roles.get(k).map(|v| hex(v)).unwrap_or(slint::Color::from_rgb_u8(128, 128, 128));
        let cap = |s: &str| {
            let mut c = s.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
        };
        // Only the chosen option stands out. The others share one quiet
        // tonal shape (each keeps its own outline); Restart's and Shut Down's
        // icons carry a faint blue and red, duller than any chosen colour.
        // Chosen: the primary colour; Restart blue, Shut Down the error red.
        let mix = |a: slint::Color, b: slint::Color, t: f32| {
            let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
            slint::Color::from_rgb_u8(l(a.red(), b.red()), l(a.green(), b.green()), l(a.blue(), b.blue()))
        };
        let (blue, on_blue) = if look.dark { (hex("#9ecaff"), hex("#003258")) } else { (hex("#2f6aa8"), hex("#ffffff")) };
        // (one tone above the dialog's own surface, so each shape still shows)
        let quiet = r("surfaceContainerHighest");
        let quiet_ink = r("onSurfaceVariant");
        let opts: Vec<ui::SOpt> = OPTIONS
            .iter()
            .enumerate()
            .map(|(i, (id, text, icon, shp, _role))| {
                let (fill_hot, ink_hot, ink, kind) = match *id {
                    "restart" => (blue, on_blue, mix(quiet_ink, blue, 0.45), 1),
                    "shutdown" => (r("error"), r("onError"), mix(quiet_ink, r("error"), 0.5), 2),
                    _ => (r("primary"), r("onPrimary"), quiet_ink, 0),
                };
                ui::SOpt {
                    id: (*id).into(),
                    text: label(text, look.material).into(),
                    icon: (*icon).into(),
                    path: shape::outline("square", shp, self.morph[i], 0.0).into(),
                    fill_hot,
                    ink_hot,
                    fill: quiet,
                    ink,
                    kind,
                }
            })
            .collect();
        w.opts.set(opts);
        w.ui.set_current(self.current as i32);
        w.ui.set_keyboard_confirm(!self.action.is_empty());
    }

    fn show(&mut self, st: &mut State, which: &str, screen: Option<String>) {
        let output = screen.or_else(|| self.win.as_ref().map(|w| w.output.clone())).or_else(|| st.outputs_now().first().map(|o| o.1.clone()));
        let Some(output) = output else { return };
        if !self.window(st, &output) {
            return;
        }
        self.hide.stop();
        self.open = which.to_string();
        if which == "session" {
            self.current = start_at(&self.action);
            self.pressed = None;
            for m in self.morph.iter_mut() {
                *m = 1.0; // (every option in its own shape, as SessionDialog.qml)
            }
        }
        self.options(st);
        let w = self.win.as_ref().unwrap();
        w.ui.set_which(which.into());
        w.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        w.surface.show(0, 0);
        w.ui.set_shown(true);
        w.ui.invoke_focus_keys();
    }

    /// Close it (`tell`: it closed itself, so the shell's Panels follows)
    fn close(&mut self, st: &State, tell: bool) {
        if self.open.is_empty() {
            return;
        }
        let which = std::mem::take(&mut self.open);
        if let Some(w) = &self.win {
            w.ui.set_shown(false);
            w.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::None);
            w.surface.layer.commit();
        }
        if tell {
            if which == "about" {
                crate::shell::tell(NAME, serde_json::json!({"close": "about"}));
            } else {
                crate::shell::closed(NAME);
            }
        }
        // (unmapped when the close animation ends, M::Closed, as Dialog.qml's
        // `visible: progress > 0`; this is CloseGuard's cap, should frames stall)
        let to = Self::send(st);
        self.hide.start(slint::TimerMode::SingleShot, Duration::from_millis(700), move || to(Box::new(M::Closed)));
    }

    fn choose(&mut self, st: &State, i: usize) {
        if self.open != "session" || i >= OPTIONS.len() {
            return;
        }
        self.close(st, true);
        crate::shell::tell(NAME, serde_json::json!({"power": OPTIONS[i].0}));
    }

    /// Material's shapes: toward their own while chosen and not pressed
    fn animate(&mut self, st: &State) {
        if !st.shell.look.material {
            return;
        }
        if !self.frames.running() {
            let to = Self::send(st);
            self.frames.start(move || to(Box::new(M::Frame)));
        }
    }

    fn frame(&mut self, st: &State) {
        let mut moving = false;
        for i in 0..self.morph.len() {
            // (each keeps its own shape; a press squares it, as SessionDialog.qml)
            let target = if self.pressed == Some(i) { 0.0 } else { 1.0 };
            let d = target - self.morph[i];
            if d.abs() > 0.004 {
                self.morph[i] += d * 0.22;
                moving = true;
            } else {
                self.morph[i] = target;
            }
        }
        self.options(st);
        if !moving {
            self.frames.stop();
        }
    }
}

impl Part for Session {
    // (the window it keeps while closed, made ahead: window() leaves it unmapped)
    fn prewarm(&mut self, st: &mut State) {
        if self.win.is_none() {
            if let Some(out) = st.likely_output() {
                self.window(st, &out);
            }
        }
    }

    fn name(&self) -> &'static str {
        NAME
    }

    fn command(&mut self, st: &mut State, cmd: &Command) {
        match cmd.cmd.as_str() {
            "open" => self.show(st, "session", cmd.screen.clone()),
            "close" => {
                if self.open == "session" {
                    self.close(st, false);
                }
            }
            "action" => {
                self.action = cmd.args["action"].as_str().unwrap_or("").to_string();
                if self.open == "session" {
                    self.current = start_at(&self.action);
                    self.options(st);
                    self.animate(st);
                }
            }
            "about" => {
                if cmd.args["open"].as_bool() == Some(true) {
                    let info = &cmd.args["info"];
                    let s = |k: &str| info[k].as_str().unwrap_or("").to_string();
                    self.show(st, "about", cmd.screen.clone());
                    if let Some(w) = &self.win {
                        w.ui.set_os_name(s("osName").into());
                        w.ui.set_host(s("hostname").into());
                        let rows: Vec<ui::SRow> = [("Processor", "cpu"), ("Graphics", "gpu"), ("Memory", "memory"), ("Kernel", "kernel")]
                            .iter()
                            .filter(|(_, k)| !s(k).is_empty())
                            .map(|(l, k)| ui::SRow { label: (*l).into(), value: s(k).into() })
                            // (which shell this session runs: this one, Rust)
                            .chain(std::iter::once(ui::SRow { label: "Shell".into(), value: "Tessera (Rust)".into() }))
                            .collect();
                        w.rows.set(rows);
                        let logo = s("logo");
                        let img = logo.strip_prefix("file://").map(str::to_string).or(Some(logo)).filter(|p| p.starts_with('/')).and_then(|p| slint::Image::load_from_path(std::path::Path::new(&p)).ok());
                        w.ui.set_has_logo(img.is_some());
                        w.ui.set_logo(img.unwrap_or_default());
                    }
                } else if self.open == "about" {
                    self.close(st, false);
                }
            }
            _ => {}
        }
    }

    fn config_changed(&mut self, st: &mut State, look: bool) {
        if look {
            if let Some(w) = &self.win {
                st.shell.apply_theme(&w.ui.global::<ui::Theme>());
            }
            self.options(st);
        }
    }

    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Hover(i) => {
                if self.action.is_empty() && i >= 0 {
                    self.current = i as usize;
                    self.options(st);
                    self.animate(st);
                }
            }
            M::Choose(i) => self.choose(st, i.max(0) as usize),
            M::Press(i, p) => {
                self.pressed = if p { Some(i.max(0) as usize) } else { None };
                self.animate(st);
            }
            M::Key(k, repeat) => {
                if k == "\u{1b}" {
                    self.close(st, true);
                } else if k == "\u{a}" {
                    // (holding Enter in Search must never confirm the action)
                    if !repeat {
                        let c = self.current;
                        self.choose(st, c);
                    }
                } else if let Some(c) = moved(self.current, &k) {
                    self.current = c;
                    self.options(st);
                    self.animate(st);
                }
            }
            M::Dismissed => {
                // a click on the scrim closes
                if !self.open.is_empty() {
                    self.close(st, true);
                }
            }
            M::Closed => {
                // after closing, the window goes (not if it opened again)
                if !self.open.is_empty() {
                    return;
                }
                self.hide.stop();
                if let Some(w) = &mut self.win {
                    w.surface.hide();
                    st.shell.glass.set(&w.output, NS, &[]);
                    w.glass_sent.clear();
                }
            }
            M::Frame => self.frame(st),
        }
    }

    fn keyboard_left(&mut self, _st: &mut State, _s: &Rc<Surface>) {}

    fn output_removed(&mut self, st: &mut State, name: &str) {
        if self.win.as_ref().is_some_and(|w| w.output == name) {
            let w = self.win.take().unwrap();
            st.drop_surface(&w.surface);
            self.open.clear();
        }
    }

    /// The dialog's glass for the plugin (Glass), as it springs open and closes
    fn after_frame(&mut self, st: &mut State) {
        let kbd = st.keyboard_surface();
        let Some(w) = &mut self.win else { return };
        let f = kbd.as_ref().is_some_and(|k| Rc::ptr_eq(k, &w.surface));
        if let Some(a) = w.a11y.as_mut() {
            a.update(false, f);
        }
        if !w.surface.mapped.get() || !st.shell.plugin || st.shell.look.material {
            return;
        }
        let ui = &w.ui;
        let s = ui.get_body_scale();
        let (bw, bh) = (ui.get_body_w(), ui.get_body_h());
        let x = ui.get_body_x() + bw * (1.0 - s) / 2.0;
        let y = ui.get_body_y() + bh * (1.0 - s) / 2.0;
        let o = ui.get_body_opacity();
        let shape = theme::glass_shape(&st.shell.look, x, y, bw * s, bh * s, 28.0 * s, o, 1.0);
        let key = tessera_core::theme::glass_key(&shape);
        if key != w.glass_sent {
            st.shell.glass.set(&w.output, NS, if o > 0.002 { std::slice::from_ref(&shape) } else { &[] });
            w.glass_sent = key;
        }
    }

    fn a11y(&mut self) -> Vec<&mut crate::a11y::A11y> {
        self.win.as_mut().and_then(|w| w.a11y.as_mut()).into_iter().collect()
    }

    fn quit(&mut self, _: &mut State) {
        if let Some(w) = &self.win {
            Glass::clear_now(&w.output, NS);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_and_keys() {
        assert_eq!(label("Log Out", true), "Log out");
        assert_eq!(label("Shut Down", false), "Shut Down");
        assert_eq!(moved(0, "\u{f702}"), Some(0));
        assert_eq!(moved(2, "\u{f703}"), Some(3));
        assert_eq!(moved(4, "\u{f701}"), Some(4));
        assert_eq!(moved(2, "a"), None);
        assert_eq!(start_at("sleep"), 1);
        assert_eq!(start_at(""), 4);
        assert_eq!(start_at("nonsense"), 4);
    }
}

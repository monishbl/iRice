//! The login intro (LoginIntro.qml), drawn here while the lock is: as the
//! desktop first appears after logging in, a cover over every screen that
//! takes no clicks or keys, gone in 1.3 s at most.
//!   Glass: from black, your wallpaper comes into focus (blurred and a touch
//!   larger, settling), then the cover lifts away
//!   Material: the Tessera mark (a cookie with the badge) opens into a window
//!   onto the desktop that grows, morphing to a circle, until it fills the
//!   screen
//! When it plays stays the shell's (the first start in a session, the
//! setting, the startup chime, LockFx's settle under it): LoginIntro.qml
//! says `{"to":"lock","intro":"arm"}` (the cover goes up), `"play"` (its
//! clock starts; `pace` slows it for recording) and `"stop"`.
//! Not like the QML: the blur is a cross-fade from a blurred copy (Slint has
//! no blur); the badge isn't cut by the opening as it grows (it has faded by
//! then).

use super::{decode_pair, shape, M};
use crate::ui;
use crate::wayland::{Spec, State, Surface};
use slint::{ComponentHandle, Image};
use smithay_client_toolkit::reexports::client::protocol::wl_output::WlOutput;
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use std::rc::Rc;
use std::time::{Duration, Instant};

pub const NS: &str = "tessera:lockveil";

/// A frame of the intro (its timer)
pub struct Tick;

struct Win {
    output: String,
    surface: Rc<Surface>,
    ui: ui::LoginIntro,
    picture: (String, (u32, u32)),
}

#[derive(Default)]
pub struct Intro {
    wins: Vec<Win>,
    shown: bool,
    started: Option<Instant>,
    pace: f64,
    material: bool,
    timer: slint::Timer,
    picture: Option<(String, Image, Image)>,
}

/// 0 .. 1 between two moments (ms)
pub fn span(t: f64, a: f64, b: f64) -> f64 {
    ((t - a) / (b - a)).clamp(0.0, 1.0)
}
pub fn out(x: f64) -> f64 {
    1.0 - (1.0 - x).powi(3)
}
pub fn in_out(x: f64) -> f64 {
    if x < 0.5 { 4.0 * x * x * x } else { 1.0 - (-2.0 * x + 2.0).powi(3) / 2.0 }
}
/// Material's expressive spring for shapes (ζ 0.6, k 200), 0 → 1, `ms` after `a`
pub fn spring(t: f64, a: f64) -> f64 {
    let s = (t - a).max(0.0) / 1000.0;
    let w = 200f64.sqrt();
    let z: f64 = 0.6;
    let wd = w * (1.0 - z * z).sqrt();
    1.0 - (-z * w * s).exp() * ((wd * s).cos() + z / (1.0 - z * z).sqrt() * (wd * s).sin())
}

/// Glass's frame at `t` ms: (cover's opacity, picture's scale, blur 0..1, black over it)
pub fn glass_frame(t: f64) -> (f64, f64, f64, f64) {
    (1.0 - out(span(t, 650.0, 1250.0)), 1.0 + 0.06 * (1.0 - out(span(t, 0.0, 1000.0))), 1.0 - out(span(t, 0.0, 900.0)), 1.0 - out(span(t, 0.0, 450.0)))
}

/// Material's shapes at `t` ms: (from, to, morph, spin, opening's diameter
/// as a share of the reach)
pub fn material_shapes(t: f64) -> (&'static str, &'static str, f64, f64, f64) {
    let from = if t < 520.0 { "cookie9" } else { "softBurst" };
    let to = if t < 250.0 { "cookie9" } else if t < 520.0 { "softBurst" } else { "circle" };
    let morph = if t < 250.0 { 1.0 } else { spring(t, if t < 520.0 { 250.0 } else { 520.0 }) };
    (from, to, morph, 90.0 * out(span(t, 0.0, 900.0)), in_out(span(t, 0.0, 900.0)))
}

impl Intro {
    fn length(&self) -> f64 {
        if self.material { 1000.0 } else { 1300.0 }
    }

    pub fn command(&mut self, st: &mut State, what: &str, pace: f64) {
        eprintln!("tessera-ui: lock: login intro {what} ({} screens)", st.outputs_now().len());
        match what {
            "arm" => {
                self.material = st.shell.look.material;
                self.pace = pace;
                self.shown = true;
                self.started = None;
                for (o, name) in st.outputs_now() {
                    self.window(st, &o, &name);
                }
                self.frame(0.0);
            }
            "play" if self.shown => {
                self.pace = pace.max(1.0);
                self.started = Some(Instant::now());
                let to = st.shell.sender(super::NAME);
                self.timer.start(slint::TimerMode::Repeated, Duration::from_millis(16), move || to(Box::new(Tick)));
            }
            _ => self.stop(st),
        }
    }

    pub fn stop(&mut self, st: &mut State) {
        self.timer.stop();
        self.shown = false;
        self.started = None;
        for w in std::mem::take(&mut self.wins) {
            st.drop_surface(&w.surface);
        }
    }

    pub fn tick(&mut self, st: &mut State) {
        let Some(s) = self.started else { return };
        let t = s.elapsed().as_secs_f64() * 1000.0 / self.pace.max(1.0);
        if t >= self.length() {
            self.stop(st);
            return;
        }
        self.frame(t);
    }

    fn frame(&self, t: f64) {
        for w in &self.wins {
            let ui = &w.ui;
            if self.material {
                let (lw, lh) = w.surface.logical_size();
                let (wd, ht) = (lw.max(1) as f64, lh.max(1) as f64);
                let mark = (0.175 * wd.min(ht)).round();
                let reach = 1.05 * wd.hypot(ht);
                let (from, to, k, spin, open) = material_shapes(t);
                let hole = open * reach / 2.0;
                let ring = (mark.max(open * reach + 0.14 * mark * (1.0 - out(span(t, 200.0, 900.0))))) / 2.0;
                ui.set_cover(shape::cover_with_hole(wd, ht, hole, from, to, k, spin).into());
                ui.set_ring(shape::ring(wd, ht, ring, hole, from, to, k, spin).into());
                ui.set_badge_opacity((1.0 - out(span(t, 0.0, 250.0))) as f32);
                ui.set_badge_scale((1.0 - 0.4 * out(span(t, 0.0, 250.0))) as f32);
                ui.set_mark(mark as f32);
            } else {
                let (cover, scale, blur, black) = glass_frame(t);
                ui.set_cover_opacity(cover as f32);
                ui.set_picture_scale(scale as f32);
                ui.set_blur(blur as f32);
                ui.set_veil(black as f32);
            }
        }
    }

    fn window(&mut self, st: &mut State, output: &WlOutput, name: &str) {
        if self.wins.iter().any(|w| w.output == name) {
            return;
        }
        let surface = st.prepare_surface(Spec {
            layer: Layer::Overlay,
            namespace: NS,
            output: Some((output, name)),
            anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
            size: (0, 0),
            exclusive: -1,
            keyboard: KeyboardInteractivity::None,
            margin_left: 0,
        });
        let ui = ui::LoginIntro::new().expect("LoginIntro");
        st.shell.apply_theme(&ui.global::<ui::Theme>());
        ui.set_material(self.material);
        // (clicks go through: it never stands in the way)
        st.set_input_region(&surface, Some(&[]));
        if let Some((_, sharp, soft)) = &self.picture {
            ui.set_wallpaper(sharp.clone());
            ui.set_soft(soft.clone());
            ui.set_has_picture(true);
        }
        ui.show().expect("show intro");
        self.wins.push(Win { output: name.to_string(), surface, ui, picture: (String::new(), (0, 0)) });
    }

    pub fn output_added(&mut self, st: &mut State, output: &WlOutput, name: &str) {
        if self.shown {
            self.window(st, output, name);
        }
    }

    pub fn output_removed(&mut self, st: &mut State, name: &str) {
        if let Some(i) = self.wins.iter().position(|w| w.output == name) {
            let w = self.wins.remove(i);
            st.drop_surface(&w.surface);
        }
    }

    /// A decoded wallpaper (the lock's decoder): the one this intro waits for
    pub fn picture(&mut self, path: &str, sharp: &Image, soft: &Image) {
        self.picture = Some((path.to_string(), sharp.clone(), soft.clone()));
        for w in &self.wins {
            if w.picture.0 == path {
                w.ui.set_wallpaper(sharp.clone());
                w.ui.set_soft(soft.clone());
                w.ui.set_has_picture(true);
            }
        }
    }

    /// Glass: each screen's wallpaper at its size, decoded once it has one
    pub fn after_frame(&mut self, st: &mut State, path: &str) {
        if self.material || !self.shown {
            return;
        }
        let path = path.to_string();
        for w in &mut self.wins {
            let (lw, lh) = w.surface.logical_size();
            let scale = w.ui.window().scale_factor();
            let size = ((lw as f32 * scale) as u32, (lh as f32 * scale) as u32);
            if lw > 1 && (w.picture.0 != path || w.picture.1 != size) {
                w.picture = (path.clone(), size);
                let to = st.shell.sender(super::NAME);
                let p = path.clone();
                std::thread::spawn(move || {
                    let px = decode_pair(&p, size.0, size.1);
                    to(Box::new(M::Wallpaper(p, size, px)));
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glass_comes_into_focus_then_lifts() {
        let (c, s, b, k) = glass_frame(0.0);
        assert_eq!((c, b, k), (1.0, 1.0, 1.0));
        assert!((s - 1.06).abs() < 1e-9);
        let (c, s, b, k) = glass_frame(1300.0);
        assert_eq!((c, s, b, k), (0.0, 1.0, 0.0, 0.0));
        // in focus before it lifts
        let (c, _, b, _) = glass_frame(900.0);
        assert!(b == 0.0 && c > 0.0 && c < 1.0);
    }

    #[test]
    fn material_opens_and_morphs() {
        let (f, t, k, spin, open) = material_shapes(0.0);
        assert_eq!((f, t, k, spin, open), ("cookie9", "cookie9", 1.0, 0.0, 0.0));
        let (f, t, _, _, _) = material_shapes(400.0);
        assert_eq!((f, t), ("cookie9", "softBurst"));
        let (f, t, k, spin, open) = material_shapes(999.0);
        assert_eq!((f, t), ("softBurst", "circle"));
        assert!((spin - 90.0).abs() < 1e-6 && (open - 1.0).abs() < 1e-9 && (k - 1.0).abs() < 0.05);
        // the spring overshoots a little, then settles
        assert!(spring(0.0, 0.0) == 0.0 && (spring(2000.0, 0.0) - 1.0).abs() < 1e-3);
        assert!((0..600).map(|ms| spring(ms as f64, 0.0)).any(|v| v > 1.0));
    }
}

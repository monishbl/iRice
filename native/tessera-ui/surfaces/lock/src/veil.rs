//! The veil (Lock.qml's `tessera:lockveil` windows): as the lock goes, the
//! wallpaper in an overlay layer above the windows, the dock and the menu
//! bar, at exactly the scale the desktop's own wallpaper shows (LockFx's
//! wallpaperScale), so the first frame without the lock looks just like the
//! last one; fading it (LockFx's veil) is what fades them in. It takes no
//! clicks or keys.
//!
//! The shell (host-shell Bridges, Lock.qml's half) says when, beside the
//! background's own settle: `{"to":"lock","veil":"prepare"|"reveal"|"rest","zoom":…,"ms":…}`.
//!   prepare  up (desktop 0: the wallpaper set back, the veil covering)
//!   reveal   LockFx's `desktop` runs 0 → 1 over `ms`, linearly; the veil
//!            fades by its own curve and is gone once it's at rest
//!   rest     gone at once
//! The picture is the lock's own, already decoded at the screen's size.

use crate::ui;
use crate::wayland::{Spec, State, Surface};
use slint::{ComponentHandle, Image};
use smithay_client_toolkit::reexports::client::protocol::wl_output::WlOutput;
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use std::rc::Rc;
use std::time::{Duration, Instant};

pub const NS: &str = "tessera:lockveil";

/// A frame of the veil's reveal (its timer)
pub struct Tick;

struct Win {
    output: String,
    surface: Rc<Surface>,
    ui: ui::LockVeil,
}

#[derive(Default)]
pub struct Veil {
    wins: Vec<Win>,
    zoom: bool,
    /// a reveal: when it began and how long it runs
    reveal: Option<(Instant, f64)>,
    /// when it went up (it lowers itself if no reveal follows)
    prepared: Option<Instant>,
    timer: slint::Timer,
}

/// LockFx's ease (out-cubic, clamped)
pub fn ease(t: f64) -> f64 {
    let c = t.clamp(0.0, 1.0);
    1.0 - (1.0 - c).powi(3)
}

/// LockFx at `desktop` (0 set back … 1 at rest): (the wallpaper's scale, the veil's opacity)
pub fn frame(desktop: f64, zoom: bool) -> (f64, f64) {
    let scale = if zoom { 1.0 + 0.05 * (1.0 - ease(desktop)) } else { 1.0 };
    (scale, 1.0 - ease((desktop / 0.8).min(1.0)))
}

impl Veil {
    pub fn shown(&self) -> bool {
        !self.wins.is_empty()
    }

    /// A step from the shell; `pictures` the lock's picture for each screen
    pub fn step(&mut self, st: &mut State, step: &str, zoom: bool, ms: f64, pictures: &[(String, Image)]) {
        match step {
            "prepare" => {
                self.zoom = zoom;
                self.reveal = None;
                for (o, name) in st.outputs_now() {
                    if let Some((_, img)) = pictures.iter().find(|(n, _)| *n == name) {
                        self.window(st, &o, &name, img);
                    }
                }
                self.show(0.0);
                // (never left over the windows: a reveal that doesn't come
                // in a few seconds lowers it all the same)
                self.prepared = Some(Instant::now());
                let to = st.shell.sender(super::NAME);
                self.timer.start(slint::TimerMode::Repeated, Duration::from_millis(500), move || to(Box::new(Tick)));
            }
            "reveal" if self.shown() => {
                self.reveal = Some((Instant::now(), ms.max(1.0)));
                let to = st.shell.sender(super::NAME);
                self.timer.start(slint::TimerMode::Repeated, Duration::from_millis(16), move || to(Box::new(Tick)));
            }
            "reveal" => {}
            _ => self.drop_all(st),
        }
    }

    pub fn tick(&mut self, st: &mut State) {
        let Some((start, ms)) = self.reveal else {
            if self.prepared.is_none_or(|p| p.elapsed() > Duration::from_secs(3)) {
                self.drop_all(st);
            }
            return;
        };
        let desktop = start.elapsed().as_secs_f64() * 1000.0 / ms;
        if desktop >= 1.0 {
            // (at rest: lowered, as Lock.qml's LockFx.active turning false)
            self.drop_all(st);
            return;
        }
        self.show(desktop);
    }

    fn show(&self, desktop: f64) {
        let (scale, veil) = frame(desktop, self.zoom);
        for w in &self.wins {
            w.ui.set_scale(scale as f32);
            w.ui.set_veil(veil as f32);
        }
    }

    pub fn drop_all(&mut self, st: &mut State) {
        self.timer.stop();
        self.reveal = None;
        self.prepared = None;
        for w in std::mem::take(&mut self.wins) {
            st.drop_surface(&w.surface);
        }
    }

    fn window(&mut self, st: &mut State, output: &WlOutput, name: &str, picture: &Image) {
        if let Some(w) = self.wins.iter().find(|w| w.output == name) {
            w.ui.set_wallpaper(picture.clone());
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
        let ui = ui::LockVeil::new().expect("LockVeil");
        ui.set_wallpaper(picture.clone());
        // (clicks go through)
        st.set_input_region(&surface, Some(&[]));
        ui.show().expect("show veil");
        self.wins.push(Win { output: name.to_string(), surface, ui });
    }

    pub fn output_removed(&mut self, st: &mut State, name: &str) {
        if let Some(i) = self.wins.iter().position(|w| w.output == name) {
            let w = self.wins.remove(i);
            st.drop_surface(&w.surface);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_veil_is_lockfx_frame_for_frame() {
        // set back: the wallpaper a little larger, the veil covering
        assert_eq!(frame(0.0, true), (1.05, 1.0));
        assert_eq!(frame(0.0, false), (1.0, 1.0));
        // gone by 80 % of the way, the wallpaper still settling
        let (s, v) = frame(0.8, true);
        assert!(v.abs() < 1e-9 && s > 1.0);
        assert_eq!(frame(1.0, true), (1.0, 0.0));
    }
}

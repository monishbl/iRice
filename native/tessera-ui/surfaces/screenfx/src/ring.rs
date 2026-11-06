//! The find-pointer ring (rust.surfaces: findpointer), compare
//! shell/services/FindPointer.qml: after a shake Hyprland calls the shell, and
//! (for the "ring" and "both" styles) a ring shrinks onto the pointer,
//! following it while it moves, on the display the pointer is on. The shake,
//! the pointer's position (assets/scripts/pointer-follow.py) and the "grow"
//! half (hyprctl setcursor) are the host's (host/desktop wm/findptr.rs); it
//! says:
//!   {"to":"findpointer","show":true,"style":"ring","x":0,"y":0}   it starts
//!   {"to":"findpointer","show":true,"x":812.0,"y":344.5}           the pointer
//!                       (global layout coordinates, many a second)
//!   {"to":"findpointer","show":false}                              it ends (1.4 s)
//! The ring: 150 px, the accent at 12% inside and a 3 px line however it is
//! scaled; it scales 1.6 → 0.5 over 450 ms (ease-out cubic) as it fades in
//! over 200 ms, holds 550 ms, then fades out over 300 ms (ease-in). It is not
//! drawn until the pointer's position is first known. Clicks go through.

use super::*;
use slint::ComponentHandle;
use std::time::{Duration, Instant};
use tessera_core::services::hypr::Snapshot;
use tessera_core::shell::Part;

const NAME: &str = "findpointer";
const NS: &str = "tessera-findpointer";

enum Msg {
    Tick,
}

struct Win {
    output: String,
    surface: Rc<Surface>,
    ui: ui::FindRing,
    /// the display's top left, in layout coordinates
    origin: (i32, i32),
    started: Instant,
}

#[derive(Default)]
pub struct FindPointer {
    win: Option<Win>,
    active: bool,
    ring: bool,
    x: f64,
    y: f64,
    tick: slint::Timer,
}

/// The ring's scale and opacity `ms` into the animation (None: it is over)
pub fn frame(ms: f32) -> (f32, f32) {
    if ms < 450.0 {
        let e = out_cubic(ms / 450.0);
        (1.6 + (0.5 - 1.6) * e, out_cubic(ms / 200.0))
    } else if ms < 1000.0 {
        (0.5, 1.0)
    } else if ms < 1300.0 {
        (0.5, 1.0 - in_cubic((ms - 1000.0) / 300.0))
    } else {
        (0.5, 0.0)
    }
}

impl FindPointer {
    /// The display under (x, y), else the focused one, else the first
    fn where_is(&self, st: &State) -> Option<(WlOutput, String, (i32, i32))> {
        let outs = st.outputs_now();
        let pos = |o: &WlOutput| st.outputs.info(o).and_then(|i| i.logical_position).unwrap_or((0, 0));
        let hit = outs.iter().find(|(o, _)| {
            let Some(info) = st.outputs.info(o) else { return false };
            let (px, py) = pos(o);
            let (w, h) = info.logical_size.unwrap_or((0, 0));
            self.x >= px as f64 && self.x < (px + w) as f64 && self.y >= py as f64 && self.y < (py + h) as f64
        });
        let focused = st.shell.get::<Snapshot>().map(|s| s.focused.clone()).unwrap_or_default();
        let pick = hit.or_else(|| outs.iter().find(|(_, n)| *n == focused)).or_else(|| outs.first())?;
        Some((pick.0.clone(), pick.1.clone(), pos(&pick.0)))
    }

    /// The ring's window, on the display the pointer is on (made again when
    /// the pointer crosses to another: the animation starts over, as the
    /// QML's window did)
    fn ensure(&mut self, st: &mut State) {
        let Some((wl, name, origin)) = self.where_is(st) else { return };
        if self.win.as_ref().is_some_and(|w| w.output != name) {
            if let Some(w) = self.win.take() {
                retire(st, &w.surface);
            }
        }
        if self.win.is_none() {
            let surface = overlay(st, &wl, &name, NS, KeyboardInteractivity::None);
            let ui = ui::FindRing::new().expect("FindRing");
            st.shell.apply_theme(&ui.global::<ui::Theme>());
            ui.show().expect("show find ring");
            st.set_input_region(&surface, Some(&[]));
            self.win = Some(Win { output: name, surface, ui, origin, started: Instant::now() });
        } else if let Some(w) = self.win.as_mut() {
            w.origin = origin;
        }
        self.place();
    }

    fn place(&self) {
        let Some(w) = &self.win else { return };
        w.ui.set_cx((self.x - w.origin.0 as f64) as f32);
        w.ui.set_cy((self.y - w.origin.1 as f64) as f32);
        w.ui.set_ring_visible(self.x != 0.0 || self.y != 0.0);
    }

    fn start(&mut self, st: &mut State, style: &str) {
        self.active = true;
        self.x = 0.0;
        self.y = 0.0; // (until it has been read: no ring in the corner)
        self.ring = style != "grow";
        if self.ring {
            self.ensure(st);
            let to = st.shell.sender(NAME);
            self.tick.start(slint::TimerMode::Repeated, Duration::from_millis(16), move || to(Box::new(Msg::Tick)));
        }
    }

    fn stop(&mut self, st: &mut State) {
        self.active = false;
        self.tick.stop();
        if let Some(w) = self.win.take() {
            retire(st, &w.surface);
        }
    }

    fn step(&mut self) {
        let Some(w) = &self.win else { return };
        let (s, o) = frame(w.started.elapsed().as_secs_f32() * 1000.0);
        w.ui.set_ring_scale(s);
        w.ui.set_ring_opacity(o);
    }
}

impl Part for FindPointer {
    fn name(&self) -> &'static str {
        NAME
    }

    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        let msg = match said(msg) {
            Ok(v) => {
                if !super::on() {
                    return;
                }
                match v["show"].as_bool() {
                    Some(true) => {
                        if !self.active {
                            self.start(st, v["style"].as_str().unwrap_or("ring"));
                        }
                        if let (Some(x), Some(y)) = (v["x"].as_f64(), v["y"].as_f64()) {
                            // (the first line, at 0, 0, only says it began)
                            if self.active && self.ring && (x != 0.0 || y != 0.0 || self.win.is_none()) {
                                self.x = x;
                                self.y = y;
                                self.ensure(st);
                            }
                        }
                    }
                    Some(false) => self.stop(st),
                    None => {}
                }
                return;
            }
            Err(m) => m,
        };
        if let Ok(m) = msg.downcast::<Msg>() {
            match *m {
                Msg::Tick => self.step(),
            }
        }
    }

    fn config_changed(&mut self, st: &mut State, look: bool) {
        if look {
            if let Some(w) = &self.win {
                st.shell.apply_theme(&w.ui.global::<ui::Theme>());
            }
        }
    }

    fn output_removed(&mut self, st: &mut State, name: &str) {
        if self.win.as_ref().is_some_and(|w| w.output == name) {
            if let Some(w) = self.win.take() {
                retire(st, &w.surface);
            }
        }
    }

    fn surface_closed(&mut self, st: &mut State, s: &Rc<Surface>) {
        if self.win.as_ref().is_some_and(|w| Rc::ptr_eq(&w.surface, s)) {
            self.win = None;
            let _ = st;
        }
    }

    fn quit(&mut self, st: &mut State) {
        self.stop(st);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ring_shrinks_holds_and_fades() {
        assert_eq!(frame(0.0), (1.6, 0.0));
        let (s, o) = frame(450.0);
        assert!((s - 0.5).abs() < 1e-5 && o == 1.0);
        assert_eq!(frame(700.0), (0.5, 1.0));
        assert!(frame(1150.0).1 < 1.0);
        assert_eq!(frame(1400.0), (0.5, 0.0));
        // fully in after 200 ms
        assert_eq!(frame(200.0).1, 1.0);
    }
}

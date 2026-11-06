//! "Which display you moved to" (rust.surfaces: spaceswitch), compare
//! shell/modules/osd/SpaceSwitch.qml. Moving to another display (from the
//! keyboard, or the pointer crossing over) makes the display you moved to
//! glow at its edges or dim briefly (Settings → Displays → "Show which
//! display you moved to": workspaces.switchCue). Nothing happens to the
//! display you left.
//!
//! The decision is the host's, where Hyprland's events and the other
//! services' state are (host/desktop wm/spaceswitch.rs: a second display,
//! the cue not "off", not during a hotplug, not for moves from the Overview
//! or Mission Control). It says `{"to":"spaceswitch","cue":"DP-1","mode":"glow"}`;
//! this draws the cue on that display:
//!   glow: in 150 ms (ease-out), held 250 ms, out in 450 ms (ease-in)
//!   dim:  the same curve, lighter (18% black)
//!   reduce motion: 100 ms in and 300 ms out; the dim is a plain linear fade
//!   (100 ms in, 150 ms held, 250 ms out)
//! A layer is made on that display only while the cue plays; clicks go through.

use super::*;
use slint::ComponentHandle;
use std::time::{Duration, Instant};
use tessera_core::shell::Part;

const NAME: &str = "spaceswitch";
const NS: &str = "tessera:dim";

enum Msg {
    Tick,
}

struct Win {
    output: String,
    surface: Rc<Surface>,
    ui: ui::Cue,
    started: Instant,
    glow: bool,
    plain: bool,
    still: bool,
}

#[derive(Default)]
pub struct SpaceSwitch {
    win: Option<Win>,
    tick: slint::Timer,
}

/// How strong the cue is `ms` into it: (in ms, held ms, out ms) and whether
/// the dim is the plain fade. None once it is over.
pub fn level(ms: f32, still: bool, plain: bool) -> Option<f32> {
    let d_in = if still { 100.0 } else { 150.0 };
    let hold = if plain { 150.0 } else { 250.0 };
    let d_out = if plain { 250.0 } else if still { 300.0 } else { 450.0 };
    let ease_in = |u: f32| if plain { clamp01(u) } else { out_cubic(u) };
    let ease_out = |u: f32| if plain { clamp01(u) } else { in_cubic(u) };
    if ms < d_in {
        Some(ease_in(ms / d_in))
    } else if ms < d_in + hold {
        Some(1.0)
    } else if ms < d_in + hold + d_out {
        Some(1.0 - ease_out((ms - d_in - hold) / d_out))
    } else {
        None
    }
}

impl SpaceSwitch {
    fn show(&mut self, st: &mut State, output: &str, mode: &str) {
        if !super::on() {
            return;
        }
        if self.win.as_ref().is_some_and(|w| w.output != output) {
            self.end(st);
        }
        let glow = mode != "dim";
        let still = st.shell.cfg["appearance"]["reduceMotion"] == true;
        let plain = !glow && still;
        if self.win.is_none() {
            let Some(wl) = st.output_named(output) else { return };
            let surface = overlay(st, &wl, output, NS, KeyboardInteractivity::None);
            let ui = ui::Cue::new().expect("Cue");
            st.shell.apply_theme(&ui.global::<ui::Theme>());
            ui.show().expect("show cue");
            st.set_input_region(&surface, Some(&[]));
            self.win = Some(Win { output: output.to_string(), surface, ui, started: Instant::now(), glow, plain, still });
        }
        let w = self.win.as_mut().unwrap();
        // (a new cue on the display it is on: from the start again)
        w.started = Instant::now();
        w.glow = glow;
        w.plain = plain;
        w.still = still;
        w.ui.set_mode((if glow { "glow" } else { "dim" }).into());
        w.ui.set_glow(0.0);
        w.ui.set_dim(0.0);
        let to = st.shell.sender(NAME);
        self.tick.start(slint::TimerMode::Repeated, Duration::from_millis(16), move || to(Box::new(Msg::Tick)));
    }

    fn end(&mut self, st: &mut State) {
        self.tick.stop();
        if let Some(w) = self.win.take() {
            retire(st, &w.surface);
        }
    }

    fn step(&mut self, st: &mut State) {
        let Some(w) = &self.win else { return };
        let ms = w.started.elapsed().as_secs_f32() * 1000.0;
        match level(ms, w.still, w.plain) {
            Some(v) => {
                if w.glow {
                    w.ui.set_glow(v);
                } else {
                    w.ui.set_dim(v);
                }
            }
            None => self.end(st),
        }
    }
}

impl Part for SpaceSwitch {
    fn name(&self) -> &'static str {
        NAME
    }

    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        let msg = match said(msg) {
            Ok(v) => {
                if let Some(o) = v["cue"].as_str().filter(|o| !o.is_empty()) {
                    self.show(st, o, v["mode"].as_str().unwrap_or("glow"));
                }
                return;
            }
            Err(m) => m,
        };
        if let Ok(m) = msg.downcast::<Msg>() {
            match *m {
                Msg::Tick => self.step(st),
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
            self.end(st);
        }
    }

    fn surface_closed(&mut self, st: &mut State, s: &Rc<Surface>) {
        if self.win.as_ref().is_some_and(|w| Rc::ptr_eq(&w.surface, s)) {
            self.end(st);
        }
    }

    fn quit(&mut self, st: &mut State) {
        self.end(st);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_glow_comes_holds_and_goes() {
        assert_eq!(level(0.0, false, false), Some(0.0));
        assert_eq!(level(150.0, false, false), Some(1.0));
        assert_eq!(level(300.0, false, false), Some(1.0));
        assert!(level(500.0, false, false).unwrap() < 1.0);
        assert_eq!(level(851.0, false, false), None);
        // reduce motion: in 100, out 300
        assert_eq!(level(100.0, true, false), Some(1.0));
        assert_eq!(level(651.0, true, false), None);
        // the plain dim: 100 + 150 + 250
        assert_eq!(level(500.1, true, true), None);
        assert!((level(375.0, true, true).unwrap() - 0.5).abs() < 1e-3);
    }
}

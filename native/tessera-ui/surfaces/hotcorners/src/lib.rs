//! Hot corners (rust.surfaces: hotcorners), compare
//! shell/modules/hotcorners/HotCorners.qml: push the pointer into a screen
//! corner and hold it there for `delay` ms to run that corner's action
//! (Settings → Desktop → Hot corners). Each corner with an action is a 3 px
//! transparent layer surface at the very corner, per screen; corners set to
//! "none" have no surface. Nothing polls: the compositor says when the
//! pointer enters, and the only timer runs while it rests there. Fires once
//! per visit (leave the corner to use it again); a press inside cancels.
//! No action while the corner's screen shows a fullscreen window (Hyprland's
//! workspace `hasfullscreen`, read here) or while the screen is locked (the
//! shell checks System.locked when it runs the action: `hotcorners.run`).
//! The actions are the shell's (Panels, Show Desktop, the lock).

pub mod ui {
    slint::include_modules!();
}
tessera_core::theme_global!(ui);

// tessera-core's modules (shell, wayland, theme, services, util…) under the
// names they had when everything was one crate: `crate::shell::Part`
#[allow(unused_imports)]
use tessera_core::*;

use tessera_core::services::hypr::Snapshot;
use tessera_core::shell::{self, Part};
use tessera_core::wayland::{Spec, State, Surface};
use serde_json::{json, Value};
use slint::ComponentHandle;
use smithay_client_toolkit::reexports::client::protocol::wl_output::WlOutput;
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use std::any::{Any, TypeId};
use std::rc::Rc;

const NAME: &str = "hotcorners";
const NS: &str = "tessera:hotcorner";
pub const CORNERS: [&str; 4] = ["topLeft", "topRight", "bottomLeft", "bottomRight"];
/// The actions a corner can have (Config.options.hotCorners.<corner>)
pub const ACTIONS: [&str; 8] = ["overview", "missionControl", "desktop", "notificationCenter", "launchpad", "lock", "standby", "quickNote"];

/// A corner's action from the settings ("none": no surface there)
pub fn action_for(cfg: &Value, corner: &str) -> String {
    let d = match corner {
        "topLeft" => "overview",
        _ => "none",
    };
    cfg["hotCorners"][corner].as_str().unwrap_or(d).to_string()
}

/// How long the pointer rests before the action (ms)
pub fn delay(cfg: &Value) -> u64 {
    cfg["hotCorners"]["delay"].as_u64().unwrap_or(200).clamp(0, 10_000)
}

/// Where a corner's surface is anchored
pub fn anchor(corner: &str) -> Anchor {
    let v = if corner.starts_with("top") { Anchor::TOP } else { Anchor::BOTTOM };
    let h = if corner.ends_with("Left") { Anchor::LEFT } else { Anchor::RIGHT };
    v | h
}

/// The monitor's current space has a fullscreen window
pub fn fullscreen(h: &Snapshot, monitor: &str) -> bool {
    let Some(ws) = h.active.get(monitor) else { return false };
    h.workspaces.iter().any(|w| w.id == *ws && w.fullscreen)
}

/// Whether the action may run now (HotCorners.qml's allowed(); the lock is
/// the shell's to check)
pub fn allowed(hovered: bool, pressed: bool, fullscreen: bool) -> bool {
    hovered && !pressed && !fullscreen
}

enum Msg {
    Press(String, &'static str),
}

struct Corner {
    corner: &'static str,
    action: String,
    surface: Rc<Surface>,
    ui: ui::HotCorner,
    hovered: bool,
    pressed: bool,
    dwell: slint::Timer,
}

struct Screen {
    output: String,
    corners: Vec<Corner>,
}

#[derive(Default)]
pub struct HotCorners {
    screens: Vec<Screen>,
    /// the settings the corners were made for (rebuilt when they change)
    made_for: String,
}

impl HotCorners {
    fn make(&mut self, st: &mut State, output: &WlOutput, name: &str) {
        let mut corners = Vec::new();
        for corner in CORNERS {
            let action = action_for(&st.shell.cfg, corner);
            if action == "none" {
                continue;
            }
            let surface = st.prepare_surface(Spec {
                layer: Layer::Overlay,
                namespace: NS,
                output: Some((output, name)),
                anchor: anchor(corner),
                size: (3, 3),
                exclusive: -1,
                keyboard: KeyboardInteractivity::None,
                margin_left: 0,
            });
            let ui = ui::HotCorner::new().expect("HotCorner");
            let to = st.shell.sender(NAME);
            let n = name.to_string();
            ui.on_press(move || to(Box::new(Msg::Press(n.clone(), corner))));
            ui.show().expect("show hot corner");
            corners.push(Corner { corner, action, surface, ui, hovered: false, pressed: false, dwell: slint::Timer::default() });
        }
        self.screens.push(Screen { output: name.to_string(), corners });
    }

    fn drop_screen(st: &mut State, s: Screen) {
        for c in s.corners {
            c.surface.hide(); // (unmapped now: dropping the window alone leaves it on screen)
            st.drop_surface(&c.surface);
        }
    }

    fn rebuild(&mut self, st: &mut State) {
        for s in std::mem::take(&mut self.screens) {
            Self::drop_screen(st, s);
        }
        for (o, n) in st.outputs_now() {
            self.make(st, &o, &n);
        }
    }

    fn settings_key(cfg: &Value) -> String {
        CORNERS.iter().map(|c| action_for(cfg, c)).collect::<Vec<_>>().join(",")
    }

    fn corner_mut(&mut self, output: &str, corner: &str) -> Option<&mut Corner> {
        self.screens.iter_mut().find(|s| s.output == output)?.corners.iter_mut().find(|c| c.corner == corner)
    }

    fn hover(&mut self, st: &mut State, output: String, corner: &'static str, on: bool) {
        if crate::env::is_set("DEBUG") {
            eprintln!("debug: hot corner {corner} on {output}: hovered {on}");
        }
        let ms = delay(&st.shell.cfg);
        let tx = st.tx.clone();
        let Some(c) = self.corner_mut(&output, corner) else { return };
        c.hovered = on;
        c.pressed = false;
        if on {
            // (once per visit: the timer runs again only after leaving)
            let (o, k) = (output.clone(), corner);
            c.dwell.start(slint::TimerMode::SingleShot, std::time::Duration::from_millis(ms), move || {
                let _ = tx.send(tessera_core::Msg::Part(NAME, Box::new(Fire(o.clone(), k))));
            });
        } else {
            c.dwell.stop();
        }
    }

    fn fire(&mut self, st: &mut State, output: String, corner: &'static str) {
        let full = st.shell.get::<Snapshot>().is_some_and(|h| fullscreen(h, &output));
        if crate::env::is_set("DEBUG") {
            eprintln!("debug: hot corner {corner} on {output}: rested (fullscreen {full})");
        }
        let Some(c) = self.corner_mut(&output, corner) else { return };
        if !allowed(c.hovered, c.pressed, full) || !ACTIONS.contains(&c.action.as_str()) {
            return;
        }
        shell::ask("hotcorners.run", json!([c.action, output]));
    }
}

/// The dwell ran out (from the timer)
struct Fire(String, &'static str);

impl Part for HotCorners {
    fn name(&self) -> &'static str {
        NAME
    }

    fn start(&mut self, st: &mut State) {
        st.shell.want("hypr");
        self.made_for = Self::settings_key(&st.shell.cfg);
    }

    fn output_added(&mut self, st: &mut State, output: &WlOutput, name: &str) {
        if !self.screens.iter().any(|s| s.output == name) {
            self.make(st, output, name);
        }
    }

    fn output_removed(&mut self, st: &mut State, name: &str) {
        if let Some(i) = self.screens.iter().position(|s| s.output == name) {
            let s = self.screens.remove(i);
            Self::drop_screen(st, s);
        }
    }

    fn config_changed(&mut self, st: &mut State, _: bool) {
        let key = Self::settings_key(&st.shell.cfg);
        if key != self.made_for {
            self.made_for = key;
            self.rebuild(st);
        }
    }

    fn data(&mut self, _: &mut State, _: TypeId) {}

    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        let msg = match msg.downcast::<Fire>() {
            Ok(f) => return self.fire(st, f.0, f.1),
            Err(m) => m,
        };
        let Ok(m) = msg.downcast::<Msg>() else { return };
        match *m {
            Msg::Press(o, c) => {
                if let Some(c) = self.corner_mut(&o, c) {
                    c.pressed = true;
                    c.dwell.stop();
                }
            }
        }
    }

    /// The pointer came or went (read here: a corner never redraws, so a
    /// Slint change handler there wouldn't run)
    fn after_frame(&mut self, st: &mut State) {
        let changed: Vec<(String, &'static str, bool)> = self
            .screens
            .iter()
            .flat_map(|s| s.corners.iter().filter(|c| c.ui.get_hovered() != c.hovered).map(|c| (s.output.clone(), c.corner, c.ui.get_hovered())))
            .collect();
        for (o, c, on) in changed {
            self.hover(st, o, c, on);
        }
    }

    fn surface_closed(&mut self, st: &mut State, s: &Rc<Surface>) {
        if self.screens.iter().any(|x| x.corners.iter().any(|c| Rc::ptr_eq(&c.surface, s))) {
            let name = s.output.clone();
            self.output_removed(st, &name);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::hypr::Workspace;

    #[test]
    fn corners_and_their_actions() {
        let cfg = json!({});
        assert_eq!(action_for(&cfg, "topLeft"), "overview");
        assert_eq!(action_for(&cfg, "bottomRight"), "none");
        let cfg = json!({"hotCorners": {"topLeft": "none", "bottomRight": "lock", "delay": 500}});
        assert_eq!(action_for(&cfg, "topLeft"), "none");
        assert_eq!(action_for(&cfg, "bottomRight"), "lock");
        assert_eq!(delay(&cfg), 500);
        assert_eq!(delay(&json!({})), 200);
        assert_eq!(anchor("bottomRight"), Anchor::BOTTOM | Anchor::RIGHT);
        assert_eq!(anchor("topLeft"), Anchor::TOP | Anchor::LEFT);
        assert_eq!(HotCorners::settings_key(&json!({})), "overview,none,none,none");
    }

    #[test]
    fn not_over_fullscreen_or_while_pressed() {
        let mut h = Snapshot { workspaces: vec![Workspace { id: 1, fullscreen: true, ..Default::default() }, Workspace { id: 2, ..Default::default() }], ..Default::default() };
        h.active.insert("DP-1".into(), 1);
        h.active.insert("HDMI-A-1".into(), 2);
        assert!(fullscreen(&h, "DP-1"));
        assert!(!fullscreen(&h, "HDMI-A-1"));
        assert!(!fullscreen(&h, "nowhere"));
        assert!(allowed(true, false, false));
        assert!(!allowed(true, true, false));
        assert!(!allowed(false, false, false));
        assert!(!allowed(true, false, true));
    }
}

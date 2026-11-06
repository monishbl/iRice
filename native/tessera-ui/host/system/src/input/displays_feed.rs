//! What the Rust display overlays are told (wave 4b): DisplayConnect.qml and
//! DisplayOverlay.qml read DisplayFx and Displays as singletons; the surfaces
//! (surfaces/screenfx: displayconnect, displayoverlay) are separate parts
//! that hear JSON lines instead. After every turn of the host this reads what
//! displays.rs keeps on the bus ("displays", "displayfx", "cooldown") and,
//! when it changed, says
//!   {"to":"displayoverlay","identifying":…,"confirming":…,"left":N,"monitors":[{name,label,number,lw,lh}]}
//!   {"to":"displayconnect","runs":[…],"motion":1.0}
//! and mirrors the hotplug time into the shell state (`displayfx.hotplugAt`)
//! for host/desktop's space-switch cue, which stays quiet for 4 s after a
//! display comes or goes (DisplayFx.settling()).
//!
//! It also answers `tessera-ui ipc call cues …` (nothing in the QML shell; to
//! see the overlays without a second display): `identify` (Identify Displays
//! for 2.5 s: displays.identify), `confirm` (the Keep question for 6 s, a
//! preview only: nothing is pending, so "Go back" changes nothing), and
//! `space [output] [glow|dim]` (the space-switch cue on a display).
//!
//! The surfaces' answers come back as the calls the QML made on Displays:
//! `displays.keep`, `displays.revert` and `displays.finish [id]`, which
//! displays.rs answers. The 15 s countdown and the revert itself are in
//! displays.rs (Displays.qml's `confirming`), not here.

use serde_json::{json, Value};
use std::any::Any;
use tessera_core::host::{Host, Service};

/// The confirm preview ended
struct PreviewEnd;

#[derive(Default)]
pub struct DisplaysFeed {
    overlay: Value,
    connect: Value,
    /// the confirm preview is up: the real state is left unsaid meanwhile
    preview: bool,
}

impl DisplaysFeed {
    /// What displayoverlay shows, from the "displays" bus entry
    pub fn overlay_of(d: &Value) -> Value {
        let monitors: Vec<Value> = d["monitors"]
            .as_array()
            .map(|a| a.iter().map(|m| json!({"name": m["name"], "label": m["label"], "number": m["number"], "lw": m["lw"], "lh": m["lh"]})).collect())
            .unwrap_or_default();
        json!({
            "identifying": d["identifying"].as_bool().unwrap_or(false),
            "confirming": d["confirming"].as_bool().unwrap_or(false),
            "left": d["confirmLeft"].as_i64().unwrap_or(0),
            "monitors": monitors,
        })
    }
}

impl Service for DisplaysFeed {
    fn name(&self) -> &'static str {
        "DisplaysFeed"
    }

    fn ipc_targets(&self) -> &'static [&'static str] {
        &["cues"]
    }

    fn ipc(&mut self, h: &mut Host, _target: &str, func: &str, args: &[String]) -> Option<String> {
        match func {
            "identify" => h.ask("displays.identify", json!([])),
            "confirm" => {
                self.preview = true;
                let mut o = Self::overlay_of(&crate::bus::get("displays"));
                o["confirming"] = json!(true);
                o["left"] = json!(15);
                h.tell("displayoverlay", o);
                super::common::after(h.sender("DisplaysFeed"), 6000, PreviewEnd);
            }
            "space" => {
                let out = args.first().cloned().filter(|s| !s.is_empty()).or_else(|| h.st.outputs_now().into_iter().next().map(|o| o.1));
                let mode = if args.get(1).map(String::as_str) == Some("dim") { "dim" } else { "glow" };
                if let Some(out) = out {
                    h.tell("spaceswitch", json!({"cue": out, "mode": mode}));
                }
            }
            _ => return Some(format!("cues: no function {func} (identify, confirm, space [output] [glow|dim])")),
        }
        None
    }

    fn message(&mut self, _h: &mut Host, msg: Box<dyn Any + Send>) {
        if msg.downcast::<PreviewEnd>().is_ok() {
            self.preview = false;
            // (the real state is said again)
            self.overlay = Value::Null;
        }
    }

    fn settled(&mut self, h: &mut Host) {
        let d = crate::bus::get("displays");
        let fx = crate::bus::get("displayfx");
        if d.is_null() && fx.is_null() {
            return;
        }
        let overlay = Self::overlay_of(&d);
        if overlay != self.overlay && !self.preview {
            self.overlay = overlay.clone();
            h.tell("displayoverlay", overlay);
        }
        let motion = crate::bus::field("cooldown", "motionScale").as_f64().unwrap_or(1.0);
        let connect = json!({"runs": if fx["runs"].is_array() { fx["runs"].clone() } else { json!([]) }, "motion": motion});
        if connect != self.connect {
            self.connect = connect.clone();
            h.tell("displayconnect", connect);
        }
        h.set_state("displayfx.hotplugAt", fx["hotplugAt"].clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_overlay_gets_the_countdown_and_the_numbers() {
        let d = json!({"identifying": false, "confirming": true, "confirmLeft": 12, "loaded": true,
            "monitors": [{"name": "DP-1", "label": "Dell", "number": 1, "lw": 2560, "lh": 1440, "x": 0}]});
        let o = DisplaysFeed::overlay_of(&d);
        assert_eq!(o["left"], 12);
        assert_eq!(o["confirming"], true);
        assert_eq!(o["monitors"][0], json!({"name": "DP-1", "label": "Dell", "number": 1, "lw": 2560, "lh": 1440}));
        assert_eq!(DisplaysFeed::overlay_of(&Value::Null)["left"], 0);
    }
}

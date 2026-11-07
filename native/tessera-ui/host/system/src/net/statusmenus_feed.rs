//! The menu bar's status menus, fed (replaces what shell/modules/menubar/
//! StatusMenusFeed.qml does beside the QML shell): while one of them is open
//! (the shell's state `open`: batteryMenu, privacyMenu, resourcesMenu,
//! soundMenu… set by the host's Panels), what only this crate's services know
//! reaches the statusmenus surface as
//!   `{"to":"statusmenus", "health": BatteryHealth's {capacity, cycles, watts,
//!     condition}, "power": {ppd, hasPerformance, profile, short},
//!     "privacy": Privacy's state, "resources": Resources' {cpu, memory, swap,
//!     memUsedGb, memTotalGb, temperature}, "profiles": AudioProfiles'
//!     alternatives as [{name, kind, busy}]}`
//! (said at once on opening, then twice a second when it changed), and what
//! the menus ask (`shell::ask("statusmenus", [what, …])`) is done here:
//!   stopSharing             Privacy.stopSharing()
//!   mode <profile>          Power.setMode (power-saver | balanced | performance)
//!   profile <i>             AudioProfiles.pick(alternatives[i])
//!   btFix                   Backends.fixBluetooth()
//!   pin <bus>               Media.pinned (Now Playing's other players)
//! Opening the battery menu reads the battery's health again (the QML menu's
//! BatteryHealth.refresh), the Sound menu the card's profiles
//! (AudioProfiles.refresh), and Resources samples while its menu shows.

use super::common::*;
use serde_json::{json, Value};
use std::any::Any;
use std::time::Duration;
use tessera_core::host::{Host, Service};

const NAME: &str = "StatusMenusFeed";

/// The panels the statusmenus surface draws (the host's panels.rs STATUS_PANELS)
pub const PANELS: &[&str] = &["bluetoothMenu", "soundMenu", "privacyMenu", "batteryMenu", "resourcesMenu", "weatherMenu", "nowPlaying"];

enum M {
    Tick,
}

#[derive(Default)]
pub struct StatusMenusFeed {
    /// the open menu ("" none)
    open: String,
    /// Resources was asked to sample for the menu
    sampling: bool,
    last_sent: Value,
    tick: slint::Timer,
}

fn bus(name: &str) -> Value {
    crate::bus::get(name)
}

/// AudioProfiles' alternatives as Sound's page lists them (cc.rs map_alts)
pub fn profiles(ap: &Value) -> Value {
    let sw = ap["switching"].as_str().unwrap_or("").to_string();
    Value::Array(
        ap["alternatives"]
            .as_array()
            .map(|a| a.iter().map(|x| json!({"name": x["name"], "kind": x["kind"], "busy": !sw.is_empty() && x["profile"].as_str() == Some(sw.as_str())})).collect())
            .unwrap_or_default(),
    )
}

fn audio_profiles() -> Value {
    let ap = bus("AudioProfiles");
    if ap.is_null() { bus("audioprofiles") } else { ap }
}

impl StatusMenusFeed {
    fn state(&self, h: &Host) -> Value {
        let p = bus("Power");
        let r = bus("Resources");
        let f = |k: &str| r[k].as_f64().unwrap_or(0.0);
        let ppd = p["ppd"].as_bool().or_else(|| h.state("backends.ppd").as_bool()).unwrap_or(false);
        let health = bus("BatteryHealth");
        json!({
            "health": if health.is_null() { json!({"capacity": -1, "cycles": -1, "watts": -1, "condition": ""}) } else { health },
            "power": {"ppd": ppd, "hasPerformance": p["hasPerformance"].as_bool().unwrap_or(false),
                      "profile": if ppd { p["profile"].as_str().unwrap_or("balanced") } else { "" },
                      "short": h.state("backends.powerShort").as_str().unwrap_or("")},
            "privacy": bus("privacy"),
            "resources": {"cpu": f("cpu"), "memory": f("memory"), "swap": f("swap"), "memUsedGb": f("memUsedGb"), "memTotalGb": f("memTotalGb"), "temperature": f("temperature")},
            "profiles": profiles(&audio_profiles()),
        })
    }

    fn send(&mut self, h: &mut Host, force: bool) {
        let v = self.state(h);
        if force || v != self.last_sent {
            self.last_sent = v.clone();
            h.tell("statusmenus", v);
        }
    }

    fn sample(&mut self, h: &mut Host, on: bool) {
        if on != self.sampling {
            self.sampling = on;
            h.ask("resources.use", json!([{"kind": "graph", "on": on}]));
        }
    }
}

impl Service for StatusMenusFeed {
    fn name(&self) -> &'static str {
        NAME
    }
    fn calls(&self) -> &'static [&'static str] {
        &["statusmenus"]
    }
    fn start(&mut self, h: &mut Host) {
        let send = h.sender(NAME);
        self.tick.start(slint::TimerMode::Repeated, Duration::from_millis(500), move || send(Box::new(M::Tick)));
    }
    fn call(&mut self, h: &mut Host, call: &str, args: &Value) {
        let a = if call == "statusmenus" {
            args_list(args)
        } else if let Some(w) = call.strip_prefix("statusmenus.") {
            let mut v = vec![json!(w)];
            v.extend(args_list(args));
            v
        } else {
            return;
        };
        match str_arg(&a, 0).as_str() {
            "stopSharing" => h.ask("privacy.stopSharing", json!([])),
            "mode" => {
                let m = str_arg(&a, 1);
                if ["power-saver", "balanced", "performance"].contains(&m.as_str()) {
                    h.ask("power.setMode", json!([m]));
                }
            }
            "profile" => {
                let i = a.get(1).and_then(Value::as_i64).unwrap_or(-1);
                let ap = audio_profiles();
                if let Some(alt) = ap["alternatives"].as_array().and_then(|x| x.get(i.max(0) as usize).filter(|_| i >= 0)) {
                    h.ask("audioprofiles.pick", json!([alt]));
                }
            }
            "btFix" => h.ask("backends.fixBluetooth", json!([])),
            "pin" => h.ask("media.pin", json!([str_arg(&a, 1)])),
            _ => {}
        }
        if !self.open.is_empty() {
            self.send(h, false);
        }
    }
    fn message(&mut self, h: &mut Host, msg: Box<dyn Any + Send>) {
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Tick => {
                let now = h.state("open").as_str().unwrap_or("").to_string();
                let open = if PANELS.contains(&now.as_str()) { now } else { String::new() };
                if open != self.open {
                    // Opening (or another menu): fresh readings, said at once
                    match open.as_str() {
                        "batteryMenu" => h.ask("batteryhealth.refresh", json!([])),
                        "soundMenu" => h.ask("audioprofiles.refresh", json!([])),
                        _ => {}
                    }
                    self.sample(h, open == "resourcesMenu");
                    let opened = !open.is_empty();
                    self.open = open;
                    if opened {
                        self.send(h, true);
                    }
                    return;
                }
                if !self.open.is_empty() {
                    self.send(h, false);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiles_as_sound_lists_them() {
        let ap = json!({"switching": "hdmi", "alternatives": [{"name": "Monitor", "kind": "display", "profile": "hdmi"}, {"name": "Speakers", "kind": "", "profile": "analog"}]});
        assert_eq!(profiles(&ap), json!([{"name": "Monitor", "kind": "display", "busy": true}, {"name": "Speakers", "kind": "", "busy": false}]));
        assert_eq!(profiles(&Value::Null), json!([]));
        assert!(PANELS.contains(&"nowPlaying") && !PANELS.contains(&"wifiMenu"));
    }
}

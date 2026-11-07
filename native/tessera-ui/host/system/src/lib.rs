//! The host's system services (wave 4, core/src/host.rs): the QML shell's
//! services, standalone. Each module names the QML file(s) it replaces;
//! PORT-NOTES.md says what is ported, what differs, and how to check it.
//!
//! Four groups (one directory each): net (network, audio, brightness, power,
//! resources), input (displays, mouse, keyboard, devices bridges), sysinfo
//! (privacy, system, about, updates, root helper), accounts (login, boot,
//! fingerprint, cloud, accounts bridges, control centre bridge).

#[allow(unused_imports)]
use tessera_core::*;
use tessera_core::host::Service;

pub mod accounts;
pub mod bus;
pub mod input;
pub mod net;
pub mod sysinfo;
pub mod util;

/// Every service of this crate (the binary collects them: src/main.rs)
pub fn services() -> Vec<Box<dyn Service>> {
    let mut v = Vec::new();
    v.extend(net::services());
    v.extend(input::services());
    v.extend(sysinfo::services());
    v.extend(accounts::services());
    v.push(Box::new(Mirror::default()));
    v
}

/// What other crates read of this one's services, mirrored from the bus into
/// the shell's state after every turn (the lock reads `fingerprint.lockScreen`
/// and `face.lockScreen`, as Lock.qml read Fingerprint and Face), and the
/// menu bar's own fields that RustBridge.qml (modules/menubar) published from
/// Audio, BatteryHealth, PowerProfiles, Resources and Updates: audio, battery,
/// power, resources, updates (same names and values as its `state` object)
#[derive(Default)]
struct Mirror {
    resources_on: Option<bool>,
    /// the About card's logo path, looked up once per logo name
    logo: Option<(String, String)>,
}

fn hm(secs: i64) -> String {
    let m = (secs as f64 / 60.0).round() as i64;
    format!("{}:{:02}", m / 60, m % 60)
}

fn mode_name(profile: &str) -> &'static str {
    match profile {
        "power-saver" => "Low Power",
        "performance" => "Performance",
        _ => "Balanced",
    }
}

impl Mirror {
    fn publish(&mut self, h: &mut tessera_core::host::Host) {
        use serde_json::json;
        for (svc, key) in [("fingerprint", "lockScreen"), ("face", "lockScreen")] {
            let v = bus::field(svc, key);
            h.set_state(&format!("{svc}.{key}"), if v.is_null() { json!(false) } else { v });
        }
        // Audio: icon, volume, muted, sink, micMuted
        let a = bus::get("Audio");
        if !a.is_null() {
            let sink = a["sinkName"].as_str().filter(|s| !s.is_empty()).unwrap_or("Sound");
            h.set_state("audio", json!({"icon": a["icon"], "volume": a["volume"], "muted": a["muted"], "sink": sink, "micMuted": a["micMuted"]}));
        }
        // Power: profile ("" without the daemon), performance; ppd itself is host-shell's (Backends)
        let p = bus::get("Power");
        let ppd = p["ppd"].as_bool().unwrap_or(false);
        let profile = if ppd { p["profile"].as_str().unwrap_or("balanced").to_string() } else { String::new() };
        h.set_state("power.profile", json!(profile));
        h.set_state("power.performance", json!(p["hasPerformance"].as_bool().unwrap_or(false)));
        // Battery: the hover's one line ("84% · 3:20 left · Low Power"), and whether Low Power is on
        let b = h.st.shell.get::<tessera_core::services::status::Battery>().cloned().unwrap_or_default();
        let level = (if b.percent > 1.0 { b.percent / 100.0 } else { b.percent }).clamp(0.0, 1.0);
        let state = match b.state {
            4 => "Fully charged".to_string(),
            1 if b.time_to_full > 0 => format!("Full in {}", hm(b.time_to_full)),
            1 => "Charging".to_string(),
            5 => "Not charging".to_string(),
            _ if b.time_to_empty > 0 => format!("{} left", hm(b.time_to_empty)),
            _ => "On battery".to_string(),
        };
        let summary = format!("{}% · {}{}", (level * 100.0).round(), state, if ppd { format!(" · {}", mode_name(&profile)) } else { String::new() });
        h.set_state("battery", json!({"summary": summary, "lowPower": profile == "power-saver"}));
        // Resources: CPU and memory only while the bar shows them (each sample would wake it)
        let on = h.cfg()["menubar"]["resources"].as_bool().unwrap_or(false);
        if self.resources_on != Some(on) {
            self.resources_on = Some(on);
            h.ask("resources.use", json!([{"kind": "menubar", "on": on}]));
        }
        let r = bus::get("Resources");
        // (whatever was sampled, for whoever asked: the bar, or the expanded
        // island's last line, which asks resources.use "island" itself)
        h.set_state("resources", json!({"cpu": r["cpu"].as_f64().unwrap_or(0.0), "memory": r["memory"].as_f64().unwrap_or(0.0)}));
        // Power.has("cooldown"): Cool-down holds a request (the island's "running hot" wording)
        let cooling = p["requests"].as_array().is_some_and(|r| r.iter().any(|q| q["owner"] == "cooldown"));
        h.set_state("power.cooldown", json!(cooling));
        // Google.connected (the health check adds --google), and the About card's facts (AboutDialog.qml's info)
        h.set_state("google.connected", json!(bus::field("google", "connected").as_bool().unwrap_or(false)));
        let si = bus::get("systeminfo");
        if !si.is_null() {
            let want = si["osLogo"].as_str().filter(|l| !l.is_empty()).unwrap_or("distributor-logo").to_string();
            if self.logo.as_ref().map(|l| &l.0) != Some(&want) {
                let path = tessera_core::icon_theme::find(&want, None).or_else(|| tessera_core::icon_theme::find("computer", None)).map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
                self.logo = Some((want, path));
            }
            let logo = self.logo.as_ref().map(|l| l.1.clone()).unwrap_or_default();
            h.set_state("about.info", json!({"osName": si["osName"], "hostname": si["hostname"], "cpu": si["cpu"], "gpu": si["gpu"], "memory": si["memory"], "kernel": si["kernel"], "logo": logo}));
        }
        // Idle (Quickshell's IdleMonitor, ext-idle-notify): ScreenTime's 2 minutes and Google's and
        // CloudAccounts' 5, both respecting inhibitors (a video playing is use). Asking again is free.
        use tessera_core::idle;
        h.st.idle_set(idle::SCREEN_TIME, Some((120_000, true)));
        h.st.idle_set(idle::LONG, Some((300_000, true)));
        let i = h.st.shell.get::<idle::Idle>().cloned().unwrap_or_default();
        bus::put("idle", json!({"isIdle": i.is(idle::SCREEN_TIME), "isIdleLong": i.is(idle::LONG)}));
        // Updates: the count of pending packages
        h.set_state("updates", json!(bus::field("updates", "count").as_i64().unwrap_or(0)));
    }
}

impl Service for Mirror {
    fn name(&self) -> &'static str {
        "SystemMirror"
    }
    fn settled(&mut self, h: &mut tessera_core::host::Host) {
        self.publish(h);
    }
    fn data(&mut self, h: &mut tessera_core::host::Host, _what: std::any::TypeId) {
        self.publish(h);
    }
}

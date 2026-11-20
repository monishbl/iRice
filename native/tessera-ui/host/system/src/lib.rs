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
    /// the bus's generation as last mirrored (after a turn that changed
    /// nothing there, there's nothing to mirror: it ran after every turn)
    seen: Option<u64>,
    /// INBOUND's values as last put on the bus
    inbound: [Value; INBOUND.len()],
    /// Google's and CloudAccounts' bus generations as last handed to core
    /// (services::accounts: their events and tasks join the calendar and
    /// the to-dos)
    accounts_seen: Option<(u64, u64)>,
}

impl Mirror {
    /// The accounts' events and tasks for every calendar and to-do list,
    /// when Google's or CloudAccounts' state changed (`force`: the settings did)
    fn accounts(&mut self, cfg: &Value, force: bool) {
        let seen = (bus::generation_of("google"), bus::generation_of("cloudaccounts"));
        if !force && self.accounts_seen == Some(seen) {
            return;
        }
        self.accounts_seen = Some(seen);
        tessera_core::services::accounts::set(tessera_core::services::accounts::from_bus(&bus::get("google"), &bus::get("cloudaccounts"), cfg));
    }
}

use serde_json::Value;

/// What the other host crates keep in the shell's state that this crate's
/// services read from its bus, by the names the QML services had (the QML
/// read the singletons themselves: QuickShare.running and FastPair.busy /
/// levelsFor for the Bluetooth alerts, CoolDown.motionScale / savedRates for
/// Displays): (bus name, shell state key). 17 Nov: nothing put them, so the
/// phone's Bluetooth alerts weren't quiet during a Quick Share transfer, nor
/// earbuds' during Fast Pair, a connected pair showed no left/right/case
/// levels, and Displays could save a cool-down's lowered refresh rate as yours
const INBOUND: [(&str, &str); 3] = [("quickshare", "quickshare"), ("fastpair", "fastpair"), ("cooldown", "cooldown")];

/// INBOUND's values from the shell's state, where they changed
/// (compared in place: nothing is copied in a turn that changed none of them)
fn inbound_changes<'a>(state: impl Fn(&str) -> &'a Value, last: &[Value]) -> Vec<(usize, &'a Value)> {
    INBOUND.iter().enumerate().map(|(i, (_, key))| (i, state(key))).filter(|(i, v)| last.get(*i) != Some(*v)).collect()
}

/// The shell's "resources" state, at the resolution the bar and the island
/// show it (whole percents, whole degrees)
fn resources_state(cpu: f64, memory: f64, temperature: f64) -> Value {
    let pct = |v: f64| (v * 100.0).round() / 100.0;
    serde_json::json!({"cpu": pct(cpu), "memory": pct(memory), "temperature": temperature.round()})
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
        // (whatever was sampled, for whoever shows it: the bar, or the expanded
        // island's last line, which asks resources.use "desktop" itself; the
        // island's status line shows the temperature too, and turns it red
        // while hot. As they show it: whole percents and degrees, so a sample
        // that reads the same changes no state, and no part's turn or bar
        // redraw follows; nothing while neither shows it, 17 Nov: the bar
        // redrew whole every ~4 s for figures nobody saw. The fields alone:
        // the whole state carries a minute of history per figure.)
        let shown = bus::field("Resources", "shown").as_bool().unwrap_or(false);
        if shown || h.state("resources").is_null() {
            let f = |k: &str| bus::field("Resources", k).as_f64().unwrap_or(0.0);
            h.set_state("resources", resources_state(f("cpu"), f("memory"), f("temperature")));
        }
        // Power.has("cooldown"): Cool-down holds a request (the island's "running hot" wording)
        let cooling = p["requests"].as_array().is_some_and(|r| r.iter().any(|q| q["owner"] == "cooldown"));
        h.set_state("power.cooldown", json!(cooling));
        // Google.connected (the health check adds --google), and the About card's facts (AboutDialog.qml's info)
        h.set_state("google.connected", json!(bus::field("google", "connected").as_bool().unwrap_or(false)));
        let si = bus::get("systeminfo");
        if !si.is_null() {
            let want = si["osLogo"].as_str().filter(|l| !l.is_empty()).unwrap_or("distributor-logo").to_string();
            if self.logo.as_ref().map(|l| &l.0) != Some(&want) {
                // (looked up on the pool: a miss walks every theme's folders,
                // 150 ms at start; the card shows it when it comes)
                let path = self.logo.take().map(|l| l.1).unwrap_or_default();
                self.logo = Some((want.clone(), path));
                let send = h.sender("SystemMirror");
                tessera_core::work::spawn(move || {
                    let path = tessera_core::icon_theme::find(&want, None).or_else(|| tessera_core::icon_theme::find("computer", None)).map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
                    send(Box::new(Logo(want, path)));
                });
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
        // The displays, each with its level (the island's brightness list: IslandLevels)
        h.set_state("displays", bus::field("Brightness", "displays"));
        // (and one running: the island's activity)
        let running = bus::field("updates", "running");
        let tessera = bus::field("tesseraupdate", "running");
        if running.as_bool().unwrap_or(false) {
            let job = bus::field("updates", "job");
            h.set_state("sysupdate", json!({"running": running, "job": {"phase": job["phase"], "text": job["text"], "package": job["package"]}, "progress": bus::field("updates", "progress"), "cancellable": bus::field("updates", "cancellable"), "step": bus::field("updates", "step"), "cancel": "updates.cancel"}));
        } else if tessera.as_bool().unwrap_or(false) {
            // (Tessera's background update: the same activity, its own words)
            let job = bus::field("tesseraupdate", "job");
            let (title, _) = sysinfo::tessera_update::job_words(&job);
            h.set_state("sysupdate", json!({"running": true, "job": {"phase": job["phase"], "text": title, "package": ""}, "progress": bus::field("tesseraupdate", "fraction"), "cancellable": true, "step": "Tessera", "cancel": "tesseraupdate.cancel"}));
        } else {
            h.set_state("sysupdate", json!({"running": false}));
        }
    }
}

impl Service for Mirror {
    fn name(&self) -> &'static str {
        "SystemMirror"
    }
    fn settled(&mut self, h: &mut tessera_core::host::Host) {
        for (i, v) in inbound_changes(|k| h.state(k), &self.inbound) {
            bus::put(INBOUND[i].0, v.clone());
            self.inbound[i] = v.clone();
        }
        let g = bus::generation();
        if self.seen == Some(g) {
            return;
        }
        self.seen = Some(g);
        self.accounts(h.cfg(), false);
        self.publish(h);
    }
    fn data(&mut self, h: &mut tessera_core::host::Host, what: std::any::TypeId) {
        // (only the readings publish() takes from the host; the rest come
        // through the bus, which settled() follows by its generation: a
        // Hyprland snapshot per focus change no longer redoes the whole mirror)
        if mirror_reads(what) {
            self.publish(h);
        }
    }
    fn config_changed(&mut self, h: &mut tessera_core::host::Host) {
        let cfg = h.cfg().clone();
        self.accounts(&cfg, true);
        self.publish(h);
    }
    fn message(&mut self, h: &mut tessera_core::host::Host, msg: Box<dyn std::any::Any + Send>) {
        if let Ok(l) = msg.downcast::<Logo>() {
            let Logo(name, path) = *l;
            if self.logo.as_ref().is_some_and(|l| l.0 == name) {
                self.logo = Some((name, path));
                self.publish(h);
            }
        }
    }
}

/// The About card's logo, found off the UI thread: its name and file
struct Logo(String, String);

/// A core reading the mirror publishes from (the battery, idle); others
/// change nothing it says
fn mirror_reads(what: std::any::TypeId) -> bool {
    use std::any::TypeId;
    what == TypeId::of::<tessera_core::services::status::Battery>() || what == TypeId::of::<tessera_core::idle::Idle>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn only_the_battery_and_idle_readings_republish() {
        assert!(mirror_reads(std::any::TypeId::of::<tessera_core::services::status::Battery>()));
        assert!(mirror_reads(std::any::TypeId::of::<tessera_core::idle::Idle>()));
        assert!(!mirror_reads(std::any::TypeId::of::<String>()));
    }

    #[test]
    fn resources_change_only_as_shown() {
        // (two samples the bar shows alike: the same state, no redraw)
        assert_eq!(resources_state(0.1234, 0.5012, 47.2), resources_state(0.1249, 0.4996, 46.8));
        assert_eq!(resources_state(0.1234, 0.5012, 47.2), json!({"cpu": 0.12, "memory": 0.5, "temperature": 47.0}));
        assert_ne!(resources_state(0.124, 0.5, 47.0), resources_state(0.126, 0.5, 47.0));
    }

    #[test]
    fn the_other_hosts_state_reaches_this_crates_bus() {
        let state = json!({"quickshare": {"running": true}, "fastpair": {"busy": false, "levels": {}}, "cooldown": {"motionScale": 1.5, "savedRates": {"eDP-1": 120.0}}});
        let none: [Value; 3] = Default::default();
        let changed = inbound_changes(|k| &state[k], &none);
        assert_eq!(changed.iter().map(|(i, _)| INBOUND[*i].0).collect::<Vec<_>>(), ["quickshare", "fastpair", "cooldown"]);
        // (one the state doesn't have yet: Null, as the bus had, so not a change)
        let partial = json!({"quickshare": {"running": false}});
        assert_eq!(inbound_changes(|k| &partial[k], &none).len(), 1);
        let mut last = none.clone();
        for (i, v) in changed {
            last[i] = v.clone();
        }
        // nothing changed: nothing put
        assert!(inbound_changes(|k| &state[k], &last).is_empty());
        // the names this crate's services read
        assert_eq!(INBOUND.map(|(b, _)| b), ["quickshare", "fastpair", "cooldown"]);
    }
}

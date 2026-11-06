//! The menu bar's Wi-Fi menu, fed (replaces modules/network/NetworkFeed.qml):
//! while the menu is open, what only the shell knows reaches the network
//! surface as `{"to":"network","wifi":…,"speed":…,"anchorX":…}` (Host::tell),
//! and what the surface asks (`shell::ask("network", [what, …])`) is done
//! here: the speed test, the switch, rescanning, joining, forgetting, Settings.
//!
//! The Wi-Fi networks come from `nmcli` (Quickshell's Networking API is
//! NetworkManager's too): listed while the menu is open, every 4 s (not while
//! the pointer is over the menu or a password is being typed: `hold`), and
//! at once on opening or Rescan. The switch and adapter are core's
//! services::status::Wifi. Joining with a password goes through WifiJoin's
//! script (the password stays off command lines); connecting a known network
//! is `nmcli dev wifi connect`.
//!
//! "Open" is the shell's state (`open` == "wifiMenu", set by the host's
//! Panels); `anchorX` is its state too (Panels.anchorX).

use super::common::*;
use super::speed_test::{format, format_ms, format_bytes, verdict};
use serde_json::{json, Value};
use std::any::Any;
use std::collections::BTreeSet;
use std::time::Duration;
use tessera_core::host::{Host, Service};
use tessera_core::services::status::Wifi;

const NAME: &str = "NetworkFeed";

/// One row of `nmcli -t -f IN-USE,SSID,SIGNAL,SECURITY dev wifi list`
#[derive(Clone, Debug, PartialEq)]
pub struct Ap {
    pub name: String,
    /// 0..1
    pub signal: f64,
    pub connected: bool,
    pub open: bool,
    pub enterprise: bool,
}

pub fn parse_list(text: &str) -> Vec<Ap> {
    let mut out: Vec<Ap> = vec![];
    for l in text.lines() {
        let f: Vec<String> = l.replace("\\:", "\u{1}").split(':').map(|x| x.replace('\u{1}', ":")).collect();
        if f.len() < 4 || f[1].is_empty() {
            continue;
        }
        let sec = f[3..].join(":");
        let ap = Ap {
            name: f[1].clone(),
            signal: f[2].parse::<f64>().unwrap_or(0.0) / 100.0,
            connected: f[0] == "*",
            open: sec.trim().is_empty() || sec.trim() == "--",
            enterprise: sec.contains("802.1X") || sec.contains("EAP"),
        };
        // (one row per network: its strongest access point; the connected one wins)
        match out.iter_mut().find(|a| a.name == ap.name) {
            Some(a) => {
                if ap.connected || (!a.connected && ap.signal > a.signal) {
                    let c = a.connected || ap.connected;
                    *a = Ap { connected: c, ..ap };
                }
            }
            None => out.push(ap),
        }
    }
    out
}

/// Names of the saved Wi-Fi profiles (`nmcli -t -f NAME,TYPE connection show`)
pub fn parse_known(text: &str) -> BTreeSet<String> {
    text.lines().filter_map(|l| {
        let f: Vec<String> = l.replace("\\:", "\u{1}").split(':').map(|x| x.replace('\u{1}', ":")).collect();
        (f.len() >= 2 && f[f.len() - 1] == "802-11-wireless").then(|| f[..f.len() - 1].join(":"))
    }).collect()
}

/// Connected first, then known, then by signal; at most 30 (sortedNow)
pub fn sorted(aps: &[Ap], known: &BTreeSet<String>) -> Vec<Ap> {
    let mut v = aps.to_vec();
    v.sort_by(|a, b| {
        b.connected.cmp(&a.connected)
            .then(known.contains(&b.name).cmp(&known.contains(&a.name)))
            .then(b.signal.partial_cmp(&a.signal).unwrap_or(std::cmp::Ordering::Equal))
    });
    v.truncate(30);
    v
}

/// The `speed` object of the feed, from SpeedTest's bus state
pub fn build_speed(st: &Value) -> Value {
    let stage = st["stage"].as_str().unwrap_or("idle");
    let has_run = stage != "idle" && stage != "error";
    let last = &st["last"];
    let shown: &Value = if has_run { st } else { last };
    let running = st["running"] == true;
    let f = |v: &Value, k: &str| v[k].as_f64().unwrap_or(-1.0);
    let mut parts = vec![];
    if !shown.is_null() && f(shown, "jitter") >= 0.0 {
        parts.push(format!("Jitter {}", format_ms(f(shown, "jitter"))));
    }
    if !shown.is_null() && (f(shown, "loadedDown") >= 0.0 || f(shown, "loadedUp") >= 0.0) {
        parts.push(format!("Under load {} ↓ {} ↑", format_ms(f(shown, "loadedDown")), format_ms(f(shown, "loadedUp"))));
    }
    let join = |a: &str, b: &str, sep: &str| [a, b].iter().filter(|x| !x.is_empty()).cloned().collect::<Vec<_>>().join(sep);
    let s = |v: &Value| v.as_str().unwrap_or("").to_string();
    let srv = if has_run {
        join(&join(&s(&st["server"]["name"]), &s(&st["server"]["location"]), ", "), &s(&st["client"]["isp"]), " · ")
    } else {
        join(&s(&last["server"]), &s(&last["isp"]), " · ")
    };
    let phase = match stage { "latency" => "Measuring ping…", "download" => "Testing download…", "upload" => "Testing upload…", _ => "" };
    let sub = if running { phase.to_string() } else if stage == "done" { format!("Just now · {} used", format_bytes(f(st, "usedBytes").max(0.0))) } else { String::new() };
    let verdict_s = if has_run { verdict(st) } else { verdict(last) };
    json!({
        "running": running, "stage": stage, "sub": sub,
        "lastAt": if !running && stage != "done" && stage != "error" && !last.is_null() { last["at"].as_f64().unwrap_or(0.0) } else { 0.0 },
        "live": if stage == "latency" { format_ms(f(st, "ping")) } else { format(st["live"].as_f64().unwrap_or(0.0)) },
        "progress": st["progress"].as_f64().unwrap_or(0.0),
        "showStats": !shown.is_null() && stage != "error",
        "stats": [
            {"label": "Download", "icon": "arrow-down", "value": format(f(shown, "download")), "active": stage == "download"},
            {"label": "Upload", "icon": "arrow-up", "value": format(f(shown, "upload")), "active": stage == "upload"},
            {"label": "Ping", "icon": "timer", "value": format_ms(f(shown, "ping")), "active": stage == "latency"},
        ],
        "resp": parts.join(" · "), "verdict": verdict_s, "server": srv,
        "error": if stage == "error" { s(&st["error"]) } else { String::new() },
        "action": if running { "Cancel" } else if stage == "done" || stage == "error" { "Test again" } else { "Test speed" },
    })
}

enum M {
    Tick,
    Listed(String, String),
    Connected(String, bool),
}

#[derive(Default)]
pub struct NetworkFeed {
    tick: slint::Timer,
    open: bool,
    hovered: bool,
    asking: bool,
    aps: Vec<Ap>,
    known: BTreeSet<String>,
    /// the snapshot the menu shows (refreshed, not live)
    shown: Vec<Ap>,
    failed: String,
    connecting: String,
    listing: bool,
    ticks: u32,
    last_sent: Value,
}

impl NetworkFeed {
    fn nm_running() -> bool {
        std::path::Path::new("/run/NetworkManager").exists() && has_tool("nmcli")
    }

    fn list(&mut self, h: &mut Host) {
        if self.listing || !has_tool("nmcli") {
            return;
        }
        self.listing = true;
        let send = h.sender(NAME);
        std::thread::spawn(move || {
            let t = Duration::from_secs(8);
            let a = crate::util::capture(&["nmcli", "-t", "-f", "IN-USE,SSID,SIGNAL,SECURITY", "dev", "wifi", "list", "--rescan", "no"], t).map(|x| x.1).unwrap_or_default();
            let k = crate::util::capture(&["nmcli", "-t", "-f", "NAME,TYPE", "connection", "show"], t).map(|x| x.1).unwrap_or_default();
            send(Box::new(M::Listed(a, k)));
        });
    }

    /// A snapshot, not a live sort (WifiList): taken on opening, on Rescan,
    /// and every few seconds when the pointer isn't over the menu
    fn refresh(&mut self, force: bool) {
        let next = sorted(&self.aps, &self.known);
        if force || next != self.shown {
            self.shown = next;
        }
    }

    fn wifi(&self, h: &Host) -> Value {
        let st = h.st.shell.get::<Wifi>().cloned().unwrap_or_default();
        let unmanaged = !Self::nm_running();
        let nets: Vec<Value> = self.shown.iter().map(|n| {
            let joining = super::wifi_join::joining(&n.name);
            let failed = super::wifi_join::failed_for(&n.name);
            let known = self.known.contains(&n.name);
            let connecting = self.connecting == n.name || joining;
            json!({
                "name": n.name, "signal": n.signal, "known": known, "connected": n.connected, "open": n.open, "secured": !n.open, "enterprise": n.enterprise,
                "status": if n.connected { "Connected" } else if connecting { "Connecting…" } else if self.failed == n.name || !failed.is_empty() { "Couldn't connect" } else if known { "Known network" } else { "" },
                "busy": connecting, "failed": failed,
            })
        }).collect();
        json!({
            "adapter": st.present, "enabled": st.enabled, "unmanaged": unmanaged,
            "notice": {
                "title": if unmanaged { if has_tool("nmcli") { "No network service is running" } else { "No network service is running" } } else { "" },
                "text": if !unmanaged { "" } else if std::path::Path::new("/usr/bin/NetworkManager").exists() { "Tessera's Wi-Fi menus need NetworkManager, which is installed but not running." } else { "Tessera's Wi-Fi menus need NetworkManager. Install networkmanager, or set up another network service." },
                "action": if unmanaged && std::path::Path::new("/usr/bin/NetworkManager").exists() { "Start NetworkManager" } else { "" },
                "busy": false,
            },
            "networks": nets,
        })
    }

    fn send(&mut self, h: &mut Host, force: bool) {
        let mut v = json!({"wifi": self.wifi(h), "speed": build_speed(&crate::bus::get("SpeedTest"))});
        if let Some(x) = h.state("anchorX").as_f64() {
            v["anchorX"] = json!(x);
        }
        if force || v != self.last_sent {
            self.last_sent = v.clone();
            h.tell("network", v);
        }
    }
}

impl Service for NetworkFeed {
    fn name(&self) -> &'static str {
        NAME
    }
    fn calls(&self) -> &'static [&'static str] {
        &["network"]
    }
    fn start(&mut self, h: &mut Host) {
        h.st.shell.want("status");
        let send = h.sender(NAME);
        self.tick.start(slint::TimerMode::Repeated, Duration::from_millis(250), move || send(Box::new(M::Tick)));
    }
    fn call(&mut self, h: &mut Host, call: &str, args: &Value) {
        // `network` [what, …] (the Wi-Fi menu), or `network.<what>` […] (the other crates': the same, without the first word)
        let a = if call == "network" {
            args_list(args)
        } else if let Some(w) = call.strip_prefix("network.") {
            let mut v = vec![json!(w)];
            v.extend(args_list(args));
            v
        } else {
            return;
        };
        let name = str_arg(&a, 1);
        let flag = |i: usize| a.get(i).and_then(Value::as_bool).unwrap_or(false);
        match str_arg(&a, 0).as_str() {
            "hold" => {
                self.hovered = flag(1);
                self.asking = flag(2);
            }
            "speedStart" => h.ask("speedtest.start", Value::Null),
            "speedCancel" => h.ask("speedtest.cancel", Value::Null),
            // a Wi-Fi code read by Live Text (a network to join, {ssid, hidden, security, password})
            "joinWifi" => {
                let w = a.get(1).cloned().unwrap_or(Value::Null);
                if let Some(ssid) = w["ssid"].as_str().filter(|s| !s.is_empty()) {
                    h.ask("wifijoin.join", json!({"ssid": ssid, "hidden": w["hidden"], "security": if w["security"] == "nopass" { "open" } else { "psk" }, "psk": w["password"]}));
                }
            }
            "wifi" | "setWifi" => crate::util::run_async(&["nmcli", "radio", "wifi", if flag(1) { "on" } else { "off" }]),
            "rescan" => {
                crate::util::run_async(&["nmcli", "dev", "wifi", "rescan"]);
                self.refresh(true);
                self.list(h);
            }
            "connect" => {
                if !name.is_empty() {
                    if self.failed == name {
                        self.failed.clear();
                    }
                    self.connecting = name.clone();
                    let send = h.sender(NAME);
                    let n2 = name.clone();
                    capture_then(vec!["nmcli".into(), "-w".into(), "45".into(), "dev".into(), "wifi".into(), "connect".into(), name], None, move |code, _| send(Box::new(M::Connected(n2, code == 0))));
                }
            }
            "disconnect" => {
                if !name.is_empty() {
                    crate::util::run_async(&["nmcli", "connection", "down", "id", &name]);
                }
            }
            "join" => {
                // (the password: only here, never logged)
                if let Some(psk) = a.get(2).and_then(Value::as_str) {
                    if !name.is_empty() {
                        self.failed.clear();
                        h.ask("wifijoin.join", json!({"ssid": name, "security": "psk", "psk": psk}));
                    }
                }
            }
            "forget" => {
                if !name.is_empty() {
                    h.ask("wifijoin.forget", json!([name]));
                    h.ask("wifijoin.loadKnown", Value::Null);
                    self.known.remove(&name);
                    self.refresh(true);
                }
            }
            "copy" => {
                if !name.is_empty() {
                    crate::util::run_async(&["wl-copy", "--", &name]);
                }
            }
            "settings" => h.ask("panels.openSettings", json!(["network"])),
            "fix" => h.ask("backends.fix", json!(["networkmanager"])),
            _ => {}
        }
        self.send(h, false);
    }
    fn message(&mut self, h: &mut Host, msg: Box<dyn Any + Send>) {
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Tick => {
                let open = h.state("open") == "wifiMenu";
                if open && !self.open {
                    // Opening: fresh rows and Ookla's tools looked up (as the card does), and said at once
                    self.ticks = 0;
                    self.list(h);
                    self.refresh(true);
                    h.ask("speedtest.probe", Value::Null);
                    self.open = true;
                    self.send(h, true);
                    return;
                }
                if !open {
                    self.open = false;
                    self.hovered = false;
                    self.asking = false;
                    return;
                }
                self.ticks += 1;
                if self.ticks % 16 == 0 {
                    self.list(h);
                }
                self.send(h, false);
            }
            M::Listed(a, k) => {
                self.listing = false;
                self.aps = parse_list(&a);
                self.known = parse_known(&k);
                // not while the pointer is over the menu or a password is being typed
                if !self.hovered && !self.asking {
                    self.refresh(false);
                }
                if self.shown.is_empty() {
                    self.refresh(true);
                }
                if self.open {
                    self.send(h, false);
                }
            }
            M::Connected(name, ok) => {
                if self.connecting == name {
                    self.connecting.clear();
                }
                if !ok {
                    self.failed = name;
                } else if self.failed == name {
                    self.failed.clear();
                }
                self.list(h);
                if self.open {
                    self.send(h, false);
                }
            }
        }
    }
    fn quit(&mut self, _h: &mut Host) {
        self.tick.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_networks_and_keeps_the_strongest() {
        let t = "*:Home:82:WPA2\n:Home:60:WPA2\n:Cafe\\: 5G:45:\n:Office:70:WPA2 802.1X\n::50:WPA2\n";
        let aps = parse_list(t);
        assert_eq!(aps.len(), 3);
        assert!(aps[0].connected && aps[0].signal == 0.82 && !aps[0].open);
        assert!(aps[1].name == "Cafe: 5G" && aps[1].open);
        assert!(aps[2].enterprise);
        let known = parse_known("Home:802-11-wireless\nVPN:vpn\nCafe\\: 5G:802-11-wireless\n");
        assert_eq!(known.len(), 2);
        let s = sorted(&aps, &known);
        assert_eq!(s[0].name, "Home");
        assert_eq!(s[1].name, "Cafe: 5G"); // known before the stronger unknown
    }

    #[test]
    fn the_speed_object() {
        let idle = json!({"stage": "idle", "running": false, "last": {"at": 5.0, "download": 100e6, "upload": 20e6, "ping": 10.0, "jitter": 2.0, "loadedDown": 30.0, "loadedUp": 40.0, "server": "Cloudflare AMS", "isp": "Fiber"}});
        let s = build_speed(&idle);
        assert_eq!((s["action"].as_str(), s["lastAt"].as_f64(), s["server"].as_str()), (Some("Test speed"), Some(5.0), Some("Cloudflare AMS · Fiber")));
        assert_eq!(s["stats"][0]["value"], "100 Mbps");
        assert_eq!(s["resp"], "Jitter 2.0 ms · Under load 30 ms ↓ 40 ms ↑");
        let run = json!({"stage": "download", "running": true, "live": 42e6, "progress": 0.4, "ping": 12.0, "download": -1.0, "upload": -1.0});
        let r = build_speed(&run);
        assert_eq!((r["sub"].as_str(), r["live"].as_str(), r["action"].as_str()), (Some("Testing download…"), Some("42.0 Mbps"), Some("Cancel")));
        assert_eq!(r["stats"][0]["active"], true);
        let err = build_speed(&json!({"stage": "error", "error": "offline"}));
        assert_eq!((err["showStats"].as_bool(), err["error"].as_str(), err["action"].as_str()), (Some(false), Some("offline"), Some("Test again")));
    }
}

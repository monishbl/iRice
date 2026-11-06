//! Internet speed test, run only when asked (replaces services/SpeedTest.qml):
//! Cloudflare's through curl by default, Ookla's when chosen and installed.
//! The work is in assets/scripts/speedtest.sh, which reports each step as it
//! goes, so the numbers fill in live. The last results are kept in
//! speedtest.json next to state.json. Also: the details of the link you're on
//! (Wi-Fi signal, rate, band, channel), read when asked.
//!
//! Calls (prefix "speedtest"): start, cancel, probe, readLink, clearHistory.
//! The state is on the bus as "speedtest" with the QML's field names (stage,
//! ping, jitter, download, upload, loadedDown, loadedUp, live, progress,
//! usedBytes, server, client, resultUrl, error, hasOokla, hasOoklaPy,
//! ooklaReady, history, link, backend, running); the strings the QML
//! formats are the pub fns below (format, format_ms, format_bytes, verdict,
//! signal_words).

use super::common::*;
use serde_json::{json, Map, Value};
use std::any::Any;
use tessera_core::host::{Host, Service};

const NAME: &str = "SpeedTest";
const PHASE_SECS: f64 = 9.0; // (as in the script)
pub const OOKLA_INSTALL: &str = "Install Ookla's speedtest with “paru -S ookla-speedtest-bin” (or “sudo pacman -S speedtest-cli”), or switch to Cloudflare.";

// ── Strings (the QML's format helpers) ─────────────────────────────────────

fn to_fixed(v: f64, d: usize) -> String {
    format!("{v:.d$}")
}

/// SpeedTest.format(bits/s)
pub fn format(bps: f64) -> String {
    if bps < 0.0 {
        "—".into()
    } else if bps >= 1e9 {
        format!("{} Gbps", to_fixed(bps / 1e9, if bps >= 1e10 { 0 } else { 2 }))
    } else if bps >= 1e6 {
        format!("{} Mbps", to_fixed(bps / 1e6, if bps >= 1e8 { 0 } else { 1 }))
    } else {
        format!("{} kbps", (bps / 1e3).round())
    }
}

pub fn format_ms(ms: f64) -> String {
    if ms < 0.0 {
        "—".into()
    } else if ms < 10.0 {
        format!("{} ms", to_fixed(ms, 1))
    } else {
        format!("{} ms", ms.round())
    }
}

pub fn format_bytes(b: f64) -> String {
    if b >= 1e9 {
        format!("{} GB", to_fixed(b / 1e9, 2))
    } else {
        format!("{} MB", (b / 1e6).round())
    }
}

/// How a line is for everyday use, from the numbers of a result ({download,
/// upload, ping, loadedDown, loadedUp}; "" for none)
pub fn verdict(r: &Value) -> String {
    if r.is_null() || r["download"].as_f64().unwrap_or(-1.0) < 0.0 {
        return String::new();
    }
    let f = |k: &str, d: f64| r[k].as_f64().unwrap_or(d);
    let lag = f("loadedDown", -1.0).max(f("loadedUp", -1.0));
    let (dl, ul, ping) = (f("download", -1.0), f("upload", -1.0), f("ping", -1.0));
    if dl >= 100e6 && ul >= 20e6 && ping < 40.0 && lag < 150.0 {
        "Great for everything, including 4K video and calls".into()
    } else if dl >= 25e6 && ul >= 5e6 && ping < 80.0 {
        let line = if lag > 300.0 { "Fast, but lag spikes when the line is busy" } else { "Good for HD video, calls and gaming" };
        line.into()
    } else if dl >= 5e6 {
        "Fine for browsing and HD video; calls may stutter".into()
    } else {
        "Slow: pages and video will lag".into()
    }
}

/// Wi-Fi signal, in words
pub fn signal_words(dbm: f64) -> &'static str {
    if dbm >= -55.0 {
        "Excellent"
    } else if dbm >= -67.0 {
        "Good"
    } else if dbm >= -75.0 {
        "Fair"
    } else {
        "Weak"
    }
}

pub fn median(a: &[f64]) -> f64 {
    if a.is_empty() {
        return -1.0;
    }
    let mut s = a.to_vec();
    s.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
    let m = s.len() / 2;
    if s.len() % 2 == 1 { s[m] } else { (s[m - 1] + s[m]) / 2.0 }
}

// ── The link ───────────────────────────────────────────────────────────────

fn num_prefix(s: &str) -> Option<f64> {
    let t: String = s.trim().chars().take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-').collect();
    t.parse().ok()
}

/// The `readLink` script's output as the link object (SpeedTest.link)
pub fn parse_link(text: &str) -> Value {
    let mut l = Map::new();
    let mut nm: Option<Value> = None;
    let mut standard = String::new();
    for raw in text.split('\n') {
        let s = raw.trim();
        for k in ["iface", "kind", "security", "speed"] {
            if let Some(v) = s.strip_prefix(&format!("{k}=")) {
                l.insert(k.into(), json!(v));
            }
        }
        if let Some(rest) = s.strip_prefix("nm=") {
            // NetworkManager's view: security, and the rest when iw isn't
            // installed (nmcli -t escapes ":" inside fields)
            let f: Vec<String> = rest.replace("\\:", "\u{1}").split(':').map(|x| x.replace('\u{1}', ":")).collect();
            let g = |i: usize| f.get(i).cloned().unwrap_or_default();
            l.insert("security".into(), json!(if g(0) == "--" { String::new() } else { g(0) }));
            nm = Some(json!({"ssid": g(1), "percent": num_prefix(&g(2)).unwrap_or(0.0), "freq": num_prefix(&g(3)), "rate": num_prefix(&g(4))}));
        } else if let Some(v) = s.strip_prefix("SSID: ") {
            l.insert("ssid".into(), json!(v));
        } else if let Some(v) = s.strip_prefix("freq: ") {
            if let Some(f) = num_prefix(v) {
                l.insert("freq".into(), json!(f.round()));
            }
        } else if let Some(v) = s.strip_prefix("signal: ") {
            if let Some(f) = num_prefix(v) {
                l.insert("signal".into(), json!(f.trunc()));
            }
        } else if s.starts_with("rx bitrate: ") || s.starts_with("tx bitrate: ") {
            let dir = &s[..2];
            let rest = &s[12..];
            if let Some(i) = rest.find(" MBit/s") {
                if let Ok(r) = rest[..i].parse::<f64>() {
                    l.insert(dir.into(), json!(r));
                    let tail = &rest[i + 7..];
                    if let Some(j) = tail.find("MHz") {
                        let digits: String = tail[..j].chars().rev().take_while(char::is_ascii_digit).collect::<Vec<_>>().into_iter().rev().collect();
                        if let Ok(w) = digits.parse::<f64>() {
                            l.insert("width".into(), json!(w));
                        }
                    }
                    standard = if tail.contains("EHT") {
                        "Wi-Fi 7".into()
                    } else if tail.contains("HE-") {
                        "Wi-Fi 6".into()
                    } else if tail.contains("VHT") {
                        "Wi-Fi 5".into()
                    } else if tail.contains("MCS") {
                        "Wi-Fi 4".into()
                    } else {
                        standard
                    };
                }
            }
        }
    }
    if let Some(nm) = nm {
        if l.get("ssid").and_then(Value::as_str).unwrap_or("").is_empty() {
            l.insert("ssid".into(), nm["ssid"].clone());
        }
        if l.get("freq").and_then(Value::as_f64).unwrap_or(0.0) == 0.0 {
            if let Some(f) = nm["freq"].as_f64().filter(|f| *f != 0.0) {
                l.insert("freq".into(), json!(f));
            }
        }
        if !l.contains_key("rx") {
            if let Some(r) = nm["rate"].as_f64().filter(|r| *r != 0.0) {
                l.insert("rx".into(), json!(r));
            }
        }
        if !l.contains_key("signal") && nm["percent"].as_f64().unwrap_or(0.0) > 0.0 {
            l.insert("signal".into(), json!((nm["percent"].as_f64().unwrap_or(0.0) / 2.0 - 100.0).round())); // NetworkManager's own percent <-> dBm mapping
        }
    }
    if let Some(f) = l.get("freq").and_then(Value::as_f64).filter(|f| *f != 0.0) {
        let band = if f >= 5925.0 { "6 GHz" } else if f >= 4900.0 { "5 GHz" } else { "2.4 GHz" };
        l.insert("band".into(), json!(band));
        let ch = if f == 2484.0 { 14.0 } else if f < 2484.0 { (f - 2407.0) / 5.0 } else if f >= 5925.0 { (f - 5950.0) / 5.0 } else { (f - 5000.0) / 5.0 };
        l.insert("channel".into(), json!(ch));
        if band == "6 GHz" && standard != "Wi-Fi 7" {
            standard = "Wi-Fi 6E".into();
        }
    }
    if !standard.is_empty() {
        l.insert("standard".into(), json!(standard));
    }
    if let Some(sig) = l.get("signal").and_then(Value::as_f64) {
        l.insert("quality".into(), json!(((sig + 90.0) / 60.0).clamp(0.0, 1.0))); // -90 dBm -> 0, -30 -> 1
    }
    // (only NetworkManager says; without it the security isn't known)
    match l.get("security").and_then(Value::as_str) {
        Some("") => {
            l.insert("security".into(), json!("Open"));
        }
        None => {
            l.insert("security".into(), json!(""));
        }
        _ => {}
    }
    if l.get("kind").and_then(Value::as_str) == Some("wifi") && l.get("ssid").and_then(Value::as_str).unwrap_or("").is_empty() {
        l.insert("kind".into(), json!("other"));
    }
    Value::Object(l)
}

const LINK_SCRIPT: &str = "dev=$(ip route get 1.1.1.1 2>/dev/null | awk '{ for (i = 1; i < NF; i++) if ($i == \"dev\") { print $(i + 1); exit } }'); echo \"iface=$dev\"; [ -z \"$dev\" ] && exit 0; if [ -d \"/sys/class/net/$dev/wireless\" ] || [ -e \"/sys/class/net/$dev/phy80211\" ]; then echo kind=wifi; iw dev \"$dev\" link 2>/dev/null; nmcli -t -f ACTIVE,SECURITY,SSID,SIGNAL,FREQ,RATE dev wifi list --rescan no 2>/dev/null | sed -n 's/^yes:/nm=/p' | head -n 1; elif [ -e \"/sys/class/net/$dev/device\" ]; then echo kind=wired; echo \"speed=$(cat /sys/class/net/$dev/speed 2>/dev/null)\"; else echo kind=other; fi";
const PROBE_SCRIPT: &str = "command -v speedtest >/dev/null && speedtest --version 2>/dev/null | grep -qi ookla && echo ookla; command -v speedtest-cli >/dev/null && echo py";

enum M {
    Line(String),
    Exit(i32, bool),
    Probe(String),
    Link(String),
}

pub struct SpeedTest {
    stage: String,
    ping: f64,
    jitter: f64,
    download: f64,
    upload: f64,
    loaded_down: f64,
    loaded_up: f64,
    live: f64,
    progress: f64,
    used_bytes: f64,
    server: Value,
    client: Value,
    result_url: String,
    error: String,
    finished_at: f64,
    has_ookla: bool,
    has_ookla_py: bool,
    history: Vec<Value>,
    link: Value,
    pings: Vec<f64>,
    loaded: (Vec<f64>, Vec<f64>),
    phase_start: f64,
    backend_run: String,
    watch: Option<Watch>,
    probing: bool,
    linking: bool,
}

impl Default for SpeedTest {
    fn default() -> Self {
        Self {
            stage: "idle".into(),
            ping: -1.0,
            jitter: -1.0,
            download: -1.0,
            upload: -1.0,
            loaded_down: -1.0,
            loaded_up: -1.0,
            live: 0.0,
            progress: 0.0,
            used_bytes: 0.0,
            server: json!({}),
            client: json!({}),
            result_url: String::new(),
            error: String::new(),
            finished_at: 0.0,
            has_ookla: false,
            has_ookla_py: false,
            history: vec![],
            link: json!({}),
            pings: vec![],
            loaded: (vec![], vec![]),
            phase_start: 0.0,
            backend_run: String::new(),
            watch: None,
            probing: false,
            linking: false,
        }
    }
}

fn join_nonempty(a: &Value, b: &Value, sep: &str) -> String {
    [a.as_str().unwrap_or(""), b.as_str().unwrap_or("")].iter().filter(|x| !x.is_empty()).cloned().collect::<Vec<_>>().join(sep)
}

impl SpeedTest {
    fn running(&self) -> bool {
        matches!(self.stage.as_str(), "latency" | "download" | "upload")
    }
    fn ookla_ready(&self) -> bool {
        self.has_ookla || self.has_ookla_py
    }
    fn backend(h: &Host) -> &'static str {
        if h.cfg()["network"]["speedTest"] == "ookla" { "ookla" } else { "cloudflare" }
    }

    fn publish(&self, h: &Host) {
        crate::bus::put(
            NAME,
            json!({
                "stage": self.stage, "running": self.running(), "backend": Self::backend(h),
                "ping": self.ping, "jitter": self.jitter, "download": self.download, "upload": self.upload,
                "loadedDown": self.loaded_down, "loadedUp": self.loaded_up, "live": self.live, "progress": self.progress,
                "usedBytes": self.used_bytes, "server": self.server, "client": self.client, "resultUrl": self.result_url,
                "error": self.error, "finishedAt": self.finished_at, "hasOokla": self.has_ookla, "hasOoklaPy": self.has_ookla_py,
                "ooklaReady": self.ookla_ready(), "ooklaInstall": OOKLA_INSTALL, "history": self.history,
                "last": self.history.first().cloned().unwrap_or(Value::Null), "link": self.link,
            }),
        );
    }

    fn probe(&mut self, h: &mut Host) {
        if self.probing {
            return;
        }
        self.probing = true;
        let send = h.sender(NAME);
        capture_then(sh_cmd(PROBE_SCRIPT, &[]), None, move |_, t| send(Box::new(M::Probe(t))));
    }

    fn read_link(&mut self, h: &mut Host) {
        if self.linking {
            return;
        }
        self.linking = true;
        let send = h.sender(NAME);
        capture_then(sh_cmd(LINK_SCRIPT, &[]), None, move |_, t| send(Box::new(M::Link(t))));
    }

    fn begin(&mut self, h: &mut Host) {
        if self.running() {
            return;
        }
        let backend = Self::backend(h);
        if backend == "ookla" && !self.ookla_ready() {
            self.probe(h);
            self.error = format!("Ookla's speedtest isn't installed. {OOKLA_INSTALL}");
            self.stage = "error".into();
            return;
        }
        self.pings.clear();
        self.loaded = (vec![], vec![]);
        self.ping = -1.0;
        self.jitter = -1.0;
        self.download = -1.0;
        self.upload = -1.0;
        self.loaded_down = -1.0;
        self.loaded_up = -1.0;
        self.live = 0.0;
        self.progress = 0.0;
        self.used_bytes = 0.0;
        self.server = json!({});
        self.client = json!({});
        self.result_url.clear();
        self.error.clear();
        self.stage = "latency".into();
        self.phase_start = now_ms();
        self.backend_run = backend.into();
        let which = if backend == "ookla" { if self.has_ookla { "ookla" } else { "ookla-py" } } else { "cloudflare" };
        let (a, b) = (h.sender(NAME), h.sender(NAME));
        let script = crate::util::script("speedtest.sh");
        self.watch = Some(Watch::start(&[&script, which], None, move |l| a(Box::new(M::Line(l))), move |c, cancelled| b(Box::new(M::Exit(c, cancelled)))));
        self.read_link(h);
    }

    fn set_phase(&mut self, p: &str) {
        self.stage = p.into();
        self.phase_start = now_ms();
        self.progress = 0.0;
        self.live = 0.0;
    }

    fn on_line(&mut self, h: &mut Host, line: &str) {
        let (kind, rest) = line.split_once(' ').unwrap_or((line, ""));
        match kind {
            "phase" => self.set_phase(rest),
            "meta" => {
                if let Ok(m) = serde_json::from_str::<Value>(rest) {
                    let s = |v: &Value| v.as_str().unwrap_or("").to_string();
                    let loc = [s(&m["city"]), s(&m["country"])].into_iter().filter(|x| !x.is_empty()).collect::<Vec<_>>().join(", ");
                    self.client = json!({"ip": s(&m["clientIp"]), "isp": s(&m["asOrganization"]), "location": loc});
                    let sloc = [s(&m["colo"]["city"]), s(&m["colo"]["cca2"])].into_iter().filter(|x| !x.is_empty()).collect::<Vec<_>>().join(", ");
                    self.server = json!({"name": format!("Cloudflare {}", s(&m["colo"]["iata"])), "location": sloc});
                }
            }
            "ping" => {
                self.pings.push(rest.trim().parse().unwrap_or(0.0));
                self.ping = median(&self.pings);
                let d: f64 = self.pings.windows(2).map(|w| (w[1] - w[0]).abs()).sum();
                self.jitter = if self.pings.len() > 1 { d / (self.pings.len() - 1) as f64 } else { -1.0 };
                self.progress = self.pings.len() as f64 / 16.0;
            }
            "live" => {
                self.live = rest.trim().parse().unwrap_or(0.0);
                self.progress = ((now_ms() - self.phase_start) / (PHASE_SECS * 1000.0)).min(1.0);
            }
            "loaded" => {
                let v = rest.trim().parse().unwrap_or(0.0);
                if self.stage == "upload" {
                    self.loaded.1.push(v);
                    self.loaded_up = median(&self.loaded.1);
                } else {
                    self.loaded.0.push(v);
                    self.loaded_down = median(&self.loaded.0);
                }
            }
            "done" => {
                let f: Vec<&str> = rest.split(' ').collect();
                let (bps, bytes) = (f.get(1).and_then(|x| x.parse().ok()).unwrap_or(0.0), f.get(2).and_then(|x| x.parse().ok()).unwrap_or(0.0));
                self.used_bytes += bytes;
                if f.first() == Some(&"download") {
                    self.download = bps;
                } else {
                    self.upload = bps;
                    self.finish(h);
                }
            }
            "ookla" => self.on_ookla(h, rest),
            "error" => {
                let (code, msg) = rest.split_once(' ').unwrap_or((rest, ""));
                self.error = match code {
                    "offline" => "You're offline: connect to Wi-Fi or a network, then test again.".into(),
                    "curl" => "The speed test needs curl. Install it with “sudo pacman -S curl”.".into(),
                    "timeout" => format!("{msg}. Your connection may be very slow or blocked; try again, or switch the server in Settings → Network."),
                    "ookla" => {
                        if msg.contains("installed") {
                            format!("Ookla's speedtest isn't installed. {OOKLA_INSTALL}")
                        } else {
                            format!("Ookla's speedtest failed: {msg}. Try again, or switch to Cloudflare.")
                        }
                    }
                    _ => msg.to_string(),
                };
                self.stage = "error".into();
            }
            _ => {}
        }
    }

    /// Ookla's official CLI: JSON lines (bandwidth in bytes/s); speedtest-cli:
    /// one JSON result (bits/s)
    fn on_ookla(&mut self, h: &mut Host, json_text: &str) {
        let Ok(j) = serde_json::from_str::<Value>(json_text) else { return };
        let f = |v: &Value, d: f64| v.as_f64().unwrap_or(d);
        if j["type"] == "py" {
            let d = &j["data"];
            self.client = json!({"ip": d["client"]["ip"].as_str().unwrap_or(""), "isp": d["client"]["isp"].as_str().unwrap_or(""), "location": d["client"]["country"].as_str().unwrap_or("")});
            self.server = json!({"name": d["server"]["sponsor"].as_str().unwrap_or(""), "location": join_nonempty(&d["server"]["name"], &d["server"]["country"], ", ")});
            self.ping = f(&d["ping"], -1.0);
            self.download = f(&d["download"], -1.0);
            self.upload = f(&d["upload"], -1.0);
            self.used_bytes = f(&d["bytes_received"], 0.0) + f(&d["bytes_sent"], 0.0);
            self.result_url = d["share"].as_str().unwrap_or("").into();
            self.finish(h);
            return;
        }
        let set_server = |me: &mut SpeedTest| {
            if !j["server"].is_null() {
                me.server = json!({"name": j["server"]["name"].as_str().unwrap_or(""), "location": join_nonempty(&j["server"]["location"], &j["server"]["country"], ", ")});
            }
            if !j["isp"].is_null() || !j["interface"].is_null() {
                me.client = json!({"ip": j["interface"]["externalIp"].as_str().unwrap_or(""), "isp": j["isp"].as_str().unwrap_or(""), "location": ""});
            }
        };
        match j["type"].as_str().unwrap_or("") {
            "testStart" => set_server(self),
            "ping" => {
                if self.stage != "latency" {
                    self.set_phase("latency");
                }
                self.ping = f(&j["ping"]["latency"], self.ping);
                self.jitter = f(&j["ping"]["jitter"], self.jitter);
                self.progress = f(&j["ping"]["progress"], self.progress);
            }
            t @ ("download" | "upload") => {
                if self.stage != t {
                    self.set_phase(t);
                }
                let p = &j[t];
                self.live = f(&p["bandwidth"], 0.0) * 8.0;
                self.progress = f(&p["progress"], self.progress);
                let lat = f(&p["latency"]["iqm"], -1.0);
                if t == "download" {
                    self.loaded_down = lat;
                } else {
                    self.loaded_up = lat;
                }
            }
            "result" => {
                set_server(self);
                self.ping = f(&j["ping"]["latency"], -1.0);
                self.jitter = f(&j["ping"]["jitter"], -1.0);
                self.download = f(&j["download"]["bandwidth"], -0.125) * 8.0;
                self.upload = f(&j["upload"]["bandwidth"], -0.125) * 8.0;
                self.loaded_down = f(&j["download"]["latency"]["iqm"], -1.0);
                self.loaded_up = f(&j["upload"]["latency"]["iqm"], -1.0);
                self.used_bytes = f(&j["download"]["bytes"], 0.0) + f(&j["upload"]["bytes"], 0.0);
                self.result_url = j["result"]["url"].as_str().unwrap_or("").into();
                self.finish(h);
            }
            "log" if j["level"] == "error" => {
                self.error = format!("Ookla's speedtest failed: {}. Try again, or switch to Cloudflare.", j["message"].as_str().unwrap_or("unknown error"));
                self.stage = "error".into();
            }
            _ => {}
        }
    }

    fn finish(&mut self, h: &Host) {
        self.live = 0.0;
        self.progress = 1.0;
        self.finished_at = now_ms();
        self.stage = "done".into();
        self.save(h);
    }

    fn save(&mut self, _h: &Host) {
        let l = &self.link;
        let network = match l["kind"].as_str() {
            Some("wifi") => l["ssid"].as_str().unwrap_or("").to_string(),
            Some("wired") => "Ethernet".into(),
            _ => String::new(),
        };
        let entry = json!({
            "at": now_ms(), "backend": self.backend_run, "download": self.download, "upload": self.upload, "ping": self.ping,
            "jitter": self.jitter, "loadedDown": self.loaded_down, "loadedUp": self.loaded_up,
            "server": join_nonempty(&self.server["name"], &self.server["location"], " · "),
            "isp": self.client["isp"].as_str().unwrap_or(""), "ip": self.client["ip"].as_str().unwrap_or(""), "network": network,
        });
        self.history.insert(0, entry);
        self.history.truncate(10);
        self.store();
    }

    fn store(&self) {
        let _ = crate::util::write_atomic(&state_dir().join("speedtest.json"), &serde_json::to_string_pretty(&json!({"history": self.history})).unwrap_or_default());
    }
}

impl Service for SpeedTest {
    fn name(&self) -> &'static str {
        NAME
    }
    fn calls(&self) -> &'static [&'static str] {
        &["speedtest"]
    }
    fn start(&mut self, h: &mut Host) {
        if let Some(a) = crate::util::read_json(&state_dir().join("speedtest.json"))["history"].as_array() {
            self.history = a.clone();
        }
        self.publish(h);
    }
    fn config_changed(&mut self, h: &mut Host) {
        self.publish(h);
    }
    fn call(&mut self, h: &mut Host, call: &str, _args: &Value) {
        match call.strip_prefix("speedtest.").unwrap_or("") {
            "start" => self.begin(h),
            "cancel" => {
                if self.running() {
                    if let Some(w) = &self.watch {
                        w.stop();
                    }
                }
            }
            "probe" => self.probe(h),
            "readLink" => self.read_link(h),
            "clearHistory" => {
                self.history.clear();
                self.store();
            }
            _ => {}
        }
        self.publish(h);
    }
    fn message(&mut self, h: &mut Host, msg: Box<dyn Any + Send>) {
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Line(l) => self.on_line(h, &l),
            M::Exit(_, cancelled) => {
                self.watch = None;
                if cancelled {
                    self.stage = if self.download >= 0.0 { "done" } else { "idle" }.into();
                    if self.download >= 0.0 {
                        self.save(h);
                    }
                } else if self.stage != "error" && self.running() {
                    // ended without a result
                    self.error = "The speed test stopped before it finished. Check your connection and try again.".into();
                    self.stage = "error".into();
                }
            }
            M::Probe(t) => {
                self.probing = false;
                self.has_ookla = t.contains("ookla");
                self.has_ookla_py = t.contains("py");
            }
            M::Link(t) => {
                self.linking = false;
                self.link = parse_link(&t);
            }
        }
        self.publish(h);
    }
    fn quit(&mut self, _h: &mut Host) {
        if let Some(w) = &self.watch {
            w.stop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings() {
        assert_eq!(format(-1.0), "—");
        assert_eq!(format(250e6), "250 Mbps");
        assert_eq!(format(12.34e6), "12.3 Mbps");
        assert_eq!(format(2.5e9), "2.50 Gbps");
        assert_eq!(format(800e3), "800 kbps");
        assert_eq!(format_ms(4.25), "4.2 ms");
        assert_eq!(format_ms(42.4), "42 ms");
        assert_eq!(format_bytes(250e6), "250 MB");
        assert_eq!(median(&[3.0, 1.0, 2.0]), 2.0);
        assert_eq!(median(&[1.0, 3.0]), 2.0);
        assert_eq!(median(&[]), -1.0);
        assert_eq!(signal_words(-60.0), "Good");
    }

    #[test]
    fn verdicts() {
        let fast = json!({"download": 300e6, "upload": 50e6, "ping": 10.0, "loadedDown": 40.0, "loadedUp": 30.0});
        assert!(verdict(&fast).starts_with("Great"));
        let laggy = json!({"download": 50e6, "upload": 10e6, "ping": 30.0, "loadedDown": 400.0});
        assert!(verdict(&laggy).starts_with("Fast, but lag"));
        assert_eq!(verdict(&Value::Null), "");
        assert!(verdict(&json!({"download": 1e6, "upload": 1e6, "ping": 10.0})).starts_with("Slow"));
    }

    #[test]
    fn link_from_iw() {
        let t = "iface=wlan0\nkind=wifi\nConnected to aa:bb\n\tSSID: Home\n\tfreq: 5180.0\n\tsignal: -52 dBm\n\trx bitrate: 866.7 MBit/s VHT-MCS 9 80MHz short GI VHT-NSS 2\n\ttx bitrate: 650.0 MBit/s VHT-MCS 7 80MHz\nnm=WPA2:Home:80:5180 MHz:866 Mbit/s\n";
        let l = parse_link(t);
        assert_eq!((l["ssid"].as_str(), l["band"].as_str(), l["channel"].as_f64()), (Some("Home"), Some("5 GHz"), Some(36.0)));
        assert_eq!((l["signal"].as_f64(), l["width"].as_f64(), l["standard"].as_str(), l["security"].as_str()), (Some(-52.0), Some(80.0), Some("Wi-Fi 5"), Some("WPA2")));
        assert!(l["quality"].as_f64().unwrap() > 0.6);
        let w = parse_link("iface=eth0\nkind=wired\nspeed=1000\n");
        assert_eq!((w["kind"].as_str(), w["speed"].as_str(), w["security"].as_str()), (Some("wired"), Some("1000"), Some("")));
    }
}

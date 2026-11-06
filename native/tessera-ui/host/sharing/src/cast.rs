//! Chromecast and Google TV (shell/services/Cast.qml; assets/scripts/cast.py:
//! Google Cast spoken directly). What's playing, or a file, goes to a TV or
//! speaker on the same network and is controlled from here: the island while
//! it plays, Control Center's Cast page to pick a device, the share sheet for
//! files, and "Cast to <TV>" in Spotlight.
//!
//! Devices come from Avahi (`avahi-browse -p -r -k _googlecast._tcp`, followed
//! while running: tessera-events' D-Bus browse is the QML's first choice, this
//! is its fallback, `avahiLine` below). One cast at a time.
//!   ipc: cast stop | toggle

use crate::localsend::js;
use crate::proc::{json_line, spawn, Backoff, Kept};
use crate::Sharing;
use serde_json::{json, Value};
use tessera_core::host::Host;

#[derive(Default)]
pub struct Cast {
    /// [{ id, name, model, host, port }]
    pub devices: Vec<Value>,
    /// "interface;protocol;avahi name" -> device, in the order heard
    pub found: Vec<(String, Value)>,
    pub browser: Kept,
    pub browse_backoff: Backoff,
    /// "" | connecting | loading | buffering | playing | paused | idle | ended | error
    pub state: String,
    pub device: Option<Value>,
    pub title: String,
    pub time: f64,
    pub duration: f64,
    pub volume: f64,
    pub error: String,
    /// A file goes to it from this computer (cast.py's file server, port
    /// 53320): the firewall lets it through meanwhile (ports.rs)
    pub serves_file: bool,
    pub session: Kept,
    pub first_command: Option<Value>,
    pub pending: Option<(Value, Value)>,
}

impl Cast {
    pub fn active(&self) -> bool {
        !self.state.is_empty() && self.state != "ended" && self.state != "error"
    }
}

impl Sharing {
    /// Not while avahi-daemon is off: it would only end, again and again
    /// (the QML also asks Needs; the restart back-off covers it here)
    fn cast_browsing(&self) -> bool {
        tessera_core::util::on_path("avahi-browse")
    }

    pub fn cast_sync(&mut self, h: &mut Host) {
        let want = self.cast_browsing();
        if want && !self.cast.browser.running && !self.sched.has("cast_browse") {
            let gen = self.next_gen();
            let cmd = crate::proc::s(&["avahi-browse", "-p", "-r", "-k", "_googlecast._tcp"]);
            match spawn(h.sender("Sharing"), "cast-browse", gen, &cmd, &[], true, true) {
                Some(k) => {
                    self.cast.browser = k;
                    self.cast.browse_backoff.began();
                }
                None => {
                    self.cast.browser = Kept { gen, ..Default::default() };
                    self.cast_browse_ended(gen);
                }
            }
        } else if !want && self.cast.browser.running {
            self.cast.browser.stop();
        }
    }
    pub fn cast_browse_ended(&mut self, gen: u64) {
        if gen != self.cast.browser.gen {
            return;
        }
        self.cast.browser.ended();
        self.cast_forget();
        if self.cast_browsing() {
            let ms = self.cast.browse_backoff.ended(10_000, 300_000);
            self.sched.set("cast_browse", ms);
        }
    }
    pub fn cast_forget(&mut self) {
        self.cast.found.clear();
        self.cast.devices.clear();
    }

    pub fn cast_avahi_line(&mut self, gen: u64, line: &str) {
        if gen != self.cast.browser.gen {
            return;
        }
        let f: Vec<&str> = line.split(';').collect();
        if f.len() < 4 {
            return;
        }
        // (a device is heard once per interface and IP version: it's gone
        // when the last of those goes)
        let key = format!("{};{};{}", f[1], f[2], f[3]);
        if f[0] == "-" {
            self.cast_seen(&key, None);
        } else if f[0] == "=" && f.len() >= 10 {
            let txt = parse_txt(f[9]);
            let get = |k: &str| txt.iter().find(|(a, _)| a == k).map(|(_, v)| v.clone()).unwrap_or_default();
            let or = |a: String, b: &str| if a.is_empty() { b.to_string() } else { a };
            let port = f[8].parse::<i64>().ok().filter(|p| *p != 0).unwrap_or(8009);
            self.cast_seen(&key, Some(json!({"id": or(get("id"), f[3]), "name": or(get("fn"), f[3]), "model": get("md"), "host": f[7], "port": port})));
        }
    }

    /// A device heard (or, None, gone) on one interface and IP version
    fn cast_seen(&mut self, key: &str, dev: Option<Value>) {
        match dev {
            Some(d) => match self.cast.found.iter_mut().find(|(k, _)| k == key) {
                Some(e) => e.1 = d,
                None => self.cast.found.push((key.to_string(), d)),
            },
            None => self.cast.found.retain(|(k, _)| k != key),
        }
        // One per device, IPv4 over IPv6
        let mut by_id: Vec<Value> = Vec::new();
        for (_, d) in &self.cast.found {
            match by_id.iter().position(|x| x["id"] == d["id"]) {
                None => by_id.push(d.clone()),
                Some(i) => {
                    if js(&by_id[i]["host"]).contains(':') && !js(&d["host"]).contains(':') {
                        by_id[i] = d.clone();
                    }
                }
            }
        }
        by_id.sort_by_key(|d| js(&d["name"]).to_lowercase());
        self.cast.devices = by_id;
    }

    pub fn cast_icon(dev: &Value) -> &'static str {
        let m = js(&dev["model"]).to_lowercase();
        if ["nest", "home", "speaker", "audio"].iter().any(|w| m.contains(w)) {
            "speaker-hifi"
        } else {
            "television-simple"
        }
    }

    // ── What can be cast right now ──────────────────────────────────────
    // What's playing, when its app says where it comes from: a file, or a
    // web page (a video site: through yt-dlp)
    fn cast_player(&self, h: &Host) -> Option<tessera_core::services::mpris::Player> {
        h.st.shell.get::<tessera_core::services::mpris::Players>().and_then(|p| p.chosen().cloned())
    }
    pub fn playing_url(&self, h: &Host) -> String {
        self.cast_player(h).map(|p| p.track.url).unwrap_or_default()
    }
    pub fn can_cast_playing(&self, h: &Host) -> bool {
        let u = self.playing_url(h);
        u.starts_with("file://") || u.starts_with("http://") || u.starts_with("https://")
    }

    pub fn cast_playing(&mut self, h: &mut Host, dev: &Value) {
        if !self.can_cast_playing(h) {
            self.cast_fail(h, "Can't cast what's playing", "The app playing it doesn't say what it is. Cast a file from Files instead.");
            return;
        }
        let p = self.cast_player(h);
        let url = self.playing_url(h);
        // (the player's position isn't in the MPRIS reading: it starts from the beginning)
        let mut cmd = json!({"cmd": "cast", "host": dev["host"], "port": dev["port"], "title": p.as_ref().map(|p| p.track.title.clone()).unwrap_or_default(), "start": 0});
        if let Some(path) = url.strip_prefix("file://") {
            cmd["file"] = json!(tessera_core::services::mpris::percent_decode(path));
        } else {
            cmd["url"] = json!(url);
        }
        self.cast_begin(h, dev.clone(), cmd);
        if let Some(p) = p {
            if p.playing {
                tessera_core::services::mpris::control(&p.bus, "Pause"); // (it plays on the TV now)
            }
        }
    }
    pub fn cast_file(&mut self, h: &mut Host, dev: &Value, path: &str) {
        let title = crate::base(path);
        let title = title.rsplit_once('.').map(|(a, _)| a.to_string()).unwrap_or(title);
        self.cast_begin(h, dev.clone(), json!({"cmd": "cast", "host": dev["host"], "port": dev["port"], "file": path, "title": title}));
    }
    fn cast_begin(&mut self, h: &mut Host, dev: Value, cmd: Value) {
        if self.cast.session.running {
            self.cast_send(json!({"cmd": "stop"}));
            self.cast.pending = Some((dev, cmd));
            return;
        }
        self.cast.serves_file = cmd.get("file").is_some_and(|f| !f.is_null());
        self.cast.title = js(&cmd["title"]);
        self.cast.time = cmd["start"].as_f64().unwrap_or(0.0);
        self.cast.duration = 0.0;
        self.cast.error.clear();
        self.cast.state = "connecting".into();
        self.cast.device = Some(dev);
        let gen = self.next_gen();
        let command = vec!["python3".to_string(), "-I".into(), self.script("cast.py"), "session".into()];
        match spawn(h.sender("Sharing"), "cast", gen, &command, &[], true, true) {
            Some(mut k) => {
                // (written once it's running)
                k.write(&format!("{cmd}\n"));
                self.cast.session = k;
            }
            None => {
                self.cast.session = Kept { gen, ..Default::default() };
                self.cast_exit(h, gen);
            }
        }
    }
    fn cast_send(&mut self, obj: Value) {
        if self.cast.session.running {
            self.cast.session.write(&format!("{obj}\n"));
        }
    }
    pub fn cast_toggle(&mut self) {
        let playing = self.cast.state == "playing" || self.cast.state == "buffering";
        self.cast_send(json!({"cmd": if playing {"pause"} else {"play"}}));
    }
    pub fn cast_seek(&mut self, t: f64) {
        self.cast_send(json!({"cmd": "seek", "t": t.max(0.0)}));
    }
    pub fn cast_volume(&mut self, v: f64) {
        self.cast_send(json!({"cmd": "volume", "v": v}));
    }
    pub fn cast_stop(&mut self) {
        self.cast_send(json!({"cmd": "stop"}));
    }

    pub fn cast_line(&mut self, h: &mut Host, gen: u64, line: &str) {
        if gen != self.cast.session.gen {
            return;
        }
        let Some(s) = json_line(line) else { return };
        let was = self.cast.state.clone();
        if let Some(st) = s["state"].as_str() {
            self.cast.state = st.into();
        }
        if let Some(t) = s["time"].as_f64() {
            self.cast.time = t;
        }
        if let Some(d) = s["duration"].as_f64() {
            self.cast.duration = d;
        }
        if let Some(v) = s["volume"].as_f64() {
            self.cast.volume = v;
        }
        if s["state"] == "error" {
            let name = self.cast.device.as_ref().map(|d| js(&d["name"])).unwrap_or_else(|| "the TV".into());
            let lost = matches!(was.as_str(), "playing" | "paused" | "buffering");
            let why = s["error"].as_str().filter(|e| !e.is_empty()).unwrap_or("Try again.").to_string();
            self.cast_fail(h, &format!("{}{name}", if lost { "Lost the connection to " } else { "Couldn't cast to " }), &why);
        }
    }
    pub fn cast_exit(&mut self, h: &mut Host, gen: u64) {
        if gen != self.cast.session.gen {
            return;
        }
        self.cast.session.ended();
        self.cast.serves_file = false;
        if self.cast.state != "error" {
            self.cast.state.clear();
        }
        if let Some((dev, cmd)) = self.cast.pending.take() {
            self.cast_begin(h, dev, cmd);
        }
    }
    fn cast_fail(&mut self, h: &mut Host, title: &str, why: &str) {
        self.cast.error = why.to_string();
        self.island(h, json!({"kind": "custom", "icon": "screencast", "title": title, "subtitle": why, "color": crate::DANGER}), 7000);
    }

    /// The cast, as Control Center's Cast page and the island read it
    pub fn cast_state(&self, h: &Host) -> Value {
        json!({"active": self.cast.active(), "state": self.cast.state, "title": self.cast.title, "time": self.cast.time.floor(), "duration": self.cast.duration.floor(),
            "volume": self.cast.volume, "error": self.cast.error, "device": self.cast.device, "devices": self.cast.devices, "canCastPlaying": self.can_cast_playing(h)})
    }
}

/// `"id=abc" "fn=Living room"` (avahi's TXT field): the pairs, in order
fn parse_txt(s: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'"' {
            if let Some(end) = s[i + 1..].find('"') {
                let item = &s[i + 1..i + 1 + end];
                if let Some((k, v)) = item.split_once('=') {
                    if !k.is_empty() {
                        out.push((k.to_string(), v.to_string()));
                    }
                }
                i += end + 2;
                continue;
            }
        }
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn txt_pairs_are_read_in_order() {
        let t = parse_txt("\"id=abc123\" \"md=Chromecast\" \"fn=Living room\"");
        assert_eq!(t, vec![("id".into(), "abc123".into()), ("md".into(), "Chromecast".into()), ("fn".into(), "Living room".into())]);
    }

    #[test]
    fn speakers_get_a_speaker_icon() {
        assert_eq!(Sharing::cast_icon(&json!({"model": "Google Nest Mini"})), "speaker-hifi");
        assert_eq!(Sharing::cast_icon(&json!({"model": "Chromecast"})), "television-simple");
    }
}

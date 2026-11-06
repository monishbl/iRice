//! Android's Quick Share (Nearby Share), built in (shell/services/
//! QuickShare.qml): Tessera speaks the protocol itself (assets/scripts/
//! quickshare, see its nearby/__init__.py), so phones send to this computer
//! and it sends to them from their own Quick Share sheet, with nothing else
//! installed. You're asked (the share card) before anything is saved, with
//! the PIN the phone shows too, and transfers show as live activities.
//! Visibility (Settings -> Sharing): Off | Everyone for 10 minutes | Everyone.
//! On the same network it goes over that; without one, phones find it over
//! Bluetooth LE and the transfer moves to a Wi-Fi link (fastest first, a
//! link gets a second try before the next; nearby/links.py). Alerts only for
//! a request, a finished transfer and a failure; how the link is going shows
//! quietly in the transfer's live activity. Bluetooth is never turned on from
//! here except by the user's own "Turn on Bluetooth" (`share.bluetoothOn`).
//! The helper only runs while visible, while the share sheet looks for
//! devices, or while a transfer is going.

use crate::localsend::{js, Active};
use crate::proc::{json_line, spawn, Backoff, Kept};
use crate::{act, base, Job, Sharing, GREEN, ORANGE};
use serde_json::{json, Value};
use std::collections::HashMap;
use tessera_core::host::Host;

#[derive(Default)]
pub struct QuickShare {
    pub everyone_until: i64,
    /// the share sheet is open: find devices
    pub looking: bool,
    /// (name, type, id)
    pub devices: Vec<(String, String, String)>,
    pub active: Active,
    pub send_pending: bool,
    pub error: String,
    /// the helper could reach BlueZ (python-gobject, bluetoothd)
    pub bluetooth_engine: bool,
    /// session -> "Wi-Fi" | "Wi-Fi Direct" | "Bluetooth": the link it's on now
    pub transports: HashMap<String, String>,
    /// session -> { pin, note, slow }: shown with the transfer, not as alerts
    pub links: HashMap<String, Value>,
    pub sending_name: String,
    pub helper: Kept,
    pub retries: u32,
    pub backoff: Backoff,
    pub trusted: bool,
    pub trust_busy: bool,
    pub trust_error: String,
    pub last_visible: bool,
    pub last_name: String,
    pub last_folder: String,
    pub last_looking: bool,
    pub last_mode: String,
    pub last_hotspot: bool,
}

impl Sharing {
    pub fn quick_available(&self) -> bool {
        tessera_core::util::on_path("python3")
    }
    pub fn quick_visible(&self) -> bool {
        let m = self.cfg_str("sharing.quickShare", "off");
        m == "everyone" || (m == "10min" && self.quick.everyone_until > tessera_core::util::now_ms())
    }
    /// One name for this computer, LocalSend's too (Settings -> Sharing -> Your name)
    pub fn my_name(&self) -> String {
        let n = self.cfg_str("sharing.name", "");
        if !n.is_empty() {
            return n;
        }
        format!("{}'s computer", std::env::var("USER").unwrap_or_else(|_| "Tessera".into()))
    }
    pub fn quick_folder(&self) -> String {
        self.cfg_str("sharing.quickShareFolder", "")
    }
    pub fn quick_needed(&self) -> bool {
        self.quick_available() && (self.quick_visible() || self.quick.looking || self.quick.send_pending || !self.quick.active.is_empty())
    }
    pub fn bluetooth_available(&self, h: &Host) -> bool {
        h.st.shell.get::<tessera_core::services::bluez::Bluetooth>().is_some_and(|b| b.present)
    }
    pub fn bluetooth_on(&self, h: &Host) -> bool {
        h.st.shell.get::<tessera_core::services::bluez::Bluetooth>().is_some_and(|b| b.present && b.powered)
    }
    pub fn bluetooth_needed(&self, h: &Host) -> bool {
        self.bluetooth_available(h) && !self.bluetooth_on(h) && (self.cfg_str("sharing.quickShare", "off") != "off" || self.quick.looking)
    }
    /// The user's "Turn on Bluetooth" (Powered = true on the adapter), off the loop
    pub fn bluetooth_turn_on(&self) {
        std::thread::spawn(|| {
            let Ok(c) = zbus::blocking::Connection::system() else { return };
            let _ = c.call_method(
                Some("org.bluez"),
                "/org/bluez/hci0",
                Some("org.freedesktop.DBus.Properties"),
                "Set",
                &("org.bluez.Adapter1", "Powered", zbus::zvariant::Value::from(true)),
            );
        });
    }

    pub fn quick_tell(&mut self, obj: Value) {
        if self.quick.helper.running {
            self.quick.helper.write(&format!("{obj}\n"));
        }
    }
    pub fn quick_configure(&mut self) {
        let folder = self.quick_folder();
        let m = json!({"cmd": "config", "visible": self.quick_visible(), "name": self.my_name(), "folder": if folder.is_empty() { "~/Downloads".to_string() } else { folder }, "pause_wifi": self.cfg_bool("sharing.quickShareHotspot", true)});
        self.quick_tell(m);
    }

    fn transport_of(&self, session: &str) -> String {
        self.quick.transports.get(session).cloned().unwrap_or_default()
    }
    fn set_link(&mut self, session: &str, change: Value) {
        let mut cur = self.quick.links.get(session).cloned().unwrap_or(json!({}));
        if let (Some(c), Some(n)) = (cur.as_object_mut(), change.as_object()) {
            for (k, v) in n {
                c.insert(k.clone(), v.clone());
            }
        }
        self.quick.links.insert(session.to_string(), cur.clone());
        if let Some(a) = self.quick.active.get(session).cloned() {
            let mut a = a;
            if let (Some(am), Some(cm)) = (a.as_object_mut(), cur.as_object()) {
                for (k, v) in cm {
                    am.insert(k.clone(), v.clone());
                }
            }
            self.quick.active.set(session, a);
        }
    }
    /// A short line for it: "on Bluetooth (slow)", "Wi-Fi paused"...
    fn note_of(&self, ev: &Value) -> String {
        if ev["slow"].as_bool().unwrap_or(false) {
            return "on Bluetooth (slow)".into();
        }
        if ev["auth"].as_bool().unwrap_or(false) {
            // (no prompt when it's trusted)
            return if self.quick.trusted { String::new() } else { "password for a faster link".into() };
        }
        if ev["message"].as_str().unwrap_or("").contains("pauses") {
            "Wi-Fi paused for it".into()
        } else {
            String::new()
        }
    }

    pub fn quick_send(&mut self, device_id: &str, paths: &[String]) {
        let first = paths.first().map(|p| base(p)).unwrap_or_default();
        self.quick.sending_name = if paths.len() > 1 { format!("{first} and {} more", paths.len() - 1) } else { first };
        self.quick.send_pending = true;
        self.quick_tell(json!({"cmd": "send", "to": device_id, "files": paths}));
    }
    pub fn quick_cancel(&mut self, session: &str) {
        self.quick_tell(json!({"cmd": "cancel", "session": session}));
        self.quick_finish(session);
    }
    fn quick_track(&mut self, ev: &Value) {
        let session = js(&ev["session"]);
        let prev = self.quick.active.get(&session).cloned();
        let now = tessera_core::util::now_ms();
        let incoming = ev["dir"] == "in";
        let done = ev["done"].as_f64().unwrap_or(0.0);
        let speed = match &prev {
            Some(p) if now > p["at"].as_i64().unwrap_or(0) => ((done - p["done"].as_f64().unwrap_or(0.0)) * 1000.0 / (now - p["at"].as_i64().unwrap_or(0)) as f64).max(0.0),
            _ => 0.0,
        };
        let name = prev.as_ref().and_then(|p| p["name"].as_str().map(str::to_string)).unwrap_or_else(|| if incoming { js(&ev["name"]) } else { self.quick.sending_name.clone() });
        let peer = if incoming { prev.as_ref().and_then(|p| p["peer"].as_str().map(str::to_string)).unwrap_or_else(|| js(&ev["name"])) } else { js(&ev["name"]) };
        let link = self.quick.links.get(&session).cloned().unwrap_or(json!({}));
        let item = json!({"id": session, "kind": if incoming {"receive"} else {"send"}, "via": "quickshare", "name": name, "peer": peer,
            "done": done, "total": ev["total"].as_f64().unwrap_or(0.0), "speed": speed, "at": now, "waiting": ev["waiting"].as_bool().unwrap_or(false),
            "transport": self.transport_of(&session), "pin": link.get("pin").cloned().unwrap_or(json!("")), "note": link.get("note").cloned().unwrap_or(json!("")), "slow": link.get("slow").and_then(Value::as_bool).unwrap_or(false)});
        self.quick.active.set(&session, item);
    }
    pub fn quick_finish(&mut self, session: &str) {
        self.quick.active.remove(session);
        self.quick.transports.remove(session);
        self.quick.links.remove(session);
    }

    /// "photo.jpg", "3 photos", "a link and 2 files"...
    fn describe(ev: &Value) -> String {
        let mut parts: Vec<String> = Vec::new();
        let files = ev["files"].as_array().cloned().unwrap_or_default();
        if files.len() == 1 {
            parts.push(js(&files[0]["name"]));
        } else if files.len() > 1 {
            let photos = files.iter().all(|f| js(&f["mime"]).starts_with("image/"));
            parts.push(format!("{}{}", files.len(), if photos { " photos" } else { " files" }));
        }
        for t in ev["texts"].as_array().cloned().unwrap_or_default() {
            parts.push(match t["kind"].as_str().unwrap_or("") {
                "url" => "a link",
                "phone" => "a phone number",
                "address" => "an address",
                _ => "some text",
            }.into());
        }
        for w in ev["wifi"].as_array().cloned().unwrap_or_default() {
            parts.push(format!("the Wi-Fi network {}", js(&w)));
        }
        match parts.len() {
            0 => "something".into(),
            1 => parts.remove(0),
            n => format!("{} and {}", parts[..n - 1].join(", "), parts[n - 1]),
        }
    }

    /// Started and stopped by hand, not bound to `needed` (a helper that
    /// exits by itself must be tried again: a few seconds later, then less
    /// often). Also the QML's change handlers (visible, name, folder, looking).
    pub fn quick_sync(&mut self, h: &mut Host) {
        let (visible, name, folder, looking) = (self.quick_visible(), self.my_name(), self.quick_folder(), self.quick.looking);
        if visible != self.quick.last_visible || name != self.quick.last_name || folder != self.quick.last_folder {
            self.quick.last_visible = visible;
            self.quick.last_name = name;
            self.quick.last_folder = folder;
            self.quick_configure();
        }
        if looking != self.quick.last_looking {
            self.quick.last_looking = looking;
            self.quick_tell(json!({"cmd": "discover", "on": looking}));
        }
        let needed = self.quick_needed();
        if needed && !self.quick.helper.running && !self.sched.has("qs_retry") {
            self.quick_start(h);
        } else if !needed && self.quick.helper.running {
            self.quick.helper.stop();
        }
        if self.quick.send_pending {
            self.sched.set_if_later("qs_pending", 180_000);
        } else {
            self.sched.cancel("qs_pending");
        }
        if !needed {
            self.sched.cancel("qs_retry");
        }
    }

    fn quick_start(&mut self, h: &mut Host) {
        let script = self.script("quickshare/qs-daemon.py");
        let gen = self.next_gen();
        // (The helper ends when its stdin closes: shell quit or restarted, or
        // no longer needed; the parent-death signal covers the shell being killed outright)
        let cmd = vec!["python3".to_string(), "-I".into(), script];
        match spawn(h.sender("Sharing"), "quickshare", gen, &cmd, &[], true, true) {
            Some(k) => {
                self.quick.helper = k;
                self.quick.backoff.began();
                self.quick.error.clear();
                self.quick_configure();
                if self.quick.looking {
                    self.quick_tell(json!({"cmd": "discover", "on": true}));
                }
            }
            None => self.quick_ended(gen, true),
        }
    }

    pub fn quick_exit(&mut self, gen: u64) {
        self.quick_ended(gen, false);
    }
    fn quick_ended(&mut self, gen: u64, never_started: bool) {
        if gen != self.quick.helper.gen && !never_started {
            return;
        }
        self.quick.helper.ended();
        self.quick.devices.clear();
        self.quick.active.clear();
        self.helper_ended("quickshare");
        self.quick.transports.clear();
        self.quick.bluetooth_engine = false;
        if self.quick_needed() {
            let ms = (3000u64 * 2u64.pow(self.quick.retries.min(10))).min(60000);
            self.sched.set("qs_retry", ms);
        }
    }
    /// The retry timer ran out
    pub fn quick_retry(&mut self, h: &mut Host) {
        self.quick.retries += 1;
        self.quick_sync(h);
    }

    pub fn quick_line(&mut self, h: &mut Host, gen: u64, line: &str) {
        if gen != self.quick.helper.gen {
            return;
        }
        let Some(ev) = json_line(line) else { return };
        match ev["ev"].as_str().unwrap_or("") {
            "status" => self.quick.retries = 0, // up and answering
            "devices" => {
                self.quick.devices = ev["devices"].as_array().map(|a| a.iter().map(|d| (js(&d["name"]), js(&d["type"]), js(&d["id"]))).collect()).unwrap_or_default();
            }
            "incoming" => {
                let what = Self::describe(&ev);
                let session = js(&ev["session"]);
                self.quick.active.set(&session, json!({"id": session, "kind": "receive", "via": "quickshare", "name": what, "peer": ev["from"], "done": 0, "total": ev["total"], "speed": 0, "at": tessera_core::util::now_ms(), "waiting": true}));
                // (the AirDrop-style card asks: the share surface's request card)
                self.add_request(json!({"id": format!("qs:{session}"), "via": "quickshare", "session": session, "from": ev["from"], "type": ev.get("type").filter(|t| !t.is_null()).cloned().unwrap_or(json!("phone")),
                    "what": what, "files": ev.get("files").cloned().unwrap_or(json!([])), "total": ev["total"].as_f64().unwrap_or(0.0), "pin": ev.get("pin").filter(|t| !t.is_null()).cloned().unwrap_or(json!(""))}));
            }
            // The PIN the phone shows too, in the live activity while it decides
            "connected" => self.set_link(&js(&ev["session"]), json!({"pin": js(&ev["pin"])})),
            "progress" => {
                self.quick_track(&ev);
                if ev["dir"] == "out" {
                    self.quick.send_pending = false;
                }
            }
            "finished" => {
                let session = js(&ev["session"]);
                self.quick_finish(&session);
                let paths: Vec<String> = ev["paths"].as_array().map(|a| a.iter().map(js).collect()).unwrap_or_default();
                if ev["dir"] == "in" {
                    // (the card says what came, when it's showing)
                    let id = format!("qs:{session}");
                    let shown = self.shown_request(&id) && !paths.is_empty();
                    if shown {
                        self.update_request(&id, json!({"state": "done", "paths": paths}));
                    } else {
                        self.drop_request(&id);
                    }
                    self.quick_received(h, &ev, shown);
                } else {
                    let to = js(&ev["to"]);
                    self.outcome(&to, "sent");
                    let sub = if paths.len() == 1 { base(&paths[0]) } else { format!("{} files", paths.len()) };
                    self.island(h, json!({"kind": "custom", "icon": "paper-plane-tilt", "title": format!("Sent to {to}"), "subtitle": sub, "color": GREEN}), 3000);
                }
            }
            "declined" => {
                let session = js(&ev["session"]);
                self.quick.send_pending = false;
                self.quick_finish(&session);
                let to = js(&ev["to"]);
                let reason = js(&ev["reason"]);
                if !to.is_empty() {
                    self.outcome(&to, if reason == "declined" { "declined" } else { "failed" });
                }
                let sub = if reason == "declined" {
                    "Nothing was sent".to_string()
                } else {
                    let mut c = reason.chars();
                    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
                };
                self.island(h, json!({"kind": "custom", "icon": "x-circle", "title": if reason == "declined" { format!("{to} declined") } else { format!("Not sent to {to}") }, "subtitle": sub, "color": crate::GREY}), 4000);
            }
            "ended" => {
                let session = js(&ev["session"]);
                self.quick_finish(&session);
                self.drop_request(&format!("qs:{session}"));
            }
            "failed" => {
                let session = js(&ev["session"]);
                let out = session.starts_with("out-");
                if out {
                    self.quick.send_pending = false;
                    if let Some(peer) = self.quick.active.get(&session).and_then(|a| a["peer"].as_str()).filter(|p| !p.is_empty()).map(str::to_string) {
                        self.outcome(&peer, "failed");
                    }
                }
                self.quick_finish(&session);
                let id = format!("qs:{session}");
                let msg = js(&ev["message"]);
                if !out && self.shown_request(&id) {
                    self.update_request(&id, json!({"state": "failed", "message": if msg == "Cancelled" { String::new() } else { msg }}));
                    return;
                }
                self.drop_request(&id);
                if msg != "Cancelled" {
                    self.island(h, json!({"kind": "custom", "icon": "warning-circle", "title": "Quick Share stopped", "subtitle": msg, "color": ORANGE}), 6000);
                }
            }
            "error" => self.quick.error = js(&ev["message"]),
            "bluetooth" => self.quick.bluetooth_engine = ev["available"].as_bool().unwrap_or(false) && ev["powered"].as_bool().unwrap_or(false),
            "transport" => {
                self.quick.transports.insert(js(&ev["session"]), js(&ev["name"]));
            }
            // how the link is going: quietly, with the transfer
            "link" => {
                let note = self.note_of(&ev);
                self.set_link(&js(&ev["session"]), json!({"note": note, "slow": ev["slow"].as_bool().unwrap_or(false), "message": js(&ev["message"])}));
            }
            _ => {}
        }
    }

    /// What arrived: files (Show), a link (Open), text (Copy), a Wi-Fi network
    /// (Copy password; joining is up to you)
    fn quick_received(&mut self, h: &mut Host, ev: &Value, files_shown: bool) {
        self.play(h, "received");
        let from = js(&ev["from"]);
        let paths: Vec<String> = if files_shown { vec![] } else { ev["paths"].as_array().map(|a| a.iter().map(js).collect()).unwrap_or_default() };
        if let Some(first) = paths.first().cloned() {
            let folder = self.quick_folder();
            let wher = if folder.is_empty() { "Downloads".to_string() } else { base(&folder) };
            let one = paths.len() == 1;
            self.island(
                h,
                json!({"kind": "custom", "icon": "check-circle", "title": format!("Received from {from}"),
                    "subtitle": if one { base(&first) } else { format!("{} files in {wher}", paths.len()) }, "color": GREEN,
                    "action": if one { "Open" } else { "Show in Folder" }, "onAction": if one { act("share.fileOpen", json!([first])) } else { act("share.fileReveal", json!([first])) },
                    "action2": if one { "Show in Folder" } else { "" }, "onAction2": act("share.fileReveal", json!([first]))}),
                8000,
            );
        }
        for t in ev["texts"].as_array().cloned().unwrap_or_default() {
            let text = js(&t["text"]);
            let link = t["kind"] == "url" && (text.to_lowercase().starts_with("http://") || text.to_lowercase().starts_with("https://"));
            let sub = if text.chars().count() > 80 { format!("{}\u{2026}", text.chars().take(79).collect::<String>()) } else { text.clone() };
            self.island(
                h,
                json!({"kind": "custom", "icon": if link {"link"} else {"text-aa"}, "title": format!("{} {from}", if link {"Link from"} else {"Text from"}), "subtitle": sub, "color": GREEN,
                    "action": if link {"Open"} else {"Copy"}, "onAction": if link { act("share.openLink", json!([text])) } else { act("share.copyText", json!([text])) },
                    "action2": if link {"Copy"} else {""}, "onAction2": act("share.copyText", json!([text]))}),
                10000,
            );
        }
        for w in ev["wifi"].as_array().cloned().unwrap_or_default() {
            self.island(
                h,
                json!({"kind": "custom", "icon": "wifi-high", "title": format!("Wi-Fi network from {from}"), "subtitle": js(&w["ssid"]), "color": GREEN,
                    "action": "Copy Password", "onAction": act("share.copyText", json!([js(&w["password"])]))}),
                10000,
            );
        }
    }

    // ── Fast transfers without asking (Settings -> Sharing) ─────────────
    // A polkit rule lets the Wi-Fi helper start without the password, for
    // you (Tessera's root helper: quickshare-trust). Turning it on or off
    // asks once. /etc/tessera/quickshare-trusted (readable by all) says it's on.
    pub fn quick_check_trust(&mut self) {
        // (/etc/irice: turned on before iRice became Tessera, until the root setup moves it)
        self.quick.trusted = std::path::Path::new("/etc/tessera/quickshare-trusted").exists() || std::path::Path::new("/etc/irice/quickshare-trusted").exists();
    }
    pub fn quick_set_trusted(&mut self, h: &mut Host, on: bool) {
        if self.quick.trust_busy {
            return;
        }
        self.quick.trust_error.clear();
        if !tessera_core::util::on_path("pkexec") {
            self.quick.trust_error = "Changing this needs polkit (pkexec), which isn't installed.".into();
            return;
        }
        self.quick.trust_busy = true;
        let cmd = Self::root_command(&["quickshare-trust", if on { "on" } else { "off" }]);
        self.run(h, Job::Trust, &cmd, None);
    }
    pub fn quick_trust_done(&mut self, stderr: &str, code: i32) {
        self.quick.trust_busy = false;
        if code != 0 && code != 126 {
            let said = stderr.trim().trim_start_matches("tessera-root: ").trim_start_matches("irice-root: ").trim().to_string();
            self.quick.trust_error = if code == 127 {
                "Not allowed (an administrator's password is needed).".into()
            } else if said.is_empty() {
                "That didn't change. Update Tessera, then try again.".into()
            } else {
                said
            };
        }
        self.quick_check_trust();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_arrived_is_said_in_words() {
        let ev = json!({"files": [{"name": "a.jpg", "mime": "image/jpeg"}, {"name": "b.jpg", "mime": "image/jpeg"}], "texts": [{"kind": "url"}]});
        assert_eq!(Sharing::describe(&ev), "2 photos and a link");
        assert_eq!(Sharing::describe(&json!({})), "something");
        assert_eq!(Sharing::describe(&json!({"files": [{"name": "x.pdf", "mime": "application/pdf"}]})), "x.pdf");
        assert_eq!(Sharing::describe(&json!({"files": [{"name": "x", "mime": ""}, {"name": "y", "mime": ""}], "texts": [{"kind": "url"}, {"kind": "text"}]})), "2 files, a link and some text");
    }
}

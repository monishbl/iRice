//! Sharing with nearby devices through the LocalSend protocol (phones and
//! computers running LocalSend, on the same network): shell/services/
//! LocalSend.qml. The work is done by assets/scripts/localsend.py; this
//! keeps the list of nearby devices, asks (the share card) before accepting
//! files, and shows transfers as a live activity.
//! Receiving follows the one visibility (visibility.rs): No one | Everyone
//! for 10 min | Everyone, Quick Share's and this together.
//! The helper only runs while receiving, or while you're sharing something.

use crate::proc::{json_line, spawn, Backoff, Kept};
use crate::{act, base, Sharing, GREEN, GREY, ORANGE};
use serde_json::{json, Value};
use tessera_core::host::Host;

/// session -> transfer item, in the order they came (Object.values)
#[derive(Default, Clone)]
pub struct Active(pub Vec<(String, Value)>);

impl Active {
    pub fn get(&self, session: &str) -> Option<&Value> {
        self.0.iter().find(|(k, _)| k == session).map(|(_, v)| v)
    }
    pub fn set(&mut self, session: &str, v: Value) {
        match self.0.iter_mut().find(|(k, _)| k == session) {
            Some(e) => e.1 = v,
            None => self.0.push((session.to_string(), v)),
        }
    }
    pub fn remove(&mut self, session: &str) {
        self.0.retain(|(k, _)| k != session);
    }
    pub fn clear(&mut self) {
        self.0.clear();
    }
    pub fn values(&self) -> Vec<Value> {
        self.0.iter().map(|(_, v)| v.clone()).collect()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[derive(Default)]
pub struct LocalSend {
    /// the share sheet is open or a send is running
    pub sharing: bool,
    /// (alias, type, id)
    pub peers: Vec<(String, String, String)>,
    pub active: Active,
    /// A send was asked for and hasn't reported back yet (the sheet may be gone)
    pub send_pending: bool,
    /// sends asked for before the helper ran or found the peer: (fingerprint,
    /// paths), told to it once its peers include it
    pub queued: Vec<(String, Vec<String>)>,
    /// what the next outgoing transfer is called
    pub sending_name: String,
    pub error: String,
    pub helper: Kept,
    /// it couldn't start (or the sandbox): not started again until it isn't needed
    pub dead: bool,
    /// the rest after it ended by itself while needed (started again after it,
    /// as Quick Share's helper is)
    pub backoff: Backoff,
    pub last_receiving: bool,
    pub last_alias: String,
}

/// What the sheet and Settings say when the helper ended on its own: its
/// last error line, if it left one
pub fn crash_text(err: &str) -> String {
    let last = err.lines().rev().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    let last: String = last.chars().take(160).collect();
    if last.is_empty() {
        "LocalSend stopped. Trying again…".into()
    } else {
        format!("LocalSend stopped ({last}). Trying again…")
    }
}

impl Sharing {
    pub fn local_available(&self) -> bool {
        crate::have("python3")
    }
    pub fn local_receiving(&self) -> bool {
        // (one visibility for both: Quick Share's, visibility.rs)
        self.visible_now()
    }
    pub fn local_alias(&self) -> String {
        self.my_name()
    }
    pub fn local_needed(&self) -> bool {
        self.local_available() && (self.local_receiving() || self.local.sharing || self.local.send_pending || !self.local.queued.is_empty() || !self.local.active.is_empty())
    }
    pub fn local_tell(&mut self, obj: Value) {
        if self.local.helper.running {
            self.local.helper.write(&format!("{obj}\n"));
        }
    }
    fn local_mode(&mut self) {
        let m = json!({"cmd": "mode", "mode": if self.local_receiving() { "everyone" } else { "off" }, "alias": self.local_alias()});
        self.local_tell(m);
    }
    pub fn local_scan(&mut self) {
        self.local_tell(json!({"cmd": "scan"}));
    }

    /// The QML's property bindings: the helper follows `needed`, and a change
    /// of receiving or the alias is told to it
    pub fn local_sync(&mut self, h: &mut Host) {
        let (recv, alias) = (self.local_receiving(), self.local_alias());
        if recv != self.local.last_receiving || alias != self.local.last_alias {
            self.local.last_receiving = recv;
            self.local.last_alias = alias;
            self.local_mode();
        }
        let needed = self.local_needed();
        if !needed {
            self.local.dead = false;
            self.local.backoff.reset();
            self.sched.cancel("ls_retry");
        }
        // (not in a test sandbox: it would announce itself on the real network)
        if needed && tessera_core::env::sandboxed() {
            if !self.local.dead {
                eprintln!("tessera-ui: localsend: not started in the sandbox (it would announce on the real network)");
                self.local.dead = true;
            }
        } else if needed && !self.local.helper.running && !self.local.dead && !self.sched.has("ls_retry") {
            let script = self.script("localsend.py");
            let gen = self.next_gen();
            let cmd = vec!["python3".to_string(), "-I".into(), script];
            match spawn(h.sender("Sharing"), "localsend", gen, &cmd, &[], true, true) {
                Some(k) => {
                    eprintln!("tessera-ui: localsend: helper started (receiving {}, looking {}, sends queued {})", self.local_receiving(), self.local.sharing, self.local.queued.len());
                    self.local.helper = k;
                    self.local.backoff.began();
                    // (a crash's message goes; the helper says again what still holds)
                    self.local.error.clear();
                    self.local_mode();
                    self.local_scan();
                }
                None => self.local.dead = true,
            }
        } else if !needed && self.local.helper.running && self.local.helper.stdin.is_some() {
            eprintln!("tessera-ui: localsend: helper stopped (not visible, not looking, nothing sending)");
            self.local.helper.stop();
        }
        // While looking for devices (or for a queued send's), ask again now and then
        if (self.local.sharing || !self.local.queued.is_empty()) && self.local.helper.running {
            self.sched.set_if_later("ls_scan", 4000);
        } else {
            self.sched.cancel("ls_scan");
        }
        if !(self.local.send_pending) {
            self.sched.cancel("ls_pending");
        } else {
            self.sched.set_if_later("ls_pending", 180_000); // in case nothing ever answers
        }
    }

    pub fn local_exit(&mut self, gen: u64, err: &str) {
        if gen != self.local.helper.gen {
            return;
        }
        let needed = self.local_needed();
        eprintln!("tessera-ui: localsend: helper ended (needed {needed})");
        self.local.helper.ended();
        self.local.peers.clear();
        self.local.active.clear();
        self.helper_ended("localsend");
        if needed {
            // said, and started again after a rest (3 s doubling, at most a minute)
            self.local.error = crash_text(err);
            let ms = self.local.backoff.ended(3000, 60_000);
            self.sched.set("ls_retry", ms);
        }
    }

    pub fn local_send(&mut self, peer_id: &str, paths: &[String]) {
        let first = paths.first().map(|p| base(p)).unwrap_or_default();
        self.local.sending_name = if paths.len() > 1 { format!("{first} and {} more", paths.len() - 1) } else { first };
        self.local.send_pending = true;
        // (as Quick Share's: kept until the helper runs and knows the peer)
        self.local.queued.retain(|(id, _)| id != peer_id);
        self.local.queued.push((peer_id.to_string(), paths.to_vec()));
        eprintln!("tessera-ui: localsend: send of {} file(s) asked (helper {})", paths.len(), if self.local.helper.running { "running" } else { "starting" });
        self.local_flush();
    }
    fn local_flush(&mut self) {
        if !self.local.helper.running {
            return;
        }
        let known: Vec<String> = self.local.peers.iter().map(|p| p.2.clone()).collect();
        let (ready, wait): (Vec<_>, Vec<_>) = std::mem::take(&mut self.local.queued).into_iter().partition(|(id, _)| known.contains(id));
        self.local.queued = wait;
        for (id, paths) in ready {
            self.local_tell(json!({"cmd": "send", "to": id, "files": paths}));
        }
    }
    pub fn local_cancel(&mut self, session: &str) {
        self.local_tell(json!({"cmd": "cancel", "session": session}));
        self.local.active.remove(session);
    }

    /// A progress report: incoming keeps the name and sender from its prompt;
    /// outgoing is named after what's being sent (ev.name is the device)
    fn local_track(&mut self, ev: &Value) {
        let session = ev["session"].as_str().unwrap_or("").to_string();
        let prev = self.local.active.get(&session).cloned();
        let now = tessera_core::util::now_ms();
        let incoming = ev["dir"] == "in";
        let done = ev["done"].as_f64().unwrap_or(0.0);
        let speed = match &prev {
            Some(p) if now > p["at"].as_i64().unwrap_or(0) => ((done - p["done"].as_f64().unwrap_or(0.0)) * 1000.0 / (now - p["at"].as_i64().unwrap_or(0)) as f64).max(0.0),
            _ => 0.0,
        };
        let name = prev.as_ref().and_then(|p| p["name"].as_str().map(str::to_string)).unwrap_or_else(|| if incoming { ev["name"].as_str().unwrap_or("").to_string() } else { self.local.sending_name.clone() });
        let peer = if incoming { prev.as_ref().and_then(|p| p["peer"].as_str().map(str::to_string)).unwrap_or_default() } else { ev["name"].as_str().unwrap_or("").to_string() };
        self.local.active.set(
            &session,
            json!({"id": session, "kind": if incoming {"receive"} else {"send"}, "name": name, "peer": peer, "done": done, "total": ev["total"].as_f64().unwrap_or(0.0), "speed": speed, "at": now, "waiting": ev["waiting"].as_bool().unwrap_or(false)}),
        );
    }

    pub fn local_line(&mut self, h: &mut Host, gen: u64, line: &str) {
        if gen != self.local.helper.gen {
            return;
        }
        let Some(ev) = json_line(line) else { return };
        let kind = ev["ev"].as_str().unwrap_or("");
        // (peers come every few seconds while looking: logged when the count changes)
        if kind != "peers" || ev["peers"].as_array().map_or(0, Vec::len) != self.local.peers.len() {
            crate::quickshare::log_event("localsend", kind, &ev);
        }
        match kind {
            "peers" => {
                self.local.peers = ev["peers"]
                    .as_array()
                    .map(|a| a.iter().map(|p| (js(&p["alias"]), js(&p["type"]), js(&p["id"]))).collect())
                    .unwrap_or_default();
                self.local_flush();
            }
            "incoming" => {
                let files = ev["files"].as_array().cloned().unwrap_or_default();
                let n = files.len();
                let first = files.first().map(|f| js(&f["name"])).unwrap_or_default();
                let what = if n <= 1 { first } else { format!("{first} and {} more", n - 1) };
                let session = js(&ev["session"]);
                self.local.active.set(&session, json!({"id": session, "kind": "receive", "name": what, "peer": ev["from"], "done": 0, "total": ev["total"], "speed": 0, "at": tessera_core::util::now_ms(), "waiting": true}));
                // (the AirDrop-style card asks: the share surface's request card)
                self.add_request(json!({"id": format!("ls:{session}"), "via": "localsend", "session": session, "from": ev["from"], "type": ev.get("type").filter(|t| !t.is_null()).cloned().unwrap_or(json!("mobile")),
                    "what": what, "files": files, "total": ev["total"].as_f64().unwrap_or(0.0), "pin": ""}));
            }
            "progress" => {
                self.local_track(&ev);
                if ev["dir"] == "out" {
                    self.local.send_pending = false; // tracked as an item now (after, or the helper would stop in between)
                }
            }
            "finished" => {
                let session = js(&ev["session"]);
                self.local.active.remove(&session);
                let paths: Vec<String> = ev["paths"].as_array().map(|a| a.iter().map(js).collect()).unwrap_or_default();
                let id = format!("ls:{session}");
                if ev["dir"] == "in" && self.shown_request(&id) && !paths.is_empty() {
                    self.update_request(&id, json!({"state": "done", "paths": paths}));
                    self.play(h, "received");
                } else if ev["dir"] == "in" {
                    self.drop_request(&id);
                    let first = paths.first().cloned().unwrap_or_default();
                    // (the folder they landed in, by its own name: Downloads in your language)
                    let wher = crate::folder_of(&first);
                    let sub = if paths.len() == 1 { base(&first) } else { format!("{} files in {wher}", paths.len()) };
                    let action = if paths.len() == 1 { act("panels.quickLook", json!([first])) } else { act("share.openDownloads", json!([])) };
                    self.island(h, json!({"kind": "custom", "icon": "check-circle", "title": format!("Received from {}", js(&ev["from"])), "subtitle": sub, "color": GREEN, "action": "Show", "onAction": action}), 6000);
                    self.play(h, "received");
                } else {
                    if let Some(to) = ev["to"].as_str() {
                        self.outcome(to, "sent");
                    }
                    let sub = if paths.len() == 1 { base(&paths[0]) } else { format!("{} files", paths.len()) };
                    self.island(h, json!({"kind": "custom", "key": format!("send:{}", ev["to"].as_str().unwrap_or("device")), "icon": "paper-plane-tilt", "title": format!("Sent to {}", ev["to"].as_str().unwrap_or("device")), "subtitle": sub, "color": GREEN}), 3000);
                }
            }
            "declined" => {
                let session = js(&ev["session"]);
                self.local.send_pending = false;
                self.local.active.remove(&session);
                // Incoming: declined here needs no word; unanswered, it expired
                if ev["dir"] == "in" {
                    self.drop_request(&format!("ls:{session}"));
                    if ev["expired"].as_bool().unwrap_or(false) {
                        self.island(h, json!({"kind": "custom", "icon": "clock", "title": format!("Request from {} expired", js(&ev["from"])), "subtitle": "Nothing was received", "color": GREY}), 3000);
                    }
                } else {
                    if let Some(to) = ev["to"].as_str() {
                        self.outcome(to, "declined");
                    }
                    self.island(h, json!({"kind": "custom", "key": format!("send:{}", ev["to"].as_str().unwrap_or("device")), "failure": true, "icon": "x-circle", "title": format!("{} declined", ev["to"].as_str().unwrap_or("They")), "subtitle": "Nothing was sent", "color": GREY}), 3000);
                }
            }
            "failed" => {
                let session = js(&ev["session"]);
                let out = session.starts_with("out-");
                if out {
                    self.local.send_pending = false;
                    if let Some(peer) = self.local.active.get(&session).and_then(|a| a["peer"].as_str()).filter(|p| !p.is_empty()).map(str::to_string) {
                        self.outcome(&peer, "failed");
                    }
                }
                self.local.active.remove(&session);
                let id = format!("ls:{session}");
                let msg = js(&ev["message"]);
                if !out && self.shown_request(&id) {
                    self.update_request(&id, json!({"state": "failed", "message": if msg == "Cancelled" { String::new() } else { msg }}));
                    return;
                }
                self.drop_request(&id);
                if msg != "Cancelled" {
                    self.island(h, json!({"kind": "custom", "key": "localsend", "failure": true, "icon": "warning-circle", "title": "Sharing stopped", "subtitle": msg, "color": ORANGE}), 5000);
                }
            }
            "error" => self.local.error = js(&ev["message"]),
            _ => {}
        }
    }
}

/// A JSON value as text ("" for null: what String(x ?? "") gives)
pub fn js(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        o => o.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_crash_is_said_with_its_last_line() {
        assert_eq!(crash_text(""), "LocalSend stopped. Trying again…");
        assert_eq!(crash_text("Traceback…\n  File x\nOSError: boom\n\n"), "LocalSend stopped (OSError: boom). Trying again…");
    }

    #[test]
    fn a_helper_that_ends_while_needed_is_started_again_after_a_rest() {
        let mut s = Sharing::default();
        s.local.sharing = true; // (the sheet is looking: it's needed)
        if !s.local_needed() {
            return; // (no python3 here: never needed)
        }
        let gen = s.local.helper.gen;
        s.local_exit(gen, "OSError: boom");
        assert!(s.local.error.contains("OSError: boom"));
        assert!(s.sched.has("ls_retry"));
        assert!(!s.local.dead);
    }
}

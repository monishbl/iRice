//! Sharing with nearby devices through the LocalSend protocol (phones and
//! computers running LocalSend, on the same network): shell/services/
//! LocalSend.qml. The work is done by assets/scripts/localsend.py; this
//! keeps the list of nearby devices, asks (the share card) before accepting
//! files, and shows transfers as a live activity.
//! Receiving (Settings -> Sharing): Off | Everyone for 10 minutes | Everyone.
//! The helper only runs while receiving, or while you're sharing something.

use crate::proc::{json_line, spawn, Kept};
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
    /// "Everyone for 10 minutes" turns itself off (ms since the epoch)
    pub everyone_until: i64,
    /// the share sheet is open or a send is running
    pub sharing: bool,
    /// (alias, type, id)
    pub peers: Vec<(String, String, String)>,
    pub active: Active,
    /// A send was asked for and hasn't reported back yet (the sheet may be gone)
    pub send_pending: bool,
    /// what the next outgoing transfer is called
    pub sending_name: String,
    pub error: String,
    pub helper: Kept,
    /// it ended by itself while needed: not started again until it isn't needed
    pub dead: bool,
    pub last_receiving: bool,
    pub last_alias: String,
}

impl Sharing {
    pub fn local_available(&self) -> bool {
        tessera_core::util::on_path("python3")
    }
    pub fn local_receiving(&self) -> bool {
        let r = self.cfg_str("sharing.receiving", "off");
        r == "everyone" || (r == "10min" && self.local.everyone_until > tessera_core::util::now_ms())
    }
    pub fn local_alias(&self) -> String {
        self.my_name()
    }
    pub fn local_needed(&self) -> bool {
        self.local_available() && (self.local_receiving() || self.local.sharing || self.local.send_pending || !self.local.active.is_empty())
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
        }
        if needed && !self.local.helper.running && !self.local.dead {
            let script = self.script("localsend.py");
            let gen = self.next_gen();
            let cmd = vec!["python3".to_string(), "-I".into(), script];
            match spawn(h.sender("Sharing"), "localsend", gen, &cmd, &[], true, true) {
                Some(k) => {
                    self.local.helper = k;
                    self.local_mode();
                    self.local_scan();
                }
                None => self.local.dead = true,
            }
        } else if !needed && self.local.helper.running {
            self.local.helper.stop();
        }
        // While looking for devices, ask again now and then
        if self.local.sharing && self.local.helper.running {
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

    pub fn local_exit(&mut self, gen: u64) {
        if gen != self.local.helper.gen {
            return;
        }
        let needed = self.local_needed();
        self.local.helper.ended();
        self.local.peers.clear();
        self.local.active.clear();
        self.helper_ended("localsend");
        if needed {
            self.local.dead = true;
        }
    }

    pub fn local_send(&mut self, peer_id: &str, paths: &[String]) {
        let first = paths.first().map(|p| base(p)).unwrap_or_default();
        self.local.sending_name = if paths.len() > 1 { format!("{first} and {} more", paths.len() - 1) } else { first };
        self.local.send_pending = true;
        self.local_tell(json!({"cmd": "send", "to": peer_id, "files": paths}));
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
        match ev["ev"].as_str().unwrap_or("") {
            "peers" => {
                self.local.peers = ev["peers"]
                    .as_array()
                    .map(|a| a.iter().map(|p| (js(&p["alias"]), js(&p["type"]), js(&p["id"]))).collect())
                    .unwrap_or_default();
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
                    let sub = if paths.len() == 1 { base(&first) } else { format!("{} files in Downloads", paths.len()) };
                    let action = if paths.len() == 1 { act("panels.quickLook", json!([first])) } else { act("share.openDownloads", json!([])) };
                    self.island(h, json!({"kind": "custom", "icon": "check-circle", "title": format!("Received from {}", js(&ev["from"])), "subtitle": sub, "color": GREEN, "action": "Show", "onAction": action}), 6000);
                    self.play(h, "received");
                } else {
                    if let Some(to) = ev["to"].as_str() {
                        self.outcome(to, "sent");
                    }
                    let sub = if paths.len() == 1 { base(&paths[0]) } else { format!("{} files", paths.len()) };
                    self.island(h, json!({"kind": "custom", "icon": "paper-plane-tilt", "title": format!("Sent to {}", ev["to"].as_str().unwrap_or("device")), "subtitle": sub, "color": GREEN}), 3000);
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
                    self.island(h, json!({"kind": "custom", "icon": "x-circle", "title": format!("{} declined", ev["to"].as_str().unwrap_or("They")), "subtitle": "Nothing was sent", "color": GREY}), 3000);
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
                    self.island(h, json!({"kind": "custom", "icon": "warning-circle", "title": "Sharing stopped", "subtitle": msg, "color": ORANGE}), 5000);
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

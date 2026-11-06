//! Mirror the screen to a TV (shell/services/Mirroring.qml; assets/scripts/
//! screen-mirror.py, through GNOME Network Displays): Miracast TVs and
//! sticks, Windows' Miracast over the network, and Chromecasts. Control
//! Center's Screen Mirroring tile and Settings -> Displays pick one; the
//! island says while it's going. TVs are looked for only while something is
//! showing them (`look`), or a mirror runs.   ipc: mirroring stop

use crate::localsend::js;
use crate::proc::{json_line, Keep};
use crate::{Job, Sharing};
use serde_json::{json, Value};
use tessera_core::host::Host;

pub struct Mirroring {
    /// [{ uuid, name, protocol, state }]
    pub displays: Vec<Value>,
    pub p2p: bool,
    pub error: String,
    pub lookers: i32,
    pub watch: Keep,
    /// the mirror: its stream (a user unit)
    pub stream: String,
    /// uuid
    pub target: String,
    pub last_name: String,
    pub install_error: String,
    pub last_state: String,
}

impl Default for Mirroring {
    fn default() -> Self {
        Self { displays: vec![], p2p: true, error: String::new(), lookers: 0, watch: Keep::new("mirror-watch", "mirror_rest"), stream: String::new(), target: String::new(), last_name: String::new(), install_error: String::new(), last_state: String::new() }
    }
}

impl Sharing {
    pub fn mirror_available(&self) -> bool {
        tessera_core::util::on_path("gnome-network-displays-daemon")
    }
    pub fn mirror_look(&mut self, on: bool) {
        self.mirror.lookers = (self.mirror.lookers + if on { 1 } else { -1 }).max(0);
    }
    fn mirror_looking(&self) -> bool {
        self.mirror_available() && (self.mirror.lookers > 0 || !self.mirror.stream.is_empty())
    }
    pub fn mirror_current(&self) -> Option<&Value> {
        self.mirror.displays.iter().find(|d| js(&d["uuid"]) == self.mirror.target)
    }
    pub fn mirror_active(&self) -> bool {
        !self.mirror.stream.is_empty()
    }
    pub fn mirror_target_name(&self) -> String {
        self.mirror_current().map(|d| js(&d["name"])).unwrap_or_else(|| self.mirror.last_name.clone())
    }
    pub fn mirror_state(&self) -> String {
        match self.mirror_current() {
            Some(d) if !d["state"].is_null() => js(&d["state"]),
            _ => if self.mirror_active() { "connecting".into() } else { String::new() },
        }
    }
    pub fn mirror_starting(&self) -> bool {
        !self.mirror.target.is_empty() && self.mirror.stream.is_empty()
    }

    /// The watcher follows `looking`; a TV going into error ends the mirror
    pub fn mirror_sync(&mut self, h: &mut Host) {
        let looking = self.mirror_looking();
        let cmd = vec!["python3".to_string(), "-I".into(), self.script("screen-mirror.py"), "watch".into()];
        self.mirror.watch.sync(&mut self.sched, h.sender("Sharing"), looking, &cmd, 10_000, true);
        let st = self.mirror_state();
        if st != self.mirror.last_state {
            self.mirror.last_state = st.clone();
            if st == "error" && self.mirror_active() {
                let name = self.mirror_target_name();
                let proto = self.mirror_current().map(|d| js(&d["protocol"])).unwrap_or_default();
                let sub = if !self.mirror.p2p && proto == "miracast" { "This computer's Wi-Fi can't do Wi-Fi Direct, which Miracast needs." } else { "Check the TV is on and showing its screen-mirroring page, then try again." };
                self.island(h, json!({"kind": "custom", "icon": "screencast", "title": format!("Couldn't mirror to {name}"), "subtitle": sub, "color": crate::DANGER}), 8000);
                self.mirror_stop();
            }
        }
    }
    pub fn mirror_line(&mut self, gen: u64, line: &str) {
        if !self.mirror.watch.is(gen) {
            return;
        }
        if let Some(m) = json_line(line) {
            self.mirror.displays = m["displays"].as_array().cloned().unwrap_or_default();
            self.mirror.p2p = m["p2p"].as_bool().unwrap_or(true);
            if let Some(e) = m["error"].as_str().filter(|e| !e.is_empty()) {
                self.mirror.error = e.into();
            }
        }
    }
    pub fn mirror_exit(&mut self, gen: u64) {
        let wanted = self.mirror_looking();
        self.mirror.watch.ended(&mut self.sched, gen, wanted, 10_000);
    }

    pub fn mirror_start(&mut self, h: &mut Host, uuid: &str, name: &str) {
        if self.mirror_active() || self.mirror_starting() {
            self.mirror_stop();
        }
        self.mirror.error.clear();
        self.mirror.target = uuid.into();
        self.mirror.last_name = name.into();
        let cmd = vec!["python3".to_string(), "-I".into(), self.script("screen-mirror.py"), "start".into(), uuid.into()];
        self.run(h, Job::MirrorStart(uuid.into()), &cmd, None);
    }
    pub fn mirror_started(&mut self, uuid: &str, stdout: &str) {
        let r = serde_json::from_str::<Value>(stdout.trim()).unwrap_or_default();
        if self.mirror.target != uuid || !self.mirror.stream.is_empty() {
            // (stopped, or another TV chosen, before it answered)
            if r["ok"].as_bool().unwrap_or(false) && !r["stream"].is_null() {
                let script = self.script("screen-mirror.py");
                crate::proc::detach(&["python3", "-I", &script, "stop", &js(&r["stream"])]);
            }
            return;
        }
        if r["ok"].as_bool().unwrap_or(false) {
            self.mirror.stream = js(&r["stream"]);
        } else {
            self.mirror.target.clear();
            self.mirror.error = r["error"].as_str().filter(|e| !e.is_empty()).unwrap_or("The TV didn't answer.").to_string();
        }
    }
    pub fn mirror_stop(&mut self) {
        if !self.mirror.stream.is_empty() {
            let script = self.script("screen-mirror.py");
            crate::proc::detach(&["python3", "-I", &script, "stop", &self.mirror.stream.clone()]);
        }
        self.mirror.stream.clear();
        self.mirror.target.clear();
    }
    pub fn protocol_name(p: &str) -> &'static str {
        match p {
            "chromecast" => "Chromecast",
            "mice" => "Miracast (network)",
            _ => "Miracast",
        }
    }
    /// gnome-network-displays, through Software Update's job
    pub fn mirror_install(&mut self) {
        if self.upd.running() {
            return;
        }
        self.mirror.install_error.clear();
        self.update_run("mirror", &["--install", "gnome-network-displays"]);
    }
    pub fn mirror_state_json(&self) -> Value {
        json!({"available": self.mirror_available(), "displays": self.mirror.displays, "p2p": self.mirror.p2p, "error": self.mirror.error, "active": self.mirror_active(),
            "target": self.mirror.target, "targetName": self.mirror_target_name(), "state": self.mirror_state(), "starting": self.mirror_starting(),
            "installing": self.upd.installing("mirror"), "installError": self.mirror.install_error})
    }
}

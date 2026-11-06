//! CloudDrives.qml: Google Drive and OneDrive as folders (~/Google Drive,
//! ~/OneDrive) with rclone (assets/scripts/cloud-drive.py). Connected
//! drives are mounted again once, at login. One job at a time, the rest wait.
//!
//! Bus "clouddrives": info, hasRclone, drives {gdrive, onedrive}, busy,
//! waiting, errors, installing, installError. Calls: clouddrives.refresh|
//! act [job, drive]|cancel|open [drive]|install|updatesFinished [job].
//! Opening a folder asks the desktop group: "files.openFolder" [path].

use super::common::*;
use serde_json::{json, Map, Value};
use std::any::Any;
use std::time::Duration;
use tessera_core::host::{Host, Service};

enum M {
    Status(Value),
    Job(String),
    Start,
}

pub struct CloudDrives {
    tx: Option<Tx>,
    info: Value,
    first: bool,
    busy: String,
    waiting: Vec<String>,
    errors: Map<String, Value>,
    install_asked: bool,
    install_error: String,
    job: Child,
    start_t: slint::Timer,
}

impl Default for CloudDrives {
    fn default() -> Self {
        Self { tx: None, info: json!({}), first: true, busy: String::new(), waiting: vec![], errors: Map::new(), install_asked: false, install_error: String::new(), job: Child::default(), start_t: slint::Timer::default() }
    }
}

impl CloudDrives {
    fn tx(&self) -> Tx {
        self.tx.clone().expect("started")
    }
    fn py(args: &[&str]) -> Vec<String> {
        let mut v = strs(&["python3", "-I"]);
        v.push(script("cloud-drive.py"));
        v.extend(args.iter().map(|s| s.to_string()));
        v
    }
    fn drive(&self, name: &str) -> Value {
        let d = &self.info["drives"][name];
        if d.is_object() { d.clone() } else { json!({"connected": false, "mounted": false, "folder": ""}) }
    }
    fn refresh(&self) {
        let tx = self.tx();
        std::thread::spawn(move || {
            let (_, out, _) = run_full(&Self::py(&["status"]), None);
            tx.send(M::Status(serde_json::from_str(&out).unwrap_or(Value::Null)));
        });
    }
    fn publish(&self) {
        put(
            "clouddrives",
            json!({"info": self.info, "hasRclone": self.info["rclone"].as_bool().unwrap_or(false),
                   "drives": {"gdrive": self.drive("gdrive"), "onedrive": self.drive("onedrive")},
                   "busy": self.busy, "waiting": self.waiting, "errors": self.errors,
                   "installing": updates_running() && self.install_asked, "installError": self.install_error}),
        );
    }
}

impl Service for CloudDrives {
    fn name(&self) -> &'static str {
        "CloudDrives"
    }
    fn calls(&self) -> &'static [&'static str] {
        &["clouddrives"]
    }
    fn start(&mut self, h: &mut Host) {
        let tx = Tx::new(h.sender("CloudDrives"));
        self.tx = Some(tx.clone());
        self.publish();
        // (Startup.later: after the desktop is up)
        self.start_t.start(slint::TimerMode::SingleShot, Duration::from_secs(5), move || tx.send(M::Start));
    }
    fn call(&mut self, h: &mut Host, call: &str, args: &Value) {
        let a = |i: usize| args.get(i).and_then(Value::as_str).unwrap_or("").to_string();
        match call.split('.').nth(1).unwrap_or("") {
            "refresh" => self.refresh(),
            "act" => {
                self.run_job(&a(0), &a(1));
            }
            // Connecting waits for the browser (up to 5 minutes): this stops waiting
            "cancel" => {
                if self.job.running() && self.busy.starts_with("connect ") {
                    self.job.stop();
                }
            }
            "open" => {
                let folder = self.drive(&a(0))["folder"].as_str().unwrap_or("").to_string();
                h.ask("files.openFolder", json!([folder]));
            }
            // rclone (and FUSE, for the folders) through Software Update's job
            "install" => {
                if updates_running() {
                    return;
                }
                self.install_asked = true;
                self.install_error.clear();
                let pk = if self.info["fuse"].as_bool().unwrap_or(false) { "rclone" } else { "rclone,fuse3" };
                h.ask("updates.update", json!([["--install", pk]]));
                self.publish();
            }
            "updatesFinished" => {
                if !self.install_asked {
                    return;
                }
                self.install_asked = false;
                let j = &args[0];
                self.install_error = if j["state"] == "done" {
                    String::new()
                } else {
                    j["error"].as_str().filter(|s| !s.is_empty()).or(j["text"].as_str().filter(|s| !s.is_empty())).unwrap_or("The install didn't finish.").to_string()
                };
                self.refresh();
                self.publish();
            }
            _ => {}
        }
        self.publish();
    }
    fn message(&mut self, _h: &mut Host, msg: Box<dyn Any + Send>) {
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Start => self.refresh(),
            M::Status(v) => {
                if v.is_object() {
                    self.info = v;
                }
                // At login: connected drives come back
                if self.first {
                    self.first = false;
                    for n in ["gdrive", "onedrive"] {
                        let d = self.drive(n);
                        if d["connected"].as_bool().unwrap_or(false) && !d["mounted"].as_bool().unwrap_or(false) {
                            self.run_job("mount", n);
                        }
                    }
                }
            }
            M::Job(out) => {
                let r: Value = serde_json::from_str(&out).unwrap_or_else(|_| json!({}));
                let name = self.busy.split(' ').nth(1).unwrap_or("").to_string();
                // ("": cancelled)
                if !r["ok"].as_bool().unwrap_or(false) && r["error"].as_str() != Some("") {
                    let e = r["error"].as_str().filter(|s| !s.is_empty()).unwrap_or("That didn't work. Try again.");
                    self.errors.insert(name, json!(e));
                }
                self.busy.clear();
                self.refresh();
                if !self.waiting.is_empty() {
                    let next = self.waiting.remove(0);
                    let mut it = next.splitn(2, ' ');
                    let (j, n) = (it.next().unwrap_or("").to_string(), it.next().unwrap_or("").to_string());
                    self.run_job(&j, &n);
                }
            }
        }
        self.publish();
    }
}

impl CloudDrives {
    /// act(job, name): runs now, or waits its turn
    fn run_job(&mut self, job: &str, name: &str) {
        let key = format!("{job} {name}");
        if self.job.running() || !self.busy.is_empty() {
            if self.busy != key && !self.waiting.contains(&key) {
                self.waiting.push(key);
            }
            return;
        }
        self.busy = key;
        self.errors.remove(name);
        let tx = self.tx();
        let buf = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let b2 = buf.clone();
        let started = self.job.spawn(
            &Self::py(&[job, name]),
            None,
            move |l| {
                if let Ok(mut b) = b2.lock() {
                    b.push_str(&l);
                    b.push('\n');
                }
            },
            move |_| tx.send(M::Job(buf.lock().map(|b| b.clone()).unwrap_or_default())),
        );
        if !started {
            self.busy.clear();
            self.errors.insert(name.to_string(), json!("That didn't work. Try again."));
        }
    }
}

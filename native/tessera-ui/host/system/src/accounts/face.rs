//! Face.qml: face unlock for the lock screen only (assets/scripts/face.py):
//! Howdy with an infrared camera. Setting up, adding and removing faces go
//! through the root helper (the password), only on a user action.
//!
//! Bus "face": info, camera, hasCamera, installed, device, setUp, faces,
//! lockScreen, installing, installAsked, installError, busy, error.
//! Calls: face.refresh|install|setUpCamera|addFace|clearFaces|
//! updatesFinished [job].

use super::common::*;
use serde_json::{json, Value};
use std::any::Any;
use tessera_core::host::{Host, Service};
use crate::util::config_set;

enum M {
    Status(Value),
    Job(i32, String, String),
}

#[derive(Default)]
pub struct Face {
    tx: Option<Tx>,
    info: Value,
    install_asked: bool,
    install_error: String,
    busy: String,
    error: String,
    running: bool,
    // config.json: lock.faceDevice, lock.faceCount, lock.face
    cfg_device: String,
    cfg_count: i64,
    cfg_face: bool,
}

impl Face {
    fn tx(&self) -> Tx {
        self.tx.clone().expect("started")
    }
    fn py(args: &[&str]) -> Vec<String> {
        let mut v = strs(&["python3", "-I"]);
        v.push(script("face.py"));
        v.extend(args.iter().map(|s| s.to_string()));
        v
    }
    fn refresh(&self) {
        let tx = self.tx();
        std::thread::spawn(move || {
            let (_, out, _) = run_full(&Self::py(&["status"]), None);
            if let Ok(v) = serde_json::from_str::<Value>(&out) {
                tx.send(M::Status(v));
            }
        });
    }
    fn read_cfg(&mut self, cfg: &Value) {
        self.cfg_device = cfg["lock"]["faceDevice"].as_str().unwrap_or("").to_string();
        self.cfg_count = cfg["lock"]["faceCount"].as_i64().unwrap_or(0);
        self.cfg_face = cfg["lock"]["face"].as_bool().unwrap_or(true);
    }
    fn camera(&self) -> Option<&Value> {
        self.info.get("camera").filter(|c| !c.is_null())
    }
    fn publish(&self) {
        let i = &self.info;
        let installed = i["howdy"].as_bool().unwrap_or(false) && i["pam"].as_bool().unwrap_or(false);
        // The camera Howdy uses: from its config, or (howdy-next keeps that
        // root's to read) the one Tessera set it up with
        let device = if i["private"].as_bool().unwrap_or(false) { self.cfg_device.clone() } else { i["device"].as_str().unwrap_or("").to_string() };
        let set_up = installed && !device.is_empty() && i["models"].as_bool().unwrap_or(false);
        put(
            "face",
            json!({"info": i, "camera": self.camera(), "hasCamera": self.camera().is_some(), "installed": installed, "device": device,
                   "setUp": set_up, "faces": self.cfg_count, "lockScreen": set_up && self.cfg_count > 0 && self.cfg_face,
                   "installing": updates_running() && self.install_asked, "installError": self.install_error,
                   "busy": self.busy, "error": self.error}),
        );
    }
    fn run(&mut self, job: &str) {
        if self.running {
            return;
        }
        self.error.clear();
        if !has_tool("pkexec") {
            self.error = NO_PKEXEC.into();
            self.publish();
            return;
        }
        let mut args: Vec<String> = vec!["face".into()];
        if job == "setup" {
            args.push("setup".into());
            args.push(self.camera().and_then(|c| c["path"].as_str()).unwrap_or("").to_string());
        } else {
            args.push(job.into());
        }
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let cmd = root_command(&refs);
        self.busy = job.into();
        self.running = true;
        let tx = self.tx();
        let job = job.to_string();
        std::thread::spawn(move || {
            let (code, out, err) = run_full(&cmd, None);
            tx.send(M::Job(code, job, format!("{err}\n{out}")));
        });
        self.publish();
    }
}

impl Service for Face {
    fn name(&self) -> &'static str {
        "Face"
    }
    fn calls(&self) -> &'static [&'static str] {
        &["face"]
    }
    fn start(&mut self, h: &mut Host) {
        self.tx = Some(Tx::new(h.sender("Face")));
        self.info = json!({});
        let cfg = h.cfg().clone();
        self.read_cfg(&cfg);
        self.publish();
        self.refresh();
    }
    fn config_changed(&mut self, h: &mut Host) {
        let cfg = h.cfg().clone();
        self.read_cfg(&cfg);
        self.publish();
    }
    fn call(&mut self, h: &mut Host, call: &str, args: &Value) {
        match call.split('.').nth(1).unwrap_or("") {
            "refresh" => self.refresh(),
            "install" => {
                let installed = self.publish_installed();
                if updates_running() || installed {
                    return;
                }
                self.install_asked = true;
                self.install_error.clear();
                h.ask("updates.update", json!([["--aur-install", "howdy-next"]]));
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
            "setUpCamera" => {
                if self.camera().is_some() {
                    self.run("setup");
                }
            }
            "addFace" => self.run("add"),
            "clearFaces" => self.run("clear"),
            _ => {}
        }
    }
    fn message(&mut self, _h: &mut Host, msg: Box<dyn Any + Send>) {
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Status(v) => self.info = v,
            M::Job(code, job, said_all) => {
                self.busy.clear();
                self.running = false;
                if code == 0 {
                    match job.as_str() {
                        "setup" => {
                            let path = self.camera().and_then(|c| c["path"].as_str()).unwrap_or("").to_string();
                            self.cfg_device = path.clone();
                            config_set("lock.faceDevice", json!(path));
                        }
                        "add" => {
                            self.cfg_count += 1;
                            config_set("lock.faceCount", json!(self.cfg_count));
                        }
                        "clear" => {
                            self.cfg_count = 0;
                            config_set("lock.faceCount", json!(0));
                        }
                        _ => {}
                    }
                } else if code != 126 {
                    // (126: the password prompt was cancelled)
                    let said = said_all.trim().lines().filter(|l| !l.trim().is_empty()).last().unwrap_or("").to_string();
                    let said = root_message(&said);
                    self.error = if code == 127 {
                        "Not allowed (an administrator's password is needed).".into()
                    } else if job == "add" {
                        format!("No face was found. Look at the camera, then try again. If the camera's infrared light stays off, it needs linux-enable-ir-emitter (AUR) set up once in a terminal: sudo linux-enable-ir-emitter configure.{}", if said.is_empty() { String::new() } else { format!(" ({said})") })
                    } else if said.is_empty() {
                        "That didn't work. Update Tessera, then try again.".into()
                    } else {
                        said
                    };
                }
                self.refresh();
            }
        }
        self.publish();
    }
}

impl Face {
    fn publish_installed(&self) -> bool {
        self.info["howdy"].as_bool().unwrap_or(false) && self.info["pam"].as_bool().unwrap_or(false)
    }
}

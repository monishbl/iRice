//! Software Update's job, as the sharing services use it (shell/services/
//! Updates.qml is host-system's): they install a package for a feature
//! (pipewire-zeroconf for AirPlay, gnome-network-displays for mirroring,
//! scrcpy for the phone, qrencode for the QR card, an app from the store).
//! The job itself is sysupdate.py, run detached exactly as Updates.update()
//! does; its progress is the status file it writes
//! ($XDG_RUNTIME_DIR/tessera-sysupdate/status.json), read here every half
//! second while a job of ours runs. Who asked is remembered, so the right
//! service hears when it finishes (the QML's `installAsked` and
//! Connections { target: Updates; onFinished }).

use crate::{Job, Sharing};
use serde_json::{json, Value};
use tessera_core::host::Host;

#[derive(Default)]
pub struct Upd {
    pub job: Option<Value>,
    /// when it was asked (an older status file is the last run's), seconds
    pub requested: f64,
    /// who asked: "airplay" | "mirror" | "phone" | "qr" | "store" | "catalogue" | "flatpak"
    pub who: Vec<&'static str>,
    pub reading: bool,
}

impl Upd {
    pub fn running(&self) -> bool {
        self.job.as_ref().is_some_and(|j| j["state"] == "running")
    }
    /// `installing` of a service: the job runs and it asked for it
    pub fn installing(&self, who: &str) -> bool {
        self.running() && self.who.contains(&who)
    }
}

impl Sharing {
    fn upd_dir() -> String {
        format!("{}/tessera-sysupdate", std::env::var("XDG_RUNTIME_DIR").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "/tmp".into()))
    }

    /// Updates.update(args) for `who`; false when one is running
    pub fn update_run(&mut self, who: &'static str, args: &[&str]) -> bool {
        if self.upd.running() {
            return false;
        }
        let fake = tessera_core::env::sandboxed() || std::env::var("TESSERA_FAKE_SYSUPDATE").as_deref() == Ok("1");
        let script = self.script("sysupdate.py");
        let mut cmd: Vec<String> = vec!["setsid".into(), "-f".into(), "python3".into(), "-I".into(), script, "run".into()];
        cmd.extend(args.iter().map(|a| a.to_string()));
        if fake {
            cmd.push("--fake".into());
        }
        // setsid, in this login session (so pkexec finds Tessera's password
        // dialog through polkit); not tied to the shell
        let refs: Vec<&str> = cmd.iter().map(String::as_str).collect();
        crate::proc::detach(&refs);
        let now = tessera_core::util::now_ms() as f64 / 1000.0;
        self.upd.requested = now;
        self.upd.who.retain(|w| *w != who);
        self.upd.who.push(who);
        self.upd.job = Some(json!({"state": "running", "phase": "starting", "text": "Starting", "percent": 0, "done": 0, "total": 0, "started": now}));
        self.sched.set("upd_poll", 500);
        true
    }

    pub fn update_poll(&mut self, h: &mut Host) {
        if !self.upd.running() || self.upd.reading {
            return;
        }
        self.upd.reading = true;
        let file = format!("{}/status.json", Self::upd_dir());
        let cmd = crate::proc::s(&["cat", &file]);
        self.run(h, Job::UpdStatus, &cmd, None);
    }

    /// The status file was read
    pub fn update_status(&mut self, h: &mut Host, text: &str) {
        self.upd.reading = false;
        if let Ok(j) = serde_json::from_str::<Value>(text.trim()) {
            // The last run's file, read before the new run wrote its own
            let older = self.upd.requested > 0.0 && j["started"].as_f64().unwrap_or(0.0) < self.upd.requested - 2.0;
            if !older {
                let was_running = self.upd.running();
                self.upd.job = Some(j.clone());
                if was_running && j["state"] != "running" {
                    self.update_finished(h, &j);
                }
            }
        }
        if self.upd.running() {
            self.sched.set("upd_poll", 500);
        }
    }

    /// Updates.finished(j): whoever asked hears it
    fn update_finished(&mut self, h: &mut Host, j: &Value) {
        let who = std::mem::take(&mut self.upd.who);
        let ok = j["state"] == "done";
        let err = || j["error"].as_str().filter(|e| !e.is_empty()).or_else(|| j["text"].as_str()).unwrap_or("The install didn't finish.").to_string();
        self.ports_updates_finished();
        for w in who {
            match w {
                "airplay" => {
                    self.air.install_error = if ok { String::new() } else { err() };
                    self.air_check(h);
                }
                "mirror" => self.mirror.install_error = if ok { String::new() } else { err() },
                "phone" => {
                    self.phone.install_error = if ok { String::new() } else { err() };
                    self.phone_check_cam(h);
                }
                "store" | "catalogue" | "flatpak" => {
                    self.store_update_finished(h, j);
                    self.store_flatpak_installed(h, j);
                }
                _ => {}
            }
        }
    }
}

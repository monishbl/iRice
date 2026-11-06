//! The firewall's way in for nearby sharing (shell/services/SharingPorts.qml):
//! open only while something needs it, closed otherwise (never for good).
//! Needed while you're visible to others (Quick Share: its ports, DHCP for
//! its hotspot, mDNS; LocalSend receiving: 53317), while the share sheet
//! looks for devices (their answers come in: LocalSend's 53317, mDNS), and
//! while a QR-code share or a cast serves a file (53319-53320). Sending
//! itself needs nothing open: the connection goes out.
//!
//! This only runs the same helper script the QML runs (sharing-check.py
//! `ports`, which asks Tessera's root helper); it never touches a firewall
//! or a root tool itself. Opening goes at once, closing a few seconds later
//! (a quick off and on asks once), and once at startup. Never in the dev
//! sandbox: it shares the host's system bus and pkexec.

use crate::{Job, Sharing};
use tessera_core::host::Host;

pub struct Ports {
    /// what's open now, as last asked ("" closed; "?" not asked yet)
    pub asked: String,
    pub error: String,
    /// The last ask's exit code when it failed: 64 an older helper (until an
    /// update), 126 cancelled, 127 not allowed (until Try again); anything
    /// else, until what's needed changes
    pub failed_with: i32,
    pub running: bool,
    /// what the running ask asks for
    pub job_sets: String,
    /// the need as last seen (a change schedules an ask)
    pub last_need: (String, String),
}

impl Default for Ports {
    fn default() -> Self {
        Self { asked: "?".into(), error: String::new(), failed_with: 0, running: false, job_sets: String::new(), last_need: (String::new(), String::new()) }
    }
}

impl Ports {
    pub fn held(&self) -> bool {
        matches!(self.failed_with, 64 | 126 | 127)
    }
}

impl Sharing {
    /// What's needed now: the root helper's set names (sorted, joined), and why
    pub fn ports_need(&self) -> (String, String) {
        let mut sets: Vec<&str> = Vec::new();
        let mut why: Vec<&str> = Vec::new();
        let mut add = |names: &[&'static str], reason: &'static str| {
            for n in names {
                if !sets.contains(n) {
                    sets.push(n);
                }
            }
            why.push(reason);
        };
        if self.quick_visible() || self.local_receiving() {
            let mut names: Vec<&'static str> = Vec::new();
            if self.quick_visible() {
                names.extend(["quickshare", "mdns"]);
            }
            if self.local_receiving() {
                names.push("localsend");
            }
            add(&names, "visible to Everyone");
        }
        if self.looking() {
            add(&["localsend", "mdns"], "looking for devices");
        }
        if self.link.stage == "loading" || self.link.stage == "ready" {
            add(&["links"], "sharing with a QR code");
        }
        if self.cast.serves_file {
            add(&["links"], "casting a file");
        }
        sets.sort();
        (sets.join(","), why.join(", "))
    }

    /// onNeedChanged (called each turn: acts only when the need changed)
    pub fn ports_check(&mut self, h: &mut Host) {
        let need = self.ports_need();
        if need != self.ports.last_need {
            self.ports.last_need = need;
            if !self.ports.held() {
                self.ports.failed_with = 0;
            }
            self.ports_schedule();
        }
        let _ = h;
    }

    pub fn ports_schedule(&mut self) {
        if tessera_core::env::sandboxed() || self.ports.failed_with != 0 || self.ports.running {
            return;
        }
        let (sets, _) = self.ports_need();
        if sets == self.ports.asked {
            self.sched.cancel("ports");
            return;
        }
        let now: Vec<&str> = if self.ports.asked == "?" { vec![] } else { self.ports.asked.split(',').collect() };
        // a set not open yet: ask at once; only closing waits
        let ms = if sets.split(',').any(|s| !s.is_empty() && !now.contains(&s)) { 300 } else { 5000 };
        self.sched.set("ports", ms);
    }

    pub fn ports_ask(&mut self, h: &mut Host) {
        if tessera_core::env::sandboxed() || self.ports.running {
            return;
        }
        let (sets, why) = self.ports_need();
        self.ports.job_sets = sets.clone();
        self.ports.running = true;
        let script = self.script("sharing-check.py");
        let cmd = vec!["python3".to_string(), "-I".into(), script, "ports".into(), if sets.is_empty() { "none".into() } else { sets }, why];
        self.run(h, Job::Ports, &cmd, None);
    }

    /// Settings' Try again
    pub fn ports_retry(&mut self, h: &mut Host) {
        self.ports.failed_with = 0;
        self.ports.error.clear();
        self.ports_ask(h);
    }

    pub fn ports_done(&mut self, stdout: &str, code: i32) {
        self.ports.running = false;
        let r = serde_json::from_str::<serde_json::Value>(stdout.trim()).unwrap_or_default();
        if code == 0 {
            self.ports.asked = self.ports.job_sets.clone();
            self.ports.error.clear();
        } else {
            self.ports.failed_with = code;
            let said = r["error"].as_str().filter(|e| !e.is_empty()).map(str::to_string);
            self.ports.error = format!(
                "{}{}",
                if self.ports.job_sets.is_empty() { "Sharing's ports didn't close. " } else { "Sharing's ports didn't open. " },
                said.unwrap_or_else(|| if code == 64 { "Tessera's system helper is from before this. Run Software Update once.".into() } else { "Try again.".into() })
            );
        }
        self.ports_schedule(); // (what's needed may have changed meanwhile)
    }

    /// Software Update finished: an update installs the newer helper
    pub fn ports_updates_finished(&mut self) {
        if self.ports.failed_with == 64 {
            self.ports.failed_with = 0;
            self.ports.error.clear();
            self.ports_schedule();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn held_codes_wait_for_try_again() {
        let mut p = Ports::default();
        assert_eq!(p.asked, "?");
        p.failed_with = 126;
        assert!(p.held());
        p.failed_with = 1;
        assert!(!p.held());
    }
}

//! The firewall's way in for nearby sharing (shell/services/SharingPorts.qml):
//! open only while something needs it, closed otherwise (never for good).
//! Needed while you're visible to others (Quick Share: its ports, DHCP for
//! its hotspot, mDNS; LocalSend receiving: 53317), while the share sheet
//! looks for devices (their answers come in: LocalSend's 53317, mDNS), and
//! while a QR-code share or a cast serves a file (53319-53320), and while a
//! send is going (17 Nov: the connection goes out, but LocalSend's answers
//! and a phone's faster Wi-Fi link come back in), then closed again.
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
    /// closes that failed in a row (each retried after a longer rest)
    pub close_tries: u32,
}

impl Default for Ports {
    fn default() -> Self {
        Self { asked: "?".into(), error: String::new(), failed_with: 0, running: false, job_sets: String::new(), last_need: (String::new(), String::new()), close_tries: 0 }
    }
}

impl Ports {
    pub fn held(&self) -> bool {
        matches!(self.failed_with, 64 | 126 | 127)
    }
}

/// When the shell quits: is anything open (or being opened) that must be
/// closed on the way out? ("?" never asked: nothing of this run's is open)
pub fn quit_closes(asked: &str, running: bool, job_sets: &str) -> bool {
    (!asked.is_empty() && asked != "?") || (running && !job_sets.is_empty())
}

/// The rest before trying a failed close again: a close that failed is never
/// left alone (the ports would stay open), 10 s, 30 s, then every 2 min
pub fn close_retry_ms(tries: u32) -> u64 {
    match tries {
        0 | 1 => 10_000,
        2 => 30_000,
        _ => 120_000,
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
            // (Quick Share's too: a phone found over Bluetooth answers on its
            // ports, and its devices' mDNS answers come in through mdns)
            add(&["localsend", "mdns", "quickshare"], "looking for devices");
        }
        if self.sending_now() {
            // a send that outlives the sheet: the phone's link upgrade
            // (Wi-Fi Direct, hotspot) and LocalSend's answers come back in
            add(&["localsend", "mdns", "quickshare"], "sending");
        }
        // a receive that outlives being visible (10 minutes ran out, or No
        // one mid-transfer): its Wi-Fi upgrade, the hotspot's DHCP and
        // LocalSend's per-file requests still come in until it ends
        let (quick_in, local_in) = self.receiving_now();
        if quick_in {
            add(&["mdns", "quickshare"], "receiving");
        }
        if local_in {
            add(&["localsend"], "receiving");
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

    /// A send asked for or going, by either protocol
    pub fn sending_now(&self) -> bool {
        let out = |v: &Vec<serde_json::Value>| v.iter().any(|it| it["kind"] == "send");
        self.quick.send_pending || self.local.send_pending || out(&self.quick.active.values()) || out(&self.local.active.values())
    }

    /// A receive going, by protocol: (Quick Share, LocalSend)
    pub fn receiving_now(&self) -> (bool, bool) {
        let inc = |v: &Vec<serde_json::Value>| v.iter().any(|it| it["kind"] == "receive");
        (inc(&self.quick.active.values()), inc(&self.local.active.values()))
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
            self.ports.close_tries = 0;
            self.sched.cancel("ports_close_retry");
        } else {
            self.ports.failed_with = code;
            if self.ports.job_sets.is_empty() && !self.ports.held() {
                // a close that failed (busy firewall, a timeout): tried again
                // on its own, not left open until the need next changes
                self.ports.close_tries += 1;
                self.sched.set("ports_close_retry", close_retry_ms(self.ports.close_tries));
            }
            let said = r["error"].as_str().filter(|e| !e.is_empty()).map(str::to_string);
            self.ports.error = format!(
                "{}{}",
                if self.ports.job_sets.is_empty() { "Sharing's ports didn't close. " } else { "Sharing's ports didn't open. " },
                said.unwrap_or_else(|| if code == 64 { "Tessera's system helper is from before this. Run Software Update once.".into() } else { "Try again.".into() })
            );
        }
        self.ports_schedule(); // (what's needed may have changed meanwhile)
    }

    /// A failed close's rest is over: ask again (unless held, or something
    /// else got through meanwhile)
    pub fn ports_close_retry(&mut self) {
        if self.ports.failed_with != 0 && !self.ports.held() {
            self.ports.failed_with = 0;
            self.ports_schedule();
        }
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

    #[test]
    fn a_failed_close_is_tried_again_with_longer_rests() {
        assert_eq!(close_retry_ms(1), 10_000);
        assert_eq!(close_retry_ms(2), 30_000);
        assert_eq!(close_retry_ms(3), 120_000);
        assert_eq!(close_retry_ms(50), 120_000);
        let mut s = Sharing::default();
        s.ports.asked = "localsend".into();
        s.ports.job_sets = String::new();
        s.ports.running = true;
        s.ports_done("{\"error\": \"busy\"}", 1);
        assert_eq!(s.ports.failed_with, 1);
        assert_eq!(s.ports.close_tries, 1);
        assert!(s.sched.has("ports_close_retry"));
        // (a cancelled close waits for Try again instead)
        let mut s = Sharing::default();
        s.ports.running = true;
        s.ports_done("", 126);
        assert!(!s.sched.has("ports_close_retry"));
        // a failed open waits for the need to change, as before
        let mut s = Sharing::default();
        s.ports.job_sets = "links".into();
        s.ports.running = true;
        s.ports_done("", 1);
        assert!(!s.sched.has("ports_close_retry"));
        // the retry's rest over: it clears the failure (the next ask closes)
        let mut s = Sharing::default();
        s.ports.failed_with = 1;
        s.ports_close_retry();
        assert_eq!(s.ports.failed_with, 0);
        s.ports.failed_with = 127;
        s.ports_close_retry();
        assert_eq!(s.ports.failed_with, 127);
    }

    #[test]
    fn quitting_closes_what_is_open() {
        assert!(!quit_closes("?", false, ""));
        assert!(!quit_closes("", false, ""));
        assert!(quit_closes("links", false, ""));
        assert!(quit_closes("", true, "localsend,mdns"));
        assert!(!quit_closes("", true, ""));
    }

    #[test]
    fn a_receive_keeps_its_ports_open_after_visibility_ends() {
        let mut s = Sharing::default();
        s.cfg = serde_json::json!({"sharing": {"quickShare": "off"}});
        assert_eq!(s.ports_need().0, "");
        s.quick.active.set("in-1", serde_json::json!({"id": "in-1", "kind": "receive"}));
        assert_eq!(s.ports_need().0, "mdns,quickshare");
        s.quick.active.clear();
        s.local.active.set("in-2", serde_json::json!({"id": "in-2", "kind": "receive"}));
        assert_eq!(s.ports_need().0, "localsend");
        assert_eq!(s.receiving_now(), (false, true));
    }
}

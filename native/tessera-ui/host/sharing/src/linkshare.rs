//! Sharing with a phone that has no Quick Share (an iPhone), through its
//! browser (shell/services/LinkShare.qml, assets/scripts/link-share.py): a QR
//! code its camera opens, on this computer's own network. Send: the page has
//! the files to save. Receive: the page takes photos and files, saved in the
//! Quick Share folder. Up only while the card shows, at most 10 minutes; the
//! link holds a secret.
//!   send(paths)   receive()   close()      (ipc: linkshare receive | close)

use crate::proc::{json_line, spawn, Kept};
use crate::{act, base, Sharing, GREEN};
use serde_json::{json, Value};
use tessera_core::host::Host;

#[derive(Default)]
pub struct LinkShare {
    /// "" | send | receive
    pub mode: String,
    /// "" | loading | ready | error
    pub stage: String,
    pub url: String,
    pub qr: String,
    pub error: String,
    /// what's being sent
    pub names: Vec<String>,
    /// files saved by the phone (send) or received
    pub count: i64,
    pub done: f64,
    pub total: f64,
    pub job: Kept,
    /// What arrived, in one alert per moment rather than one per file
    pub arrived: Vec<String>,
}

/// The card's words when sharing ended on its own while its code showed
pub const STOPPED: &str = "Sharing stopped. Make a new code to share again.";

impl LinkShare {
    pub fn busy(&self) -> bool {
        self.stage == "ready" && self.total > 0.0 && self.done < self.total
    }
}

impl Sharing {
    fn link_folder(&self) -> String {
        let q = self.quick_folder();
        if !q.is_empty() {
            return q;
        }
        // (your Downloads folder, in your language: user-dirs.dirs)
        tessera_core::util::user_dir("DOWNLOAD", "Downloads").to_string_lossy().into_owned()
    }
    pub fn link_send(&mut self, h: &mut Host, paths: &[String]) {
        self.link.names = paths.iter().map(|p| base(p)).collect();
        self.link_start(h, "send", paths.to_vec());
    }
    pub fn link_receive(&mut self, h: &mut Host) {
        self.link.names.clear();
        let folder = self.link_folder();
        self.link_start(h, "receive", vec![folder]);
    }
    fn link_start(&mut self, h: &mut Host, mode: &str, args: Vec<String>) {
        self.link_close(h);
        self.link.mode = mode.into();
        self.link.stage = "loading".into();
        h.ask("panels.close", json!([]));
        let mut cmd = vec!["python3".to_string(), "-I".into(), self.script("link-share.py"), "--name".into(), self.my_name(), mode.into()];
        cmd.extend(args);
        let gen = self.next_gen();
        match spawn(h.sender("Sharing"), "linkshare", gen, &cmd, &[], true, true) {
            Some(k) => self.link.job = k,
            None => {
                self.link.job = Kept { gen, ..Default::default() };
                self.link_exit(gen);
            }
        }
    }
    pub fn link_close(&mut self, h: &mut Host) {
        // (stopped, it removes its code and stops serving)
        self.link.job.stop();
        self.link.job = Kept::default();
        self.link.mode.clear();
        self.link.stage.clear();
        self.link.url.clear();
        self.link.qr.clear();
        self.link.error.clear();
        self.link.count = 0;
        self.link.done = 0.0;
        self.link.total = 0.0;
        self.link_flush(h);
    }

    fn link_flush(&mut self, h: &mut Host) {
        let paths = std::mem::take(&mut self.link.arrived);
        self.sched.cancel("link_gather");
        let Some(first) = paths.first().cloned() else { return };
        self.play(h, "received");
        let one = paths.len() == 1;
        self.island(
            h,
            json!({"kind": "custom", "icon": "check-circle", "title": if one { format!("Received \u{201c}{}\u{201d}", base(&first)) } else { format!("Received {} files", paths.len()) },
                "subtitle": "From a phone's browser", "color": GREEN,
                "action": if one { "Open" } else { "Show in Folder" }, "onAction": if one { act("share.fileOpen", json!([first])) } else { act("share.fileReveal", json!([first])) },
                "action2": if one { "Show in Folder" } else { "" }, "onAction2": act("share.fileReveal", json!([first]))}),
            8000,
        );
    }
    pub fn link_gathered(&mut self, h: &mut Host) {
        self.link_flush(h);
    }

    pub fn link_line(&mut self, gen: u64, line: &str) {
        if gen != self.link.job.gen {
            return; // (one that was closed)
        }
        let Some(ev) = json_line(line) else { return };
        match ev["ev"].as_str().unwrap_or("") {
            "ready" => {
                self.link.url = crate::localsend::js(&ev["url"]);
                self.link.qr = crate::localsend::js(&ev["qr"]);
                self.link.stage = "ready".into();
            }
            "progress" => {
                self.link.done = ev["done"].as_f64().unwrap_or(0.0);
                self.link.total = ev["total"].as_f64().unwrap_or(0.0);
            }
            "sent" => self.link.count += 1,
            "received" => {
                let paths: Vec<String> = ev["paths"].as_array().map(|a| a.iter().map(crate::localsend::js).collect()).unwrap_or_default();
                self.link.count += paths.len() as i64;
                self.link.arrived.extend(paths);
                self.sched.set("link_gather", 1500);
            }
            "error" => {
                self.link.error = crate::localsend::js(&ev["message"]);
                self.link.stage = "error".into();
            }
            "closed" => {
                // (any end while the code shows: a dead code isn't left up,
                // and the 'links' ports close with it)
                if self.link.stage == "ready" {
                    self.link.error = (if ev["why"] == "expired" { "The code expired after 10 minutes. Make a new one to share again." } else { STOPPED }).to_string();
                    self.link.stage = "error".into();
                }
            }
            _ => {}
        }
    }
    pub fn link_exit(&mut self, gen: u64) {
        if gen != self.link.job.gen {
            return;
        }
        self.link.job.ended();
        if self.link.stage == "loading" {
            if self.link.error.is_empty() {
                self.link.error = "Sharing couldn't start. Check that Python 3 is installed, then try again.".into();
            }
            self.link.stage = "error".into();
        } else if self.link.stage == "ready" {
            // it ended on its own (a crash, killed) with the code still up
            self.link.error = STOPPED.into();
            self.link.stage = "error".into();
        }
    }

    /// The card, as the share feed carries it (null when none)
    pub fn link_feed(&self) -> Value {
        if self.link.mode.is_empty() {
            return Value::Null;
        }
        let l = &self.link;
        json!({"mode": l.mode, "stage": l.stage, "names": l.names, "qr": l.qr, "error": l.error, "busy": l.busy(), "done": l.done, "total": l.total, "count": l.count,
            "installing": self.upd.installing("qr"), "updates": self.upd.running()})
    }

    /// The dev sandbox only: the card in a state, nothing served
    /// (linkshare demo ready|loading|error|busy <qr png>)
    pub fn link_demo(&mut self, stage: &str, qr: &str) {
        if !tessera_core::env::sandboxed() {
            return;
        }
        let l = &mut self.link;
        l.mode = "send".into();
        l.names = vec!["Holiday.jpg".into()];
        l.qr = qr.into();
        l.error = if stage == "error" { "Making the code needs qrencode.".into() } else { String::new() };
        l.count = if stage == "busy" { 1 } else { 0 };
        l.total = if stage == "busy" { 100.0 } else { 0.0 };
        l.done = if stage == "busy" { 40.0 } else { 0.0 };
        l.stage = if stage == "busy" { "ready".into() } else { stage.into() };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_code_that_stops_on_its_own_is_not_left_up() {
        let mut s = Sharing::default();
        s.link.mode = "send".into();
        s.link.stage = "ready".into();
        s.link_exit(s.link.job.gen);
        assert_eq!(s.link.stage, "error");
        assert_eq!(s.link.error, STOPPED);
        // (and the firewall's 'links' set is no longer needed)
        assert!(!s.ports_need().0.contains("links"));

        let mut s = Sharing::default();
        s.link.mode = "receive".into();
        s.link.stage = "ready".into();
        let gen = s.link.job.gen;
        s.link_line(gen, r#"{"ev": "closed", "why": "closed"}"#);
        assert_eq!(s.link.stage, "error");
        assert_eq!(s.link.error, STOPPED);
        s.link.stage = "ready".into();
        s.link_line(gen, r#"{"ev": "closed", "why": "expired"}"#);
        assert!(s.link.error.contains("expired"));
    }
}

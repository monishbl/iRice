//! shell/services/Updates.qml: system updates. What's available (checkupdates,
//! and yay/paru -Qua for the AUR; only when asked) and the system part of the
//! update, without a terminal: assets/scripts/sysupdate.py runs detached
//! (setsid; it keeps going with Settings closed and across shell restarts)
//! and writes its progress to $XDG_RUNTIME_DIR/tessera-sysupdate/, read here.
//! In the dev sandbox the update is simulated (`--fake`).
//!
//! A plain model owned by the About service (bridge_about.rs), which routes
//! its ticks and events; `update`, `cancel`... are also its calls
//! (`updates.update [args]`, `updates.cancel`, `updates.dismiss`,
//! `updates.copyLog`, `updates.refresh`, `updates.install`).

use super::helpers::{self, Ticker, Tx};
use crate::util;
use serde_json::{json, Value};
use std::time::Duration;
use tessera_core::host::Host;

pub enum Ev {
    /// the status file, read (None: absent or not JSON)
    Status(Option<Value>),
    /// flock -n on the job's lock: exit code
    Lock(i32),
    /// (reset the buffer, new text, the file's size now)
    Log(bool, String, f64),
    /// `checkupdates` + AUR output, exit code
    Check(String, i32),
}

pub struct Updates {
    pub count: usize,
    pub list: Vec<Value>, // [{ name, from, to }]
    pub aur_count: usize,
    pub aur_list: Vec<Value>,
    pub available: bool,
    pub checking: bool,
    pub fake: bool,
    pub dir: String,
    pub script: String,
    /// sysupdate.py's status: { state: running|done|error|cancelled, phase, text, done,
    /// total, percent, package, upgraded, restart, aur, error, started, finished, steps, then, kind }
    pub job: Option<Value>,
    requested: f64,
    pub interrupted: bool,
    pub log_users: i64,
    pub show_details: bool,
    pub log: String,
    log_size: f64,
    log_busy: bool,
    lock_busy: bool,
    t_poll: Ticker,
    t_log: Ticker,
    t_lock: Ticker,
    lock_first: bool,
}

impl Default for Updates {
    fn default() -> Self {
        let run = std::env::var("XDG_RUNTIME_DIR").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "/tmp".into());
        Self {
            count: 0,
            list: vec![],
            aur_count: 0,
            aur_list: vec![],
            available: false,
            checking: false,
            fake: helpers::env_is("SANDBOX") || std::env::var("TESSERA_FAKE_SYSUPDATE").map(|v| v == "1").unwrap_or(false),
            dir: format!("{run}/tessera-sysupdate"),
            script: util::script("sysupdate.py"),
            job: None,
            requested: 0.0,
            interrupted: false,
            log_users: 0,
            show_details: false,
            log: String::new(),
            log_size: 0.0,
            log_busy: false,
            lock_busy: false,
            t_poll: Ticker::new("upPoll"),
            t_log: Ticker::new("upLog"),
            t_lock: Ticker::new("upLock"),
            lock_first: true,
        }
    }
}

/// `checkupdates` lines "name old -> new" (Updates.check's parse)
pub fn parse_list(s: &str) -> Vec<Value> {
    s.lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>())
        .filter(|f| f.len() >= 4 && f[2] == "->")
        .map(|f| json!({"name": f[0], "from": f[1], "to": f[3]}))
        .collect()
}

impl Updates {
    pub fn total(&self) -> usize {
        self.count + self.aur_count
    }
    pub fn state(&self) -> &str {
        self.job.as_ref().and_then(|j| j["state"].as_str()).unwrap_or("")
    }
    pub fn running(&self) -> bool {
        self.state() == "running" && !self.interrupted
    }
    pub fn cancellable(&self) -> bool {
        let phase = self.job.as_ref().and_then(|j| j["phase"].as_str()).unwrap_or("");
        self.running() && (phase == "starting" || phase == "auth")
    }
    /// -1 when nothing runs
    pub fn progress(&self) -> f64 {
        if self.running() {
            self.job.as_ref().and_then(|j| j["percent"].as_f64()).unwrap_or(0.0) / 100.0
        } else {
            -1.0
        }
    }

    /// Re-arm the timers that depend on state (running, log users)
    pub fn sync_timers(&mut self, tx: &Tx) {
        if self.running() {
            if !self.t_poll.running() {
                self.t_poll.every(tx, 500);
            }
        } else {
            self.t_poll.stop();
        }
        if self.running() && self.log_users > 0 {
            if !self.t_log.running() {
                self.t_log.every(tx, 1500);
            }
        } else {
            self.t_log.stop();
        }
        // Is the job still alive (does it hold its lock)? Once 5 s in, then every 10 s
        if self.state() == "running" {
            if !self.t_lock.running() {
                self.lock_first = true;
                self.t_lock.every(tx, 5000);
            }
        } else {
            self.t_lock.stop();
        }
    }

    pub fn publish(&self) {
        crate::bus::put(
            "updates",
            json!({"running": self.running(), "count": self.count, "aurCount": self.aur_count, "total": self.total(), "checking": self.checking, "available": self.available, "interrupted": self.interrupted, "fake": self.fake}),
        );
    }

    /// Read the status once at start, to re-attach to a run that outlived the last shell
    pub fn start(&mut self, tx: &Tx) {
        self.read_status(tx);
        self.publish();
    }

    fn read_status(&self, tx: &Tx) {
        let (tx, path) = (tx.clone(), format!("{}/status.json", self.dir));
        std::thread::spawn(move || {
            let j = std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str::<Value>(&s).ok());
            tx(Box::new(Ev::Status(j)));
        });
    }

    /// args: sysupdate.py's plan (--system, --snapshot, --ignore, --install, --remove, --aur)
    pub fn update(&mut self, tx: &Tx, args: &[String]) {
        if self.running() {
            return;
        }
        self.interrupted = false;
        self.log.clear();
        self.log_size = 0.0;
        // setsid, in this login session (so pkexec finds Tessera's password dialog
        // through polkit); not tied to the shell
        let mut c: Vec<String> = ["setsid", "-f", "python3", "-I", &self.script, "run"].iter().map(|s| s.to_string()).collect();
        c.extend(args.iter().cloned());
        if self.fake {
            c.push("--fake".into());
        }
        let c: Vec<&str> = c.iter().map(String::as_str).collect();
        tessera_core::services::bridge::run(&c);
        self.requested = helpers::now_s();
        self.job = Some(json!({"state": "running", "phase": "starting", "text": "Starting", "percent": 0, "done": 0, "total": 0, "started": self.requested}));
        self.sync_timers(tx);
        self.publish();
    }

    pub fn cancel(&self) {
        if self.cancellable() {
            tessera_core::services::bridge::run(&["python3", "-I", &self.script, "cancel"]);
        }
    }

    /// Forget a finished run (the Settings card and the island stop showing it)
    pub fn dismiss(&mut self) {
        if !self.running() {
            self.job = None;
        }
    }

    pub fn copy_log(&self) {
        tessera_core::services::bridge::run(&["sh", "-c", "wl-copy < \"$1\"", "sh", &format!("{}/log", self.dir)]);
    }

    pub fn refresh(&mut self, tx: &Tx) {
        if self.checking {
            return;
        }
        self.checking = true;
        let tx = tx.clone();
        std::thread::spawn(move || {
            let o = util::sh("command -v checkupdates >/dev/null || exit 3; checkupdates 2>/dev/null; echo '--aur--'; h=$(command -v yay || command -v paru) && \"$h\" -Qua 2>/dev/null; exit 0", Duration::from_secs(300));
            let (code, out) = o.unwrap_or((3, String::new()));
            tx(Box::new(Ev::Check(out, code)));
        });
    }

    pub fn read_log(&mut self, tx: &Tx) {
        if self.log_busy {
            return;
        }
        self.log_busy = true;
        let (tx, path, off) = (tx.clone(), format!("{}/log", self.dir), self.log_size);
        // from logSize on, up to the size the file has now (a smaller file is a
        // new run: start over)
        std::thread::spawn(move || {
            use std::io::{Read, Seek, SeekFrom};
            let size = std::fs::metadata(&path).map(|m| m.len() as f64).unwrap_or(0.0);
            let (reset, off) = if size < off { (true, 0.0) } else { (false, off) };
            let mut body = String::new();
            if size > off {
                if let Ok(mut f) = std::fs::File::open(&path) {
                    if f.seek(SeekFrom::Start(off as u64)).is_ok() {
                        let mut buf = Vec::new();
                        let _ = f.take((size - off) as u64).read_to_end(&mut buf);
                        body = String::from_utf8_lossy(&buf).into_owned();
                    }
                }
            }
            tx(Box::new(Ev::Log(reset, body, size)));
        });
    }

    pub fn set_log_users(&mut self, tx: &Tx, delta: i64) {
        self.log_users = (self.log_users + delta).max(0);
        if delta > 0 && self.log_users > 0 {
            self.read_log(tx);
        }
        self.sync_timers(tx);
    }

    pub fn tick(&mut self, tx: &Tx, name: &str) {
        match name {
            "upPoll" => self.read_status(tx),
            "upLog" => self.read_log(tx),
            "upLock" => {
                self.t_lock.every(tx, 10000);
                if self.lock_first {
                    self.lock_first = false;
                }
                if !self.lock_busy {
                    self.lock_busy = true;
                    let (tx, lock) = (tx.clone(), format!("{}/lock", self.dir));
                    std::thread::spawn(move || {
                        let c = util::capture(&["flock", "-n", &lock, "true"], Duration::from_secs(5)).map(|(c, _)| c).unwrap_or(-1);
                        tx(Box::new(Ev::Lock(c)));
                    });
                }
            }
            _ => {}
        }
    }

    /// An event of its threads; Some(job) when a run just finished
    pub fn event(&mut self, h: &mut Host, tx: &Tx, ev: Ev) -> Option<Value> {
        let mut finished = None;
        match ev {
            Ev::Status(j) => {
                let Some(j) = j else { return None };
                // The last run's file, read before the new run wrote its own
                if self.requested > 0.0 && j["started"].as_f64().unwrap_or(0.0) < self.requested - 2.0 {
                    return None;
                }
                // A run that finished long ago (before this shell) isn't news
                if self.job.is_none() && j["state"] != "running" && helpers::now_s() - j["finished"].as_f64().unwrap_or(0.0) > 3600.0 {
                    return None;
                }
                let was = self.state().to_string();
                let now_state = j["state"].as_str().unwrap_or("").to_string();
                self.job = Some(j.clone());
                if now_state != "running" && was == "running" {
                    h.ask("tools.recheck", json!([])); // (packages may have come or gone)
                    finished = Some(j);
                }
            }
            Ev::Lock(code) => {
                self.lock_busy = false;
                // A "running" status nobody holds the lock for: the job died (machine
                // went down, killed); say so instead of showing progress forever
                if code == 0 && self.state() == "running" {
                    // (just started: the job may not have taken the lock yet)
                    if helpers::now_s() - self.job.as_ref().and_then(|j| j["started"].as_f64()).unwrap_or(0.0) > 5.0 {
                        self.interrupted = true;
                    }
                }
            }
            Ev::Log(reset, body, size) => {
                self.log_busy = false;
                let base = if reset { String::new() } else { std::mem::take(&mut self.log) };
                let joined = format!("{base}{body}");
                let all: Vec<&str> = joined.split('\n').collect();
                let keep = if all.len() > 2000 { &all[all.len() - 2000..] } else { &all[..] };
                self.log = keep.join("\n");
                self.log_size = size;
            }
            Ev::Check(out, code) => {
                let (repo, aur) = (format!("{out}\n")).split_once("--aur--\n").map(|(a, b)| (a.to_string(), b.to_string())).unwrap_or((out.clone(), String::new()));
                self.list = parse_list(&repo);
                self.count = self.list.len();
                self.aur_list = parse_list(&aur);
                self.aur_count = self.aur_list.len();
                self.available = code != 3;
                self.checking = false;
            }
        }
        self.sync_timers(tx);
        self.publish();
        if let Some(j) = &finished {
            self.finished_alert(h, j);
        }
        finished
    }

    /// Done: an alert in the island (a notification without it), and a fresh check
    fn finished_alert(&mut self, h: &mut Host, j: &Value) {
        // (the caller refreshes the lists: check.running = true)
        if j["state"] == "cancelled" {
            return;
        }
        let ok = j["state"] == "done";
        // Tessera's part follows (its update screen): no alert in between
        if ok && (helpers::truthy(&j["then"]["tessera"]) || j["then"]["rebuild"].as_array().is_some_and(|r| !r.is_empty())) {
            return;
        }
        let n = j["upgraded"].as_array().map(|a| a.len()).unwrap_or(0);
        let back = j["kind"] == "undo";
        let text = j["text"].as_str().unwrap_or("").to_string();
        let restart = j["restart"].as_bool().unwrap_or(false);
        let title = if ok {
            if back {
                "Back to the earlier versions".to_string()
            } else if n > 0 {
                format!("{n} update{} installed", if n == 1 { "" } else { "s" })
            } else {
                text.clone()
            }
        } else {
            text.clone()
        };
        let sub = if ok {
            if restart {
                "Restart to finish: the kernel or system libraries changed".to_string()
            } else if back {
                if n == 1 { "1 package went back".to_string() } else { format!("{n} packages went back") }
            } else {
                "Your system is up to date".to_string()
            }
        } else {
            let e = j["error"].as_str().unwrap_or("");
            let e = if e.is_empty() { "See Software Update for details" } else { e };
            e.split('\n').next_back().unwrap_or("").to_string()
        };
        if helpers::island_enabled(h) && helpers::island_wants(h, "sysupdate") {
            let act = if ok && restart { json!({"call": "about.reboot", "args": []}) } else { json!({"call": "panels.openSettings", "args": ["update"]}) };
            helpers::island_post(
                h,
                json!({
                    "kind": "sysupdate",
                    "icon": if ok { if restart { "arrow-clockwise" } else { "check-circle" } } else { "warning-circle" },
                    "title": title, "subtitle": sub,
                    "color": if ok { "#30D158" } else { "#FF453A" },
                    "action": if ok && restart { "Restart" } else { "Details" },
                    "onAction": act,
                }),
                8000,
            );
        } else {
            helpers::notify_send(&["-a", "Tessera", "-i", if ok { "system-software-update" } else { "dialog-error" }, &title, &sub]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lists() {
        let l = parse_list("hyprland 0.55.0-1 -> 0.56.2-1\nbad line\nfoo 1 -> 2\n");
        assert_eq!(l.len(), 2);
        assert_eq!(l[0]["to"], "0.56.2-1");
    }
}

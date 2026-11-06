//! Downloads and file copies in progress, for the island's live activity
//! (shell/services/Transfers.qml):
//!   downloads  browsers' partial files in the Downloads folder (.part,
//!              .crdownload): the folder is watched (inotify), and their sizes
//!              are read once a second while one is growing, every 10 s while
//!              they only sit there. Browsers don't say how big the file will
//!              be, so these show size and speed. One that stops growing isn't
//!              shown, and when one finishes the island says so
//!              (download_tracker.rs decides).
//!   copies     cp / mv / rsync / dd and friends, with how far through the
//!              current file they are (assets/scripts/transfers.sh)
//! (Copies made inside a file manager's own process can't be seen.)
//! The items, with LocalSend's and Quick Share's, are published as the shell
//! state `transfers` for the island's live activity.

use crate::download_tracker as tracker;
use crate::localsend::js;
use crate::proc::Keep;
use crate::{act, base, Job, Sharing, GREEN};
use serde_json::{json, Value};
use std::collections::HashMap;
use tessera_core::host::Host;

pub struct Transfers {
    pub downloads: Vec<Value>,
    pub tracked: HashMap<String, tracker::Seen>,
    pub finishing: Vec<tracker::Gone>,
    pub copies: Vec<Value>,
    /// pid -> (pos, at)
    pub last_copy: HashMap<i64, (f64, i64)>,
    pub watch: Keep,
    pub watching: bool,
    pub looking: bool,
}

impl Default for Transfers {
    fn default() -> Self {
        Self { downloads: vec![], tracked: HashMap::new(), finishing: vec![], copies: vec![], last_copy: HashMap::new(), watch: Keep::new("copies", "copies_rest"), watching: false, looking: false }
    }
}

/// "1.2 MB", "340 KB" (powers of 1000)
pub fn size_text(bytes: f64) -> String {
    let u = ["B", "KB", "MB", "GB", "TB"];
    let mut i = 0;
    let mut v = bytes;
    while v >= 1000.0 && i < u.len() - 1 {
        v /= 1000.0;
        i += 1;
    }
    if i == 0 {
        format!("{} {}", v, u[i])
    } else {
        format!("{:.*} {}", if v < 10.0 { 1 } else { 0 }, v, u[i])
    }
}

/// "45%" when the size is known, else the size so far
pub fn label(item: &Value) -> String {
    let total = item["total"].as_f64().unwrap_or(0.0);
    let done = item["done"].as_f64().unwrap_or(0.0);
    if total > 0.0 {
        format!("{}%", (100.0 * done / total).round())
    } else {
        size_text(done)
    }
}

impl Sharing {
    fn downloads_dir() -> String {
        std::env::var("XDG_DOWNLOAD_DIR").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| format!("{}/Downloads", std::env::var("HOME").unwrap_or_default()))
    }
    /// Island.enabled && Island.wants("transfers")
    fn transfers_on(&self) -> bool {
        let events: Vec<String> = self.cfg_list("island.events");
        let known: Vec<String> = self.cfg_list("island.known");
        self.cfg_bool("island.enabled", true) && (events.iter().any(|e| e == "transfers") || !known.iter().any(|e| e == "transfers"))
    }

    /// [{ id, kind: "download" | "copy" | "send" | "receive", name, done, total (0 = unknown), speed, pid? }]
    pub fn transfer_items(&self) -> Vec<Value> {
        let mut v = self.tr.downloads.clone();
        v.extend(self.tr.copies.clone());
        v.extend(self.local.active.values());
        v.extend(self.quick.active.values());
        v
    }
    /// Overall 0..1 when every item's size is known, else -1
    pub fn transfer_progress(&self) -> f64 {
        let items = self.transfer_items();
        if items.is_empty() || items.iter().any(|i| i["total"].as_f64().unwrap_or(0.0) <= 0.0) {
            return -1.0;
        }
        let done: f64 = items.iter().map(|i| i["done"].as_f64().unwrap_or(0.0)).sum();
        let total: f64 = items.iter().map(|i| i["total"].as_f64().unwrap_or(0.0)).sum();
        if total > 0.0 { (done / total).min(1.0) } else { -1.0 }
    }
    /// Stop a copy (downloads are the browser's to cancel)
    pub fn transfer_cancel(&mut self, item: &Value) {
        let kind = item["kind"].as_str().unwrap_or("");
        if kind == "send" || kind == "receive" {
            let id = js(&item["id"]);
            if item["via"] == "quickshare" {
                self.quick_cancel(&id);
            } else {
                self.local_cancel(&id);
            }
        } else if kind == "copy" && item["pid"].as_i64().unwrap_or(0) > 0 && !item["start"].is_null() {
            let script = self.script("process-identity.py");
            crate::proc::detach(&["python3", "-I", &script, "interrupt", &item["pid"].to_string(), &js(&item["start"])]);
        }
    }
    pub fn open_downloads(&self) {
        self.open_folder(&Self::downloads_dir());
    }

    // ── Downloads ───────────────────────────────────────────────────────
    pub fn transfers_start(&mut self, h: &mut Host) {
        let dir = Self::downloads_dir();
        if self.tr.watching || !std::path::Path::new(&dir).is_dir() {
            return;
        }
        self.tr.watching = true;
        let to = h.sender("Sharing");
        std::thread::spawn(move || unsafe {
            let fd = libc::inotify_init1(0);
            if fd < 0 {
                return;
            }
            let Ok(c) = std::ffi::CString::new(dir) else { return };
            let wd = libc::inotify_add_watch(fd, c.as_ptr(), libc::IN_CREATE | libc::IN_DELETE | libc::IN_MOVED_FROM | libc::IN_MOVED_TO);
            if wd < 0 {
                return;
            }
            let mut buf = [0u8; 4096];
            loop {
                let n = libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len());
                if n <= 0 {
                    break;
                }
                to(Box::new(crate::M::DlEvent));
            }
        });
        self.sched.set("dl_look", 100);
    }
    pub fn transfers_sync(&mut self, h: &mut Host) {
        let on = self.transfers_on();
        if on != self.tr.looking {
            self.tr.looking = on;
            // (turned off: what is shown goes; turned on: a look)
            self.sched.set("dl_look", 50);
        }
        let cmd = vec!["bash".to_string(), self.script("transfers.sh")];
        self.tr.watch.sync(&mut self.sched, h.sender("Sharing"), on, &cmd, 5000, true);
    }
    pub fn dl_look(&mut self, h: &mut Host) {
        let dir = Self::downloads_dir();
        // Every second while something downloads: find, not a python3 start
        // each time. "size<TAB>path", NUL-separated (any file name is safe)
        let cmd = crate::proc::s(&["find", &dir, "-maxdepth", "1", "-type", "f", "(", "-name", "*.part", "-o", "-name", "*.crdownload", "-o", "-name", "*.download", ")", "-printf", "%s\\t%p\\0"]);
        self.run(h, Job::DlSizes, &cmd, None);
    }
    pub fn dl_sizes(&mut self, h: &mut Host, text: &str) {
        let mut entries: Vec<(String, f64)> = Vec::new();
        for e in text.split('\0') {
            if let Some((b, p)) = e.split_once('\t') {
                if let Ok(n) = b.parse::<f64>() {
                    entries.push((p.to_string(), n));
                }
            }
        }
        let on = self.transfers_on();
        let seen = entries.clone();
        let (tracked, items, gone) = tracker::update(&self.tr.tracked, if on { &entries } else { &[] }, tessera_core::util::now_ms());
        self.tr.tracked = tracked;
        self.tr.downloads = items;
        if !gone.is_empty() && on {
            self.tr.finishing.extend(gone);
            let dir = Self::downloads_dir();
            let cmd = crate::proc::s(&["find", &dir, "-maxdepth", "1", "-type", "f", "-mmin", "-2", "-printf", "%s\\t%T@\\t%p\\0"]);
            self.run(h, Job::DlRecent, &cmd, None);
        }
        // (while partial files sit there: again, every second if one grows, else every 10 s)
        if on && !seen.is_empty() {
            self.sched.set("dl_look", if self.tr.downloads.is_empty() { 10_000 } else { 1000 });
        } else if !self.tr.tracked.is_empty() {
            self.sched.set("dl_look", 1000); // (one more look, so a finished download is seen to finish)
        }
    }
    /// Downloads that went away after growing: finished (renamed to their
    /// real name) or cancelled (deleted). The files changed in the last minute
    /// say which; a finished one gets "Downloaded <name>".
    pub fn dl_recent(&mut self, h: &mut Host, text: &str) {
        let mut files: Vec<tracker::File> = Vec::new();
        for e in text.split('\0') {
            let f: Vec<&str> = e.splitn(3, '\t').collect();
            if f.len() >= 3 {
                files.push(tracker::File { bytes: f[0].parse().unwrap_or(0.0), mtime: (f[1].parse::<f64>().unwrap_or(0.0) * 1000.0) as i64, path: f[2].to_string() });
            }
        }
        let done = std::mem::take(&mut self.tr.finishing);
        for g in done {
            let path = tracker::final_file(&g, &files);
            if !path.is_empty() {
                self.dl_announce(h, &path);
            }
        }
    }
    fn dl_announce(&mut self, h: &mut Host, path: &str) {
        let dir = Self::downloads_dir();
        let home = std::env::var("HOME").unwrap_or_default();
        let shown = if home.is_empty() { dir } else { dir.replacen(&home, "~", 1) };
        self.island(
            h,
            json!({"kind": "transfer", "icon": "download-simple", "color": GREEN, "title": format!("Downloaded {}", base(path)), "subtitle": format!("In {shown}"),
                "action": "Open", "onAction": act("share.fileOpen", json!([path])), "action2": "Show in Folder", "onAction2": act("share.fileReveal", json!([path]))}),
            600_000, // up for 10 minutes (or until dismissed)
        );
    }

    // ── Copies ──────────────────────────────────────────────────────────
    /// [{ pid, start, cmd, file, pos, size }]
    pub fn copies_line(&mut self, gen: u64, line: &str) {
        if !self.tr.watch.is(gen) {
            return;
        }
        let Some(list) = crate::proc::json_line(line) else { return };
        let Some(list) = list.as_array() else { return };
        let now = tessera_core::util::now_ms();
        let mut seen = HashMap::new();
        let mut out = Vec::new();
        for c in list {
            let pid = c["pid"].as_i64().unwrap_or(0);
            let pos = c["pos"].as_f64().unwrap_or(0.0);
            let speed = match self.tr.last_copy.get(&pid) {
                Some((lp, la)) if now > *la && pos >= *lp => (pos - lp) * 1000.0 / (now - la) as f64,
                _ => 0.0,
            };
            seen.insert(pid, (pos, now));
            out.push(json!({"id": format!("copy{pid}"), "kind": "copy", "name": base(&js(&c["file"])), "done": pos, "total": c["size"].as_f64().unwrap_or(0.0), "speed": speed, "pid": pid, "start": c["start"], "cmd": c["cmd"]}));
        }
        self.tr.copies = out;
        self.tr.last_copy = seen;
    }
    pub fn copies_exit(&mut self, gen: u64) {
        let on = self.transfers_on();
        self.tr.watch.ended(&mut self.sched, gen, on, 5000);
    }

    pub fn transfers_state(&self) -> Value {
        let items = self.transfer_items();
        json!({"items": items, "active": !items.is_empty(), "first": items.first(), "progress": self.transfer_progress(),
            "speed": items.iter().map(|i| i["speed"].as_f64().unwrap_or(0.0)).sum::<f64>()})
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_read_in_powers_of_a_thousand() {
        assert_eq!(size_text(0.0), "0 B");
        assert_eq!(size_text(999.0), "999 B");
        assert_eq!(size_text(1500.0), "1.5 KB");
        assert_eq!(size_text(12_400_000.0), "12 MB");
        assert_eq!(size_text(6_200_000.0), "6.2 MB");
    }

    #[test]
    fn a_label_is_a_percentage_or_a_size() {
        assert_eq!(label(&json!({"done": 50, "total": 200})), "25%");
        assert_eq!(label(&json!({"done": 2000, "total": 0})), "2.0 KB");
    }
}

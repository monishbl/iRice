//! Browsers' partial downloads in the Downloads folder, followed from one
//! look to the next (shell/services/DownloadTracker.js; used by transfers.rs).
//!
//! What a look sees: (path, bytes) for every *.part / *.crdownload /
//! *.download file. From that:
//!   - a file that hasn't grown for a while (a paused or abandoned download,
//!     or Chromium's empty "Unconfirmed 1234.crdownload" waiting behind its
//!     save dialog) isn't shown: a card reading "Downloading 0 B" forever
//!     helps nobody
//!   - a file that went away after it had grown is a download that finished
//!     (renamed to its real name) or was cancelled (deleted): reported in
//!     `gone`, and the caller looks for the finished file to say so
//!   - Chromium names a download "Unconfirmed 1234" until the save dialog is
//!     answered: shown as "Download" until it has its real name

use serde_json::{json, Value};
use std::collections::HashMap;

/// A download that hasn't grown for this long isn't shown
pub const STALL_MS: i64 = 20000;

#[derive(Clone, Debug, PartialEq)]
pub struct Seen {
    pub bytes: f64,
    pub at: i64,
    pub changed_at: i64,
    pub grew: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Gone {
    pub path: String,
    pub name: String,
    pub unconfirmed: bool,
    pub bytes: f64,
    pub at: i64,
}

pub struct File {
    pub path: String,
    pub bytes: f64,
    /// ms
    pub mtime: i64,
}

fn is_partial(path: &str) -> bool {
    path.ends_with(".part") || path.ends_with(".crdownload") || path.ends_with(".download")
}

pub fn partial_name(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    let name = name.strip_prefix('.').unwrap_or(name);
    for ext in [".part", ".crdownload", ".download"] {
        if let Some(n) = name.strip_suffix(ext) {
            return n.to_string();
        }
    }
    name.to_string()
}
pub fn is_unconfirmed(name: &str) -> bool {
    name.strip_prefix("Unconfirmed ").is_some_and(|d| !d.is_empty() && d.chars().all(|c| c.is_ascii_digit()))
}
pub fn display_name(path: &str) -> String {
    let n = partial_name(path);
    if is_unconfirmed(&n) {
        "Download".into()
    } else {
        n
    }
}

/// tracked: path -> last look; entries: this look; now: ms
/// -> (tracked, items (to show), gone (went away after growing))
pub fn update(tracked: &HashMap<String, Seen>, entries: &[(String, f64)], now: i64) -> (HashMap<String, Seen>, Vec<Value>, Vec<Gone>) {
    let mut next: HashMap<String, Seen> = HashMap::new();
    let mut items = Vec::new();
    for (path, bytes) in entries {
        let last = tracked.get(path);
        let grew = last.is_some_and(|l| *bytes > l.bytes);
        let t = Seen {
            bytes: *bytes,
            at: now,
            changed_at: if last.is_none() || grew { now } else { last.map(|l| l.changed_at).unwrap_or(now) },
            grew: last.is_some_and(|l| l.grew) || grew || (last.is_none() && *bytes > 0.0),
        };
        let speed = match last {
            Some(l) if now > l.at => ((*bytes - l.bytes) * 1000.0 / (now - l.at) as f64).max(0.0),
            _ => 0.0,
        };
        if *bytes > 0.0 && now - t.changed_at < STALL_MS {
            items.push(json!({"id": path, "kind": "download", "name": display_name(path), "done": bytes, "total": 0, "speed": speed}));
        }
        next.insert(path.clone(), t);
    }
    let mut gone = Vec::new();
    for (path, old) in tracked {
        if next.contains_key(path) {
            continue;
        }
        if old.grew && old.bytes > 0.0 {
            let name = partial_name(path);
            gone.push(Gone { path: path.clone(), unconfirmed: is_unconfirmed(&name), name, bytes: old.bytes, at: old.changed_at });
        }
    }
    (next, items, gone)
}

/// The finished file for a download that went away: the same name without the
/// partial suffix, else (Chromium's "Unconfirmed" ones, or a browser that
/// renamed it) the newest file changed since it was last seen growing that's
/// at least as big. "" when nothing fits (it was cancelled).
pub fn final_file(g: &Gone, files: &[File]) -> String {
    let dir = g.path.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
    let mut best: Option<&File> = None;
    for f in files {
        if is_partial(&f.path) {
            continue;
        }
        if !g.unconfirmed && f.path == format!("{dir}/{}", g.name) {
            return f.path.clone();
        }
        if f.bytes >= g.bytes && f.mtime >= g.at - 2000 && best.map_or(true, |b| f.mtime > b.mtime) {
            best = Some(f);
        }
    }
    best.map(|b| b.path.clone()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_lose_the_partial_suffix() {
        assert_eq!(partial_name("/d/.report.pdf.part"), "report.pdf");
        assert_eq!(display_name("/d/Unconfirmed 123.crdownload"), "Download");
        assert_eq!(display_name("/d/movie.mkv.crdownload"), "movie.mkv");
    }

    #[test]
    fn a_growing_file_shows_and_a_stalled_one_does_not() {
        let (t1, items1, _) = update(&HashMap::new(), &[("/d/a.part".into(), 100.0)], 1000);
        assert_eq!(items1.len(), 1);
        let (t2, items2, _) = update(&t1, &[("/d/a.part".into(), 300.0)], 2000);
        assert_eq!(items2[0]["speed"], 200.0);
        let (_, items3, _) = update(&t2, &[("/d/a.part".into(), 300.0)], 2000 + STALL_MS + 1);
        assert!(items3.is_empty());
    }

    #[test]
    fn a_file_that_went_after_growing_is_reported() {
        let (t1, _, _) = update(&HashMap::new(), &[("/d/a.zip.part".into(), 500.0)], 1000);
        let (_, _, gone) = update(&t1, &[], 3000);
        assert_eq!(gone.len(), 1);
        assert_eq!(gone[0].name, "a.zip");
        let files = vec![File { path: "/d/a.zip".into(), bytes: 500.0, mtime: 2500 }];
        assert_eq!(final_file(&gone[0], &files), "/d/a.zip");
        assert_eq!(final_file(&gone[0], &[]), "");
    }

    #[test]
    fn unconfirmed_ones_find_the_newest_big_enough_file() {
        let g = Gone { path: "/d/Unconfirmed 9.crdownload".into(), name: "Unconfirmed 9".into(), unconfirmed: true, bytes: 100.0, at: 5000 };
        let files = vec![
            File { path: "/d/old.bin".into(), bytes: 900.0, mtime: 1000 },
            File { path: "/d/new.bin".into(), bytes: 120.0, mtime: 6000 },
            File { path: "/d/newer.part".into(), bytes: 120.0, mtime: 7000 },
        ];
        assert_eq!(final_file(&g, &files), "/d/new.bin");
    }
}

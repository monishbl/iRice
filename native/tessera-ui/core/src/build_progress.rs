//! A build's progress, as tools/build-progress.py writes it
//! ($XDG_STATE_HOME/tessera/build-progress.json, and the whole output in
//! build.log): Software Update and the update screen show it while the
//! shell, tessera-events or the glass plugin is built (by the updater, by
//! `tessera rebuild`, by the installer).
//!
//! The file's fields: phase (prepare | download | compile | link | done |
//! failed), label, title ("the shell"), kind (cargo | make), step (in
//! words: "Compiling slint (212 of 431)"), done, total, exact, fraction
//! (0…1 or null: nothing honest to measure against), crate, elapsed_s,
//! eta_s (null: no guess), log, started_at, updated_at, pid, warnings,
//! error (the first error line), tail (on failure: the compiler's last lines).

use serde_json::Value;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// A running build that hasn't said anything for this long is gone (killed
/// with its terminal, the machine slept): not shown as running
const STALE_S: f64 = 30.0;
/// A failed build is shown this long after (until dismissed)
const FAILED_FOR_S: f64 = 30.0 * 60.0;

pub fn dir() -> PathBuf {
    crate::util::xdg("XDG_STATE_HOME", ".local/state").join("tessera")
}
pub fn path() -> PathBuf {
    dir().join("build-progress.json")
}
pub fn log_path() -> PathBuf {
    dir().join("build.log")
}

/// What to show of a build
#[derive(Clone, Debug, PartialEq, Default)]
pub struct View {
    /// running (not done, not failed, still writing)
    pub active: bool,
    pub failed: bool,
    /// "Building the shell", "Linking the shell", "The shell didn't build"
    pub title: String,
    /// "Compiling slint (212 of 431)", or the error on failure
    pub step: String,
    /// "3:12 · about 4 min left"
    pub time: String,
    /// 0…1, None: indeterminate
    pub fraction: Option<f32>,
    /// started_at: which build this is (a dismissed failure stays dismissed)
    pub id: f64,
    pub log: String,
    /// the compiler's last lines, on failure
    pub tail: Vec<String>,
}

fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}

/// m:ss (h:mm:ss from an hour)
pub fn clock(secs: f64) -> String {
    let t = secs.max(0.0) as u64;
    if t >= 3600 {
        format!("{}:{:02}:{:02}", t / 3600, t / 60 % 60, t % 60)
    } else {
        format!("{}:{:02}", t / 60, t % 60)
    }
}

/// "about 4 min left", "less than a minute left"
pub fn left(secs: f64) -> String {
    if secs < 60.0 {
        "less than a minute left".into()
    } else {
        format!("about {} min left", (secs / 60.0).round() as u64)
    }
}

fn alive(pid: i64) -> bool {
    pid > 0 && Path::new(&format!("/proc/{pid}")).exists()
}

/// A sentence's start ("The shell"); a program's name (tessera-events) as spelled
fn cap(t: &str) -> String {
    if t.split(' ').next().is_some_and(|w| w.contains('-')) {
        return t.to_string();
    }
    let mut c = t.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// The view of a snapshot at `now` (seconds since the epoch); None: nothing
/// to show (done, gone, an old failure, or not a snapshot). `pid_alive`:
/// whether the build's process is still there.
pub fn view(snap: &Value, now: f64, pid_alive: bool) -> Option<View> {
    let phase = s(&snap["phase"]);
    if phase.is_empty() {
        return None;
    }
    let updated = snap["updated_at"].as_f64().unwrap_or(0.0);
    let since = (now - updated).max(0.0);
    let title = if s(&snap["title"]).is_empty() { s(&snap["label"]).to_string() } else { s(&snap["title"]).to_string() };
    let mut v = View { id: snap["started_at"].as_f64().unwrap_or(0.0), log: s(&snap["log"]).to_string(), ..Default::default() };
    match phase {
        "failed" => {
            if since > FAILED_FOR_S {
                return None;
            }
            v.failed = true;
            v.title = format!("{} didn't build", cap(&title));
            v.step = if s(&snap["error"]).is_empty() { "The log below says why.".into() } else { s(&snap["error"]).to_string() };
            v.time = format!("Stopped after {}", clock(snap["elapsed_s"].as_f64().unwrap_or(0.0)));
            v.tail = snap["tail"].as_array().map(|a| a.iter().map(|l| s(l).to_string()).collect()).unwrap_or_default();
        }
        "prepare" | "download" | "compile" | "link" => {
            if since > STALE_S || !pid_alive {
                return None;
            }
            v.active = true;
            v.title = if phase == "link" { format!("Linking {title}") } else { format!("Building {title}") };
            v.step = s(&snap["step"]).to_string();
            // (the times as of now, not as of the last write)
            let mut t = vec![clock(snap["elapsed_s"].as_f64().unwrap_or(0.0) + since)];
            if let Some(eta) = snap["eta_s"].as_f64() {
                t.push(left((eta - since).max(0.0)));
            }
            v.time = t.join(" · ");
            v.fraction = snap["fraction"].as_f64().map(|f| f.clamp(0.0, 1.0) as f32);
        }
        _ => return None,
    }
    Some(v)
}

/// The build in progress (or just failed), from the file
pub fn read() -> Option<View> {
    let text = std::fs::read_to_string(path()).ok()?;
    let snap: Value = serde_json::from_str(&text).ok()?;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0);
    view(&snap, now, alive(snap["pid"].as_i64().unwrap_or(0)))
}

/// The last `lines` lines of a log (read from its end: logs get long)
pub fn tail(path: &Path, lines: usize) -> String {
    let Ok(mut f) = std::fs::File::open(path) else { return String::new() };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let want = (lines as u64 * 240).max(16 * 1024);
    let start = len.saturating_sub(want);
    if f.seek(SeekFrom::Start(start)).is_err() {
        return String::new();
    }
    let mut buf = Vec::new();
    if f.read_to_end(&mut buf).is_err() {
        return String::new();
    }
    tail_of(&String::from_utf8_lossy(&buf), lines, start > 0)
}

/// The last `lines` lines of `text` (`cut`: it starts mid-line, drop that one).
/// Progress bookkeeping lines (BUILD {…}, PROGRESS n …, @@step) aren't output.
pub fn tail_of(text: &str, lines: usize, cut: bool) -> String {
    let mut all: Vec<&str> = text.split('\n').collect();
    if cut && !all.is_empty() {
        all.remove(0);
    }
    while all.last().is_some_and(|l| l.is_empty()) {
        all.pop();
    }
    let keep: Vec<&str> = all.into_iter().filter(|l| !l.starts_with("BUILD {") && !l.starts_with("PROGRESS ") && !l.starts_with("@@step ")).collect();
    keep[keep.len().saturating_sub(lines)..].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn snap(phase: &str) -> Value {
        json!({"phase": phase, "label": "tessera-ui", "title": "the shell", "kind": "cargo", "step": "Compiling slint (212 of 431)",
               "done": 212, "total": 431, "exact": true, "fraction": 0.49, "elapsed_s": 192.0, "eta_s": 250.0,
               "started_at": 1000.0, "updated_at": 1200.0, "pid": 1, "error": "error[E0308]: mismatched types", "tail": ["a", "b"], "log": "/x/build.log"})
    }

    #[test]
    fn a_running_build() {
        let v = view(&snap("compile"), 1202.0, true).unwrap();
        assert!(v.active && !v.failed);
        assert_eq!(v.title, "Building the shell");
        assert_eq!(v.step, "Compiling slint (212 of 431)");
        // (the times move on from the last write: 2 s later)
        assert_eq!(v.time, "3:14 · about 4 min left");
        assert_eq!(v.fraction, Some(0.49));
        assert_eq!(view(&snap("link"), 1200.0, true).unwrap().title, "Linking the shell");
    }

    #[test]
    fn no_total_no_fraction_no_eta() {
        let mut s = snap("compile");
        s["fraction"] = Value::Null;
        s["eta_s"] = Value::Null;
        let v = view(&s, 1200.0, true).unwrap();
        assert_eq!((v.fraction, v.time.as_str()), (None, "3:12"));
    }

    #[test]
    fn gone_done_or_stale_shows_nothing() {
        assert!(view(&snap("compile"), 1200.0, false).is_none());
        assert!(view(&snap("compile"), 1300.0, true).is_none());
        assert!(view(&snap("done"), 1200.0, true).is_none());
        assert!(view(&json!({}), 1200.0, true).is_none());
    }

    #[test]
    fn a_failure_says_why_for_a_while() {
        let v = view(&snap("failed"), 1300.0, false).unwrap();
        assert!(v.failed && !v.active);
        assert_eq!(v.title, "The shell didn't build");
        assert_eq!(v.step, "error[E0308]: mismatched types");
        assert_eq!(v.tail, vec!["a", "b"]);
        assert!(view(&snap("failed"), 1200.0 + 3600.0, false).is_none());
    }

    #[test]
    fn times_in_words() {
        assert_eq!(clock(59.0), "0:59");
        assert_eq!(clock(3725.0), "1:02:05");
        assert_eq!(left(30.0), "less than a minute left");
        assert_eq!(left(150.0), "about 3 min left");
    }

    #[test]
    fn the_end_of_a_log() {
        assert_eq!(tail_of("a\nb\nBUILD {\"x\":1}\nc\n", 2, false), "b\nc");
        assert_eq!(tail_of("rtial\nwhole\n", 5, true), "whole");
        assert_eq!(tail_of("", 5, false), "");
    }
}

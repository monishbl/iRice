//! Settings → Appearance's "Glass effect is off" row, the shell's side: what
//! is known of the tessera-glass plugin's build (the state `glassBuild`, in
//! Settings' look feed), and its Build glass button, which builds it inside
//! the shell, not in a terminal: tools/build-plugin.sh --load, detached
//! (setsid), from the checkout tessera_core::env::root() finds, else the
//! installed copy of its sources ($XDG_DATA_HOME/tessera/src). The script runs
//! through tools/build-progress.py, so how far it is, and why it failed, are
//! in $XDG_STATE_HOME/tessera/build-progress.json (and its output in
//! build.log); a thread here reads that, the plugin's built-for file and the
//! running Hyprland's version, and says when they change. When the build ends
//! the plugin is pinged at once (the row goes when it answers).
//!
//! State `glassBuild`: running, headers (Hyprland's version, its headers'),
//! builtFor, built (the plugin file is there), build {active, failed,
//! fraction, time, error, log}, other (another build is running), starting
//! (Build was just pressed), skipped (pressed in a sandbox: never run there),
//! fake (a sandbox's made-up state, from $XDG_RUNTIME_DIR/tessera-fake-glass,
//! for screenshots of each state: missing, updated, outdated, unloaded,
//! failed, headers, building, ok).
//!
//! Calls: glassbuild.start, glassbuild.log (opens build.log).

use crate::common::*;
use serde_json::{json, Value};
use std::any::Any;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;
use tessera_core::host::{Host, Service};

/// The progress file's label for the plugin's builds (build-plugin.sh's)
const LABEL: &str = "tessera-glass";

enum Msg {
    /// what the thread read (changed since last time)
    Read(Value),
}

#[derive(Default)]
pub struct GlassBuild {
    /// when Build was pressed (ms), until its build shows in the progress file
    starting: Option<i64>,
    skipped: bool,
    /// the last reading, and whether its build was running
    read: Value,
    was_active: bool,
    /// wakes the reader at once (and makes it read every second for a while)
    wake: Option<mpsc::Sender<()>>,
}

/// The X.Y.Z of a version string ("v0.57.2-b", "Hyprland 0.57.2 built from …")
pub fn xyz(v: &str) -> String {
    let start = v.find(|c: char| c.is_ascii_digit()).unwrap_or(v.len());
    let rest = &v[start..];
    let end = rest.find(|c: char| !(c.is_ascii_digit() || c == '.')).unwrap_or(rest.len());
    rest[..end].trim_end_matches('.').to_string()
}

/// The running Hyprland's version (`hyprctl version -j`: "version", else "tag")
fn running_version() -> String {
    if let Some(j) = capture(&["hyprctl", "version", "-j"]).and_then(|t| serde_json::from_str::<Value>(&t).ok()) {
        for k in ["version", "tag"] {
            let v = xyz(j[k].as_str().unwrap_or(""));
            if !v.is_empty() {
                return v;
            }
        }
    }
    capture(&["hyprctl", "version"]).map(|t| xyz(t.lines().next().unwrap_or(""))).unwrap_or_default()
}

/// The installed headers' version (what a build now would be for)
fn headers_version() -> String {
    capture(&["pkg-config", "--modversion", "hyprland"]).map(|t| xyz(&t)).unwrap_or_default()
}

/// Where the build runs from: the checkout (root()) when it has the plugin's
/// sources and the script, else the installed copy of them
pub fn source_dir(root: PathBuf, data: PathBuf) -> PathBuf {
    let has = |d: &PathBuf| d.join("tools/build-plugin.sh").is_file() && d.join("hypr/plugin/tessera-glass/Makefile").is_file();
    if has(&root) {
        root
    } else {
        data.join("src")
    }
}

/// The plugin files this session's Hyprland config may load (hypr/tessera/glass.lua's
/// candidates: a console session from a checkout loads the checkout's build)
fn plugin_files() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if tessera_core::env::var("TTY").as_deref() == Some("1") {
        v.push(tessera_core::env::root().join("hypr/plugin/tessera-glass/tessera-glass.so"));
    }
    let data = data_dir();
    v.push(data.join("tessera-glass.so"));
    v
}

/// What a progress snapshot says of the plugin's build (Null: none of its)
pub fn build_of(snap: &Value, now: f64, alive: bool) -> Value {
    if snap["label"] != LABEL {
        return Value::Null;
    }
    // (a failure stays until the next build: it says why glass is off)
    if snap["phase"] == "failed" {
        return json!({"active": false, "failed": true, "error": snap["error"].as_str().unwrap_or(""), "log": snap["log"].clone(), "id": snap["started_at"].clone()});
    }
    // (done: which build, so one that ended between two readings is noticed)
    if snap["phase"] == "done" {
        return json!({"active": false, "failed": false, "done": true, "id": snap["started_at"].clone()});
    }
    match tessera_core::build_progress::view(snap, now, alive) {
        Some(v) if v.active => json!({"active": true, "failed": false, "fraction": v.fraction.map_or(-1.0, |f| f as f64), "time": v.time, "log": v.log, "id": v.id}),
        _ => Value::Null,
    }
}

/// Another build running (the shell's, an update's): one at a time
fn other_of(snap: &Value, now: f64, alive: bool) -> bool {
    snap["label"] != LABEL && tessera_core::build_progress::view(snap, now, alive).is_some_and(|v| v.active)
}

fn now_s() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

/// One reading of the files (cheap: a few small files; on the reader's thread)
fn read_files(running: &str, headers: &str) -> Value {
    let snap: Value = std::fs::read_to_string(tessera_core::build_progress::path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null);
    let alive = snap["pid"].as_i64().is_some_and(|p| p > 0 && std::path::Path::new(&format!("/proc/{p}")).exists());
    let now = now_s();
    let data = data_dir();
    let built_for = std::fs::read_to_string(data.join("tessera-glass.built-for")).ok().map(|t| xyz(&t)).unwrap_or_default();
    let fake = if sandboxed() { std::fs::read_to_string(runtime_dir().join("tessera-fake-glass")).map(|t| t.trim().to_string()).unwrap_or_default() } else { String::new() };
    json!({
        "running": running, "headers": headers, "builtFor": built_for,
        "built": plugin_files().iter().any(|p| p.is_file()),
        "build": build_of(&snap, now, alive),
        "other": other_of(&snap, now, alive),
        "fake": fake,
    })
}

/// A sandbox's made-up state (for screenshots of each): (loaded, version, glassBuild)
pub fn fake(kind: &str) -> Option<(bool, i64, Value)> {
    let base = |v: Value| {
        let mut b = json!({"running": "0.57.2", "headers": "0.57.2", "builtFor": "", "built": false, "build": null, "other": false, "starting": false, "skipped": false});
        for (k, x) in v.as_object().into_iter().flatten() {
            b[k] = x.clone();
        }
        b
    };
    Some(match kind {
        "missing" => (false, 0, base(json!({}))),
        "updated" => (false, 0, base(json!({"builtFor": "0.56.2", "built": true}))),
        "unloaded" => (false, 0, base(json!({"builtFor": "0.57.2", "built": true}))),
        "outdated" => (true, 4, base(json!({"builtFor": "0.57.2", "built": true}))),
        "failed" => (false, 0, base(json!({"build": {"failed": true, "error": "main.cpp:412:18: error: 'class CHyprOpenGLImpl' has no member named 'renderTextureWithBlur'"}}))),
        "headers" => (false, 0, base(json!({"running": "0.57.1", "build": {"failed": true, "error": "error: Hyprland's headers (0.57.2) don't match the running Hyprland (0.57.1)"}}))),
        "building" => (false, 0, base(json!({"build": {"active": true, "fraction": 0.4, "time": "0:16 · about 1 min left"}}))),
        "ok" => (true, 5, base(json!({"builtFor": "0.57.2", "built": true}))),
        _ => return None,
    })
}

impl GlassBuild {
    fn publish(&mut self, h: &mut Host) {
        let mut v = self.read.clone();
        if !v.is_object() {
            v = json!({});
        }
        v["starting"] = json!(self.starting.is_some());
        v["skipped"] = json!(self.skipped);
        h.set_state("glassBuild", v);
    }

    fn start_build(&mut self, h: &mut Host) {
        if self.starting.is_some() || self.read["build"]["active"] == true || self.read["other"] == true {
            return;
        }
        // Never in a sandbox: it would build and load into the real Hyprland's
        // plugin folder (as other actions there, it says it skipped)
        if sandboxed() {
            self.skipped = true;
            run(&["notify-send", "-a", "Tessera sandbox", "Skipped building the glass plugin", "tools/build-plugin.sh --load"]);
            self.publish(h);
            return;
        }
        let dir = source_dir(tessera_core::env::root(), data_dir());
        let script = dir.join("tools/build-plugin.sh");
        eprintln!("tessera-ui: glass plugin: building ({} --load)", script.display());
        self.starting = Some(now_ms());
        self.skipped = false;
        // (setsid: its own session, so it finishes if the shell restarts; the
        // shell's environment, so a console session's build lands where its
        // Hyprland loads it from, and its progress where this reads it)
        std::thread::spawn(move || {
            let _ = std::process::Command::new("setsid")
                .arg("-f")
                .arg("bash")
                .arg(&script)
                .arg("--load")
                .current_dir(&dir)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
        });
        if let Some(w) = &self.wake {
            let _ = w.send(());
        }
        self.publish(h);
    }

    /// build.log in the text editor (the default app for text), else a pager in a terminal
    fn show_log(&self) {
        let log = self.read["build"]["log"].as_str().filter(|l| !l.is_empty()).map(PathBuf::from).unwrap_or_else(tessera_core::build_progress::log_path);
        let path = log.to_string_lossy().into_owned();
        std::thread::spawn(move || {
            let editor = capture(&["xdg-mime", "query", "default", "text/plain"]).is_some_and(|d| !d.trim().is_empty());
            if editor {
                launch_open(&path);
            } else {
                launch_terminal(&["less", "+G", &path], false);
            }
        });
    }
}

impl Service for GlassBuild {
    fn name(&self) -> &'static str {
        "GlassBuild"
    }
    fn calls(&self) -> &'static [&'static str] {
        &["glassbuild"]
    }
    fn start(&mut self, h: &mut Host) {
        let send = h.sender("GlassBuild");
        let (tx, rx) = mpsc::channel::<()>();
        self.wake = Some(tx);
        std::thread::Builder::new()
            .name("glass-build".into())
            .spawn(move || {
                let mut running = running_version();
                let mut headers = headers_version();
                let mut last = Value::Null;
                let mut fast = 0u32;
                loop {
                    let v = read_files(&running, &headers);
                    let active = v["build"]["active"] == true;
                    if v != last {
                        // (a build ended: the headers may have changed with it)
                        if last["build"]["active"] == true && !active {
                            headers = headers_version();
                        }
                        last = v.clone();
                        send(Box::new(Msg::Read(v)));
                    }
                    // every second while a build runs (or was just asked for), else every 5
                    let wait = if active || fast > 0 { Duration::from_secs(1) } else { Duration::from_secs(5) };
                    fast = fast.saturating_sub(1);
                    match rx.recv_timeout(wait) {
                        Ok(()) => fast = 20,
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                        Err(mpsc::RecvTimeoutError::Disconnected) => return,
                    }
                    if running.is_empty() {
                        running = running_version();
                    }
                }
            })
            .ok();
        self.publish(h);
    }
    fn call(&mut self, h: &mut Host, call: &str, _args: &Value) {
        match call {
            "glassbuild.start" => self.start_build(h),
            "glassbuild.log" => self.show_log(),
            _ => {}
        }
    }
    fn message(&mut self, h: &mut Host, msg: Box<dyn Any + Send>) {
        let Ok(m) = msg.downcast::<Msg>() else { return };
        let Msg::Read(v) = *m;
        let active = v["build"]["active"] == true;
        // (Build pressed: until its build shows, or 20 s)
        if let Some(t) = self.starting {
            let id_ms = v["build"]["id"].as_f64().unwrap_or(0.0) * 1000.0;
            if (v["build"].is_object() && id_ms + 2000.0 >= t as f64) || now_ms() - t > 20_000 {
                self.starting = None;
            }
        }
        let ended = (self.was_active && !active) || (v["build"]["done"] == true && v["build"]["id"] != self.read["build"]["id"]);
        if ended {
            // The build ended: is the plugin there now? (asked again shortly
            // after: Hyprland may take a moment with a plugin it just loaded)
            eprintln!("tessera-ui: glass plugin: build {}", if v["build"]["failed"] == true { "failed" } else { "ended" });
            let glass = h.st.shell.glass.clone();
            glass.ping();
            slint::Timer::single_shot(Duration::from_millis(1500), move || glass.ping());
        }
        self.was_active = active;
        self.read = v;
        self.publish(h);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert_eq!(xyz("v0.57.2-b"), "0.57.2");
        assert_eq!(xyz("Hyprland 0.57.2 built from branch main"), "0.57.2");
        assert_eq!(xyz("0.57.2\n"), "0.57.2");
        assert_eq!(xyz(""), "");
    }

    #[test]
    fn where_it_builds() {
        let tmp = std::env::temp_dir().join(format!("tessera-glassbuild-{}", std::process::id()));
        let co = tmp.join("checkout");
        std::fs::create_dir_all(co.join("tools")).unwrap();
        std::fs::create_dir_all(co.join("hypr/plugin/tessera-glass")).unwrap();
        let data = tmp.join("data");
        // (no script, no sources: the installed copy)
        assert_eq!(source_dir(co.clone(), data.clone()), data.join("src"));
        std::fs::write(co.join("tools/build-plugin.sh"), "").unwrap();
        assert_eq!(source_dir(co.clone(), data.clone()), data.join("src"));
        std::fs::write(co.join("hypr/plugin/tessera-glass/Makefile"), "").unwrap();
        assert_eq!(source_dir(co.clone(), data.clone()), co);
        let _ = std::fs::remove_dir_all(tmp);
    }

    #[test]
    fn its_build_only() {
        let snap = json!({"phase": "compile", "label": "tessera-glass", "title": "the glass plugin", "kind": "make", "step": "Building the glass plugin",
                          "fraction": 0.4, "elapsed_s": 16.0, "eta_s": 24.0, "started_at": 1000.0, "updated_at": 1016.0, "pid": 1, "log": "/l"});
        let b = build_of(&snap, 1016.0, true);
        assert_eq!((b["active"].clone(), b["fraction"].as_f64().map(|f| (f * 100.0).round())), (json!(true), Some(40.0)));
        assert!(!other_of(&snap, 1016.0, true));
        // (gone: its process ended without a word)
        assert_eq!(build_of(&snap, 1016.0, false), Value::Null);
        // another build: not this one's, but one at a time
        let mut shell = snap.clone();
        shell["label"] = json!("tessera-ui");
        assert_eq!(build_of(&shell, 1016.0, true), Value::Null);
        assert!(other_of(&shell, 1016.0, true));
        // a failure stays (however old) with its reason
        let mut f = snap.clone();
        f["phase"] = json!("failed");
        f["error"] = json!("error: Hyprland's headers (0.57.2) don't match the running Hyprland (0.57.1)");
        let b = build_of(&f, 1_000_000.0, false);
        assert_eq!((b["failed"].clone(), b["error"].as_str().unwrap().starts_with("error: Hyprland's headers")), (json!(true), true));
        // done: which one, nothing more
        f["phase"] = json!("done");
        assert_eq!(build_of(&f, 1016.0, false), json!({"active": false, "failed": false, "done": true, "id": 1000.0}));
        // (an old snapshot without a label, or none)
        assert_eq!(build_of(&Value::Null, 1016.0, false), Value::Null);
    }

    #[test]
    fn made_up_states() {
        for k in ["missing", "updated", "unloaded", "outdated", "failed", "headers", "building", "ok"] {
            assert!(fake(k).is_some(), "{k}");
        }
        assert!(fake("").is_none());
        assert_eq!(fake("outdated").unwrap().1, 4);
    }
}

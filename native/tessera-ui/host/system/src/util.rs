//! Small helpers every module of this crate uses (processes, files, paths).
//! Never block the loop: `capture`/`run_async` are for threads; the callers
//! send the result back with `Host::sender`.

use serde_json::Value;
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Run a command to the end (blocking: call on a thread). Stdout as text;
/// None if it could not start. `timeout` kills a hung child (our own child only).
pub fn capture(cmd: &[&str], timeout: Duration) -> Option<(i32, String)> {
    let mut c = Command::new(cmd.first()?).args(&cmd[1..]).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
    let mut out = c.stdout.take()?;
    let t = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = out.read_to_string(&mut s);
        s
    });
    let start = Instant::now();
    let code = loop {
        match c.try_wait() {
            Ok(Some(st)) => break st.code().unwrap_or(-1),
            Ok(None) if start.elapsed() > timeout => {
                let _ = c.kill();
                let _ = c.wait();
                break -1;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(_) => break -1,
        }
    };
    Some((code, t.join().unwrap_or_default()))
}

/// `capture` with sh -c (blocking)
pub fn sh(script: &str, timeout: Duration) -> Option<(i32, String)> {
    capture(&["sh", "-c", script], timeout)
}

/// Fire and forget on a thread (output ignored)
pub fn run_async(cmd: &[&str]) {
    tessera_core::services::bridge::run(cmd);
}

/// Run on a thread, then hand the (code, stdout) to `then` (also on that thread)
pub fn spawn_capture(cmd: Vec<String>, timeout: Duration, then: impl FnOnce(Option<(i32, String)>) + Send + 'static) {
    std::thread::spawn(move || {
        let c: Vec<&str> = cmd.iter().map(String::as_str).collect();
        then(capture(&c, timeout));
    });
}

pub fn home() -> std::path::PathBuf {
    std::env::var_os("HOME").map(Into::into).unwrap_or_else(|| "/".into())
}

/// The repo the shell runs from (shell-ipc.sh's root: TESSERA_ROOT, else this build's)
pub fn root() -> std::path::PathBuf {
    std::env::var_os("TESSERA_ROOT").filter(|r| !r.is_empty()).map(Into::into).unwrap_or_else(|| std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../..")))
}

/// shell/assets/scripts/<name> (the helper scripts the QML calls)
pub fn script(name: &str) -> String {
    root().join("shell/assets/scripts").join(name).to_string_lossy().into_owned()
}

/// $XDG_RUNTIME_DIR, else /run/user/<uid>
pub fn runtime_dir() -> std::path::PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR").map(Into::into).unwrap_or_else(|| format!("/run/user/{}", unsafe { libc::getuid() }).into())
}

/// ~/.config/tessera-or-irice style state dir for files the QML keeps under
/// ~/.local/state/<name> (callers pass the same leaf the QML used)
pub fn state_dir() -> std::path::PathBuf {
    std::env::var_os("XDG_STATE_HOME").map(Into::into).unwrap_or_else(|| home().join(".local/state"))
}

pub fn read_json(path: &std::path::Path) -> Value {
    std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Null)
}

/// Write a file by temp + rename (readers never see half a file)
pub fn write_atomic(path: &std::path::Path, text: &str) -> std::io::Result<()> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

pub fn s(v: &Value, k: &str) -> String {
    v[k].as_str().unwrap_or("").to_string()
}
pub fn n(v: &Value, k: &str) -> f64 {
    v[k].as_f64().unwrap_or(0.0)
}
pub fn b(v: &Value, k: &str) -> bool {
    v[k].as_bool().unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capture_runs_and_times_out() {
        let (c, o) = capture(&["echo", "hi"], Duration::from_secs(2)).unwrap();
        assert_eq!((c, o.trim()), (0, "hi"));
        let (c, _) = capture(&["sleep", "5"], Duration::from_millis(100)).unwrap();
        assert_eq!(c, -1);
    }
}

/// Set one key of config.json (dotted path, "network.autojoin"), as
/// Config.qml's adapter write does: read, set, atomic write. The config
/// watcher then raises `config_changed`. (host-shell's Config may do this
/// better; until then every module of this crate uses this one.)
pub fn config_set(path: &str, v: Value) {
    let file = tessera_core::theme::config_path();
    let mut cfg = read_json(&file);
    if !cfg.is_object() {
        cfg = serde_json::json!({});
    }
    let keys: Vec<&str> = path.split('.').collect();
    let mut at = &mut cfg;
    for k in &keys[..keys.len() - 1] {
        if !at[*k].is_object() {
            at[*k] = serde_json::json!({});
        }
        at = &mut at[*k];
    }
    at[keys[keys.len() - 1]] = v;
    let _ = write_atomic(&file, &serde_json::to_string_pretty(&cfg).unwrap_or_default());
}

/// One value of config.json by dotted path (Null if unset)
pub fn config_get(cfg: &Value, path: &str) -> Value {
    path.split('.').fold(cfg, |v, k| &v[k]).clone()
}

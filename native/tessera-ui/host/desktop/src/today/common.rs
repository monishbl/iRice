//! What the services of this group share: the state file (UiState's
//! state.json, read-modify-written whole), the settings file, time helpers,
//! `notify-send`, and the island's lines (Island.post/update/remove as the
//! surfaces read them: {"src":"shell","kind":"island",…}).

use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, Ordering};
use tessera_core::host::Host;
use tessera_core::services::calendar as cal;
use tessera_core::util;

// ── The state file (common/UiState.qml) ─────────────────────────────────

pub fn state_path() -> PathBuf {
    util::state_dir().join("state.json")
}

/// The whole state file as an object ({} when there is none)
pub fn state_read() -> Value {
    std::fs::read_to_string(state_path())
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}))
}

/// One key of it (Null when absent)
pub fn state_get(key: &str) -> Value {
    state_read()[key].clone()
}

/// Set one key and write the file whole (UiState.data.<key> = value). The
/// file is read again first, so a key another part wrote meanwhile stays.
pub fn state_set(key: &str, v: Value) {
    let mut all = state_read();
    all[key] = v;
    let text = serde_json::to_string_pretty(&all).unwrap_or_default();
    let path = state_path();
    if let Err(e) = util::write_atomic(&path, text.as_bytes()) {
        eprintln!("tessera-ui: host: can't write {}: {e}", path.display());
    }
}

// ── Settings (Config.options) ───────────────────────────────────────────

/// A setting by path ("today.mode"), from the file as it is now
pub fn cfg_get(path: &str) -> Value {
    let v = tessera_core::theme::read_json();
    path.split('.').fold(&v, |v, k| &v[k]).clone()
}

/// Config.options.<path> = value, written to config.json whole
pub fn cfg_set(path: &str, value: Value) {
    let mut v = tessera_core::theme::read_json();
    if !v.is_object() {
        v = json!({});
    }
    let keys: Vec<&str> = path.split('.').collect();
    let mut at = &mut v;
    for k in &keys[..keys.len() - 1] {
        if !at.get(*k).is_some_and(Value::is_object) {
            at[*k] = json!({});
        }
        at = &mut at[*k];
    }
    at[keys[keys.len() - 1]] = value;
    let p = tessera_core::theme::config_path();
    if let Err(e) = util::write_atomic(&p, serde_json::to_string_pretty(&v).unwrap_or_default().as_bytes()) {
        eprintln!("tessera-ui: host: can't write {}: {e}", p.display());
    }
}

// ── Time ────────────────────────────────────────────────────────────────

pub fn now() -> i64 {
    util::now_ms()
}

/// TimeFormat.hm: "9:41 AM" or "09:41" (appearance.clock24)
pub fn hm(ms: i64) -> String {
    let h24 = cfg_get("appearance.clock24").as_bool().unwrap_or(false);
    util::strftime(if h24 { "%H:%M" } else { "%-I:%M %p" }, ms.div_euclid(1000))
}

/// A short id: time and a few random characters in base 36
pub fn new_id() -> String {
    static N: AtomicI64 = AtomicI64::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.subsec_nanos() as i64);
    format!("{}{}", base36(now()), base36((nanos ^ (n << 16) ^ (std::process::id() as i64) << 8) & 0x3fff_ffff))
}

fn base36(mut n: i64) -> String {
    if n <= 0 {
        return "0".into();
    }
    let mut s = Vec::new();
    while n > 0 {
        s.push(b"0123456789abcdefghijklmnopqrstuvwxyz"[(n % 36) as usize]);
        n /= 36;
    }
    s.reverse();
    String::from_utf8(s).unwrap_or_default()
}

/// Midnight (local) of the day a time falls on
pub fn day_start(ms: i64) -> i64 {
    cal::day_bounds(ms).0
}

/// "YYYY-MM-DD" of the day it is, days starting at 4 AM (Today.dayKey)
pub fn day_key(ms: i64) -> String {
    util::strftime("%Y-%m-%d", (ms - 4 * 3600 * 1000).div_euclid(1000))
}

// ── Processes ───────────────────────────────────────────────────────────

/// A command on a thread, output ignored (Quickshell.execDetached)
pub fn detached(cmd: &[&str]) {
    let cmd: Vec<String> = cmd.iter().map(|s| s.to_string()).collect();
    std::thread::spawn(move || {
        let Some((p, a)) = cmd.split_first() else { return };
        let _ = std::process::Command::new(p)
            .args(a)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    });
}

/// notify-send -a <app> [-i icon] [-u urgency] -- title body
pub fn notify_send(app: &str, icon: Option<&str>, urgency: Option<&str>, title: &str, body: &str) {
    let mut c: Vec<&str> = vec!["notify-send", "-a", app];
    if let Some(i) = icon {
        c.extend(["-i", i]);
    }
    if let Some(u) = urgency {
        c.extend(["-u", u]);
    }
    c.extend(["--", title, body]);
    detached(&c);
}

// ── The island (services/Island.qml's side of the seam) ─────────────────

/// Alert uids this group makes start here, so they never meet the island
/// service's own (RustBridge's islandAct for them goes to `today islandAct`)
pub const UID_BASE: i64 = 1_000_000_000;
static UID: AtomicI64 = AtomicI64::new(UID_BASE);

/// Island.post(event, ms): returns the uid
pub fn island_post(h: &mut Host, event: Value, ms: i64) -> i64 {
    let uid = UID.fetch_add(1, Ordering::Relaxed);
    h.say(json!({"src": "shell", "kind": "island", "op": "post", "uid": uid, "ms": ms, "event": event}));
    uid
}

pub fn island_update(h: &mut Host, uid: i64, patch: Value) {
    h.say(json!({"src": "shell", "kind": "island", "op": "update", "uid": uid, "patch": patch}));
}

pub fn island_remove(h: &mut Host, uid: i64) {
    h.say(json!({"src": "shell", "kind": "island", "op": "remove", "uid": uid}));
}

/// Theme.accents.<name> for the current look
pub fn accent(h: &Host, name: &str) -> String {
    tessera_core::theme::accent_hex(name, h.st.shell.look.dark)
}

/// A string argument of a call (empty when missing)
pub fn arg_str(args: &Value, i: usize) -> String {
    args.get(i).and_then(Value::as_str).unwrap_or("").to_string()
}

/// A call's arguments as an array (a lone value counts as one)
pub fn args_of(args: &Value) -> Vec<Value> {
    match args {
        Value::Array(a) => a.clone(),
        Value::Null => vec![],
        v => vec![v.clone()],
    }
}

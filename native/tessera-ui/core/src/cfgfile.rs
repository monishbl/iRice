//! Tessera's own JSON files kept safe: config.json (the settings) and
//! state.json (UiState's file). Whatever is in them, they're read without a
//! panic; a change is always the file as it is now with that change made,
//! under a lock (a thread of this process, another process of ours, and
//! state-merge.py all take turns), written whole by a rename.
//!
//! A file that can't be read as a JSON object is never written over: it's
//! kept aside first (config.json.broken-<time>, state.json.corrupt-<time> as
//! state-merge.py names it), and for the settings the person is told once.
//! The settings then go on from the last good copy this process read (a hand
//! edit with a typo loses only that edit), else from the defaults.
//!
//! The UI thread never waits on a write (each one is a whole file synced to
//! disk under a lock: 30 ms, and longer while another writer holds it):
//! `set_state`, `set_config` and the `*_later` edits called there go to one
//! lane (crate::work), in order, and what this process reads meanwhile has
//! the change laid over the file already. Reads are kept while the file is
//! unchanged (one stat a read).

use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

/// What a file held
#[derive(Debug)]
pub enum Read {
    /// not there
    Missing,
    /// a JSON object (an empty file counts as an empty one: nothing to lose)
    Fine(Map<String, Value>),
    /// there, but not a JSON object (bad JSON, a list, a number…), or not text
    Broken(String),
    /// there, but it can't be opened (permissions, a folder): left alone
    Unreadable(String),
}

/// A file's text as one of the above
pub fn parse(text: &str) -> Read {
    let t = text.trim_start_matches('\u{feff}');
    if t.trim().is_empty() {
        return Read::Fine(Map::new());
    }
    match serde_json::from_str::<Value>(t) {
        Ok(Value::Object(m)) => Read::Fine(m),
        Ok(other) => Read::Broken(format!("not an object but {}", kind(&other))),
        Err(e) => Read::Broken(e.to_string()),
    }
}

fn kind(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "true/false",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "a list",
        Value::Object(_) => "an object",
    }
}

/// A file read once, as it is now
pub fn read(path: &Path) -> Read {
    match std::fs::read(path) {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(s) => parse(&s),
            Err(_) => Read::Broken("not UTF-8 text".into()),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Read::Missing,
        Err(e) => Read::Unreadable(e.to_string()),
    }
}

/// The same, read again a few times while it looks broken: something that
/// writes in place (an editor, a script) may be half way through
pub fn read_settled(path: &Path) -> Read {
    let mut r = read(path);
    for _ in 0..3 {
        if !matches!(r, Read::Broken(_)) {
            break;
        }
        std::thread::sleep(Duration::from_millis(40));
        r = read(path);
    }
    r
}

/// The file's object ({} when it's missing or broken: callers that only read),
/// with this process's changes not yet written laid over it
pub fn read_object(path: &Path) -> Map<String, Value> {
    let mut m = match read_kept(path) {
        Read::Fine(m) => m,
        _ => Map::new(),
    };
    overlay(path, &mut m);
    m
}

/// What a file's last read found, while its identity, size and time are the
/// same (state.json is read every couple of seconds: one stat, no parse)
type Kept = (PathBuf, (u64, u64, std::time::SystemTime), Map<String, Value>);
static KEPT: Mutex<Vec<Kept>> = Mutex::new(Vec::new());

fn read_kept(path: &Path) -> Read {
    use std::os::unix::fs::MetadataExt;
    let Ok(md) = std::fs::metadata(path) else { return read(path) };
    let Ok(mtime) = md.modified() else { return read(path) };
    let id = (md.ino(), md.len(), mtime);
    if let Ok(k) = KEPT.lock() {
        if let Some((_, _, m)) = k.iter().find(|(p, i, _)| p == path && *i == id) {
            return Read::Fine(m.clone());
        }
    }
    let r = read(path);
    if let (Read::Fine(m), Ok(mut k)) = (&r, KEPT.lock()) {
        k.retain(|(p, _, _)| p != path);
        k.push((path.to_path_buf(), id, m.clone()));
    }
    r
}

// ── Changes not yet written ────────────────────────────────────────────────

/// A change to a file's object; true when it changed anything
pub type Change = std::sync::Arc<dyn Fn(&mut Map<String, Value>) -> bool + Send + Sync>;

/// Changes queued on the files lane, laid over reads until they're written
static PENDING: Mutex<Vec<(u64, PathBuf, Change)>> = Mutex::new(Vec::new());
static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// The settings' and state's writes, in the order they were asked for
pub static FILES: crate::work::Lane = crate::work::Lane::new("files");

fn overlay(path: &Path, m: &mut Map<String, Value>) {
    if let Ok(p) = PENDING.lock() {
        for (_, at, c) in p.iter() {
            if at == path {
                c(m);
            }
        }
    }
}

fn pending_for(path: &Path) -> bool {
    PENDING.lock().is_ok_and(|p| p.iter().any(|(_, at, _)| at == path))
}

fn edit_later(path: &Path, kind: Kind, change: Change, what: String) {
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if let Ok(mut p) = PENDING.lock() {
        p.push((seq, path.to_path_buf(), change.clone()));
    }
    let path = path.to_path_buf();
    FILES.run(move || {
        if let Err(e) = edit(&path, kind, |m| change(m)) {
            eprintln!("tessera-ui: can't save {what}: {e}");
        }
        if let Ok(mut p) = PENDING.lock() {
            p.retain(|e| e.0 != seq);
        }
    });
}

/// `edit_config` without waiting: on the files lane, after the changes asked
/// for before it; this process reads the change at once
pub fn edit_config_later(f: impl Fn(&mut Map<String, Value>) -> bool + Send + Sync + 'static) {
    edit_later(&crate::theme::config_path(), Kind::Config, std::sync::Arc::new(f), "a setting".into());
}

/// A file of our own written whole (write_atomic) on the files lane, in order
/// with the settings and state: focus.json, spaces.lua…
pub fn write_later(path: PathBuf, bytes: Vec<u8>) {
    FILES.run(move || {
        if let Err(e) = crate::util::write_atomic(&path, &bytes) {
            eprintln!("tessera-ui: can't write {}: {e}", path.display());
        }
    });
}

/// Wait (a moment at most) for the changes queued: on the way out
pub fn flush() {
    FILES.flush(Duration::from_secs(3));
}

// ── The settings ───────────────────────────────────────────────────────────

/// The settings as last read whole (path, object), so a broken file mid-edit
/// neither flashes the defaults nor costs the rest of the settings
static LAST_GOOD: Mutex<Vec<(PathBuf, Map<String, Value>)>> = Mutex::new(Vec::new());

fn remember(path: &Path, m: &Map<String, Value>) {
    if let Ok(mut g) = LAST_GOOD.lock() {
        match g.iter_mut().find(|(p, _)| p == path) {
            Some(e) => e.1 = m.clone(),
            None => g.push((path.to_path_buf(), m.clone())),
        }
    }
}

fn last_good(path: &Path) -> Option<Map<String, Value>> {
    LAST_GOOD.lock().ok()?.iter().find(|(p, _)| p == path).map(|(_, m)| m.clone())
}

/// The settings as JSON: the file's object; while it's broken, the last good
/// copy this process read; Null when there's none
pub fn config_value() -> Value {
    config_value_at(&crate::theme::config_path())
}

pub fn config_value_at(path: &Path) -> Value {
    let v = match read_kept(path) {
        Read::Fine(m) => {
            remember(path, &m);
            Value::Object(m)
        }
        Read::Missing => Value::Null,
        Read::Broken(_) | Read::Unreadable(_) => last_good(path).map_or(Value::Null, Value::Object),
    };
    // (changes this process made that aren't written yet)
    if !pending_for(path) {
        return v;
    }
    let mut m = match v {
        Value::Object(m) => m,
        _ => Map::new(),
    };
    overlay(path, &mut m);
    Value::Object(m)
}

/// config.json as Qt writes it (4-space indent, a newline at the end)
pub fn config_text(v: &Value) -> String {
    use serde::Serialize;
    let mut out = Vec::new();
    let fmt = serde_json::ser::PrettyFormatter::with_indent(b"    ");
    let mut ser = serde_json::Serializer::with_formatter(&mut out, fmt);
    if v.serialize(&mut ser).is_err() {
        return String::new();
    }
    String::from_utf8(out).unwrap_or_default() + "\n"
}

/// Set a dotted path ("dock.iconSize") in an object, making objects on the
/// way (a section that isn't one becomes one); false when it was already so
pub fn put_path(m: &mut Map<String, Value>, path: &str, v: Value) -> bool {
    let keys: Vec<&str> = path.split('.').collect();
    let Some((last, parents)) = keys.split_last() else { return false };
    let mut at = m;
    for k in parents {
        let e = at.entry(k.to_string()).or_insert_with(|| json!({}));
        if !e.is_object() {
            *e = json!({});
        }
        at = e.as_object_mut().expect("just made an object");
    }
    if at.get(*last) == Some(&v) {
        return false;
    }
    at.insert(last.to_string(), v);
    true
}

/// Change the settings file: `f` gets its object (the file as it is now) and
/// says whether it changed anything; then it's written whole. Ok(false):
/// nothing to write. An unreadable file is left alone (Err).
pub fn edit_config(f: impl FnOnce(&mut Map<String, Value>) -> bool) -> std::io::Result<bool> {
    edit_config_at(&crate::theme::config_path(), f)
}

pub fn edit_config_at(path: &Path, f: impl FnOnce(&mut Map<String, Value>) -> bool) -> std::io::Result<bool> {
    edit(path, Kind::Config, f)
}

/// `Config.options.<path> = v`, written at once (from the UI thread: queued,
/// and read as set at once); whether the file has it now
pub fn set_config(path: &str, v: Value) -> bool {
    let p = path.to_string();
    if crate::work::on_ui_thread() {
        edit_later(&crate::theme::config_path(), Kind::Config, std::sync::Arc::new(move |m| put_path(m, &p, v.clone())), format!("the setting {path}"));
        return true;
    }
    match edit_config(move |m| put_path(m, &p, v)) {
        Ok(_) => true,
        Err(e) => {
            eprintln!("tessera-ui: can't save the setting {path}: {e}");
            false
        }
    }
}

/// What a first start writes (no settings yet, or a broken file kept
/// aside): Config.qml's defaults apply to every key missing; the welcome
/// screen shows (a file from the defaults has `welcome.done: false`, which
/// is also what keeps an older shell's upgrade from skipping it), and the
/// upgrades for files from before defaultsVersion 3 aren't needed
pub fn first_run() -> Map<String, Value> {
    match json!({ "defaultsVersion": 3, "welcome": { "done": false } }) {
        Value::Object(m) => m,
        _ => Map::new(),
    }
}

/// At the shell's start (standalone: it owns the settings): no file yet, one
/// is made; a broken one is kept aside, the person is told, and a fresh one
/// is written. What happened, for the log.
pub fn check_at_start() -> String {
    check_config_at(&crate::theme::config_path())
}

pub fn check_config_at(path: &Path) -> String {
    let _g = PROCESS_LOCK.lock();
    let _dir = DirLock::new(path, Kind::Config);
    match read_settled(path) {
        Read::Fine(m) => {
            remember(path, &m);
            "fine".into()
        }
        Read::Missing => match write_whole(path, &config_text(&Value::Object(first_run()))) {
            Ok(()) => "made".into(),
            Err(e) => format!("can't make it: {e}"),
        },
        Read::Broken(why) => {
            let aside = keep_aside(path, Kind::Config);
            match &aside {
                Some(a) => tell_broken(path, a, &why),
                None => return format!("broken ({why}), and it can't be moved aside: left alone"),
            }
            let fresh = config_text(&Value::Object(first_run()));
            match write_whole(path, &fresh) {
                Ok(()) => format!("broken ({why}): kept as {}", aside.map(|a| a.display().to_string()).unwrap_or_default()),
                Err(e) => format!("broken ({why}), kept aside, but can't write a new one: {e}"),
            }
        }
        Read::Unreadable(e) => format!("unreadable ({e}): left alone, defaults used"),
    }
}

// ── The state file ─────────────────────────────────────────────────────────

/// $XDG_STATE_HOME/tessera/state.json
pub fn state_path() -> PathBuf {
    crate::util::state_dir().join("state.json")
}

/// state.json's object ({} when missing or broken)
pub fn read_state() -> Map<String, Value> {
    read_object(&state_path())
}

/// One key of state.json (Null when there's none)
pub fn state_get(key: &str) -> Value {
    read_state().remove(key).unwrap_or(Value::Null)
}

/// Change state.json under state-merge.py's lock (.state.lock), so neither
/// that merge nor another writer of ours loses the other's keys
pub fn edit_state(f: impl FnOnce(&mut Map<String, Value>) -> bool) -> std::io::Result<bool> {
    edit_state_at(&state_path(), f)
}

pub fn edit_state_at(path: &Path, f: impl FnOnce(&mut Map<String, Value>) -> bool) -> std::io::Result<bool> {
    edit(path, Kind::State, f)
}

/// `UiState.data.<key> = v`, written at once (from the UI thread: queued,
/// and read as set at once)
pub fn set_state(key: &str, v: Value) {
    let k = key.to_string();
    if crate::work::on_ui_thread() {
        let change = move |m: &mut Map<String, Value>| {
            if m.get(&k) == Some(&v) {
                return false;
            }
            m.insert(k.clone(), v.clone());
            true
        };
        edit_later(&state_path(), Kind::State, std::sync::Arc::new(change), format!("{key} in state.json"));
        return;
    }
    if let Err(e) = edit_state(move |m| {
        if m.get(&k) == Some(&v) {
            return false;
        }
        m.insert(k, v);
        true
    }) {
        eprintln!("tessera-ui: can't save {key} in state.json: {e}");
    }
}

// ── The machinery ──────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    Config,
    State,
}

/// Threads of this process take turns (flock is per open file, so two
/// threads would each get their own)
static PROCESS_LOCK: Mutex<()> = Mutex::new(());

/// The lock other processes see: state.json's is state-merge.py's
/// `.state.lock`; the settings' is the folder itself (no extra file in it)
struct DirLock(Option<std::fs::File>);

impl DirLock {
    fn new(path: &Path, kind: Kind) -> Self {
        use std::os::unix::io::AsRawFd;
        let Some(dir) = path.parent() else { return Self(None) };
        let _ = std::fs::create_dir_all(dir);
        let f = match kind {
            Kind::State => std::fs::OpenOptions::new().create(true).append(true).open(dir.join(".state.lock")),
            Kind::Config => std::fs::File::open(dir),
        };
        let Ok(f) = f else { return Self(None) };
        // (never forever: a stuck holder costs a moment, not the shell)
        for _ in 0..200 {
            if unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
                return Self(Some(f));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Self(None)
    }
}

impl Drop for DirLock {
    fn drop(&mut self) {
        use std::os::unix::io::AsRawFd;
        if let Some(f) = &self.0 {
            unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_UN) };
        }
    }
}

fn edit(path: &Path, kind: Kind, f: impl FnOnce(&mut Map<String, Value>) -> bool) -> std::io::Result<bool> {
    let _g = PROCESS_LOCK.lock();
    let _dir = DirLock::new(path, kind);
    let mut m = match read_settled(path) {
        Read::Fine(m) => m,
        Read::Missing => match kind {
            // (a first change before the start made a file: as a first start)
            Kind::Config => first_run(),
            Kind::State => Map::new(),
        },
        Read::Broken(why) => {
            let Some(aside) = keep_aside(path, kind) else {
                return Err(std::io::Error::other(format!("{} can't be read ({why}) nor moved aside", path.display())));
            };
            match kind {
                Kind::Config => {
                    tell_broken(path, &aside, &why);
                    last_good(path).unwrap_or_else(first_run)
                }
                Kind::State => {
                    eprintln!("tessera-ui: {} couldn't be read ({why}); kept as {}", path.display(), aside.display());
                    Map::new()
                }
            }
        }
        Read::Unreadable(e) => return Err(std::io::Error::other(format!("{} can't be opened: {e}", path.display()))),
    };
    if !f(&mut m) {
        return Ok(false);
    }
    let v = Value::Object(m);
    let text = match kind {
        Kind::Config => config_text(&v),
        // (as state-merge.py writes it: compact)
        Kind::State => serde_json::to_string(&v).unwrap_or_default(),
    };
    if text.is_empty() {
        return Err(std::io::Error::other("can't put it into words"));
    }
    write_whole(path, &text)?;
    if kind == Kind::Config {
        if let Value::Object(m) = v {
            remember(path, &m);
        }
    }
    Ok(true)
}

/// Written whole: to the file a link points at (a dotfiles link stays a
/// link), keeping its permissions
fn write_whole(path: &Path, text: &str) -> std::io::Result<()> {
    crate::util::write_atomic(path, text.as_bytes())
}

/// The broken file renamed beside itself, timed (a second one in the same
/// second gets a number); its new name, None when it couldn't be moved
fn keep_aside(path: &Path, kind: Kind) -> Option<PathBuf> {
    let target = crate::util::link_target(path);
    let name = target.file_name()?.to_string_lossy().into_owned();
    let tag = match kind {
        Kind::Config => "broken",
        Kind::State => "corrupt",
    };
    let stamp = crate::util::now("%Y%m%d-%H%M%S");
    let mut aside = target.with_file_name(format!("{name}.{tag}-{stamp}"));
    let mut n = 1;
    while aside.exists() {
        n += 1;
        aside = target.with_file_name(format!("{name}.{tag}-{stamp}-{n}"));
    }
    std::fs::rename(&target, &aside).or_else(|_| std::fs::copy(&target, &aside).map(|_| ())).ok()?;
    Some(aside)
}

/// The person told, once per broken file: a notification a few seconds on
/// (the shell's own notification server may be starting with it)
fn tell_broken(path: &Path, aside: &Path, why: &str) {
    eprintln!("tessera-ui: {} couldn't be read ({why}); kept as {}, defaults used", path.display(), aside.display());
    static TOLD: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
    if let Ok(mut t) = TOLD.lock() {
        if t.iter().any(|p| p == aside) {
            return;
        }
        t.push(aside.to_path_buf());
    }
    if !may_notify() {
        return;
    }
    let file = aside.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let body = format!("Tessera is using its default settings. Your old file is kept as {file}: fix it and rename it to config.json to get your settings back.");
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(4));
        let _ = std::process::Command::new("notify-send")
            .args(["-a", "Tessera", "-i", "dialog-warning", "Your settings couldn't be read", &body])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    });
}

/// Only inside a running Hyprland session, never from a test
fn may_notify() -> bool {
    !cfg!(test) && std::env::var_os("TESSERA_NO_NOTIFY").is_none() && std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some() && std::env::var_os("WAYLAND_DISPLAY").is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("tessera-cfgfile-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn asides(dir: &Path, tag: &str) -> Vec<PathBuf> {
        std::fs::read_dir(dir).unwrap().flatten().map(|e| e.path()).filter(|p| p.file_name().unwrap().to_string_lossy().contains(tag)).collect()
    }

    #[test]
    fn what_a_file_holds() {
        assert!(matches!(parse(""), Read::Fine(m) if m.is_empty()));
        assert!(matches!(parse("  \n"), Read::Fine(m) if m.is_empty()));
        assert!(matches!(parse("\u{feff}{\"a\": 1}"), Read::Fine(m) if m["a"] == 1));
        for bad in ["{", "{\"a\": }", "[]", "[1, 2]", "3", "\"text\"", "null", "true", "{\"a\":1}}", "{'a': 1}"] {
            assert!(matches!(parse(bad), Read::Broken(_)), "{bad}");
        }
        let d = tmp("read");
        assert!(matches!(read(&d.join("none.json")), Read::Missing));
        std::fs::write(d.join("bin.json"), [0xff, 0xfe, 0x00, 0x7b]).unwrap();
        assert!(matches!(read(&d.join("bin.json")), Read::Broken(_)));
        // (a folder where the file should be)
        std::fs::create_dir_all(d.join("dir.json")).unwrap();
        assert!(matches!(read(&d.join("dir.json")), Read::Unreadable(_)));
        assert!(read_object(&d.join("bin.json")).is_empty());
    }

    #[test]
    fn a_change_keeps_every_other_key() {
        let d = tmp("keep");
        let p = d.join("config.json");
        std::fs::write(&p, r#"{"dock": {"iconSize": 60, "pinned": ["a"]}, "unknownFuture": {"x": [1, 2]}, "menubar": "odd"}"#).unwrap();
        assert!(edit_config_at(&p, |m| put_path(m, "dock.enabled", json!(false))).unwrap());
        assert!(edit_config_at(&p, |m| put_path(m, "menubar.autoHide", json!(true))).unwrap());
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!(v["dock"], json!({"iconSize": 60, "pinned": ["a"], "enabled": false}));
        assert_eq!(v["unknownFuture"], json!({"x": [1, 2]}));
        assert_eq!(v["menubar"], json!({"autoHide": true}));
        // (the same value again: nothing written)
        assert!(!edit_config_at(&p, |m| put_path(m, "dock.enabled", json!(false))).unwrap());
        // (Qt's layout)
        assert!(std::fs::read_to_string(&p).unwrap().starts_with("{\n    \""));
        assert!(asides(&d, "broken").is_empty());
    }

    #[test]
    fn a_broken_file_is_kept_aside_never_written_over() {
        let d = tmp("broken");
        let p = d.join("config.json");
        let bad = "{\"dock\": {\"iconSize\": 60,, }";
        std::fs::write(&p, bad).unwrap();
        assert!(edit_config_at(&p, |m| put_path(m, "dock.enabled", json!(false))).unwrap());
        let kept = asides(&d, "config.json.broken-");
        assert_eq!(kept.len(), 1);
        assert_eq!(std::fs::read_to_string(&kept[0]).unwrap(), bad);
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!(v["dock"]["enabled"], json!(false));
        assert_eq!(v["welcome"]["done"], json!(false));
        // A second broken file in the same second gets its own name
        std::fs::write(&p, "[1]").unwrap();
        edit_config_at(&p, |m| put_path(m, "a", json!(1))).unwrap();
        assert_eq!(asides(&d, "config.json.broken-").len(), 2);
    }

    #[test]
    fn a_broken_file_mid_edit_keeps_the_last_good_settings() {
        let d = tmp("lastgood");
        let p = d.join("config.json");
        std::fs::write(&p, r#"{"appearance": {"accent": "pink"}, "dock": {"iconSize": 40}}"#).unwrap();
        assert_eq!(config_value_at(&p)["appearance"]["accent"], json!("pink"));
        std::fs::write(&p, r#"{"appearance": {"accent": "pink""#).unwrap();
        // (what the surfaces read meanwhile: the last good, not the defaults)
        assert_eq!(config_value_at(&p)["dock"]["iconSize"], json!(40));
        // (a change now: the last good plus it, the broken one kept)
        edit_config_at(&p, |m| put_path(m, "dock.enabled", json!(true))).unwrap();
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!((v["appearance"]["accent"].clone(), v["dock"]["iconSize"].clone(), v["dock"]["enabled"].clone()), (json!("pink"), json!(40), json!(true)));
        assert_eq!(asides(&d, "broken").len(), 1);
    }

    #[test]
    fn an_unreadable_file_is_left_alone() {
        let d = tmp("unreadable");
        let p = d.join("config.json");
        std::fs::create_dir_all(&p).unwrap();
        assert!(edit_config_at(&p, |m| put_path(m, "a", json!(1))).is_err());
        assert!(p.is_dir());
        assert!(check_config_at(&p).starts_with("unreadable"));
    }

    #[test]
    fn the_start_makes_or_mends_the_file() {
        let d = tmp("start");
        let p = d.join("tessera/config.json");
        // first start: no folder, no file
        assert_eq!(check_config_at(&p), "made");
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!(v, json!({"defaultsVersion": 3, "welcome": {"done": false}}));
        assert_eq!(check_config_at(&p), "fine");
        // broken: kept, a fresh one written
        std::fs::write(&p, "{ not json").unwrap();
        assert!(check_config_at(&p).starts_with("broken"));
        assert_eq!(asides(&d.join("tessera"), "broken").len(), 1);
        assert_eq!(check_config_at(&p), "fine");
        // (an empty file is nothing to keep)
        std::fs::write(&p, "").unwrap();
        assert_eq!(check_config_at(&p), "fine");
        assert_eq!(asides(&d.join("tessera"), "broken").len(), 1);
    }

    #[test]
    fn a_linked_file_stays_a_link() {
        let d = tmp("link");
        std::fs::create_dir_all(d.join("dotfiles")).unwrap();
        std::fs::create_dir_all(d.join("cfg")).unwrap();
        std::fs::write(d.join("dotfiles/config.json"), "{\"a\": 1}").unwrap();
        std::os::unix::fs::symlink(d.join("dotfiles/config.json"), d.join("cfg/config.json")).unwrap();
        edit_config_at(&d.join("cfg/config.json"), |m| put_path(m, "b", json!(2))).unwrap();
        assert!(std::fs::symlink_metadata(d.join("cfg/config.json")).unwrap().file_type().is_symlink());
        let v: Value = serde_json::from_str(&std::fs::read_to_string(d.join("dotfiles/config.json")).unwrap()).unwrap();
        assert_eq!(v, json!({"a": 1, "b": 2}));
    }

    #[test]
    fn permissions_are_kept() {
        use std::os::unix::fs::PermissionsExt;
        let d = tmp("perm");
        let p = d.join("config.json");
        std::fs::write(&p, "{}").unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600)).unwrap();
        edit_config_at(&p, |m| put_path(m, "a", json!(1))).unwrap();
        assert_eq!(std::fs::metadata(&p).unwrap().permissions().mode() & 0o777, 0o600);
    }

    #[test]
    fn state_writers_never_drop_each_others_keys() {
        let d = tmp("state");
        let p = d.join("state.json");
        std::fs::write(&p, r#"{"todos": [{"text": "milk"}], "_tessera_revisions": {"todos": [1, "a", 1]}}"#).unwrap();
        // many writers at once, each its own key (threads; processes share the flock)
        let hs: Vec<_> = (0..16)
            .map(|i| {
                let p = p.clone();
                std::thread::spawn(move || {
                    for j in 0..10 {
                        edit_state_at(&p, |m| {
                            m.insert(format!("k{i}"), json!(j));
                            true
                        })
                        .unwrap();
                    }
                })
            })
            .collect();
        for h in hs {
            h.join().unwrap();
        }
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        for i in 0..16 {
            assert_eq!(v[format!("k{i}")], json!(9), "k{i}");
        }
        assert_eq!(v["todos"][0]["text"], json!("milk"));
        assert!(v["_tessera_revisions"].is_object());
    }

    #[test]
    fn a_corrupt_state_file_is_set_aside_as_the_merge_does() {
        let d = tmp("statebad");
        let p = d.join("state.json");
        std::fs::write(&p, "{\"todos\": [").unwrap();
        edit_state_at(&p, |m| {
            m.insert("screenTime".into(), json!({}));
            true
        })
        .unwrap();
        assert_eq!(asides(&d, "state.json.corrupt-").len(), 1);
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!(v, json!({"screenTime": {}}));
    }

    #[test]
    fn a_queued_change_reads_at_once_and_lands_in_order() {
        let d = tmp("later");
        let p = d.join("state.json");
        std::fs::write(&p, r#"{"keep": 1}"#).unwrap();
        // (held back: the lane's first job waits until the reads are checked)
        let (go, wait) = std::sync::mpsc::channel::<()>();
        FILES.run(move || {
            let _ = wait.recv_timeout(Duration::from_secs(5));
        });
        for i in 0..5 {
            let change = move |m: &mut Map<String, Value>| {
                m.insert("n".into(), json!(i));
                true
            };
            edit_later(&p, Kind::State, std::sync::Arc::new(change), "n".into());
        }
        let m = read_object(&p);
        assert_eq!((m["keep"].clone(), m["n"].clone()), (json!(1), json!(4)));
        assert_eq!(std::fs::read_to_string(&p).unwrap(), r#"{"keep": 1}"#);
        go.send(()).unwrap();
        flush();
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!(v, json!({"keep": 1, "n": 4}));
        assert!(!pending_for(&p));
    }

    #[test]
    fn a_kept_read_follows_the_file() {
        let d = tmp("kept");
        let p = d.join("state.json");
        std::fs::write(&p, r#"{"a": 1}"#).unwrap();
        assert_eq!(read_object(&p)["a"], json!(1));
        assert_eq!(read_object(&p)["a"], json!(1));
        // (another writer: a new file renamed over it)
        crate::util::write_atomic(&p, br#"{"a": 2}"#).unwrap();
        assert_eq!(read_object(&p)["a"], json!(2));
    }

    #[test]
    fn paths_put_through_odd_sections() {
        let mut m = Map::new();
        m.insert("dock".into(), json!("yes"));
        m.insert("menubar".into(), json!(null));
        assert!(put_path(&mut m, "dock.iconSize", json!(50)));
        assert!(put_path(&mut m, "menubar.a.b", json!(true)));
        assert!(put_path(&mut m, "top", json!(1)));
        assert!(!put_path(&mut m, "top", json!(1)));
        assert_eq!(Value::Object(m), json!({"dock": {"iconSize": 50}, "menubar": {"a": {"b": true}}, "top": 1}));
    }
}

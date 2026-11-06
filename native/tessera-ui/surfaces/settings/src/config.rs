//! Settings' model of config.json, as shell/common/Config.qml has it: the
//! same keys and the same defaults (read from Config.qml itself, compiled in,
//! so the two can't drift), every change written back whole and atomically
//! (a temporary file renamed over it), a moment after the last change (a
//! slider's drag is one write, as Config.qml's 300 ms timer). The QML shell
//! watches the file and takes the change; so does this process (main.rs's
//! watcher), which then reads it again for every surface.
//!
//! Paths are dotted keys: "dock.enabled", "appearance.glassFrost".

use serde_json::{Map, Value};
use std::sync::mpsc;
use std::sync::OnceLock;
use std::time::Duration;

const CONFIG_QML: &str = include_str!("../../../../../shell/common/Config.qml");

/// The text before a `//` comment, outside strings ("http://…" stays)
fn strip_comment(line: &str) -> &str {
    let mut in_str = false;
    let mut esc = false;
    let b = line.as_bytes();
    for i in 0..b.len() {
        let c = b[i];
        if in_str {
            if esc {
                esc = false;
            } else if c == b'\\' {
                esc = true;
            } else if c == b'"' {
                in_str = false;
            }
        } else if c == b'"' {
            in_str = true;
        } else if c == b'/' && b.get(i + 1) == Some(&b'/') {
            return &line[..i];
        }
    }
    line
}

/// Config.qml's JsonAdapter as JSON: `property JsonObject x: JsonObject {`
/// opens an object, `property <type> name: <value>` is a key with its
/// default (a JSON literal: strings, numbers, booleans, lists)
pub fn parse_defaults(qml: &str) -> Value {
    let mut root = Map::new();
    // the objects open now: (name, brace depth at which it closes)
    let mut stack: Vec<(String, i32)> = Vec::new();
    let mut depth = 0i32;
    let mut started = false;
    fn at<'a>(root: &'a mut Map<String, Value>, stack: &[(String, i32)]) -> &'a mut Map<String, Value> {
        let mut m = root;
        for (k, _) in stack {
            m = m.entry(k.clone()).or_insert_with(|| Value::Object(Map::new())).as_object_mut().unwrap();
        }
        m
    }
    for raw in qml.lines() {
        let line = strip_comment(raw).trim();
        if line.starts_with("JsonAdapter {") {
            started = true;
            depth = 0;
            continue;
        }
        if !started {
            continue;
        }
        let opens = line.matches('{').count() as i32;
        let closes = line.matches('}').count() as i32;
        if let Some(rest) = line.strip_prefix("property JsonObject ") {
            let name = rest.split(':').next().unwrap_or("").trim().to_string();
            depth += opens - closes;
            stack.push((name, depth - 1));
            continue;
        }
        if let Some(rest) = line.strip_prefix("property ") {
            // "<type> name: value"
            if let Some((decl, value)) = rest.split_once(':') {
                let name = decl.split_whitespace().last().unwrap_or("").to_string();
                let real = decl.split_whitespace().next() == Some("real");
                let value = value.trim().trim_end_matches(';').trim();
                if let Ok(mut v) = serde_json::from_str::<Value>(value) {
                    // (a real whose default is written whole, `0`: still a real)
                    if real && v.is_i64() {
                        v = serde_json::Number::from_f64(v.as_f64().unwrap_or(0.0)).map(Value::Number).unwrap_or(v);
                    }
                    at(&mut root, &stack).insert(name, v);
                }
            }
        }
        depth += opens - closes;
        while stack.last().is_some_and(|(_, d)| depth <= *d) {
            stack.pop();
        }
        if depth < 0 {
            break; // (the adapter's own closing brace)
        }
    }
    Value::Object(root)
}

/// The defaults (Config.qml's), parsed once
pub fn defaults() -> &'static Value {
    static D: OnceLock<Value> = OnceLock::new();
    D.get_or_init(|| parse_defaults(CONFIG_QML))
}

/// The value at a dotted path, or Null
pub fn lookup<'a>(v: &'a Value, path: &str) -> &'a Value {
    let mut v = v;
    for k in path.split('.') {
        v = &v[k];
    }
    v
}

/// Set a dotted path, making objects on the way; false when it was already so
pub fn set_path(cfg: &mut Value, path: &str, value: Value) -> bool {
    if !cfg.is_object() {
        *cfg = Value::Object(Map::new());
    }
    let keys: Vec<&str> = path.split('.').collect();
    let mut v = cfg;
    for k in &keys[..keys.len() - 1] {
        if !v[*k].is_object() {
            v[*k] = Value::Object(Map::new());
        }
        v = &mut v[*k];
    }
    let last = keys[keys.len() - 1];
    if v[last] == value {
        return false;
    }
    v[last] = value;
    true
}

/// A number for a key: an integer where Config.qml's property is an int
/// (`property int`), else as it is
pub fn number_for(path: &str, n: f64) -> Value {
    if lookup(defaults(), path).is_i64() || lookup(defaults(), path).is_u64() {
        Value::from(n.round() as i64)
    } else {
        serde_json::Number::from_f64(n).map(Value::Number).unwrap_or(Value::Null)
    }
}

/// config.json written whole, as Qt writes it (4-space indent)
pub fn to_text(cfg: &Value) -> String {
    use serde::Serialize;
    let mut out = Vec::new();
    let fmt = serde_json::ser::PrettyFormatter::with_indent(b"    ");
    let mut ser = serde_json::Serializer::with_formatter(&mut out, fmt);
    if cfg.serialize(&mut ser).is_err() {
        return String::new();
    }
    String::from_utf8(out).unwrap_or_default() + "\n"
}

/// Settings' view of config.json: the file as last read with what this
/// window changed since laid over it, and the defaults beneath
pub struct Cfg {
    pub file: Value,
    tx: Option<mpsc::Sender<(String, Value)>>,
}

impl Default for Cfg {
    fn default() -> Self {
        Self { file: Value::Null, tx: None }
    }
}

impl Cfg {
    pub fn new(file: Value) -> Self {
        Self { file, tx: Some(writer()) }
    }

    /// For tests: no file is written
    pub fn detached(file: Value) -> Self {
        Self { file, tx: None }
    }

    /// The file was read again (the shell or another window changed it)
    pub fn reload(&mut self, file: Value) {
        self.file = file;
    }

    /// The setting, or Config.qml's default
    pub fn get(&self, path: &str) -> Value {
        let v = lookup(&self.file, path);
        if v.is_null() {
            lookup(defaults(), path).clone()
        } else {
            v.clone()
        }
    }
    pub fn b(&self, path: &str) -> bool {
        match self.get(path) {
            Value::Bool(b) => b,
            Value::String(s) => s == "true",
            Value::Number(n) => n.as_f64().unwrap_or(0.0) != 0.0,
            _ => false,
        }
    }
    pub fn s(&self, path: &str) -> String {
        match self.get(path) {
            Value::String(s) => s,
            Value::Null => String::new(),
            v => v.to_string(),
        }
    }
    pub fn f(&self, path: &str) -> f64 {
        match self.get(path) {
            Value::Number(n) => n.as_f64().unwrap_or(0.0),
            Value::String(s) => s.parse().unwrap_or(0.0),
            Value::Bool(b) => b as i32 as f64,
            _ => 0.0,
        }
    }
    pub fn i(&self, path: &str) -> i64 {
        self.f(path).round() as i64
    }
    pub fn list(&self, path: &str) -> Vec<String> {
        self.get(path).as_array().map(|a| a.iter().map(|x| x.as_str().map(str::to_string).unwrap_or_else(|| x.to_string())).collect()).unwrap_or_default()
    }

    /// Change a setting: at once here, written to the file a moment later
    pub fn set(&mut self, path: &str, value: Value) {
        if !set_path(&mut self.file, path, value.clone()) {
            return;
        }
        if let Some(tx) = &self.tx {
            let _ = tx.send((path.to_string(), value));
        }
    }
}

/// The writer: one thread for every Settings change, so two changes in a
/// row never race. It waits for a quiet moment (as Config.qml's 300 ms
/// timer), reads the file as it is now, lays the changes over it and
/// renames the whole of it into place
fn writer() -> mpsc::Sender<(String, Value)> {
    static TX: OnceLock<std::sync::Mutex<mpsc::Sender<(String, Value)>>> = OnceLock::new();
    TX.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<(String, Value)>();
        std::thread::spawn(move || {
            while let Ok(first) = rx.recv() {
                let mut pending = vec![first];
                while let Ok(next) = rx.recv_timeout(Duration::from_millis(250)) {
                    pending.push(next);
                }
                let path = crate::theme::config_path();
                let mut cfg: Value = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null);
                if !cfg.is_object() {
                    cfg = defaults().clone();
                }
                let mut changed = false;
                for (k, v) in pending {
                    changed |= set_path(&mut cfg, &k, v);
                }
                if changed {
                    let text = to_text(&cfg);
                    if text.is_empty() {
                        continue;
                    }
                    if let Err(e) = crate::util::write_atomic(&path, text.as_bytes()) {
                        eprintln!("tessera-ui: settings: couldn't save: {e}");
                    }
                }
            }
        });
        std::sync::Mutex::new(tx)
    })
    .lock()
    .unwrap()
    .clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn defaults_are_config_qmls() {
        let d = defaults();
        assert_eq!(d["dock"]["enabled"], json!(true));
        assert_eq!(d["dock"]["iconSize"], json!(48));
        assert_eq!(d["dock"]["gap"], json!(0.18));
        assert_eq!(d["dock"]["pinned"][0], json!("org.kde.dolphin"));
        assert_eq!(d["appearance"]["look"], json!("glass"));
        assert_eq!(d["appearance"]["glass"], json!("frosted"));
        assert_eq!(d["menubar"]["utilButtons"], json!(["screenshot"]));
        assert_eq!(d["ai"]["ollamaUrl"], json!("http://localhost:11434"));
        assert_eq!(d["rust"]["surfaces"], json!([]));
        assert_eq!(d["defaultsVersion"], json!(0));
        assert_eq!(d["lock"]["clockWeight"], json!(640));
        assert_eq!(d["power"]["idleSleepBattery"], json!(900));
        assert!(d["appearance"]["materialTransparency"].is_f64());
        assert_eq!(number_for("appearance.materialTransparency", 0.35), json!(0.35));
        assert_eq!(number_for("dock.iconSize", 51.6), json!(52));
        // (every section, none lost to a brace in a comment or a string)
        for k in ["google", "fastPair", "phone", "cloud", "appearance", "lock", "notificationCenter", "sharing", "audio", "sounds", "recorder", "screenshot", "wallpaper", "menubar", "controlCenter", "ai", "overlays", "sidebar", "desktop", "welcome", "today", "weather", "island", "windowRules", "defaultApps", "notes", "emoji", "resources", "network", "power", "cooling", "performance", "keyboard", "shelf", "dictation", "captions", "findPointer", "workspaces", "mouse", "trackpad", "displays", "switcher", "privacy", "hotCorners", "overview", "loginItems", "stageManager", "dock", "rust"] {
            assert!(d[k].is_object(), "no {k}");
        }
        assert!(d["dock"].get("google").is_none() && d.get("taskListId").is_none());
    }

    #[test]
    fn reads_fall_back_to_the_defaults() {
        let c = Cfg::detached(json!({"dock": {"iconSize": 60}, "menubar": {"autoHide": true}}));
        assert_eq!(c.i("dock.iconSize"), 60);
        assert!(c.b("dock.enabled"));
        assert!(c.b("menubar.autoHide"));
        assert_eq!(c.s("dock.autoHide"), "off");
        assert_eq!(c.list("dock.pinned").len(), 7);
        assert_eq!(c.f("dock.gap"), 0.18);
        assert_eq!(c.s("nothing.here"), "");
    }

    #[test]
    fn changes_land_at_their_path() {
        let mut c = Cfg::detached(json!({}));
        c.set("dock.iconSize", number_for("dock.iconSize", 52.0));
        c.set("appearance.glassFrost", number_for("appearance.glassFrost", 0.5));
        assert_eq!(c.file, json!({"dock": {"iconSize": 52}, "appearance": {"glassFrost": 0.5}}));
        assert!(!set_path(&mut c.file, "dock.iconSize", json!(52)));
    }

    #[test]
    fn written_as_qt_writes_it() {
        assert_eq!(to_text(&json!({"a": {"b": 1}})), "{\n    \"a\": {\n        \"b\": 1\n    }\n}\n");
    }

    #[test]
    fn comments_end_outside_strings() {
        assert_eq!(strip_comment(r#"property string u: "http://x" // a url"#).trim(), r#"property string u: "http://x""#);
    }
}

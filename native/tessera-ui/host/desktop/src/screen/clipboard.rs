//! The clipboard's write side (shell/services/Clipboard.qml). The history
//! itself is read by core (services/clipboard.rs: cliphist's list, previews,
//! the pins and times files); what only the shell did is here:
//!   * `clipboard.refresh`, `.pin`, `.unpin` (the panel's calls);
//!   * stamping when each entry was copied (clip-times.json beside state.json:
//!     a thread watches cliphist's database, stamps ids not seen before with
//!     now, all as "from before" on the very first run, forgets dropped ones);
//!   * pins in state.json's `clipPins`, pinned pictures copied to clip-pins/;
//!   * copy / paste / remove / clear / copyText, for Search, Live Text and
//!     Dictation (pub fns).

use super::util;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;
use tessera_core::host::{Host, Service};
use tessera_core::services::clipboard as cb;

const TERMINALS: &[&str] = &["foot", "kitty", "alacritty", "org.wezfurlong.wezterm", "com.mitchellh.ghostty", "konsole", "org.gnome.ptyxis", "xterm"];

pub fn pin_dir() -> PathBuf {
    tessera_core::util::state_dir().join("clip-pins")
}

fn db() -> String {
    cb::db_path()
}

/// The ids cliphist lists, newest first (200 at most, as the QML keeps)
fn list_ids(db: &str) -> Vec<String> {
    let out = Command::new("cliphist").args(["-db-path", db, "list"]).stdin(Stdio::null()).stderr(Stdio::null()).output();
    let text = out.map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    text.lines().filter_map(|l| l.split_once('\t').map(|(id, _)| id.to_string())).take(200).collect()
}

fn stamps_path() -> PathBuf {
    tessera_core::util::state_dir().join("clip-times.json")
}

/// Clipboard.stampRows: stamp the ids not seen before, forget the dropped;
/// true when the file changed
pub fn stamp_rows(stamps: &mut HashMap<String, i64>, existed: &mut bool, ids: &[String], now: i64) -> bool {
    let fresh = !*existed && stamps.is_empty();
    let mut next = HashMap::new();
    let mut changed = false;
    for id in ids {
        match stamps.get(id) {
            Some(t) => {
                next.insert(id.clone(), *t);
            }
            None => {
                next.insert(id.clone(), if fresh { 0 } else { now });
                changed = true;
            }
        }
    }
    if changed || next.len() != stamps.len() {
        *stamps = next;
        *existed = true;
        return true;
    }
    false
}

/// The watcher thread: lists when the database's time changes (or asked)
fn watch(ask: mpsc::Receiver<()>) {
    let db = db();
    let (mut stamps, mut existed): (HashMap<String, i64>, bool) = match std::fs::read_to_string(stamps_path()) {
        Ok(t) => match serde_json::from_str::<Value>(&t) {
            Ok(v) => (v.as_object().into_iter().flatten().filter_map(|(k, t)| Some((k.clone(), t.as_f64()? as i64))).collect(), true),
            Err(_) => (HashMap::new(), false),
        },
        Err(_) => (HashMap::new(), false),
    };
    let mut seen = None;
    let mut first = true;
    loop {
        let asked = match ask.recv_timeout(Duration::from_millis(600)) {
            Ok(()) => true,
            Err(mpsc::RecvTimeoutError::Timeout) => false,
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        };
        let m = std::fs::metadata(&db).and_then(|m| m.modified()).ok();
        if !(asked || first || m != seen) {
            continue;
        }
        first = false;
        seen = m;
        if !util::has("cliphist") {
            continue;
        }
        std::thread::sleep(Duration::from_millis(if asked { 0 } else { 300 }));
        let ids = list_ids(&db);
        if stamp_rows(&mut stamps, &mut existed, &ids, tessera_core::util::now_ms()) {
            let j: serde_json::Map<String, Value> = stamps.iter().map(|(k, v)| (k.clone(), json!(v))).collect();
            let _ = tessera_core::util::write_atomic(&stamps_path(), Value::Object(j).to_string().as_bytes());
            // (core reads the times: once more, now they're in)
            cb::refresh();
        }
    }
}

// ── Copy and paste (pub: Search, Live Text, Dictation) ──────────────────

/// Copy an item (history row by id, or a pin by image/text). plain: text only
pub fn copy(id: Option<&str>, image: Option<&str>, text: &str, plain: bool) {
    let db = db();
    if let (Some(id), None) = (id, image) {
        let cmd = format!("cliphist -db-path \"$2\" decode \"$1\" | wl-copy{}", if plain { " --type text/plain" } else { "" });
        util::detached(&["sh", "-c", &cmd, "sh", id, &db]);
    } else if let (Some(id), Some(_)) = (id, image) {
        util::detached(&["sh", "-c", "cliphist -db-path \"$2\" decode \"$1\" | wl-copy", "sh", id, &db]);
    } else if let Some(img) = image {
        util::detached(&["sh", "-c", "wl-copy < \"$1\"", "sh", img]);
    } else {
        util::detached(&["wl-copy", "--type", "text/plain", text]);
    }
}

/// Copy, then paste into the window that had focus before the panel opened
/// (Ctrl+V; terminals Ctrl+Shift+V) after the panel releases the keyboard
pub fn paste(h: &mut Host, id: Option<&str>, image: Option<&str>, text: &str, plain: bool) {
    copy(id, image, text, plain);
    let class = h.st.shell.get::<tessera_core::services::clients::Windows>().and_then(|w| w.active_client()).map(|c| c.class.to_lowercase()).unwrap_or_default();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(180));
        let mods = if TERMINALS.contains(&class.as_str()) { "CTRL SHIFT" } else { "CTRL" };
        let _ = tessera_core::services::hypr::request(&format!("dispatch hl.dsp.send_shortcut({{ mods = \"{mods}\", key = \"V\" }})"));
    });
}

pub fn remove(id: &str, text: &str, image: Option<&str>) {
    util::detached(&["sh", "-c", "printf '%s\\t%s\\n' \"$1\" \"$2\" | cliphist -db-path \"$3\" delete", "sh", id, text, &db()]);
    if let Some(i) = image {
        util::detached(&["rm", "-f", "--", i]);
    }
    cb::refresh();
}

pub fn clear() {
    util::detached(&["cliphist", "-db-path", &db(), "wipe"]);
    util::detached(&["sh", "-c", "rm -f -- \"$1\"/*", "sh", &cb::preview_dir().to_string_lossy()]);
    cb::refresh();
}

// ── Pins ─────────────────────────────────────────────────────────────────

fn pins() -> Vec<Value> {
    util::state_get("clipPins").as_array().cloned().unwrap_or_default()
}

fn pin_matches(x: &Value, p: &Value) -> bool {
    x == p || (x["text"].is_string() && x["text"] == p["text"]) || (x["image"].as_str().is_some_and(|s| !s.is_empty()) && x["image"] == p["image"])
}

/// Clipboard.unpin
pub fn unpin(p: &Value) {
    let keep: Vec<Value> = pins().into_iter().filter(|x| !pin_matches(x, p)).collect();
    util::state_set("clipPins", Value::Array(keep));
    if let Some(i) = p["image"].as_str() {
        util::detached(&["rm", "-f", i]);
    }
    cb::refresh();
}

/// Message from the decode thread: a text to pin in full
struct PinText(String);

pub struct ClipboardSvc {
    ask: Option<mpsc::Sender<()>>,
}

impl ClipboardSvc {
    pub fn new() -> Self {
        Self { ask: None }
    }

    fn pin(&mut self, h: &mut Host, item: &Value) {
        let id = item["id"].as_str().map(String::from).or_else(|| item["id"].as_i64().map(|n| n.to_string())).unwrap_or_default();
        if id.is_empty() {
            return;
        }
        let text = item["text"].as_str().unwrap_or("").to_string();
        let image = item["image"].as_str().unwrap_or("").to_string();
        let entry = cb::Entry { id: id.clone(), text: text.clone(), image: (!image.is_empty()).then(|| PathBuf::from(&image)) };
        let cur: Vec<cb::Pin> = cb::parse_pins(&json!({"clipPins": pins()}).to_string());
        if cb::is_pinned(&entry, &cur) {
            return;
        }
        if !image.is_empty() {
            let ext = image.rsplit('.').next().unwrap_or("png").to_string();
            let dest = format!("{}/{}.{}", pin_dir().display(), tessera_core::util::now_ms(), ext);
            let dir = pin_dir().to_string_lossy().into_owned();
            // (the copy first: the pin points at it)
            let _ = Command::new("sh").args(["-c", "umask 077; mkdir -p \"$1\" && cp \"$2\" \"$3\"", "sh", &dir, &image, &dest]).status();
            let mut next = vec![json!({"image": dest, "source": id})];
            next.extend(pins());
            util::state_set("clipPins", Value::Array(next));
            cb::refresh();
        } else {
            // cliphist list truncates long text; pin the full entry
            let send = h.sender("Clipboard");
            let db = db();
            std::thread::spawn(move || {
                let out = Command::new("cliphist").args(["-db-path", &db, "decode", &id]).stdin(Stdio::null()).stderr(Stdio::null()).output();
                if let Ok(o) = out {
                    let t = String::from_utf8_lossy(&o.stdout).into_owned();
                    if !t.is_empty() {
                        send(Box::new(PinText(t)));
                    }
                }
            });
        }
    }
}

impl Default for ClipboardSvc {
    fn default() -> Self {
        Self::new()
    }
}

impl Service for ClipboardSvc {
    fn name(&self) -> &'static str {
        "Clipboard"
    }
    fn calls(&self) -> &'static [&'static str] {
        &["clipboard"]
    }
    fn start(&mut self, h: &mut Host) {
        h.st.shell.want("clipboard");
        h.st.shell.want("clients");
        let (tx, rx) = mpsc::channel();
        self.ask = Some(tx);
        std::thread::spawn(move || watch(rx));
    }
    fn call(&mut self, h: &mut Host, call: &str, args: &Value) {
        match call {
            "clipboard.refresh" => {
                // (it stamps new entries with when they came)
                cb::refresh();
                if let Some(a) = &self.ask {
                    let _ = a.send(());
                }
            }
            "clipboard.pin" if args[0]["id"].is_string() || args[0]["id"].is_number() => self.pin(h, &args[0]),
            "clipboard.unpin" if args[0].is_object() => unpin(&args[0]),
            _ => {}
        }
    }
    fn message(&mut self, _h: &mut Host, msg: Box<dyn std::any::Any + Send>) {
        if let Ok(p) = msg.downcast::<PinText>() {
            let mut next = vec![json!({"text": p.0})];
            next.extend(pins());
            util::state_set("clipPins", Value::Array(next));
            cb::refresh();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_run_is_from_before_then_new_entries_get_now() {
        let (mut s, mut existed) = (HashMap::new(), false);
        let ids = vec!["2".to_string(), "1".to_string()];
        assert!(stamp_rows(&mut s, &mut existed, &ids, 1000));
        assert_eq!(s["1"], 0);
        let ids = vec!["3".to_string(), "2".to_string()];
        assert!(stamp_rows(&mut s, &mut existed, &ids, 2000));
        assert_eq!((s["3"], s.contains_key("1")), (2000, false));
        assert!(!stamp_rows(&mut s, &mut existed, &ids, 3000));
    }
}

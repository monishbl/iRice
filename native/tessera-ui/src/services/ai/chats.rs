//! Chat history, in the shell's own files (shell/services/AiChats.js and
//! Ai.qml), so switching between the QML sidebar and this one keeps every
//! chat. Each chat is a session file next to the UI state:
//!   ai-chats/<id>.json  { id, title, created, updated, model, modelName, pinned, rows, history }
//! and ai-chats/index.json lists them lightly:
//!   { version, current, trash, chats: [{ id, title, created, updated, model, modelName, pinned, count, preview }] }
//! (times in ms; `model` is the model's key; pictures are never stored).
//! Both shells may write the index: every change here reads it from disk,
//! changes it and writes it back whole (owner-only, a temporary file renamed
//! over it), so the other's changes are kept.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub const TITLE_MAX: usize = 60;
pub const PREVIEW_MAX: usize = 300;
const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
const DAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
pub const DAY: i64 = 86_400_000;

/// A chat row as the sidebar shows it (and as it's saved)
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Row {
    pub role: String,
    pub text: String,
    pub thinking: String,
    pub status: String,
    pub model: String,
    pub image: String,
    pub search: String,
}

impl Row {
    pub fn new(role: &str, text: &str, status: &str) -> Self {
        Row { role: role.into(), text: text.into(), status: status.into(), ..Default::default() }
    }
    /// A saved row, read tolerantly (Ai.qml's fillRows)
    pub fn from_value(r: &Value) -> Self {
        let t = |k: &str| r[k].as_str().unwrap_or("").to_string();
        let or = |s: String, d: &str| if s.is_empty() { d.to_string() } else { s };
        Row { role: or(t("role"), "note"), text: t("text"), thinking: t("thinking"), status: or(t("status"), "done"), model: t("model"), image: t("image"), search: t("search") }
    }
}

/// A line in the index
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    pub title: String,
    pub created: i64,
    pub updated: i64,
    pub model: String,
    #[serde(rename = "modelName")]
    pub model_name: String,
    pub pinned: bool,
    pub count: i64,
    pub preview: String,
}

/// The whole saved chat
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub created: i64,
    pub updated: i64,
    pub model: String,
    #[serde(rename = "modelName")]
    pub model_name: String,
    pub pinned: bool,
    pub rows: Vec<Row>,
    pub history: Vec<Value>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Index {
    pub current: String,
    pub trash: Vec<String>,
    pub chats: Vec<Entry>,
}

// ── Pure parts (AiChats.js) ─────────────────────────────────────────────

/// File names are ids: only these characters, so a damaged index can't name
/// a path outside the folder
pub fn valid_id(id: &str) -> bool {
    (1..=64).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn base36(mut n: u64) -> String {
    let digits = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut out = Vec::new();
    loop {
        out.push(digits[(n % 36) as usize]);
        n /= 36;
        if n == 0 {
            break;
        }
    }
    out.reverse();
    String::from_utf8(out).unwrap()
}

/// now in base 36, a dash, a random number under 36⁴ in base 36
pub fn new_id(now: i64, random: f64) -> String {
    format!("{}-{}", base36(now.max(0) as u64), base36((random * 1_679_616.0).floor() as u64))
}

/// UTF-16 length (JS's .length)
fn js_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// JS's .slice(0, n)
fn js_head(s: &str, n: usize) -> String {
    let mut used = 0;
    s.chars().take_while(|c| {
        used += c.len_utf16();
        used <= n
    })
    .collect()
}

/// One line, spaces collapsed
pub fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The first thing the person asked, on one line, cut at a word near 60
/// characters
pub fn title_from(text: &str) -> String {
    let line = text.split('\n').map(one_line).find(|l| !l.is_empty()).unwrap_or_default();
    if line.is_empty() {
        return "New chat".into();
    }
    if js_len(&line) <= TITLE_MAX {
        return line;
    }
    let cut = js_head(&line, TITLE_MAX);
    let space = cut.rfind(' ').map(|b| js_len(&cut[..b]));
    let kept = match space {
        Some(s) if s as f64 > TITLE_MAX as f64 * 0.6 => js_head(&cut, s),
        _ => cut,
    };
    kept.trim_end_matches(|c: char| c.is_whitespace() || ",.;:!?-".contains(c)).to_string() + "…"
}

pub fn clean_title(text: &str) -> String {
    js_head(&one_line(text), 120)
}

/// What search reads besides the title: the words of the chat, on one line
pub fn preview_of(rows: &[Row]) -> String {
    let mut out = String::new();
    for r in rows.iter().filter(|r| r.role == "user" || r.role == "assistant") {
        if !out.is_empty() {
            out.push(' ');
        }
        out += &one_line(&r.text);
        if js_len(&out) >= PREVIEW_MAX {
            break;
        }
    }
    js_head(&out, PREVIEW_MAX)
}

pub fn message_count(rows: &[Row]) -> i64 {
    rows.iter().filter(|r| r.role == "user" || r.role == "assistant").count() as i64
}

/// A chat is kept once the person has sent something in it
pub fn worth_keeping(rows: &[Row]) -> bool {
    rows.iter().any(|r| r.role == "user")
}

/// Pictures are never stored: a saved turn says there was one
pub fn strip_pictures(history: &[Value]) -> Vec<Value> {
    history
        .iter()
        .map(|h| {
            let n = h["images"].as_array().map_or(if h["image"].is_object() { 1 } else { 0 }, |a| a.len());
            if n == 0 {
                return h.clone();
            }
            let what = if n == 1 { "A picture was".to_string() } else { format!("{n} pictures were") };
            json!({"role": h["role"], "text": format!("[{what} attached here]\n{}", h["text"].as_str().unwrap_or(""))})
        })
        .collect()
}

pub fn entry_of(s: &Session) -> Entry {
    Entry {
        id: s.id.clone(),
        title: if s.title.is_empty() { "New chat".into() } else { s.title.clone() },
        created: s.created,
        updated: s.updated,
        model: s.model.clone(),
        model_name: s.model_name.clone(),
        pinned: s.pinned,
        count: message_count(&s.rows),
        preview: preview_of(&s.rows),
    }
}

/// The index with this entry added or replaced, newest first
pub fn upsert(chats: &[Entry], entry: Entry) -> Vec<Entry> {
    let mut out: Vec<Entry> = chats.iter().filter(|c| c.id != entry.id).cloned().collect();
    out.push(entry);
    out.sort_by(|a, b| b.updated.cmp(&a.updated));
    out
}

pub fn without(chats: &[Entry], id: &str) -> Vec<Entry> {
    chats.iter().filter(|c| c.id != id).cloned().collect()
}

fn num(v: &Value) -> Option<i64> {
    v.as_f64().map(|f| f as i64)
}

/// A tolerant read of index.json: anything unusable is left out
pub fn parse_index(text: &str) -> Option<Index> {
    let j: Value = serde_json::from_str(text).ok()?;
    let list = j.get("chats")?.as_array()?;
    let mut seen = std::collections::HashSet::new();
    let mut chats: Vec<Entry> = Vec::new();
    for c in list {
        let Some(id) = c["id"].as_str().filter(|id| valid_id(id)) else { continue };
        let Some(updated) = num(&c["updated"]) else { continue };
        if !seen.insert(id.to_string()) {
            continue;
        }
        let s = |k: &str| c[k].as_str().unwrap_or("").to_string();
        chats.push(Entry {
            id: id.into(),
            title: c["title"].as_str().filter(|t| !t.is_empty()).unwrap_or("New chat").into(),
            created: num(&c["created"]).unwrap_or(updated),
            updated,
            model: s("model"),
            model_name: s("modelName"),
            pinned: c["pinned"] == true,
            count: num(&c["count"]).unwrap_or(0),
            preview: s("preview"),
        });
    }
    chats.sort_by(|a, b| b.updated.cmp(&a.updated));
    let current = j["current"].as_str().filter(|c| valid_id(c) && seen.contains(*c)).unwrap_or("").to_string();
    let trash = j["trash"].as_array().into_iter().flatten().filter_map(|t| t.as_str()).filter(|t| valid_id(t)).map(String::from).collect();
    Some(Index { current, trash, chats })
}

/// index.json's text, as JSON.stringify writes it
pub fn index_text(idx: &Index) -> String {
    #[derive(Serialize)]
    struct Out<'a> {
        version: i32,
        current: &'a str,
        trash: &'a [String],
        chats: &'a [Entry],
    }
    serde_json::to_string(&Out { version: 1, current: &idx.current, trash: &idx.trash, chats: &idx.chats }).unwrap()
}

/// Chats older than `days` (by their last message), except pinned ones and
/// those in `keep`: what to delete. 0 days: none.
pub fn prune(chats: &[Entry], days: i64, now: i64, keep: &[&str]) -> Vec<String> {
    if days <= 0 {
        return Vec::new();
    }
    let before = now - days * DAY;
    chats.iter().filter(|c| !c.pinned && c.updated < before && !keep.contains(&c.id.as_str())).map(|c| c.id.clone()).collect()
}

// ── Search ──────────────────────────────────────────────────────────────

fn words(query: &str) -> Vec<String> {
    one_line(&query.to_lowercase()).split(' ').filter(|w| !w.is_empty()).map(String::from).collect()
}

/// Every word of the query, somewhere in the title or the chat's words
pub fn matches(entry: &Entry, query: &str) -> bool {
    let hay = format!("{} {}", entry.title, entry.preview).to_lowercase();
    words(query).iter().all(|w| hay.contains(w.as_str()))
}

/// `found`: ids a search through the whole files also matched
pub fn filter(chats: &[Entry], query: &str, found: &[String]) -> Vec<Entry> {
    chats.iter().filter(|c| matches(c, query) || found.contains(&c.id)).cloned().collect()
}

/// The chats whose title or messages hold every word (ai-chat-search.py,
/// here: no process for it)
pub fn search_files(dir: &Path, query: &str) -> Vec<String> {
    let want = words(query);
    if want.is_empty() {
        return Vec::new();
    }
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "json")).collect();
    paths.sort();
    let mut found = Vec::new();
    for p in paths {
        let Some(stem) = p.file_stem().and_then(|s| s.to_str()) else { continue };
        if !valid_id(stem) || stem == "index" {
            continue;
        }
        let Ok(chat) = std::fs::read_to_string(&p).map(|t| serde_json::from_str::<Value>(&t)) else { continue };
        let Ok(chat) = chat else { continue };
        if !chat.is_object() {
            continue;
        }
        let mut hay = chat["title"].as_str().unwrap_or("").to_string();
        for r in chat["rows"].as_array().into_iter().flatten() {
            if r["role"] == "user" || r["role"] == "assistant" {
                hay.push(' ');
                hay += r["text"].as_str().unwrap_or("");
            }
        }
        let hay = hay.to_lowercase();
        if want.iter().all(|w| hay.contains(w.as_str())) {
            found.push(stem.to_string());
        }
    }
    found
}

// ── Groups and times (local time) ───────────────────────────────────────

struct Local {
    year: i32,
    month: usize,
    day: i32,
    weekday: usize,
}

fn local(t: i64) -> (Local, i64) {
    unsafe {
        let secs = (t.div_euclid(1000)) as libc::time_t;
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&secs, &mut tm);
        let l = Local { year: tm.tm_year + 1900, month: tm.tm_mon as usize, day: tm.tm_mday, weekday: tm.tm_wday as usize };
        tm.tm_hour = 0;
        tm.tm_min = 0;
        tm.tm_sec = 0;
        tm.tm_isdst = -1;
        let start = libc::mktime(&mut tm) as i64 * 1000;
        (l, start)
    }
}

/// Whole calendar days between two times (local time)
pub fn days_ago(t: i64, now: i64) -> i64 {
    ((local(now).1 - local(t).1) as f64 / DAY as f64).round() as i64
}

pub fn group_name(t: i64, now: i64) -> String {
    let ago = days_ago(t, now);
    if ago <= 0 {
        return "Today".into();
    }
    if ago == 1 {
        return "Yesterday".into();
    }
    if ago <= 7 {
        return "Previous 7 days".into();
    }
    let (d, n) = (local(t).0, local(now).0);
    if d.year == n.year {
        MONTHS[d.month].to_string()
    } else {
        format!("{} {}", MONTHS[d.month], d.year)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ListRow {
    Header(String),
    Chat(Entry),
}

/// The list as shown: Pinned first, then newest first by day and month
pub fn group(chats: &[Entry], now: i64) -> Vec<ListRow> {
    let mut sorted = chats.to_vec();
    sorted.sort_by(|a, b| b.updated.cmp(&a.updated));
    let mut out = Vec::new();
    let pinned: Vec<&Entry> = sorted.iter().filter(|c| c.pinned).collect();
    if !pinned.is_empty() {
        out.push(ListRow::Header("Pinned".into()));
        out.extend(pinned.into_iter().map(|c| ListRow::Chat(c.clone())));
    }
    let mut last = String::new();
    for c in sorted.iter().filter(|c| !c.pinned) {
        let name = group_name(c.updated, now);
        if name != last {
            out.push(ListRow::Header(name.clone()));
        }
        last = name;
        out.push(ListRow::Chat(c.clone()));
    }
    out
}

/// "Just now", "5 min ago", "3 h ago", "Yesterday", "Monday", "3 Oct", "3 Oct 2025"
pub fn relative_time(t: i64, now: i64) -> String {
    let s = (now - t).max(0);
    let ago = days_ago(t, now);
    if s < 60_000 {
        return "Just now".into();
    }
    if ago <= 0 && s < 3_600_000 {
        return format!("{} min ago", s / 60_000);
    }
    if ago <= 0 {
        return format!("{} h ago", s / 3_600_000);
    }
    if ago == 1 {
        return "Yesterday".into();
    }
    let d = local(t).0;
    if ago < 7 {
        return DAYS[d.weekday].into();
    }
    let short = &MONTHS[d.month][..3];
    if d.year == local(now).0.year {
        format!("{} {short}", d.day)
    } else {
        format!("{} {short} {}", d.day, d.year)
    }
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

// ── The files ───────────────────────────────────────────────────────────

/// Tessera's state folder ($XDG_STATE_HOME/tessera), or iRice's while only
/// that one has settings (shell/common/Paths.qml)
pub fn state_dir() -> PathBuf {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/state"));
    let old = crate::theme::config_path().parent().is_some_and(|p| p.ends_with("irice"));
    base.join(if old { "irice" } else { "tessera" })
}

/// Owner-only, whole or not at all: a temporary file renamed over it
pub fn write_private(path: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
    if let Some(dir) = path.parent() {
        std::fs::DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    let mut f = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&tmp)?;
    f.write_all(text.as_bytes())?;
    drop(f);
    std::fs::rename(&tmp, path)
}

pub struct Store {
    pub dir: PathBuf,
}

impl Store {
    pub fn new(dir: PathBuf) -> Self {
        Store { dir }
    }
    pub fn default_dir() -> PathBuf {
        state_dir().join("ai-chats")
    }
    pub fn path_of(&self, id: &str) -> Option<PathBuf> {
        valid_id(id).then(|| self.dir.join(format!("{id}.json")))
    }
    pub fn read_index(&self) -> Index {
        std::fs::read_to_string(self.dir.join("index.json")).ok().and_then(|t| parse_index(&t)).unwrap_or_default()
    }
    /// Read, change, write back (the other shell's changes kept); the index as written
    pub fn update_index(&self, f: impl FnOnce(&mut Index)) -> Index {
        let mut idx = self.read_index();
        f(&mut idx);
        if let Err(e) = write_private(&self.dir.join("index.json"), &index_text(&idx)) {
            eprintln!("tessera-ui: couldn't save the chat list: {e}");
        }
        idx
    }
    pub fn read_session(&self, id: &str) -> Option<Value> {
        let v: Value = serde_json::from_str(&std::fs::read_to_string(self.path_of(id)?).ok()?).ok()?;
        v["rows"].is_array().then_some(v)
    }
    pub fn write_session(&self, s: &Session) {
        if let Some(p) = self.path_of(&s.id) {
            if let Err(e) = write_private(&p, &serde_json::to_string(s).unwrap()) {
                eprintln!("tessera-ui: couldn't save the chat: {e}");
            }
        }
    }
    /// A saved chat's title or pin
    pub fn patch(&self, id: &str, fields: &Value) {
        let (Some(p), Some(mut s)) = (self.path_of(id), self.read_session(id)) else { return };
        for (k, v) in fields.as_object().into_iter().flatten() {
            s[k] = v.clone();
        }
        let _ = write_private(&p, &serde_json::to_string(&s).unwrap());
    }
    pub fn remove(&self, ids: &[String]) {
        for id in ids {
            if let Some(p) = self.path_of(id) {
                let _ = std::fs::remove_file(p);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, updated: i64, pinned: bool) -> Entry {
        Entry { id: id.into(), title: id.into(), created: updated, updated, pinned, ..Default::default() }
    }

    #[test]
    fn ids_and_titles() {
        assert!(valid_id("lx3k2-a9z") && !valid_id("../etc") && !valid_id("") && !valid_id("A"));
        assert_eq!(new_id(36, 0.0), "10-0");
        assert_eq!(new_id(35, 0.999_999_9), "z-zzzz");
        assert_eq!(title_from("\n  Plan   a weekend\nin Lisbon"), "Plan a weekend");
        assert_eq!(title_from(""), "New chat");
        let long = "Explain how the Hyprland compositor decides which window gets the keyboard focus after a close";
        let t = title_from(long);
        assert!(t.ends_with('…') && js_len(&t) <= TITLE_MAX + 1, "{t}");
        assert_eq!(clean_title("  a \n b "), "a b");
    }

    /// The same answers as AiChats.js for titles, previews and the index
    /// (node runs it, when node is there)
    #[test]
    fn matches_the_shells_chats_js() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../shell/services/AiChats.js");
        let titles = ["Plan a weekend in Lisbon", "Explain how the Hyprland compositor decides which window gets the keyboard focus after a close", "x, y, z, and then a lot of other words that go on and on and on, past it", "Ünïcödé ✓ words 🙂 and more words after the emoji that run very long indeed"];
        let rows = vec![Row::new("user", "Hello  there", "done"), Row::new("note", "skip", "done"), Row::new("assistant", &"word ".repeat(80), "done")];
        let idx = r#"{"version":1,"current":"b","trash":["x","../bad"],"chats":[{"id":"a","updated":5,"title":""},{"id":"b","updated":9,"pinned":true,"count":2},{"id":"a","updated":7},{"id":"BAD","updated":1},{"id":"c"}]}"#;
        let script = format!(
            r#"const fs=require("fs");const src=fs.readFileSync({:?},"utf8").replace(/^\.pragma.*$/m,"");
               const C=new Function(src+";return {{titleFrom,previewOf,parseIndex,indexText,session,entryOf}};")();
               const rows={};
               const s=C.session({{id:"abc",title:"T",created:1,updated:2,model:"k",modelName:"M",pinned:false}},rows,[{{role:"user",text:"q",images:[{{data:"x"}},{{data:"y"}}]}}]);
               const i=C.parseIndex({:?});
               console.log(JSON.stringify({{titles:{}.map(C.titleFrom),preview:C.previewOf(rows),session:JSON.stringify(s),entry:C.entryOf(s),index:C.indexText(i.current,i.chats,i.trash)}}));"#,
            path,
            serde_json::to_string(&rows).unwrap(),
            idx,
            serde_json::to_string(&titles).unwrap()
        );
        let Ok(out) = std::process::Command::new("node").args(["-e", &script]).output() else {
            eprintln!("node not installed: skipped");
            return;
        };
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let js: Value = serde_json::from_slice(&out.stdout).unwrap();
        for (i, t) in titles.iter().enumerate() {
            assert_eq!(title_from(t), js["titles"][i].as_str().unwrap());
        }
        assert_eq!(preview_of(&rows), js["preview"].as_str().unwrap());
        let s = Session {
            id: "abc".into(),
            title: "T".into(),
            created: 1,
            updated: 2,
            model: "k".into(),
            model_name: "M".into(),
            pinned: false,
            rows: rows.clone(),
            history: strip_pictures(&[json!({"role": "user", "text": "q", "images": [{"data": "x"}, {"data": "y"}]})]),
        };
        // the same file, byte for byte
        assert_eq!(serde_json::to_string(&s).unwrap(), js["session"].as_str().unwrap());
        assert_eq!(serde_json::to_value(entry_of(&s)).unwrap(), js["entry"]);
        assert_eq!(index_text(&parse_index(idx).unwrap()), js["index"].as_str().unwrap());
    }

    #[test]
    fn a_session_round_trips_through_the_files() {
        let dir = std::env::temp_dir().join(format!("tessera-ui-chats-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = Store::new(dir.clone());
        let mut rows = vec![Row::new("user", "Plan a weekend in Lisbon", "done"), Row::new("assistant", "**A weekend**", "done")];
        rows[1].model = "Atlas 8B".into();
        let s = Session {
            id: "lx1-ab".into(),
            title: title_from(&rows[0].text),
            created: 10,
            updated: 20,
            model: "custom|atlas-8b".into(),
            model_name: "Atlas 8B".into(),
            pinned: false,
            rows: rows.clone(),
            history: vec![json!({"role": "user", "text": "Plan a weekend in Lisbon"}), json!({"role": "assistant", "text": "**A weekend**"})],
        };
        store.write_session(&s);
        // (another shell's chat already in the index stays)
        std::fs::write(dir.join("index.json"), r#"{"version":1,"current":"","trash":[],"chats":[{"id":"qml-1","title":"From QML","created":5,"updated":30,"model":"","modelName":"","pinned":true,"count":2,"preview":"hi"}]}"#).unwrap();
        let idx = store.update_index(|i| {
            i.chats = upsert(&i.chats, entry_of(&s));
            i.current = s.id.clone();
        });
        assert_eq!(idx.chats.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(), ["qml-1", "lx1-ab"]);
        assert_eq!(store.read_index(), idx);
        let back = store.read_session("lx1-ab").unwrap();
        assert_eq!(back["rows"].as_array().unwrap().iter().map(Row::from_value).collect::<Vec<_>>(), rows);
        assert_eq!(back["modelName"], "Atlas 8B");
        assert_eq!(back["history"][1]["text"], "**A weekend**");
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(dir.join("lx1-ab.json")).unwrap().permissions().mode() & 0o777, 0o600);
        store.patch("lx1-ab", &json!({"title": "Lisbon", "pinned": true}));
        assert_eq!(store.read_session("lx1-ab").unwrap()["title"], "Lisbon");
        assert_eq!(search_files(&dir, "weekend LISBON"), vec!["lx1-ab".to_string()]);
        assert!(search_files(&dir, "tokyo").is_empty());
        store.remove(&["lx1-ab".into(), "../x".into()]);
        assert!(store.read_session("lx1-ab").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn groups_times_and_search() {
        let now = now_ms();
        let chats = vec![entry("a", now - 1000, false), entry("b", now - 3 * DAY, false), entry("p", now - 40 * DAY, true), entry("c", now - 400 * DAY, false)];
        let g = group(&chats, now);
        assert_eq!(g[0], ListRow::Header("Pinned".into()));
        assert_eq!(g[1], ListRow::Chat(chats[2].clone()));
        assert_eq!(g[2], ListRow::Header("Today".into()));
        assert_eq!(g[4], ListRow::Header("Previous 7 days".into()));
        assert!(matches!(&g[6], ListRow::Header(h) if h.contains(' '))); // (a month of another year)
        assert_eq!(relative_time(now - 5000, now), "Just now");
        assert_eq!(relative_time(now - 400 * DAY, now).split(' ').count(), 3);
        assert_eq!(prune(&chats, 30, now, &["a"]), vec!["c".to_string()]);
        assert!(prune(&chats, 0, now, &[]).is_empty());
        let mut e = entry("x", now, false);
        e.title = "Trip plan: a weekend in Goa".into();
        assert!(matches(&e, "goa TRIP") && !matches(&e, "goa lisbon"));
        assert_eq!(filter(&[e.clone()], "nothing", &["x".into()]).len(), 1);
    }
}

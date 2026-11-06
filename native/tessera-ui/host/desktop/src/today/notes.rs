//! Notes.qml and modules/notes/NotesFeed.qml. Notes are Markdown files, one
//! per note, in a folder (~/Documents/Notes unless Settings → Desktop says
//! otherwise). Saving runs shell/assets/scripts/save-note.py (it never
//! replaces another note), then the island says "Note saved". The quick note
//! (surface `notes`) is told the folder (with ~) and how a save went:
//! {"to":"notes","folder":…}, {"saving":path}, then {"saved":path} or
//! {"failed":path}. It asks: notes ["save", text], notes ["openFolder"].

use super::common::*;
use serde_json::{json, Value};
use std::io::Write;
use tessera_core::host::Host;

/// A note found in the folder
#[derive(Clone, Debug, PartialEq)]
pub struct NoteFile {
    pub name: String,
    pub path: String,
    /// ms
    pub modified: i64,
}

pub fn folder() -> String {
    cfg_get("notes.folder")
        .as_str()
        .filter(|f| !f.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("{}/Documents/Notes", std::env::var("HOME").unwrap_or_default()))
}

/// The folder with ~ for the home (what the quick note shows)
pub fn folder_short() -> String {
    let f = folder();
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() => f.replacen(&home, "~", 1),
        _ => f,
    }
}

/// Notes.list: the newest notes first (*.md, *.markdown, *.txt)
pub fn list(limit: usize) -> Vec<NoteFile> {
    let mut out: Vec<NoteFile> = std::fs::read_dir(folder())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            let ext = p.extension()?.to_str()?.to_ascii_lowercase();
            if !["md", "markdown", "txt"].contains(&ext.as_str()) || !e.metadata().ok()?.is_file() {
                return None;
            }
            let modified = e.metadata().ok()?.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_millis() as i64;
            Some(NoteFile { name: p.file_stem()?.to_str()?.to_string(), path: p.to_str()?.to_string(), modified })
        })
        .collect();
    out.sort_by(|a, b| b.modified.cmp(&a.modified));
    if limit > 0 {
        out.truncate(limit);
    }
    out
}

/// Notes.search
pub fn search(q: &str, limit: usize) -> Vec<NoteFile> {
    let t = q.to_lowercase();
    list(0).into_iter().filter(|n| n.name.to_lowercase().contains(&t)).take(if limit == 0 { 5 } else { limit }).collect()
}

/// A note's title: its first line without the #s, cut at 60
pub fn title_of(text: &str) -> String {
    let first = text.trim().lines().next().unwrap_or("").trim_start_matches('#').trim_start();
    if first.chars().count() > 60 {
        format!("{}…", first.chars().take(57).collect::<String>().trim())
    } else {
        first.to_string()
    }
}

/// "2026-10-27 093015123-ab12cd34 Call the bank.md"
pub fn file_name(title: &str) -> String {
    let clean: String = title.chars().map(|c| if "/\\:*?\"<>|#\n\r\t".contains(c) { ' ' } else { c }).collect();
    let clean = clean.split_whitespace().collect::<Vec<_>>().join(" ");
    let clean: String = clean.chars().take(60).collect::<String>().trim().to_string();
    let t = now();
    let stamp = format!("{}{:03}", tessera_core::util::strftime("%Y-%m-%d %H%M%S", t.div_euclid(1000)), t.rem_euclid(1000));
    let rand: String = new_id().chars().rev().take(8).collect();
    format!("{stamp}-{rand}{}.md", if clean.is_empty() { String::new() } else { format!(" {clean}") })
}

/// The file's content: the first line is the heading, the rest the body
pub fn content_of(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    let rest = lines.iter().skip(1).copied().collect::<Vec<_>>().join("\n");
    let rest = rest.trim();
    if body.starts_with('#') {
        format!("{body}\n")
    } else {
        format!("# {}\n{}", lines.first().unwrap_or(&"").trim(), if rest.is_empty() { String::new() } else { format!("\n{rest}\n") })
    }
}

/// What the save thread tells the service
pub struct Saved {
    pub path: String,
    pub title: String,
    pub ok: bool,
    pub open: bool,
}

/// Notes.create: starts the save, returns the path ("" for an empty note)
pub fn create(h: &mut Host, text: &str, open: bool) -> String {
    let body = text.trim();
    if body.is_empty() {
        return String::new();
    }
    let title = title_of(body);
    let path = format!("{}/{}", folder(), file_name(&title));
    let content = content_of(body);
    let script = tessera_core::util::asset("shell/assets/scripts/save-note.py");
    let send = h.sender(super::NAME);
    let (p, t) = (path.clone(), title);
    std::thread::spawn(move || {
        let ok = script
            .and_then(|s| {
                let mut child = std::process::Command::new("python3")
                    .args(["-I"])
                    .arg(s)
                    .arg(&p)
                    .stdin(std::process::Stdio::piped())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                    .ok()?;
                child.stdin.take()?.write_all(content.as_bytes()).ok()?;
                child.wait().ok().map(|s| s.success())
            })
            .unwrap_or(false);
        send(Box::new(super::Msg::NoteSaved(Saved { path: p, title: t, ok, open })));
    });
    path
}

/// The save thread finished: the island's line, the quick note's answer
pub fn saved(h: &mut Host, s: Saved) {
    if s.ok {
        let c = accent(h, "yellow");
        island_post(h, json!({"kind": "custom", "icon": "note-pencil", "title": "Note saved", "subtitle": s.title, "color": c}), 3000);
        h.tell("notes", json!({"saved": s.path}));
        if s.open {
            open(&s.path);
        }
    } else {
        let c = accent(h, "red");
        island_post(h, json!({"kind": "custom", "icon": "warning", "title": "Couldn't save note", "subtitle": "Check the notes folder and available space.", "color": c}), 3000);
        h.tell("notes", json!({"failed": s.path}));
    }
}

/// Notes.saveLink: "save https://…" keeps the link as a note
pub fn save_link(h: &mut Host, url: &str, words: &str) -> String {
    let w = words.trim();
    let note = format!("{}<{}>", if w.is_empty() { String::new() } else { format!("{w}\n\n") }, url);
    let host = url.trim_start_matches("https://").trim_start_matches("http://").split('/').next().unwrap_or("");
    create(h, &format!("Saved link: {}\n\n{}", if w.is_empty() { host } else { w }, note), false)
}

pub fn open(path: &str) {
    detached(&["xdg-open", &format!("file://{path}")]);
}

/// NotesFeed: tell the quick note the folder
pub fn send_folder(h: &mut Host) {
    h.tell("notes", json!({"folder": folder_short()}));
}

/// A call from the quick note (RustUi's "notes" call: [what, …])
pub fn call(h: &mut Host, call: &str, args: &Value) {
    let a = args_of(args);
    let what = if let Some(w) = call.strip_prefix("notes.") { w.to_string() } else { arg_str(args, 0) };
    let off = if call.contains('.') { 0 } else { 1 };
    match what.as_str() {
        "save" => {
            let text = a.get(off).and_then(Value::as_str).unwrap_or("").to_string();
            let path = if text.trim().is_empty() { String::new() } else { create(h, &text, false) };
            h.tell("notes", json!({"saving": path}));
        }
        "openFolder" => {
            h.command("close", "notes", None, json!({}));
            let f = folder();
            let _ = std::fs::create_dir_all(&f);
            h.ask("files.open", json!([f]));
        }
        other => eprintln!("tessera-ui: host: no notes call {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_and_content() {
        assert_eq!(title_of("# Call the bank\nabout the card"), "Call the bank");
        assert_eq!(content_of("Call the bank\nabout the card"), "# Call the bank\n\nabout the card\n");
        assert_eq!(content_of("# Already\nbody"), "# Already\nbody\n");
        assert_eq!(content_of("One line"), "# One line\n");
        let n = file_name("Call: the/bank");
        assert!(n.ends_with(" Call the bank.md"), "{n}");
        assert!(title_of(&"x".repeat(80)).ends_with('…'));
    }
}

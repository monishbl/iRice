//! Spotlight's shell-side half (shell/services/Search.qml's "Search drawn in
//! Rust" part, rustQuery / rustSend / rustRun / rustAction / rustCommand).
//! The Rust spotlight ranks apps, actions, answers, conversions, definitions,
//! files and the web itself (surfaces/spotlight/src/rank.rs, providers.rs);
//! what it asks the host for is the rows only the shell knows:
//!   `search.query [raw, store]` → `{"to":"spotlight","extras":{q, ai, cast,
//!       intents, reminder, store, calendar, contacts, notes, shots, clipboard}}`
//!   `search.run [key]` runs a row of those; `search.action [title]` an
//!       action by its title; `search.command [cmd]` a ">command".
//! Differences (their owners are other hosts): no intents (Intents is the
//! shell host's), no reminder row (Calendar.parseQuick), no App Store rows
//! (sharing host), no contacts (CloudAccounts); cast actions are empty.

use super::{clipboard, files, livetext, util};
use serde_json::{json, Value};
use std::collections::HashMap;
use tessera_core::host::{Host, Service};
use tessera_core::services::clipboard as cb;

/// What a row does when picked
#[derive(Clone, Debug)]
enum Run {
    Paste { id: Option<String>, image: Option<String>, text: String },
    NotificationCenter,
    OpenFile(String),
    /// (path, what was searched for: LiveText.preview(path, query))
    PreviewShot(String, String),
}

/// "2026-10-27 1432 Call the bank" → "Call the bank"
pub fn note_title(name: &str) -> String {
    let b = name.as_bytes();
    let dated = b.len() > 16 && b[..4].iter().all(u8::is_ascii_digit) && b[4] == b'-' && b[5..7].iter().all(u8::is_ascii_digit) && b[7] == b'-' && b[8..10].iter().all(u8::is_ascii_digit) && b[10] == b' ' && b[11..15].iter().all(u8::is_ascii_digit) && b[15] == b' ';
    if dated { name[16..].to_string() } else { name.to_string() }
}

#[derive(Default)]
pub struct SearchSvc {
    runs: HashMap<String, Run>,
    index: Option<(std::time::SystemTime, Value)>,
}

impl SearchSvc {
    pub fn new() -> Self {
        Self::default()
    }

    /// The screenshot index, re-read only when its file changed
    fn shots(&mut self) -> &Value {
        let m = std::fs::metadata(livetext::index_file()).and_then(|m| m.modified()).ok();
        let stale = match (&self.index, m) {
            (Some((t, _)), Some(m)) => *t != m,
            (None, _) => true,
            (_, None) => true,
        };
        if stale {
            self.index = Some((m.unwrap_or(std::time::UNIX_EPOCH), livetext::read_index()));
        }
        &self.index.as_ref().unwrap().1
    }

    fn pack(&mut self, list: Vec<(Value, Run)>, n: &mut usize) -> Vec<Value> {
        list.into_iter()
            .map(|(mut row, run)| {
                let key = format!("r{n}");
                *n += 1;
                self.runs.insert(key.clone(), run);
                row["key"] = json!(key);
                row
            })
            .collect()
    }

    fn row(section: &str, title: &str, subtitle: &str, glyph: &str) -> Value {
        json!({"section": section, "title": title, "subtitle": subtitle, "glyph": glyph, "icon": "", "picture": "", "path": "", "loading": false})
    }

    fn ai_meta(h: &Host) -> Value {
        let ai = &h.cfg()["ai"];
        let models = tessera_core::services::ai::models::read(&ai["models"]);
        let want = ai["model"].as_str().unwrap_or("");
        match models.iter().find(|m| m.key == want).or_else(|| models.first()) {
            Some(m) if !m.id.is_empty() => json!({"configured": true, "name": m.name}),
            _ => json!({"configured": false, "name": "Add a model"}),
        }
    }

    fn query(&mut self, h: &mut Host, raw: &str) {
        let q = raw.trim().to_lowercase();
        let raw = raw.trim();
        self.runs.clear();
        let mut n = 0usize;
        let mut out = json!({"q": raw, "ai": Self::ai_meta(h), "cast": [], "intents": [], "reminder": [], "store": [], "calendar": [], "contacts": [], "notes": [], "shots": [], "clipboard": []});
        if let Some(rest) = raw.strip_prefix(';') {
            let rows = h.st.shell.get::<cb::History>().map(|hist| cb::filtered(hist, rest, cb::Filter::All, tessera_core::util::now_ms())).unwrap_or_default();
            let list = rows
                .into_iter()
                .take(30)
                .map(|r| {
                    let is_image = r.image.is_some();
                    let title = if is_image { "Image".to_string() } else { r.text.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(120).collect() };
                    let row = Self::row(if r.pinned() { "Pinned" } else { "Clipboard" }, &title, "", if is_image { "image" } else { "clipboard-text" });
                    let run = match (&r.pin, &r.entry) {
                        (Some(p), _) => Run::Paste { id: None, image: p.image.clone(), text: p.text.clone().unwrap_or_default() },
                        (_, Some(e)) => Run::Paste { id: Some(e.id.clone()), image: e.image.as_ref().map(|i| i.to_string_lossy().into_owned()), text: e.text.clone() },
                        _ => Run::Paste { id: None, image: None, text: r.text.clone() },
                    };
                    (row, run)
                })
                .collect();
            out["clipboard"] = json!(self.pack(list, &mut n));
        } else if !raw.is_empty() && !raw.starts_with(['?', '>', ':', '/']) {
            if q.chars().count() >= 3 {
                // Calendar.search: the next year's events whose title has it
                let now = tessera_core::util::now_ms();
                let events: Vec<_> = h
                    .st
                    .shell
                    .get::<tessera_core::services::calendar::Calendar>()
                    .map(|c| c.between(now - 86_400_000, now + 366 * 86_400_000).into_iter().filter(|e| e.title.to_lowercase().contains(&q)).take(3).collect())
                    .unwrap_or_default();
                let list = events
                    .into_iter()
                    .map(|e| {
                        let when = if e.all_day { "All day".to_string() } else { tessera_core::util::strftime("%-I:%M %p", e.start / 1000) };
                        (Self::row("Calendar", &e.title, &format!("{} · {}", tessera_core::util::strftime("%a %-d %b", e.start / 1000), when), "calendar-blank"), Run::NotificationCenter)
                    })
                    .collect();
                out["calendar"] = json!(self.pack(list, &mut n));
                // Notes by name
                let folder = h.cfg()["notes"]["folder"].as_str().filter(|s| !s.is_empty()).map(String::from).unwrap_or_else(|| format!("{}/Documents/Notes", util::home()));
                let mut found: Vec<(i64, String, String)> = std::fs::read_dir(&folder)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .filter_map(|e| {
                        let p = e.path();
                        let ext = p.extension()?.to_str()?.to_lowercase();
                        if !["md", "markdown", "txt"].contains(&ext.as_str()) || !e.file_type().ok()?.is_file() {
                            return None;
                        }
                        let name = p.file_stem()?.to_string_lossy().into_owned();
                        let m = e.metadata().ok()?.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64;
                        name.to_lowercase().contains(&q).then(|| (m, name, p.to_string_lossy().into_owned()))
                    })
                    .collect();
                found.sort_by(|a, b| b.0.cmp(&a.0));
                let list = found.into_iter().take(4).map(|(m, name, path)| (Self::row("Notes", &note_title(&name), &tessera_core::util::strftime("%-d %b %Y", m), "note"), Run::OpenFile(path))).collect();
                out["notes"] = json!(self.pack(list, &mut n));
                // Screenshots by the words in them: the words around the match, the shot as the picture
                if h.cfg()["screenshot"]["textSearch"].as_bool().unwrap_or(true) {
                    let hits = livetext::search(self.shots(), raw, 4);
                    let list = hits
                        .into_iter()
                        .map(|s| {
                            let mut row = Self::row("Screenshots", &s.snippet, &tessera_core::util::strftime("%-d %b %Y, %-I:%M %p", s.modified), "image");
                            row["picture"] = json!(s.path);
                            row["path"] = json!(s.path);
                            (row, Run::PreviewShot(s.path, raw.to_string()))
                        })
                        .collect();
                    out["shots"] = json!(self.pack(list, &mut n));
                }
            }
        }
        h.tell("spotlight", json!({"extras": out}));
    }

    /// Search.rustAction: an action by its title
    fn action(&mut self, h: &mut Host, title: &str) {
        let dark = h.st.shell.look.dark;
        match title {
            "Lock Screen" => h.ask("system.power", json!(["lock"])),
            "Sleep…" => h.ask("panels.showSession", json!(["sleep"])),
            "Restart…" => h.ask("panels.showSession", json!(["restart"])),
            "Shut Down…" => h.ask("panels.showSession", json!(["shutdown"])),
            "Log Out…" => h.ask("panels.showSession", json!(["logout"])),
            "Send Files…" => h.ask("panels.pickShare", json!([])),
            // Like Android's Quick Share tile: visible to phones nearby for a while
            "Receive with Quick Share" => {
                util::cfg_set("sharing.quickShare", json!("10min"));
                util::island_post(h, json!({"kind": "custom", "icon": "share-network", "title": "Visible to phones for 10 minutes", "subtitle": "In their Quick Share", "color": "blue"}), 4000);
            }
            "Toggle Dark Mode" => util::cfg_set("appearance.mode", json!(if dark { "light" } else { "dark" })),
            "Quick Settings" => h.ask("panels.show", json!(["controlCenter"])),
            "Notification Center" => h.ask("panels.show", json!(["notificationCenter"])),
            "Ask AI" | "Translate" => h.ask("panels.show", json!(["sidebar"])),
            "Ask AI About the Screen" => h.ask("screen.do", json!(["select", "ask"])),
            "System Settings" => h.ask("panels.openSettings", json!([])),
            "Take Screenshot" => h.ask("screen.do", json!(["select", "screenshot"])),
            "Copy Text from Screen" => h.ask("screen.do", json!(["select", "ocr"])),
            "Record Screen" => h.ask("screen.do", json!(["select", "record"])),
            "Keyboard Shortcuts" | "Periodic Table" => h.ask("panels.show", json!(["cheatsheet"])),
            "On-Screen Keyboard" => h.ask("panels.show", json!(["keyboard"])),
            "Clipboard History" => h.ask("panels.show", json!(["clipboard"])),
            "Overlay Widgets" => h.ask("panels.show", json!(["overlay"])),
            "Identify Song" => h.ask("system.identifySong", json!([])),
            "Run Health Check" => h.ask("show", json!(["health"])),
            "Report a Bug…" => h.ask("show", json!(["bugReport"])),
            "Check for Updates" => h.ask("system.checkUpdates", json!([])),
            "Change Wallpaper" => h.ask("panels.show", json!(["wallpaper"])),
            "Show Today" => {
                h.ask("panels.closeOpen", json!([]));
                h.ask("today.show", json!([""]));
            }
            "Screen Recording Options" => {
                h.ask("panels.closeOpen", json!([]));
                h.ask("recorder.options", json!([]));
            }
            "Stages" => {
                let on = h.cfg()["stageManager"]["enabled"].as_bool().unwrap_or(false);
                util::cfg_set("stageManager.enabled", json!(!on));
            }
            "Software" => h.ask("panels.openAppStore", json!([])),
            "App Drawer" => h.ask("panels.show", json!(["launchpad"])),
            "All Windows" => h.ask("panels.show", json!(["missionControl"])),
            _ => {}
        }
    }

    fn run(&mut self, h: &mut Host, key: &str) {
        let Some(r) = self.runs.get(key).cloned() else { return };
        match r {
            Run::Paste { id, image, text } => clipboard::paste(h, id.as_deref(), image.as_deref(), &text, false),
            Run::NotificationCenter => h.ask("panels.show", json!(["notificationCenter"])),
            Run::OpenFile(p) => files::open(&h.cfg().clone(), &p),
            Run::PreviewShot(p, query) => {
                // (Quick Preview takes it once when it opens: LiveText.takeQuery)
                h.set_state("livetext.query", json!(query));
                h.ask("panels.quickLook", json!([p]));
            }
        }
    }
}

impl Service for SearchSvc {
    fn name(&self) -> &'static str {
        "Search"
    }
    fn calls(&self) -> &'static [&'static str] {
        &["search"]
    }
    fn start(&mut self, h: &mut Host) {
        h.st.shell.want("clipboard");
        h.st.shell.want("calendar");
        h.st.shell.want("clients");
    }
    fn call(&mut self, h: &mut Host, call: &str, args: &Value) {
        match call {
            "search.query" => self.query(h, args[0].as_str().unwrap_or("")),
            "search.run" => self.run(h, args[0].as_str().unwrap_or("")),
            "search.action" => self.action(h, args[0].as_str().unwrap_or("")),
            // ">command" typed in the Rust search: run in a terminal
            "search.command" => {
                let cmd = args[0].as_str().unwrap_or("").trim().to_string();
                if !cmd.is_empty() {
                    h.ask("launch.terminal", json!([["sh", "-c", cmd], true]));
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_lose_their_date_prefix() {
        assert_eq!(note_title("2026-10-27 1432 Call the bank"), "Call the bank");
        assert_eq!(note_title("Groceries"), "Groceries");
    }
}

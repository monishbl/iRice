//! The sidebar's settings and numbers: where the panel sits (Sidebar.qml),
//! Translate's languages (Translator.qml), and writing a setting back to
//! config.json (the shell's Config.qml reads edits to the file live).

use serde_json::Value;

/// Room below the panel: the dock's, when there is one (Sidebar.qml's dockClearance)
pub fn dock_clearance(cfg: &Value) -> f32 {
    if cfg["dock"]["enabled"].as_bool().unwrap_or(true) {
        cfg["dock"]["iconSize"].as_f64().unwrap_or(48.0) as f32 + 14.0 + 6.0 + 8.0
    } else {
        8.0
    }
}

pub const PANEL_W: f32 = 420.0;
pub const PANEL_X: f32 = 8.0;
/// Under the menu bar (30) and a 6 px gap
pub const PANEL_Y: f32 = 36.0;
/// Clicks above this pass to the menu bar; below, outside the panel, they close it
pub const BAR_H: i32 = 30;

/// The panel's height on a screen this tall (logical px)
pub fn panel_height(screen_h: f32, cfg: &Value) -> f32 {
    (screen_h - PANEL_Y - dock_clearance(cfg)).max(200.0)
}

/// Translate's languages: [code, name] (the first, Detect, only as the source)
pub const LANGUAGES: &[(&str, &str)] = &[
    ("auto", "Detect"),
    ("en", "English"),
    ("hi", "Hindi"),
    ("es", "Spanish"),
    ("fr", "French"),
    ("de", "German"),
    ("it", "Italian"),
    ("pt", "Portuguese"),
    ("ru", "Russian"),
    ("ja", "Japanese"),
    ("ko", "Korean"),
    ("zh-CN", "Chinese"),
    ("ar", "Arabic"),
    ("bn", "Bengali"),
    ("ta", "Tamil"),
    ("te", "Telugu"),
    ("mr", "Marathi"),
    ("tr", "Turkish"),
    ("nl", "Dutch"),
];

pub fn lang_name(code: &str) -> String {
    LANGUAGES.iter().find(|l| l.0 == code).map_or(code, |l| l.1).to_string()
}

pub fn translate_target(cfg: &Value) -> String {
    cfg["sidebar"]["translateTarget"].as_str().unwrap_or("en").to_string()
}

/// The tabs (Sidebar.qml's): Anime only with sidebar.booru
pub fn tabs(booru: bool) -> Vec<&'static str> {
    if booru {
        vec!["AI", "Translate", "Anime"]
    } else {
        vec!["AI", "Translate"]
    }
}

/// Where a chat's row sits in the past chats (ChatHistory.qml's list: a
/// group's header 26 px, 36 in Material, plus 8 above all but the first; a
/// chat 46, 60 in Material): its top and bottom, for scrolling it into view
pub fn row_span(list: &[crate::services::ai::chats::ListRow], id: &str, material: bool) -> Option<(f32, f32)> {
    use crate::services::ai::chats::ListRow;
    let mut y = 0.0;
    for (i, r) in list.iter().enumerate() {
        let h = match r {
            ListRow::Header(_) => (if i == 0 { 0.0 } else { 8.0 }) + if material { 36.0 } else { 26.0 },
            ListRow::Chat(_) => if material { 60.0 } else { 46.0 },
        };
        if matches!(r, ListRow::Chat(c) if c.id == id) {
            return Some((y, y + h));
        }
        y += h;
    }
    None
}

/// A setting changed here, written back to config.json (`path`: its keys);
/// the rest of the file as it is. In place, as the shell writes it: the
/// shell watches the file and takes the change.
pub fn write(path: &[&str], value: Value) {
    let file = crate::theme::config_path();
    let mut cfg: Value = std::fs::read_to_string(&file).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Object(Default::default()));
    if !set(&mut cfg, path, value) {
        return;
    }
    if let Ok(text) = serde_json::to_string_pretty(&cfg) {
        if let Err(e) = std::fs::write(&file, text + "\n") {
            eprintln!("tessera-ui: couldn't save a setting: {e}");
        }
    }
}

/// Set a value at a path of keys, making objects on the way; false when it was already so
pub fn set(cfg: &mut Value, path: &[&str], value: Value) -> bool {
    let mut v = cfg;
    for k in &path[..path.len() - 1] {
        if !v[*k].is_object() {
            v[*k] = Value::Object(Default::default());
        }
        v = &mut v[*k];
    }
    let last = path[path.len() - 1];
    if v[last] == value {
        return false;
    }
    v[last] = value;
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_panel_clears_the_dock() {
        assert_eq!(dock_clearance(&json!({})), 76.0);
        assert_eq!(dock_clearance(&json!({"dock": {"iconSize": 64}})), 92.0);
        assert_eq!(dock_clearance(&json!({"dock": {"enabled": false}})), 8.0);
        assert_eq!(panel_height(1080.0, &json!({})), 1080.0 - 36.0 - 76.0);
    }

    #[test]
    fn the_selected_chat_scrolls_into_view() {
        use crate::services::ai::chats::{Entry, ListRow};
        let chat = |id: &str| ListRow::Chat(Entry { id: id.into(), ..Default::default() });
        let list = vec![ListRow::Header("Pinned".into()), chat("a"), ListRow::Header("Today".into()), chat("b"), chat("c")];
        assert_eq!(row_span(&list, "a", false), Some((26.0, 72.0)));
        assert_eq!(row_span(&list, "b", false), Some((72.0 + 34.0, 152.0)));
        assert_eq!(row_span(&list, "c", true), Some((36.0 + 60.0 + 44.0 + 60.0, 260.0)));
        assert_eq!(row_span(&list, "zz", true), None);
    }

    #[test]
    fn settings_written_in_place() {
        let mut c = json!({"ai": {"model": "a", "keep": 1}, "z": true});
        assert!(set(&mut c, &["ai", "model"], json!("b")));
        assert!(!set(&mut c, &["ai", "model"], json!("b")));
        assert!(set(&mut c, &["sidebar", "translateTarget"], json!("fr")));
        assert_eq!(c, json!({"ai": {"model": "b", "keep": 1}, "z": true, "sidebar": {"translateTarget": "fr"}}));
        // (the file's own order is kept)
        assert_eq!(serde_json::to_string(&c).unwrap(), r#"{"ai":{"model":"b","keep":1},"z":true,"sidebar":{"translateTarget":"fr"}}"#);
        assert_eq!(lang_name("zh-CN"), "Chinese");
        assert_eq!(lang_name("xx"), "xx");
    }
}

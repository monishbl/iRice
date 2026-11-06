//! Keyboard shortcuts and trackpad gestures for Settings: Tessera's defaults
//! (hypr/tessera/keymap.json and gestures.json) merged with your changes
//! (<config dir>/keybinds.json), replacing the data and editing half of
//! shell/services/Keymap.qml. Hyprland reads the same files (keybinds.lua,
//! gestures.lua), so saving writes the file and reloads Hyprland's config.
//! (Reading keys and naming actions: keyfmt.rs. The service that runs it,
//! saves it and answers Settings: input_bridge.rs.)
//!
//! keybinds.json:
//!   { "version": 1,
//!     "keybinds":  { "<default id>": { "keys": "...", "disabled": true } },
//!     "shortcuts": [ { id: "custom-…", keys, group: "Custom", label, action, opts? } ],
//!     "gestures":  { "changes": { "<id>": { fingers, direction, action, disabled } },
//!                    "custom": [ { id, fingers, direction, label, action } ] } }

use super::keyfmt::*;
use serde_json::{json, Map, Value};
use std::collections::HashMap;

pub fn blank_user() -> Value {
    json!({"version": 1, "keybinds": {}, "shortcuts": [], "gestures": {"changes": {}, "custom": []}})
}

pub fn normalize_user(u: &Value) -> Value {
    let mut out = blank_user();
    if !u.is_object() {
        return out;
    }
    if u["keybinds"].is_object() {
        out["keybinds"] = u["keybinds"].clone();
    }
    if let Some(a) = u["shortcuts"].as_array() {
        out["shortcuts"] = Value::Array(a.iter().filter(|s| s.is_object()).cloned().collect());
    }
    let g = &u["gestures"];
    if g.is_object() {
        if g["changes"].is_object() {
            out["gestures"]["changes"] = g["changes"].clone();
        }
        if let Some(a) = g["custom"].as_array() {
            out["gestures"]["custom"] = Value::Array(a.iter().filter(|x| x.is_object()).cloned().collect());
        }
    }
    out
}

/// prefix + Date.now() in base 36 + a random 0..1296 in base 36
pub fn new_id(prefix: &str) -> String {
    fn b36(mut n: u128) -> String {
        if n == 0 {
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
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    format!("{prefix}{}{}", b36(now.as_millis()), b36((now.subsec_nanos() as u128 / 7) % 1296))
}

fn is_empty_obj(v: &Value) -> bool {
    v.as_object().is_some_and(Map::is_empty)
}

pub struct Keymap {
    pub defaults: Vec<Value>,
    pub gesture_defaults: Vec<Value>,
    pub user: Value,
    pub defaults_loaded: bool,
    pub user_loaded: bool,
    pub defaults_error: String,
    pub user_error: String,
    pub save_error: String,
    pub revision: u64,
    pub mod_key: String,
    /// mac | windows | linux (KeyNames.style)
    pub style: String,
    /// desktop entry id -> name (Apps.byId(id).name)
    pub apps: HashMap<String, String>,
    /// a change was made that wants saving
    pub dirty: bool,
}

impl Default for Keymap {
    fn default() -> Self {
        Self {
            defaults: vec![], gesture_defaults: vec![], user: blank_user(), defaults_loaded: false, user_loaded: false,
            defaults_error: String::new(), user_error: String::new(), save_error: String::new(), revision: 0,
            mod_key: "SUPER".into(), style: "linux".into(), apps: HashMap::new(), dirty: false,
        }
    }
}

impl Keymap {
    pub fn ready(&self) -> bool {
        self.defaults_loaded && self.user_loaded
    }
    fn app_name(&self, id: &str) -> Option<String> {
        self.apps.get(id).cloned()
    }
    pub fn describe(&self, a: &Value) -> String {
        describe(a, &|id| self.app_name(id))
    }
    pub fn describe_gesture(&self, a: &Value) -> String {
        describe_gesture(a, &|id| self.app_name(id))
    }
    pub fn key_text(&self, keys: &str) -> String {
        key_text(keys, &self.mod_key, &self.style)
    }
    pub fn same_action(a: &Value, b: &Value) -> bool {
        action_key(a) == action_key(b)
    }

    // ── Merged lists ──

    /// Each bind: { id, keys, defaultKeys, group, label, action, opts, custom,
    /// disabled, changed, keysChanged, isDefault, keyList, keyText, actionText }
    pub fn binds(&self) -> Vec<Value> {
        let mut out = Vec::new();
        let none = json!({});
        for d in &self.defaults {
            let id = d["id"].as_str().unwrap_or("");
            let c = self.user["keybinds"].get(id).unwrap_or(&none);
            let dkeys = d["keys"].as_str().unwrap_or("");
            let keys = c["keys"].as_str().unwrap_or(dkeys);
            let keys_changed = c["keys"].as_str().is_some_and(|k| canonical(k) != canonical(dkeys));
            let disabled = truthy(&c["disabled"]);
            out.push(json!({
                "id": id, "keys": keys, "defaultKeys": dkeys, "group": d["group"].as_str().unwrap_or("Other"),
                "label": d["label"].as_str().unwrap_or(id), "action": d["action"], "opts": if d["opts"].is_null() { json!({}) } else { d["opts"].clone() },
                "custom": false, "disabled": disabled, "changed": keys_changed || disabled, "keysChanged": keys_changed,
                "isDefault": !keys_changed && !disabled,
                "keyList": key_list(keys, &self.mod_key), "keyText": self.key_text(keys), "actionText": self.describe(&d["action"])
            }));
        }
        for s in self.user["shortcuts"].as_array().into_iter().flatten() {
            let Some(id) = s["id"].as_str().filter(|i| !i.is_empty()) else { continue };
            let keys = s["keys"].as_str().unwrap_or("");
            let label = s["label"].as_str().filter(|l| !l.is_empty()).map(String::from).unwrap_or_else(|| self.describe(&s["action"]));
            out.push(json!({
                "id": id, "keys": keys, "defaultKeys": "", "group": "Your shortcuts", "label": label, "action": s["action"],
                "opts": if s["opts"].is_null() { json!({}) } else { s["opts"].clone() }, "custom": true, "disabled": truthy(&s["disabled"]),
                "changed": false, "keysChanged": false, "isDefault": false,
                "keyList": key_list(keys, &self.mod_key), "keyText": self.key_text(keys), "actionText": self.describe(&s["action"])
            }));
        }
        out
    }

    fn decorate_gesture(&self, mut g: Value) -> Value {
        g["text"] = json!(format!("{} fingers, {}", js_str(&g["fingers"]), direction_label(g["direction"].as_str().unwrap_or("")).to_lowercase()));
        g["actionText"] = json!(self.describe_gesture(&g["action"]));
        g
    }

    /// Gestures: { id, fingers, direction, label, action, custom, disabled,
    /// changed, isDefault, text ("4 fingers up"), actionText }
    pub fn gestures(&self) -> Vec<Value> {
        let mut out = Vec::new();
        let none = json!({});
        for d in &self.gesture_defaults {
            let id = d["id"].as_str().unwrap_or("");
            let c = self.user["gestures"]["changes"].get(id).unwrap_or(&none);
            let pick = |k: &str| if c[k].is_null() { d[k].clone() } else { c[k].clone() };
            let (fingers, direction, action) = (pick("fingers"), pick("direction"), pick("action"));
            let disabled = truthy(&c["disabled"]);
            let changed = disabled || fingers != d["fingers"] || direction != d["direction"] || !Self::same_action(&action, &d["action"]);
            out.push(self.decorate_gesture(json!({
                "id": id, "fingers": fingers, "direction": direction, "action": action,
                "label": d["label"].as_str().unwrap_or(id), "custom": false, "disabled": disabled, "changed": changed, "isDefault": !changed
            })));
        }
        for x in self.user["gestures"]["custom"].as_array().into_iter().flatten() {
            let Some(id) = x["id"].as_str().filter(|i| !i.is_empty()) else { continue };
            let label = x["label"].as_str().filter(|l| !l.is_empty()).map(String::from).unwrap_or_else(|| self.describe_gesture(&x["action"]));
            out.push(self.decorate_gesture(json!({
                "id": id, "fingers": x["fingers"], "direction": x["direction"], "action": x["action"], "label": label,
                "custom": true, "disabled": truthy(&x["disabled"]), "changed": false, "isDefault": false
            })));
        }
        out
    }

    pub fn bind(&self, id: &str) -> Option<Value> {
        self.binds().into_iter().find(|b| b["id"] == id)
    }

    pub fn three_finger_drag(&self) -> bool {
        self.gestures().iter().find(|g| g["id"] == "move").is_some_and(|m| m["disabled"] != true && m["fingers"] == 3)
    }

    /// Binds (turned on) that use these keys, except `except_id`
    pub fn conflicts(&self, keys: &str, except_id: &str) -> Vec<Value> {
        if keys.is_empty() {
            return vec![];
        }
        let c = canonical(keys);
        self.binds().into_iter().filter(|b| b["disabled"] != true && b["id"] != except_id && canonical(b["keys"].as_str().unwrap_or("")) == c).collect()
    }

    // ── Editing ──

    fn change(&mut self, f: impl FnOnce(&mut Value)) {
        let mut u = self.user.clone();
        f(&mut u);
        self.user = u;
        self.revision += 1;
        self.dirty = true;
    }

    fn is_default_id(&self, id: &str) -> bool {
        self.defaults.iter().any(|d| d["id"] == id)
    }

    pub fn set_keys(&mut self, id: &str, keys: &str) {
        let d = self.defaults.iter().find(|x| x["id"] == id).cloned();
        self.change(|u| {
            if let Some(d) = d {
                let mut c = u["keybinds"].get(id).cloned().filter(Value::is_object).unwrap_or_else(|| json!({}));
                if canonical(keys) == canonical(d["keys"].as_str().unwrap_or("")) {
                    c.as_object_mut().map(|o| o.shift_remove("keys"));
                } else {
                    c["keys"] = json!(keys);
                }
                if is_empty_obj(&c) {
                    u["keybinds"].as_object_mut().map(|o| o.shift_remove(id));
                } else {
                    u["keybinds"][id] = c;
                }
            } else if let Some(s) = u["shortcuts"].as_array_mut().and_then(|a| a.iter_mut().find(|x| x["id"] == id)) {
                s["keys"] = json!(keys);
            }
        });
    }

    fn apply_disable(&self, u: &mut Value, id: &str, on: bool) {
        if self.is_default_id(id) {
            let mut c = u["keybinds"].get(id).cloned().filter(Value::is_object).unwrap_or_else(|| json!({}));
            if on {
                c["disabled"] = json!(true);
            } else {
                c.as_object_mut().map(|o| o.shift_remove("disabled"));
            }
            if is_empty_obj(&c) {
                u["keybinds"].as_object_mut().map(|o| o.shift_remove(id));
            } else {
                u["keybinds"][id] = c;
            }
        } else if let Some(s) = u["shortcuts"].as_array_mut().and_then(|a| a.iter_mut().find(|x| x["id"] == id)) {
            if on {
                s["disabled"] = json!(true);
            } else {
                s.as_object_mut().map(|o| o.shift_remove("disabled"));
            }
        }
    }

    pub fn disable(&mut self, id: &str, on: bool) {
        let mut u = self.user.clone();
        self.apply_disable(&mut u, id, on);
        self.change(|x| *x = u);
    }

    pub fn reset(&mut self, id: &str) {
        self.change(|u| {
            u["keybinds"].as_object_mut().map(|o| o.shift_remove(id));
        });
    }

    /// Keys for one bind, turning off the ones that used them
    pub fn set_keys_taking_over(&mut self, id: &str, keys: &str) {
        let others: Vec<String> = self.conflicts(keys, id).iter().filter_map(|b| b["id"].as_str().map(String::from)).collect();
        let mut u = self.user.clone();
        for o in &others {
            self.apply_disable(&mut u, o, true);
        }
        self.change(|x| *x = u);
        self.set_keys(id, keys);
    }

    pub fn clean_shortcut(e: &Value) -> Value {
        let id = e["id"].as_str().filter(|i| !i.is_empty()).map(String::from).unwrap_or_else(|| new_id("custom-"));
        let mut s = json!({"id": id, "keys": e["keys"].as_str().unwrap_or(""), "group": "Custom", "label": e["label"].as_str().unwrap_or(""), "action": e["action"]});
        if e["opts"].as_object().is_some_and(|o| !o.is_empty()) {
            s["opts"] = e["opts"].clone();
        }
        if truthy(&e["disabled"]) {
            s["disabled"] = json!(true);
        }
        s
    }

    /// Returns the new shortcut's id
    pub fn add_shortcut(&mut self, entry: &Value) -> String {
        let s = Self::clean_shortcut(entry);
        let id = s["id"].as_str().unwrap_or("").to_string();
        self.change(|u| u["shortcuts"].as_array_mut().map(|a| a.push(s)).unwrap_or(()));
        id
    }

    pub fn update_shortcut(&mut self, id: &str, entry: &Value) {
        self.change(|u| {
            if let Some(a) = u["shortcuts"].as_array_mut() {
                if let Some(i) = a.iter().position(|x| x["id"] == id) {
                    let mut merged = a[i].clone();
                    if let Some(o) = entry.as_object() {
                        for (k, v) in o {
                            merged[k] = v.clone();
                        }
                    }
                    merged["id"] = json!(id);
                    a[i] = Self::clean_shortcut(&merged);
                }
            }
        });
    }

    pub fn remove_shortcut(&mut self, id: &str) {
        self.change(|u| {
            if let Some(a) = u["shortcuts"].as_array_mut() {
                a.retain(|x| x["id"] != id);
            }
        });
    }

    // ── Gestures ──

    /// Three-finger drag (Settings → Trackpad), as on macOS: on, three fingers
    /// move the window under them and spaces swipe with four (the defaults);
    /// off, three fingers swipe between spaces and nothing drags
    pub fn set_three_finger_drag(&mut self, on: bool) {
        self.change(|u| {
            let ch = &mut u["gestures"]["changes"];
            let mut set = |id: &str, k: &str, v: Option<Value>| {
                let mut c = ch.get(id).cloned().filter(Value::is_object).unwrap_or_else(|| json!({}));
                match v {
                    None => {
                        c.as_object_mut().map(|o| o.shift_remove(k));
                    }
                    Some(v) => c[k] = v,
                }
                if is_empty_obj(&c) {
                    ch.as_object_mut().map(|o| o.shift_remove(id));
                } else {
                    ch[id] = c;
                }
            };
            set("move", "disabled", if on { None } else { Some(json!(true)) });
            set("move", "fingers", None);
            set("workspaces", "fingers", if on { None } else { Some(json!(3)) });
        });
    }

    /// patch: any of { fingers, direction, action }
    pub fn set_gesture(&mut self, id: &str, patch: &Value) {
        let d = self.gesture_defaults.iter().find(|x| x["id"] == id).cloned();
        let label = (!patch["action"].is_null()).then(|| self.describe_gesture(&patch["action"]));
        self.change(|u| {
            if let Some(d) = d {
                let mut c = u["gestures"]["changes"].get(id).cloned().filter(Value::is_object).unwrap_or_else(|| json!({}));
                if let Some(p) = patch.as_object() {
                    for (k, v) in p {
                        c[k] = v.clone();
                    }
                }
                for k in ["fingers", "direction"] {
                    if c[k] == d[k] {
                        c.as_object_mut().map(|o| o.shift_remove(k));
                    }
                }
                if !c["action"].is_null() && Self::same_action(&c["action"], &d["action"]) {
                    c.as_object_mut().map(|o| o.shift_remove("action"));
                }
                if is_empty_obj(&c) {
                    u["gestures"]["changes"].as_object_mut().map(|o| o.shift_remove(id));
                } else {
                    u["gestures"]["changes"][id] = c;
                }
            } else if let Some(g) = u["gestures"]["custom"].as_array_mut().and_then(|a| a.iter_mut().find(|x| x["id"] == id)) {
                if let Some(p) = patch.as_object() {
                    for (k, v) in p {
                        g[k] = v.clone();
                    }
                }
                if let Some(l) = label {
                    g["label"] = json!(l);
                }
            }
        });
    }

    pub fn disable_gesture(&mut self, id: &str, on: bool) {
        let d = self.gesture_defaults.iter().any(|x| x["id"] == id);
        self.change(|u| {
            if d {
                let mut c = u["gestures"]["changes"].get(id).cloned().filter(Value::is_object).unwrap_or_else(|| json!({}));
                if on {
                    c["disabled"] = json!(true);
                } else {
                    c.as_object_mut().map(|o| o.shift_remove("disabled"));
                }
                if is_empty_obj(&c) {
                    u["gestures"]["changes"].as_object_mut().map(|o| o.shift_remove(id));
                } else {
                    u["gestures"]["changes"][id] = c;
                }
            } else if let Some(g) = u["gestures"]["custom"].as_array_mut().and_then(|a| a.iter_mut().find(|x| x["id"] == id)) {
                if on {
                    g["disabled"] = json!(true);
                } else {
                    g.as_object_mut().map(|o| o.shift_remove("disabled"));
                }
            }
        });
    }

    pub fn reset_gesture(&mut self, id: &str) {
        self.change(|u| {
            u["gestures"]["changes"].as_object_mut().map(|o| o.shift_remove(id));
        });
    }

    pub fn add_gesture(&mut self, g: &Value) -> String {
        let id = new_id("gesture-");
        let x = json!({"id": id, "fingers": g["fingers"], "direction": g["direction"], "label": self.describe_gesture(&g["action"]), "action": g["action"]});
        self.change(|u| u["gestures"]["custom"].as_array_mut().map(|a| a.push(x)).unwrap_or(()));
        id
    }

    pub fn remove_gesture(&mut self, id: &str) {
        self.change(|u| {
            if let Some(a) = u["gestures"]["custom"].as_array_mut() {
                a.retain(|x| x["id"] != id);
            }
        });
    }

    // ── Undo ──

    pub fn snapshot(&self) -> String {
        self.user.to_string()
    }
    pub fn restore(&mut self, snap: &str) {
        if let Ok(v) = serde_json::from_str::<Value>(snap) {
            self.user = normalize_user(&v);
            self.revision += 1;
            self.dirty = true;
        }
    }

    /// An app from its desktop entry, as a bind action
    pub fn app_action(entry_id: &str, argv: &[String], has_gtk_launch: bool) -> Value {
        if has_gtk_launch {
            return json!({"exec": format!("gtk-launch {entry_id}"), "desktop": entry_id});
        }
        let q = |s: &str| {
            if !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || c == '_' || "@%+=:,./-".contains(c)) {
                s.to_string()
            } else {
                format!("'{}'", s.replace('\'', "'\\''"))
            }
        };
        json!({"exec": argv.iter().map(|a| q(a)).collect::<Vec<_>>().join(" "), "desktop": entry_id})
    }

    // ── Export / import ──

    /// sections: any of "keybinds", "shortcuts", "gestures"
    pub fn export_data(&self, sections: &[String]) -> Value {
        let mut out = json!({"tessera-keybinds": 1});
        let has = |s: &str| sections.iter().any(|x| x == s);
        if has("keybinds") {
            out["keybinds"] = self.user["keybinds"].clone();
        }
        if has("shortcuts") {
            out["shortcuts"] = self.user["shortcuts"].clone();
        }
        if has("gestures") {
            out["gestures"] = self.user["gestures"].clone();
        }
        out
    }

    /// A preview: { title, summary, lines: [{ text, kind }], skipped: [...],
    /// result (the user file after applying), total } or { error }
    pub fn preview_file(&self, text: &str, sections: &[String]) -> Value {
        let data: Value = match serde_json::from_str(text) {
            Ok(d) => d,
            Err(e) => return json!({"error": format!("That file isn't valid JSON ({e}). Export it again from Tessera, or fix it in a text editor.")}),
        };
        if !data.is_object() || (data.get("tessera-keybinds").is_none() && data.get("irice-keybinds").is_none()) {
            return json!({"error": "That isn't a Tessera shortcuts file. Choose a file made with Export here."});
        }
        let has = |s: &str| data.get(s).is_some_and(|v| !v.is_null());
        let wanted: Vec<&String> = sections.iter().filter(|s| has(s)).collect();
        if wanted.is_empty() {
            let names = |s: &str| match s {
                "keybinds" => "changes to default shortcuts",
                "shortcuts" => "shortcuts of your own",
                _ => "gestures",
            };
            let n: Vec<&str> = sections.iter().map(|s| names(s)).collect();
            return json!({"error": format!("That file has no {} . Choose a file exported from the same section, or one made with Export all.", n.join(" or ")).replace(" .", ".")});
        }
        let want = |s: &str| wanted.iter().any(|x| x.as_str() == s);
        let mut u = self.user.clone();
        let mut lines: Vec<Value> = Vec::new();
        let (mut changed, mut added, mut unknown, mut gestures) = (0, 0, 0, 0);
        if want("keybinds") && data["keybinds"].is_object() {
            for (id, c) in data["keybinds"].as_object().unwrap() {
                let d = self.defaults.iter().find(|x| x["id"] == id.as_str());
                let (Some(d), true) = (d, c.is_object()) else {
                    unknown += 1;
                    continue;
                };
                let mut clean = json!({});
                if let Some(k) = c["keys"].as_str() {
                    if canonical(k) != canonical(d["keys"].as_str().unwrap_or("")) {
                        clean["keys"] = json!(k);
                    }
                }
                if truthy(&c["disabled"]) {
                    clean["disabled"] = json!(true);
                }
                let cur = u["keybinds"].get(id).cloned().unwrap_or_else(|| json!({}));
                if cur.to_string() == clean.to_string() {
                    continue;
                }
                if is_empty_obj(&clean) {
                    u["keybinds"].as_object_mut().map(|o| o.shift_remove(id));
                } else {
                    u["keybinds"][id.as_str()] = clean.clone();
                }
                changed += 1;
                let shown = if clean["disabled"] == true { "off".to_string() } else { self.key_text(clean["keys"].as_str().unwrap_or(d["keys"].as_str().unwrap_or(""))) };
                lines.push(json!({"kind": "changed", "text": format!("{}: {}", d["label"].as_str().unwrap_or(""), shown)}));
            }
        }
        if want("shortcuts") && data["shortcuts"].is_array() {
            for s in data["shortcuts"].as_array().unwrap() {
                if !s["keys"].is_string() || !s["action"].is_object() {
                    unknown += 1;
                    continue;
                }
                let sid = s["id"].as_str().filter(|i| !i.is_empty());
                let arr = u["shortcuts"].as_array().cloned().unwrap_or_default();
                let i = sid.and_then(|sid| arr.iter().position(|x| x["id"] == sid));
                let dup = arr.iter().any(|x| canonical(x["keys"].as_str().unwrap_or("")) == canonical(s["keys"].as_str().unwrap_or("")) && Self::same_action(&x["action"], &s["action"]));
                let label_of = |n: &Value| n["label"].as_str().filter(|l| !l.is_empty()).map(String::from).unwrap_or_else(|| self.describe(&n["action"]));
                if let Some(i) = i {
                    let next = Self::clean_shortcut(s);
                    if arr[i].to_string() == next.to_string() {
                        continue;
                    }
                    lines.push(json!({"kind": "changed", "text": format!("{}: {}", label_of(&next), self.key_text(next["keys"].as_str().unwrap_or("")))}));
                    u["shortcuts"][i] = next;
                    changed += 1;
                } else if !dup {
                    let next = Self::clean_shortcut(s);
                    lines.push(json!({"kind": "added", "text": format!("{}: {}", label_of(&next), self.key_text(next["keys"].as_str().unwrap_or("")))}));
                    u["shortcuts"].as_array_mut().map(|a| a.push(next));
                    added += 1;
                }
            }
        }
        if want("gestures") && data["gestures"].is_object() {
            let g = &data["gestures"];
            for (id, c) in g["changes"].as_object().into_iter().flatten() {
                let Some(d) = self.gesture_defaults.iter().find(|x| x["id"] == id.as_str()) else {
                    unknown += 1;
                    continue;
                };
                let cur = u["gestures"]["changes"].get(id).cloned().unwrap_or_else(|| json!({}));
                if cur.to_string() == c.to_string() {
                    continue;
                }
                u["gestures"]["changes"][id.as_str()] = c.clone();
                gestures += 1;
                lines.push(json!({"kind": "changed", "text": format!("Gesture: {}", d["label"].as_str().unwrap_or(id))}));
            }
            for x in g["custom"].as_array().into_iter().flatten() {
                if x["fingers"].is_null() || x["direction"].is_null() || x["action"].is_null() || !x.is_object() {
                    unknown += 1;
                    continue;
                }
                let arr = u["gestures"]["custom"].as_array().cloned().unwrap_or_default();
                let xid = x["id"].as_str().filter(|i| !i.is_empty());
                let i = xid.and_then(|xid| arr.iter().position(|y| y["id"] == xid));
                let label = x["label"].as_str().filter(|l| !l.is_empty()).map(String::from).unwrap_or_else(|| self.describe_gesture(&x["action"]));
                let next = json!({"id": xid.map(String::from).unwrap_or_else(|| new_id("gesture-")), "fingers": x["fingers"], "direction": x["direction"], "label": label, "action": x["action"]});
                if let Some(i) = i {
                    if arr[i].to_string() == next.to_string() {
                        continue;
                    }
                    u["gestures"]["custom"][i] = next.clone();
                } else {
                    u["gestures"]["custom"].as_array_mut().map(|a| a.push(next.clone()));
                }
                gestures += 1;
                lines.push(json!({"kind": "added", "text": format!("Gesture: {} fingers, {} → {}", js_str(&next["fingers"]), direction_label(next["direction"].as_str().unwrap_or("")).to_lowercase(), self.describe_gesture(&next["action"]))}));
            }
        }
        let mut parts: Vec<String> = Vec::new();
        if changed > 0 {
            parts.push(format!("{changed}{}", if changed == 1 { " shortcut changed" } else { " shortcuts changed" }));
        }
        if added > 0 {
            parts.push(format!("{added} added"));
        }
        if gestures > 0 {
            parts.push(format!("{gestures}{}", if gestures == 1 { " gesture" } else { " gestures" }));
        }
        if unknown > 0 {
            parts.push(format!("{unknown} skipped (not in this version of Tessera)"));
        }
        let total = changed + added + gestures;
        json!({
            "title": "Import from file",
            "summary": if total == 0 { "Nothing to change: this file matches what you have.".to_string() } else { parts.join(", ") },
            "lines": lines, "skipped": [], "result": u, "total": total
        })
    }

    /// end4: matches the helper's parsed output to Tessera's binds
    pub fn match_end4(&self, parsed: &Value) -> Value {
        let mut u = self.user.clone();
        let mut lines: Vec<Value> = Vec::new();
        let current = self.binds();
        let pbinds = parsed["binds"].as_array().cloned().unwrap_or_default();
        // How many binds each action has, on each side
        let mut end4_by: Vec<(String, Vec<Value>)> = Vec::new();
        for e in &pbinds {
            let k = action_key(&e["action"]);
            match end4_by.iter_mut().find(|(x, _)| *x == k) {
                Some((_, v)) => v.push(e.clone()),
                None => end4_by.push((k, vec![e.clone()])),
            }
        }
        let mut tessera_by: HashMap<String, Vec<Value>> = HashMap::new();
        for b in current.iter().filter(|b| b["custom"] != true) {
            tessera_by.entry(action_key(&b["action"])).or_default().push(b.clone());
        }
        // Keys end4 gives to each action, to see which Tessera binds it displaced
        let mut end4_keys: HashMap<String, String> = HashMap::new();
        for e in &pbinds {
            end4_keys.insert(canonical(e["keys"].as_str().unwrap_or("")), action_key(&e["action"]));
        }
        let mut touched: Vec<String> = Vec::new();
        let mut claimed: Vec<String> = Vec::new();
        let (mut changed, mut added, mut same, mut off) = (0, 0, 0, 0);
        for (k, ends) in &end4_by {
            let all = tessera_by.get(k).cloned().unwrap_or_default();
            let mut mine = all.clone();
            let mut left: Vec<Value> = Vec::new();
            for e in ends {
                let ek = canonical(e["keys"].as_str().unwrap_or(""));
                if let Some(i) = mine.iter().position(|b| canonical(b["keys"].as_str().unwrap_or("")) == ek && b["disabled"] != true) {
                    touched.push(mine[i]["id"].as_str().unwrap_or("").into());
                    claimed.push(ek);
                    mine.remove(i);
                    same += 1;
                } else {
                    left.push(e.clone());
                }
            }
            for e in &left {
                // Move a Tessera default when it's unambiguous (one bind each
                // side) or end4 gave its keys to something else
                let i = if ends.len() == 1 && all.len() == 1 && mine.len() == 1 {
                    Some(0)
                } else {
                    mine.iter().position(|b| end4_keys.get(&canonical(b["keys"].as_str().unwrap_or(""))).is_some_and(|x| x != k))
                };
                let ekeys = e["keys"].as_str().unwrap_or("");
                let label = e["label"].as_str().filter(|l| !l.is_empty()).map(String::from).unwrap_or_else(|| {
                    if let Some(s) = e["action"]["shell"].as_str() {
                        shell_label(s)
                    } else {
                        self.describe(&e["action"])
                    }
                });
                if let Some(i) = i {
                    let b = mine.remove(i);
                    let bid = b["id"].as_str().unwrap_or("").to_string();
                    let mut c = u["keybinds"].get(&bid).cloned().filter(Value::is_object).unwrap_or_else(|| json!({}));
                    c.as_object_mut().map(|o| o.shift_remove("disabled"));
                    if canonical(ekeys) == canonical(b["defaultKeys"].as_str().unwrap_or("")) {
                        c.as_object_mut().map(|o| o.shift_remove("keys"));
                    } else {
                        c["keys"] = json!(ekeys);
                    }
                    if is_empty_obj(&c) {
                        u["keybinds"].as_object_mut().map(|o| o.shift_remove(&bid));
                    } else {
                        u["keybinds"][bid.as_str()] = c;
                    }
                    touched.push(bid);
                    claimed.push(canonical(ekeys));
                    changed += 1;
                    lines.push(json!({"kind": "changed", "text": format!("{}: {} → {}", b["label"].as_str().unwrap_or(""), self.key_text(b["keys"].as_str().unwrap_or("")), self.key_text(ekeys))}));
                } else {
                    // Already one of yours?
                    if u["shortcuts"].as_array().into_iter().flatten().any(|s| canonical(s["keys"].as_str().unwrap_or("")) == canonical(ekeys) && Self::same_action(&s["action"], &e["action"])) {
                        same += 1;
                        claimed.push(canonical(ekeys));
                        continue;
                    }
                    let s = Self::clean_shortcut(&json!({"keys": ekeys, "label": label, "action": e["action"], "opts": e["opts"]}));
                    touched.push(s["id"].as_str().unwrap_or("").into());
                    u["shortcuts"].as_array_mut().map(|a| a.push(s));
                    claimed.push(canonical(ekeys));
                    added += 1;
                    lines.push(json!({"kind": "added", "text": format!("{label}: {}", self.key_text(ekeys))}));
                }
            }
        }
        // end4 wins a clash: Tessera binds left on keys end4 now uses turn off
        for b in &current {
            let bid = b["id"].as_str().unwrap_or("");
            if touched.iter().any(|t| t == bid) || b["disabled"] == true {
                continue;
            }
            let k = u["keybinds"][bid]["keys"].as_str().map(String::from).unwrap_or_else(|| if b["custom"] == true { b["keys"].as_str().unwrap_or("").into() } else { b["defaultKeys"].as_str().unwrap_or("").into() });
            if claimed.contains(&canonical(&k)) {
                let mut u2 = u.clone();
                self.apply_disable(&mut u2, bid, true);
                u = u2;
                off += 1;
                lines.push(json!({"kind": "off", "text": format!("{} ({}) turns off: end4 uses those keys", b["label"].as_str().unwrap_or(""), self.key_text(&k))}));
            }
        }
        // Gestures: same fingers and direction → that gesture's action
        let (mut g_changed, mut g_added) = (0, 0);
        let g_now = self.gestures();
        for g in parsed["gestures"].as_array().into_iter().flatten() {
            let at = g_now.iter().find(|x| x["fingers"] == g["fingers"] && x["direction"] == g["direction"]);
            if let Some(at) = at {
                if Self::same_action(&at["action"], &g["action"]) && at["disabled"] != true {
                    continue;
                }
            }
            match at {
                Some(at) if at["custom"] != true => {
                    let id = at["id"].as_str().unwrap_or("");
                    let d = self.gesture_defaults.iter().find(|x| x["id"] == id).cloned().unwrap_or_else(|| json!({}));
                    let mut c = u["gestures"]["changes"].get(id).cloned().filter(Value::is_object).unwrap_or_else(|| json!({}));
                    c.as_object_mut().map(|o| o.shift_remove("disabled"));
                    if Self::same_action(&d["action"], &g["action"]) {
                        c.as_object_mut().map(|o| o.shift_remove("action"));
                    } else {
                        c["action"] = g["action"].clone();
                    }
                    if is_empty_obj(&c) {
                        u["gestures"]["changes"].as_object_mut().map(|o| o.shift_remove(id));
                    } else {
                        u["gestures"]["changes"][id] = c;
                    }
                    g_changed += 1;
                    lines.push(json!({"kind": "changed", "text": format!("Gesture {}: {}", at["text"].as_str().unwrap_or(""), self.describe_gesture(&g["action"]))}));
                }
                Some(_) => {}
                None => {
                    u["gestures"]["custom"].as_array_mut().map(|a| a.push(json!({"id": new_id("gesture-"), "fingers": g["fingers"], "direction": g["direction"], "label": self.describe_gesture(&g["action"]), "action": g["action"]})));
                    g_added += 1;
                    lines.push(json!({"kind": "added", "text": format!("Gesture {} fingers, {}: {}", js_str(&g["fingers"]), direction_label(g["direction"].as_str().unwrap_or("")).to_lowercase(), self.describe_gesture(&g["action"]))}));
                }
            }
        }
        // Skipped, without end4's fallbacks (they only run without its shell)
        let mut seen: Vec<String> = Vec::new();
        let mut skipped: Vec<Value> = Vec::new();
        for s in parsed["skipped"].as_array().into_iter().flatten() {
            if truthy(&s["fallback"]) {
                continue;
            }
            let key = format!("{}|{}", canonical(s["keys"].as_str().unwrap_or("")), s["reason"].as_str().unwrap_or(""));
            if seen.contains(&key) {
                continue;
            }
            seen.push(key);
            let label = s["label"].as_str().filter(|l| !l.is_empty()).map(|l| format!("{l} ")).unwrap_or_default();
            skipped.push(json!({"text": format!("{label}({}): {}", self.key_text(s["keys"].as_str().unwrap_or("")), s["reason"].as_str().unwrap_or(""))}));
        }
        for s in parsed["gesturesSkipped"].as_array().into_iter().flatten() {
            skipped.push(json!({"text": format!("Gesture {} fingers {}: {}", js_str(&s["fingers"]), s["direction"].as_str().unwrap_or(""), s["reason"].as_str().unwrap_or(""))}));
        }
        let mut parts: Vec<String> = Vec::new();
        if changed + same + added + off > 0 || !pbinds.is_empty() {
            parts.push(format!("{changed}{}", if changed == 1 { " key changed" } else { " keys changed" }));
            parts.push(format!("{added} added"));
        }
        if off > 0 {
            parts.push(format!("{off} turned off"));
        }
        if g_changed + g_added > 0 {
            parts.push(format!("{}{}", g_changed + g_added, if g_changed + g_added == 1 { " gesture" } else { " gestures" }));
        }
        parts.push(format!("{} skipped", skipped.len()));
        let total = changed + added + off + g_changed + g_added;
        let ignored = parsed["ignored"].as_i64().unwrap_or(0);
        json!({
            "title": "Import from end4",
            "summary": format!("Import from end4: {}{}", parts.join(", "), if total == 0 { format!(". {same} already match.") } else { String::new() }),
            "lines": lines, "skipped": skipped, "result": u, "total": total,
            "note": if ignored > 0 { format!("{ignored} binds inside loops (workspaces 1–10) are left out: Tessera's defaults have them.") } else { String::new() }
        })
    }

    /// Applies a preview's result
    pub fn apply_preview(&mut self, p: &Value) {
        if p["result"].is_null() {
            return;
        }
        self.user = normalize_user(&p["result"]);
        self.revision += 1;
        self.dirty = true;
    }
}

fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().unwrap_or(0.0) != 0.0,
        Value::String(s) => !s.is_empty(),
        _ => true,
    }
}

fn js_str(v: &Value) -> String {
    v.as_str().map(String::from).unwrap_or_else(|| v.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn km() -> Keymap {
        let mut k = Keymap::default();
        k.defaults = vec![
            json!({"id": "search", "keys": "$MOD + Space", "group": "Basics", "label": "Search", "action": {"shell": "spotlight"}}),
            json!({"id": "lock", "keys": "$MOD + L", "group": "Basics", "label": "Lock", "action": {"shell": "lock"}}),
        ];
        k.gesture_defaults = vec![
            json!({"id": "move", "fingers": 3, "direction": "swipe", "action": "move", "label": "Move"}),
            json!({"id": "workspaces", "fingers": 4, "direction": "horizontal", "action": "workspace", "label": "Spaces"}),
        ];
        k
    }

    #[test]
    fn keys_changes_and_resets() {
        let mut k = km();
        k.set_keys("search", "$MOD + SHIFT + Space");
        assert!(k.dirty);
        let b = k.bind("search").unwrap();
        assert_eq!((b["keys"].as_str(), b["changed"].as_bool()), (Some("$MOD + SHIFT + Space"), Some(true)));
        assert_eq!(k.conflicts("$MOD + SHIFT + space", "").len(), 1);
        // back to the default keys removes the entry
        k.set_keys("search", "super + space");
        assert!(k.user["keybinds"].as_object().unwrap().is_empty());
        k.disable("lock", true);
        assert_eq!(k.bind("lock").unwrap()["disabled"], true);
        k.reset("lock");
        assert_eq!(k.bind("lock").unwrap()["disabled"], false);
    }

    #[test]
    fn shortcuts_and_undo() {
        let mut k = km();
        let snap = k.snapshot();
        let id = k.add_shortcut(&json!({"keys": "$MOD + T", "label": "", "action": {"app": "terminal"}}));
        assert!(id.starts_with("custom-"));
        let b = k.bind(&id).unwrap();
        assert_eq!((b["label"].as_str(), b["group"].as_str()), (Some("Opens your terminal"), Some("Your shortcuts")));
        k.restore(&snap);
        assert!(k.bind(&id).is_none());
    }

    #[test]
    fn three_finger_drag_toggles_two_gestures() {
        let mut k = km();
        assert!(k.three_finger_drag());
        k.set_three_finger_drag(false);
        assert!(!k.three_finger_drag());
        assert_eq!(k.user["gestures"]["changes"]["workspaces"]["fingers"], 3);
        k.set_three_finger_drag(true);
        assert!(k.user["gestures"]["changes"].as_object().unwrap().is_empty());
    }

    #[test]
    fn import_preview_and_export() {
        let mut k = km();
        k.set_keys("lock", "$MOD + ALT + L");
        let text = serde_json::to_string(&k.export_data(&["keybinds".to_string()])).unwrap();
        let fresh = km();
        let p = fresh.preview_file(&text, &["keybinds".to_string()]);
        assert_eq!(p["total"], 1);
        assert!(p["summary"].as_str().unwrap().contains("1 shortcut changed"));
        assert!(fresh.preview_file("nope", &["keybinds".to_string()])["error"].is_string());
        assert!(fresh.preview_file(&text, &["gestures".to_string()])["error"].as_str().unwrap().contains("gestures"));
        let mut f2 = km();
        f2.apply_preview(&p);
        assert_eq!(f2.bind("lock").unwrap()["keys"], "$MOD + ALT + L");
    }

    #[test]
    fn gestures_list_and_edit() {
        let mut k = km();
        k.set_gesture("workspaces", &json!({"fingers": 3}));
        let g = k.gestures().into_iter().find(|g| g["id"] == "workspaces").unwrap();
        assert_eq!((g["changed"].as_bool(), g["text"].as_str()), (Some(true), Some("3 fingers, left or right")));
        k.reset_gesture("workspaces");
        let id = k.add_gesture(&json!({"fingers": 2, "direction": "pinch", "action": "fullscreen"}));
        assert_eq!(k.gestures().last().unwrap()["id"], id);
        assert_eq!(k.gestures().last().unwrap()["label"], "Full screen");
    }

    #[test]
    fn end4_matching() {
        let k = km();
        let parsed = json!({"binds": [{"keys": "$MOD + K", "action": {"shell": "spotlight"}, "label": ""}], "skipped": [], "gestures": [], "gesturesSkipped": [], "ignored": 2});
        let p = k.match_end4(&parsed);
        assert_eq!(p["total"], 1);
        assert_eq!(p["result"]["keybinds"]["search"]["keys"], "$MOD + K");
        assert!(p["note"].as_str().unwrap().starts_with("2 binds"));
    }
}

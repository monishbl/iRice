//! The shortcuts, read live from Hyprland (services/Keybinds.qml): binds
//! with a description "Group: Action" are listed under that group, the
//! numbered ones merged ("Go to space 1–9"), and the keys written in the
//! chosen style (common/KeyNames.qml: Settings → Appearance → Shortcut
//! symbols).

use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize, Default)]
struct Bind {
    #[serde(default)]
    modmask: u32,
    #[serde(default)]
    key: String,
    #[serde(default)]
    description: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub keys: Vec<String>,
    pub description: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub name: String,
    pub binds: Vec<Row>,
}

/// Keybinds.prettyKey
pub fn pretty_key(k: &str) -> String {
    let mapped = match k {
        "Return" => Some("Enter"),
        "space" | "Space" => Some("Space"),
        "Tab" => Some("Tab"),
        "Escape" => Some("Esc"),
        "Print" => Some("Print Screen"),
        "Period" | "period" => Some("."),
        "Comma" => Some(","),
        "Slash" => Some("/"),
        "mouse:272" => Some("Left Click"),
        "mouse:273" => Some("Right Click"),
        "mouse:274" => Some("Middle Click"),
        "SUPER_L" | "SUPER_R" => Some(""),
        "mouse_down" => Some("Scroll Down"),
        "mouse_up" => Some("Scroll Up"),
        _ => None,
    };
    if let Some(m) = mapped {
        return m.to_string();
    }
    if let Some(rest) = k.strip_prefix("XF86") {
        // "AudioRaiseVolume" -> "Audio Raise Volume"
        let mut out = String::new();
        let mut prev_lower = false;
        for c in rest.chars() {
            if c.is_ascii_uppercase() && prev_lower {
                out.push(' ');
            }
            prev_lower = c.is_ascii_lowercase();
            out.push(c);
        }
        return out;
    }
    if k.chars().count() == 1 { k.to_uppercase() } else { k.to_string() }
}

/// "Go to space 1" … "Go to space 9" → one row "Go to space 1–9" with keys "1–9"
pub fn merge_numbered(binds: Vec<Row>) -> Vec<Row> {
    let mut out: Vec<Row> = Vec::new();
    // key -> (index in out, min, max)
    let mut seen: Vec<(String, usize, u32, u32)> = Vec::new();
    for b in binds {
        let num = b.description.rsplit_once(' ').and_then(|(head, n)| n.parse::<u32>().ok().filter(|_| n.chars().all(|c| c.is_ascii_digit())).map(|v| (head.to_string(), n.to_string(), v)));
        let last = b.keys.last().cloned().unwrap_or_default();
        let Some((head, n, v)) = num.filter(|(_, n, _)| *n == last) else {
            out.push(b);
            continue;
        };
        let _ = n;
        let key = format!("{head}|{}", b.keys[..b.keys.len() - 1].join("+"));
        if let Some(e) = seen.iter_mut().find(|e| e.0 == key) {
            e.2 = e.2.min(v);
            e.3 = e.3.max(v);
            let row = &mut out[e.1];
            row.description = format!("{head} {}–{}", e.2, e.3);
            let mut keys = b.keys[..b.keys.len() - 1].to_vec();
            keys.push(format!("{}–{}", e.2, e.3));
            row.keys = keys;
            continue;
        }
        seen.push((key, out.len(), v, v));
        out.push(b);
    }
    out
}

/// `hyprctl -j binds` into groups, in the order they first appear
pub fn groups(json: &str) -> Option<Vec<Group>> {
    let binds: Vec<Bind> = serde_json::from_str(json).ok()?;
    let mut order: Vec<Group> = Vec::new();
    for b in binds {
        if b.description.is_empty() {
            continue;
        }
        let (group, what) = match b.description.find(": ") {
            Some(i) if i > 0 => (b.description[..i].to_string(), b.description[i + 2..].to_string()),
            _ => ("General".to_string(), b.description.clone()),
        };
        let mut keys: Vec<String> = [(64, "Super"), (4, "Ctrl"), (8, "Alt"), (1, "Shift")].iter().filter(|(bit, _)| b.modmask & bit != 0).map(|(_, n)| n.to_string()).collect();
        let key = pretty_key(&b.key);
        if !key.is_empty() {
            keys.push(key);
        }
        let g = match order.iter().position(|g| g.name == group) {
            Some(i) => &mut order[i],
            None => {
                order.push(Group { name: group, binds: Vec::new() });
                order.last_mut().unwrap()
            }
        };
        if !g.binds.iter().any(|x| x.description == what) {
            g.binds.push(Row { keys, description: what });
        }
    }
    Some(order.into_iter().map(|g| Group { name: g.name, binds: merge_numbered(g.binds) }).collect())
}

/// KeyNames.style: mac | windows | linux ("auto" follows the menu icon)
pub fn style(cfg: &Value) -> &'static str {
    match cfg["appearance"]["keySymbols"].as_str().unwrap_or("auto") {
        "mac" => "mac",
        "windows" => "windows",
        "linux" => "linux",
        _ if cfg["appearance"]["menuIcon"].as_str() == Some("command") => "mac",
        _ => "linux",
    }
}

fn names(style: &str, key: &str) -> Option<&'static str> {
    let mac = style == "mac";
    let win = style == "windows";
    Some(match key {
        "SUPER" => if mac { "⌘" } else if win { "Win" } else { "Super" },
        "CTRL" => if mac { "⌃" } else { "Ctrl" },
        "ALT" => if mac { "⌥" } else { "Alt" },
        "SHIFT" => if mac { "⇧" } else { "Shift" },
        "ESCAPE" => if mac { "⎋" } else { "Esc" },
        "RETURN" => if mac { "↩" } else { "Enter" },
        "BACKSPACE" => if mac { "⌫" } else { "Backspace" },
        "TAB" => if mac { "⇥" } else { "Tab" },
        "SPACE" => "Space",
        "LEFT" => "←",
        "RIGHT" => "→",
        "UP" => "↑",
        "DOWN" => "↓",
        _ => return None,
    })
}

/// KeyNames.name: one key's label
pub fn name(style: &str, k: &str) -> String {
    let up = k.trim_end_matches("_L").trim_end_matches("_R").to_uppercase();
    let up = match up.as_str() {
        "CONTROL" => "CTRL".to_string(),
        "META" | "MOD4" | "WIN" => "SUPER".to_string(),
        _ => up,
    };
    if let Some(n) = names(style, &up) {
        return n.to_string();
    }
    if k.chars().count() == 1 { k.to_uppercase() } else { k.to_string() }
}

/// KeyNames.keys: modifiers first in the platform's order, then the rest
pub fn keys(style: &str, list: &[String]) -> Vec<String> {
    let norm = |k: &str| k.to_uppercase().trim_end_matches("_L").trim_end_matches("_R").replace("CONTROL", "CTRL");
    let up: Vec<String> = list.iter().map(|k| norm(k)).collect();
    let mods = ["CTRL", "ALT", "SHIFT", "SUPER"];
    let m: Vec<&str> = mods.iter().copied().filter(|x| up.iter().any(|u| u == x)).collect();
    let ordered: Vec<&str> = if style == "mac" { m } else { ["SUPER", "CTRL", "ALT", "SHIFT"].into_iter().filter(|x| m.contains(x)).collect() };
    let rest = list.iter().zip(up.iter()).filter(|(_, u)| !mods.contains(&u.as_str())).map(|(k, _)| k.clone());
    ordered.into_iter().map(String::from).chain(rest).map(|k| name(style, &k)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn groups_from_hyprland() {
        let j = r#"[
            {"modmask":64,"key":"Space","description":"Launch: Search"},
            {"modmask":64,"key":"1","description":"Spaces: Go to space 1"},
            {"modmask":64,"key":"2","description":"Spaces: Go to space 2"},
            {"modmask":64,"key":"9","description":"Spaces: Go to space 9"},
            {"modmask":65,"key":"1","description":"Spaces: Move to space 1"},
            {"modmask":0,"key":"XF86AudioRaiseVolume","description":"Volume up"},
            {"modmask":64,"key":"Q","description":""},
            {"modmask":64,"key":"Return","description":"Launch: Search"}
        ]"#;
        let g = groups(j).unwrap();
        assert_eq!(g.iter().map(|g| g.name.as_str()).collect::<Vec<_>>(), vec!["Launch", "Spaces", "General"]);
        assert_eq!(g[0].binds, vec![Row { keys: vec!["Super".into(), "Space".into()], description: "Search".into() }]);
        assert_eq!(g[1].binds[0], Row { keys: vec!["Super".into(), "1–9".into()], description: "Go to space 1–9".into() });
        assert_eq!(g[1].binds[1].keys, vec!["Super", "Shift", "1"]);
        assert_eq!(g[2].binds[0].keys, vec!["Audio Raise Volume"]);
        assert!(groups("not json").is_none());
    }

    #[test]
    fn keys_in_each_style() {
        let k: Vec<String> = ["Super", "Shift", "S"].iter().map(|s| s.to_string()).collect();
        assert_eq!(keys("linux", &k), vec!["Super", "Shift", "S"]);
        assert_eq!(keys("windows", &k), vec!["Win", "Shift", "S"]);
        assert_eq!(keys("mac", &k), vec!["⇧", "⌘", "S"]);
        assert_eq!(keys("linux", &["Enter".to_string()]), vec!["Enter"]);
        assert_eq!(style(&json!({})), "linux");
        assert_eq!(style(&json!({"appearance": {"menuIcon": "command"}})), "mac");
        assert_eq!(style(&json!({"appearance": {"keySymbols": "windows"}})), "windows");
        assert_eq!(pretty_key("period"), ".");
        assert_eq!(pretty_key("q"), "Q");
    }
}

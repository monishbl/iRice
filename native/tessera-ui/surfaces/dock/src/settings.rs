//! The dock's settings from config.json (`dock.*`), with Config.qml's
//! defaults when a key is missing, and writing the few the dock itself
//! changes (Keep in Dock, Remove from Dock) back the way the shell would.

use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutoHide {
    Off,
    Always,
    /// only while a window overlaps the dock
    Windows,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub enabled: bool,
    pub pinned: Vec<String>,
    pub icon_size: f32,
    /// space between icons, a fraction of the icon size (0.05 .. 0.4)
    pub gap: f32,
    pub apps_button: bool,
    pub magnification: bool,
    pub magnified_scale: f32,
    pub auto_hide: AutoHide,
    pub recent_apps: bool,
    pub downloads: bool,
    pub trash: bool,
}

pub const DEFAULT_PINNED: &[&str] = &["org.kde.dolphin", "thunar", "firefox", "code", "foot", "org.gnome.Calculator", "org.xfce.mousepad"];

impl Settings {
    pub fn read(cfg: &Value) -> Self {
        let d = &cfg["dock"];
        let b = |k: &str, def: bool| d[k].as_bool().unwrap_or(def);
        let f = |k: &str, def: f64| d[k].as_f64().unwrap_or(def) as f32;
        Settings {
            enabled: b("enabled", true),
            pinned: match d["pinned"].as_array() {
                Some(a) => a.iter().filter_map(|v| v.as_str().map(String::from)).collect(),
                None => DEFAULT_PINNED.iter().map(|s| s.to_string()).collect(),
            },
            icon_size: f("iconSize", 48.0),
            gap: f("gap", 0.18),
            apps_button: b("appsButton", true),
            magnification: b("magnification", false),
            magnified_scale: f("magnifiedScale", 1.45),
            auto_hide: match d["autoHide"].as_str().unwrap_or("off") {
                "always" => AutoHide::Always,
                "windows" => AutoHide::Windows,
                _ => AutoHide::Off,
            },
            recent_apps: b("recentApps", true),
            downloads: b("downloads", true),
            trash: b("trash", true),
        }
    }
}

/// config.json with one change made, as the shell would write it (Apps.pin,
/// unpin, "Remove from Dock"): read again just now, so nothing else changes
pub fn change(cfg: &mut Value, key: &str, value: Value) {
    if !cfg.is_object() {
        *cfg = Value::Object(Default::default());
    }
    if !cfg["dock"].is_object() {
        cfg["dock"] = Value::Object(Default::default());
    }
    cfg["dock"][key] = value;
}

/// Keep in Dock (at the end) / Remove from Dock
pub fn pinned_with(pinned: &[String], id: &str, keep: bool) -> Vec<String> {
    let mut p: Vec<String> = pinned.iter().filter(|x| *x != id).cloned().collect();
    if keep {
        if pinned.iter().any(|x| x == id) {
            return pinned.to_vec();
        }
        p.push(id.to_string());
    }
    p
}

/// Write a dock setting into config.json (in place, as one write: the shell
/// watches the file and reloads it, and so does this process; a rename
/// would leave a watcher on the old file)
pub fn write(key: &str, value: Value) {
    let path = crate::theme::config_path();
    let key = key.to_string();
    std::thread::spawn(move || {
        let mut cfg = std::fs::read_to_string(&path).ok().and_then(|s| serde_json::from_str::<Value>(&s).ok()).unwrap_or(Value::Null);
        change(&mut cfg, &key, value);
        let Ok(text) = serde_json::to_string_pretty(&cfg) else { return };
        let _ = std::fs::write(&path, text + "\n");
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn defaults_are_the_shells() {
        let s = Settings::read(&json!({}));
        assert!(s.enabled && s.apps_button && s.recent_apps && s.downloads && s.trash && !s.magnification);
        assert_eq!((s.icon_size, s.gap, s.magnified_scale, s.auto_hide), (48.0, 0.18, 1.45, AutoHide::Off));
        assert_eq!(s.pinned[0], "org.kde.dolphin");
    }

    /// The same defaults Config.qml has (its dock section, read from the file)
    #[test]
    fn defaults_match_config_qml() {
        let qml = std::fs::read_to_string(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../shell/common/Config.qml")).unwrap();
        let start = qml.find("property JsonObject dock: JsonObject {").unwrap();
        let section = &qml[start..start + qml[start..].find("\n            }").unwrap()];
        let value = |name: &str| -> String {
            let l = section.lines().find(|l| l.contains(&format!(" {name}:"))).unwrap_or_else(|| panic!("no {name} in Config.qml"));
            l.split_once(':').unwrap().1.split("//").next().unwrap().trim().to_string()
        };
        let s = Settings::read(&json!({}));
        assert_eq!(value("enabled"), s.enabled.to_string());
        assert_eq!(value("iconSize").parse::<f32>().unwrap(), s.icon_size);
        assert_eq!(value("gap").parse::<f32>().unwrap(), s.gap);
        assert_eq!(value("appsButton"), s.apps_button.to_string());
        assert_eq!(value("magnification"), s.magnification.to_string());
        assert_eq!(value("magnifiedScale").parse::<f32>().unwrap(), s.magnified_scale);
        assert_eq!(value("autoHide"), "\"off\"");
        assert_eq!(value("recentApps"), s.recent_apps.to_string());
        assert_eq!(value("downloads"), s.downloads.to_string());
        assert_eq!(value("trash"), s.trash.to_string());
        let pinned: Vec<String> = serde_json::from_str(&value("pinned")).unwrap();
        assert_eq!(pinned, s.pinned);
    }

    #[test]
    fn user_settings_win() {
        let s = Settings::read(&json!({"dock": {"pinned": [], "iconSize": 64, "autoHide": "windows", "magnification": true, "trash": false}}));
        assert!(s.pinned.is_empty() && s.magnification && !s.trash);
        assert_eq!((s.icon_size, s.auto_hide), (64.0, AutoHide::Windows));
        assert_eq!(Settings::read(&json!({"dock": {"autoHide": "always"}})).auto_hide, AutoHide::Always);
    }

    #[test]
    fn keep_and_remove() {
        let p: Vec<String> = vec!["a".into(), "b".into()];
        assert_eq!(pinned_with(&p, "c", true), vec!["a", "b", "c"]);
        assert_eq!(pinned_with(&p, "a", true), vec!["a", "b"]);
        assert_eq!(pinned_with(&p, "a", false), vec!["b"]);
        let mut cfg = json!({"appearance": {"look": "glass"}});
        change(&mut cfg, "appsButton", json!(false));
        assert_eq!(cfg, json!({"appearance": {"look": "glass"}, "dock": {"appsButton": false}}));
    }
}

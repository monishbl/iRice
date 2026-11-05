//! The menu bar's settings from config.json, read as the QML shell reads them:
//! the system menu's icon (appearance.menuIcon, MenuBar.qml's table), the
//! clock (appearance.clock24, menubar.showDate, menubar.showSeconds:
//! TimeFormat.qml and MenuBar.qml's clock), every other `menubar.*` key and
//! the island's (Config.qml's defaults when a key is missing).

use serde_json::Value;

/// The leftmost menu's icon, a Phosphor-Fill glyph (MenuBar.qml's choices;
/// "distro" draws the system's logo there, which the Rust bar can't yet: Linux's)
pub fn menu_icon(cfg: &Value) -> &'static str {
    match cfg["appearance"]["menuIcon"].as_str().unwrap_or("tessera") {
        "linux" | "distro" => "\u{eb02}",
        "command" => "\u{e1c4}",
        "terminal" => "\u{eae8}",
        "cpu" => "\u{e610}",
        "atom" => "\u{e5e4}",
        "planet" => "\u{e652}",
        "rocket" => "\u{e3fe}",
        "star" => "\u{e6a4}",
        "lightning" => "\u{e2de}",
        "flame" => "\u{e624}",
        "leaf" => "\u{e2da}",
        "snowflake" => "\u{e5aa}",
        "moon" => "\u{e58e}",
        "cat" => "\u{e748}",
        "ghost" => "\u{e62a}",
        "alien" => "\u{e8a6}",
        // tessera (the Badge), and iRice's names for it
        _ => "\u{e604}",
    }
}

/// strftime patterns for the bar's clock and its spoken form: "Wed 4 Nov
/// 7:26 PM" (12-hour) or "Wed 4 Nov 19:26", with seconds or without the date
/// as the settings say
pub fn clock_formats(cfg: &Value) -> (String, String) {
    let h24 = cfg["appearance"]["clock24"].as_bool().unwrap_or(false);
    let date = cfg["menubar"]["showDate"].as_bool().unwrap_or(true);
    let secs = cfg["menubar"]["showSeconds"].as_bool().unwrap_or(false);
    let time = match (h24, secs) {
        (true, false) => "%H:%M",
        (true, true) => "%H:%M:%S",
        (false, false) => "%-I:%M %p",
        (false, true) => "%-I:%M:%S %p",
    };
    let short = if date { format!("%a %-d %b  {time}") } else { time.to_string() };
    (short, format!("%A %-d %B %Y, {time}"))
}

/// Every menubar.* setting (and the island's the bar needs)
#[derive(Clone, Debug, PartialEq)]
pub struct Bar {
    pub background: bool,
    pub show_seconds: bool,
    pub show_date: bool,
    pub auto_hide: bool,
    /// off | dots | icons
    pub workspaces: String,
    pub workspace_count: i32,
    /// arrows | dynamic
    pub workspace_overflow: String,
    pub other_spaces: bool,
    pub window_title: bool,
    pub resources: bool,
    pub weather: bool,
    pub search: bool,
    pub ai: bool,
    pub control_center: bool,
    pub clock: bool,
    pub app_menu: bool,
    pub window_menu: bool,
    pub battery: bool,
    /// off | inside | outside ("true"/"false" from before: outside/off)
    pub battery_percent: String,
    pub wifi: bool,
    pub bluetooth: bool,
    pub sound: bool,
    pub keyboard_layout: bool,
    pub collapse_tray: bool,
    /// bellDot | dot | count | bellCount | off
    pub notification_indicator: String,
    pub scroll_gestures: bool,
    pub util_buttons: Vec<String>,
    // the island (island.*)
    pub island: bool,
    /// replace | split | beside
    pub activity_layout: String,
    /// one | all
    pub activity_mode: String,
    /// pieces of the collapsed live activities ("music.title"…)
    pub live_pieces: Vec<String>,
    // appearance.*
    pub screen_corners: bool,
    pub corner_radius: i32,
    pub reduce_motion: bool,
}

fn b(v: &Value, k: &str, d: bool) -> bool {
    v[k].as_bool().unwrap_or(d)
}
fn s(v: &Value, k: &str, d: &str) -> String {
    v[k].as_str().unwrap_or(d).to_string()
}
fn list(v: &Value, k: &str, d: &[&str]) -> Vec<String> {
    match v[k].as_array() {
        Some(a) => a.iter().filter_map(|x| x.as_str().map(String::from)).collect(),
        None => d.iter().map(|x| x.to_string()).collect(),
    }
}

const LIVE_PIECES: &[&str] = &[
    "music.art", "music.title", "music.scroll", "music.bars", "music.prev", "music.play", "music.next", "timer.icon", "timer.label", "timer.time", "timer.play", "timer.skip", "stopwatch.icon", "stopwatch.time", "stopwatch.play",
    "stopwatch.reset", "recording.icon", "recording.time", "recording.stop",
];

pub fn bar(cfg: &Value) -> Bar {
    let m = &cfg["menubar"];
    let i = &cfg["island"];
    let a = &cfg["appearance"];
    let percent = match &m["batteryPercent"] {
        Value::Bool(true) => "outside".to_string(),
        Value::Bool(false) => "off".to_string(),
        Value::String(x) if x == "true" => "outside".into(),
        Value::String(x) if x == "false" => "off".into(),
        Value::String(x) => x.clone(),
        _ => "off".into(),
    };
    Bar {
        background: b(m, "background", false),
        show_seconds: b(m, "showSeconds", false),
        show_date: b(m, "showDate", true),
        auto_hide: b(m, "autoHide", false),
        workspaces: s(m, "workspaces", "icons"),
        workspace_count: m["workspaceCount"].as_i64().unwrap_or(5).max(1) as i32,
        workspace_overflow: s(m, "workspaceOverflow", "arrows"),
        other_spaces: b(m, "otherSpaces", true),
        window_title: b(m, "windowTitle", false),
        resources: b(m, "resources", false),
        weather: b(m, "weather", true),
        search: b(m, "search", true),
        ai: b(m, "ai", true),
        control_center: b(m, "controlCenter", true),
        clock: b(m, "clock", true),
        app_menu: b(m, "appMenu", true),
        window_menu: b(m, "windowMenu", true),
        battery: b(m, "battery", true),
        battery_percent: percent,
        wifi: b(m, "wifi", true),
        bluetooth: b(m, "bluetooth", false),
        sound: b(m, "sound", false),
        keyboard_layout: b(m, "keyboardLayout", true),
        collapse_tray: b(m, "collapseTray", true),
        notification_indicator: s(m, "notificationIndicator", "bellDot"),
        scroll_gestures: b(m, "scrollGestures", true),
        util_buttons: list(m, "utilButtons", &["screenshot"]),
        island: b(i, "enabled", true),
        activity_layout: s(i, "activityLayout", "replace"),
        activity_mode: s(i, "activityMode", "one"),
        live_pieces: list(i, "livePieces", LIVE_PIECES),
        screen_corners: b(a, "screenCorners", true),
        corner_radius: a["screenCornerRadius"].as_i64().unwrap_or(12) as i32,
        reduce_motion: b(a, "reduceMotion", false),
    }
}

impl Bar {
    /// Is this piece of a collapsed music activity on? (Island.shows: music
    /// has no defaults of its own, so it's whatever livePieces lists)
    pub fn shows(&self, kind: &str, piece: &str) -> bool {
        self.live_pieces.iter().any(|p| p == &format!("{kind}.{piece}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_badge_by_default_and_for_old_names() {
        for v in [json!({}), json!({"appearance": {"menuIcon": "tessera"}}), json!({"appearance": {"menuIcon": "circle"}}), json!({"appearance": {"menuIcon": "irice"}})] {
            assert_eq!(menu_icon(&v), "\u{e604}");
        }
        assert_eq!(menu_icon(&json!({"appearance": {"menuIcon": "leaf"}})), "\u{e2da}");
    }

    fn config_qml() -> String {
        std::fs::read_to_string(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../shell/common/Config.qml")).unwrap()
    }

    /// Every choice Settings offers has a glyph here (Config.qml lists them)
    #[test]
    fn every_choice_has_a_glyph() {
        let cfg = config_qml();
        let line = cfg.lines().find(|l| l.contains("property string menuIcon:")).unwrap();
        // ("// leftmost menu: tessera (the badge) | distro | linux | …")
        let list = line.split("//").nth(1).unwrap().split(':').nth(1).unwrap();
        let names: Vec<&str> = list.split('|').map(|s| s.split_whitespace().next().unwrap()).collect();
        assert!(names.len() >= 15, "{names:?}");
        for n in names {
            let g = menu_icon(&json!({"appearance": {"menuIcon": n}}));
            assert!(n == "tessera" || g != "\u{e604}", "{n} has no glyph of its own");
        }
    }

    #[test]
    fn the_bar_is_clear_unless_asked() {
        assert!(!bar(&json!({})).background);
        assert!(bar(&json!({"menubar": {"background": true}})).background);
    }

    #[test]
    fn clock_follows_the_settings() {
        assert_eq!(clock_formats(&json!({})).0, "%a %-d %b  %-I:%M %p");
        assert_eq!(clock_formats(&json!({"appearance": {"clock24": true}})).0, "%a %-d %b  %H:%M");
        assert_eq!(clock_formats(&json!({"menubar": {"showDate": false, "showSeconds": true}})).0, "%-I:%M:%S %p");
        assert_eq!(clock_formats(&json!({"appearance": {"clock24": true}, "menubar": {"showSeconds": true}})).1, "%A %-d %B %Y, %H:%M:%S");
    }

    /// A missing key reads as Config.qml's default, for every menubar.* key
    /// (the defaults are taken from Config.qml itself)
    #[test]
    fn defaults_are_config_qmls() {
        let src = config_qml();
        let start = src.find("property JsonObject menubar: JsonObject {").unwrap();
        let block = &src[start..start + src[start..].find("\n            }").unwrap()];
        let d = bar(&json!({}));
        let mut checked = 0;
        for l in block.lines().skip(1) {
            let l = l.trim();
            let Some(rest) = l.strip_prefix("property ") else { continue };
            let (ty, rest) = rest.split_once(' ').unwrap();
            let (name, rest) = rest.split_once(':').unwrap();
            let value = rest.split("//").next().unwrap().trim();
            let got: Value = match name {
                "background" => json!(d.background),
                "showSeconds" => json!(d.show_seconds),
                "showDate" => json!(d.show_date),
                "autoHide" => json!(d.auto_hide),
                "workspaces" => json!(d.workspaces),
                "workspaceCount" => json!(d.workspace_count),
                "workspaceOverflow" => json!(d.workspace_overflow),
                "otherSpaces" => json!(d.other_spaces),
                "windowTitle" => json!(d.window_title),
                "resources" => json!(d.resources),
                "weather" => json!(d.weather),
                "search" => json!(d.search),
                "ai" => json!(d.ai),
                "controlCenter" => json!(d.control_center),
                "clock" => json!(d.clock),
                "appMenu" => json!(d.app_menu),
                "windowMenu" => json!(d.window_menu),
                "battery" => json!(d.battery),
                "batteryPercent" => json!(d.battery_percent),
                "wifi" => json!(d.wifi),
                "bluetooth" => json!(d.bluetooth),
                "sound" => json!(d.sound),
                "keyboardLayout" => json!(d.keyboard_layout),
                "collapseTray" => json!(d.collapse_tray),
                "notificationIndicator" => json!(d.notification_indicator),
                "scrollGestures" => json!(d.scroll_gestures),
                "utilButtons" => json!(d.util_buttons),
                "screens" => continue, // (surfaces::on_screen)
                other => panic!("menubar.{other} isn't read by the Rust bar"),
            };
            let want: Value = match ty {
                "bool" | "int" | "list<string>" => serde_json::from_str(value).unwrap(),
                "string" => serde_json::from_str(value).unwrap(),
                t => panic!("{t}"),
            };
            assert_eq!(got, want, "menubar.{name}");
            checked += 1;
        }
        assert!(checked >= 26, "{checked}");
        // and the island's pieces
        let start = src.find("property list<string> livePieces:").unwrap();
        let line = &src[start..start + src[start..].find('\n').unwrap()];
        let want: Vec<String> = serde_json::from_str(line.split(':').nth(1).unwrap().split("//").next().unwrap().trim()).unwrap();
        assert_eq!(d.live_pieces, want);
        assert!(d.island && d.shows("music", "bars") && !d.shows("music", "nothing"));
    }

    #[test]
    fn old_battery_percent_values() {
        assert_eq!(bar(&json!({"menubar": {"batteryPercent": true}})).battery_percent, "outside");
        assert_eq!(bar(&json!({"menubar": {"batteryPercent": "false"}})).battery_percent, "off");
        assert_eq!(bar(&json!({"menubar": {"batteryPercent": "inside"}})).battery_percent, "inside");
    }
}

//! The menu bar's settings from config.json, read as the QML shell reads them:
//! the system menu's icon (appearance.menuIcon, MenuBar.qml's table) and the
//! clock (appearance.clock24, menubar.showDate, menubar.showSeconds:
//! TimeFormat.qml and MenuBar.qml's clock).

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

/// Glass: a strip behind the bar (menubar.background; off by default: the bar
/// is clear over the wallpaper)
pub fn bar_background(cfg: &Value) -> bool {
    cfg["menubar"]["background"].as_bool().unwrap_or(false)
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

    /// Every choice Settings offers has a glyph here (Config.qml lists them)
    #[test]
    fn every_choice_has_a_glyph() {
        let cfg = std::fs::read_to_string(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../shell/common/Config.qml")).unwrap();
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
        assert!(!bar_background(&json!({})));
        assert!(bar_background(&json!({"menubar": {"background": true}})));
    }

    #[test]
    fn clock_follows_the_settings() {
        assert_eq!(clock_formats(&json!({})).0, "%a %-d %b  %-I:%M %p");
        assert_eq!(clock_formats(&json!({"appearance": {"clock24": true}})).0, "%a %-d %b  %H:%M");
        assert_eq!(clock_formats(&json!({"menubar": {"showDate": false, "showSeconds": true}})).0, "%-I:%M:%S %p");
        assert_eq!(clock_formats(&json!({"appearance": {"clock24": true}, "menubar": {"showSeconds": true}})).1, "%A %-d %B %Y, %H:%M:%S");
    }
}

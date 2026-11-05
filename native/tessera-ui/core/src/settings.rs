//! Settings more than one surface reads, from config.json with Config.qml's
//! defaults (each surface's own are in its crate's settings.rs)

use serde_json::Value;

/// The island shows the HUD instead (Glass only; either menu bar's island:
/// Osd.qml's Island.hud reaches the Rust one over the bridge)
pub fn island_hud(cfg: &Value, material: bool) -> bool {
    let i = &cfg["island"];
    !material && i["enabled"].as_bool().unwrap_or(true) && i["hud"].as_bool().unwrap_or(true)
}

/// The emoji font for Settings' style (EmojiFont.qml), Noto when that one
/// isn't installed
pub fn emoji_font(cfg: &Value, installed: &[String]) -> String {
    let want = match cfg["emoji"]["style"].as_str().unwrap_or("android") {
        "apple" => "Apple Color Emoji",
        "windows" => "Fluent Emoji Color",
        _ => "Noto Color Emoji",
    };
    let has = |f: &str| installed.iter().any(|i| i.eq_ignore_ascii_case(f) || (f == "Fluent Emoji Color" && i.eq_ignore_ascii_case("FluentEmojiColor")));
    if installed.is_empty() || has(want) {
        want.into()
    } else {
        "Noto Color Emoji".into()
    }
}

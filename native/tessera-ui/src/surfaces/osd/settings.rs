//! The OSD's settings from config.json, with Config.qml's defaults: the
//! island's HUD (island.enabled, island.hud: on by default, so in Glass the
//! island shows volume and brightness, not the capsule), Reduce motion and
//! Glass's motion choice (its pace).

use serde_json::Value;

/// The island shows the HUD instead (Glass only; either menu bar's island:
/// Osd.qml's Island.hud reaches the Rust one over the bridge)
pub fn island_hud(cfg: &Value, material: bool) -> bool {
    let i = &cfg["island"];
    !material && i["enabled"].as_bool().unwrap_or(true) && i["hud"].as_bool().unwrap_or(true)
}

/// appearance.reduceMotion: the fullscreen bar fades without travel
pub fn reduce_motion(cfg: &Value) -> bool {
    cfg["appearance"]["reduceMotion"].as_bool().unwrap_or(false)
}

/// Theme.motion.pace: Glass's "snappy" motion is a little quicker
pub fn pace(cfg: &Value, material: bool) -> f32 {
    if !material && cfg["appearance"]["motion"].as_str() == Some("snappy") {
        0.85
    } else {
        1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_island_takes_it_by_default_in_glass() {
        assert!(island_hud(&json!({}), false));
        assert!(!island_hud(&json!({}), true));
        assert!(!island_hud(&json!({"island": {"hud": false}}), false));
        assert!(!island_hud(&json!({"island": {"enabled": false}}), false));
    }

    /// The defaults here are Config.qml's
    #[test]
    fn defaults_match_config_qml() {
        let cfg = std::fs::read_to_string(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../shell/common/Config.qml")).unwrap();
        let island = &cfg[cfg.find("property JsonObject island").unwrap()..];
        let island = &island[..island.find('}').unwrap()];
        assert!(island.contains("property bool enabled: true"), "island.enabled");
        assert!(island.contains("property bool hud: true"), "island.hud");
        assert!(cfg.contains("property bool reduceMotion: false"));
        assert!(cfg.contains("property string motion: \"subtle\""));
    }

    #[test]
    fn motion() {
        assert!(!reduce_motion(&json!({})));
        assert!(reduce_motion(&json!({"appearance": {"reduceMotion": true}})));
        assert_eq!(pace(&json!({"appearance": {"motion": "snappy"}}), false), 0.85);
        assert_eq!(pace(&json!({"appearance": {"motion": "snappy"}}), true), 1.0);
        assert_eq!(pace(&json!({}), false), 1.0);
    }
}

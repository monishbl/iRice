//! Who can see this computer: ONE switch for Android phones (Quick Share)
//! and LocalSend together (17 Nov, the user's choice): No one | Everyone for
//! 10 min | Everyone. The one setting is `sharing.quickShare` (what the
//! Control Center tile and the Quick Share helper already used); the old
//! LocalSend key `sharing.receiving` is only written as a copy of it, for
//! the QML shell kept as a crash fallback, and never read here.
//!
//! "Everyone for 10 min" counts down from when it was chosen and goes back
//! to No one. Its end is kept on disk ($XDG_STATE_HOME/tessera/
//! sharing-visible-until, ms since the epoch), so a shell restart neither
//! extends it nor loses it.

use serde_json::Value;
use std::path::{Path, PathBuf};

pub const KEY: &str = "sharing.quickShare";
/// the QML fallback's LocalSend key: a copy of KEY, never read
pub const OLD_KEY: &str = "sharing.receiving";
pub const LEVELS: [&str; 3] = ["off", "10min", "everyone"];
pub const TEN_MIN: i64 = 10 * 60_000;

/// A level as one of LEVELS (anything else is off)
pub fn norm(l: &str) -> &'static str {
    match l {
        "10min" => "10min",
        "everyone" => "everyone",
        _ => "off",
    }
}

/// The setting from config.json's `sharing` object. KEY is the truth; a
/// config that has only the old key (from before 17 Nov, set from
/// Settings' LocalSend row) gets its value.
pub fn level_of(sharing: &Value) -> &'static str {
    match sharing.get("quickShare").and_then(Value::as_str) {
        Some(q) => norm(q),
        None => norm(sharing.get("receiving").and_then(Value::as_str).unwrap_or("off")),
    }
}

/// What config.json must be told so both keys say the same (the migration,
/// run at start and after every change): (key, value) pairs to write
pub fn migration(sharing: &Value) -> Vec<(&'static str, &'static str)> {
    let l = level_of(sharing);
    let mut out = vec![];
    if sharing.get("quickShare").and_then(Value::as_str) != Some(l) {
        out.push((KEY, l));
    }
    if sharing.get("receiving").and_then(Value::as_str) != Some(l) {
        out.push((OLD_KEY, l));
    }
    out
}

/// Visible now?
pub fn visible(level: &str, until: i64, now: i64) -> bool {
    match level {
        "everyone" => true,
        "10min" => until > now,
        _ => false,
    }
}

/// Whole minutes left of "Everyone for 10 min" (rounded up; 0 when it's over)
pub fn minutes_left(until: i64, now: i64) -> i64 {
    if until <= now {
        0
    } else {
        (until - now + 59_999) / 60_000
    }
}

/// When "10 min" ends, given what was kept on disk: the kept end when it is
/// for this choice (no more than 10 minutes away), else a fresh 10 minutes
pub fn deadline(kept: Option<i64>, now: i64) -> i64 {
    match kept {
        Some(t) if t <= now + TEN_MIN => t,
        _ => now + TEN_MIN,
    }
}

/// The status words: "No one", "Everyone", "7 min left" (fits the tile)
pub fn status(level: &str, until: i64, now: i64) -> String {
    match level {
        "everyone" => "Everyone".into(),
        "10min" if until > now => format!("{} min left", minutes_left(until, now)),
        _ => "No one".into(),
    }
}

pub fn until_file() -> PathBuf {
    tessera_core::util::xdg("XDG_STATE_HOME", ".local/state").join("tessera/sharing-visible-until")
}

pub fn read_until(path: &Path) -> Option<i64> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

pub fn write_until(path: &Path, until: Option<i64>) {
    match until {
        Some(t) => {
            if let Some(d) = path.parent() {
                let _ = std::fs::create_dir_all(d);
            }
            let _ = std::fs::write(path, format!("{t}\n"));
        }
        None => {
            let _ = std::fs::remove_file(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_tiles_key_is_the_truth() {
        // the user's config on 17 Nov: Quick Share everyone, no LocalSend key
        let s = json!({"quickShare": "everyone", "quickShareHotspot": true, "quickShareFolder": ""});
        assert_eq!(level_of(&s), "everyone");
        assert_eq!(migration(&s), vec![(OLD_KEY, "everyone")]);
        // the two differed (set apart on the old page): quickShare wins
        let s = json!({"quickShare": "off", "receiving": "everyone"});
        assert_eq!(level_of(&s), "off");
        assert_eq!(migration(&s), vec![(OLD_KEY, "off")]);
        let s = json!({"quickShare": "10min", "receiving": "off"});
        assert_eq!(level_of(&s), "10min");
    }

    #[test]
    fn only_the_old_key_is_carried_over() {
        let s = json!({"receiving": "10min"});
        assert_eq!(level_of(&s), "10min");
        assert_eq!(migration(&s), vec![(KEY, "10min")]);
        assert_eq!(level_of(&json!({})), "off");
        assert_eq!(migration(&json!({})), vec![(KEY, "off"), (OLD_KEY, "off")]);
        assert_eq!(level_of(&Value::Null), "off");
    }

    #[test]
    fn agreeing_keys_need_nothing() {
        assert!(migration(&json!({"quickShare": "everyone", "receiving": "everyone"})).is_empty());
        // (a junk value reads as off and is written back as off)
        assert_eq!(level_of(&json!({"quickShare": "on"})), "off");
    }

    #[test]
    fn ten_minutes_count_down_then_end() {
        let t0 = 1_000_000;
        let until = deadline(None, t0);
        assert_eq!(until, t0 + TEN_MIN);
        assert!(visible("10min", until, t0));
        assert_eq!(status("10min", until, t0), "10 min left");
        assert_eq!(status("10min", until, t0 + 1), "10 min left");
        assert_eq!(status("10min", until, t0 + 60_000), "9 min left");
        assert_eq!(minutes_left(until, t0 + TEN_MIN - 1), 1);
        assert!(!visible("10min", until, t0 + TEN_MIN));
        assert_eq!(status("10min", until, t0 + TEN_MIN), "No one");
        assert!(visible("everyone", 0, t0));
        assert!(!visible("off", until, t0));
        assert_eq!(status("everyone", 0, t0), "Everyone");
    }

    #[test]
    fn a_restart_keeps_the_end_it_had() {
        let t0 = 5_000_000;
        // kept 3 minutes ahead: still that
        assert_eq!(deadline(Some(t0 + 180_000), t0), t0 + 180_000);
        // already over: kept (so it ends now, not 10 minutes later)
        assert_eq!(deadline(Some(t0 - 1), t0), t0 - 1);
        // nonsense far ahead: a fresh 10 minutes
        assert_eq!(deadline(Some(t0 + 10 * TEN_MIN), t0), t0 + TEN_MIN);
    }

    #[test]
    fn the_end_is_kept_on_disk() {
        let dir = std::env::temp_dir().join(format!("tessera-vis-{}", std::process::id()));
        let f = dir.join("tessera/sharing-visible-until");
        write_until(&f, Some(123456));
        assert_eq!(read_until(&f), Some(123456));
        write_until(&f, None);
        assert_eq!(read_until(&f), None);
        let _ = std::fs::remove_dir_all(dir);
    }
}

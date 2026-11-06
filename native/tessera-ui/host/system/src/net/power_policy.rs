//! Battery & Power decisions (replaces services/PowerPolicy.js), pure so the
//! logic is tested. Config values are read with the QML's `??` defaults.

use serde_json::Value;

/// A request for an energy mode: { owner, mode, priority, since (ms), until
/// (ms, 0 = until withdrawn), why }
#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    pub owner: String,
    pub mode: String,
    pub priority: i64,
    pub since: f64,
    pub until: f64,
    pub why: String,
}

/// The priorities (PRIORITY in the JS; others 10):
///   40 Cool-down, 30 Battery saver, 20 your choice and Game Mode
pub fn priority(owner: &str) -> i64 {
    match owner {
        "cooldown" => 40,
        "battery" => 30,
        "manual" | "game" => 20,
        _ => 10,
    }
}

/// The highest priority wins; among equals, the newest. An expired request
/// counts as gone. With no request at all: balanced.
pub fn resolve(requests: &[Request], now: f64) -> Request {
    let mut best: Option<&Request> = None;
    for r in requests {
        if r.mode.is_empty() || (r.until > 0.0 && r.until <= now) {
            continue;
        }
        if best.is_none_or(|b| r.priority > b.priority || (r.priority == b.priority && r.since > b.since)) {
            best = Some(r);
        }
    }
    best.cloned().unwrap_or(Request { owner: "default".into(), mode: "balanced".into(), priority: 0, since: 0.0, until: 0.0, why: String::new() })
}

/// The mode a request asks for, made to fit this computer (no Performance
/// profile: balanced instead)
pub fn fit(mode: &str, has_performance: bool) -> String {
    if mode == "performance" && !has_performance { "balanced".into() } else { mode.into() }
}

/// 0 nothing, 1 low (10%: a warning), 2 very low (5%: urgent, Sleep now),
/// 3 critical (3%: sleep, Caffeine or not). Only on battery.
pub fn battery_stage(percent: f64, on_battery: bool) -> u8 {
    if !on_battery || !(percent >= 0.0) {
        0
    } else if percent <= 3.0 {
        3
    } else if percent <= 5.0 {
        2
    } else if percent <= 10.0 {
        1
    } else {
        0
    }
}

fn s<'a>(cfg: &'a Value, k: &str, d: &'a str) -> &'a str {
    cfg[k].as_str().unwrap_or(d)
}

/// What closing the lid does: "sleep" | "lock" | "nothing". On the charger
/// with another display connected, its own setting (like a docked laptop).
/// With Caffeine on, a lid that would sleep does what lidCaffeine says.
pub fn lid_action(on_ac: bool, external_displays: usize, cfg: &Value, caffeine: bool) -> String {
    let act = if on_ac && external_displays > 0 { s(cfg, "lidExternal", "nothing") } else if on_ac { s(cfg, "lidAc", "sleep") } else { s(cfg, "lidBattery", "sleep") };
    if caffeine && act == "sleep" { s(cfg, "lidCaffeine", "lock").into() } else { act.into() }
}

/// Tessera only takes the lid over from logind (an inhibitor) when some
/// setting isn't logind's own default, or Caffeine keeps it awake
pub fn lid_needs_inhibit(cfg: &Value, caffeine: bool) -> bool {
    s(cfg, "lidAc", "sleep") != "sleep" || s(cfg, "lidBattery", "sleep") != "sleep" || s(cfg, "lidExternal", "nothing") != "nothing" || (caffeine && s(cfg, "lidCaffeine", "lock") != "sleep")
}

/// (source, action, timeout seconds): the hypridle listeners for both power
/// sources, one per distinct (timeout, action). Seconds; 0 = never. Each step
/// is clamped to come after the one before it on the same source.
pub fn idle_listeners(cfg: &Value) -> Vec<(String, String, i64)> {
    let mut out = vec![];
    for (src, suffix) in [("ac", "Ac"), ("battery", "Battery")] {
        let mut last = 0;
        for action in ["dim", "lock", "off", "sleep"] {
            let key = format!("idle{}{}{}", action[..1].to_uppercase(), &action[1..], suffix);
            let mut t = cfg[&key].as_f64().unwrap_or(0.0).round().max(0.0) as i64;
            if t == 0 {
                continue;
            }
            t = t.max(last);
            last = t;
            out.push((src.to_string(), action.to_string(), t.max(10)));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn req(owner: &str, mode: &str, since: f64, until: f64) -> Request {
        Request { owner: owner.into(), mode: mode.into(), priority: priority(owner), since, until, why: String::new() }
    }

    #[test]
    fn the_highest_priority_wins_then_the_newest() {
        let all = [req("manual", "balanced", 5.0, 0.0), req("game", "performance", 9.0, 0.0), req("battery", "power-saver", 1.0, 0.0)];
        assert_eq!(resolve(&all, 10.0).owner, "battery");
        let two = [req("manual", "balanced", 5.0, 0.0), req("game", "performance", 9.0, 0.0)];
        assert_eq!(resolve(&two, 10.0).owner, "game");
        assert_eq!(resolve(&[req("cooldown", "power-saver", 1.0, 5.0)], 10.0).owner, "default"); // expired
        assert_eq!(resolve(&[], 0.0).mode, "balanced");
        assert_eq!(fit("performance", false), "balanced");
        assert_eq!(fit("performance", true), "performance");
    }

    #[test]
    fn critical_stages_only_on_battery() {
        assert_eq!([100.0, 10.0, 5.0, 3.0, 2.0].map(|p| battery_stage(p, true)), [0, 1, 2, 3, 3]);
        assert_eq!(battery_stage(2.0, false), 0);
        assert_eq!(battery_stage(-1.0, true), 0);
    }

    #[test]
    fn the_lid() {
        let cfg = json!({});
        assert_eq!(lid_action(false, 0, &cfg, false), "sleep");
        assert_eq!(lid_action(true, 1, &cfg, false), "nothing");
        assert_eq!(lid_action(true, 0, &cfg, true), "lock");
        assert!(!lid_needs_inhibit(&cfg, false) && lid_needs_inhibit(&cfg, true));
        assert!(lid_needs_inhibit(&json!({"lidAc": "lock"}), false));
    }

    #[test]
    fn idle_steps_come_in_order() {
        let cfg = json!({"idleDimAc": 300, "idleLockAc": 100, "idleOffAc": 0, "idleSleepAc": 0, "idleDimBattery": 5, "idleSleepBattery": 900});
        let l = idle_listeners(&cfg);
        assert_eq!(l[0], ("ac".into(), "dim".into(), 300));
        assert_eq!(l[1], ("ac".into(), "lock".into(), 300)); // clamped after dim
        assert_eq!(l[2], ("battery".into(), "dim".into(), 10)); // at least 10 s
        assert_eq!(l[3], ("battery".into(), "sleep".into(), 900));
    }
}

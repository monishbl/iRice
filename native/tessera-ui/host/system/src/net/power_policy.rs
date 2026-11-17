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
///   40 Cool-down, 30 Battery saver (Low Power at a low battery, or the
///   shell's battery saver with "Also use Low Power"), 20 your choice and
///   Game Mode
pub fn priority(owner: &str) -> i64 {
    match owner {
        "cooldown" => 40,
        "battery" | "saver" => 30,
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

// ── The battery's alerts: one per mark, one owner (Power) ─────────────────

/// The marks a draining battery passes, each announced once: the battery
/// saver's level (20% unless changed), low (10%), very low (5%: Sleep now)
/// and empty (3%: it sleeps). Power is the only one that announces them (the
/// island used to post its own 20/10/5% alerts on top: two cards at 10%); the
/// empty one comes from the battery guard while it runs (it does the sleep,
/// and has to say so even with the shell gone).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum BatteryMark {
    None,
    Saver,
    Low,
    VeryLow,
    Empty,
}

impl BatteryMark {
    /// The stage on the bus (power.stage): 0 none or the saver mark, 1 low, 2 very low, 3 empty
    pub fn stage(self) -> u8 {
        match self {
            Self::None | Self::Saver => 0,
            Self::Low => 1,
            Self::VeryLow => 2,
            Self::Empty => 3,
        }
    }
}

/// Where the battery is (`critical`: Settings → Critical battery, which
/// sleeps at 3%: without it there's no very low or empty mark, as nothing
/// sleeps). A saver level at or under 10% is the low mark.
pub fn battery_mark(percent: f64, on_battery: bool, saver_level: f64, critical: bool) -> BatteryMark {
    match battery_stage(percent, on_battery) {
        0 if on_battery && percent >= 0.0 && percent <= saver_level => BatteryMark::Saver,
        0 => BatteryMark::None,
        1 => BatteryMark::Low,
        _ if !critical => BatteryMark::Low,
        2 => BatteryMark::VeryLow,
        _ => BatteryMark::Empty,
    }
}

/// Once per mark per discharge: a mark is announced when the battery first
/// reaches it, never again on the way down (a reading that wobbles between
/// 10 and 11% doesn't repeat it), and everything is forgotten when it's
/// plugged in or back above the saver level plus the band.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BatteryAlerts {
    shown: Option<BatteryMark>,
}

impl BatteryAlerts {
    /// A reading: Some(mark) when that mark is to be announced now
    pub fn step(&mut self, mark: BatteryMark, percent: f64, on_battery: bool, saver_level: f64) -> Option<BatteryMark> {
        if !on_battery || (mark == BatteryMark::None && percent > saver_level + SAVER_BAND) {
            self.shown = None;
            return None;
        }
        if mark == BatteryMark::None || self.shown.is_some_and(|s| s >= mark) {
            return None;
        }
        self.shown = Some(mark);
        Some(mark)
    }

    pub fn shown(&self) -> BatteryMark {
        self.shown.unwrap_or(BatteryMark::None)
    }
}

/// What a mark says, the same everywhere (island card or notification): a
/// short title, "<n>% left · what happens", the icon and the action
/// (label, call). `saver`: the shell's battery saver came on by itself at
/// this mark (its notice is the saver mark's alert, with Turn off).
pub struct BatteryAlert {
    pub icon: &'static str,
    pub title: &'static str,
    pub subtitle: String,
    pub action: Option<(&'static str, &'static str)>,
    /// red (low, very low, empty) or the saver's yellow
    pub urgent: bool,
    /// its live entry's severity (core notify/live.rs): each mark more
    /// urgent than the last, so each pops up and the rest change quietly
    pub severity: u8,
}

pub fn battery_alert(mark: BatteryMark, percent: f64, saver: bool) -> Option<BatteryAlert> {
    let p = percent.max(0.0).round();
    let severity = match mark {
        BatteryMark::None | BatteryMark::Saver => 0,
        BatteryMark::Low => 1,
        BatteryMark::VeryLow => 2,
        BatteryMark::Empty => 3,
    };
    let a = |icon, title, what: &str, action, urgent| Some(BatteryAlert { icon, title, subtitle: format!("{p}% left · {what}"), action, urgent, severity });
    match mark {
        BatteryMark::None => None,
        BatteryMark::Saver if saver => a("leaf", "Battery saver is on", "lighter until you plug in", Some(("Turn off", "power.saverOff")), false),
        BatteryMark::Saver | BatteryMark::Low => a("battery-warning", "Low battery", "plug in soon", None, true),
        BatteryMark::VeryLow => a("battery-warning", "Battery very low", "sleeps at 3% to keep your work", Some(("Sleep now", "system.power")), true),
        BatteryMark::Empty => a("battery-empty", "Battery empty", "sleeping now to keep your work", None, true),
    }
}

fn s<'a>(cfg: &'a Value, k: &str, d: &'a str) -> &'a str {
    cfg[k].as_str().unwrap_or(d)
}

/// What closing the lid does: "sleep" | "lock" | "nothing". On the charger
/// with another display connected, its own setting (like a docked laptop).
/// With Caffeine on and on the charger, a lid that would sleep does what
/// lidCaffeine says. On battery a closed lid always sleeps, whatever Caffeine
/// or the setting say: a laptop awake in a bag overheats and runs flat (16 Nov:
/// 7 hours, 99% to 8%, hot).
pub fn lid_action(on_ac: bool, external_displays: usize, cfg: &Value, caffeine: bool) -> String {
    if !on_ac {
        return "sleep".into();
    }
    let act = if external_displays > 0 { s(cfg, "lidExternal", "nothing") } else { s(cfg, "lidAc", "sleep") };
    if caffeine && act == "sleep" { s(cfg, "lidCaffeine", "lock").into() } else { act.into() }
}

/// A closed lid that's still awake: sleep anyway? On battery always (after a
/// minute: a resume with the lid shut, an unplug after closing, a missed lid
/// event); on the charger when it's running hot (over 90 °C for a minute)
/// unless it's docked to another display.
pub fn lid_should_sleep_now(on_ac: bool, external_displays: usize, hot_for_s: f64) -> bool {
    !on_ac || (hot_for_s >= 60.0 && external_displays == 0)
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

// ── Battery saver (the shell's: saver.rs in core) ──────────────────────────

/// power.saver: when the shell's battery saver comes on by itself
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaverMode {
    /// at or below the level, on battery (the default)
    Auto,
    /// whenever it runs on battery
    OnBattery,
    /// never by itself (the quick toggle still turns it on)
    Off,
}

impl SaverMode {
    pub fn from_cfg(power: &Value) -> Self {
        match power["saver"].as_str() {
            Some("battery") => Self::OnBattery,
            Some("off") => Self::Off,
            _ => Self::Auto,
        }
    }
}

/// power.saverLevel, held to 10–50% (20 when unset or not a number)
pub fn saver_level(power: &Value) -> f64 {
    power["saverLevel"].as_f64().filter(|v| v.is_finite()).unwrap_or(20.0).clamp(10.0, 50.0)
}

/// Auto ends only this far above the level (a battery that hovers around it,
/// or climbs a point on a lighter load, doesn't switch it on and off)
pub const SAVER_BAND: f64 = 5.0;

/// The shell's battery saver: on while running on battery and the setting
/// says so (Auto at or below the level until 5 points above it, or always on
/// battery), or turned on or off by hand; plugging in ends it, and forgets
/// what was chosen by hand.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Saver {
    pub on: bool,
    /// Auto's latch: reached the level, not yet back above it plus the band
    low: bool,
    /// turned on or off by hand (the quick toggle, the alert's Turn off),
    /// until it's plugged in (and a Turn off also until the battery is back
    /// above the band, so a second run-down turns it on again)
    pub forced: Option<bool>,
}

impl Saver {
    /// A battery reading (`percent` below 0: not known). Some(on) when it
    /// came on or went off.
    pub fn step(&mut self, mode: SaverMode, level: f64, percent: f64, on_battery: bool) -> Option<bool> {
        if !on_battery {
            self.low = false;
            self.forced = None;
        } else if percent >= 0.0 {
            if percent <= level {
                self.low = true;
            } else if percent > level + SAVER_BAND {
                self.low = false;
                if self.forced == Some(false) {
                    self.forced = None;
                }
            }
        }
        let automatic = match mode {
            SaverMode::Auto => self.low,
            SaverMode::OnBattery => true,
            SaverMode::Off => false,
        };
        let on = on_battery && self.forced.unwrap_or(automatic);
        (on != self.on).then(|| {
            self.on = on;
            on
        })
    }

    /// It came on by itself now (not by hand): worth a notice
    pub fn came_on_by_itself(&self, mode: SaverMode) -> bool {
        self.on && self.forced.is_none() && mode == SaverMode::Auto
    }

    /// The quick toggle (on battery only: plugged in, it's always off).
    /// False when there was nothing to switch.
    pub fn toggle(&mut self, on_battery: bool) -> bool {
        if !on_battery {
            return false;
        }
        self.forced = Some(!self.on);
        true
    }

    /// The alert's Turn off: off until it's plugged in (or back above the band)
    pub fn turn_off(&mut self) {
        self.forced = Some(false);
    }
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
    fn saver_settings() {
        assert_eq!(SaverMode::from_cfg(&json!({})), SaverMode::Auto);
        assert_eq!(SaverMode::from_cfg(&json!({"saver": "battery"})), SaverMode::OnBattery);
        assert_eq!(SaverMode::from_cfg(&json!({"saver": "off"})), SaverMode::Off);
        assert_eq!(SaverMode::from_cfg(&json!({"saver": 3})), SaverMode::Auto);
        assert_eq!(saver_level(&json!({})), 20.0);
        assert_eq!(saver_level(&json!({"saverLevel": 5})), 10.0);
        assert_eq!(saver_level(&json!({"saverLevel": 80})), 50.0);
        assert_eq!(saver_level(&json!({"saverLevel": "x"})), 20.0);
        assert_eq!(saver_level(&json!({"saverLevel": 35})), 35.0);
    }

    #[test]
    fn saver_auto_threshold_and_band() {
        let mut s = Saver::default();
        let a = SaverMode::Auto;
        assert_eq!(s.step(a, 20.0, 40.0, true), None);
        assert_eq!(s.step(a, 20.0, 21.0, true), None);
        assert_eq!(s.step(a, 20.0, 20.0, true), Some(true)); // at the level
        assert!(s.came_on_by_itself(a));
        // (charging a little on a lighter load: stays on within the band)
        assert_eq!(s.step(a, 20.0, 23.0, true), None);
        assert_eq!(s.step(a, 20.0, 25.0, true), None);
        assert!(s.on);
        assert_eq!(s.step(a, 20.0, 25.5, true), Some(false)); // above level + 5
        assert_eq!(s.step(a, 20.0, 22.0, true), None); // (and not back on until the level)
        // an unknown reading changes nothing
        assert_eq!(s.step(a, 20.0, 19.0, true), Some(true));
        assert_eq!(s.step(a, 20.0, -1.0, true), None);
        assert!(s.on);
    }

    #[test]
    fn saver_ends_when_plugged_in() {
        let mut s = Saver::default();
        let a = SaverMode::Auto;
        assert_eq!(s.step(a, 20.0, 12.0, true), Some(true));
        assert_eq!(s.step(a, 20.0, 12.0, false), Some(false)); // plugged in
        assert_eq!(s.step(a, 20.0, 13.0, false), None);
        // unplugged again, still low: on again
        assert_eq!(s.step(a, 20.0, 13.0, true), Some(true));
        // on the charger, nothing turns it on: not the mode, not the toggle
        let mut s = Saver::default();
        assert_eq!(s.step(SaverMode::OnBattery, 20.0, 90.0, false), None);
        assert!(!s.toggle(false));
        assert_eq!(s.step(SaverMode::OnBattery, 20.0, 90.0, false), None);
        assert!(!s.on);
    }

    #[test]
    fn saver_always_on_battery_and_off() {
        let mut s = Saver::default();
        assert_eq!(s.step(SaverMode::OnBattery, 20.0, 95.0, true), Some(true));
        assert!(!s.came_on_by_itself(SaverMode::OnBattery)); // (no notice: you chose it)
        assert_eq!(s.step(SaverMode::OnBattery, 20.0, 95.0, false), Some(false));
        let mut s = Saver::default();
        assert_eq!(s.step(SaverMode::Off, 20.0, 5.0, true), None);
        // the toggle still turns it on, until plugged in
        assert!(s.toggle(true));
        assert_eq!(s.step(SaverMode::Off, 20.0, 5.0, true), Some(true));
        assert!(!s.came_on_by_itself(SaverMode::Off));
        assert_eq!(s.step(SaverMode::Off, 20.0, 5.0, false), Some(false));
        assert_eq!(s.forced, None);
        // changing the setting applies at the next reading
        let mut s = Saver::default();
        assert_eq!(s.step(SaverMode::Off, 30.0, 25.0, true), None);
        assert_eq!(s.step(SaverMode::Auto, 30.0, 25.0, true), Some(true));
        assert_eq!(s.step(SaverMode::Off, 30.0, 25.0, true), Some(false));
    }

    #[test]
    fn saver_turned_off_by_hand() {
        let mut s = Saver::default();
        let a = SaverMode::Auto;
        assert_eq!(s.step(a, 20.0, 18.0, true), Some(true));
        s.turn_off();
        assert_eq!(s.step(a, 20.0, 17.0, true), Some(false));
        assert_eq!(s.step(a, 20.0, 10.0, true), None); // stays off while it runs down
        // the toggle turns it on again
        assert!(s.toggle(true));
        assert_eq!(s.step(a, 20.0, 10.0, true), Some(true));
        assert!(!s.came_on_by_itself(a));
        assert!(s.toggle(true));
        assert_eq!(s.step(a, 20.0, 10.0, true), Some(false));
        // back above the band, then down again: on by itself again
        assert_eq!(s.step(a, 20.0, 26.0, true), None);
        assert_eq!(s.forced, None);
        assert_eq!(s.step(a, 20.0, 20.0, true), Some(true));
        assert!(s.came_on_by_itself(a));
        // turned off, then plugged in: forgotten
        s.turn_off();
        s.step(a, 20.0, 19.0, true);
        s.step(a, 20.0, 19.0, false);
        assert_eq!(s.step(a, 20.0, 19.0, true), Some(true));
    }

    #[test]
    fn critical_stages_only_on_battery() {
        assert_eq!([100.0, 10.0, 5.0, 3.0, 2.0].map(|p| battery_stage(p, true)), [0, 1, 2, 3, 3]);
        assert_eq!(battery_stage(2.0, false), 0);
        assert_eq!(battery_stage(-1.0, true), 0);
    }

    #[test]
    fn battery_marks() {
        use BatteryMark::*;
        let m = |p| battery_mark(p, true, 20.0, true);
        assert_eq!([50.0, 21.0, 20.0, 11.0, 10.0, 6.0, 5.0, 4.0, 3.0, 1.0].map(m), [None, None, Saver, Saver, Low, Low, VeryLow, VeryLow, Empty, Empty]);
        assert_eq!(battery_mark(10.0, false, 20.0, true), None); // plugged in
        assert_eq!(battery_mark(-1.0, true, 20.0, true), None); // not known
        // Critical battery off: nothing sleeps, so low is the last mark
        assert_eq!(battery_mark(2.0, true, 20.0, false), Low);
        // a saver level at 10%: the low mark
        assert_eq!(battery_mark(10.0, true, 10.0, true), Low);
        assert_eq!(battery_mark(30.0, true, 35.0, true), Saver);
    }

    /// The bug: at 10% the island showed its own "Low battery" and Power's
    /// "Battery low" together. Now a whole run-down gives one alert per
    /// mark, whatever the readings do.
    #[test]
    fn one_alert_per_mark_per_discharge() {
        use BatteryMark::*;
        let mut a = BatteryAlerts::default();
        let mut run = |readings: &[(f64, bool)]| -> Vec<BatteryMark> {
            readings.iter().filter_map(|&(p, b)| a.step(battery_mark(p, b, 20.0, true), p, b, 20.0)).collect()
        };
        // a run-down with readings that wobble and skip
        let down = [(40.0, true), (21.0, true), (20.0, true), (21.0, true), (20.0, true), (19.0, true), (11.0, true), (10.0, true), (11.0, true), (10.0, true), (9.0, true), (5.0, true), (6.0, true), (5.0, true), (3.0, true), (2.0, true), (1.0, true)];
        assert_eq!(run(&down), vec![Saver, Low, VeryLow, Empty]);
        // a jump straight to 4% (asleep through the rest): only the mark it's at
        assert_eq!(run(&[(30.0, false), (4.0, true), (3.5, true)]), vec![VeryLow]);
        // plugged in, then unplugged at 15%: the saver mark again
        assert_eq!(run(&[(15.0, false), (15.0, true)]), vec![Saver]);
        // back above the level, but not past the band: nothing new at the level
        assert_eq!(run(&[(22.0, true), (20.0, true)]), Vec::<BatteryMark>::new());
        // past the band (charged on a light load): the level counts again
        assert_eq!(run(&[(26.0, true), (20.0, true)]), vec![Saver]);
    }

    #[test]
    fn battery_alert_words() {
        let t = |m, p, s| battery_alert(m, p, s).map(|a| (a.title, a.subtitle));
        assert_eq!(t(BatteryMark::Saver, 20.0, true), Some(("Battery saver is on", "20% left · lighter until you plug in".into())));
        assert_eq!(t(BatteryMark::Saver, 20.0, false), Some(("Low battery", "20% left · plug in soon".into())));
        assert_eq!(t(BatteryMark::Low, 9.6, false), Some(("Low battery", "10% left · plug in soon".into())));
        assert_eq!(t(BatteryMark::VeryLow, 5.0, false), Some(("Battery very low", "5% left · sleeps at 3% to keep your work".into())));
        assert_eq!(t(BatteryMark::Empty, 1.0, false), Some(("Battery empty", "1% left · sleeping now to keep your work".into())));
        assert!(t(BatteryMark::None, 50.0, false).is_none());
        assert_eq!(battery_alert(BatteryMark::VeryLow, 5.0, false).unwrap().action, Some(("Sleep now", "system.power")));
        // each mark more urgent than the one before (so each pops up once)
        let sev: Vec<u8> = [BatteryMark::Saver, BatteryMark::Low, BatteryMark::VeryLow, BatteryMark::Empty].iter().map(|&m| battery_alert(m, 7.0, false).unwrap().severity).collect();
        assert!(sev.windows(2).all(|w| w[0] < w[1]));
        // every alert says it the same way: "<n>% left · …"
        for m in [BatteryMark::Saver, BatteryMark::Low, BatteryMark::VeryLow, BatteryMark::Empty] {
            assert!(battery_alert(m, 7.0, true).unwrap().subtitle.starts_with("7% left · "));
        }
    }

    #[test]
    fn the_lid() {
        let cfg = json!({});
        assert_eq!(lid_action(false, 0, &cfg, false), "sleep");
        assert_eq!(lid_action(true, 1, &cfg, false), "nothing");
        assert_eq!(lid_action(true, 0, &cfg, true), "lock");
        // on battery a closed lid sleeps, Caffeine or "lock" or not
        assert_eq!(lid_action(false, 0, &cfg, true), "sleep");
        assert_eq!(lid_action(false, 0, &json!({"lidBattery": "lock"}), false), "sleep");
        assert!(lid_should_sleep_now(false, 0, 0.0));
        assert!(!lid_should_sleep_now(true, 0, 10.0));
        assert!(lid_should_sleep_now(true, 0, 61.0));
        assert!(!lid_should_sleep_now(true, 1, 600.0));
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

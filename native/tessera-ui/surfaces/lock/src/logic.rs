//! The lock's rules as plain functions (Lock.qml, LockSurface.qml,
//! StandBy.qml, LockGlass.qml, LockMaterial.qml): too many attempts
//! (pam_faillock), what the fingerprint reader said, Bedside Mode's moves
//! against burn-in and its night, the clock's and the date's text, the
//! notification shelf, the morph's ramps. The part (mod.rs) keeps the state.

/// pam_faillock's settings (faillock.conf), its defaults when unread
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Faillock {
    pub deny: u32,
    /// seconds; 0: never unlocks by itself ("never")
    pub unlock: i64,
    pub interval: i64,
}

impl Default for Faillock {
    fn default() -> Self {
        Self { deny: 3, unlock: 600, interval: 900 }
    }
}

/// faillock.conf's `deny`, `unlock_time` and `fail_interval` (the last
/// setting of each wins, comments skipped)
pub fn faillock_conf(text: &str) -> Faillock {
    let mut f = Faillock::default();
    let get = |key: &str| -> Option<String> {
        let mut found = None;
        for l in text.lines() {
            let l = l.trim();
            if l.starts_with('#') {
                continue;
            }
            let Some(rest) = l.strip_prefix(key) else { continue };
            let Some(v) = rest.trim_start().strip_prefix('=') else { continue };
            let v: String = v.trim().chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
            found = Some(v);
        }
        found
    };
    if let Some(d) = get("deny").and_then(|v| v.parse::<u32>().ok()).filter(|d| *d > 0) {
        f.deny = d;
    }
    match get("unlock_time").as_deref() {
        Some("never") => f.unlock = 0,
        Some(v) => {
            if let Ok(n) = v.parse::<i64>() {
                f.unlock = n;
            }
        }
        None => {}
    }
    if let Some(i) = get("fail_interval").and_then(|v| v.parse::<i64>().ok()).filter(|i| *i > 0) {
        f.interval = i;
    }
    f
}

/// What the error says while the account is locked by too many attempts
pub fn locked_text(seconds: i64) -> String {
    if seconds <= 0 {
        return "Too many attempts: this account stays locked until it's unlocked with faillock --reset from another session.".into();
    }
    let m = ((seconds as f64) / 60.0).ceil().max(1.0) as i64;
    format!("Too many attempts: try again in about {m} {}.", if m == 1 { "minute" } else { "minutes" })
}

/// Seconds until pam_faillock lets the account in again (None: not locked).
/// `tally`: `faillock --user` (None where it can't be read: then `fails` in
/// a row, from this lock, decide); `local`: a tally line's local time in ms
pub fn lockout(conf: &Faillock, tally: Option<&str>, fails: u32, now_ms: i64, local: impl Fn(i32, i32, i32, i32, i32, i32) -> i64) -> Option<i64> {
    let readable = tally.filter(|t| t.lines().any(|l| l.starts_with("When")));
    let Some(text) = readable else {
        return (fails >= conf.deny).then_some(conf.unlock);
    };
    // "2026-10-30 13:27:01 TTY /dev/pts/1 V"
    let mut times = Vec::new();
    for l in text.lines() {
        let l = l.trim_end();
        if !l.ends_with(" V") && !l.ends_with("\tV") {
            continue;
        }
        let b = l.as_bytes();
        if b.len() < 19 || b[4] != b'-' || b[7] != b'-' || b[10] != b' ' || b[13] != b':' || b[16] != b':' {
            continue;
        }
        let n = |a: usize, z: usize| l[a..z].parse::<i32>().ok();
        if let (Some(y), Some(mo), Some(d), Some(h), Some(mi), Some(s)) = (n(0, 4), n(5, 7), n(8, 10), n(11, 13), n(14, 16), n(17, 19)) {
            times.push(local(y, mo, d, h, mi, s));
        }
    }
    let recent: Vec<i64> = times.into_iter().filter(|t| now_ms - t <= conf.interval * 1000).collect();
    if (recent.len() as u32) < conf.deny {
        return None;
    }
    if conf.unlock == 0 {
        return Some(0);
    }
    let last = *recent.iter().max()?;
    let left = ((last + conf.unlock * 1000 - now_ms) as f64 / 1000.0).round() as i64;
    (left > 0).then_some(left)
}

/// What a fingerprint conversation's message means (Lock.qml's tests)
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Finger {
    /// not this finger (or a failed read)
    Miss,
    /// the reader asks for a finger
    Ask,
    Other,
}

pub fn finger_message(m: &str) -> Finger {
    let l = m.to_lowercase();
    if ["fail", "no match", "not match", "not recogni"].iter().any(|w| l.contains(w)) {
        Finger::Miss
    } else if ["place", "swipe", "touch", "finger"].iter().any(|w| l.contains(w)) {
        Finger::Ask
    } else {
        Finger::Other
    }
}

/// A new spot for Bedside Mode's content, up or down (-1 .. 1 of the room it
/// has), well away from the last one so no pixel holds a colour for long
pub fn next_drift(prev: f64, mut rnd: impl FnMut() -> f64) -> f64 {
    let mut y = 0.0;
    for _ in 0..8 {
        y = rnd() * 2.0 - 1.0;
        if (y - prev).abs() > 0.5 {
            break;
        }
    }
    y
}

/// How far content `cw`×`ch` can move on a `w`×`h` screen and stay 48 px in
pub fn room(w: f64, h: f64, cw: f64, ch: f64) -> (f64, f64) {
    (((w - cw) / 2.0 - 48.0).max(0.0), ((h - ch) / 2.0 - 48.0).max(0.0))
}

/// Bedside Mode's night (a dim red): after dark, between 22:00 and 6:00
pub fn night(day: bool, hour: u32) -> bool {
    !day && (hour >= 22 || hour < 6)
}

/// A tiny random number from the clock (0 .. 1): Bedside Mode's spots
pub fn random() -> f64 {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
    (h.finish() >> 11) as f64 / (1u64 << 53) as f64
}

/// The hour as the menu bar writes it (TimeFormat.hour): 24-hour "09", or
/// 12-hour "9"
pub fn hour(h: u32, h24: bool) -> String {
    if h24 {
        format!("{h:02}")
    } else {
        format!("{}", (h + 11) % 12 + 1)
    }
}

/// "9:41" (large clocks: no AM/PM)
pub fn clock(h: u32, m: u32, h24: bool) -> String {
    format!("{}:{m:02}", hour(h, h24))
}

/// TimeFormat.hm: "21:41", or "9:41 PM"
pub fn hm(h: u32, m: u32, h24: bool) -> String {
    if h24 {
        format!("{h:02}:{m:02}")
    } else {
        format!("{}:{m:02} {}", (h + 11) % 12 + 1, if h < 12 { "AM" } else { "PM" })
    }
}

const DAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

/// Qt's "dddd d MMMM": "Thursday 5 November" (`wday` 0 = Sunday)
pub fn date_long(wday: u32, day: u32, month0: u32) -> String {
    format!("{} {day} {}", DAYS[(wday % 7) as usize], MONTHS[(month0 % 12) as usize])
}

/// Qt's "d MMM": "5 Nov" (the dynamic clock with 24-hour time)
pub fn date_short(day: u32, month0: u32) -> String {
    format!("{day} {}", &MONTHS[(month0 % 12) as usize][..3])
}

pub fn month_name(month0: u32) -> &'static str {
    MONTHS[(month0 % 12) as usize]
}

/// Notifications.timeLabel: "now", "5m ago", else the time it came
pub fn time_label(received_ms: i64, now_ms: i64, received_hm: &str) -> String {
    if received_ms <= 0 {
        return String::new();
    }
    let mins = (now_ms - received_ms).div_euclid(60_000);
    if mins < 1 {
        "now".into()
    } else if mins < 60 {
        format!("{mins}m ago")
    } else {
        received_hm.to_string()
    }
}

/// The apps that sent them, newest first, each once, at most six (`keys`:
/// app name, else desktop entry, else id; newest first)
pub fn shelf_apps(keys: &[String]) -> Vec<usize> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for (i, k) in keys.iter().enumerate() {
        if out.len() >= 6 {
            break;
        }
        if seen.insert(k.as_str()) {
            out.push(i);
        }
    }
    out
}

/// Strips markup from a notification's body (`replace(/<[^>]*>/g, "")`)
pub fn plain(body: &str) -> String {
    let mut out = String::with_capacity(body.len());
    let mut tag = false;
    for c in body.chars() {
        match c {
            '<' => tag = true,
            '>' if tag => tag = false,
            _ if !tag => out.push(c),
            _ => {}
        }
    }
    out
}

/// LockSurface's ramp: `sb` mapped to 0..1 over [a, b], smoothstepped
pub fn ramp(sb: f64, a: f64, b: f64) -> f64 {
    let t = ((sb - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The Material lock clock's digit size (LockMaterial.clockSize)
pub fn material_clock_size(w: f64, h: f64, split: bool, ios: bool) -> f64 {
    (if split { (h * 0.2).min(w * 0.12) } else { (h * if ios { 0.17 } else { 0.15 }).min(w * 0.2) }).round()
}

/// Bedside Mode's clock weights (StandBy: thin, regular, bold), Glass's and
/// Material's (the font's weight axis)
pub fn standby_weight(setting: &str, material: bool) -> i32 {
    match (setting, material) {
        ("bold", false) => 600,
        ("regular", false) => 400,
        (_, false) => 200,
        ("bold", true) => 600,
        ("regular", true) => 400,
        (_, true) => 200,
    }
}

/// The month for Bedside Mode's calendar: (day, column 0 = Monday, row) for
/// each day of a month starting on `first_wday` (0 = Sunday) with `days` days
pub fn month_grid(first_wday: u32, days: u32) -> Vec<(u32, u32, u32)> {
    let lead = (first_wday + 6) % 7; // Monday first
    (1..=days).map(|d| {
        let i = lead + d - 1;
        (d, i % 7, i / 7)
    }).collect()
}

/// Days in a month (month0 0 = January)
pub fn days_in_month(year: i32, month0: u32) -> u32 {
    match month0 {
        1 => {
            if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 {
                29
            } else {
                28
            }
        }
        3 | 5 | 8 | 10 => 30,
        _ => 31,
    }
}

/// Whether to treat the face camera's or the reader's tries as given back
/// (Lock.qml's woke): someone came back after 30 s away or from Bedside Mode
pub fn came_back(idle: bool, now_ms: i64, last_touch_ms: i64) -> bool {
    idle || now_ms - last_touch_ms > 30_000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faillock_settings() {
        assert_eq!(faillock_conf(""), Faillock::default());
        let f = faillock_conf("# deny = 9\ndeny = 5\n  unlock_time=120 # two minutes\nfail_interval = 60\n");
        assert_eq!(f, Faillock { deny: 5, unlock: 120, interval: 60 });
        assert_eq!(faillock_conf("unlock_time = never").unlock, 0);
        // (nonsense keeps the defaults; the last one wins)
        assert_eq!(faillock_conf("deny = 0\ndeny = x").deny, 3);
        assert_eq!(faillock_conf("deny = 4\ndeny = 6").deny, 6);
    }

    #[test]
    fn locked_texts() {
        assert_eq!(locked_text(600), "Too many attempts: try again in about 10 minutes.");
        assert_eq!(locked_text(30), "Too many attempts: try again in about 1 minute.");
        assert!(locked_text(0).contains("faillock --reset"));
    }

    fn local(y: i32, mo: i32, d: i32, h: i32, mi: i32, s: i32) -> i64 {
        // (a fake clock: minutes of the day only)
        let _ = (y, mo, d);
        ((h * 3600 + mi * 60 + s) as i64) * 1000
    }

    #[test]
    fn lockouts() {
        let c = Faillock::default();
        // unreadable: three in a row here
        assert_eq!(lockout(&c, None, 2, 0, local), None);
        assert_eq!(lockout(&c, Some("NOREAD"), 3, 0, local), Some(600));
        let tally = "user:\nWhen                Type  Source                                           Valid\n2026-10-30 13:27:01 TTY /dev/pts/1 V\n2026-10-30 13:27:05 TTY /dev/pts/1 V\n2026-10-30 13:27:09 TTY /dev/pts/1 V\n";
        let now = local(2026, 10, 30, 13, 28, 9);
        assert_eq!(lockout(&c, Some(tally), 0, now, local), Some(540));
        // too long ago: not counted
        assert_eq!(lockout(&c, Some(tally), 0, local(2026, 10, 30, 13, 50, 0), local), None);
        // invalid entries (I) don't count
        let invalid = tally.replace(" V\n", " I\n");
        assert_eq!(lockout(&c, Some(&invalid), 9, now, local), None);
        // never unlocks by itself
        let never = Faillock { unlock: 0, ..c };
        assert_eq!(lockout(&never, Some(tally), 0, now, local), Some(0));
    }

    #[test]
    fn finger_messages() {
        assert_eq!(finger_message("Place your finger on the reader"), Finger::Ask);
        assert_eq!(finger_message("Swipe your right index finger"), Finger::Ask);
        assert_eq!(finger_message("Failed to match fingerprint"), Finger::Miss);
        assert_eq!(finger_message("No match"), Finger::Miss);
        assert_eq!(finger_message("Fingerprint not recognised"), Finger::Miss);
        assert_eq!(finger_message("Verification timed out"), Finger::Other);
    }

    #[test]
    fn drifts_move_away() {
        let mut seq = [0.6, 0.62, 0.1].into_iter();
        // 0.6 → 0.2 and 0.62 → 0.24 are too near 0; 0.1 → -0.8 is far enough
        let y = next_drift(0.0, || seq.next().unwrap_or(0.5));
        assert!((y + 0.8).abs() < 1e-9);
        for _ in 0..50 {
            let y = next_drift(0.3, random);
            assert!((-1.0..=1.0).contains(&y));
        }
        assert_eq!(room(1920.0, 1080.0, 600.0, 300.0), (612.0, 342.0));
        assert_eq!(room(500.0, 300.0, 600.0, 300.0), (0.0, 0.0));
    }

    #[test]
    fn night_is_late_and_dark() {
        assert!(night(false, 23) && night(false, 3));
        assert!(!night(true, 23) && !night(false, 21) && !night(false, 6));
    }

    #[test]
    fn clocks_and_dates() {
        assert_eq!(clock(21, 5, false), "9:05");
        assert_eq!(clock(0, 0, false), "12:00");
        assert_eq!(clock(9, 41, true), "09:41");
        assert_eq!(hm(13, 7, false), "1:07 PM");
        assert_eq!(hm(0, 7, false), "12:07 AM");
        assert_eq!(hm(13, 7, true), "13:07");
        assert_eq!(date_long(4, 5, 10), "Thursday 5 November");
        assert_eq!(date_short(5, 10), "5 Nov");
        assert_eq!(time_label(1_000, 30_000, "9:00 AM"), "now");
        assert_eq!(time_label(1_000, 1_000 + 5 * 60_000, "9:00 AM"), "5m ago");
        assert_eq!(time_label(1_000, 1_000 + 61 * 60_000, "9:00 AM"), "9:00 AM");
        assert_eq!(time_label(0, 5, "x"), "");
    }

    #[test]
    fn shelf_has_each_app_once() {
        let keys: Vec<String> = ["Mail", "Chat", "Mail", "A", "B", "C", "D", "E"].iter().map(|s| s.to_string()).collect();
        assert_eq!(shelf_apps(&keys), vec![0, 1, 3, 4, 5, 6]);
        assert!(shelf_apps(&[]).is_empty());
        assert_eq!(plain("<b>Hi</b> <i>there</i>"), "Hi there");
        assert_eq!(plain("a < b"), "a ");
    }

    #[test]
    fn ramps_and_sizes() {
        assert_eq!(ramp(0.0, 0.3, 0.8), 0.0);
        assert_eq!(ramp(1.0, 0.3, 0.8), 1.0);
        assert!((ramp(0.55, 0.3, 0.8) - 0.5).abs() < 1e-9);
        assert_eq!(material_clock_size(1920.0, 1080.0, false, true), 184.0);
        assert_eq!(material_clock_size(1920.0, 1080.0, true, true), 216.0);
        assert_eq!(standby_weight("thin", false), 200);
        assert_eq!(standby_weight("bold", true), 600);
    }

    #[test]
    fn months() {
        // November 2026 starts on a Sunday: the 1st in the last column
        let g = month_grid(0, 30);
        assert_eq!(g[0], (1, 6, 0));
        assert_eq!(g[1], (2, 0, 1));
        assert_eq!(g.last(), Some(&(30, 0, 5)));
        assert_eq!(days_in_month(2028, 1), 29);
        assert_eq!(days_in_month(2026, 1), 28);
        assert_eq!(days_in_month(2026, 10), 30);
        assert!(came_back(true, 0, 0));
        assert!(came_back(false, 40_000, 0));
        assert!(!came_back(false, 20_000, 0));
    }
}

//! The widgets' words and shapes from their data, as the QML widgets write
//! them: dates and times, the calendar's month, a world clock's "Tomorrow,
//! +9h", a stock's price and change, the battery's 24 hours as paths, Screen
//! Time's durations. Pure functions with tests.

use std::collections::HashSet;

// ── Dates ───────────────────────────────────────────────────────────────────

/// Days since 1970-01-01 of a civil date (proleptic Gregorian)
pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    era * 146097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719468
}

pub fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        _ => 28,
    }
}

/// Monday 0 .. Sunday 6
pub fn weekday_mon(y: i64, m: i64, d: i64) -> i64 {
    // 1970-01-01 was a Thursday (3 from Monday)
    (days_from_civil(y, m, d) + 3).rem_euclid(7)
}

/// A local date: (year, month 1-12, day), hour, minute, of ms since the epoch
pub fn local(ms: i64) -> ((i64, i64, i64), i64, i64) {
    unsafe {
        let t = (ms.div_euclid(1000)) as libc::time_t;
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        ((tm.tm_year as i64 + 1900, tm.tm_mon as i64 + 1, tm.tm_mday as i64), tm.tm_hour as i64, tm.tm_min as i64)
    }
}

/// Local midnight of a date, in ms
pub fn midnight_ms(y: i64, m: i64, d: i64) -> i64 {
    unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        tm.tm_year = (y - 1900) as i32;
        tm.tm_mon = (m - 1) as i32;
        tm.tm_mday = d as i32;
        tm.tm_isdst = -1;
        libc::mktime(&mut tm) as i64 * 1000
    }
}

/// strftime on ms
pub fn fmt(pattern: &str, ms: i64) -> String {
    crate::util::strftime(pattern, ms.div_euclid(1000))
}

/// TimeFormat.hm: "9:41 AM" or "09:41"
pub fn hm(ms: i64, h24: bool) -> String {
    fmt(if h24 { "%H:%M" } else { "%-I:%M %p" }, ms)
}

/// TimeFormat.hmShort (big clocks): "9:41" or "09:41"
pub fn hm_short(ms: i64, h24: bool) -> String {
    fmt(if h24 { "%H:%M" } else { "%-I:%M" }, ms)
}

pub fn same_day(a: (i64, i64, i64), b: (i64, i64, i64)) -> bool {
    a == b
}

/// Calendar.when: an event's time on the small and medium calendar
/// ("Tomorrow 3:00 PM", "Fri all day"; medium: just the time)
pub fn when(start: i64, all_day: bool, today: (i64, i64, i64), small: bool, h24: bool) -> String {
    let (d, _, _) = local(start);
    let diff = days_from_civil(d.0, d.1, d.2) - days_from_civil(today.0, today.1, today.2);
    let prefix = if diff == 0 || !small {
        String::new()
    } else if diff == 1 {
        "Tomorrow ".into()
    } else {
        format!("{} ", fmt("%a", start))
    };
    if all_day {
        if prefix.is_empty() { "All day".into() } else { format!("{prefix}all day") }
    } else {
        format!("{prefix}{}", hm(start, h24))
    }
}

/// One day of the medium calendar's month: its grid column (Monday first)
/// and row (from the first week)
#[derive(Clone, Debug, PartialEq)]
pub struct MonthDay {
    pub day: i64,
    pub col: i64,
    pub row: i64,
    pub today: bool,
    pub selected: bool,
    pub busy: bool,
}

/// The month of `today`, its busy days ("y-m-d", month from 0) dotted
pub fn month(today: (i64, i64, i64), selected: (i64, i64, i64), busy: &HashSet<String>) -> (Vec<MonthDay>, i64) {
    let (y, m, _) = today;
    let lead = weekday_mon(y, m, 1);
    let days = days_in_month(y, m);
    let weeks = (lead + days + 6) / 7;
    let out = (1..=days)
        .map(|d| {
            let cell = d - 1 + lead;
            MonthDay { day: d, col: cell % 7, row: cell / 7, today: d == today.2, selected: (y, m, d) == selected, busy: busy.contains(&format!("{}-{}-{}", y, m - 1, d)) }
        })
        .collect();
    (out, weeks)
}

// ── World clock ─────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub struct ZoneTime {
    pub h: i64,
    pub m: i64,
    /// hours from here
    pub offset: f64,
    /// -1 yesterday, 0 today, 1 tomorrow
    pub day: i64,
}

/// "Name|Area/Zone" (or just the zone) -> (name, zone)
pub fn city(entry: &str) -> (String, String) {
    match entry.split_once('|') {
        Some((n, z)) if !z.is_empty() => (n.to_string(), z.to_string()),
        Some((n, _)) => (n.to_string(), n.to_string()),
        None => (entry.to_string(), entry.to_string()),
    }
}

/// What `date '+%H %M %z %j'` printed: "here 09 41 +0530 309" then one line
/// per zone "Asia/Tokyo 13 11 +0900 309"
pub fn zone_times(out: &str) -> Vec<(String, ZoneTime)> {
    let mins = |z: &str| -> i64 {
        let sign = if z.starts_with('-') { -1 } else { 1 };
        let d: String = z.chars().filter(|c| c.is_ascii_digit()).collect();
        if d.len() < 4 {
            return 0;
        }
        sign * (d[0..2].parse::<i64>().unwrap_or(0) * 60 + d[2..4].parse::<i64>().unwrap_or(0))
    };
    let lines: Vec<Vec<&str>> = out.trim().lines().map(|l| l.split(' ').collect()).collect();
    let Some(here) = lines.first().filter(|l| l.len() >= 5) else { return Vec::new() };
    lines[1..]
        .iter()
        .filter(|l| l.len() >= 5)
        .map(|l| {
            let d = l[4].parse::<i64>().unwrap_or(0) - here[4].parse::<i64>().unwrap_or(0);
            // (day of the year wraps at New Year)
            let day = if d > 1 { -1 } else if d < -1 { 1 } else { d.signum() };
            (l[0].to_string(), ZoneTime { h: l[1].parse().unwrap_or(0), m: l[2].parse().unwrap_or(0), offset: (mins(l[3]) - mins(here[3])) as f64 / 60.0, day })
        })
        .collect()
}

/// "Tomorrow, +9h", "Today, -5.5h", "Today"
pub fn describe(t: &ZoneTime) -> String {
    let day = if t.day > 0 { "Tomorrow" } else if t.day < 0 { "Yesterday" } else { "Today" };
    if t.offset == 0.0 {
        return day.into();
    }
    let n = if t.offset.fract() == 0.0 { format!("{}", t.offset as i64) } else { format!("{:.1}", t.offset) };
    format!("{day}, {}{n}h", if t.offset > 0.0 { "+" } else { "" })
}

// ── Stocks ──────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
pub struct Quote {
    pub s: String,
    #[serde(default)]
    pub n: String,
    pub p: f64,
    #[serde(default)]
    pub pc: f64,
    #[serde(default)]
    pub c: Vec<f64>,
}

pub fn quote(line: &str) -> Option<Quote> {
    serde_json::from_str(line.trim()).ok()
}

/// The change since yesterday's close, 0..1
pub fn change(q: &Quote) -> f64 {
    if q.pc != 0.0 { (q.p - q.pc) / q.pc } else { 0.0 }
}

/// A price as the locale writes it: grouped thousands, 3 decimals under 10
pub fn price(p: f64) -> String {
    let s = format!("{:.*}", if p < 10.0 { 3 } else { 2 }, p.abs());
    let (int, frac) = s.split_once('.').unwrap_or((&s, ""));
    let mut g = String::new();
    for (i, ch) in int.chars().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 {
            g.push(',');
        }
        g.push(ch);
    }
    format!("{}{g}.{frac}", if p < 0.0 { "-" } else { "" })
}

pub fn change_text(c: f64) -> String {
    format!("{}{:.2}%", if c >= 0.0 { "+" } else { "" }, c * 100.0)
}

/// The day so far as a line in a `w` × `h` box (yesterday's close counts
/// toward the range)
pub fn spark(q: &Quote, w: f32, h: f32) -> String {
    if q.c.len() < 2 {
        return String::new();
    }
    let lo = q.c.iter().cloned().fold(q.pc, f64::min);
    let hi = q.c.iter().cloned().fold(q.pc, f64::max);
    let n = q.c.len();
    q.c.iter()
        .enumerate()
        .map(|(i, v)| {
            let x = i as f32 / (n - 1).max(1) as f32 * w;
            let y = h - ((v - lo) / (hi - lo).max(1e-9)) as f32 * h;
            format!("{}{x:.2} {y:.2} ", if i == 0 { "M " } else { "L " })
        })
        .collect()
}

// ── Battery history ─────────────────────────────────────────────────────────

/// battery-history.sh's lines: "<unix time> <percent> <c|d>"
pub fn battery_samples(out: &str) -> Vec<(i64, f64, bool)> {
    out.lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split_whitespace().collect();
            Some((f.first()?.parse().ok()?, f.get(1)?.parse().ok()?, f.get(2) == Some(&"c")))
        })
        .collect()
}

/// One filled run per charging / discharging stretch, in a `w` × `h` box
/// over the last day: (area, line, charging)
pub fn battery_runs(s: &[(i64, f64, bool)], now: i64, w: f32, h: f32) -> Vec<(String, String, bool)> {
    let mut out = Vec::new();
    if s.len() < 2 {
        return out;
    }
    let from = now - 86400;
    let x = |t: i64| (t - from) as f32 / 86400.0 * w;
    let y = |p: f64| h - p as f32 / 100.0 * h;
    let mut i = 0;
    while i < s.len() - 1 {
        let mut j = i;
        while j < s.len() - 1 && s[j + 1].2 == s[i].2 {
            j += 1;
        }
        let end = (j + 1).min(s.len() - 1);
        let mut area = format!("M {:.2} {h:.2} ", x(s[i].0));
        let mut line = String::new();
        for (k, p) in s.iter().enumerate().take(end + 1).skip(i) {
            area += &format!("L {:.2} {:.2} ", x(p.0), y(p.1));
            line += &format!("{} {:.2} {:.2} ", if k == i { "M" } else { "L" }, x(p.0), y(p.1));
        }
        area += &format!("L {:.2} {h:.2} Z", x(s[end].0));
        out.push((area, line, s[i].2));
        i = end;
    }
    out
}

/// ScreenTime.format: "45m", "2h 5m"
pub fn duration(secs: i64) -> String {
    let m = (secs as f64 / 60.0).round() as i64;
    if m >= 60 { format!("{}h {}m", m / 60, m % 60) } else { format!("{m}m") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendar_maths() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(2026, 2), 28);
        assert_eq!(days_in_month(2026, 11), 30);
        // 5 November 2026 is a Thursday; 1 November 2026 a Sunday
        assert_eq!(weekday_mon(2026, 11, 5), 3);
        assert_eq!(weekday_mon(2026, 11, 1), 6);
        let busy: HashSet<String> = ["2026-10-12".to_string()].into_iter().collect();
        let (days, weeks) = month((2026, 11, 5), (2026, 11, 12), &busy);
        assert_eq!((days.len(), weeks), (30, 6));
        assert_eq!((days[0].col, days[0].row), (6, 0));
        assert_eq!((days[1].col, days[1].row), (0, 1));
        assert!(days[4].today && !days[4].selected);
        assert!(days[11].selected && days[11].busy);
    }

    #[test]
    fn times_follow_the_24_hour_setting() {
        let ms = midnight_ms(2026, 11, 5) + (9 * 60 + 41) * 60_000;
        assert_eq!(hm(ms, true), "09:41");
        assert_eq!(hm(ms, false), "9:41 AM");
        assert_eq!(hm_short(ms, false), "9:41");
        assert_eq!(local(ms), ((2026, 11, 5), 9, 41));
        let today = (2026, 11, 5);
        assert_eq!(when(ms, false, today, true, true), "09:41");
        assert_eq!(when(ms + 86_400_000, false, today, true, true), "Tomorrow 09:41");
        assert_eq!(when(ms + 86_400_000, true, today, true, true), "Tomorrow all day");
        assert_eq!(when(ms + 86_400_000, true, today, false, true), "All day");
        assert_eq!(when(ms + 2 * 86_400_000, false, today, true, true), "Sat 09:41");
    }

    #[test]
    fn world_clocks() {
        assert_eq!(city("London|Europe/London"), ("London".into(), "Europe/London".into()));
        assert_eq!(city("UTC"), ("UTC".into(), "UTC".into()));
        let t = zone_times("here 09 41 +0530 309\nAsia/Tokyo 13 11 +0900 309\nAmerica/Los_Angeles 20 11 -0800 308\nPacific/Kiritimati 18 11 +1400 001\n");
        assert_eq!(t[0], ("Asia/Tokyo".into(), ZoneTime { h: 13, m: 11, offset: 3.5, day: 0 }));
        assert_eq!(describe(&t[0].1), "Today, +3.5h");
        assert_eq!(describe(&t[1].1), "Yesterday, -13.5h");
        // (New Year: day 1 against 309 is tomorrow... of a short year; d < -1 means ahead)
        assert_eq!(t[2].1.day, 1);
        assert_eq!(describe(&ZoneTime { h: 0, m: 0, offset: 0.0, day: 0 }), "Today");
        assert_eq!(describe(&ZoneTime { h: 0, m: 0, offset: 9.0, day: 1 }), "Tomorrow, +9h");
        assert!(zone_times("").is_empty());
    }

    #[test]
    fn stocks() {
        let q = quote(r#"{"s":"AAPL","n":"Apple Inc.","p":1234.5,"pc":1200,"cur":"USD","c":[1200,1250,1234.5]}"#).unwrap();
        assert_eq!(price(q.p), "1,234.50");
        assert_eq!(price(9.5), "9.500");
        assert_eq!(price(123456.0), "123,456.00");
        assert_eq!(change_text(change(&q)), "+2.88%");
        assert_eq!(change_text(-0.0123), "-1.23%");
        let s = spark(&q, 100.0, 20.0);
        assert!(s.starts_with("M 0.00 20.00") && s.contains("L 50.00 0.00"), "{s}");
        assert!(quote("not json").is_none());
    }

    #[test]
    fn battery_history() {
        let s = battery_samples("100 50 d\n200 40 d\n300 60 c\nbad line\n400 80 c\n");
        assert_eq!(s.len(), 4);
        let runs = battery_runs(&s, 86_400 + 0, 864.0, 100.0);
        // a discharging run (to where charging began), then the charging one
        assert_eq!(runs.len(), 2);
        assert!(!runs[0].2 && runs[1].2);
        assert!(runs[0].1.starts_with("M 1.00 50.00 L 2.00 60.00 L 3.00 40.00"), "{}", runs[0].1);
        assert!(battery_runs(&s[..1], 0, 1.0, 1.0).is_empty());
        assert_eq!(duration(2700), "45m");
        assert_eq!(duration(7500), "2h 5m");
    }
}

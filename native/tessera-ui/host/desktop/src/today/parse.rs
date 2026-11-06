//! The quick-text readers, ported without a regex engine: Calendar.parseQuick
//! ("call mum at 5pm", "stretch in 20 minutes"), Todos.parse ("pay rent
//! tomorrow 5pm"), Alarms.parseDuration/parseClock ("1h 30m", "7:30 pm") and
//! Alarms.describe. All pure, with `now` given.

use tessera_core::services::calendar as cal;

const DAYS: [&str; 7] = ["sun", "mon", "tue", "wed", "thu", "fri", "sat"];

fn cap(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

fn is_ws(c: char) -> bool {
    c.is_whitespace()
}

/// The text before a trailing word and the word, when whitespace separates them
fn last_word(s: &str) -> Option<(&str, &str)> {
    let s = s.trim_end();
    let i = s.rfind(is_ws)?;
    Some((&s[..i], s[i..].trim_start()))
}

fn strip_prefix_ci<'a>(s: &'a str, p: &str) -> Option<&'a str> {
    (s.len() >= p.len() && s.is_char_boundary(p.len()) && s[..p.len()].eq_ignore_ascii_case(p)).then(|| &s[p.len()..])
}

/// "remind me (to )?" and "add (an? )?(event|reminder) " at the start
fn strip_lead(t: &str) -> String {
    let mut t = t.trim().to_string();
    if let Some(r) = strip_prefix_ci(&t, "remind me ") {
        let r = r.to_string();
        t = strip_prefix_ci(&r, "to ").map_or(r.clone(), str::to_string);
    } else if let Some(r) = strip_prefix_ci(&t, "add ") {
        let r = r.trim_start_matches(' ');
        let r2 = strip_prefix_ci(r, "an ").or_else(|| strip_prefix_ci(r, "a ")).unwrap_or(r);
        for w in ["event ", "reminder "] {
            if let Some(x) = strip_prefix_ci(r2, w) {
                t = x.to_string();
                break;
            }
        }
    }
    t
}

/// A trailing clock: "5", "5pm", "5:30 pm", "17.45" -> (index of its first
/// digit, hour, minute, Some(pm) when am/pm given)
pub fn time_tail(l: &str) -> Option<(usize, u32, u32, Option<bool>)> {
    let mut s = l.trim_end();
    let mut ap = None;
    if s.len() >= 2 && (s.ends_with("am") || s.ends_with("pm")) {
        ap = Some(s.ends_with("pm"));
        s = s[..s.len() - 2].trim_end();
    }
    let b = s.as_bytes();
    let digits_end = |mut end: usize, max: usize| -> Option<(usize, u32)> {
        let mut n = 0;
        while end > 0 && b[end - 1].is_ascii_digit() && n < max {
            end -= 1;
            n += 1;
        }
        (n > 0).then(|| (end, s[end..end + n].parse().unwrap_or(0)))
    };
    let (mut start, last) = digits_end(b.len(), 2)?;
    let (h, m);
    if start >= 1 && (b[start - 1] == b':' || b[start - 1] == b'.') && b.len() - start == 2 {
        let (hs, hv) = digits_end(start - 1, 2)?;
        start = hs;
        h = hv;
        m = last;
    } else {
        h = last;
        m = 0;
    }
    // (digits run into a longer number: not a clock)
    if start > 0 && b[start - 1].is_ascii_digit() {
        return None;
    }
    Some((start, h, m, ap))
}

/// Calendar.parseQuick: (title, start ms)
pub fn parse_quick(text: &str, now: i64) -> Option<(String, i64)> {
    let t = strip_lead(text);
    let l = t.to_ascii_lowercase();
    let n = cal::local(now);
    let start;
    let mut title_end;
    // "… in 20 min|hours"
    let ltrim = l.trim_end();
    let unit_start = ltrim.rfind(|c: char| !c.is_ascii_alphabetic()).map_or(0, |i| i + 1);
    let unit = &ltrim[unit_start..];
    let in_match = ["min", "mins", "minute", "minutes", "h", "hr", "hrs", "hour", "hours"].contains(&unit).then_some(()).and_then(|_| {
        let before = ltrim[..unit_start].trim_end();
        let ds = before.rfind(|c: char| !c.is_ascii_digit()).map_or(0, |i| i + 1);
        let digits = &before[ds..];
        let pre = &before[..ds];
        let pre_t = pre.trim_end();
        if digits.is_empty() || pre_t.len() == pre.len() {
            return None;
        }
        let (rest, w) = last_word(pre_t)?;
        (w == "in").then_some(())?;
        Some((rest.len(), digits.parse::<i64>().ok()?))
    });
    if let Some((cut, k)) = in_match {
        let mult = if unit.starts_with('h') { 3_600_000 } else { 60_000 };
        start = now + k * mult;
        title_end = cut;
    } else {
        let (ts, mut h, mi, ap) = time_tail(&l)?;
        // "at" before the digits, after whitespace
        let before = l[..ts].trim_end();
        if before.len() == ts {
            return None; // (needs whitespace between "at" and the clock)
        }
        let (rest, w) = last_word(before)?;
        if w != "at" {
            return None;
        }
        // an optional day word before "at"
        let mut day = String::new();
        let mut dow: Option<usize> = None;
        let mut cut = rest.len();
        if let Some((r2, w2)) = last_word(rest) {
            if ["today", "tonight", "tomorrow"].contains(&w2) {
                day = w2.to_string();
                cut = r2.len();
            } else if w2.len() >= 3 && w2.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                if let Some(i) = DAYS.iter().position(|d| w2.starts_with(d)) {
                    if let Some((r3, w3)) = last_word(r2) {
                        if w3 == "on" {
                            dow = Some(i);
                            cut = r3.len();
                        }
                    }
                }
            }
        }
        if let Some(pm) = ap {
            h = (h % 12) + if pm { 12 } else { 0 };
        } else if day == "tonight" && h < 12 {
            h += 12;
        }
        if h > 23 || mi > 59 {
            return None;
        }
        let mut s = cal::make(n.y, n.mo, n.d, h as i32, mi as i32, 0);
        if day == "tomorrow" {
            s = cal::make(n.y, n.mo, n.d + 1, h as i32, mi as i32, 0);
        } else if let Some(want) = dow {
            let mut diff = (want as i32 - n.wday + 7) % 7;
            if diff == 0 && s <= now {
                diff = 7;
            }
            s = cal::make(n.y, n.mo, n.d + diff, h as i32, mi as i32, 0);
        } else if day.is_empty() && s <= now {
            s = cal::make(n.y, n.mo, n.d + 1, h as i32, mi as i32, 0);
        }
        start = s;
        title_end = cut;
    }
    title_end = title_end.min(t.len());
    let title = t.get(..title_end)?.trim();
    if title.is_empty() {
        return None;
    }
    Some((cap(title), start))
}

/// A parsed to-do: text, due ms (0 none), all day
#[derive(Debug, PartialEq)]
pub struct Parsed {
    pub text: String,
    pub due: i64,
    pub all_day: bool,
}

/// Todos.parse
pub fn parse_todo(text: &str, now: i64) -> Parsed {
    let mut t = text.trim().to_string();
    let l = t.to_ascii_lowercase();
    // "tomorrow 5pm" -> "tomorrow at 5pm" (the calendar's parser wants "at")
    if let Some((ts, _, _, ap)) = time_tail(&l) {
        let before = l[..ts].trim_end();
        let has_at = before.len() < ts && last_word(before).is_some_and(|(_, w)| w == "at");
        if ap.is_some() && !has_at && before.len() < ts && !before.is_empty() {
            t = format!("{} at {}", &t[..before.len()], &t[ts..]);
        }
    }
    if let Some((title, start)) = parse_quick(&t, now) {
        return Parsed { text: title, due: start, all_day: false };
    }
    // A day alone: due by the end of it
    let tl = t.to_ascii_lowercase();
    if let Some((rest0, w)) = last_word(&tl) {
        if w.chars().all(|c| c.is_ascii_alphabetic()) {
            let names = ["today", "tonight", "tomorrow", "mon", "tue", "wed", "thu", "fri", "sat", "sun"];
            if let Some(name) = names.iter().find(|n| w.starts_with(**n)) {
                let mut rest = rest0;
                if let Some((r, p)) = last_word(rest) {
                    if p == "by" || p == "on" {
                        rest = r;
                    }
                }
                let rest = t[..rest.len().min(t.len())].trim();
                if !rest.is_empty() {
                    let n = cal::local(now);
                    let mut day = n.d;
                    if *name == "tomorrow" {
                        day += 1;
                    } else if !["today", "tonight"].contains(name) {
                        let want = DAYS.iter().position(|d| d == name).unwrap_or(0) as i32;
                        let diff = (want - n.wday + 7) % 7;
                        day += if diff == 0 { 7 } else { diff };
                    }
                    return Parsed { text: cap(rest), due: cal::make(n.y, n.mo, day, 23, 59, 0), all_day: true };
                }
            }
        }
    }
    Parsed { text: text.trim().to_string(), due: 0, all_day: false }
}

/// Alarms.parseDuration: "10 min", "1h 30m", "90 seconds" -> seconds (0 none);
/// a bare number is minutes
pub fn parse_duration(text: &str) -> i64 {
    let b: Vec<char> = text.to_lowercase().chars().collect();
    let (mut total, mut found, mut i) = (0.0f64, false, 0usize);
    while i < b.len() {
        if b[i].is_ascii_digit() {
            let mut j = i;
            while j < b.len() && (b[j].is_ascii_digit() || (b[j] == '.' && j + 1 < b.len() && b[j + 1].is_ascii_digit())) {
                j += 1;
            }
            let n: f64 = b[i..j].iter().collect::<String>().parse().unwrap_or(0.0);
            let mut k = j;
            while k < b.len() && b[k].is_whitespace() {
                k += 1;
            }
            let us = k;
            while k < b.len() && b[k].is_ascii_alphabetic() {
                k += 1;
            }
            let u: String = b[us..k].iter().collect();
            let mult = match u.as_str() {
                "h" | "hr" | "hrs" | "hour" | "hours" => Some(3600.0),
                "m" | "min" | "mins" | "minute" | "minutes" => Some(60.0),
                "s" | "sec" | "secs" | "second" | "seconds" => Some(1.0),
                _ => None,
            };
            if let Some(m) = mult {
                total += n * m;
                found = true;
                i = k;
                continue;
            }
            i = j;
            continue;
        }
        i += 1;
    }
    if !found {
        let t = text.trim();
        if !t.is_empty() && t.chars().all(|c| c.is_ascii_digit()) {
            total = t.parse::<f64>().unwrap_or(0.0) * 60.0;
        }
    }
    total.round() as i64
}

/// Alarms.parseClock: "7", "7am", "7:30 pm", "19:45" -> the next such time (ms)
pub fn parse_clock(text: &str, now: i64) -> Option<i64> {
    let l = text.trim().to_ascii_lowercase();
    let (ts, mut h, mi, ap) = time_tail(&l)?;
    if ts != 0 || h > 23 || mi > 59 || (ap.is_some() && h > 12) {
        return None;
    }
    match ap {
        Some(true) if h < 12 => h += 12,
        Some(false) if h == 12 => h = 0,
        _ => {}
    }
    let n = cal::local(now);
    let mut d = cal::make(n.y, n.mo, n.d, h as i32, mi as i32, 0);
    if d <= now {
        d = cal::make(n.y, n.mo, n.d + 1, h as i32, mi as i32, 0);
    }
    Some(d)
}

/// Alarms.describe: "1 h 30 min"
pub fn describe(seconds: i64) -> String {
    let (h, m, s) = (seconds / 3600, seconds % 3600 / 60, seconds % 60);
    let parts: Vec<String> = [
        (h > 0).then(|| format!("{h} h")),
        (m > 0).then(|| format!("{m} min")),
        (s > 0 && h == 0).then(|| format!("{s} s")),
    ]
    .into_iter()
    .flatten()
    .collect();
    if parts.is_empty() { "0 s".into() } else { parts.join(" ") }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(y: i32, mo: i32, d: i32, h: i32, mi: i32) -> i64 {
        cal::make(y, mo, d, h, mi, 0)
    }

    #[test]
    fn durations_and_clocks() {
        assert_eq!(parse_duration("10 min"), 600);
        assert_eq!(parse_duration("1h 30m"), 5400);
        assert_eq!(parse_duration("90 seconds"), 90);
        assert_eq!(parse_duration("5"), 300);
        assert_eq!(parse_duration("tea"), 0);
        let now = at(2026, 10, 6, 10, 0);
        assert_eq!(parse_clock("7pm", now), Some(at(2026, 10, 6, 19, 0)));
        assert_eq!(parse_clock("7am", now), Some(at(2026, 10, 7, 7, 0)));
        assert_eq!(parse_clock("19:45", now), Some(at(2026, 10, 6, 19, 45)));
        assert_eq!(parse_clock("25", now), None);
        assert_eq!(describe(5400), "1 h 30 min");
        assert_eq!(describe(45), "45 s");
    }

    #[test]
    fn quick_text() {
        let now = at(2026, 10, 6, 10, 0);
        let (t, s) = parse_quick("remind me to call mum at 5pm", now).unwrap();
        assert_eq!((t.as_str(), s), ("Call mum", at(2026, 10, 6, 17, 0)));
        let (t, s) = parse_quick("stretch in 20 minutes", now).unwrap();
        assert_eq!((t.as_str(), s), ("Stretch", now + 20 * 60_000));
        let (t, s) = parse_quick("dentist tomorrow at 9:30", now).unwrap();
        assert_eq!((t.as_str(), s), ("Dentist", at(2026, 10, 7, 9, 30)));
        assert!(parse_quick("just words", now).is_none());
        // "at 9" after 9 means tomorrow
        let (_, s) = parse_quick("run at 9", now).unwrap();
        assert_eq!(s, at(2026, 10, 7, 9, 0));
    }

    #[test]
    fn todo_text() {
        let now = at(2026, 10, 6, 10, 0);
        let p = parse_todo("pay rent tomorrow 5pm", now);
        assert_eq!((p.text.as_str(), p.due, p.all_day), ("Pay rent", at(2026, 10, 7, 17, 0), false));
        let p = parse_todo("submit report tomorrow", now);
        assert_eq!((p.text.as_str(), p.due, p.all_day), ("Submit report", at(2026, 10, 7, 23, 59), true));
        let p = parse_todo("water plants", now);
        assert_eq!((p.text.as_str(), p.due), ("water plants", 0));
    }
}

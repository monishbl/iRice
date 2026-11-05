//! Calendar events, as shell/services/Calendar.qml has them: your own events
//! (Reminders: $XDG_STATE_HOME/tessera/events.json) and the iCalendar
//! subscriptions in $XDG_CONFIG_HOME/tessera/calendars.json whose link is a
//! file on this computer ("/…" or "file://…"), parsed here with the same
//! rules (TZID read as local time; DAILY/WEEKLY/MONTHLY/YEARLY with INTERVAL,
//! COUNT, UNTIL, BYDAY, BYMONTHDAY, BYMONTH, BYSETPOS, WKST; EXDATE and moved
//! instances; a rule with other parts shows its first date only).
//!
//! Not here, on purpose: subscriptions over the network and the accounts'
//! calendars (Google, iCloud, Microsoft 365). They need the accounts' sign-in,
//! which only the QML shell holds; a surface that shows them gets them from
//! the shell (Today: RustUi.tell's `accounts`).
//!
//! The files are watched (their times, every 2 s); a change sends a new
//! `Calendar` (Msg::Data). A surface asks `Calendar::between(from, to)`.

use crate::Msg;
use smithay_client_toolkit::reexports::calloop::channel::Sender;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

const DAY: i64 = 86_400_000;

// ── Local time (JavaScript's Date in the local zone) ─────────────────────

/// A moment's local date and time
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tm {
    pub y: i32,
    /// 0-11, as JavaScript's getMonth
    pub mo: i32,
    pub d: i32,
    pub h: i32,
    pub mi: i32,
    pub s: i32,
    /// 0 Sunday … 6 Saturday
    pub wday: i32,
}

pub fn local(ms: i64) -> Tm {
    unsafe {
        let t = ms.div_euclid(1000) as libc::time_t;
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        Tm { y: tm.tm_year + 1900, mo: tm.tm_mon, d: tm.tm_mday, h: tm.tm_hour, mi: tm.tm_min, s: tm.tm_sec, wday: tm.tm_wday }
    }
}

/// `new Date(y, mo, d, h, mi, s)`: out-of-range parts roll over (day 0 is the
/// month before's last, month 12 next January), in ms
pub fn make(y: i32, mo: i32, d: i32, h: i32, mi: i32, s: i32) -> i64 {
    // (months normalised first: mktime takes any, but keep the year exact)
    let (y, mo) = (y + mo.div_euclid(12), mo.rem_euclid(12));
    unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        tm.tm_year = y - 1900;
        tm.tm_mon = mo;
        tm.tm_mday = d;
        tm.tm_hour = h;
        tm.tm_min = mi;
        tm.tm_sec = s;
        tm.tm_isdst = -1;
        libc::mktime(&mut tm) as i64 * 1000
    }
}

/// `Date.UTC(y, mo, d, h, mi, s)`
pub fn make_utc(y: i32, mo: i32, d: i32, h: i32, mi: i32, s: i32) -> i64 {
    let (y, mo) = (y as i64 + mo.div_euclid(12) as i64, mo.rem_euclid(12) as i64 + 1);
    // (days from civil)
    let yy = if mo <= 2 { y - 1 } else { y };
    let era = yy.div_euclid(400);
    let yoe = yy - era * 400;
    let doy = (153 * (if mo > 2 { mo - 3 } else { mo + 9 }) + 2) / 5 + d as i64 - 1;
    let days = era * 146097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719468;
    ((days * 86400) + h as i64 * 3600 + mi as i64 * 60 + s as i64) * 1000
}

// ── Events ───────────────────────────────────────────────────────────────

/// One occurrence, as Calendar.between gives it (times in ms)
#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub key: String,
    pub title: String,
    pub start: i64,
    pub end: i64,
    pub all_day: bool,
    /// the calendar's colour ("" for your own)
    pub color: String,
}

/// Your own event (events.json)
#[derive(Clone, Debug, PartialEq)]
pub struct Local {
    pub id: String,
    pub title: String,
    pub start: i64,
    /// 0: none (the same as its start)
    pub end: i64,
    pub all_day: bool,
}

/// A subscription (calendars.json)
#[derive(Clone, Debug, PartialEq)]
pub struct Source {
    pub name: String,
    pub url: String,
    pub color: String,
    pub enabled: bool,
}

impl Source {
    /// A file on this computer (read here), not a link to fetch
    pub fn is_file(&self) -> bool {
        self.url.starts_with('/') || self.url.starts_with("file://")
    }
}

/// An iCalendar VEVENT
#[derive(Clone, Debug, Default, PartialEq)]
pub struct IcsEvent {
    pub uid: String,
    pub title: String,
    pub start: i64,
    pub end: i64,
    pub all_day: bool,
    pub rrule: Option<Vec<(String, String)>>,
    pub exdates: Vec<i64>,
    pub recurrence: Option<i64>,
    pub cancelled: bool,
}

/// What the service reads: your events and the file subscriptions, parsed
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Calendar {
    pub local: Vec<Local>,
    pub sources: Vec<Source>,
    /// parsed events by subscription url (files only)
    pub parsed: HashMap<String, Vec<IcsEvent>>,
}

impl Calendar {
    /// Occurrences overlapping [from, to), all-day ones first, then by start
    /// (Calendar.between without the accounts')
    pub fn between(&self, from: i64, to: i64) -> Vec<Event> {
        let mut out = Vec::new();
        for e in &self.local {
            let end = if e.end != 0 { e.end } else { e.start };
            if e.start < to && (end > from || ((e.end == 0 || e.end == e.start) && e.start >= from)) {
                out.push(Event { key: format!("local:{}", e.id), title: e.title.clone(), start: e.start, end, all_day: e.all_day, color: String::new() });
            }
        }
        for src in &self.sources {
            if !src.enabled {
                continue;
            }
            for ev in self.parsed.get(&src.url).map(Vec::as_slice).unwrap_or(&[]) {
                for (s, e) in expand(ev, from, to) {
                    out.push(Event { key: format!("{}|{}|{}", src.url, ev.uid, s), title: ev.title.clone(), start: s, end: e, all_day: ev.all_day, color: src.color.clone() });
                }
            }
        }
        sort(&mut out);
        out
    }
}

/// All-day first, then by start (stable, as Array.sort)
pub fn sort(list: &mut [Event]) {
    list.sort_by(|a, b| b.all_day.cmp(&a.all_day).then(a.start.cmp(&b.start)));
}

/// The events of a local day (Calendar.eventsOn)
pub fn day_bounds(ms: i64) -> (i64, i64) {
    let t = local(ms);
    (make(t.y, t.mo, t.d, 0, 0, 0), make(t.y, t.mo, t.d + 1, 0, 0, 0))
}

// ── iCalendar ────────────────────────────────────────────────────────────

/// "20261026T090000Z" | "20261026T090000" | "20261026" -> (ms, all day)
pub fn ics_date(value: &str, params: &str) -> (i64, bool) {
    let v = value.trim();
    let n = |a: usize, b: usize| v.get(a..b).and_then(|s| s.parse::<i32>().ok()).unwrap_or(0);
    let (y, mo, d) = (n(0, 4), n(4, 6) - 1, n(6, 8));
    // (VALUE=DATE, not VALUE=DATE-TIME)
    let date_only = params.match_indices("VALUE=DATE").any(|(i, _)| !params[i + 10..].starts_with('-'));
    if v.len() == 8 || date_only {
        return (make(y, mo, d, 0, 0, 0), true);
    }
    let (h, mi, s) = (n(9, 11), n(11, 13), n(13, 15));
    if v.ends_with('Z') {
        (make_utc(y, mo, d, h, mi, s), false)
    } else {
        (make(y, mo, d, h, mi, s), false)
    }
}

/// "-PT15M", "PT1H", "-P1D", "P1DT2H" -> minutes (negative = before)
pub fn duration(v: &str) -> Option<i64> {
    let (neg, rest) = match v.as_bytes().first() {
        Some(b'-') => (true, &v[1..]),
        Some(b'+') => (false, &v[1..]),
        _ => (false, v),
    };
    let rest = rest.strip_prefix('P')?;
    let (date, time) = match rest.split_once('T') {
        Some((d, t)) => (d, Some(t)),
        None => (rest, None),
    };
    let mut mins = 0i64;
    let mut take = |s: &str, units: &[(char, i64)]| -> Option<()> {
        let mut num = String::new();
        let mut at = 0;
        for c in s.chars() {
            if c.is_ascii_digit() {
                num.push(c);
            } else {
                // (units in order, each once)
                let i = units.iter().skip(at).position(|(u, _)| *u == c)? + at;
                mins += num.parse::<i64>().ok()? * units[i].1;
                num.clear();
                at = i + 1;
            }
        }
        num.is_empty().then_some(())
    };
    take(date, &[('W', 7 * 1440), ('D', 1440)])?;
    if let Some(t) = time {
        // (seconds count for nothing here, as in Calendar.qml)
        take(t, &[('H', 60), ('M', 1), ('S', 0)])?;
    }
    Some(if neg { -mins } else { mins })
}

fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.peek() {
                Some('n') | Some('N') => {
                    it.next();
                    out.push('\n');
                }
                Some(&x @ (',' | ';' | '\\')) => {
                    it.next();
                    out.push(x);
                }
                _ => out.push(c),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// The VEVENTs of a calendar file (Calendar.parseIcs)
pub fn parse_ics(text: &str) -> Vec<IcsEvent> {
    let unfolded = text.replace("\r\n ", "").replace("\r\n\t", "").replace("\n ", "").replace("\n\t", "");
    let mut events = Vec::new();
    let mut overrides = Vec::new();
    let mut ev: Option<(IcsEvent, bool, Option<i64>)> = None; // (event, has start, duration minutes)
    let mut has_end = false;
    let mut in_alarm = false;
    for line in unfolded.split('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line == "BEGIN:VEVENT" {
            ev = Some((IcsEvent { title: "(No title)".into(), ..Default::default() }, false, None));
            has_end = false;
            continue;
        }
        let Some((e, started, dur)) = ev.as_mut() else { continue };
        if line == "BEGIN:VALARM" {
            in_alarm = true;
            continue;
        }
        if line == "END:VALARM" {
            in_alarm = false;
            continue;
        }
        if line == "END:VEVENT" {
            if *started {
                if !has_end {
                    e.end = match dur {
                        Some(m) => e.start + *m * 60_000,
                        None if e.all_day => e.start + DAY,
                        None => e.start,
                    };
                }
                let e = std::mem::take(e);
                if e.recurrence.is_some() {
                    overrides.push(e);
                } else {
                    events.push(e);
                }
            }
            ev = None;
            continue;
        }
        if in_alarm {
            continue;
        }
        let Some(colon) = line.find(':') else { continue };
        let (head, value) = (&line[..colon], &line[colon + 1..]);
        let name = head.split(';').next().unwrap_or("").to_uppercase();
        let params = &head[name.len().min(head.len())..];
        match name.as_str() {
            "UID" => e.uid = value.to_string(),
            "SUMMARY" => e.title = unescape(value),
            "DTSTART" => {
                let (d, all) = ics_date(value, params);
                e.start = d;
                e.all_day = all;
                *started = true;
            }
            "DTEND" => {
                e.end = ics_date(value, params).0;
                has_end = true;
            }
            "DURATION" => *dur = duration(value),
            "RRULE" => {
                e.rrule = Some(value.split(';').map(|p| p.split_once('=').map_or((p.to_string(), String::new()), |(k, v)| (k.to_string(), v.to_string()))).collect());
            }
            "EXDATE" => {
                for v in value.split(',') {
                    e.exdates.push(ics_date(v, params).0);
                }
            }
            "RECURRENCE-ID" => e.recurrence = Some(ics_date(value, params).0),
            "STATUS" => {
                if value == "CANCELLED" {
                    e.cancelled = true;
                }
            }
            _ => {}
        }
    }
    // Moved instances replace the original occurrence
    for o in overrides {
        if let Some(master) = events.iter_mut().find(|e| e.uid == o.uid) {
            master.exdates.push(o.recurrence.unwrap_or(0));
        }
        if !o.cancelled {
            events.push(IcsEvent { recurrence: None, rrule: None, ..o });
        }
    }
    events.retain(|e| !e.cancelled);
    events
}

const DAYS: [&str; 7] = ["SU", "MO", "TU", "WE", "TH", "FR", "SA"];

#[derive(Clone, Copy)]
struct ByDay {
    n: i32,
    d: i32,
}

/// Occurrences of one event overlapping [from, to), as (start, end)
/// (Calendar.expand)
pub fn expand(ev: &IcsEvent, from: i64, to: i64) -> Vec<(i64, i64)> {
    if to <= from || ev.end < ev.start {
        return Vec::new();
    }
    let len = ev.end - ev.start;
    let once: Vec<(i64, i64)> = if (ev.start < to && ev.end > from) || (len == 0 && ev.start >= from && ev.start < to) { vec![(ev.start, ev.end)] } else { Vec::new() };
    let Some(rule) = &ev.rrule else { return once };
    let get = |k: &str| rule.iter().find(|(x, _)| x == k).map(|(_, v)| v.as_str());
    let freq = get("FREQ").unwrap_or("");
    let interval: i64 = match get("INTERVAL") {
        Some(v) => match v.parse::<i64>() {
            Ok(i) if i >= 1 => i,
            _ => return Vec::new(),
        },
        None => 1,
    };
    let count: i64 = get("COUNT").map_or(i64::MAX, |v| v.parse::<f64>().map_or(i64::MAX, |c| if c.is_finite() { c as i64 } else { i64::MAX }));
    let until: i64 = get("UNTIL").map_or(i64::MAX, |u| ics_date(u, "").0 + if u.len() == 8 { DAY - 1 } else { 0 });
    let byday: Option<Vec<ByDay>> = get("BYDAY").map(|v| {
        v.split(',')
            .map(|s| {
                let (num, day) = s.split_at(s.len().saturating_sub(2));
                let d = DAYS.iter().position(|x| *x == day).map_or(-1, |i| i as i32);
                let n_ok = num.is_empty() || (num.trim_start_matches(['+', '-']).len() <= 2 && num.trim_start_matches(['+', '-']).chars().all(|c| c.is_ascii_digit()) && !num.trim_start_matches(['+', '-']).is_empty());
                if d < 0 || !n_ok {
                    ByDay { n: 99, d: -1 }
                } else {
                    ByDay { n: if num.is_empty() { 0 } else { num.parse().unwrap_or(99) }, d }
                }
            })
            .collect()
    });
    // (NaN where a value isn't a usable number: the rule isn't followed)
    let ints = |v: Option<&str>, max: i64| -> Option<Vec<Option<i64>>> {
        v.map(|v| {
            v.split(',')
                .map(|s| {
                    let digits = s.trim_start_matches(['+', '-']);
                    let ok = !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()) && s.len() - digits.len() <= 1;
                    let n: Option<i64> = if ok { s.parse().ok() } else { None };
                    n.filter(|n| *n != 0 && n.abs() <= max)
                })
                .collect()
        })
    };
    let bymonthday = ints(get("BYMONTHDAY"), 31);
    let bymonth = ints(get("BYMONTH"), 12);
    let bysetpos = ints(get("BYSETPOS"), 366);
    let wkst = DAYS.iter().position(|x| *x == get("WKST").unwrap_or("MO")).map_or(-1, |i| i as i32);
    let nth = byday.as_ref().is_some_and(|b| b.iter().any(|x| x.n != 0));
    let known = ["FREQ", "INTERVAL", "COUNT", "UNTIL", "BYDAY", "BYMONTHDAY", "BYMONTH", "BYSETPOS", "WKST"];
    let bad = |l: &Option<Vec<Option<i64>>>| l.as_ref().is_some_and(|l| l.iter().any(Option::is_none));
    let supported = rule.iter().all(|(k, _)| k.is_empty() || known.contains(&k.as_str()))
        && ["DAILY", "WEEKLY", "MONTHLY", "YEARLY"].contains(&freq)
        && wkst >= 0
        && !bad(&bymonthday)
        && !bad(&bymonth)
        && !bad(&bysetpos)
        && !byday.as_ref().is_some_and(|b| b.iter().any(|x| x.d < 0 || x.n.abs() > 5))
        && !(nth && (freq == "DAILY" || freq == "WEEKLY"))
        && !(bymonthday.is_some() && freq == "WEEKLY")
        && !(bysetpos.is_some() && (freq == "DAILY" || freq == "WEEKLY"))
        && !(freq == "YEARLY" && bymonth.is_none() && (byday.is_some() || bymonthday.is_some()));
    if !supported {
        return once;
    }
    let flat = |l: &Option<Vec<Option<i64>>>| l.as_ref().map(|l| l.iter().map(|x| x.unwrap_or(0)).collect::<Vec<i64>>());
    let (bymonthday, bymonth, bysetpos) = (flat(&bymonthday), flat(&bymonth), flat(&bysetpos));

    let s0 = local(ev.start);
    let at = |y: i32, m: i32, d: i32| make(y, m, d, s0.h, s0.mi, s0.s);
    let last_of = |y: i32, m: i32| local(make(y, m + 1, 0, 12, 0, 0)).d;
    let nth_weekday = |y: i32, m: i32, wd: i32, k: i32| -> Option<i32> {
        if k > 0 {
            let first = local(make(y, m, 1, 12, 0, 0)).wday;
            let d = 1 + (wd - first + 7) % 7 + (k - 1) * 7;
            (d <= last_of(y, m)).then_some(d)
        } else {
            let last = local(make(y, m + 1, 0, 12, 0, 0));
            let d = last.d - (last.wday - wd + 7) % 7 + (k + 1) * 7;
            (d >= 1).then_some(d)
        }
    };
    // The days of a month (m may run past December) the rule picks, in order
    let month_days = |y: i32, m: i32| -> Vec<i64> {
        let base = local(make(y, m, 1, 12, 0, 0));
        let (yy, mm) = (base.y, base.mo);
        let last = last_of(yy, mm);
        if bymonth.as_ref().is_some_and(|b| !b.contains(&(mm as i64 + 1))) {
            return Vec::new();
        }
        let selected: Option<Vec<i32>> = bymonthday.as_ref().map(|b| b.iter().map(|&d| if d < 0 { last + d as i32 + 1 } else { d as i32 }).filter(|d| *d >= 1 && *d <= last).collect());
        let weekdays: Option<Vec<i32>> = byday.as_ref().map(|b| {
            b.iter()
                .flat_map(|x| {
                    if x.n != 0 {
                        nth_weekday(yy, mm, x.d, x.n).into_iter().collect::<Vec<_>>()
                    } else {
                        (1..=last).filter(|d| local(make(yy, mm, *d, 12, 0, 0)).wday == x.d).collect()
                    }
                })
                .collect()
        });
        let mut dates: Vec<i32> = match (weekdays, &selected) {
            (Some(w), _) => w.into_iter().filter(|d| selected.as_ref().is_none_or(|s| s.contains(d))).collect(),
            (None, Some(s)) => s.clone(),
            (None, None) => vec![s0.d],
        };
        dates.sort();
        dates.dedup();
        dates.into_iter().filter(|d| *d >= 1 && *d <= last).map(|d| at(yy, mm, d)).collect()
    };
    // BYSETPOS: the 1st, 2nd... or (negative) last of each period's dates
    let setpos = |c: Vec<i64>| -> Vec<i64> {
        let Some(p) = &bysetpos else { return c };
        let mut out: Vec<i64> = p.iter().filter_map(|&p| if p > 0 { c.get(p as usize - 1) } else { c.len().checked_sub(p.unsigned_abs() as usize).and_then(|i| c.get(i)) }).copied().collect();
        out.sort();
        out.dedup();
        out
    };
    let filtered = byday.is_none() && bymonthday.is_none() && bymonth.is_none();

    let mut out = Vec::new();
    let mut n: i64 = 0;
    // Without COUNT, jump close to the window instead of walking from the first occurrence
    let mut i0: i64 = 0;
    if (count == i64::MAX || (freq == "DAILY" && filtered)) && from > ev.start {
        let earliest = from - len.max(0);
        let e = local(earliest);
        let months = (e.y - s0.y) as f64 * 12.0 + (e.mo - s0.mo) as f64;
        let steps = match freq {
            "DAILY" => (earliest - ev.start) as f64 / (DAY * interval) as f64,
            "WEEKLY" => (earliest - ev.start) as f64 / (7 * DAY * interval) as f64,
            "MONTHLY" => months / interval as f64,
            "YEARLY" => months / 12.0 / interval as f64,
            _ => 0.0,
        };
        i0 = (steps.floor() as i64 - 2).max(0);
    }
    if count != i64::MAX && freq == "DAILY" && filtered {
        n = i0;
    }
    let mut i = i0;
    // (a guard: a rule this can't step through ends rather than spins)
    while n < count && i - i0 < 100_000 {
        let ii = i as i32;
        let iv = interval as i32;
        let (period, candidates): (i64, Vec<i64>) = match freq {
            "DAILY" => {
                let d = at(s0.y, s0.mo, s0.d + ii * iv);
                let t = local(d);
                let last = last_of(t.y, t.mo);
                let ok = byday.as_ref().is_none_or(|b| b.iter().any(|x| x.d == t.wday))
                    && bymonth.as_ref().is_none_or(|b| b.contains(&(t.mo as i64 + 1)))
                    && bymonthday.as_ref().is_none_or(|b| b.iter().any(|&x| (if x < 0 { last as i64 + x + 1 } else { x }) == t.d as i64));
                (d, if ok { vec![d] } else { Vec::new() })
            }
            "WEEKLY" => {
                let p = at(s0.y, s0.mo, s0.d - (s0.wday - wkst + 7) % 7 + ii * 7 * iv);
                let pt = local(p);
                let days: Vec<i32> = byday.as_ref().map_or_else(|| vec![s0.wday], |b| b.iter().map(|x| x.d).collect());
                let mut c: Vec<i64> = days.iter().map(|d| at(pt.y, pt.mo, pt.d + (d - wkst + 7) % 7)).filter(|d| bymonth.as_ref().is_none_or(|b| b.contains(&(local(*d).mo as i64 + 1)))).collect();
                c.sort();
                (p, c)
            }
            "MONTHLY" => (make(s0.y, s0.mo + ii * iv, 1, 0, 0, 0), setpos(month_days(s0.y, s0.mo + ii * iv))),
            _ => {
                let year = s0.y + ii * iv;
                let months: Vec<i64> = bymonth.clone().unwrap_or_else(|| vec![s0.mo as i64 + 1]);
                let mut c: Vec<i64> = months.iter().flat_map(|m| month_days(year, *m as i32 - 1)).collect();
                c.sort();
                (make(year, 0, 1, 0, 0, 0), setpos(c))
            }
        };
        if period >= to || period > until {
            break;
        }
        if candidates.first().is_some_and(|c| *c >= to) {
            break;
        }
        let mut stop = false;
        for c in candidates {
            // (push: before the first, past UNTIL, or COUNT reached → false)
            let pushed = if c < ev.start || c > until || n >= count {
                false
            } else {
                n += 1;
                if !ev.exdates.contains(&c) && c < to && (c + len > from || (len == 0 && c >= from)) {
                    out.push((c, c + len));
                }
                true
            };
            if !pushed && c > until {
                stop = true;
                break;
            }
        }
        if stop {
            break;
        }
        i += 1;
    }
    out
}

// ── Reading the files ────────────────────────────────────────────────────

fn config_dir() -> PathBuf {
    crate::theme::config_path().parent().map(PathBuf::from).unwrap_or_default()
}

fn num(v: &serde_json::Value) -> i64 {
    v.as_f64().map_or(0, |f| f as i64)
}

/// events.json: [{ id, title, start, end, allDay, … }]
pub fn read_local(text: &str) -> Vec<Local> {
    let v: serde_json::Value = serde_json::from_str(text).unwrap_or_default();
    v.as_array()
        .map(|a| {
            a.iter()
                .filter(|e| e["start"].is_number())
                .map(|e| Local {
                    id: e["id"].as_str().unwrap_or("").to_string(),
                    title: e["title"].as_str().unwrap_or("").to_string(),
                    start: num(&e["start"]),
                    end: num(&e["end"]),
                    all_day: e["allDay"].as_bool().unwrap_or(false),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// calendars.json: { sources: [{ name, url, color, enabled }] }
pub fn read_sources(text: &str) -> Vec<Source> {
    let v: serde_json::Value = serde_json::from_str(text).unwrap_or_default();
    v["sources"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|s| {
                    Some(Source {
                        name: s["name"].as_str().unwrap_or("Calendar").to_string(),
                        url: s["url"].as_str()?.to_string(),
                        color: s["color"].as_str().unwrap_or("").to_string(),
                        enabled: s["enabled"].as_bool() != Some(false),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn mtime(p: &std::path::Path) -> Option<SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

fn read_all() -> (Calendar, Vec<(PathBuf, Option<SystemTime>)>) {
    let events = crate::util::state_dir().join("events.json");
    let sources = config_dir().join("calendars.json");
    let mut cal = Calendar {
        local: std::fs::read_to_string(&events).map(|t| read_local(&t)).unwrap_or_default(),
        sources: std::fs::read_to_string(&sources).map(|t| read_sources(&t)).unwrap_or_default(),
        parsed: HashMap::new(),
    };
    let mut watched = vec![(events.clone(), mtime(&events)), (sources.clone(), mtime(&sources))];
    for s in cal.sources.iter().filter(|s| s.enabled && s.is_file()) {
        let path = PathBuf::from(s.url.strip_prefix("file://").unwrap_or(&s.url));
        watched.push((path.clone(), mtime(&path)));
        if let Ok(t) = std::fs::read_to_string(&path) {
            if t.contains("BEGIN:VCALENDAR") {
                cal.parsed.insert(s.url.clone(), parse_ics(&t));
            }
        }
    }
    (cal, watched)
}

pub fn spawn(tx: Sender<Msg>) {
    std::thread::spawn(move || {
        let (cal, mut watched) = read_all();
        if tx.send(Msg::Data(Box::new(cal))).is_err() {
            return;
        }
        loop {
            std::thread::sleep(Duration::from_secs(2));
            if watched.iter().any(|(p, t)| mtime(p) != *t) {
                let (cal, w) = read_all();
                watched = w;
                if tx.send(Msg::Data(Box::new(cal))).is_err() {
                    return;
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(start: i64, end: i64, rule: &str) -> IcsEvent {
        IcsEvent {
            uid: "u".into(),
            title: "t".into(),
            start,
            end,
            rrule: (!rule.is_empty()).then(|| rule.split(';').map(|p| p.split_once('=').map(|(k, v)| (k.to_string(), v.to_string())).unwrap()).collect()),
            ..Default::default()
        }
    }

    fn days(v: &[(i64, i64)]) -> Vec<(i32, i32, i32)> {
        v.iter().map(|(s, _)| local(*s)).map(|t| (t.y, t.mo + 1, t.d)).collect()
    }

    #[test]
    fn local_dates_roll_over_as_javascript() {
        let a = local(make(2026, 0, 32, 9, 0, 0));
        assert_eq!((a.y, a.mo, a.d, a.h), (2026, 1, 1, 9));
        assert_eq!(local(make(2026, 2, 0, 12, 0, 0)).d, 28); // (the last of February)
        assert_eq!(local(make(2026, 12, 1, 12, 0, 0)).y, 2027);
        assert_eq!(make_utc(1970, 0, 1, 0, 0, 0), 0);
        assert_eq!(make_utc(2026, 10, 5, 9, 30, 0), 1793871000000);
    }

    #[test]
    fn ics_dates_and_durations() {
        assert_eq!(ics_date("20261105", ""), (make(2026, 10, 5, 0, 0, 0), true));
        assert_eq!(ics_date("20261105T093000", ";TZID=Europe/Paris"), (make(2026, 10, 5, 9, 30, 0), false));
        assert_eq!(ics_date("20261105T093000Z", ""), (make_utc(2026, 10, 5, 9, 30, 0), false));
        assert!(ics_date("20261105", ";VALUE=DATE").1);
        assert!(!ics_date("20261105T090000", ";VALUE=DATE-TIME").1);
        assert_eq!(duration("-PT15M"), Some(-15));
        assert_eq!(duration("PT1H"), Some(60));
        assert_eq!(duration("P1DT2H"), Some(1560));
        assert_eq!(duration("P1W"), Some(10080));
        assert_eq!(duration("1H"), None);
    }

    #[test]
    fn a_calendar_file_parsed() {
        let text = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:a\r\nSUMMARY:Stand-up\\, daily\r\nDTSTART:20261102T093000\r\nDURATION:PT15M\r\nRRULE:FREQ=DAILY;COUNT=5\r\nEXDATE:20261104T093000\r\nBEGIN:VALARM\r\nTRIGGER:-PT5M\r\nSUMMARY:not the title\r\nEND:VALARM\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nUID:a\r\nRECURRENCE-ID:20261105T093000\r\nSUMMARY:Stand-up (moved)\r\nDTSTART:20261105T110000\r\nDTEND:20261105T111500\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nUID:b\r\nSUMMARY:Holiday\r\nDTSTART;VALUE=DATE:20261105\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nUID:c\r\nSUMMARY:Gone\r\nDTSTART:20261105T120000\r\nSTATUS:CANCELLED\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nUID:d\r\nSUMMARY:Long\r\n  title\r\nDTSTART:20261105T150000\r\nDTEND:20261105T160000\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let evs = parse_ics(text);
        assert_eq!(evs.iter().map(|e| e.title.as_str()).collect::<Vec<_>>(), vec!["Stand-up, daily", "Holiday", "Long title", "Stand-up (moved)"]);
        assert_eq!(evs[0].end - evs[0].start, 15 * 60_000);
        assert_eq!(evs[1].end - evs[1].start, DAY);
        let mut cal = Calendar::default();
        cal.sources.push(Source { name: "Work".into(), url: "/w.ics".into(), color: "#6D7CDF".into(), enabled: true });
        cal.parsed.insert("/w.ics".into(), evs);
        cal.local.push(Local { id: "x".into(), title: "Mine".into(), start: make(2026, 10, 5, 8, 0, 0), end: 0, all_day: false });
        let (a, b) = day_bounds(make(2026, 10, 5, 13, 0, 0));
        let today = cal.between(a, b);
        assert_eq!(today.iter().map(|e| e.title.as_str()).collect::<Vec<_>>(), vec!["Holiday", "Mine", "Stand-up (moved)", "Long title"]);
        assert_eq!(today[0].color, "#6D7CDF");
        assert_eq!(today[1].key, "local:x");
        // five from the 2nd: the 4th taken out, the 5th moved (both still count)
        let (a, _) = day_bounds(make(2026, 10, 2, 13, 0, 0));
        let (_, b) = day_bounds(make(2026, 10, 7, 13, 0, 0));
        let ups: Vec<_> = cal.between(a, b).into_iter().filter(|e| e.title == "Stand-up, daily").map(|e| local(e.start).d).collect();
        assert_eq!(ups, vec![2, 3, 6]);
        // (a disabled calendar shows nothing)
        cal.sources[0].enabled = false;
        assert_eq!(cal.between(a, b).len(), 1);
    }

    #[test]
    fn repeating_rules() {
        let s = make(2026, 0, 5, 9, 0, 0); // a Monday
        let e = s + 3_600_000;
        let win = |from: (i32, i32, i32), to: (i32, i32, i32)| (make(from.0, from.1 - 1, from.2, 0, 0, 0), make(to.0, to.1 - 1, to.2, 0, 0, 0));
        let (a, b) = win((2026, 1, 1), (2026, 1, 20));
        assert_eq!(days(&expand(&ev(s, e, "FREQ=WEEKLY;BYDAY=MO,WE"), a, b)), vec![(2026, 1, 5), (2026, 1, 7), (2026, 1, 12), (2026, 1, 14), (2026, 1, 19)]);
        assert_eq!(days(&expand(&ev(s, e, "FREQ=DAILY;INTERVAL=3;COUNT=3"), a, b)), vec![(2026, 1, 5), (2026, 1, 8), (2026, 1, 11)]);
        assert_eq!(days(&expand(&ev(s, e, "FREQ=DAILY;UNTIL=20260107"), a, b)), vec![(2026, 1, 5), (2026, 1, 6), (2026, 1, 7)]);
        // the last Friday of each month; the second Tuesday
        let (a, b) = win((2026, 1, 1), (2026, 4, 1));
        assert_eq!(days(&expand(&ev(s, e, "FREQ=MONTHLY;BYDAY=-1FR"), a, b)), vec![(2026, 1, 30), (2026, 2, 27), (2026, 3, 27)]);
        assert_eq!(days(&expand(&ev(s, e, "FREQ=MONTHLY;BYDAY=2TU"), a, b)), vec![(2026, 1, 13), (2026, 2, 10), (2026, 3, 10)]);
        // the last weekday of the month (BYSETPOS); the 31st only where there is one
        assert_eq!(days(&expand(&ev(s, e, "FREQ=MONTHLY;BYDAY=MO,TU,WE,TH,FR;BYSETPOS=-1"), a, b)), vec![(2026, 1, 30), (2026, 2, 27), (2026, 3, 31)]);
        assert_eq!(days(&expand(&ev(s, e, "FREQ=MONTHLY;BYMONTHDAY=31"), a, b)), vec![(2026, 1, 31), (2026, 3, 31)]);
        // yearly, far from its start (jumps close to the window)
        let bday = make(1990, 10, 5, 0, 0, 0);
        let mut b1 = ev(bday, bday + DAY, "FREQ=YEARLY");
        b1.all_day = true;
        let (a, b) = day_bounds(make(2026, 10, 5, 12, 0, 0));
        assert_eq!(days(&expand(&b1, a, b)), vec![(2026, 11, 5)]);
        // a rule with parts it doesn't follow: the first date only
        let (a, b) = win((2026, 1, 1), (2026, 1, 20));
        assert_eq!(days(&expand(&ev(s, e, "FREQ=DAILY;BYHOUR=9"), a, b)), vec![(2026, 1, 5)]);
        assert!(expand(&ev(s, e, "FREQ=DAILY;INTERVAL=0"), a, b).is_empty());
    }

    #[test]
    fn your_events_and_subscriptions_from_their_files() {
        let l = read_local(r#"[{"id":"a","title":"Dentist","start":1793865600000,"end":1793869200000,"allDay":false,"alert":10},{"title":"no start"}]"#);
        assert_eq!(l.len(), 1);
        assert_eq!((l[0].title.as_str(), l[0].end - l[0].start), ("Dentist", 3_600_000));
        let s = read_sources(r##"{"sources":[{"name":"Work","url":"/home/u/w.ics","color":"#6D7CDF","enabled":true},{"name":"Web","url":"https://example.org/c.ics","enabled":false}],"defaultAlert":10}"##);
        assert_eq!(s.len(), 2);
        assert!(s[0].is_file() && !s[1].is_file() && !s[1].enabled);
        assert!(read_local("nonsense").is_empty() && read_sources("{}").is_empty());
    }
}

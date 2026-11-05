//! What Today shows, from the services' readings and settings: plain values
//! (the words, the times, which rows) as TodayContent.qml,
//! MaterialTodayContent.qml, TodayOptions.qml and services/Today.qml work
//! them out. English names, as the shell's Qt formats give them.

use crate::services::calendar::{self, Event};
use crate::services::todos::{self, Todo};

// ── Settings ─────────────────────────────────────────────────────────────

pub const MODES: [&str; 4] = ["ask", "widget", "both", "off"];
pub const ENTRANCES: [&str; 3] = ["bloom", "shrink", "drop"];

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    /// today.mode (ask | widget | both | off)
    pub mode: String,
    /// today.entrance (bloom | shrink | drop)
    pub entrance: String,
    /// today.name ("" = the account's)
    pub name: String,
    /// sounds.today
    pub chime: bool,
    /// appearance.reduceMotion
    pub reduce_motion: bool,
    /// appearance.clock24
    pub clock24: bool,
}

/// Config.qml's defaults where a key is missing (Today.qml: an unknown mode is "ask")
pub fn settings(cfg: &serde_json::Value) -> Settings {
    let s = |v: &serde_json::Value, d: &str| v.as_str().unwrap_or(d).to_string();
    let mode = s(&cfg["today"]["mode"], "ask");
    Settings {
        mode: if MODES.contains(&mode.as_str()) { mode } else { "ask".into() },
        entrance: s(&cfg["today"]["entrance"], "bloom"),
        name: s(&cfg["today"]["name"], ""),
        chime: cfg["sounds"]["today"].as_bool().unwrap_or(true),
        reduce_motion: cfg["appearance"]["reduceMotion"].as_bool().unwrap_or(false),
        clock24: cfg["appearance"]["clock24"].as_bool().unwrap_or(false),
    }
}

impl Settings {
    /// The widget is on the desktop every day (Today.widgetAlways): no Pin for today
    pub fn widget_always(&self) -> bool {
        self.mode == "widget" || self.mode == "both"
    }
}

/// The entrance a show uses (Today.show: the one asked for, else the setting's)
pub fn entrance(asked: &str, setting: &str) -> &'static str {
    ENTRANCES.iter().find(|e| **e == asked).or_else(|| ENTRANCES.iter().find(|e| **e == setting)).copied().unwrap_or("bloom")
}

// ── Words ────────────────────────────────────────────────────────────────

const WEEKDAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

/// TimeFormat.hm: "9:05 AM", or "09:05"
pub fn hm(ms: i64, h24: bool) -> String {
    let t = calendar::local(ms);
    if h24 {
        format!("{:02}:{:02}", t.h, t.mi)
    } else {
        format!("{}:{:02} {}", (t.h + 11) % 12 + 1, t.mi, if t.h < 12 { "AM" } else { "PM" })
    }
}

/// TimeFormat.hOnly: "9 AM", or "09"
pub fn hour(h: u32, h24: bool) -> String {
    if h24 {
        format!("{h:02}")
    } else {
        format!("{} {}", (h + 11) % 12 + 1, if h < 12 { "AM" } else { "PM" })
    }
}

/// "dddd, d MMMM": "Thursday, 5 November"
pub fn long_date(ms: i64) -> String {
    let t = calendar::local(ms);
    format!("{}, {} {}", WEEKDAYS[t.wday as usize], t.d, MONTHS[t.mo as usize])
}

/// "ddd d MMM": "Thu 5 Nov"
pub fn short_date(ms: i64) -> String {
    let t = calendar::local(ms);
    format!("{} {} {}", &WEEKDAYS[t.wday as usize][..3], t.d, &MONTHS[t.mo as usize][..3])
}

/// "MMM", upper case: "NOV"; and the day of the month
pub fn cookie(ms: i64) -> (String, String) {
    let t = calendar::local(ms);
    (MONTHS[t.mo as usize][..3].to_uppercase(), t.d.to_string())
}

/// Today.dayKey: "YYYY-MM-DD" of the day, days starting at 4 AM
pub fn day_key(ms: i64) -> String {
    let t = calendar::local(ms - 4 * 3_600_000);
    format!("{:04}-{:02}-{:02}", t.y, t.mo + 1, t.d)
}

/// The first name it greets (Today.name)
pub fn first_name(setting: &str, account: &str) -> String {
    let n = if setting.trim().is_empty() { account } else { setting };
    n.split_whitespace().next().unwrap_or("").to_string()
}

/// Today.greeting
pub fn greeting(ms: i64, name: &str) -> String {
    let h = calendar::local(ms).h;
    let part = if (4..12).contains(&h) {
        "Good morning"
    } else if (12..17).contains(&h) {
        "Good afternoon"
    } else {
        "Good evening"
    };
    if name.is_empty() { part.to_string() } else { format!("{part}, {name}") }
}

/// Calendar.timeText
pub fn time_text(e: &Event, h24: bool) -> String {
    if e.all_day {
        return "All day".into();
    }
    let mut s = hm(e.start, h24);
    if e.end > e.start {
        s += " – ";
        s += &hm(e.end, h24);
    }
    s
}

/// What the day holds, in a few words (Material's greeting card)
pub fn summary(events: usize, todos: usize) -> String {
    if events == 0 && todos == 0 {
        return "A clear day".into();
    }
    let mut parts = Vec::new();
    if events > 0 {
        parts.push(format!("{events} {}", if events == 1 { "event" } else { "events" }));
    }
    if todos > 0 {
        parts.push(format!("{todos} {}", if todos == 1 { "to-do" } else { "to-dos" }));
    }
    parts.join(" · ")
}

pub fn mode_hint(mode: &str) -> &'static str {
    match mode {
        "widget" => "No morning screen. The Today widget stays on your desktop; remove it and it comes back tomorrow.",
        "both" => "Opens on your first unlock after 4 AM, and the Today widget stays on your desktop.",
        "off" => "Today doesn't open by itself. Show Today in Search opens it, and the Today widget is in the widget gallery.",
        _ => "Opens on your first unlock after 4 AM. Close it and it's gone until tomorrow; Pin for today keeps it on the desktop until the day ends.",
    }
}

pub fn entrance_hint(entrance: &str) -> &'static str {
    match entrance {
        "shrink" => "A full-screen sheet draws in to the panel",
        "drop" => "A drop falls from the top and spreads into the panel",
        _ => "The panel settles out of the frosted screen",
    }
}

/// The name field's placeholder and hint
pub fn name_words(account: &str) -> (String, String) {
    let first = first_name("", account);
    let placeholder = if first.is_empty() { "Your name".to_string() } else { first.clone() };
    let hint = if account.is_empty() { "What the greeting calls you. Empty: no name".to_string() } else { format!("What the greeting calls you. Empty: \"{first}\", from your account") };
    (placeholder, hint)
}

pub fn weather_problem(error: &str) -> &'static str {
    if error == "Offline" {
        "Offline. The weather shows once you're connected."
    } else {
        "The weather isn't available right now."
    }
}

// ── The day ──────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub struct EventRow {
    pub time: String,
    pub title: String,
    /// "" for the accent / primary
    pub color: String,
    pub past: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TodoRow {
    pub key: String,
    pub text: String,
    pub due: String,
    pub overdue: bool,
}

/// Today's events (past ones dimmed), as rows
pub fn event_rows(events: &[Event], now: i64, h24: bool) -> Vec<EventRow> {
    events.iter().map(|e| EventRow { time: time_text(e, h24), title: e.title.clone(), color: e.color.clone(), past: !e.all_day && e.end < now }).collect()
}

/// The open to-dos, soonest first, five at most
pub fn open_todos(list: &[Todo]) -> Vec<Todo> {
    let mut v = todos::sorted(list.iter().filter(|t| !t.done).cloned().collect());
    v.truncate(5);
    v
}

pub fn todo_rows(open: &[Todo], now: i64, h24: bool) -> Vec<TodoRow> {
    open.iter()
        .map(|t| TodoRow {
            key: t.key.clone(),
            text: t.text.clone(),
            due: if t.due == 0 {
                String::new()
            } else if t.all_day {
                if day_key(t.due) == day_key(now) { "Today".into() } else { short_date(t.due) }
            } else {
                hm(t.due, h24)
            },
            overdue: t.due != 0 && t.due < now,
        })
        .collect()
}

/// When there are no open to-dos
pub fn todos_empty(any: bool) -> &'static str {
    if any { "All done" } else { "No to-dos. Add them in Notification Center." }
}

/// Today's events from every calendar: this computer's and the accounts'
/// (the shell's), in Calendar.between's order
pub fn merge(mut mine: Vec<Event>, accounts: &[Event], from: i64, to: i64) -> Vec<Event> {
    mine.extend(accounts.iter().filter(|e| e.start < to && (e.end > from || (e.end == e.start && e.start >= from))).cloned());
    calendar::sort(&mut mine);
    mine
}

/// The accounts' events and tasks from the shell's line
pub fn accounts(v: &serde_json::Value) -> (Vec<Event>, Vec<Todo>) {
    let n = |x: &serde_json::Value| x.as_f64().map_or(0, |f| f as i64);
    let events = v["events"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|e| e["start"].is_number())
                .map(|e| Event {
                    key: e["key"].as_str().unwrap_or("").to_string(),
                    title: e["title"].as_str().unwrap_or("").to_string(),
                    start: n(&e["start"]),
                    end: n(&e["end"]).max(n(&e["start"])),
                    all_day: e["allDay"].as_bool().unwrap_or(false),
                    color: e["color"].as_str().unwrap_or("").to_string(),
                })
                .collect()
        })
        .unwrap_or_default();
    let tasks = v["todos"].as_array().map(|a| a.iter().map(|t| todos::todo_from(t, t["key"].as_str().unwrap_or("").to_string())).collect()).unwrap_or_default();
    (events, tasks)
}

/// The account's full name (its passwd entry's), else the login (Account.qml)
pub fn account_name() -> String {
    unsafe {
        let uid = libc::getuid();
        let mut pw: libc::passwd = std::mem::zeroed();
        let mut out: *mut libc::passwd = std::ptr::null_mut();
        let mut buf = vec![0 as libc::c_char; 4096];
        if libc::getpwuid_r(uid, &mut pw, buf.as_mut_ptr(), buf.len(), &mut out) != 0 || out.is_null() {
            return std::env::var("USER").unwrap_or_default();
        }
        let s = |p: *const libc::c_char| if p.is_null() { String::new() } else { std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned() };
        let full = s(pw.pw_gecos).split(',').next().unwrap_or("").trim().to_string();
        if full.is_empty() { s(pw.pw_name) } else { full }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::calendar::make;
    use serde_json::json;

    #[test]
    fn settings_with_the_shells_defaults() {
        let s = settings(&serde_json::Value::Null);
        assert_eq!((s.mode.as_str(), s.entrance.as_str(), s.chime, s.clock24), ("ask", "bloom", true, false));
        assert!(!s.widget_always());
        let s = settings(&json!({"today": {"mode": "both", "name": "Sam"}, "sounds": {"today": false}}));
        assert!(s.widget_always() && !s.chime);
        assert_eq!(settings(&json!({"today": {"mode": "weird"}})).mode, "ask");
        assert_eq!(entrance("", "drop"), "drop");
        assert_eq!(entrance("shrink", "drop"), "shrink");
        assert_eq!(entrance("x", "y"), "bloom");
    }

    #[test]
    fn times_and_dates_in_words() {
        let t = make(2026, 10, 5, 9, 5, 0);
        assert_eq!(hm(t, false), "9:05 AM");
        assert_eq!(hm(make(2026, 10, 5, 0, 30, 0), false), "12:30 AM");
        assert_eq!(hm(make(2026, 10, 5, 13, 0, 0), true), "13:00");
        assert_eq!((hour(15, false), hour(0, false), hour(9, true)), ("3 PM".into(), "12 AM".into(), "09".into()));
        assert_eq!(long_date(t), "Thursday, 5 November");
        assert_eq!(short_date(t), "Thu 5 Nov");
        assert_eq!(cookie(t), ("NOV".into(), "5".into()));
        // days start at 4 AM
        assert_eq!(day_key(make(2026, 10, 6, 3, 0, 0)), "2026-11-05");
        assert_eq!(day_key(make(2026, 10, 6, 5, 0, 0)), "2026-11-06");
    }

    #[test]
    fn greetings() {
        assert_eq!(greeting(make(2026, 10, 5, 4, 0, 0), "Sam"), "Good morning, Sam");
        assert_eq!(greeting(make(2026, 10, 5, 12, 0, 0), ""), "Good afternoon");
        assert_eq!(greeting(make(2026, 10, 5, 3, 59, 0), "Sam"), "Good evening, Sam");
        assert_eq!(first_name("", "Sam Lee"), "Sam");
        assert_eq!(first_name("  Alex  B ", "Sam Lee"), "Alex");
        assert_eq!(name_words("Sam Lee").0, "Sam");
        assert_eq!(name_words("").0, "Your name");
    }

    #[test]
    fn the_days_rows() {
        let now = make(2026, 10, 5, 13, 0, 0);
        let ev = |t: &str, s: i64, e: i64, all: bool| Event { key: t.into(), title: t.into(), start: s, end: e, all_day: all, color: String::new() };
        let mine = vec![ev("Lunch", make(2026, 10, 5, 12, 0, 0), make(2026, 10, 5, 12, 30, 0), false)];
        let (a, b) = calendar::day_bounds(now);
        let acct = [ev("Holiday", a, b, true), ev("Review", make(2026, 10, 5, 15, 0, 0), make(2026, 10, 5, 16, 0, 0), false), ev("Tomorrow", b + 3_600_000, b + 7_200_000, false)];
        let all = merge(mine, &acct, a, b);
        let rows = event_rows(&all, now, false);
        assert_eq!(rows.iter().map(|r| r.title.as_str()).collect::<Vec<_>>(), vec!["Holiday", "Lunch", "Review"]);
        assert_eq!((rows[0].time.as_str(), rows[0].past), ("All day", false));
        assert_eq!((rows[1].time.as_str(), rows[1].past), ("12:00 PM – 12:30 PM", true));
        assert_eq!(summary(3, 1), "3 events · 1 to-do");
        assert_eq!(summary(0, 0), "A clear day");
        assert_eq!(summary(1, 0), "1 event");

        let td = |k: &str, due: i64, all: bool, done: bool| Todo { key: k.into(), text: k.into(), done, due, all_day: all };
        let list = vec![td("a", 0, false, false), td("b", make(2026, 10, 5, 23, 59, 0), true, false), td("c", make(2026, 10, 5, 9, 0, 0), false, false), td("d", 0, false, true), td("e", make(2026, 10, 8, 23, 59, 0), true, false)];
        let open = open_todos(&list);
        let rows = todo_rows(&open, now, false);
        assert_eq!(rows.iter().map(|r| (r.key.as_str(), r.due.as_str(), r.overdue)).collect::<Vec<_>>(), vec![("c", "9:00 AM", true), ("b", "Today", false), ("e", "Sun 8 Nov", false), ("a", "", false)]);
        assert_eq!(todos_empty(true), "All done");
    }

    #[test]
    fn the_accounts_items_from_the_shell() {
        let (e, t) = accounts(&json!({"events": [{"key": "g1", "title": "Standup", "start": 1000, "end": 2000, "allDay": false, "color": "#409D48"}, {"title": "no start"}], "todos": [{"key": "google/l/1", "text": "Call", "done": false}]}));
        assert_eq!((e.len(), e[0].color.as_str()), (1, "#409D48"));
        assert_eq!((t[0].key.as_str(), t[0].text.as_str()), ("google/l/1", "Call"));
        assert!(!account_name().is_empty());
    }
}

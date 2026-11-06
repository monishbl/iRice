//! Spotlight in plain words: shell/services/Intents.qml. What to do with a
//! typed phrase; each match is a result row on top of the search results:
//!   tasks    "remind me to call mum at 6", "todo buy milk friday"
//!   notes    "note gate code is 4512", "save https://… recipe"
//!   events   "meeting with Sam tomorrow 3pm", "event dentist on fri at 9"
//!   timers   "timer 10 min", "tea timer 4m", "alarm 7am", "wake me up at 6:30"
//!   system   "caffeine 1h", "dark mode", "wifi off", "bluetooth on",
//!            "focus 30m", "night light on", "volume 40", "mute",
//!            "brightness 70", "lock", "sleep"
//! A row is `{section, title, subtitle, glyph, run}`; `run` is what the QML
//! closure did, as `{"call":…,"args":[…],"done":{icon,title,subtitle,color}?}`.
//!
//!   intents.rows [query]   answers Spotlight with a line to it:
//!                          `{"to":"spotlight","intents":{"query":…,"rows":[…]}}`
//!   intents.run  [run]     does a row's `run` (its call, then the island note)
//!
//! Parsing is by hand (no regex crate): the same phrases, the same defaults.

use crate::common::*;
use serde_json::{json, Value};
use tessera_core::host::{Host, Service};

fn row(title: &str, subtitle: &str, glyph: &str, call: &str, args: Value, done: Option<(&str, &str, String, &str)>) -> Value {
    let mut run = json!({"call": call, "args": args});
    if let Some((icon, t, sub, color)) = done {
        run["done"] = json!({"icon": icon, "title": t, "subtitle": sub, "color": color});
    }
    json!({"section": "Do", "title": title, "subtitle": subtitle, "glyph": glyph, "run": run})
}

/// The rest of `t` after one of the leads (case-insensitive, then a space or a colon), trimmed
fn after<'a>(t: &'a str, leads: &[&str]) -> Option<&'a str> {
    for lead in leads {
        // (the leads are ASCII, so the first bytes of `t` are exactly them when they match)
        let Some(head) = t.get(..lead.len()) else { continue };
        if !head.eq_ignore_ascii_case(lead) {
            continue;
        }
        let rest = &t[lead.len()..];
        if rest.starts_with(char::is_whitespace) || rest.starts_with(':') {
            let tail = rest.trim_start_matches(':').trim();
            if !tail.is_empty() {
                return Some(tail);
            }
        }
    }
    None
}

/// "10 min", "1h 30m", "90s", "2 hours": seconds (0 when there is no duration).
/// Alarms.parseDuration
pub fn parse_duration(text: &str) -> i64 {
    let low = text.to_lowercase();
    let b = low.as_bytes();
    let mut total = 0.0f64;
    let mut i = 0;
    let mut found = false;
    while i < b.len() {
        if b[i].is_ascii_digit() {
            let start = i;
            while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'.') {
                i += 1;
            }
            let n: f64 = low[start..i].parse().unwrap_or(0.0);
            while i < b.len() && b[i] == b' ' {
                i += 1;
            }
            let ustart = i;
            while i < b.len() && b[i].is_ascii_alphabetic() {
                i += 1;
            }
            let mult = match &low[ustart..i] {
                "h" | "hr" | "hrs" | "hour" | "hours" => 3600.0,
                "m" | "min" | "mins" | "minute" | "minutes" => 60.0,
                "s" | "sec" | "secs" | "second" | "seconds" => 1.0,
                _ => continue,
            };
            total += n * mult;
            found = true;
        } else {
            i += 1;
        }
    }
    if found {
        total.round() as i64
    } else {
        0
    }
}

/// Alarms.describe: "1 h 30 min", "45 s"
pub fn describe(secs: i64) -> String {
    let (h, m, s) = (secs / 3600, (secs % 3600) / 60, secs % 60);
    let mut parts = vec![];
    if h > 0 {
        parts.push(format!("{h} h"));
    }
    if m > 0 {
        parts.push(format!("{m} min"));
    }
    if s > 0 && h == 0 {
        parts.push(format!("{s} s"));
    }
    if parts.is_empty() {
        "0 s".into()
    } else {
        parts.join(" ")
    }
}

/// "7am", "6:30", "19.15 pm" as minutes since midnight
pub fn parse_clock(text: &str) -> Option<i64> {
    let t = text.trim().to_lowercase().replace(' ', "");
    let (num, pm, am) = match (t.strip_suffix("pm"), t.strip_suffix("am")) {
        (Some(n), _) => (n.to_string(), true, false),
        (_, Some(n)) => (n.to_string(), false, true),
        _ => (t.clone(), false, false),
    };
    let mut it = num.split([':', '.']);
    let mut h: i64 = it.next()?.parse().ok()?;
    let m: i64 = it.next().map_or(Some(0), |m| m.parse().ok())?;
    if pm && h < 12 {
        h += 12;
    }
    if am && h == 12 {
        h = 0;
    }
    (h < 24 && m < 60).then_some(h * 60 + m)
}

/// The rows for a phrase
pub fn rows(raw: &str) -> Vec<Value> {
    let t = raw.trim();
    let mut out = Vec::new();
    if t.chars().count() < 2 {
        return out;
    }
    tasks(t, &mut out);
    notes(t, &mut out);
    events(t, &mut out);
    timers(t, &mut out);
    system(t, &mut out);
    out
}

fn tasks(t: &str, out: &mut Vec<Value>) {
    let Some(text) = after(t, &["remind me to", "remind me", "to-do", "todo", "task", "add a task", "add task", "add a to-do", "add a todo", "add to-do", "add todo"]) else { return };
    out.push(row(&format!("Add to-do: {text}"), "Reminders · no due date", "check-circle", "todos.addText", json!([text]), Some(("check-circle", "To-do added", text.to_string(), "#FF9F0A"))));
}

fn notes(t: &str, out: &mut Vec<Value>) {
    if let Some(text) = after(t, &["note", "jot down", "jot", "write down", "new note"]) {
        let title: String = text.lines().next().unwrap_or("").chars().take(40).collect();
        out.push(row(&format!("New note: {title}"), "Saved to your Notes folder", "note-pencil", "notes.create", json!([text, false]), None));
        out.push(row("New note and open it", &title, "note-pencil", "notes.create", json!([text, true]), None));
        return;
    }
    if let Some(rest) = after(t, &["save", "keep", "bookmark"]) {
        let url = rest.split_whitespace().find(|w| w.starts_with("http://") || w.starts_with("https://") || (w.contains('.') && !w.ends_with('.') && !w.starts_with('.') && w.rsplit('.').next().is_some_and(|tld| tld.len() >= 2 && tld.chars().all(|c| c.is_ascii_alphabetic()))));
        if let Some(url) = url {
            let full = if url.starts_with("http") { url.to_string() } else { format!("https://{url}") };
            let words = rest.replace(url, "").trim().to_string();
            out.push(row(&format!("Save link{}", if words.is_empty() { String::new() } else { format!(": {words}") }), &full, "bookmark-simple", "notes.saveLink", json!([full, words]), None));
        } else {
            let title: String = rest.chars().take(40).collect();
            out.push(row(&format!("Save as a note: {title}"), "Saved to your Notes folder", "note-pencil", "notes.create", json!([rest, false]), None));
        }
        return;
    }
    let low = t.to_lowercase();
    if low == "notes" || low == "note" || low == "open notes" || low == "open note" {
        out.push(row("Open the Notes folder", "", "folder-open", "notes.openFolder", json!([]), None));
    }
}

/// The calendar's parser reads "… [today | tomorrow | on <day>] at <time>" at the
/// end; this puts everyday phrasings into that shape: "tomorrow 3pm" ->
/// "tomorrow at 3pm", "at 3pm tomorrow" -> "tomorrow at 3pm", "fri at 9" -> "on fri at 9"
pub fn normalize_when(text: &str) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    let is_day = |w: &str| {
        let l = w.to_lowercase();
        ["today", "tonight", "tomorrow"].contains(&l.as_str()) || ["mon", "tue", "wed", "thu", "fri", "sat", "sun"].iter().any(|d| l.starts_with(d))
    };
    let is_time = |w: &str| {
        let l = w.to_lowercase();
        l.chars().next().is_some_and(|c| c.is_ascii_digit()) && l.trim_end_matches(['a', 'p', 'm']).chars().all(|c| c.is_ascii_digit() || c == ':' || c == '.')
    };
    let n = words.len();
    // "... at <time> <day>"
    if n >= 4 && is_time(words[n - 2]) && is_day(words[n - 1]) && words[n - 3].eq_ignore_ascii_case("at") {
        return format!("{} {} at {}", words[..n - 3].join(" "), words[n - 1], words[n - 2]);
    }
    // "... <day> <time>"
    if n >= 3 && is_day(words[n - 2]) && is_time(words[n - 1]) && !words[n - 2].eq_ignore_ascii_case("at") {
        let day = words[n - 2];
        let prefix = if ["today", "tonight", "tomorrow"].contains(&day.to_lowercase().as_str()) { "" } else { "on " };
        return format!("{} {prefix}{day} at {}", words[..n - 2].join(" "), words[n - 1]);
    }
    // "... <am/pm time>"
    if n >= 2 && is_time(words[n - 1]) && words[n - 1].to_lowercase().ends_with('m') && !words[n - 2].eq_ignore_ascii_case("at") {
        return format!("{} at {}", words[..n - 1].join(" "), words[n - 1]);
    }
    // "fri at 9" -> "on fri at 9"
    let mut out: Vec<String> = words.iter().map(|w| w.to_string()).collect();
    for i in 0..n.saturating_sub(1) {
        let weekday = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"].iter().any(|d| words[i].to_lowercase().starts_with(d));
        if weekday && words[i + 1].eq_ignore_ascii_case("at") && (i == 0 || !words[i - 1].eq_ignore_ascii_case("on")) {
            out[i] = format!("on {}", words[i]);
            break;
        }
    }
    out.join(" ")
}

fn events(t: &str, out: &mut Vec<Value>) {
    let low = t.to_lowercase();
    let lead = ["event", "add an event", "add event", "meeting", "meet", "call", "lunch", "dinner", "breakfast", "appointment", "schedule"].iter().any(|l| low == *l || low.starts_with(&format!("{l} ")));
    let with_when = low.split_whitespace().any(|w| w == "with") && low.split_whitespace().any(|w| ["at", "on", "today", "tomorrow", "tonight"].contains(&w) || ["mon", "tue", "wed", "thu", "fri", "sat", "sun"].iter().any(|d| w.starts_with(d)));
    if !lead && !with_when {
        return;
    }
    let stripped = after(t, &["add an event", "add event", "event", "schedule"]).unwrap_or(t);
    let q = normalize_when(stripped);
    // (the calendar's parser answers; the row is added by it)
    out.push(row(&format!("Add event: {q}"), "In your calendar", "calendar-plus", "calendar.addQuick", json!([q]), Some(("calendar-plus", "Event added", q.clone(), "#FF453A"))));
}

fn timers(t: &str, out: &mut Vec<Value>) {
    let low = t.to_lowercase();
    // "timer 10 min", "set a timer for 5m pasta", "pasta timer 10 min", "10 min timer"
    let timer_text = low
        .strip_prefix("set a timer for ")
        .or_else(|| low.strip_prefix("set timer for "))
        .or_else(|| low.strip_prefix("set a timer "))
        .or_else(|| low.strip_prefix("timer for "))
        .or_else(|| low.strip_prefix("timer "))
        .map(str::to_string)
        .or_else(|| {
            let words: Vec<&str> = low.split_whitespace().collect();
            words.iter().position(|w| *w == "timer").filter(|&i| i > 0 || words.len() > 1).map(|i| words.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, w)| *w).collect::<Vec<_>>().join(" "))
        });
    if let Some(text) = timer_text {
        let secs = parse_duration(&text);
        if secs > 0 {
            // the label: the words that aren't the duration
            let mut label = String::new();
            let mut skip_unit = false;
            for w in text.split_whitespace() {
                let w_digits = w.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect::<String>();
                if w == "for" || skip_unit && parse_duration(&format!("1 {w}")) > 0 {
                    skip_unit = false;
                    continue;
                }
                if !w_digits.is_empty() {
                    skip_unit = w_digits.len() == w.len(); // ("10 min": the unit is the next word)
                    if parse_duration(w) > 0 || w_digits.len() == w.len() {
                        continue;
                    }
                }
                skip_unit = false;
                if !label.is_empty() {
                    label.push(' ');
                }
                label.push_str(w);
            }
            let ends = tessera_core::util::strftime("%H:%M", now_ms() / 1000 + secs);
            out.push(row(&format!("Start a {} timer{}", describe(secs), if label.is_empty() { String::new() } else { format!(": {label}") }), &format!("Ends at {ends}"), "timer", "alarms.addTimer", json!([secs, label]), Some(("timer", "Timer started", format!("{}{}", describe(secs), if label.is_empty() { String::new() } else { format!(" · {label}") }), "#FF9F0A"))));
        }
        return;
    }
    let alarm = ["set an alarm at ", "set an alarm for ", "set an alarm ", "alarm at ", "alarm for ", "alarm ", "wake me up at ", "wake me up for ", "wake me at ", "wake me "].iter().find_map(|p| low.strip_prefix(p));
    if let Some(rest) = alarm {
        let mut words = rest.split_whitespace();
        let when = words.next().unwrap_or("");
        let label = words.collect::<Vec<_>>().join(" ");
        if let Some(min) = parse_clock(when) {
            let now_min = tessera_core::util::now("%H %M").split(' ').map(|x| x.parse::<i64>().unwrap_or(0)).fold(0, |a, x| a * 60 + x);
            let day = if min > now_min { "today" } else { "tomorrow" };
            let clock = format!("{:02}:{:02}", min / 60, min % 60);
            out.push(row(&format!("Set an alarm for {clock} {day}{}", if label.is_empty() { String::new() } else { format!(": {label}") }), "Sounds even with interface sounds off", "alarm", "alarms.addAlarm", json!([min, label]), Some(("alarm", "Alarm set", format!("{clock} {day}"), "#FF9F0A"))));
        }
        return;
    }
    if ["timer", "timers", "alarm", "alarms"].contains(&low.as_str()) {
        out.push(row("Timers and alarms", "Try “timer 10 min” or “alarm 7am”", "timer", "alarms.list", json!([]), None));
    }
}

fn system(t: &str, out: &mut Vec<Value>) {
    let low = t.to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ");
    let onoff = |s: &str, prefixes: &[&str]| -> Option<Option<bool>> {
        for p in prefixes {
            if let Some(r) = s.strip_prefix(p) {
                return match r.trim() {
                    "" => Some(None),
                    "on" => Some(Some(true)),
                    "off" => Some(Some(false)),
                    _ => None,
                };
            }
        }
        None
    };
    // Caffeine
    for p in ["caffeine", "keep awake", "stay awake"] {
        if let Some(r) = low.strip_prefix(p) {
            let r = r.trim();
            if r == "off" {
                out.push(row("Turn Caffeine off", "Sleeps when idle again", "coffee", "system.setKeepAwake", json!([false, 0]), None));
            } else {
                let rest = r.strip_prefix("on").map_or(r, str::trim).trim_start_matches("for ").trim();
                let mins = if rest.is_empty() { 0 } else { (parse_duration(rest) as f64 / 60.0).round() as i64 };
                let title = if mins > 0 { format!("Caffeine for {}", describe(mins * 60)) } else { "Turn Caffeine on".to_string() };
                out.push(row(&title, &format!("The screen stays awake{}", if mins > 0 { "" } else { " until you turn it off" }), "coffee", "system.setKeepAwake", json!([true, mins]), None));
            }
            return;
        }
    }
    for (word, glyph) in [("dark", "moon"), ("light", "sun")] {
        if [word.to_string(), format!("{word} mode"), format!("{word} appearance"), format!("turn on {word} mode"), format!("toggle {word} mode"), format!("switch to {word} mode"), format!("switch to {word}")].contains(&low) {
            out.push(row(&format!("Switch to {word} appearance"), "", glyph, "system.setAppearance", json!([word]), None));
            return;
        }
    }
    for (what, call, glyph_on, glyph_off) in [("wifi", "network.setWifi", "wifi-high", "wifi-slash"), ("wi-fi", "network.setWifi", "wifi-high", "wifi-slash"), ("bluetooth", "devices.setBluetooth", "bluetooth", "bluetooth")] {
        let on = if let Some(Some(v)) = onoff(&low, &[&format!("{what} "), &format!("turn {what} ")]) {
            Some(v)
        } else if low == format!("turn on {what}") {
            Some(true)
        } else if low == format!("turn off {what}") {
            Some(false)
        } else {
            None
        };
        if let Some(on) = on {
            out.push(row(&format!("Turn {} {}", if what == "bluetooth" { "Bluetooth" } else { "Wi-Fi" }, if on { "on" } else { "off" }), "", if on { glyph_on } else { glyph_off }, call, json!([on]), None));
            return;
        }
    }
    for p in ["focus", "dnd", "do not disturb"] {
        if let Some(r) = low.strip_prefix(p) {
            let r = r.trim();
            if r == "off" {
                out.push(row("Turn Focus off", "", "moon", "focus.deactivate", json!([]), None));
            } else {
                let rest = r.strip_prefix("on").map_or(r, str::trim).trim_start_matches("for ").trim();
                let secs = if rest.is_empty() { 0 } else { parse_duration(rest) };
                out.push(row(&format!("Do Not Disturb{}", if secs > 0 { format!(" for {}", describe(secs)) } else { String::new() }), if secs > 0 { "For a while" } else { "Until you turn it off" }, "moon", "focus.activate", json!(["dnd", secs * 1000, false]), None));
            }
            return;
        }
    }
    if let Some(r) = onoff(&low, &["night light", "nightlight"]) {
        let state = r.map(|v| json!(v)).unwrap_or(Value::Null);
        out.push(row(&format!("Turn Night Light {}", match r { Some(true) => "on", Some(false) => "off", None => "on or off" }), "", "sun-dim", "system.setNightLight", json!([state]), None));
        return;
    }
    for (p, call, what, glyph) in [("volume ", "audio.setVolume", "volume", "speaker-high"), ("set volume ", "audio.setVolume", "volume", "speaker-high"), ("brightness ", "brightness.set", "brightness", "sun"), ("set brightness ", "brightness.set", "brightness", "sun")] {
        if let Some(r) = low.strip_prefix(p) {
            let digits = r.trim_start_matches("to ").trim().trim_end_matches('%');
            if let Ok(v) = digits.parse::<i64>() {
                let v = v.clamp(0, 100);
                out.push(row(&format!("Set the {what} to {v}%"), "", glyph, call, json!([v as f64 / 100.0]), None));
                return;
            }
        }
    }
    if low == "mute" || low == "unmute" {
        out.push(row(if low == "mute" { "Mute" } else { "Unmute" }, "", if low == "mute" { "speaker-slash" } else { "speaker-high" }, "audio.setMuted", json!([low == "mute"]), None));
        return;
    }
    let power = match low.as_str() {
        "lock" | "lock screen" => Some(("lock", "Lock the screen", "lock")),
        "sleep" => Some(("sleep", "Sleep", "moon-stars")),
        "log out" => Some(("logout", "Log out", "sign-out")),
        "restart" => Some(("restart", "Restart", "arrow-clockwise")),
        "shut down" | "shutdown" => Some(("shutdown", "Shut down", "power")),
        _ => None,
    };
    if let Some((what, title, glyph)) = power {
        out.push(row(title, "", glyph, "system.power", json!([what]), None));
    }
}

#[derive(Default)]
pub struct Intents;

impl Service for Intents {
    fn name(&self) -> &'static str {
        "Intents"
    }
    fn calls(&self) -> &'static [&'static str] {
        &["intents"]
    }
    fn call(&mut self, h: &mut Host, call: &str, args: &Value) {
        match call {
            "intents.rows" => {
                let q = args[0].as_str().unwrap_or("");
                h.tell("spotlight", json!({"intents": {"query": q, "rows": rows(q)}}));
            }
            "intents.run" => {
                let run = &args[0];
                if let Some(c) = run["call"].as_str() {
                    h.ask(c, run["args"].clone());
                }
                let d = &run["done"];
                if d.is_object() {
                    h.ask("island.post", json!([{"kind": "custom", "icon": d["icon"], "title": d["title"], "subtitle": d["subtitle"], "color": d["color"]}, 3000]));
                }
            }
            other => eprintln!("tessera-ui: host: no {other}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn titles(q: &str) -> Vec<String> {
        rows(q).iter().map(|r| r["title"].as_str().unwrap().to_string()).collect()
    }

    #[test]
    fn durations_read_like_the_alarms() {
        assert_eq!(parse_duration("10 min"), 600);
        assert_eq!(parse_duration("1h 30m"), 5400);
        assert_eq!(parse_duration("90s"), 90);
        assert_eq!(parse_duration("2 hours"), 7200);
        assert_eq!(parse_duration("soon"), 0);
        assert_eq!(describe(5400), "1 h 30 min");
        assert_eq!(describe(45), "45 s");
    }

    #[test]
    fn clocks() {
        assert_eq!(parse_clock("7am"), Some(420));
        assert_eq!(parse_clock("6:30"), Some(390));
        assert_eq!(parse_clock("12am"), Some(0));
        assert_eq!(parse_clock("7:30 pm"), Some(19 * 60 + 30));
        assert_eq!(parse_clock("25"), None);
    }

    #[test]
    fn tasks_notes_timers_and_system_rows() {
        assert_eq!(titles("remind me to call mum at 6"), vec!["Add to-do: call mum at 6"]);
        assert_eq!(titles("todo buy milk")[0], "Add to-do: buy milk");
        assert_eq!(titles("note gate code is 4512")[0], "New note: gate code is 4512");
        assert_eq!(titles("timer 10 min")[0], "Start a 10 min timer");
        assert_eq!(titles("set a timer for 5m pasta")[0], "Start a 5 min timer: pasta");
        assert!(titles("alarm 7am")[0].starts_with("Set an alarm for 07:00"));
        assert_eq!(titles("dark mode"), vec!["Switch to dark appearance"]);
        assert_eq!(titles("wifi off"), vec!["Turn Wi-Fi off"]);
        assert_eq!(titles("volume 40"), vec!["Set the volume to 40%"]);
        assert_eq!(titles("caffeine 1h"), vec!["Caffeine for 1 h"]);
        assert_eq!(titles("lock"), vec!["Lock the screen"]);
        assert!(titles("hello there").is_empty());
        assert!(titles("a").is_empty());
    }

    #[test]
    fn save_a_link() {
        let r = rows("save https://example.org/pie nice recipe");
        assert_eq!(r[0]["title"], "Save link: nice recipe");
        assert_eq!(r[0]["subtitle"], "https://example.org/pie");
    }

    #[test]
    fn when_is_put_where_the_calendar_reads_it() {
        assert_eq!(normalize_when("meeting with Sam tomorrow 3pm"), "meeting with Sam tomorrow at 3pm");
        assert_eq!(normalize_when("lunch at 1pm tomorrow"), "lunch tomorrow at 1pm");
        assert_eq!(normalize_when("dentist fri at 9"), "dentist on fri at 9");
        assert_eq!(normalize_when("dentist fri 9am"), "dentist on fri at 9am");
    }
}

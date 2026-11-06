//! Calendar.qml, the part that isn't the reader (core/src/services/calendar.rs
//! reads your events and the calendar files and expands the repeats): the
//! queries Notification Center and Today make (events on a day, "Every day"
//! series), your own events' writes (events.json), the alerts that arrive as
//! notifications (every 30 s: once, within ten minutes of the alert time),
//! and the "calendar" ipc target (`qs ipc call calendar remind "call mum at
//! 5pm"` | `refresh`).
//!
//! Differences: the accounts' calendars (Google, iCloud, Microsoft 365) are
//! the system host's, and the reader has no VALARM, location or notes for
//! subscriptions, so their alerts use the default alert (10 min; none for
//! all-day) and a notification has no location line.

use super::common::*;
use super::parse;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashSet};
use tessera_core::host::Host;
use tessera_core::services::calendar::{self as cal, Calendar, Event};

pub const DAY: i64 = 86_400_000;

#[derive(Default)]
pub struct Cal {
    /// alert keys already shown this session
    pub alerted: HashSet<String>,
}

pub fn events_path() -> std::path::PathBuf {
    tessera_core::util::state_dir().join("events.json")
}

pub fn sources_path() -> std::path::PathBuf {
    tessera_core::theme::config_path().parent().map(|p| p.join("calendars.json")).unwrap_or_default()
}

/// events.json as it is (raw: id, title, start, end, allDay, alert, notes)
pub fn local_raw() -> Vec<Value> {
    std::fs::read_to_string(events_path()).ok().and_then(|s| serde_json::from_str::<Value>(&s).ok()).and_then(|v| v.as_array().cloned()).unwrap_or_default()
}

fn save_local(l: &[Value]) {
    // Owner-only: events can be private
    use std::os::unix::fs::PermissionsExt;
    let p = events_path();
    let _ = tessera_core::util::write_atomic(&p, serde_json::to_string_pretty(l).unwrap_or_default().as_bytes());
    let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600));
}

/// Calendar.defaultAlert (calendars.json's, 10 minutes unless it says)
pub fn default_alert() -> i64 {
    std::fs::read_to_string(sources_path())
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .and_then(|v| v["defaultAlert"].as_i64())
        .unwrap_or(10)
}

/// calendars.json whole (sources and the default alert), as the QML wrote it
fn sources_doc() -> Value {
    let mut v: Value = std::fs::read_to_string(sources_path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_else(|| json!({}));
    if !v.is_object() {
        v = json!({});
    }
    if !v["sources"].is_array() {
        v["sources"] = json!([]);
    }
    v
}

/// Calendar.saveSources: owner-only (links can carry secret tokens); the reader notices the file
fn save_sources(v: &Value) {
    use std::os::unix::fs::PermissionsExt;
    let p = sources_path();
    let _ = tessera_core::util::write_atomic(&p, serde_json::to_string_pretty(v).unwrap_or_default().as_bytes());
    let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600));
}

/// The calls Settings -> Calendar makes (host-system's accounts bridge):
/// `calendar.addSource [name, url]`, `removeSource [url]`, `setSource [url, patch]`,
/// `setDefaultAlert [minutes]`
pub fn call(call: &str, a: &Value) {
    let mut doc = sources_doc();
    match call.strip_prefix("calendar.").unwrap_or("") {
        "addSource" => {
            const COLORS: [&str; 6] = ["#6D7CDF", "#D05F43", "#409D48", "#BB5FAA", "#BE7100", "#1D9999"];
            let n = doc["sources"].as_array().map_or(0, Vec::len);
            let name = a[0].as_str().filter(|n| !n.is_empty()).unwrap_or("Calendar");
            doc["sources"].as_array_mut().unwrap().push(json!({"name": name, "url": a[1].as_str().unwrap_or("").trim(), "color": COLORS[n % COLORS.len()], "enabled": true}));
        }
        "removeSource" => doc["sources"].as_array_mut().unwrap().retain(|s| s["url"] != a[0]),
        "setSource" => {
            for s in doc["sources"].as_array_mut().unwrap().iter_mut().filter(|s| s["url"] == a[0]) {
                if let (Some(o), Some(p)) = (s.as_object_mut(), a[1].as_object()) {
                    for (k, v) in p {
                        o.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        "setDefaultAlert" => doc["defaultAlert"] = json!(a[0].as_i64().or_else(|| a[0].as_f64().map(|f| f as i64)).unwrap_or(10)),
        _ => return,
    }
    save_sources(&doc);
}

/// Calendar.add: your own event (returns it)
pub fn add(title: &str, start: i64, end: i64, all_day: bool, alert: Option<i64>, notes: &str) -> Value {
    let e = json!({"id": new_id(), "title": title, "start": start, "end": end, "allDay": all_day, "alert": alert.unwrap_or_else(default_alert), "notes": notes});
    let mut l = local_raw();
    l.push(e.clone());
    save_local(&l);
    e
}

pub fn update(id: &str, patch: &Value) {
    let mut l = local_raw();
    for e in l.iter_mut().filter(|e| e["id"].as_str() == Some(id)) {
        if let Value::Object(p) = patch {
            for (k, v) in p {
                e[k] = v.clone();
            }
        }
    }
    save_local(&l);
}

/// Calendar.remove: the event, for Undo
pub fn remove(id: &str) -> Option<Value> {
    let l = local_raw();
    let e = l.iter().find(|e| e["id"].as_str() == Some(id)).cloned();
    save_local(&l.into_iter().filter(|e| e["id"].as_str() != Some(id)).collect::<Vec<_>>());
    e
}

pub fn restore(e: &Value) {
    let mut l = local_raw();
    if !l.iter().any(|x| x["id"] == e["id"]) {
        l.push(e.clone());
        save_local(&l);
    }
}

/// "ddd h:mm AP"
fn when_text(ms: i64) -> String {
    tessera_core::util::strftime("%a %-I:%M %p", ms.div_euclid(1000))
}

/// IPC `calendar remind <text>`
fn remind(text: &str) -> String {
    match parse::parse_quick(text, now()) {
        None => "Couldn't read a time. Try “call mum at 5pm” or “stretch in 20 minutes”.".to_string(),
        Some((title, start)) => {
            add(&title, start, start, false, Some(0), "");
            format!("{} — {}", title, when_text(start))
        }
    }
}

pub fn ipc(func: &str, args: &[String]) -> Option<String> {
    match func {
        "remind" => Some(remind(&args.join(" "))),
        // (the reader watches the files itself, every 2 s)
        "refresh" => None,
        _ => None,
    }
}

// ── Queries (for Notification Center and Today) ─────────────────────────

/// Calendar.eventsOn(day)
pub fn events_on(c: &Calendar, day: i64) -> Vec<Event> {
    let (a, b) = cal::day_bounds(day);
    c.between(a, b)
}

/// Calendar.timeText
pub fn time_text(e: &Event) -> String {
    if e.all_day {
        return "All day".into();
    }
    format!("{}{}", hm(e.start), if e.end > e.start { format!(" – {}", hm(e.end)) } else { String::new() })
}

/// The calendar an event belongs to ("Reminders" for your own)
pub fn name_of(c: &Calendar, e: &Event) -> String {
    if e.key.starts_with("local:") {
        return "Reminders".into();
    }
    let url = e.key.rsplitn(3, '|').nth(2).unwrap_or("");
    c.sources.iter().find(|s| s.url == url).map(|s| s.name.clone()).unwrap_or_default()
}

/// Calendar.seriesOf: the event's series (its key without the instance)
pub fn series_of(e: &Event) -> String {
    e.key.rfind('|').map(|i| e.key[..i].to_string()).unwrap_or_default()
}

/// Calendar.dailyIn: series on at least 80% of the days (a week or more)
/// -> title (listed once as "Every day" instead of dotting each day)
pub fn daily_in(list: &[Event], from: i64, to: i64) -> BTreeMap<String, String> {
    let days = ((to - from) as f64 / DAY as f64).round() as i64;
    let mut out = BTreeMap::new();
    if days < 7 {
        return out;
    }
    let mut seen: BTreeMap<String, (String, HashSet<i64>)> = BTreeMap::new();
    for e in list {
        let s = series_of(e);
        if s.is_empty() {
            continue;
        }
        seen.entry(s).or_insert_with(|| (e.title.clone(), HashSet::new())).1.insert(cal::day_bounds(e.start).0);
    }
    for (s, (title, d)) in seen {
        if d.len() as f64 >= days as f64 * 0.8 {
            out.insert(s, title);
        }
    }
    out
}

// ── Alerts ──────────────────────────────────────────────────────────────

impl Cal {
    pub fn check_alerts(&mut self, h: &mut Host) {
        let now = now();
        let Some(c) = h.st.shell.get::<Calendar>().cloned() else { return };
        let def = default_alert();
        let raw = local_raw();
        for e in c.between(now - 3_600_000, now + 24 * 3_600_000) {
            let local = e.key.starts_with("local:");
            let alert = if local {
                let id = &e.key["local:".len()..];
                raw.iter().find(|r| r["id"].as_str() == Some(id)).and_then(|r| r["alert"].as_i64()).unwrap_or(def)
            } else if e.all_day {
                -1
            } else {
                def
            };
            if alert < 0 || self.alerted.contains(&e.key) {
                continue;
            }
            let at = e.start - alert * 60_000;
            // Fire once, within ten minutes of the alert time (not for alerts long past)
            if now >= at && now - at < 600_000 {
                self.alerted.insert(e.key.clone());
                let when = if alert == 0 || (local && e.start <= now) {
                    "Now".to_string()
                } else if e.all_day {
                    "Today".to_string()
                } else {
                    format!("At {}{}", hm(e.start), if alert > 0 { format!(" · in {alert} min") } else { String::new() })
                };
                notify_send("Calendar", Some("x-office-calendar"), Some("normal"), &e.title, &when);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(key: &str, start: i64) -> Event {
        Event { key: key.into(), title: "Pill".into(), start, end: start + 1, all_day: false, color: String::new() }
    }

    #[test]
    fn a_daily_series_is_listed_once() {
        let from = cal::make(2026, 10, 1, 0, 0, 0);
        let list: Vec<Event> = (0..7).map(|d| ev("u|uid|1", cal::make(2026, 10, 1 + d, 9, 0, 0))).collect();
        let m = daily_in(&list, from, from + 7 * DAY);
        assert_eq!(m.len(), 1);
        assert!(daily_in(&list[..3], from, from + 3 * DAY).is_empty());
        assert_eq!(series_of(&ev("u|uid|99", 0)), "u|uid");
    }

    #[test]
    fn the_remind_answer_names_the_thing() {
        assert!(remind("???").starts_with("Couldn't read a time"));
    }
}

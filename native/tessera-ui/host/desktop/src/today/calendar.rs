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
use std::collections::HashSet;
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

// ── Notification Center's editor (CalendarWidget.qml's editor, moveTo, remove) ──

/// "y-mm-dd" -> (y, month 0-11, d)
fn ymd(day: &str) -> Option<(i32, i32, i32)> {
    let mut p = day.split('-').map(|x| x.parse::<i32>());
    Some((p.next()?.ok()?, p.next()?.ok()? - 1, p.next()?.ok()?))
}

/// editor.save: `{key, todo, day, title, allDay, start, end (minutes; a
/// to-do's start -1 for no time), alert, notes, location, target}`; your own
/// events and to-dos (the accounts' are the system host's: an error says so)
pub fn nc_save(h: &mut Host, e: &Value) -> Result<(), String> {
    let (y, mo, d) = ymd(e["day"].as_str().unwrap_or("")).ok_or("That date isn't valid.")?;
    let title = e["title"].as_str().unwrap_or("").trim().to_string();
    let key = e["key"].as_str().unwrap_or("");
    let n = |k: &str| e[k].as_i64().unwrap_or(-1) as i32;
    if e["todo"].as_bool().unwrap_or(false) {
        let t = n("start");
        let due = if t < 0 { cal::make(y, mo, d, 23, 59, 0) } else { cal::make(y, mo, d, 0, t, 0) };
        let patch = json!({"text": title, "due": due, "allDay": t < 0});
        let saved = if key.is_empty() {
            let mut item = patch;
            item["done"] = json!(false);
            super::todos::add_item(h, item)
        } else {
            super::todos::update(key, &patch)
        };
        return if saved { Ok(()) } else { Err("Could not save the task.".into()) };
    }
    let all_day = e["allDay"].as_bool().unwrap_or(false);
    let (start, end) = if all_day { (cal::make(y, mo, d, 0, 0, 0), cal::make(y, mo, d + 1, 0, 0, 0)) } else { (cal::make(y, mo, d, 0, n("start"), 0), cal::make(y, mo, d, 0, n("end").max(n("start")), 0)) };
    let alert = if all_day { -1 } else { e["alert"].as_i64().unwrap_or_else(default_alert) };
    let notes = e["notes"].as_str().unwrap_or("");
    if !key.is_empty() {
        let id = key.strip_prefix("local:").ok_or("This calendar is read-only here. Change the event in its own calendar.")?;
        update(id, &json!({"title": title, "start": start, "end": end, "allDay": all_day, "alert": alert, "notes": notes}));
        return Ok(());
    }
    if !e["target"].as_str().unwrap_or("").is_empty() {
        return Err("That calendar isn't available. Choose another one.".into());
    }
    add(&title, start, end, all_day, Some(alert), notes);
    if cfg_get("google.newEventCalendar").as_str().is_some_and(|c| !c.is_empty()) {
        cfg_set("google.newEventCalendar", json!(""));
    }
    Ok(())
}

/// remove: your event or to-do, kept for Undo (`{kind, item, title}`)
pub fn nc_remove(key: &str) -> Result<Value, String> {
    if let Some(id) = key.strip_prefix("local:") {
        let e = remove(id).ok_or("That event is gone already.")?;
        let title = e["title"].clone();
        return Ok(json!({"kind": "event", "item": e, "title": title}));
    }
    if key.starts_with("local/") {
        let t = super::todos::remove(key).ok_or("That to-do is gone already.")?;
        let title = t["text"].clone();
        return Ok(json!({"kind": "todo", "item": t, "title": title}));
    }
    Err("This calendar is read-only here.".into())
}

/// Undo
pub fn nc_restore(u: &Value) {
    if u["kind"] == "event" {
        restore(&u["item"]);
    } else {
        super::todos::restore(&u["item"]);
    }
}

/// moveTo: to another day, keeping its time
pub fn nc_move(key: &str, day: &str) -> Result<(), String> {
    let (y, mo, d) = ymd(day).ok_or("That date isn't valid.")?;
    if let Some(id) = key.strip_prefix("local:") {
        let src = local_raw().into_iter().find(|e| e["id"].as_str() == Some(id)).ok_or("That event is gone.")?;
        let num = |v: &Value| v.as_f64().map_or(0, |f| f as i64);
        let (s0, e0) = (num(&src["start"]), num(&src["end"]));
        let t = cal::local(s0);
        let shift = cal::make(y, mo, d, t.h, t.mi, 0) - s0;
        update(id, &json!({"start": s0 + shift, "end": (if e0 != 0 { e0 } else { s0 }) + shift}));
        return Ok(());
    }
    if key.starts_with("local/") {
        let t = super::todos::find(key).ok_or("That to-do is gone.")?;
        let at = cal::local(t.due);
        super::todos::update(key, &json!({"due": cal::make(y, mo, d, at.h, at.mi, 0)}));
        return Ok(());
    }
    Err("This calendar is read-only here.".into())
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

// (the calendar an event belongs to, its series and the "Every day" ones:
// core's, shared with the lock's Bedside calendar)
pub use cal::{daily_in, name_of, series_of};

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
    fn days_read() {
        assert_eq!(ymd("2026-11-07"), Some((2026, 10, 7)));
        assert_eq!(ymd("nope"), None);
        assert!(nc_remove("u|uid|1").is_err());
    }

    #[test]
    fn the_remind_answer_names_the_thing() {
        assert!(remind("???").starts_with("Couldn't read a time"));
    }
}

//! Calendar.qml, the part that isn't the reader (core/src/services/calendar.rs
//! reads your events and the calendar files and expands the repeats): the
//! queries Notification Center and Today make (events on a day, "Every day"
//! series), your own events' writes (events.json), the alerts that arrive as
//! notifications (every 30 s: once, within ten minutes of the alert time),
//! and the "calendar" ipc target (`qs ipc call calendar remind "call mum at
//! 5pm"` | `refresh`).
//!
//! The accounts' events (Google, iCloud, Microsoft 365) are in the reader's
//! `between` too (core services/accounts.rs; the system host holds the
//! sign-in). Changing one here asks the system host's service
//! (`google.updateEvent|addEvent|removeEvent|restoreEvent`,
//! `cloudaccounts.…`), which shows the change at once and sends it (Google
//! keeps an outbox while offline, CloudAccounts too); what it can't do comes
//! back as a notification that says what to do.
//!
//! Differences: an account's timed event alerts with its own alarm (iCloud's
//! VALARM) or else the default alert, where the QML gave Google's and
//! Microsoft's none (Google and Outlook ring on the phone; this computer had
//! nothing). Subscriptions have no notes in the reader.

use super::common::*;
use super::parse;
use serde_json::{json, Value};
use std::collections::HashSet;
use tessera_core::host::Host;
use tessera_core::services::accounts::{self, AccEvent, Times};
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

/// What an account's event the editor can't change says (CalendarWidget's info line)
pub fn read_only_text(e: &AccEvent) -> String {
    if e.provider != "google" && e.recurring {
        format!("From {} ({}). Change repeating events on your phone or on the web.", e.calendar, e.account)
    } else {
        format!("From {} ({}). You can view this calendar but not change it.", e.calendar, e.account)
    }
}

/// Calendar.change for an account's event: the system host's service shows
/// it at once and sends it (`every`: a repeating Google event's whole series)
fn ask_update(h: &mut Host, e: &AccEvent, fields: Value, every: bool) {
    if e.provider == "google" {
        h.ask("google.updateEvent", json!([e.source, fields, every]));
    } else {
        h.ask("cloudaccounts.updateEvent", json!([{"source": e.source, "recurring": e.recurring}, fields]));
    }
}

/// When an account's event is: (start, end, all day)
fn times_of(e: &AccEvent) -> (i64, i64, bool) {
    match &e.times {
        Times::Fixed { start, end, all_day } => (*start, *end, *all_day),
        Times::Ics(l) => l.first().map_or((0, 0, false), |x| (x.start, x.end, x.all_day)),
    }
}

/// editor.save: `{key, todo, day, title, allDay, start, end (minutes; a
/// to-do's start -1 for no time), alert, notes, location, target, every}`:
/// your events and to-dos here, the accounts' through the system host
pub fn nc_save(h: &mut Host, e: &Value) -> Result<(), String> {
    let (y, mo, d) = ymd(e["day"].as_str().unwrap_or("")).ok_or("That date isn't valid.")?;
    let title = e["title"].as_str().unwrap_or("").trim().to_string();
    let key = e["key"].as_str().unwrap_or("");
    let n = |k: &str| e[k].as_i64().unwrap_or(-1) as i32;
    if e["todo"].as_bool().unwrap_or(false) {
        let t = n("start");
        let due = if t < 0 { cal::make(y, mo, d, 23, 59, 0) } else { cal::make(y, mo, d, 0, t, 0) };
        let patch = json!({"text": title, "due": due, "allDay": t < 0});
        if key.is_empty() {
            let mut item = patch;
            item["done"] = json!(false);
            return super::todos::add_item(h, item);
        }
        return super::todos::update(h, key, &patch);
    }
    let all_day = e["allDay"].as_bool().unwrap_or(false);
    let (start, end) = if all_day { (cal::make(y, mo, d, 0, 0, 0), cal::make(y, mo, d + 1, 0, 0, 0)) } else { (cal::make(y, mo, d, 0, n("start"), 0), cal::make(y, mo, d, 0, n("end").max(n("start")), 0)) };
    let alert = if all_day { -1 } else { e["alert"].as_i64().unwrap_or_else(default_alert) };
    let notes = e["notes"].as_str().unwrap_or("");
    let location = e["location"].as_str().unwrap_or("");
    // (the accounts keep their own alerts)
    let fields = json!({"title": title, "start": start, "end": end, "allDay": all_day, "location": location, "notes": notes});
    if !key.is_empty() {
        if accounts::is_account_key(key) {
            let data = accounts::current();
            let ev = data.event(key).ok_or("That event is gone. It may have been deleted on another device.")?;
            if !ev.writable {
                return Err(read_only_text(ev));
            }
            ask_update(h, ev, fields, e["every"].as_bool().unwrap_or(false));
            return Ok(());
        }
        let id = key.strip_prefix("local:").ok_or("This calendar is read-only here. Change the event in its own calendar.")?;
        update(id, &json!({"title": title, "start": start, "end": end, "allDay": all_day, "alert": alert, "notes": notes}));
        return Ok(());
    }
    let target = e["target"].as_str().unwrap_or("");
    if !target.is_empty() {
        let data = accounts::current();
        if data.target(target).is_none() {
            return Err("That calendar isn't available any more. Choose another one, or This computer.".into());
        }
        match target.strip_prefix("cloud|").and_then(|r| r.split_once('|')) {
            Some((account, calendar)) => h.ask("cloudaccounts.addEvent", json!([account, calendar, fields])),
            None => h.ask("google.addEvent", json!([target, fields])),
        }
        // (the next new event goes there too, as the QML remembered it)
        cfg_set("google.newEventCalendar", json!(target));
        return Ok(());
    }
    add(&title, start, end, all_day, Some(alert), notes);
    if cfg_get("google.newEventCalendar").as_str().is_some_and(|c| !c.is_empty()) {
        cfg_set("google.newEventCalendar", json!(""));
    }
    Ok(())
}

/// remove: an event or a to-do, kept for Undo (`{kind, item, title}`); an
/// account's is hidden now and deleted when its Undo has run out (the
/// system host's 8 s)
pub fn nc_remove(h: &mut Host, key: &str, every: bool) -> Result<Value, String> {
    if let Some(id) = key.strip_prefix("local:") {
        let e = remove(id).ok_or("That event is gone already.")?;
        let title = e["title"].clone();
        return Ok(json!({"kind": "event", "item": e, "title": title}));
    }
    if key.starts_with("local/") || key.starts_with("google/") || key.starts_with("cloud/") {
        let t = super::todos::remove(h, key).ok_or("That to-do is gone already.")?;
        let title = t["text"].clone();
        return Ok(json!({"kind": "todo", "item": t, "title": title}));
    }
    if accounts::is_account_key(key) {
        let data = accounts::current();
        let ev = data.event(key).ok_or("That event is gone already.")?;
        if !ev.writable {
            return Err(read_only_text(ev));
        }
        if ev.provider == "google" {
            h.ask("google.removeEvent", json!([ev.source, every]));
        } else {
            h.ask("cloudaccounts.removeEvent", json!([{"source": ev.source, "recurring": ev.recurring}]));
        }
        return Ok(json!({"kind": "accountEvent", "provider": ev.provider, "item": ev.source, "title": ev.title}));
    }
    Err("This calendar is read-only here.".into())
}

/// Undo
pub fn nc_restore(h: &mut Host, u: &Value) {
    match u["kind"].as_str().unwrap_or("") {
        "event" => restore(&u["item"]),
        "accountEvent" if u["provider"] == "google" => h.ask("google.restoreEvent", json!([u["item"]])),
        "accountEvent" => h.ask("cloudaccounts.restoreEvent", json!([{"source": u["item"]}])),
        _ => super::todos::restore(h, &u["item"]),
    }
}

/// moveTo: to another day, keeping its time (`every`: a repeating Google
/// event's whole series)
pub fn nc_move(h: &mut Host, key: &str, day: &str, every: bool) -> Result<(), String> {
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
    if key.starts_with("local/") || key.starts_with("google/") || key.starts_with("cloud/") {
        let t = super::todos::find(key).ok_or("That to-do is gone.")?;
        let at = cal::local(t.due);
        return super::todos::update(h, key, &json!({"due": cal::make(y, mo, d, at.h, at.mi, 0), "allDay": t.all_day}));
    }
    if accounts::is_account_key(key) {
        let data = accounts::current();
        let ev = data.event(key).ok_or("That event is gone.")?;
        if !ev.writable {
            return Err(read_only_text(ev));
        }
        // (this occurrence's start: its key's for an iCloud one)
        let (mut s0, mut e0, all_day) = times_of(ev);
        if let Some(at) = key.rsplit_once('|').and_then(|(_, s)| s.parse::<i64>().ok()) {
            (s0, e0) = (at, at + (e0 - s0));
        }
        let t = cal::local(s0);
        let start = cal::make(y, mo, d, t.h, t.mi, 0);
        ask_update(h, ev, json!({"start": start, "end": start + (e0 - s0).max(0), "allDay": all_day}), every);
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

/// The minutes before `e` its alert comes (-1 none): your event's own, an
/// alarm the calendar has (iCloud's, a subscription's), else the default for
/// a timed event
pub fn alert_of(e: &Event, raw: &[Value], def: i64) -> i64 {
    if let Some(id) = e.key.strip_prefix("local:") {
        return raw.iter().find(|r| r["id"].as_str() == Some(id)).and_then(|r| r["alert"].as_i64()).unwrap_or(def);
    }
    match e.alert {
        Some(a) => a,
        None if e.all_day => -1,
        None => def,
    }
}

/// The notification's line: "Now", "Today", "At 9:30 AM · in 10 min", and
/// the place under it
pub fn alert_text(e: &Event, alert: i64, now: i64) -> String {
    let local = e.key.starts_with("local:");
    let when = if alert == 0 || (local && e.start <= now) {
        "Now".to_string()
    } else if e.all_day {
        "Today".to_string()
    } else {
        format!("At {}{}", hm(e.start), if alert > 0 { format!(" · in {alert} min") } else { String::new() })
    };
    if e.location.is_empty() { when } else { format!("{when}\n{}", e.location) }
}

impl Cal {
    pub fn check_alerts(&mut self, h: &mut Host) {
        let now = now();
        let Some(c) = h.st.shell.get::<Calendar>().cloned() else { return };
        let def = default_alert();
        let raw = local_raw();
        for e in c.between(now - 3_600_000, now + 24 * 3_600_000) {
            let alert = alert_of(&e, &raw, def);
            if alert < 0 || self.alerted.contains(&e.key) {
                continue;
            }
            let at = e.start - alert * 60_000;
            // Fire once, within ten minutes of the alert time (not for alerts long past)
            if now >= at && now - at < 600_000 {
                self.alerted.insert(e.key.clone());
                notify_send("Calendar", Some("x-office-calendar"), Some("normal"), &e.title, &alert_text(&e, alert, now));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(key: &str, start: i64) -> Event {
        Event { key: key.into(), title: "Pill".into(), start, end: start + 1, ..Default::default() }
    }

    #[test]
    fn a_daily_series_is_listed_once() {
        let from = cal::make(2026, 10, 1, 0, 0, 0);
        let list: Vec<Event> = (0..7).map(|d| ev("u|uid|1", cal::make(2026, 10, 1 + d, 9, 0, 0))).collect();
        let m = daily_in(&list, from, from + 7 * DAY);
        assert_eq!(m.len(), 1);
        assert!(daily_in(&list[..3], from, from + 3 * DAY).is_empty());
        assert_eq!(series_of(&ev("u|uid|99", 0)), "u|uid");
        // (an account's own series; a single event has none)
        let g = Event { account: "jane@gmail.com".into(), series: "google:s1".into(), ..ev("google:a:c:e", 0) };
        assert_eq!(series_of(&g), "google:s1");
        assert_eq!(series_of(&Event { series: String::new(), ..g }), "");
    }

    #[test]
    fn days_read() {
        assert_eq!(ymd("2026-11-07"), Some((2026, 10, 7)));
        assert_eq!(ymd("nope"), None);
    }

    #[test]
    fn account_events_alert_with_their_own_alarm_or_the_default() {
        let raw = vec![json!({"id": "a", "alert": 30})];
        assert_eq!(alert_of(&ev("local:a", 0), &raw, 10), 30);
        let icloud = Event { alert: Some(15), location: "Park".into(), ..ev("cloud:i:/h.ics|0", cal::make(2026, 10, 7, 9, 0, 0)) };
        assert_eq!(alert_of(&icloud, &raw, 10), 15);
        assert_eq!(alert_of(&ev("google:a:c:e", 0), &raw, 10), 10);
        assert_eq!(alert_of(&Event { all_day: true, ..ev("google:a:c:h", 0) }, &raw, 10), -1);
        let text = alert_text(&icloud, 15, icloud.start - 15 * 60_000);
        assert!(text.starts_with("At ") && text.contains("in 15 min") && text.ends_with("\nPark"), "{text}");
    }

    #[test]
    fn read_only_account_events_say_where_to_change_them() {
        let e = AccEvent {
            key: "cloud:i:/h.ics".into(),
            provider: "icloud".into(),
            account: "iCloud".into(),
            calendar: "Home".into(),
            color: String::new(),
            title: "Run".into(),
            location: String::new(),
            notes: String::new(),
            writable: false,
            recurring: true,
            series: String::new(),
            times: Times::Fixed { start: 5, end: 9, all_day: false },
            source: json!({}),
        };
        assert_eq!(read_only_text(&e), "From Home (iCloud). Change repeating events on your phone or on the web.");
        assert!(read_only_text(&AccEvent { provider: "google".into(), ..e.clone() }).ends_with("You can view this calendar but not change it."));
        assert_eq!(times_of(&e), (5, 9, false));
    }

    #[test]
    fn the_remind_answer_names_the_thing() {
        assert!(remind("???").starts_with("Couldn't read a time"));
    }
}
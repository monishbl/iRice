//! The accounts' calendars and tasks (Google Calendar and Tasks; iCloud
//! calendars and Reminders; Outlook calendars and Microsoft To Do), as the
//! QML's Calendar.between and Todos.list merged them in (Google.events,
//! CloudAccounts.between, Google.tasks, CloudAccounts.tasks).
//!
//! The system host's Google and CloudAccounts services hold the sign-in and
//! publish what their helpers last read (with the changes still on their way
//! applied: `overrides`). Its mirror turns that into `Data` (`from_bus`) and
//! calls `set` whenever it changed. `Data` then rides along in every
//! `calendar::Calendar` (its `accounts`; `Calendar::between` includes them) and
//! every `todos::Todos` (the tasks after yours), so Today, the calendar and
//! Reminders widgets, Notification Center, the lock's calendar and Search all
//! see them without asking. Writes go back through the system host
//! (`google.*`, `cloudaccounts.*` calls: host/desktop's today/todos.rs and
//! calendar.rs make them). No token or password passes through here.
//!
//! Each `Calendar`/`Todos` is made of two halves: the files' (the watcher
//! threads in calendar.rs and todos.rs) and the accounts'. This keeps the
//! last of each and sends them together whenever either changes.

use super::calendar::{self as cal, Calendar, Event, IcsEvent};
use super::todos::{self, Todo, Todos};
use crate::Msg;
use serde_json::Value;
use smithay_client_toolkit::reexports::calloop::channel::Sender;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// When an account's event happens
#[derive(Clone, Debug, PartialEq)]
pub enum Times {
    /// one occurrence (Google and Microsoft give each occurrence by itself)
    Fixed { start: i64, end: i64, all_day: bool },
    /// iCloud's iCalendar text, parsed: expanded for each range asked
    Ics(Arc<Vec<IcsEvent>>),
}

/// One of an account's events (one item on the server)
#[derive(Clone, Debug, PartialEq)]
pub struct AccEvent {
    /// "google:<account>:<calendar>:<id>", "cloud:<account>:<href or id>"
    pub key: String,
    /// "google" | "icloud" | "microsoft"
    pub provider: String,
    /// who it's from, as the editor says it: Google's account
    /// ("jane@gmail.com"), "iCloud", "Microsoft 365"
    pub account: String,
    pub calendar: String,
    pub color: String,
    pub title: String,
    pub location: String,
    pub notes: String,
    pub writable: bool,
    pub recurring: bool,
    /// Google's / Microsoft's series id ("" none)
    pub series: String,
    pub times: Times,
    /// the item as the system host has it: what google.updateEvent and
    /// cloudaccounts.updateEvent take back
    pub source: Value,
}

/// One of an account's tasks
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Task {
    pub todo: Todo,
    /// "google" | "icloud" | "microsoft"
    pub provider: String,
    /// the list's name
    pub list: String,
    pub source: Value,
}

/// A calendar a new event can go to, or a list a new to-do can go to: the
/// value Settings keeps (google.newEventCalendar / google.taskListId:
/// "<account>|<id>" for Google, "cloud|<account>|<id>" for the others)
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Target {
    pub value: String,
    pub label: String,
    pub provider: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Data {
    pub events: Vec<AccEvent>,
    pub tasks: Vec<Task>,
    /// calendars you can add to (shown and writable)
    pub targets: Vec<Target>,
    /// task lists
    pub lists: Vec<Target>,
    /// Google is connected
    pub google: bool,
    /// changes kept here until the accounts can be reached again
    pub waiting: i64,
    /// the last sync could not reach an account
    pub offline: bool,
}

impl Data {
    /// The event an item of `between` came from (its key, or its key and
    /// "|<start>" for one occurrence of iCloud's)
    pub fn event(&self, key: &str) -> Option<&AccEvent> {
        self.events.iter().find(|e| e.key == key || (key.starts_with(&e.key) && key[e.key.len()..].starts_with('|')))
    }
    pub fn task(&self, key: &str) -> Option<&Task> {
        self.tasks.iter().find(|t| t.todo.key == key)
    }
    pub fn target(&self, value: &str) -> Option<&Target> {
        self.targets.iter().find(|t| t.value == value)
    }
    pub fn list(&self, value: &str) -> Option<&Target> {
        self.lists.iter().find(|t| t.value == value)
    }
}

/// "iCloud", "Microsoft 365", "Google"
pub fn provider_name(p: &str) -> &'static str {
    match p {
        "icloud" => "iCloud",
        "microsoft" => "Microsoft 365",
        _ => "Google",
    }
}

/// An account key is one of these
pub fn is_account_key(key: &str) -> bool {
    ["google:", "cloud:", "google/", "cloud/"].iter().any(|p| key.starts_with(p))
}

// ── Reading the system host's state ─────────────────────────────────────

fn s(v: &Value, k: &str) -> String {
    match &v[k] {
        Value::String(x) => x.clone(),
        Value::Null => String::new(),
        x => x.to_string(),
    }
}

fn arr(v: &Value) -> &[Value] {
    v.as_array().map(Vec::as_slice).unwrap_or(&[])
}

fn ms(v: &Value) -> i64 {
    v.as_f64().map_or(0, |f| f as i64)
}

/// new Date(rfc3339).getTime(): "2026-11-02T09:00:00(.fff)(Z|±hh:mm)"; no
/// zone counts as UTC (Graph's, which says "UTC" beside it)
pub fn parse_iso(text: &str) -> Option<i64> {
    let (date, rest) = text.split_once('T')?;
    let d: Vec<i32> = date.split('-').filter_map(|x| x.parse().ok()).collect();
    if d.len() != 3 {
        return None;
    }
    let (clock, off) = match rest.find(['Z', 'z', '+']).or_else(|| rest.rfind('-')) {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "Z"),
    };
    let (hms, frac) = clock.split_once('.').unwrap_or((clock, "0"));
    let t: Vec<i32> = hms.split(':').filter_map(|x| x.parse().ok()).collect();
    if t.len() < 2 {
        return None;
    }
    let ms3: i64 = format!("{frac:0<3}").chars().take(3).collect::<String>().parse().unwrap_or(0);
    let mut at = cal::make_utc(d[0], d[1] - 1, d[2], t[0], t[1], t.get(2).copied().unwrap_or(0)) + ms3;
    if off.len() >= 6 && (off.starts_with('+') || off.starts_with('-')) {
        let oh: i64 = off[1..3].parse().ok()?;
        let om: i64 = off[4..6].parse().ok()?;
        let shift = (oh * 60 + om) * 60_000;
        at += if off.starts_with('+') { -shift } else { shift };
    }
    Some(at)
}

/// Google's / Graph's `{dateTime}` | `{date: "yyyy-mm-dd"}` -> (ms, all day);
/// a date is local midnight, as `new Date(y, m - 1, d)`
pub fn when(v: &Value) -> Option<(i64, bool)> {
    if let Some(dt) = v["dateTime"].as_str() {
        return parse_iso(dt).map(|t| (t, false));
    }
    let p: Vec<i32> = v["date"].as_str()?.split('-').filter_map(|x| x.parse().ok()).collect();
    (p.len() == 3).then(|| (cal::make(p[0], p[1] - 1, p[2], 0, 0, 0), true))
}

/// iCloud's iCalendar text, parsed once per version (every answer from the
/// helper has them all)
fn ics_cache() -> &'static Mutex<HashMap<String, Arc<Vec<IcsEvent>>>> {
    static C: OnceLock<Mutex<HashMap<String, Arc<Vec<IcsEvent>>>>> = OnceLock::new();
    C.get_or_init(Default::default)
}

fn version_of(key: &str, etag: &str, ics: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    ics.hash(&mut h);
    format!("{key}|{etag}|{:x}", h.finish())
}

fn hidden(cfg: &Value, group: &str, account: &str, id: &str) -> bool {
    let full = format!("{account}|{id}");
    arr(&cfg[group]["hiddenCalendars"]).iter().any(|h| h.as_str() == Some(&full) || (group == "google" && h.as_str() == Some(id)))
}

/// What the system host's Google ("google" on its bus) and CloudAccounts
/// ("cloudaccounts") last published, with the settings (hidden calendars)
pub fn from_bus(google: &Value, cloud: &Value, cfg: &Value) -> Data {
    let mut d = Data { google: google["connected"].as_bool().unwrap_or(false), ..Default::default() };
    let g_accounts = arr(&google["accounts"]);
    let identity = |aid: &str| g_accounts.iter().find(|a| a["id"].as_str() == Some(aid)).map(|a| s(a, "identity")).unwrap_or_default();

    // Google: one item per occurrence, overrides applied there
    for e in arr(&google["events"]) {
        let (Some((start, all_day)), Some((end, _))) = (when(&e["start"]), when(&e["end"])) else { continue };
        let account = s(e, "accountName");
        d.events.push(AccEvent {
            key: s(e, "key"),
            provider: "google".into(),
            account: if account.is_empty() { "Google".into() } else { account },
            calendar: s(e, "calendar"),
            color: s(e, "color"),
            title: s(e, "title"),
            location: s(e, "location"),
            notes: s(e, "notes"),
            writable: e["writable"].as_bool().unwrap_or(false),
            recurring: !s(e, "series").is_empty(),
            series: s(e, "series"),
            times: Times::Fixed { start, end: end.max(start), all_day },
            source: e.clone(),
        });
    }
    for c in arr(&google["calendars"]) {
        let (acc, id) = (s(c, "account"), s(c, "id"));
        if c["writable"].as_bool().unwrap_or(false) && !hidden(cfg, "google", &acc, &id) {
            let who = if g_accounts.len() > 1 { format!(" · {}", identity(&acc)) } else { String::new() };
            d.targets.push(Target { value: format!("{acc}|{id}"), label: format!("{}{who}", s(c, "title")), provider: "google".into() });
        }
    }
    for l in arr(&google["taskLists"]) {
        let who = if g_accounts.len() > 1 { format!(" · {}", identity(&s(l, "account"))) } else { String::new() };
        d.lists.push(Target { value: format!("{}|{}", s(l, "account"), s(l, "id")), label: format!("{}{who}", s(l, "title")), provider: "google".into() });
    }
    for t in arr(&google["tasks"]) {
        let key = s(t, "key");
        d.tasks.push(Task { todo: todos::todo_from(t, key), provider: "google".into(), list: s(t, "listName"), source: t.clone() });
    }

    // iCloud and Microsoft 365
    let c_accounts = arr(&cloud["accounts"]);
    let provider = |aid: &str| c_accounts.iter().find(|a| a["id"].as_str() == Some(aid)).map(|a| s(a, "provider")).unwrap_or_default();
    let mut seen = HashMap::new();
    {
        let cache = ics_cache().lock().map(|c| c.clone()).unwrap_or_default();
        for p in arr(&cloud["events"]) {
            if p["hidden"].as_bool().unwrap_or(false) {
                continue;
            }
            let (raw, c, patch) = (&p["raw"], &p["cal"], &p["patch"]);
            let key = s(p, "key");
            let prov = provider(&s(raw, "account"));
            let mut ev = AccEvent {
                key: key.clone(),
                account: provider_name(&prov).into(),
                provider: prov,
                calendar: s(c, "title"),
                color: s(c, "color"),
                title: s(raw, "title"),
                location: s(raw, "location"),
                notes: s(p, "notes"),
                writable: raw["writable"].as_bool().unwrap_or(false),
                recurring: !s(raw, "series").is_empty(),
                series: s(raw, "series"),
                times: Times::Fixed { start: 0, end: 0, all_day: false },
                source: raw.clone(),
            };
            if let Some(ics) = p["ics"].as_str() {
                let v = version_of(&key, &s(raw, "etag"), ics);
                let list = cache.get(&v).cloned().unwrap_or_else(|| Arc::new(cal::parse_ics(ics)));
                seen.insert(v, list.clone());
                ev.recurring = list.iter().any(|x| x.rrule.is_some());
                ev.series = if ev.recurring { key.clone() } else { String::new() };
                ev.title = list.first().map(|x| x.title.clone()).unwrap_or_default();
                ev.times = Times::Ics(list);
            } else {
                let (Some((start, all_day)), Some((end, _))) = (when(&raw["start"]), when(&raw["end"])) else { continue };
                ev.times = Times::Fixed { start, end: end.max(start), all_day };
            }
            apply_patch(&mut ev, patch);
            d.events.push(ev);
        }
    }
    if let Ok(mut c) = ics_cache().lock() {
        *c = seen;
    }
    let list_name = |acc: &str, id: &str| arr(&cloud["taskLists"]).iter().find(|l| s(l, "account") == acc && s(l, "id") == id).map(|l| s(l, "title")).unwrap_or_default();
    for c in arr(&cloud["calendars"]) {
        let (acc, id) = (s(c, "account"), s(c, "id"));
        if c["writable"].as_bool().unwrap_or(false) && !hidden(cfg, "cloud", &acc, &id) {
            let prov = provider(&acc);
            d.targets.push(Target { value: format!("cloud|{acc}|{id}"), label: format!("{} · {}", s(c, "title"), provider_name(&prov)), provider: prov });
        }
    }
    for l in arr(&cloud["taskLists"]) {
        let (acc, id) = (s(l, "account"), s(l, "id"));
        let prov = provider(&acc);
        let what = if prov == "icloud" { "Reminders" } else { "To Do" };
        d.lists.push(Target { value: format!("cloud|{acc}|{id}"), label: format!("{} · {what}", s(l, "title")), provider: prov });
    }
    for t in arr(&cloud["tasks"]) {
        let acc = s(t, "account");
        d.tasks.push(Task { todo: todos::todo_from(t, s(t, "key")), provider: provider(&acc), list: list_name(&acc, &s(t, "listId")), source: t.clone() });
    }
    d.waiting = google["outbox"].as_i64().unwrap_or(0) + cloud["outbox"].as_i64().unwrap_or(0);
    d.offline = google["offline"].as_bool().unwrap_or(false) || cloud["offline"].as_bool().unwrap_or(false);
    d
}

/// CloudAccounts.shownPatch: a change on its way, shown already
fn apply_patch(ev: &mut AccEvent, patch: &Value) {
    if !patch.is_object() {
        return;
    }
    let text = |k: &str| patch[k].as_str().map(String::from);
    if let Some(t) = text("title") {
        ev.title = t.clone();
        if let Times::Ics(list) = &ev.times {
            ev.times = Times::Ics(Arc::new(list.iter().map(|x| IcsEvent { title: t.clone(), ..x.clone() }).collect()));
        }
    }
    if let Some(l) = text("location") {
        ev.location = l.clone();
        if let Times::Ics(list) = &ev.times {
            ev.times = Times::Ics(Arc::new(list.iter().map(|x| IcsEvent { location: l.clone(), ..x.clone() }).collect()));
        }
    }
    if let Some(n) = text("notes") {
        ev.notes = n;
    }
    if patch["start"].is_number() {
        let start = ms(&patch["start"]);
        let end = if patch["end"].is_number() { ms(&patch["end"]) } else { start };
        let all_day = patch["allDay"].as_bool().unwrap_or(match &ev.times {
            Times::Fixed { all_day, .. } => *all_day,
            Times::Ics(l) => l.first().is_some_and(|x| x.all_day),
        });
        ev.times = Times::Fixed { start, end: end.max(start), all_day };
    }
}

/// The occurrences overlapping [from, to), as `Calendar::between` lists them
pub fn between(d: &Data, from: i64, to: i64) -> Vec<Event> {
    let mut out = Vec::new();
    for e in &d.events {
        let base = || Event {
            key: e.key.clone(),
            title: e.title.clone(),
            color: e.color.clone(),
            calendar: e.calendar.clone(),
            account: e.account.clone(),
            location: e.location.clone(),
            notes: e.notes.clone(),
            writable: e.writable,
            recurring: e.recurring,
            series: if e.series.is_empty() { String::new() } else { format!("{}:{}", e.provider, e.series) },
            ..Default::default()
        };
        match &e.times {
            Times::Fixed { start, end, all_day } => {
                if *start < to && (*end > from || (*end == *start && *start >= from)) {
                    out.push(Event { start: *start, end: *end, all_day: *all_day, ..base() });
                }
            }
            Times::Ics(list) => {
                for ev in list.iter() {
                    for (s, en) in cal::expand(ev, from, to) {
                        out.push(Event {
                            key: format!("{}|{}", e.key, s),
                            title: ev.title.clone(),
                            location: ev.location.clone(),
                            start: s,
                            end: en,
                            all_day: ev.all_day,
                            alert: ev.alert,
                            recurring: ev.rrule.is_some(),
                            series: if ev.rrule.is_some() { format!("{}|{}", e.key, ev.uid) } else { String::new() },
                            ..base()
                        });
                    }
                }
            }
        }
    }
    out
}

// ── Both halves together ────────────────────────────────────────────────

#[derive(Default)]
struct Hub {
    data: Arc<Data>,
    cal: Option<Calendar>,
    cal_tx: Option<Sender<Msg>>,
    todos: Option<Todos>,
    todos_tx: Option<Sender<Msg>>,
}

fn hub() -> &'static Mutex<Hub> {
    static H: OnceLock<Mutex<Hub>> = OnceLock::new();
    H.get_or_init(Default::default)
}

/// The accounts' items as last set
pub fn current() -> Arc<Data> {
    hub().lock().map(|h| h.data.clone()).unwrap_or_default()
}

fn send_calendar(h: &Hub) -> bool {
    let (Some(c), Some(tx)) = (&h.cal, &h.cal_tx) else { return true };
    let mut c = c.clone();
    c.accounts = h.data.clone();
    tx.send(Msg::Data(Box::new(c))).is_ok()
}

/// Your to-dos, then the accounts' tasks
pub fn merged_todos(files: &Todos, d: &Data) -> Todos {
    let mut list = files.list.clone();
    list.extend(d.tasks.iter().map(|t| t.todo.clone()));
    Todos { list }
}

fn send_todos(h: &Hub) -> bool {
    let (Some(t), Some(tx)) = (&h.todos, &h.todos_tx) else { return true };
    tx.send(Msg::Data(Box::new(merged_todos(t, &h.data)))).is_ok()
}

/// The accounts' items changed (the system host's mirror): the calendar and
/// the to-dos go out again with them
pub fn set(d: Data) {
    let Ok(mut h) = hub().lock() else { return };
    if *h.data == d {
        return;
    }
    h.data = Arc::new(d);
    send_calendar(&h);
    send_todos(&h);
}

/// The calendar files were read (calendar.rs's watcher): false once nobody listens
pub(crate) fn calendar_read(c: Calendar, tx: &Sender<Msg>) -> bool {
    let Ok(mut h) = hub().lock() else { return tx.send(Msg::Data(Box::new(c))).is_ok() };
    h.cal = Some(c);
    h.cal_tx = Some(tx.clone());
    send_calendar(&h)
}

/// state.json's to-dos were read (todos.rs's watcher)
pub(crate) fn todos_read(t: Todos, tx: &Sender<Msg>) -> bool {
    let Ok(mut h) = hub().lock() else { return tx.send(Msg::Data(Box::new(t))).is_ok() };
    h.todos = Some(t);
    h.todos_tx = Some(tx.clone());
    send_todos(&h)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// What the system host publishes, as its helpers answered (fixtures in
    /// the shape google-account.py and cloud-accounts.py give)
    fn google() -> Value {
        json!({
            "connected": true, "outbox": 1, "offline": true,
            "accounts": [{"id": "g1", "identity": "jane@gmail.com"}],
            "calendars": [{"account": "g1", "id": "primary", "title": "Jane", "writable": true},
                          {"account": "g1", "id": "holidays", "title": "Holidays", "writable": false},
                          {"account": "g1", "id": "team", "title": "Team", "writable": true}],
            "taskLists": [{"account": "g1", "id": "L1", "title": "My Tasks"}],
            "events": [
                {"key": "google:g1:primary:e1", "account": "g1", "calendarId": "primary", "id": "e1", "title": "Dentist",
                 "start": {"dateTime": "2026-11-20T09:00:00+01:00"}, "end": {"dateTime": "2026-11-20T10:00:00+01:00"},
                 "location": "Main St", "notes": "Bring card", "color": "#039be5", "calendar": "Jane", "writable": true,
                 "series": "", "accountName": "jane@gmail.com", "etag": "\"1\""},
                {"key": "google:g1:primary:s1_1", "account": "g1", "calendarId": "primary", "id": "s1_1", "title": "Standup",
                 "start": {"dateTime": "2026-11-20T09:30:00Z"}, "end": {"dateTime": "2026-11-20T09:45:00Z"},
                 "calendar": "Jane", "writable": true, "series": "s1", "accountName": "jane@gmail.com"},
                {"key": "google:g1:holidays:h", "account": "g1", "calendarId": "holidays", "id": "h", "title": "Holiday",
                 "start": {"date": "2026-11-21"}, "end": {"date": "2026-11-22"}, "calendar": "Holidays", "writable": false, "accountName": "jane@gmail.com"}
            ],
            "tasks": [{"key": "google/g1/L1/t1", "account": "g1", "listId": "L1", "listName": "My Tasks", "id": "t1", "text": "Pay rent",
                       "done": false, "date": "2026-11-20", "due": 1795219140000i64, "allDay": true, "google": true}]
        })
    }

    fn cloud() -> Value {
        let ics = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:w\r\nDTSTART:20261116T080000Z\r\nDTEND:20261116T083000Z\r\nRRULE:FREQ=DAILY\r\nSUMMARY:Run\r\nLOCATION:Park\r\nBEGIN:VALARM\r\nTRIGGER:-PT15M\r\nACTION:DISPLAY\r\nEND:VALARM\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        json!({
            "accounts": [{"id": "icloud-1", "provider": "icloud"}, {"id": "ms-1", "provider": "microsoft"}],
            "calendars": [{"account": "icloud-1", "id": "/cal/home/", "title": "Home", "color": "#ff2968", "writable": true},
                          {"account": "ms-1", "id": "c1", "title": "Calendar", "color": "#0078d4", "writable": true}],
            "taskLists": [{"account": "ms-1", "id": "l1", "title": "Tasks"}, {"account": "icloud-1", "id": "/cal/rem/", "title": "Errands"}],
            "events": [
                {"raw": {"account": "icloud-1", "calendarId": "/cal/home/", "href": "/cal/home/w.ics", "etag": "\"a\"", "ics": ics, "writable": false},
                 "cal": {"account": "icloud-1", "id": "/cal/home/", "title": "Home", "color": "#ff2968"}, "key": "cloud:icloud-1:/cal/home/w.ics",
                 "ics": ics, "notes": "", "hidden": false, "patch": null},
                {"raw": {"account": "ms-1", "calendarId": "c1", "id": "m1", "title": "Review", "start": {"dateTime": "2026-11-20T13:00:00Z"},
                         "end": {"dateTime": "2026-11-20T14:00:00Z"}, "location": "Teams", "series": "", "writable": true},
                 "cal": {"account": "ms-1", "id": "c1", "title": "Calendar", "color": "#0078d4"}, "key": "cloud:ms-1:m1", "notes": "Agenda",
                 "hidden": false, "patch": {"title": "Review (moved)", "start": 1795222800000i64, "end": 1795226400000i64}},
                {"raw": {"account": "ms-1", "calendarId": "c1", "id": "gone", "title": "Deleted", "start": {"dateTime": "2026-11-20T13:00:00Z"},
                         "end": {"dateTime": "2026-11-20T14:00:00Z"}},
                 "cal": {"account": "ms-1", "id": "c1"}, "key": "cloud:ms-1:gone", "hidden": true}
            ],
            "tasks": [{"key": "cloud/ms-1/l1/x", "account": "ms-1", "listId": "l1", "id": "x", "text": "Buy milk", "done": false, "due": 0, "allDay": true, "cloud": true},
                      {"key": "cloud/icloud-1//cal/rem//r1", "account": "icloud-1", "listId": "/cal/rem/", "id": "r1", "text": "Call mum", "done": false,
                       "due": 1795230000000i64, "allDay": false, "cloud": true}],
            "outbox": 2
        })
    }

    #[test]
    fn events_merge_with_their_calendar_account_and_changes() {
        let cfg = json!({"google": {"hiddenCalendars": ["g1|team"]}});
        let d = from_bus(&google(), &cloud(), &cfg);
        let from = cal::make_utc(2026, 10, 16, 0, 0, 0);
        let to = cal::make_utc(2026, 10, 23, 0, 0, 0);
        let list = between(&d, from, to);
        let dentist = list.iter().find(|e| e.title == "Dentist").unwrap();
        assert_eq!(dentist.start, cal::make_utc(2026, 10, 20, 8, 0, 0)); // (+01:00)
        assert_eq!((dentist.calendar.as_str(), dentist.account.as_str(), dentist.location.as_str()), ("Jane", "jane@gmail.com", "Main St"));
        assert!(dentist.writable && !dentist.recurring && dentist.series.is_empty());
        let standup = list.iter().find(|e| e.title == "Standup").unwrap();
        assert!(standup.recurring);
        assert_eq!(standup.series, "google:s1");
        let holiday = list.iter().find(|e| e.title == "Holiday").unwrap();
        assert!(holiday.all_day && !holiday.writable);
        // iCloud's repeating event, every day, with its alarm and place
        let runs: Vec<&Event> = list.iter().filter(|e| e.title == "Run").collect();
        assert_eq!(runs.len(), 7);
        assert_eq!((runs[0].alert, runs[0].location.as_str(), runs[0].account.as_str()), (Some(15), "Park", "iCloud"));
        assert!(runs[0].key.starts_with("cloud:icloud-1:/cal/home/w.ics|"));
        assert_eq!(cal::daily_in(&list, from, to).len(), 1);
        // Microsoft's, with the change on its way shown; the deleted one gone
        let review = list.iter().find(|e| e.key == "cloud:ms-1:m1").unwrap();
        assert_eq!((review.title.as_str(), review.start, review.notes.as_str()), ("Review (moved)", 1795222800000, "Agenda"));
        assert!(!list.iter().any(|e| e.title == "Deleted"));
        // the occurrence finds its event
        assert_eq!(d.event(&runs[2].key).map(|e| e.provider.as_str()), Some("icloud"));
        assert_eq!(d.event("google:g1:primary:e1").map(|e| e.source["etag"].clone()), Some(json!("\"1\"")));
        // where new ones can go: writable and shown
        let t: Vec<&str> = d.targets.iter().map(|t| t.value.as_str()).collect();
        assert_eq!(t, vec!["g1|primary", "cloud|icloud-1|/cal/home/", "cloud|ms-1|c1"]);
        assert_eq!(d.targets[1].label, "Home · iCloud");
        let l: Vec<&str> = d.lists.iter().map(|t| t.label.as_str()).collect();
        assert_eq!(l, vec!["My Tasks", "Tasks · To Do", "Errands · Reminders"]);
        assert_eq!((d.waiting, d.offline), (3, true));
    }

    #[test]
    fn tasks_join_the_todos_after_yours() {
        let d = from_bus(&google(), &cloud(), &json!({}));
        let mine = Todos { list: vec![Todo { key: "local/a".into(), text: "Water plants".into(), ..Default::default() }] };
        let all = merged_todos(&mine, &d);
        let keys: Vec<&str> = all.list.iter().map(|t| t.key.as_str()).collect();
        assert_eq!(keys, vec!["local/a", "google/g1/L1/t1", "cloud/ms-1/l1/x", "cloud/icloud-1//cal/rem//r1"]);
        let rem = d.task("cloud/icloud-1//cal/rem//r1").unwrap();
        assert_eq!((rem.provider.as_str(), rem.list.as_str(), rem.todo.all_day, rem.todo.due), ("icloud", "Errands", false, 1795230000000));
        assert_eq!(d.task("google/g1/L1/t1").unwrap().todo.due, 1795219140000);
    }

    #[test]
    fn times_read() {
        assert_eq!(parse_iso("1970-01-01T00:00:01Z"), Some(1000));
        assert_eq!(parse_iso("1970-01-01T01:00:00+01:00"), Some(0));
        assert_eq!(parse_iso("1970-01-01T00:00:00.5-00:30"), Some(1_800_500));
        assert_eq!(parse_iso("1970-01-01T00:00:00.0000000"), Some(0));
        assert_eq!(when(&json!({"date": "2026-11-20"})), Some((cal::make(2026, 10, 20, 0, 0, 0), true)));
        assert!(when(&json!({})).is_none());
        assert!(is_account_key("google/a/b/c") && is_account_key("cloud:a:b") && !is_account_key("local/x"));
    }
}

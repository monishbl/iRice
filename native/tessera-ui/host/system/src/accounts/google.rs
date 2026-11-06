//! Google.qml: Google Calendar and Tasks through assets/scripts/
//! google-account.py: one request per run, JSON in on stdin, JSON lines back
//! (the account's state, errors, "the change went through"). Several
//! accounts; their calendars and task lists show together.
//!
//! Changes to tasks and events show at once (`overrides`), queue, and go to
//! the helper as soon as it is free; the helper keeps its own outbox on disk
//! (offline changes are sent later: 30 s, then 2, 8, 30 minutes). Deleting
//! hides first and is sent when its Undo (8 s) has run out. Syncs: at login,
//! on unlock, when Notification Center opens and the last sync is over 2
//! minutes old, every 15 minutes in use; never locked or idle; after a
//! failure the next try waits longer (1 minute … 2 hours).
//!
//! No tokens are read here: the helper owns them.
//!
//! Bus "google": { account (the helper's last report), connected,
//! configured, busy, lastError, operation, pending, taskLists, accounts,
//! outbox, calendars, syncedAt, tasks, events, offline, signingIn,
//! mutationSeq }. `tasks`/`events` are the override-applied lists the QML's
//! properties had. Calls (args as the QML function's, in an array):
//! google.run [request] | connect | addTask [listRef, patch] | updateTask
//! [task, patch] | removeTask [task] | restoreTask [task] | addEvent
//! [calendarRef, fields] | updateEvent [e, fields, every] | removeEvent
//! [e, every] | restoreEvent [e] | freshen [age] | openKeep | refresh |
//! write [request] | setError [text].

use super::common::*;
use crate::util::config_set;
use serde_json::{json, Map, Value};
use std::any::Any;
use std::time::Duration;
use tessera_core::host::{Host, Service};

enum M {
    Line(String),
    Exit(i32),
    Resend,
    Poll,
}

struct Queued {
    request: Value,
    key: String,
}

struct Deleting {
    key: String,
    request: Value,
    at: i64,
}

pub struct Google {
    tx: Option<Tx>,
    account: Value,
    last_error: String,
    operation: String,
    pending: Value,
    queue: Vec<Queued>,
    current: Option<Queued>,
    overrides: Map<String, Value>,
    deleting: Vec<Deleting>,
    running: bool,
    replied: bool,
    offline: bool,
    signing_in: bool,
    failures: u32,
    retry_at: i64,
    send_failures: u32,
    mutation_seq: i64,
    hidden: Vec<String>, // google.hiddenCalendars
    task_list_id: String,
    new_event_cal: String,
    // edges seen by the poll
    was_locked: bool,
    was_idle: bool,
    was_nc: bool,
    ticks: u32,
    child: Child,
    resend: slint::Timer,
    poll: slint::Timer,
}

impl Default for Google {
    fn default() -> Self {
        Self {
            tx: None,
            account: json!({}),
            last_error: String::new(),
            operation: String::new(),
            pending: json!({}),
            queue: vec![],
            current: None,
            overrides: Map::new(),
            deleting: vec![],
            running: false,
            replied: false,
            offline: false,
            signing_in: false,
            failures: 0,
            retry_at: 0,
            send_failures: 0,
            mutation_seq: 0,
            hidden: vec![],
            task_list_id: String::new(),
            new_event_cal: String::new(),
            was_locked: false,
            was_idle: false,
            was_nc: false,
            ticks: 0,
            child: Child::default(),
            resend: slint::Timer::default(),
            poll: slint::Timer::default(),
        }
    }
}

pub fn event_key(e: &Value) -> String {
    format!("google:{}:{}:{}", js(e, "account"), js(e, "calendarId"), js(e, "id"))
}
pub fn task_key(t: &Value) -> String {
    format!("google/{}/{}/{}", js(t, "account"), js(t, "listId"), js(t, "id"))
}

/// when(ms, allDay)
fn when(ms: i64, all_day: bool) -> Value {
    if all_day { json!({"date": day_of(ms)}) } else { json!({"dateTime": iso_ms(ms)}) }
}

/// shownFields(f): what an event shows before Google has it
fn shown_fields(f: &Value) -> Value {
    let mut out = Map::new();
    for k in ["title", "location", "notes"] {
        if !f[k].is_null() {
            out.insert(k.into(), f[k].clone());
        }
    }
    if let Some(start) = f["start"].as_i64() {
        let all = f["allDay"].as_bool().unwrap_or(false);
        out.insert("start".into(), when(start, all));
        out.insert("end".into(), when(f["end"].as_i64().unwrap_or(start), all));
    }
    Value::Object(out)
}

/// startOf(e)
pub fn start_of(e: &Value) -> i64 {
    let x = &e["start"];
    if let Some(dt) = x["dateTime"].as_str() {
        return parse_iso(dt).unwrap_or(0);
    }
    let d = x["date"].as_str().unwrap_or("");
    let p: Vec<i32> = d.split('-').filter_map(|v| v.parse().ok()).collect();
    if p.len() == 3 { local_ms(p[0], p[1], p[2], 0, 0) } else { 0 }
}

fn merge(a: &Value, b: &Value) -> Value {
    let mut m = a.as_object().cloned().unwrap_or_default();
    if let Some(o) = b.as_object() {
        for (k, v) in o {
            m.insert(k.clone(), v.clone());
        }
    }
    Value::Object(m)
}

/// find(rows, ref): "<account>|<id>" (a bare id: the first account that has it)
pub fn find_ref<'a>(rows: &'a [Value], r: &str) -> Option<&'a Value> {
    if r.is_empty() {
        return None;
    }
    match r.find('|') {
        None => rows.iter().find(|x| x["id"].as_str() == Some(r)),
        Some(i) => rows.iter().find(|x| x["account"].as_str() == Some(&r[..i]) && x["id"].as_str() == Some(&r[i + 1..])),
    }
}

impl Google {
    fn tx(&self) -> Tx {
        self.tx.clone().expect("started")
    }
    fn arr(&self, k: &str) -> Vec<Value> {
        self.account[k].as_array().cloned().unwrap_or_default()
    }
    fn connected(&self) -> bool {
        self.account["connected"].as_bool().unwrap_or(false)
    }
    fn outbox(&self) -> i64 {
        self.account["outbox"].as_i64().unwrap_or(0)
    }
    fn synced_at(&self) -> i64 {
        self.account["syncedAt"].as_f64().unwrap_or(0.0) as i64
    }
    fn identity(&self, aid: &str) -> String {
        self.arr("accounts").iter().find(|a| a["id"].as_str() == Some(aid)).map(|a| js(a, "identity")).unwrap_or_default()
    }
    fn shown(&self, c: &Value) -> bool {
        let cal = c["calendarId"].as_str().or(c["id"].as_str()).unwrap_or("");
        let full = format!("{}|{}", js(c, "account"), cal);
        !self.hidden.contains(&full) && !self.hidden.iter().any(|h| h == cal)
    }
    fn set_override(&mut self, key: &str, o: Option<Value>) {
        match o {
            Some(v) => {
                self.overrides.insert(key.to_string(), v);
            }
            None => {
                self.overrides.remove(key);
            }
        }
    }
    fn patch_of(&self, key: &str) -> Value {
        self.overrides.get(key).map(|o| o["patch"].clone()).unwrap_or(Value::Null)
    }

    fn tasks(&self) -> Vec<Value> {
        self.arr("tasks")
            .iter()
            .filter_map(|t| {
                let key = task_key(t);
                let o = self.overrides.get(&key);
                if o.is_some_and(|o| o["hidden"].as_bool().unwrap_or(false)) {
                    return None;
                }
                let due = t["date"].as_str().map_or(0, due_of);
                let base = merge(t, &json!({"key": key, "google": true, "allDay": true, "due": due}));
                Some(merge(&base, &o.map(|o| o["patch"].clone()).unwrap_or(Value::Null)))
            })
            .collect()
    }
    fn events(&self) -> Vec<Value> {
        self.arr("events")
            .iter()
            .filter(|e| self.shown(e))
            .filter_map(|e| {
                let key = event_key(e);
                let o = self.overrides.get(&key);
                if o.is_some_and(|o| o["hidden"].as_bool().unwrap_or(false)) {
                    return None;
                }
                let base = merge(e, &json!({"key": key, "accountName": self.identity(e["account"].as_str().unwrap_or(""))}));
                Some(merge(&base, &o.map(|o| o["patch"].clone()).unwrap_or(Value::Null)))
            })
            .collect()
    }

    fn publish(&self) {
        put(
            "google",
            json!({"account": self.account, "connected": self.connected(), "configured": self.account["configured"].as_bool().unwrap_or(false),
                   "busy": self.running, "lastError": self.last_error, "operation": self.operation, "pending": self.pending,
                   "taskLists": self.arr("taskLists"), "accounts": self.arr("accounts"), "outbox": self.outbox(),
                   "calendars": self.arr("calendars"), "syncedAt": self.synced_at(), "tasks": self.tasks(), "events": self.events(),
                   "offline": self.offline, "signingIn": self.signing_in, "mutationSeq": self.mutation_seq,
                   "queued": self.queue.len(), "deleting": self.deleting.len()}),
        );
    }

    // ── The helper ──────────────────────────────────────────────────────
    fn start_request(&mut self, request: Value) {
        self.last_error.clear();
        self.replied = false;
        self.pending = request.clone();
        self.operation = js(&request, "cmd");
        self.offline = false;
        self.running = true;
        let cfgdir = tessera_core::theme::config_path().parent().map(|p| p.join("google").to_string_lossy().into_owned()).unwrap_or_default();
        let cmd = vec!["python3".to_string(), "-I".into(), script("google-account.py"), cfgdir];
        let (t1, t2) = (self.tx(), self.tx());
        let ok = self.child.spawn(&cmd, Some(format!("{}\n", request)), move |l| t1.send(M::Line(l)), move |c| t2.send(M::Exit(c)));
        if !ok {
            self.running = false;
            self.last_error = "Google helper failed. Check that python3 and python-gobject are installed.".into();
            if self.current.take().is_some() {}
        }
    }
    /// run(request): one at a time, turned away while busy
    fn run(&mut self, request: Value) -> bool {
        if self.running || !self.queue.is_empty() {
            self.last_error = "Google is busy. Wait for the current operation, then try again.".into();
            return false;
        }
        self.start_request(request);
        true
    }
    fn write(&mut self, request: Value, key: &str, over: Option<Value>) -> bool {
        if !key.is_empty() {
            if let Some(o) = over {
                self.set_override(key, Some(o));
            }
        }
        self.queue.push(Queued { request, key: key.to_string() });
        self.pump();
        true
    }
    fn pump(&mut self) {
        if self.running || self.queue.is_empty() {
            return;
        }
        let next = self.queue.remove(0);
        let req = next.request.clone();
        self.current = Some(next);
        self.start_request(req);
    }
    fn freshen(&mut self, age: i64) {
        let now = now_ms();
        if !self.connected() || self.running || !self.queue.is_empty() || locked() || now < self.retry_at {
            return;
        }
        if now - self.synced_at() >= age {
            self.run(json!({"cmd": "sync"}));
        }
    }
    fn hide(&mut self, key: &str, request: Value) {
        self.set_override(key, Some(json!({"hidden": true})));
        self.deleting.retain(|d| d.key != key);
        self.deleting.push(Deleting { key: key.to_string(), request, at: now_ms() });
    }
    fn unhide(&mut self, key: &str) {
        self.deleting.retain(|d| d.key != key);
        self.set_override(key, None);
    }
    fn resend_interval(&self) -> Duration {
        let ms = (30_000f64 * 4f64.powi(self.send_failures.saturating_sub(1) as i32)).min(30.0 * 60_000.0);
        Duration::from_millis(ms as u64)
    }
    fn arm_resend(&self) {
        let tx = self.tx();
        self.resend.start(slint::TimerMode::SingleShot, self.resend_interval(), move || tx.send(M::Resend));
    }

    // ── Calls ───────────────────────────────────────────────────────────
    fn add_task(&mut self, list_ref: &str, patch: Value) -> bool {
        let lists = self.arr("taskLists");
        let Some(list) = find_ref(&lists, list_ref).cloned() else {
            self.last_error = "That task list isn't available. Choose another in Settings → Google.".into();
            return false;
        };
        self.write(json!({"cmd": "taskAdd", "account": list["account"], "listId": list["id"], "patch": patch}), "", None)
    }
    fn update_task(&mut self, task: &Value, patch: &Value) -> bool {
        let mut change = patch.clone();
        if !patch["due"].is_null() {
            let due = patch["due"].as_i64().unwrap_or(0);
            if due != 0 && patch["allDay"] == false {
                self.last_error = "Google Tasks supports due dates, not reminder times. Choose an all-day date.".into();
                return false;
            }
            change["date"] = json!(if due != 0 { day_of(due) } else { String::new() });
        }
        let key = task_key(task);
        let shown = merge(&self.patch_of(&key), patch);
        let req = json!({"cmd": "taskUpdate", "account": task["account"], "listId": task["listId"], "id": task["id"], "etag": task["etag"].as_str().unwrap_or(""),
                         "title": task["text"], "patch": change});
        self.write(req, &key, Some(json!({"patch": shown})))
    }
    fn add_event(&mut self, cal_ref: &str, fields: Value) -> bool {
        let cals = self.arr("calendars");
        let Some(cal) = find_ref(&cals, cal_ref).cloned() else {
            self.last_error = "That calendar isn't available. Choose another one.".into();
            return false;
        };
        if !cal["writable"].as_bool().unwrap_or(false) {
            self.last_error = "This calendar is read-only for your account. Choose a calendar you own.".into();
            return false;
        }
        self.write(json!({"cmd": "eventInsert", "account": cal["account"], "calendarId": cal["id"], "title": fields["title"], "fields": fields}), "", None)
    }
    fn series_of(&self, e: &Value) -> Vec<Value> {
        self.arr("events").into_iter().filter(|x| x["account"] == e["account"] && !e["series"].is_null() && x["series"] == e["series"]).collect()
    }
    fn update_event(&mut self, e: &Value, fields: &Value, every: bool) -> bool {
        if !e["writable"].as_bool().unwrap_or(false) {
            self.last_error = "This calendar is read-only for your account. Change it in a calendar you own, or ask its owner for edit access.".into();
            return false;
        }
        let mut request = json!({"cmd": "eventPatch", "account": e["account"], "calendarId": e["calendarId"], "id": e["id"], "etag": e["etag"].as_str().unwrap_or(""),
                                  "title": e["title"], "fields": fields});
        if every && !e["series"].is_null() && e["series"] != "" {
            request["series"] = e["series"].clone();
            let was = start_of(e);
            request["was"] = json!(was);
            // Every occurrence shown changes now too
            let moves = fields["start"].as_i64();
            let shift = moves.map_or(0, |s| s - was);
            let length = moves.map_or(0, |s| fields["end"].as_i64().unwrap_or(s) - s);
            for other in self.series_of(e) {
                let mut f = fields.clone();
                if moves.is_some() {
                    let st = start_of(&other) + shift;
                    f["start"] = json!(st);
                    f["end"] = json!(st + length);
                }
                let key = event_key(&other);
                let patch = merge(&self.patch_of(&key), &shown_fields(&f));
                self.set_override(&key, Some(json!({"patch": patch})));
            }
            return self.write(request, "", None);
        }
        let key = event_key(e);
        let patch = merge(&self.patch_of(&key), &shown_fields(fields));
        self.write(request, &key, Some(json!({"patch": patch})))
    }
    fn remove_event(&mut self, e: &Value, every: bool) -> bool {
        if !e["writable"].as_bool().unwrap_or(false) {
            self.last_error = "This calendar is read-only for your account.".into();
            return false;
        }
        let mut request = json!({"cmd": "eventDelete", "account": e["account"], "calendarId": e["calendarId"], "id": e["id"], "etag": e["etag"].as_str().unwrap_or(""), "title": e["title"]});
        if every && !e["series"].is_null() && e["series"] != "" {
            request["series"] = e["series"].clone();
            request.as_object_mut().map(|o| o.remove("etag"));
            for other in self.series_of(e).into_iter().filter(|x| x["id"] != e["id"]) {
                self.set_override(&event_key(&other), Some(json!({"hidden": true})));
            }
        }
        self.hide(&event_key(e), request);
        true
    }
    fn restore_event(&mut self, e: &Value) {
        let key = event_key(e);
        let series = self.deleting.iter().find(|d| d.key == key).map(|d| d.request["series"].clone()).filter(|s| !s.is_null());
        if let Some(s) = series {
            for other in self.arr("events").into_iter().filter(|x| x["account"] == e["account"] && x["series"] == s) {
                self.set_override(&event_key(&other), None);
            }
        }
        self.unhide(&key);
    }

    /// Google kept its own version of something you changed: say what you had
    fn conflict(c: &Value) {
        let reason = js(c, "reason");
        let why = match reason.as_str() {
            "deleted" => "it was deleted on Google".to_string(),
            "changed" => "it changed on Google".to_string(),
            r => format!("Google refused it ({r})"),
        };
        let body = format!("Your change to {} couldn't be saved because {why}. Here's what you had: {}", js(c, "title"), js(c, "what"));
        let cmd = vec![
            "sh".to_string(),
            "-c".into(),
            "[ \"$(timeout 600 notify-send -a Tessera --action=copy=Copy \"$1\" \"$2\")\" = copy ] && printf %s \"$3\" | wl-copy".into(),
            "sh".into(),
            "Google change not saved".into(),
            body,
            js(c, "what"),
        ];
        std::thread::spawn(move || {
            let _ = run_full(&cmd, None);
        });
    }
}

impl Service for Google {
    fn name(&self) -> &'static str {
        "Google"
    }
    fn calls(&self) -> &'static [&'static str] {
        &["google"]
    }
    fn start(&mut self, h: &mut Host) {
        let tx = Tx::new(h.sender("Google"));
        self.tx = Some(tx.clone());
        self.read_cfg(h.cfg());
        self.poll.start(slint::TimerMode::Repeated, Duration::from_secs(1), move || tx.send(M::Poll));
        self.run(json!({"cmd": "status"}));
        self.publish();
    }
    fn config_changed(&mut self, h: &mut Host) {
        self.read_cfg(h.cfg());
        self.publish();
    }
    fn call(&mut self, h: &mut Host, call: &str, args: &Value) {
        let a = |i: usize| args.get(i).cloned().unwrap_or(Value::Null);
        let s = |i: usize| args.get(i).and_then(Value::as_str).unwrap_or("").to_string();
        match call.split('.').nth(1).unwrap_or("") {
            "run" => {
                self.run(a(0));
            }
            // Connect: one click; the browser opens for a new account, an
            // account already in Online Accounts is used as it is
            "connect" => {
                self.run(json!({"cmd": "signin"}));
            }
            "refresh" => {}
            // Google.write(request): a queued change (the Settings bridge's eventAdd)
            "write" => {
                self.write(a(0), "", None);
            }
            // Google.lastError = …
            "setError" => self.last_error = s(0),
            "addTask" => {
                self.add_task(&s(0), a(1));
            }
            "updateTask" => {
                self.update_task(&a(0), &a(1));
            }
            "removeTask" => {
                let t = a(0);
                let req = json!({"cmd": "taskDelete", "account": t["account"], "listId": t["listId"], "id": t["id"], "etag": t["etag"].as_str().unwrap_or(""), "title": t["text"]});
                self.hide(&task_key(&t), req);
            }
            "restoreTask" => self.unhide(&task_key(&a(0))),
            "addEvent" => {
                self.add_event(&s(0), a(1));
            }
            "updateEvent" => {
                self.update_event(&a(0), &a(1), a(2).as_bool().unwrap_or(false));
            }
            "removeEvent" => {
                self.remove_event(&a(0), a(1).as_bool().unwrap_or(false));
            }
            "restoreEvent" => self.restore_event(&a(0)),
            "freshen" => self.freshen(a(0).as_i64().unwrap_or(0)),
            "openKeep" => h.ask("launch.open", json!(["https://keep.google.com/"])),
            _ => {}
        }
        self.publish();
    }
    fn message(&mut self, h: &mut Host, msg: Box<dyn Any + Send>) {
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Line(line) => {
                self.replied = true;
                let Ok(m) = serde_json::from_str::<Value>(&line) else {
                    self.last_error = "Could not read the Google helper response.".into();
                    self.publish();
                    return;
                };
                if m["signingIn"].as_bool().unwrap_or(false) {
                    self.signing_in = true;
                }
                if let Some(u) = m["authUrl"].as_str().filter(|u| u.starts_with("https://accounts.google.com/o/oauth2/v2/auth?")) {
                    self.signing_in = true;
                    h.ask("launch.open", json!([u]));
                }
                if m["data"].is_object() || (!m["data"].is_null() && m["data"] != false) {
                    self.account = m["data"].clone();
                    // A destination whose account went away: back to this computer
                    if ["disconnect", "forget"].contains(&self.operation.as_str()) {
                        let lists = self.arr("taskLists");
                        if find_ref(&lists, &self.task_list_id).is_none() {
                            self.task_list_id.clear();
                            config_set("google.taskListId", json!(""));
                        }
                        let cals = self.arr("calendars");
                        if find_ref(&cals, &self.new_event_cal).is_none() {
                            self.new_event_cal.clear();
                            config_set("google.newEventCalendar", json!(""));
                        }
                    }
                }
                if m["mutationComplete"].as_bool().unwrap_or(false) {
                    self.mutation_seq += 1;
                }
                if m["conflict"].is_object() {
                    Self::conflict(&m["conflict"]);
                }
                if let Some(err) = m["error"].as_str().filter(|e| !e.is_empty()) {
                    self.last_error = err.to_string();
                    self.offline = m["offline"].as_bool().unwrap_or(false);
                    // (offline: the change is kept and shown; no alarm for that)
                    if !self.offline && !["sync", "status", "flush"].contains(&self.operation.as_str()) {
                        let cmd = strs(&["notify-send", "-a", "Tessera", "Google", err]);
                        std::thread::spawn(move || {
                            let _ = run_full(&cmd, None);
                        });
                    }
                }
            }
            M::Exit(code) => {
                self.running = false;
                self.signing_in = false;
                if code != 0 && !self.replied {
                    self.last_error = "Google helper failed. Check that python3 and python-gobject are installed.".into();
                }
                // The write is in Google's data now (or failed, and is shown as
                // Google has it): its override goes either way
                if let Some(cur) = self.current.take() {
                    if !cur.key.is_empty() {
                        self.set_override(&cur.key, None);
                    }
                }
                // A failed sync waits longer each time before the automatic ones try again
                if self.operation == "sync" || self.operation == "status" {
                    if !self.last_error.is_empty() && self.connected() {
                        self.failures += 1;
                        // 1, 4, 16, 64 min, then 2 h
                        let ms = (60_000f64 * 4f64.powi(self.failures as i32 - 1)).min(2.0 * 3_600_000.0);
                        self.retry_at = now_ms() + ms as i64;
                    } else if self.last_error.is_empty() {
                        self.failures = 0;
                        self.retry_at = 0;
                    }
                }
                // Changes still waiting (offline): tried again later
                if self.outbox() > 0 && self.queue.is_empty() {
                    if self.offline {
                        self.send_failures += 1;
                    }
                    self.arm_resend();
                } else if self.outbox() == 0 {
                    self.send_failures = 0;
                    self.resend.stop();
                }
                if self.operation == "status" && self.connected() && self.queue.is_empty() {
                    self.freshen(2 * 60_000);
                } else {
                    self.pump();
                }
            }
            M::Resend => {
                if self.outbox() > 0 && !self.running && self.queue.is_empty() {
                    self.start_request(json!({"cmd": "flush"}));
                } else if self.outbox() > 0 {
                    self.arm_resend();
                }
            }
            M::Poll => self.poll(h),
        }
        self.publish();
    }
}

impl Google {
    fn read_cfg(&mut self, cfg: &Value) {
        self.hidden = cfg["google"]["hiddenCalendars"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default();
        self.task_list_id = js(&cfg["google"], "taskListId");
        self.new_event_cal = js(&cfg["google"], "newEventCalendar");
    }
    /// Once a second: deletions whose Undo ran out, and the moments a sync is useful
    fn poll(&mut self, h: &mut Host) {
        let now = now_ms();
        let due: Vec<Deleting> = {
            let (due, keep): (Vec<_>, Vec<_>) = std::mem::take(&mut self.deleting).into_iter().partition(|d| now - d.at >= 8000);
            self.deleting = keep;
            due
        };
        for d in due {
            self.write(d.request, &d.key, Some(json!({"hidden": true})));
        }
        let lk = locked();
        let id = idle_long();
        if self.was_locked && !lk {
            self.freshen(2 * 60_000); // unlocked, or woke up (sleep locks first)
        }
        if self.was_idle && !id {
            self.freshen(15 * 60_000);
        }
        let nc = h.state("open").as_str() == Some("notificationCenter");
        if nc && !self.was_nc {
            self.freshen(2 * 60_000);
        }
        (self.was_locked, self.was_idle, self.was_nc) = (lk, id, nc);
        self.ticks += 1;
        // looks every minute; syncs when 15 have passed
        if self.ticks % 60 == 0 && self.connected() && !lk && !id {
            self.freshen(15 * 60_000);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keys_refs_and_fields() {
        assert_eq!(task_key(&json!({"account": "a", "listId": "l", "id": "1"})), "google/a/l/1");
        assert_eq!(event_key(&json!({"account": "a", "calendarId": "c", "id": "e"})), "google:a:c:e");
        let rows = vec![json!({"account": "a1", "id": "x"}), json!({"account": "a2", "id": "x"})];
        assert_eq!(find_ref(&rows, "a2|x"), Some(&rows[1]));
        assert_eq!(find_ref(&rows, "x"), Some(&rows[0]));
        assert!(find_ref(&rows, "").is_none() && find_ref(&rows, "a3|x").is_none());
        let f = shown_fields(&json!({"title": "T", "start": 0, "allDay": false}));
        assert_eq!(f["start"]["dateTime"], "1970-01-01T00:00:00.000Z");
        assert_eq!(f["end"]["dateTime"], "1970-01-01T00:00:00.000Z");
        assert_eq!(start_of(&json!({"start": {"dateTime": "1970-01-01T00:00:01Z"}})), 1000);
    }
    #[test]
    fn overrides_hide_and_patch() {
        let mut g = Google::default();
        g.account = json!({"tasks": [{"account": "a", "listId": "l", "id": "1", "text": "x", "date": ""}, {"account": "a", "listId": "l", "id": "2", "text": "y"}]});
        g.set_override("google/a/l/1", Some(json!({"patch": {"text": "z"}})));
        g.set_override("google/a/l/2", Some(json!({"hidden": true})));
        let t = g.tasks();
        assert_eq!(t.len(), 1);
        assert_eq!(t[0]["text"], "z");
        assert_eq!(t[0]["google"], true);
    }
}

//! CloudAccounts.qml: iCloud (calendars, contacts) and Microsoft 365
//! (Outlook calendars, Microsoft To Do), through assets/scripts/
//! cloud-accounts.py: one request per run, its state back. Changes show at
//! once and go to the helper in turn; deleting hides first and is sent when
//! its Undo (8 s) has run out. Syncs: at login (20 s after start), on
//! unlock, when Notification Center opens after 2 minutes, every 15 minutes
//! in use; never while locked or idle. The Apple ID password and every token
//! only ever go to the helper's stdin; nothing here logs them.
//!
//! Bus "cloudaccounts": { state, accounts, hasICloud, hasMicrosoft,
//! calendars, taskLists, contacts, syncedAt, busy, lastError, offline,
//! signingIn, signingInto, errorFrom, tasks (override-applied), events
//! (the QML's `parsed`: [{raw, cal, key, ics?, notes, hidden, patch}]; the
//! desktop group's Calendar expands the iCalendar text and applies
//! hidden/patch, as CloudAccounts.between did) }.
//! Calls: cloudaccounts.addEvent [account, calendar, fields] | updateEvent
//! [e, fields] | removeEvent [e] | restoreEvent [e] | addTask [ref, item] |
//! updateTask [t, patch] | removeTask [t] | restoreTask [t] | connectICloud
//! [appleId, password] | connectMicrosoft | disconnect [id] | sync |
//! freshen [age] | searchContacts [q, limit] is read from the bus' contacts.

use super::common::*;
use serde_json::{json, Map, Value};
use std::any::Any;
use std::time::Duration;
use tessera_core::host::{Host, Service};

enum M {
    Done(String),
    MsDone(String),
    Start,
    Poll,
}

struct Req {
    request: Value,
    key: String,
}

struct Deleting {
    key: String,
    request: Value,
    at: i64,
}

#[derive(Default)]
pub struct CloudAccounts {
    tx: Option<Tx>,
    state: Value,
    last_error: String,
    error_from: String,
    offline: bool,
    signing_in: bool,
    signing_into: String,
    overrides: Map<String, Value>,
    deleting: Vec<Deleting>,
    queue: Vec<Req>,
    current: Option<Req>,
    running: bool,
    ms_running: bool,
    hidden: Vec<String>,
    was_locked: bool,
    was_nc: bool,
    ticks: u32,
    start_t: slint::Timer,
    poll: slint::Timer,
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

/// keyOf(raw)
pub fn key_of(raw: &Value) -> String {
    let id = if raw["href"].is_null() { js(raw, "id") } else { js(raw, "href") };
    format!("cloud:{}:{}", js(raw, "account"), id)
}
pub fn task_key(t: &Value) -> String {
    format!("cloud/{}/{}/{}", js(t, "account"), js(t, "listId"), js(t, "id"))
}

/// descriptionOf(ics): the DESCRIPTION line, unfolded and unescaped
pub fn description_of(ics: &str) -> String {
    let flat = ics.replace("\r\n ", "").replace("\r\n\t", "").replace("\n ", "").replace("\n\t", "");
    for line in flat.lines() {
        if let Some(rest) = line.strip_prefix("DESCRIPTION") {
            if let Some((name, value)) = rest.split_once(':') {
                if !name.contains('\n') {
                    let v = value.replace("\\n", "\n").replace("\\N", "\n");
                    let mut out = String::new();
                    let mut it = v.chars().peekable();
                    while let Some(c) = it.next() {
                        if c == '\\' && matches!(it.peek(), Some(',' | ';' | '\\')) {
                            out.push(it.next().unwrap_or('\\'));
                        } else {
                            out.push(c);
                        }
                    }
                    return out.trim().to_string();
                }
            }
        }
    }
    String::new()
}

impl CloudAccounts {
    fn tx(&self) -> Tx {
        self.tx.clone().expect("started")
    }
    fn arr(&self, k: &str) -> Vec<Value> {
        self.state[k].as_array().cloned().unwrap_or_default()
    }
    fn account(&self, id: &str) -> Option<Value> {
        self.arr("accounts").into_iter().find(|a| a["id"].as_str() == Some(id))
    }
    fn calendar(&self, account: &str, id: &str) -> Option<Value> {
        self.arr("calendars").into_iter().find(|c| c["account"].as_str() == Some(account) && c["id"].as_str() == Some(id))
    }
    fn shown(&self, c: &Value) -> bool {
        !self.hidden.contains(&format!("{}|{}", js(c, "account"), js(c, "id")))
    }
    fn synced_at(&self) -> i64 {
        self.state["syncedAt"].as_f64().unwrap_or(0.0) as i64
    }
    fn set_override(&mut self, key: &str, o: Option<Value>) {
        match o {
            Some(v) => self.overrides.insert(key.to_string(), v),
            None => self.overrides.remove(key),
        };
    }
    fn patch_of(&self, key: &str) -> Value {
        self.overrides.get(key).map(|o| o["patch"].clone()).unwrap_or(Value::Null)
    }
    fn provider_of(&self, request: &Value) -> String {
        match request["cmd"].as_str().unwrap_or("") {
            "connectICloud" => "icloud".into(),
            "connectMicrosoft" => "microsoft".into(),
            _ => request["account"].as_str().and_then(|a| self.account(a)).map(|a| js(&a, "provider")).unwrap_or_default(),
        }
    }

    fn events(&self) -> Vec<Value> {
        self.arr("events")
            .into_iter()
            .filter_map(|e| {
                let cal = self.calendar(e["account"].as_str().unwrap_or(""), e["calendarId"].as_str().unwrap_or(""))?;
                if !self.shown(&cal) {
                    return None;
                }
                let key = key_of(&e);
                let o = self.overrides.get(&key);
                let (ics, notes) = match e["ics"].as_str() {
                    Some(i) => (Value::from(i), description_of(i)),
                    None => (Value::Null, js(&e, "notes")),
                };
                Some(json!({"raw": e, "cal": cal, "key": key, "ics": ics, "notes": notes,
                            "hidden": o.is_some_and(|o| o["hidden"].as_bool().unwrap_or(false)),
                            "patch": o.map(|o| o["patch"].clone()).unwrap_or(Value::Null)}))
            })
            .collect()
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
                let base = merge(t, &json!({"key": key, "cloud": true, "allDay": true, "due": due}));
                Some(merge(&base, &o.map(|o| o["patch"].clone()).unwrap_or(Value::Null)))
            })
            .collect()
    }
    fn publish(&self) {
        let accounts = self.arr("accounts");
        let has = |p: &str| accounts.iter().any(|a| a["provider"].as_str() == Some(p));
        put(
            "cloudaccounts",
            json!({"state": self.state, "accounts": accounts, "hasICloud": has("icloud"), "hasMicrosoft": has("microsoft"),
                   "calendars": self.arr("calendars"), "taskLists": self.arr("taskLists"), "contacts": self.arr("contacts"),
                   "syncedAt": self.synced_at(), "busy": self.running, "lastError": self.last_error, "offline": self.offline,
                   "signingIn": self.signing_in, "signingInto": self.signing_into, "errorFrom": self.error_from,
                   "tasks": self.tasks(), "events": self.events(), "queued": self.queue.len()}),
        );
    }

    // ── The helper: one request at a time ───────────────────────────────
    fn send(&mut self, request: Value, key: &str) {
        self.queue.push(Req { request, key: key.to_string() });
        self.pump();
    }
    fn pump(&mut self) {
        if self.running || self.queue.is_empty() {
            return;
        }
        let next = self.queue.remove(0);
        let request = self.fresh_etag(next.request);
        self.current = Some(Req { request: request.clone(), key: next.key });
        self.running = true;
        let cmd = vec!["python3".to_string(), "-I".into(), script("cloud-accounts.py")];
        let tx = self.tx();
        std::thread::spawn(move || {
            let (_, out, _) = run_full(&cmd, Some(&format!("{}\n", request)));
            tx.send(M::Done(out));
        });
    }
    /// A change waits its turn: by then an earlier one may have changed the
    /// event's version (etag), which the server checks
    fn fresh_etag(&self, r: Value) -> Value {
        if r["etag"].is_null() {
            return r;
        }
        let cmd = r["cmd"].as_str().unwrap_or("");
        let cur = if cmd.starts_with("event") {
            self.arr("events").into_iter().find(|e| e["account"] == r["account"] && e["calendarId"] == r["calendarId"] && (if e["href"].is_null() { &e["id"] } else { &e["href"] }) == &r["id"])
        } else {
            self.arr("tasks").into_iter().find(|t| t["account"] == r["account"] && t["listId"] == r["listId"] && t["id"] == r["id"])
        };
        match cur {
            Some(c) => merge(&r, &json!({"etag": c["etag"].as_str().unwrap_or("")})),
            None => r,
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
    fn freshen(&mut self, age: i64) {
        if self.arr("accounts").is_empty() || self.running || !self.queue.is_empty() || locked() {
            return;
        }
        if now_ms() - self.synced_at() >= age {
            self.send(json!({"cmd": "sync"}), "");
        }
    }
    /// the raw event of an `e` the QML passes (its `source`)
    fn raw(e: &Value) -> Value {
        if e["source"].is_object() { e["source"].clone() } else { e.clone() }
    }
}

impl Service for CloudAccounts {
    fn name(&self) -> &'static str {
        "CloudAccounts"
    }
    fn calls(&self) -> &'static [&'static str] {
        &["cloudaccounts"]
    }
    fn start(&mut self, h: &mut Host) {
        let tx = Tx::new(h.sender("CloudAccounts"));
        self.tx = Some(tx.clone());
        self.state = json!({});
        self.read_cfg(h.cfg());
        self.send(json!({"cmd": "status"}), "");
        let t2 = tx.clone();
        self.start_t.start(slint::TimerMode::SingleShot, Duration::from_secs(20), move || t2.send(M::Start));
        self.poll.start(slint::TimerMode::Repeated, Duration::from_secs(1), move || tx.send(M::Poll));
        self.publish();
    }
    fn config_changed(&mut self, h: &mut Host) {
        self.read_cfg(h.cfg());
        self.publish();
    }
    fn call(&mut self, _h: &mut Host, call: &str, args: &Value) {
        let a = |i: usize| args.get(i).cloned().unwrap_or(Value::Null);
        let s = |i: usize| args.get(i).and_then(Value::as_str).unwrap_or("").to_string();
        match call.split('.').nth(1).unwrap_or("") {
            "addEvent" => {
                let (acc, cal) = (s(0), s(1));
                if !self.calendar(&acc, &cal).is_some_and(|c| c["writable"].as_bool().unwrap_or(false)) {
                    self.last_error = "That calendar can't be changed from here. Choose another one.".into();
                } else {
                    self.send(json!({"cmd": "eventInsert", "account": acc, "calendarId": cal, "fields": a(2)}), "");
                }
            }
            "updateEvent" => {
                let e = a(0);
                let raw = Self::raw(&e);
                if !raw["writable"].as_bool().unwrap_or(false) {
                    self.last_error = if e["recurring"].as_bool().unwrap_or(false) { "Change repeating events on your phone or on the web." } else { "This calendar can't be changed from here." }.into();
                } else {
                    let key = key_of(&raw);
                    let fields = a(1);
                    let patch = merge(&self.patch_of(&key), &fields);
                    self.set_override(&key, Some(json!({"patch": patch})));
                    let id = if raw["href"].is_null() { raw["id"].clone() } else { raw["href"].clone() };
                    self.send(json!({"cmd": "eventPatch", "account": raw["account"], "calendarId": raw["calendarId"], "id": id, "etag": raw["etag"].as_str().unwrap_or(""), "fields": fields}), &key);
                }
            }
            "removeEvent" => {
                let e = a(0);
                let raw = Self::raw(&e);
                if !raw["writable"].as_bool().unwrap_or(false) {
                    self.last_error = if e["recurring"].as_bool().unwrap_or(false) { "Delete repeating events on your phone or on the web." } else { "This calendar can't be changed from here." }.into();
                } else {
                    let id = if raw["href"].is_null() { raw["id"].clone() } else { raw["href"].clone() };
                    let req = json!({"cmd": "eventDelete", "account": raw["account"], "calendarId": raw["calendarId"], "id": id, "etag": raw["etag"].as_str().unwrap_or("")});
                    self.hide(&key_of(&raw), req);
                }
            }
            "restoreEvent" => self.unhide(&key_of(&Self::raw(&a(0)))),
            "addTask" => {
                let r = s(0);
                let (acc, list) = r.split_once('|').unwrap_or((&r, ""));
                let item = a(1);
                let date = item["due"].as_i64().filter(|d| *d != 0).map(day_of).unwrap_or_default();
                self.send(json!({"cmd": "taskInsert", "account": acc, "listId": list, "fields": {"text": item["text"], "done": item["done"].as_bool().unwrap_or(false), "date": date}}), "");
            }
            "updateTask" => {
                let (t, patch) = (a(0), a(1));
                let mut f = Map::new();
                if !patch["text"].is_null() {
                    f.insert("text".into(), patch["text"].clone());
                }
                if !patch["done"].is_null() {
                    f.insert("done".into(), patch["done"].clone());
                }
                if !patch["due"].is_null() {
                    f.insert("date".into(), json!(patch["due"].as_i64().filter(|d| *d != 0).map(day_of).unwrap_or_default()));
                }
                let key = task_key(&t);
                let shown = merge(&self.patch_of(&key), &patch);
                self.set_override(&key, Some(json!({"patch": shown})));
                self.send(json!({"cmd": "taskPatch", "account": t["account"], "listId": t["listId"], "id": t["id"], "etag": t["etag"].as_str().unwrap_or(""), "fields": f}), &key);
            }
            "removeTask" => {
                let t = a(0);
                let req = json!({"cmd": "taskDelete", "account": t["account"], "listId": t["listId"], "id": t["id"]});
                self.hide(&task_key(&t), req);
            }
            "restoreTask" => self.unhide(&task_key(&a(0))),
            "connectICloud" => {
                self.signing_in = true;
                self.signing_into = "icloud".into();
                self.send(json!({"cmd": "connectICloud", "appleId": s(0), "password": s(1)}), "");
            }
            // Microsoft: GNOME Online Accounts' Microsoft 365 sign-in (goa-signin
            // ms_graph), then the account it made
            "connectMicrosoft" => {
                if self.ms_running {
                    return;
                }
                self.signing_in = true;
                self.signing_into = "microsoft".into();
                self.last_error.clear();
                self.ms_running = true;
                let shell_dir = crate::util::root().join("shell").to_string_lossy().into_owned();
                let sh = "exe=\"$1/native/build/goa-signin\"; [ -x \"$exe\" ] || exe=$(command -v tessera-goa-signin) || { echo '{\"error\":\"Connecting needs Tessera\\u2019s native helpers. Run Software Update, then try again.\"}'; exit 1; }; exec \"$exe\" ms_graph";
                let cmd = vec!["sh".to_string(), "-c".into(), sh.into(), "sh".into(), shell_dir];
                let tx = self.tx();
                std::thread::spawn(move || {
                    let (_, out, _) = run_full(&cmd, None);
                    tx.send(M::MsDone(out));
                });
            }
            "disconnect" => self.send(json!({"cmd": "disconnect", "account": s(0)}), ""),
            "sync" => {
                if !self.arr("accounts").is_empty() {
                    self.send(json!({"cmd": "sync"}), "");
                }
            }
            "freshen" => self.freshen(a(0).as_i64().unwrap_or(0)),
            _ => {}
        }
        self.publish();
    }
    fn message(&mut self, h: &mut Host, msg: Box<dyn Any + Send>) {
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Start => self.freshen(0),
            M::MsDone(out) => {
                self.ms_running = false;
                let r: Value = out.trim().lines().last().and_then(|l| serde_json::from_str(l).ok()).unwrap_or(json!({}));
                if r["account"].is_string() || r["account"].is_number() {
                    self.send(json!({"cmd": "connectMicrosoft", "goa": r["account"], "identity": r["identity"].as_str().unwrap_or("")}), "");
                } else {
                    self.signing_in = false;
                    if !r["cancelled"].as_bool().unwrap_or(false) {
                        self.last_error = r["error"].as_str().filter(|e| !e.is_empty()).unwrap_or("Couldn't open the Microsoft sign-in. Update Tessera, then try again.").to_string();
                        self.error_from = "microsoft".into();
                    }
                }
            }
            M::Done(out) => {
                let s: Option<Value> = out.trim().lines().last().and_then(|l| serde_json::from_str(l).ok());
                let done = self.current.take();
                if let Some(s) = s {
                    self.offline = s["offline"].as_bool().unwrap_or(false);
                    self.last_error = s["error"].as_str().unwrap_or("").to_string();
                    self.error_from = done.as_ref().map(|d| self.provider_of(&d.request)).unwrap_or_default();
                    let cmd = done.as_ref().map(|d| js(&d.request, "cmd")).unwrap_or_default();
                    if !self.last_error.is_empty() && (cmd.starts_with("event") || cmd.starts_with("task")) {
                        let c = strs(&["notify-send", "-a", "Tessera", "Change not saved", &self.last_error]);
                        std::thread::spawn(move || {
                            let _ = run_full(&c, None);
                        });
                    }
                    self.state = s;
                } else {
                    self.last_error = "The calendar helper didn't answer. Check that Python 3 is installed.".into();
                }
                // the server's version is shown now (unless more changes to it
                // are still on their way, or it's waiting to be deleted)
                if let Some(d) = &done {
                    if !d.key.is_empty() && !self.queue.iter().any(|q| q.key == d.key) && !self.deleting.iter().any(|x| x.key == d.key) {
                        let k = d.key.clone();
                        self.set_override(&k, None);
                    }
                    if ["connectICloud", "connectMicrosoft"].contains(&js(&d.request, "cmd").as_str()) {
                        self.signing_in = false;
                    }
                }
                self.running = false;
                self.pump();
            }
            M::Poll => {
                let now = now_ms();
                let (due, keep): (Vec<_>, Vec<_>) = std::mem::take(&mut self.deleting).into_iter().partition(|d| now - d.at >= 8000);
                self.deleting = keep;
                for d in due {
                    self.send(d.request, &d.key);
                }
                let lk = locked();
                if self.was_locked && !lk {
                    self.freshen(2 * 60_000);
                }
                let nc = h.state("open").as_str() == Some("notificationCenter");
                if nc && !self.was_nc {
                    self.freshen(2 * 60_000);
                }
                (self.was_locked, self.was_nc) = (lk, nc);
                self.ticks += 1;
                if self.ticks % 60 == 0 && !self.arr("accounts").is_empty() && !lk && !idle_long() {
                    self.freshen(15 * 60_000);
                }
            }
        }
        self.publish();
    }
}

impl CloudAccounts {
    fn read_cfg(&mut self, cfg: &Value) {
        self.hidden = cfg["cloud"]["hiddenCalendars"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn descriptions_unfold_and_unescape() {
        let ics = "BEGIN:VEVENT\r\nSUMMARY:x\r\nDESCRIPTION:Line one\\nline\r\n  two\\, ok\r\nEND:VEVENT";
        assert_eq!(description_of(ics), "Line one\nline two, ok");
        assert_eq!(description_of("BEGIN:VEVENT\nEND:VEVENT"), "");
    }
    #[test]
    fn keys() {
        assert_eq!(key_of(&json!({"account": "a", "href": "/h"})), "cloud:a:/h");
        assert_eq!(key_of(&json!({"account": "a", "id": "7"})), "cloud:a:7");
        assert_eq!(task_key(&json!({"account": "a", "listId": "l", "id": "1"})), "cloud/a/l/1");
    }
}

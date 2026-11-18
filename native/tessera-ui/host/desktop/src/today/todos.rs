//! Todos.qml: your to-dos with due dates (Notification Center's To-do, the
//! Reminders widget, Today). Local ones live in state.json's `todos`
//! ([{ id, text, done, due?, allDay?, notified? }]); keys are "local/<id>".
//! The accounts' tasks (Google Tasks "google/…", Microsoft To Do and iCloud
//! Reminders "cloud/…") come after them (core services/accounts.rs); adding,
//! ticking, renaming, moving and removing one asks the system host's
//! service (`google.addTask|updateTask|removeTask|restoreTask`,
//! `cloudaccounts.…`), which shows the change at once and sends it, keeping
//! it while offline. New to-dos go where Settings → Calendar says
//! (`google.taskListId`: "" this computer, "<account>|<list>" Google,
//! "cloud|<account>|<list>" Microsoft or iCloud). Timed to-dos notify once
//! when they come due (every 30 s, as the QML timer).
//!
//! Calls: todos.toggle [key] (Today), todo [key] (the desktop widget),
//! todoAdd [text] (the widget's add box).

use super::common::*;
use super::parse;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::sync::Mutex;
use tessera_core::host::Host;
use tessera_core::services::accounts;
use tessera_core::services::calendar as cal;

/// One to-do as the surfaces and widgets read it
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub key: String,
    pub text: String,
    pub done: bool,
    pub due: i64,
    pub all_day: bool,
}

pub fn local_list() -> Vec<Value> {
    state_get("todos").as_array().cloned().unwrap_or_default()
}

fn save(list: Vec<Value>) {
    state_set("todos", Value::Array(list));
}

fn num(v: &Value) -> i64 {
    v.as_f64().map_or(0, |f| f as i64)
}

/// Todos.list: yours, then the accounts'
pub fn list() -> Vec<Item> {
    let mut out: Vec<Item> = local_list()
        .iter()
        .filter(|t| t.is_object())
        .map(|t| Item {
            key: format!("local/{}", t["id"].as_str().unwrap_or("")),
            text: t["text"].as_str().unwrap_or("").to_string(),
            done: t["done"].as_bool().unwrap_or(false),
            due: num(&t["due"]),
            all_day: t["allDay"].as_bool().unwrap_or(false),
        })
        .collect();
    out.extend(accounts::current().tasks.iter().map(|t| Item { key: t.todo.key.clone(), text: t.todo.text.clone(), done: t.todo.done, due: t.todo.due, all_day: t.todo.all_day }));
    out
}

/// To-dos saved before keys existed get theirs once
pub fn give_ids() {
    let l = local_list();
    if l.iter().any(|t| t.is_object() && t["id"].as_str().unwrap_or("").is_empty()) {
        save(
            l.into_iter()
                .map(|mut t| {
                    if t.is_object() && t["id"].as_str().unwrap_or("").is_empty() {
                        t["id"] = json!(new_id());
                    }
                    t
                })
                .collect(),
        );
    }
}

fn id_of(key: &str) -> Option<&str> {
    key.strip_prefix("local/")
}

pub fn find(key: &str) -> Option<Item> {
    list().into_iter().find(|t| t.key == key)
}

/// Todos.add(text): parse "text [when]" and keep it; a list that can't take
/// it says why in a notification
pub fn add(h: &mut Host, text: &str) -> bool {
    let p = parse::parse_todo(text, now());
    if p.text.is_empty() {
        return false;
    }
    let mut item = json!({"id": new_id(), "text": p.text, "done": false});
    if p.due != 0 {
        item["due"] = json!(p.due);
        item["allDay"] = json!(p.all_day);
    }
    match add_item(h, item) {
        Ok(()) => true,
        Err(why) => {
            notify_send("Reminders", None, None, "Task not added", &why);
            false
        }
    }
}

const GOOGLE_TIMES: &str = "Google Tasks keeps a date but not a time. Leave the time out, or send new to-dos to This computer in Settings → Calendar.";

/// "yyyy-mm-dd" of a due time ("" undated)
fn date_of(due: i64) -> String {
    if due == 0 { String::new() } else { tessera_core::util::strftime("%Y-%m-%d", due.div_euclid(1000)) }
}

/// The task list `google.taskListId` names: Google's "<account>|<id>" (a
/// bare id: the first account's list with it) or "cloud|<account>|<id>"
pub fn destination(d: &accounts::Data, setting: &str) -> Option<accounts::Target> {
    if let Some(t) = d.list(setting) {
        return Some(t.clone());
    }
    (!setting.contains('|')).then(|| d.lists.iter().find(|l| l.provider == "google" && l.value.ends_with(&format!("|{setting}"))).cloned()).flatten()
}

/// What adding `item` to the list `setting` asks of the system host, or why it can't
pub fn add_request(d: &accounts::Data, setting: &str, item: &Value) -> Result<(String, Value), String> {
    let due = num(&item["due"]);
    let all_day = item["allDay"].as_bool().unwrap_or(false);
    let Some(list) = destination(d, setting) else {
        return Err(if setting.starts_with("cloud|") {
            "That Microsoft To Do or iCloud Reminders list isn't available. Choose another list in Settings → Calendar, or This computer.".into()
        } else if !d.google {
            "Google isn't connected. Connect it in Settings → Google, or send new to-dos to This computer in Settings → Calendar.".into()
        } else {
            "That Google task list isn't available any more. Choose another in Settings → Calendar, or This computer.".into()
        });
    };
    let done = item["done"].as_bool().unwrap_or(false);
    let text = item["text"].as_str().unwrap_or("");
    if let Some(r) = list.value.strip_prefix("cloud|") {
        // (Microsoft To Do keeps the date; iCloud Reminders the time too)
        let mut it = json!({"text": text, "done": done, "due": due, "allDay": all_day || due == 0});
        if due != 0 && !all_day && list.provider == "icloud" {
            it["dueAt"] = json!(due);
        }
        return Ok(("cloudaccounts.addTask".into(), json!([r, it])));
    }
    if due != 0 && !all_day {
        return Err(GOOGLE_TIMES.into());
    }
    Ok(("google.addTask".into(), json!([list.value, {"text": text, "done": done, "date": date_of(due)}])))
}

/// Todos.addItem: to the list new tasks go to (this computer unless Settings
/// says a Google, Microsoft or iCloud list)
pub fn add_item(h: &mut Host, item: Value) -> Result<(), String> {
    let dest = cfg_get("google.taskListId");
    let dest = dest.as_str().unwrap_or("");
    if !dest.is_empty() {
        let (call, args) = add_request(&accounts::current(), dest, &item)?;
        h.ask(&call, args);
        return Ok(());
    }
    let mut l = local_list();
    let mut item = item;
    if item["id"].as_str().unwrap_or("").is_empty() {
        item["id"] = json!(new_id());
    }
    l.push(item);
    save(l);
    Ok(())
}

/// What changing an account's task asks of the system host, or why it can't
pub fn update_request(d: &accounts::Data, key: &str, patch: &Value) -> Result<(String, Value), String> {
    let t = d.task(key).ok_or("That task is gone. It may have been deleted on another device.")?;
    if t.provider == "google" {
        if num(&patch["due"]) != 0 && patch["allDay"] == false {
            return Err(GOOGLE_TIMES.into());
        }
        return Ok(("google.updateTask".into(), json!([t.source, patch])));
    }
    let mut p = patch.clone();
    if t.provider == "icloud" && num(&patch["due"]) != 0 && patch["allDay"] == false {
        p["dueAt"] = patch["due"].clone();
    }
    Ok(("cloudaccounts.updateTask".into(), json!([t.source, p])))
}

/// Todos.update(key, patch); a new time notifies again
pub fn update(h: &mut Host, key: &str, patch: &Value) -> Result<(), String> {
    let Some(id) = id_of(key) else {
        let (call, args) = update_request(&accounts::current(), key, patch)?;
        h.ask(&call, args);
        if patch.get("due").is_some() {
            notified().retain(|k| k != key);
        }
        return Ok(());
    };
    let mut l = local_list();
    let Some(t) = l.iter_mut().find(|t| t["id"].as_str() == Some(id)) else { return Err("That to-do is gone.".into()) };
    if let Value::Object(p) = patch {
        for (k, v) in p {
            t[k] = v.clone();
        }
        if p.contains_key("due") {
            if let Some(o) = t.as_object_mut() {
                o.remove("notified");
            }
        }
    }
    save(l);
    Ok(())
}

pub fn toggle(h: &mut Host, key: &str) {
    if let Some(t) = find(key) {
        if let Err(why) = update(h, key, &json!({"done": !t.done})) {
            notify_send("Reminders", None, None, "Task not changed", &why);
        }
    }
}

/// Todos.remove(key): the to-do with its place, for Undo (an account's is
/// hidden now and deleted when its Undo has run out: the system host's 8 s)
pub fn remove(h: &mut Host, key: &str) -> Option<Value> {
    let Some(id) = id_of(key) else {
        let d = accounts::current();
        let t = d.task(key)?;
        let call = if t.provider == "google" { "google.removeTask" } else { "cloudaccounts.removeTask" };
        h.ask(call, json!([t.source]));
        return Some(json!({"key": key, "text": t.todo.text, "account": t.provider, "source": t.source}));
    };
    let l = local_list();
    let at = l.iter().position(|t| t["id"].as_str() == Some(id))?;
    let mut gone = l[at].clone();
    save(l.iter().enumerate().filter(|(i, _)| *i != at).map(|(_, t)| t.clone()).collect());
    gone["at"] = json!(at);
    Some(gone)
}

/// Todos.restore(removed)
pub fn restore(h: &mut Host, t: &Value) {
    if let Some(p) = t["account"].as_str() {
        let call = if p == "google" { "google.restoreTask" } else { "cloudaccounts.restoreTask" };
        h.ask(call, json!([t["source"]]));
        return;
    }
    let mut item = t.clone();
    let at = t["at"].as_u64().unwrap_or(u64::MAX) as usize;
    if let Some(o) = item.as_object_mut() {
        for k in ["key", "i", "at"] {
            o.remove(k);
        }
    }
    let mut l = local_list();
    let at = at.min(l.len());
    l.insert(at, item);
    save(l);
}

/// Todos.on(day): the ones due on a local day (ms of any moment in it)
pub fn on(day: i64) -> Vec<Item> {
    let (a, b) = cal::day_bounds(day);
    list().into_iter().filter(|t| t.due >= a && t.due < b).collect()
}

/// "Today 5:00 PM", "Tomorrow", "Fri", "12 Nov"; "" when undated
pub fn due_label(t: &Item, now: i64) -> String {
    if t.due == 0 {
        return String::new();
    }
    let day = |ms: i64| cal::day_bounds(ms).0;
    let diff = ((day(t.due) - day(now)) as f64 / 86_400_000.0).round() as i64;
    let secs = t.due.div_euclid(1000);
    let date = match diff {
        0 => "Today".to_string(),
        1 => "Tomorrow".to_string(),
        -1 => "Yesterday".to_string(),
        2..=6 => tessera_core::util::strftime("%a", secs),
        _ => tessera_core::util::strftime("%-d %b", secs),
    };
    if t.all_day { date } else { format!("{date} {}", hm(t.due)) }
}

pub fn overdue(t: &Item, now: i64) -> bool {
    t.due != 0 && !t.done && t.due < now
}

/// Todos.sorted: soonest due first, the undated after in the order added
pub fn sorted(mut items: Vec<Item>) -> Vec<Item> {
    items.sort_by_key(|t| if t.due != 0 { t.due } else { i64::MAX });
    items
}

/// The accounts' timed tasks that said "Due now" this session (theirs
/// can't carry `notified` the way state.json's do)
fn notified() -> std::sync::MutexGuard<'static, HashSet<String>> {
    static N: std::sync::OnceLock<Mutex<HashSet<String>>> = std::sync::OnceLock::new();
    N.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner())
}

/// The 30 s timer: timed to-dos notify once when they come due
pub fn check_due() {
    let now = now();
    let l = local_list();
    let mut changed = false;
    let l: Vec<Value> = l
        .into_iter()
        .map(|mut t| {
            let due = num(&t["due"]);
            if due != 0 && !t["done"].as_bool().unwrap_or(false) && !t["allDay"].as_bool().unwrap_or(false) && !t["notified"].as_bool().unwrap_or(false) && due <= now {
                changed = true;
                if now - due < 3_600_000 {
                    // (not for ones long past)
                    notify_send("Reminders", Some("checkbox"), None, t["text"].as_str().unwrap_or(""), "Due now");
                }
                t["notified"] = json!(true);
            }
            t
        })
        .collect();
    if changed {
        save(l);
    }
    // (iCloud Reminders can have a time too)
    for t in accounts::current().tasks.iter().map(|t| &t.todo) {
        if t.due != 0 && !t.done && !t.all_day && t.due <= now && notified().insert(t.key.clone()) && now - t.due < 3_600_000 {
            notify_send("Reminders", Some("checkbox"), None, &t.text, "Due now");
        }
    }
}

/// The rows Notification Center's To-do shows ({ key, text, done, due, overdue })
pub fn rows(now: i64) -> Vec<Value> {
    list()
        .iter()
        .map(|t| json!({"key": t.key, "text": t.text, "done": t.done, "due": due_label(t, now), "overdue": overdue(t, now)}))
        .collect()
}

/// A call from a surface
pub fn call(h: &mut Host, call: &str, args: &Value) {
    let a = args_of(args);
    let first = a.first().and_then(Value::as_str).unwrap_or("").to_string();
    match call {
        "todos.toggle" | "todo" => toggle(h, &first),
        // (todoAdd: the Desktop widget's field; todos.add / todos.addText: the shell's menu and Spotlight)
        "todoAdd" | "todos.add" | "todos.addText" => {
            if !first.trim().is_empty() {
                add(h, &first);
            }
        }
        other => eprintln!("tessera-ui: host: no to-do call {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_core::services::accounts::{Data, Target, Task};
    use tessera_core::services::todos::Todo;

    #[test]
    fn labels_say_when() {
        let now = cal::make(2026, 10, 6, 10, 0, 0);
        let t = |due, all_day| Item { key: "local/a".into(), text: "x".into(), done: false, due, all_day };
        assert_eq!(due_label(&t(0, false), now), "");
        assert_eq!(due_label(&t(cal::make(2026, 10, 7, 0, 0, 0), true), now), "Tomorrow");
        assert_eq!(due_label(&t(cal::make(2026, 10, 5, 0, 0, 0), true), now), "Yesterday");
        assert!(overdue(&t(now - 1, false), now) && !overdue(&t(now + 1, false), now));
        let s = sorted(vec![t(0, false), t(5, false)]);
        assert_eq!(s[0].due, 5);
    }

    fn data() -> Data {
        let task = |key: &str, provider: &str| Task {
            todo: Todo { key: key.into(), text: "Pay rent".into(), ..Default::default() },
            provider: provider.into(),
            list: "L".into(),
            source: json!({"key": key, "account": "a", "listId": "l", "id": "1"}),
        };
        Data {
            google: true,
            lists: vec![
                Target { value: "g1|L1".into(), label: "My Tasks".into(), provider: "google".into() },
                Target { value: "cloud|ms|l1".into(), label: "Tasks · To Do".into(), provider: "microsoft".into() },
                Target { value: "cloud|ic|/rem/".into(), label: "Errands · Reminders".into(), provider: "icloud".into() },
            ],
            tasks: vec![task("google/g1/L1/1", "google"), task("cloud/ic//rem//1", "icloud")],
            ..Default::default()
        }
    }

    #[test]
    fn adding_to_an_account_list_asks_its_service() {
        let d = data();
        let day = cal::make(2026, 10, 20, 23, 59, 0);
        let (call, args) = add_request(&d, "g1|L1", &json!({"text": "Pay rent", "due": day, "allDay": true})).unwrap();
        assert_eq!((call.as_str(), args[0].as_str(), args[1]["date"].as_str()), ("google.addTask", Some("g1|L1"), Some("2026-11-20")));
        // (a bare Google list id, as older settings kept it)
        assert_eq!(add_request(&d, "L1", &json!({"text": "x"})).unwrap().1[0], "g1|L1");
        // Google has no times: says what to do instead
        let timed = json!({"text": "Call", "due": day, "allDay": false});
        assert!(add_request(&d, "g1|L1", &timed).unwrap_err().contains("Settings → Calendar"));
        // iCloud keeps the time, Microsoft the date
        let (call, args) = add_request(&d, "cloud|ic|/rem/", &timed).unwrap();
        assert_eq!((call.as_str(), args[0].as_str(), args[1]["dueAt"].as_i64()), ("cloudaccounts.addTask", Some("ic|/rem/"), Some(day)));
        assert!(add_request(&d, "cloud|ms|l1", &timed).unwrap().1[1].get("dueAt").is_none());
        // a list that went away, Google signed out
        assert!(add_request(&d, "cloud|gone|x", &timed).unwrap_err().contains("isn't available"));
        assert!(add_request(&Data::default(), "g1|L1", &timed).unwrap_err().contains("Google isn't connected"));
    }

    #[test]
    fn changing_an_account_task_asks_its_service() {
        let d = data();
        let (call, args) = update_request(&d, "google/g1/L1/1", &json!({"done": true})).unwrap();
        assert_eq!((call.as_str(), args[1]["done"].as_bool()), ("google.updateTask", Some(true)));
        assert_eq!(args[0]["id"], "1");
        assert!(update_request(&d, "google/g1/L1/1", &json!({"due": 5, "allDay": false})).is_err());
        let (call, args) = update_request(&d, "cloud/ic//rem//1", &json!({"text": "Pay rent today", "due": 5, "allDay": false})).unwrap();
        assert_eq!((call.as_str(), args[1]["dueAt"].as_i64()), ("cloudaccounts.updateTask", Some(5)));
        assert!(update_request(&d, "google/g1/L1/9", &json!({})).unwrap_err().contains("gone"));
    }
}

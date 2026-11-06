//! Todos.qml: your to-dos with due dates (Notification Center's To-do, the
//! Reminders widget, Today). Local ones live in state.json's `todos`
//! ([{ id, text, done, due?, allDay?, notified? }]); keys are "local/<id>".
//! The accounts' tasks (Google Tasks, Microsoft To Do) are the system host's
//! (Google.qml, CloudAccounts.qml): adding to one of those lists is told to
//! the user, not done here (see PORT-NOTES). Timed to-dos notify once when
//! they come due (every 30 s, as the QML timer).
//!
//! Calls: todos.toggle [key] (Today), todo [key] (the desktop widget),
//! todoAdd [text] (the widget's add box).

use super::common::*;
use super::parse;
use serde_json::{json, Value};
use tessera_core::host::Host;
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

/// Todos.list (the local ones)
pub fn list() -> Vec<Item> {
    local_list()
        .iter()
        .filter(|t| t.is_object())
        .map(|t| Item {
            key: format!("local/{}", t["id"].as_str().unwrap_or("")),
            text: t["text"].as_str().unwrap_or("").to_string(),
            done: t["done"].as_bool().unwrap_or(false),
            due: num(&t["due"]),
            all_day: t["allDay"].as_bool().unwrap_or(false),
        })
        .collect()
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

/// Todos.add(text): parse "text [when]" and keep it
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
    add_item(h, item)
}

/// Todos.addItem: to the list new tasks go to (local unless Settings says a
/// Google or Microsoft list)
pub fn add_item(_h: &mut Host, item: Value) -> bool {
    let dest = cfg_get("google.taskListId");
    if dest.as_str().is_some_and(|d| !d.is_empty()) {
        // (those lists need the account sign-in, which the system host holds)
        notify_send(
            "Reminders",
            None,
            None,
            "Task not added",
            "That task list needs your account, which this session doesn't hold yet. Switch new tasks to Local in Settings → Google.",
        );
        return false;
    }
    let mut l = local_list();
    let mut item = item;
    if item["id"].as_str().unwrap_or("").is_empty() {
        item["id"] = json!(new_id());
    }
    l.push(item);
    save(l);
    true
}

/// Todos.update(key, patch); a new time notifies again
pub fn update(key: &str, patch: &Value) -> bool {
    let Some(id) = id_of(key) else {
        eprintln!("tessera-ui: host: no account tasks here to change ({key})");
        return false;
    };
    let mut l = local_list();
    let Some(t) = l.iter_mut().find(|t| t["id"].as_str() == Some(id)) else { return false };
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
    true
}

pub fn toggle(key: &str) {
    if let Some(t) = find(key) {
        update(key, &json!({"done": !t.done}));
    }
}

/// Todos.remove(key): the to-do with its place, for Undo
pub fn remove(key: &str) -> Option<Value> {
    let id = id_of(key)?;
    let l = local_list();
    let at = l.iter().position(|t| t["id"].as_str() == Some(id))?;
    let mut gone = l[at].clone();
    save(l.iter().enumerate().filter(|(i, _)| *i != at).map(|(_, t)| t.clone()).collect());
    gone["at"] = json!(at);
    Some(gone)
}

/// Todos.restore(removed)
pub fn restore(t: &Value) {
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
        "todos.toggle" | "todo" => toggle(&first),
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
}

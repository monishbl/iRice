//! Your to-dos, as shell/services/Todos.qml keeps them on this computer
//! (UiState's `todos` in $XDG_STATE_HOME/tessera/state.json). The file is
//! watched (its time, every second); a change sends new `Todos` (Msg::Data).
//!
//! Changing one (ticking it off) is the shell's: it owns state.json while it
//! runs and the accounts' tasks (Google Tasks, Microsoft To Do). A surface
//! asks it (`shell::ask("todos.toggle", [key])`) and sees the change here.

use crate::Msg;
use smithay_client_toolkit::reexports::calloop::channel::Sender;
use std::time::{Duration, SystemTime};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Todo {
    /// "local/<id>" (Todos.list's key; the accounts' have their own)
    pub key: String,
    pub text: String,
    pub done: bool,
    /// ms, 0 when undated
    pub due: i64,
    pub all_day: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Todos {
    pub list: Vec<Todo>,
}

/// Todos.sorted: soonest due first, the undated after in the order added
pub fn sorted(mut items: Vec<Todo>) -> Vec<Todo> {
    items.sort_by_key(|t| if t.due != 0 { t.due } else { i64::MAX });
    items
}

/// One to-do from JSON (state.json's, or the shell's for an account's task)
pub fn todo_from(t: &serde_json::Value, key: String) -> Todo {
    Todo {
        key,
        text: t["text"].as_str().unwrap_or("").to_string(),
        done: t["done"].as_bool().unwrap_or(false),
        due: t["due"].as_f64().map_or(0, |f| f as i64),
        all_day: t["allDay"].as_bool().unwrap_or(false),
    }
}

/// state.json's to-dos
pub fn read(text: &str) -> Todos {
    let v: serde_json::Value = serde_json::from_str(text).unwrap_or_default();
    let list = v["todos"]
        .as_array()
        .map(|a| a.iter().filter(|t| t.is_object()).map(|t| todo_from(t, format!("local/{}", t["id"].as_str().unwrap_or("")))).collect())
        .unwrap_or_default();
    Todos { list }
}

pub fn spawn(tx: Sender<Msg>) {
    std::thread::spawn(move || {
        let path = crate::util::state_dir().join("state.json");
        let mtime = || std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        let mut seen: Option<SystemTime> = None;
        let mut first = true;
        loop {
            let t = mtime();
            if first || t != seen {
                first = false;
                seen = t;
                let todos = std::fs::read_to_string(&path).map(|s| read(&s)).unwrap_or_default();
                if tx.send(Msg::Data(Box::new(todos))).is_err() {
                    return;
                }
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn todos_from_the_state_file_soonest_first() {
        let t = read(r#"{"todos":[{"id":"a","text":"water plants","done":false},{"id":"b","text":"Buy milk","done":false,"due":1793277000000,"allDay":false,"notified":true},{"id":"c","text":"Ship","done":true}],"overlay":{}}"#);
        assert_eq!(t.list.len(), 3);
        assert_eq!(t.list[1].key, "local/b");
        let open: Vec<_> = sorted(t.list.into_iter().filter(|x| !x.done).collect()).into_iter().map(|x| x.text).collect();
        assert_eq!(open, vec!["Buy milk", "water plants"]);
        assert!(read("{}").list.is_empty() && read("nope").list.is_empty());
    }
}

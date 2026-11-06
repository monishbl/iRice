//! Notifications.qml, the part that stays with the shell while the
//! notifications surface serves (it owns org.freedesktop.Notifications, the
//! banners, the list and Notification Center). Here: its list as it says it
//! (`{"src":"ui","kind":"notifications","list":[Note…]}`, reaching this as the
//! call `notifications`), what the shell does for each new one (the unread
//! count for the bell, the sound, the island's card, the call card), and
//! Notification Center's widgets (`{"to":"notifications","do":"widgets"}` and
//! `"calendar"`) with what it asks back (`{"kind":"nc","act":…}`, the call
//! `nc`). Hello, covered/osd and clear go to it as the QML sent them.
//!
//! Calls: notify [what, …] / notify.<what> (Settings: clearAll, play,
//! playStartup, setMode, addMode, removeMode, restoreMode, focusOn, focusOff,
//! installHyprbars), notifications (its list), nc (its acts).
//!
//! Differences: the island's duplicate checks (a notification about what the
//! island just said, or from the playing app, stays quiet) and "Only
//! important" over a fullscreen app are the island service's (the surface
//! says it does not do them either); the island's alert history in
//! Notification Center is passed through from the shell state
//! `island.history` when the island service says it (else none).

use super::calendar as calq;
use super::common::*;
use super::timers::{self, Timers};
use super::todos;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use tessera_core::host::Host;
use tessera_core::services::apps::Apps;
use tessera_core::services::calendar::{self as cal, Calendar};
use tessera_core::services::notify::{markup, model::Note};

/// What an island alert's buttons do (the island's `islandAct uid which`)
#[derive(Clone, Debug, Default)]
pub struct Acts {
    /// the card itself ("1"): the default action, else open Notification Center
    pub default: Option<Act>,
    /// the card's own buttons ("btn <i>")
    pub buttons: Vec<Act>,
}

#[derive(Clone, Debug)]
pub enum Act {
    /// invoke the notification's action (id, action key)
    Invoke(u32, String),
    /// Reply: open Notification Center on it
    Reply(u32),
    OpenCenter,
}

#[derive(Default)]
pub struct Notif {
    pub list: Vec<Note>,
    pub primed: bool,
    pub unread: i64,
    pub dismissed_here: HashSet<u32>,
    pub acts: HashMap<i64, Acts>,
    pub center_open: bool,
    pub last_shell: (bool, bool),
    pub last_widgets: String,
    pub last_calendar: String,
    /// the calendar range Notification Center asked for: (from, to, day) as "y-m-d"
    pub nc_cal: Option<(String, String, String)>,
    /// a to-do just deleted, offered back for a few seconds
    pub undo: Option<Value>,
    pub undo_at: i64,
    pub fine: bool,
    pub tab: i64,
}

fn words_initials(s: &str) -> String {
    s.split_whitespace()
        .filter(|w| w.chars().next().is_some_and(|c| c.to_uppercase().ne(c.to_lowercase()) || c.is_ascii_digit()))
        .take(2)
        .filter_map(|w| w.chars().next())
        .flat_map(|c| c.to_uppercase())
        .collect()
}

/// "y-mm-dd" -> midnight (local) in ms
pub fn parse_day(s: &str) -> Option<i64> {
    let mut p = s.split('-').map(|x| x.parse::<i32>());
    let (y, m, d) = (p.next()?.ok()?, p.next()?.ok()?, p.next()?.ok()?);
    Some(cal::make(y, m - 1, d, 0, 0, 0))
}

fn key_of(ms: i64) -> String {
    let t = cal::local(ms);
    format!("{}-{:02}-{:02}", t.y, t.mo + 1, t.d)
}

impl Notif {
    pub fn start(&mut self, h: &mut Host) {
        self.primed = false;
        h.tell("notifications", json!({"do": "hello"}));
        self.last_shell = (false, false);
        self.tell_shell(h);
    }

    /// Its banners hide under Notification Center and Control Center, and
    /// move down while the OSD shows
    pub fn tell_shell(&mut self, h: &mut Host) {
        let open = h.state("open").as_str().unwrap_or("").to_string();
        let shell = (open == "notificationCenter" || open == "controlCenter", h.state("panels.osdVisible").as_bool().unwrap_or(false));
        if shell != self.last_shell {
            self.last_shell = shell;
            h.tell("notifications", json!({"do": "shell", "covered": shell.0, "osd": shell.1}));
        }
    }

    pub fn publish_unread(&self, h: &mut Host) {
        h.set_state("unread", json!(self.unread));
    }

    /// Notification Center opened or closed (the shell's state `open`)
    pub fn center_changed(&mut self, h: &mut Host, timers: &mut Timers) {
        let open = h.state("open").as_str() == Some("notificationCenter");
        if open == self.center_open {
            return;
        }
        self.center_open = open;
        if open {
            self.unread = 0;
            self.publish_unread(h);
            self.last_widgets.clear();
            self.last_calendar.clear();
        } else {
            self.set_fine(false, timers);
        }
    }

    pub fn set_fine(&mut self, on: bool, timers: &mut Timers) {
        if on == self.fine {
            return;
        }
        self.fine = on;
        timers.sw_fine = (timers.sw_fine + if on { 1 } else { -1 }).max(0);
    }

    // ── tessera-ui's list ───────────────────────────────────────────────

    /// The surface's list changed (`fromRust`)
    pub fn from_surface(&mut self, h: &mut Host, items: &Value, calls: &mut super::calls::Calls) {
        let new: Vec<Note> = items.as_array().map(|a| a.iter().filter_map(|x| serde_json::from_value::<Note>(x.clone()).ok()).collect()).unwrap_or_default();
        let old: HashSet<u32> = self.list.iter().map(|n| n.id).collect();
        let now_ids: HashSet<u32> = new.iter().map(|n| n.id).collect();
        let fresh: Vec<Note> = new.iter().filter(|n| !old.contains(&n.id)).cloned().collect();
        let gone: Vec<u32> = old.difference(&now_ids).copied().collect();
        self.list = new;
        // the lock screen's list (Lock.qml tellNotes): whoever serves them, the
        // lock shows them; it keeps the latest, so it's current when it locks
        let notes: Vec<Value> = self
            .list
            .iter()
            .map(|n| {
                let key = [&n.app_name, &n.desktop_entry].into_iter().find(|s| !s.is_empty()).cloned().unwrap_or_else(|| n.id.to_string());
                let icon = [&n.image, &n.app_icon, &n.desktop_entry].into_iter().find(|s| !s.is_empty()).cloned().unwrap_or_default();
                serde_json::json!({"key": key, "app": n.app_name, "icon": icon, "summary": n.summary, "body": n.body, "at": n.at})
            })
            .collect();
        h.tell("lock", serde_json::json!({"notes": notes}));
        for id in gone {
            let dismissed = self.dismissed_here.remove(&id);
            calls.note_gone(h, id, dismissed);
        }
        let primed = self.primed;
        self.primed = true;
        if primed {
            for n in fresh {
                self.arrived(h, &n, calls);
            }
        }
    }

    /// What the shell does for a new one the surface received: the bell,
    /// and its sound, island card or call card (the banner is the surface's)
    fn arrived(&mut self, h: &mut Host, n: &Note, calls: &mut super::calls::Calls) {
        if !self.center_open {
            self.unread += 1;
            self.publish_unread(h);
        }
        let sound = |h: &mut Host| h.ask("sound.notify", serde_json::to_value(n).unwrap_or(Value::Null));
        match n.route.as_str() {
            "banner" => sound(h),
            "island" => {
                sound(h);
                // (the island on and armed: its card; else a banner)
                if tessera_core::services::notify::model::island_on(h.cfg()) {
                    self.to_island(h, n);
                } else {
                    h.tell("notifications", json!({"id": n.id, "do": "banner"}));
                }
            }
            "call" => {
                if !calls.ring(h, n) {
                    sound(h);
                    h.tell("notifications", json!({"id": n.id, "do": "banner"}));
                }
            }
            _ => {}
        }
    }

    /// Notifications.toIsland: the notification as a card in the island
    fn to_island(&mut self, h: &mut Host, n: &Note) {
        let name = if n.summary.is_empty() { n.app_name.clone() } else { n.summary.clone() };
        let apps = h.st.shell.get::<Apps>();
        let entry_icon = apps.and_then(|a| a.entry_for(if n.desktop_entry.is_empty() { &n.app_name } else { &n.desktop_entry })).map(|e| e.icon.clone());
        let app_icon = if !n.app_icon.is_empty() { n.app_icon.clone() } else { entry_icon.unwrap_or_else(|| "dialog-information".into()) };
        let mut buttons: Vec<(String, Act)> = vec![];
        if n.has_inline_reply {
            buttons.push(("Reply".into(), Act::Reply(n.id)));
        }
        for a in n.actions.iter().filter(|a| a.id != "default" && a.id != "inline-reply" && !a.text.is_empty()) {
            buttons.push((a.text.clone(), Act::Invoke(n.id, a.id.clone())));
        }
        buttons.truncate(2);
        let def = n.actions.iter().find(|a| a.id == "default").map(|a| Act::Invoke(n.id, a.id.clone())).unwrap_or(Act::OpenCenter);
        let event = json!({
            "kind": "notification", "nid": n.id,
            "card": {
                "name": name, "time": "now", "text": markup::plain(&n.body), "initials": words_initials(&name),
                "avatar": n.image, "appIcon": app_icon,
                "buttons": buttons.iter().map(|(t, _)| json!({"text": t})).collect::<Vec<_>>(),
            },
        });
        let uid = island_post(h, event, 5000);
        self.acts.insert(uid, Acts { default: Some(def), buttons: buttons.into_iter().map(|(_, a)| a).collect() });
        // (the table is small: the oldest go)
        if self.acts.len() > 64 {
            let mut keys: Vec<i64> = self.acts.keys().copied().collect();
            keys.sort();
            for k in keys.iter().take(self.acts.len() - 64) {
                self.acts.remove(k);
            }
        }
    }

    /// `islandAct <uid> <which>` for an alert this made
    pub fn island_act(&mut self, h: &mut Host, uid: i64, which: &str) {
        let Some(acts) = self.acts.get(&uid).cloned() else { return };
        let act = match which {
            "1" => acts.default,
            w => w.strip_prefix("btn ").and_then(|i| i.parse::<usize>().ok()).and_then(|i| acts.buttons.get(i).cloned()),
        };
        match act {
            Some(Act::Invoke(id, key)) => h.tell("notifications", json!({"id": id, "do": "invoke", "action": key})),
            // Reply (Notifications.startReply): the notification's own banner in Notification Center, where its reply field is
            Some(Act::Reply(id)) => {
                h.tell("notifications", json!({"id": id, "do": "banner"}));
                h.ask("panels.show", json!(["notificationCenter"]));
            }
            Some(Act::OpenCenter) => h.ask("panels.show", json!(["notificationCenter"])),
            None => {}
        }
    }

    pub fn clear_all(&mut self, h: &mut Host) {
        if !self.list.is_empty() {
            h.tell("notifications", json!({"do": "clear"}));
        }
        self.unread = 0;
        self.publish_unread(h);
    }

    /// notify [what, …] / notify.<what>: Settings → Notifications
    pub fn settings_call(&mut self, h: &mut Host, call: &str, args: &Value) {
        let a = args_of(args);
        let (what, rest): (String, Vec<Value>) = match call.strip_prefix("notify.") {
            Some(w) => (w.to_string(), a),
            None => (a.first().and_then(Value::as_str).unwrap_or("").to_string(), a.into_iter().skip(1).collect()),
        };
        let rest = Value::Array(rest);
        match what.as_str() {
            "clearAll" => self.clear_all(h),
            // the quick-reply shortcut: the newest notification you can answer, in Notification Center
            "replyLatest" => match self.list.iter().rev().find(|n| n.has_inline_reply).map(|n| n.id) {
                Some(id) => {
                    h.tell("notifications", json!({"id": id, "do": "banner"}));
                    h.ask("panels.show", json!(["notificationCenter"]));
                }
                None => {
                    island_post(h, json!({"kind": "custom", "icon": "chat-circle", "title": "Nothing to reply to", "subtitle": "Messages you can answer from here show a Reply button", "color": "#5E5CE6"}), 3000);
                }
            },
            "play" => h.ask("sound.play", json!([arg_str(&rest, 0), true])),
            "playStartup" => h.ask("sound.playStartup", rest),
            "setMode" | "addMode" | "removeMode" | "restoreMode" | "focusOn" | "focusOff" => h.ask(&format!("focus.{what}"), rest),
            "installHyprbars" => {
                if let Some(s) = tessera_core::util::asset("shell/assets/scripts/install-hyprbars.sh") {
                    detached(&["sh", &s.to_string_lossy()]);
                }
            }
            other => eprintln!("tessera-ui: host: no notify call {other}"),
        }
    }

    // ── Notification Center's widgets ───────────────────────────────────

    /// The widgets' data while the panel is open (`rustWidgets`)
    pub fn widgets(&self, h: &Host, timers: &Timers, now: i64) -> Value {
        let focus_on = h.state("focus.on").as_bool().unwrap_or(false);
        let focus_name = h.state("focus.name").as_str().filter(|s| !s.is_empty()).unwrap_or("Focus").to_string();
        let laps: Vec<Value> = timers.laps.iter().enumerate().map(|(i, l)| json!({"name": format!("Lap {}", timers.laps.len() - i), "time": timers::format_ms(*l)})).collect();
        json!({
            "to": "notifications", "do": "widgets", "tab": self.tab,
            "focus": if focus_on { format!("{focus_name} is on") } else { String::new() },
            "alerts": h.state("island.list").as_array().cloned().unwrap_or_default(),
            "todos": todos::rows(now),
            "undo": self.undo.as_ref().and_then(|u| u["text"].as_str()).unwrap_or(""),
            "timer": {"remaining": timers::format(timers.remaining), "phase": timers.phase, "progress": timers.progress(), "running": timers.running, "completed": timers.completed},
            "sw": {"elapsed": timers::format_ms(timers.sw_elapsed(now)), "running": timers.sw_running, "laps": laps},
        })
    }

    /// Sends the widgets (and the calendar) when they changed
    pub fn push_widgets(&mut self, h: &mut Host, timers: &Timers, now: i64) {
        if !self.center_open {
            return;
        }
        let w = self.widgets(h, timers, now);
        let s = w.to_string();
        if s != self.last_widgets {
            self.last_widgets = s;
            h.say(w);
        }
        self.send_calendar(h);
    }

    /// The days it shows ({from, to, day}): CalendarWidget's marks, everyDay
    /// and items
    pub fn send_calendar(&mut self, h: &mut Host) {
        if !self.center_open {
            return;
        }
        let Some((f, t, d)) = self.nc_cal.clone() else { return };
        let (Some(first), Some(to), Some(day)) = (parse_day(&f), parse_day(&t), parse_day(&d)) else { return };
        let Some(c) = h.st.shell.get::<Calendar>().cloned() else { return };
        let accent = h.st.shell.look.accent.clone();
        let v = calendar_payload(&c, &todos::list(), first, to, day, &accent);
        let s = v.to_string();
        if s != self.last_calendar {
            self.last_calendar = s;
            h.say(v);
        }
    }

    pub fn undo_expired(&mut self, now: i64) {
        if self.undo.is_some() && now - self.undo_at > 8000 {
            self.undo = None;
        }
    }
}

/// `{"to":"notifications","do":"calendar",…}` for the days Notification
/// Center shows (Notifications.sendCalendar)
pub fn calendar_payload(c: &Calendar, todo_list: &[todos::Item], first: i64, to: i64, day: i64, accent: &str) -> Value {
    let list = c.between(first, to);
    let daily = calq::daily_in(&list, first, to);
    let mut marks: BTreeMap<String, (Vec<String>, i64, Vec<Value>, i64)> = BTreeMap::new();
    for e in &list {
        if daily.contains_key(&calq::series_of(e)) {
            continue;
        }
        let end = if e.all_day { e.end - 1 } else { e.end };
        let (mut d, endd) = (cal::day_bounds(e.start).0, cal::day_bounds(end.max(e.start)).0);
        while d <= endd {
            let color = if e.color.is_empty() { accent.to_string() } else { e.color.clone() };
            let m = marks.entry(key_of(d)).or_default();
            if !m.0.contains(&color) {
                m.0.push(color);
            }
            d = cal::day_bounds(d).1;
        }
    }
    for t in todo_list.iter().filter(|t| t.due != 0 && !t.done) {
        marks.entry(key_of(t.due)).or_default().1 += 1;
    }
    let event = |e: &cal::Event| {
        let name = calq::name_of(c, e);
        json!({"kind": "event", "title": e.title, "sub": format!("{}{}", calq::time_text(e), if name.is_empty() { String::new() } else { format!(" · {name}") }),
               "color": if e.color.is_empty() { accent.to_string() } else { e.color.clone() }, "key": e.key})
    };
    let todo = |t: &todos::Item| {
        json!({"kind": "todo", "title": t.text, "sub": format!("To-do{}", if t.all_day { String::new() } else { format!(" · {}", hm(t.due)) }), "done": t.done, "key": t.key})
    };
    let on = |d: i64| -> Vec<todos::Item> { todo_list.iter().filter(|t| t.due >= cal::day_bounds(d).0 && t.due < cal::day_bounds(d).1).cloned().collect() };
    // (the week: a few of what's on each day, then "+N more")
    if (to - first) as f64 / calq::DAY as f64 <= 7.5 {
        let mut d = first;
        while d < to {
            let evs = calq::events_on(c, d);
            let tds: Vec<todos::Item> = on(d).into_iter().filter(|t| !t.done).collect();
            let mut lines: Vec<Value> = evs.iter().take(4).map(event).collect();
            lines.extend(tds.iter().take(3).map(todo));
            let more = (evs.len() + tds.len()) as i64 - lines.len() as i64;
            let m = marks.entry(key_of(d)).or_default();
            m.2 = lines;
            m.3 = more;
            d = cal::day_bounds(d).1;
        }
    }
    let marks: serde_json::Map<String, Value> = marks
        .into_iter()
        .map(|(k, (colors, todos, lines, more))| {
            let mut m = json!({"colors": colors, "todos": todos});
            if !lines.is_empty() || more != 0 {
                m["lines"] = json!(lines);
                m["more"] = json!(more);
            }
            (k, m)
        })
        .collect();
    let mut items: Vec<Value> = calq::events_on(c, day).iter().map(event).collect();
    items.extend(on(day).iter().map(todo));
    json!({"to": "notifications", "do": "calendar", "marks": marks, "everyDay": daily.values().cloned().collect::<Vec<_>>().join(", "), "items": items})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_round_trip() {
        let d = parse_day("2026-11-06").unwrap();
        assert_eq!(key_of(d), "2026-11-06");
        assert!(parse_day("nope").is_none());
        assert_eq!(words_initials("maya chen"), "MC");
    }

    #[test]
    fn the_calendar_marks_days_and_todos() {
        let mut c = Calendar::default();
        let at = cal::make(2026, 10, 6, 9, 0, 0);
        c.local.push(cal::Local { id: "a".into(), title: "Dentist".into(), start: at, end: at + 3_600_000, all_day: false });
        let first = cal::make(2026, 10, 1, 0, 0, 0);
        let todo = todos::Item { key: "local/t".into(), text: "Pay".into(), done: false, due: cal::make(2026, 10, 7, 17, 0, 0), all_day: false };
        let v = calendar_payload(&c, &[todo], first, first + 30 * calq::DAY, at, "#0A84FF");
        assert_eq!(v["marks"]["2026-11-06"]["colors"][0], "#0A84FF");
        assert_eq!(v["marks"]["2026-11-07"]["todos"], 1);
        assert_eq!(v["items"][0]["title"], "Dentist");
    }
}

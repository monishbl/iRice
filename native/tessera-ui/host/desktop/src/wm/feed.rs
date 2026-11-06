//! DesktopFeed.qml: while the desktop is drawn in Rust, what only the shell
//! knows reaches it as one line, `{"to":"desktop","feed":{…}}` (weather, this
//! month's events, to-dos, Screen Time, the player, batteries, Spotlight's
//! actions, the notes' text, each stack's and Shortcuts widget's choices,
//! Edit Widgets, Today's greeting), and what the desktop asks (its "desktop"
//! call: `[what, …]`) is done here. The feed is built from core's readings
//! (weather, calendar, todos, mpris, battery) and the shell's state file, and
//! said again, a moment after it changes (checked every two seconds).

use super::hx;
use super::Io;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use tessera_core::host::Host;
use tessera_core::services::calendar::{self as cal, Calendar};
use tessera_core::services::{mpris, status, todos, weather};
use tessera_core::util::{now_ms, strftime};

#[derive(Default)]
pub struct Feed {
    editing: bool,
    edit_screen: String,
    sent: String,
    last_gen: Value,
}

fn ev(e: &cal::Event) -> Value {
    json!({"title": e.title, "color": e.color, "allDay": e.all_day, "start": e.start, "end": e.end})
}

/// Todos.dueLabel: "Today", "Tomorrow", "Mon", "3 Nov", then the time unless all day
pub fn due_label(due: i64, all_day: bool, now: i64, clock24: bool) -> String {
    if due == 0 {
        return String::new();
    }
    let (d, n) = (cal::local(due), cal::local(now));
    let day = |t: &cal::Tm| cal::make(t.y, t.mo, t.d, 0, 0, 0);
    let diff = ((day(&d) - day(&n)) as f64 / 86_400_000.0).round() as i64;
    let date = match diff {
        0 => "Today".to_string(),
        1 => "Tomorrow".into(),
        -1 => "Yesterday".into(),
        2..=6 => strftime("%a", due / 1000),
        _ => strftime("%-d %b", due / 1000),
    };
    if all_day {
        date
    } else {
        format!("{date} {}", strftime(if clock24 { "%H:%M" } else { "%-I:%M %p" }, due / 1000))
    }
}

/// Today.greeting
pub fn greeting(hour: i32, name: &str) -> String {
    let part = if (4..12).contains(&hour) { "Good morning" } else if (12..17).contains(&hour) { "Good afternoon" } else { "Good evening" };
    if name.is_empty() { part.into() } else { format!("{part}, {name}") }
}

/// Calendar.busyDays for [first, last]: "y-m-d" (month from 0) of the days that
/// have an event (a daily series isn't left out here, as the QML's is)
fn busy_days(c: &Calendar, first: i64, last_next: i64) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    for e in c.between(first, last_next) {
        let s = cal::local(e.start);
        let mut d = cal::make(s.y, s.mo, s.d, 0, 0, 0);
        let end_src = if e.all_day { e.end - 1 } else { e.end };
        let t = cal::local(end_src);
        let mut end = cal::make(t.y, t.mo, t.d, 0, 0, 0);
        if end < d {
            end = d;
        }
        while d <= end {
            let x = cal::local(d);
            set.insert(format!("{}-{}-{}", x.y, x.mo, x.d));
            d = cal::make(x.y, x.mo, x.d + 1, 0, 0, 0);
        }
    }
    set
}

impl Feed {
    pub fn build(&self, h: &Host) -> Value {
        let cfg = h.cfg();
        let now = now_ms();
        let clock24 = cfg["appearance"]["clock24"].as_bool().unwrap_or(false);
        let t = cal::local(now);
        let (first, next) = (cal::make(t.y, t.mo, 1, 0, 0, 0), cal::make(t.y, t.mo + 1, 1, 0, 0, 0));
        let calendar = h.st.shell.get::<Calendar>().cloned().unwrap_or_default();
        let list = h.st.shell.get::<todos::Todos>().map(|t| t.list.clone()).unwrap_or_default();

        let mut busy = busy_days(&calendar, first, next);
        for td in list.iter().filter(|t| t.due != 0 && !t.done) {
            let d = cal::local(td.due);
            busy.insert(format!("{}-{}-{}", d.y, d.mo, d.d));
        }
        let upcoming: Vec<Value> = calendar.between(now, now + 14 * 86_400_000).into_iter().filter(|e| e.all_day || e.end > now).take(1).map(|e| ev(&e)).collect();

        let w = h.st.shell.get::<weather::Weather>().cloned().unwrap_or_default();
        let hourly: Vec<Value> = w.hourly.iter().map(|x| json!({"time": strftime(if clock24 { "%H" } else { "%-I %p" }, cal::make(t.y, t.mo, t.d, x.hour as i32, 0, 0) / 1000), "temp": x.temp, "icon": x.icon})).collect();

        let bat = h.st.shell.get::<status::Battery>().cloned().unwrap_or_default();
        let level = if bat.percent > 1.0 { bat.percent / 100.0 } else { bat.percent };
        let mut batteries = Vec::new();
        if bat.present {
            batteries.push(json!({"icon": if bat.charging { "lightning" } else { "laptop" }, "picture": "", "value": level, "name": "This computer"}));
        }

        let player = h.st.shell.get::<mpris::Players>().and_then(|p| p.chosen().cloned());
        let notes = {
            let mut m = hx::state_get("desktopNotes");
            if !m.is_object() {
                m = json!({});
            }
            m[""] = json!(hx::state_get("desktopNote").as_str().unwrap_or(""));
            m
        };
        let obj = |v: Value| if v.is_object() { v } else { json!({}) };

        json!({
            "editing": self.editing,
            "editScreen": self.edit_screen,
            "greeting": greeting(t.h, cfg["today"]["name"].as_str().unwrap_or("").trim()),
            "weather": {"ready": w.ready, "error": w.error, "place": w.place, "temp": w.temp, "unit": "°", "high": w.high, "low": w.low, "condition": w.condition, "icon": w.icon, "hourly": hourly, "rainChance": w.rain_chance},
            "events": calendar.between(first, next).iter().map(ev).collect::<Vec<_>>(),
            "busy": busy.into_iter().collect::<Vec<_>>(),
            "upcoming": upcoming,
            "todos": list.iter().map(|t| json!({"key": t.key, "text": t.text, "done": t.done, "due": t.due, "dueLabel": due_label(t.due, t.all_day, now, clock24), "overdue": t.due != 0 && !t.done && t.due < now})).collect::<Vec<_>>(),
            // (Screen Time is host/system's: nothing here yet)
            "screenTime": {"total": 0, "week": [], "apps": []},
            "media": {"has": player.is_some(), "playing": player.as_ref().is_some_and(|p| p.playing), "title": player.as_ref().map(|p| p.track.title.clone()).unwrap_or_default(), "artist": player.as_ref().map(|p| p.track.artist.clone()).unwrap_or_default(), "art": player.as_ref().map(|p| p.track.art_url.clone()).unwrap_or_default()},
            "batteries": batteries,
            "power": {"has": bat.present, "level": if bat.present { level } else { 1.0 }, "charging": bat.charging, "onBattery": bat.present && !bat.charging},
            "actions": if h.state("search.actions").is_array() { h.state("search.actions").clone() } else { json!([]) },
            "notes": notes,
            "stacks": obj(hx::state_get("stacks")),
            "shortcutButtons": obj(hx::state_get("shortcutButtons")),
        })
    }

    pub fn send(&mut self, h: &mut Host, force: bool) {
        let feed = self.build(h);
        let s = feed.to_string();
        if force || s != self.sent {
            self.sent = s;
            h.tell("desktop", json!({"feed": feed}));
        }
    }

    pub fn begin(&mut self, io: &mut Io) {
        io.every("feed.send", 2000);
    }

    pub fn add_widget(&self, h: &mut Host, kind: &str, size: &str) {
        h.tell("desktop", json!({"add": [kind, size]}));
    }

    /// The desktop's calls: ["what", args…]
    pub fn call(&mut self, h: &mut Host, io: &mut Io, args: &[Value]) {
        let what = args.first().and_then(|v| v.as_str()).unwrap_or("");
        let a = &args[args.len().min(1)..];
        let s = |i: usize| a.get(i).and_then(|v| v.as_str()).unwrap_or("").to_string();
        let screen = h.st.shell.get::<tessera_core::services::clients::Windows>().and_then(|w| w.focused_monitor().map(|m| m.name.clone())).unwrap_or_default();
        match what {
            "todo" => h.ask("todos.toggle", json!([s(0)])),
            "todoAdd" => {
                if !s(0).trim().is_empty() {
                    h.ask("todos.add", json!([s(0)]));
                }
            }
            "media" => {
                if let Some(p) = h.st.shell.get::<mpris::Players>().and_then(|p| p.chosen().cloned()) {
                    mpris::control(&p.bus, match s(0).as_str() { "previous" => "Previous", "next" => "Next", _ => "PlayPause" });
                }
            }
            "shortcut" => h.ask("search.action", json!([s(0)])),
            "note" => {
                let text = s(1);
                if s(0).is_empty() {
                    hx::state_set("desktopNote", json!(text));
                } else {
                    let mut m = hx::state_get("desktopNotes");
                    if !m.is_object() {
                        m = json!({});
                    }
                    m[s(0)] = json!(text);
                    hx::state_set("desktopNotes", m);
                }
                self.send(h, false);
            }
            "stacks" | "buttons" => {
                let key = if what == "stacks" { "stacks" } else { "shortcutButtons" };
                let mut m = hx::state_get(key);
                if !m.is_object() {
                    m = json!({});
                }
                m[s(0)] = a.get(1).cloned().filter(|v| !v.is_null()).unwrap_or(json!([]));
                hx::state_set(key, m);
                self.send(h, false);
            }
            "calendar" => {
                h.ask("panels.set", json!(["ncTab", 0]));
                h.ask("panels.show", json!(["notificationCenter", ""]));
            }
            "today" => h.ask("today.show", json!([a.first().and_then(|v| v.as_bool()).unwrap_or(false)])),
            "options" => {
                h.ask("panels.set", json!(["widgetOptionsKind", s(0)]));
                h.ask("panels.show", json!(["widgetOptions", screen]));
            }
            "edit" => {
                h.ask("panels.close", json!([]));
                self.editing = true;
                self.edit_screen = screen;
                self.send(h, false);
            }
            "endEdit" => {
                self.editing = false;
                self.send(h, false);
            }
            "noRoom" => h.ask("island.post", json!([{"kind": "custom", "icon": "squares-four", "title": "No room on the desktop", "subtitle": "Remove or shrink a widget first", "color": tessera_core::theme::accent_hex("orange", h.st.shell.look.dark)}])),
            "show" => {
                if s(0) == "wallpaper" {
                    h.ask("panels.show", json!(["wallpaper", screen]));
                }
            }
            "open" => h.ask("files.open", json!([s(0)])),
            "reveal" => h.ask("files.reveal", json!([s(0)])),
            "quickLook" => h.ask("panels.quickLook", json!([s(0)])),
            "share" => h.ask("panels.share", json!([a.first().cloned().unwrap_or(json!([])), screen])),
            "sound" => {
                if s(0) == "trash" {
                    h.ask("sound.play", json!(["trash"]));
                }
            }
            _ => {}
        }
        let _ = io;
    }

    /// Edit Widgets ends when the panels it covers close (Panels.screen changes)
    pub fn stop_editing(&mut self, h: &mut Host) {
        if self.editing {
            self.editing = false;
            self.send(h, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greetings_follow_the_hour() {
        assert_eq!(greeting(8, ""), "Good morning");
        assert_eq!(greeting(13, "Ada"), "Good afternoon, Ada");
        assert_eq!(greeting(2, ""), "Good evening");
    }

    #[test]
    fn busy_days_cover_a_multi_day_event() {
        let c = Calendar { local: vec![], sources: vec![], parsed: Default::default() };
        assert!(busy_days(&c, 0, 1000).is_empty());
    }

    #[test]
    fn no_due_date_no_label() {
        assert_eq!(due_label(0, false, 1_000_000, false), "");
    }
}

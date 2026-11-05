//! What the QML shell knows and the widgets show, until those services are
//! in Rust too: shell/modules/desktop/DesktopFeed.qml sends it while this
//! surface draws the desktop, one line on stdin (RustUi.tell):
//!   {"to":"desktop","feed":{…}}      the whole picture, again when it changes
//!   {"to":"desktop","add":["notes","s"],"screen":"DP-1"}   the gallery (or
//!                                    Today's Pin) added a widget there
//! Weather, the calendar's events, to-dos, Screen Time, the player, the
//! batteries Bluetooth reports, Spotlight's actions (Shortcuts), the notes'
//! text and each stack's and Shortcuts widget's choices (the shell's state
//! file), whether Edit Widgets is on and Today's greeting.

use serde::Deserialize;
use std::collections::HashMap;

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Hour {
    pub time: String,
    pub temp: i64,
    pub icon: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Weather {
    pub ready: bool,
    pub error: String,
    pub place: String,
    pub temp: i64,
    pub unit: String,
    pub high: i64,
    pub low: i64,
    pub condition: String,
    pub icon: String,
    pub hourly: Vec<Hour>,
    #[serde(rename = "rainChance")]
    pub rain_chance: i64,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Event {
    pub title: String,
    pub color: String,
    #[serde(rename = "allDay")]
    pub all_day: bool,
    /// ms since the epoch
    pub start: i64,
    pub end: i64,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Todo {
    pub key: String,
    pub text: String,
    pub done: bool,
    /// ms, 0 when undated
    pub due: i64,
    #[serde(rename = "dueLabel")]
    pub due_label: String,
    pub overdue: bool,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Day {
    pub label: String,
    pub seconds: i64,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct AppTime {
    pub id: String,
    pub name: String,
    /// the icon's file (Apps.iconFor), "" if none
    pub icon: String,
    pub seconds: i64,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct ScreenTime {
    pub total: i64,
    pub week: Vec<Day>,
    pub apps: Vec<AppTime>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Media {
    pub has: bool,
    pub playing: bool,
    pub title: String,
    pub artist: String,
    /// the cover's file:// or http(s) address, "" if none
    pub art: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Battery {
    pub icon: String,
    /// a device picture's file ("" if none)
    pub picture: String,
    pub value: f64,
    pub name: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Action {
    pub title: String,
    pub glyph: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Power {
    pub has: bool,
    pub level: f64,
    pub charging: bool,
    #[serde(rename = "onBattery")]
    pub on_battery: bool,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Feed {
    /// Edit Widgets is on, and on which screen
    pub editing: bool,
    #[serde(rename = "editScreen")]
    pub edit_screen: String,
    pub greeting: String,
    pub weather: Weather,
    /// this month's events, all-day first then by start (Calendar.between)
    pub events: Vec<Event>,
    /// "y-m-d" (month from 0, as JavaScript) of this month's busy days,
    /// to-dos due included (Calendar.busyDays)
    pub busy: Vec<String>,
    /// what's next (Calendar.upcoming(1))
    pub upcoming: Vec<Event>,
    /// every to-do (Todos.list); the open ones are sorted here
    pub todos: Vec<Todo>,
    #[serde(rename = "screenTime")]
    pub screen_time: ScreenTime,
    pub media: Media,
    pub batteries: Vec<Battery>,
    pub power: Power,
    pub actions: Vec<Action>,
    /// a note's id ("" the first note) -> its text
    pub notes: HashMap<String, String>,
    /// a stack's id -> its kinds
    pub stacks: HashMap<String, Vec<String>>,
    /// a Shortcuts widget's id -> its buttons' titles
    #[serde(rename = "shortcutButtons")]
    pub shortcut_buttons: HashMap<String, Vec<String>>,
}

impl Feed {
    pub fn from(v: &serde_json::Value) -> Option<Feed> {
        serde_json::from_value(v.clone()).ok()
    }

    /// Open to-dos, soonest due first, then the undated in the order added
    /// (Todos.sorted)
    pub fn open_todos(&self) -> Vec<&Todo> {
        let mut l: Vec<&Todo> = self.todos.iter().filter(|t| !t.done).collect();
        l.sort_by_key(|t| if t.due > 0 { t.due } else { i64::MAX });
        l
    }

    /// Events on a local day (its first ms .. the next day's)
    pub fn events_between(&self, from: i64, to: i64) -> Vec<&Event> {
        self.events.iter().filter(|e| e.start < to && (e.end > from || (e.end <= e.start && e.start >= from))).collect()
    }
}

/// A stack's kinds by default (StackWidget.qml)
pub const STACK_DEFAULTS: &[&str] = &["weather", "calendar", "reminders", "nowPlaying", "batteries"];
/// A Shortcuts widget's buttons by default
pub const SHORTCUT_DEFAULTS: &[&str] = &["Take Screenshot", "Toggle Dark Mode", "Lock Screen", "Clipboard History", "Record Screen", "All Windows", "App Drawer", "System Settings"];

/// Short labels for the Shortcuts buttons (the menu keeps the full names)
pub fn short_name(title: &str) -> String {
    let s = match title {
        "Take Screenshot" => "Screen\u{AD}shot",
        "Toggle Dark Mode" => "Dark Mode",
        "Lock Screen" => "Lock",
        "Clipboard History" => "Clip\u{AD}board",
        "Record Screen" => "Record",
        "System Settings" => "Settings",
        "Copy Text from Screen" => "Copy Text",
        "Ask AI About the Screen" => "Ask AI",
        "On-Screen Keyboard" => "Keyboard",
        "Keyboard Shortcuts" => "Shortcuts",
        "Notification Center" => "Notifications",
        "Check for Updates" => "Updates",
        "Change Wallpaper" => "Wallpaper",
        "Overlay Widgets" => "Overlay",
        t => return t.trim_end_matches('…').to_string(),
    };
    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_what_the_shell_sends_and_fills_the_rest() {
        let f = Feed::from(&json!({
            "editing": true,
            "weather": {"ready": true, "place": "Pune", "temp": 24, "hourly": [{"time": "3 PM", "temp": 25, "icon": "sun"}], "rainChance": 40},
            "todos": [{"key": "a", "text": "later", "due": 0}, {"key": "b", "text": "soon", "due": 5}, {"key": "c", "text": "done", "done": true}],
            "notes": {"": "first", "ab": "second"},
            "stacks": {"x": ["weather"]},
            "unknown": 1
        }))
        .unwrap();
        assert!(f.editing && f.weather.ready);
        assert_eq!((f.weather.place.as_str(), f.weather.rain_chance, f.weather.hourly[0].temp), ("Pune", 40, 25));
        assert_eq!(f.open_todos().iter().map(|t| t.key.as_str()).collect::<Vec<_>>(), vec!["b", "a"]);
        assert_eq!(f.notes[""], "first");
        assert!(f.media.title.is_empty() && f.batteries.is_empty());
        assert!(Feed::from(&json!("nope")).is_none());
    }

    #[test]
    fn events_on_a_day() {
        let f = Feed {
            events: vec![
                Event { title: "yesterday".into(), start: 0, end: 10, ..Default::default() },
                Event { title: "spans".into(), start: 50, end: 150, ..Default::default() },
                Event { title: "in".into(), start: 120, end: 130, ..Default::default() },
                Event { title: "instant".into(), start: 140, end: 140, ..Default::default() },
                Event { title: "tomorrow".into(), start: 200, end: 210, ..Default::default() },
            ],
            ..Default::default()
        };
        let on: Vec<&str> = f.events_between(100, 200).iter().map(|e| e.title.as_str()).collect();
        assert_eq!(on, vec!["spans", "in", "instant"]);
    }

    #[test]
    fn short_names() {
        assert_eq!(short_name("Lock Screen"), "Lock");
        assert_eq!(short_name("Open Downloads…"), "Open Downloads");
    }
}

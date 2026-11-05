//! What a notification is, and the rules it meets, as pure functions (the
//! part in mod.rs only wires them to the bus, the screen and the shell):
//!   - a Note from what an app sent (services/notify.rs reads the bus);
//!   - where it goes when it arrives (`route`): a banner, the island, a call,
//!     or quietly to the list only, as Notifications.qml's onNotification;
//!   - Focus (Do Not Disturb and the other modes, focus.json) as Focus.qml's
//!     `allows`; calls as Notifications.qml's isCall;
//!   - how long a banner stays (NotificationPopups.qml's timer);
//!   - the list Notification Center shows (the newest 200, grouped by app,
//!     NotificationGroups.js), and its file (history.rs's format: see
//!     `History`).

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[allow(dead_code)] // (the tests, and Sounds.qml: no sound for low ones)
pub const LOW: u8 = 0;
pub const NORMAL: u8 = 1;
pub const CRITICAL: u8 = 2;

/// Notification Center keeps the newest this many (Notifications.qml's `keep`)
pub const KEEP: usize = 200;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Action {
    pub id: String,
    pub text: String,
}

/// One notification, as the QML shell knows it (Quickshell's Notification:
/// the same names, so the shell's mirror of the list reads it as one)
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Note {
    pub id: u32,
    pub app_name: String,
    /// as sent: an icon name, a path or a file: URL ("" none)
    pub app_icon: String,
    pub desktop_entry: String,
    pub summary: String,
    /// as sent, markup and all (markup.rs makes it safe to show)
    pub body: String,
    /// a picture: a file (image-path, or image-data saved by the server)
    pub image: String,
    /// the app's buttons, "default" among them (the click), not inline-reply
    pub actions: Vec<Action>,
    pub urgency: u8,
    /// ms, as sent: -1 the server decides, 0 never
    pub expire_timeout: i32,
    pub category: String,
    pub resident: bool,
    pub transient: bool,
    pub suppress_sound: bool,
    pub sound_file: String,
    /// progress the app reports (the "value" hint), -1 none
    pub value: i32,
    pub has_inline_reply: bool,
    pub inline_reply_placeholder: String,
    /// when it arrived, ms since the epoch
    pub at: i64,
    /// where it went when it arrived (Route's names)
    pub route: String,
}

impl Note {
    pub fn default_action(&self) -> Option<&Action> {
        self.actions.iter().find(|a| a.id == "default")
    }
    /// The buttons a card shows (not the click, not inline reply)
    pub fn buttons(&self) -> Vec<&Action> {
        self.actions.iter().filter(|a| a.id != "default" && a.id != "inline-reply" && !a.text.is_empty()).collect()
    }
}

/// The hints the shell uses, read from what came over the bus
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Hints {
    pub urgency: Option<u8>,
    pub category: String,
    pub desktop_entry: String,
    pub image_path: String,
    pub resident: bool,
    pub transient: bool,
    pub suppress_sound: bool,
    pub sound_file: String,
    pub value: Option<i32>,
    pub reply_placeholder: String,
}

/// A Note from Notify's arguments (the picture is the server's: a file it
/// saved from image-data, or ""). Actions come in pairs (key, label); an odd
/// list is ignored, as Quickshell does
pub fn note_from(id: u32, app_name: &str, app_icon: &str, summary: &str, body: &str, actions: &[String], hints: &Hints, expire_timeout: i32, image: &str, at: i64) -> Note {
    let mut n = Note {
        id,
        app_name: app_name.to_string(),
        app_icon: app_icon.to_string(),
        desktop_entry: hints.desktop_entry.clone(),
        summary: summary.to_string(),
        body: body.to_string(),
        image: if image.is_empty() { hints.image_path.clone() } else { image.to_string() },
        urgency: hints.urgency.unwrap_or(NORMAL).min(CRITICAL),
        expire_timeout,
        category: hints.category.clone(),
        resident: hints.resident,
        transient: hints.transient,
        suppress_sound: hints.suppress_sound,
        sound_file: hints.sound_file.clone(),
        value: hints.value.map_or(-1, |v| v.clamp(0, 100)),
        at,
        ..Default::default()
    };
    if actions.len() % 2 == 0 {
        for pair in actions.chunks(2) {
            if pair[0] == "inline-reply" {
                if !n.has_inline_reply {
                    n.has_inline_reply = true;
                    n.inline_reply_placeholder = pair[1].clone();
                }
                continue;
            }
            n.actions.push(Action { id: pair[0].clone(), text: pair[1].clone() });
        }
    }
    if n.inline_reply_placeholder.is_empty() {
        n.inline_reply_placeholder = hints.reply_placeholder.clone();
    }
    n
}

/// The id Notify answers with: the one it replaces while that one is still
/// here, else the next (as Quickshell: an unknown replaces_id gets a new id)
pub fn assign_id(live: impl Fn(u32) -> bool, next: &mut u32, replaces: u32) -> (u32, bool) {
    if replaces != 0 && live(replaces) {
        return (replaces, true);
    }
    let id = *next;
    *next = next.wrapping_add(1).max(1);
    (id, false)
}

// ── Where a new one goes ────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    /// a banner (and its sound)
    Banner,
    /// the island's card instead of a banner (island.events has "notifications")
    Island,
    /// a ringing call, for the island's call card (island.events has "calls")
    Call,
    /// only to Notification Center and the bell: a Focus mode holds it
    Quiet,
}

impl Route {
    pub fn name(self) -> &'static str {
        match self {
            Route::Banner => "banner",
            Route::Island => "island",
            Route::Call => "call",
            Route::Quiet => "quiet",
        }
    }
}

fn norm(x: &str) -> String {
    let mut out = String::new();
    let mut space = false;
    let lower = super::markup::plain(x).to_lowercase();
    for c in lower.chars() {
        if c.is_ascii_alphanumeric() || (c as u32) >= 0xc0 {
            if space && !out.is_empty() {
                out.push(' ');
            }
            out.push(c);
            space = false;
        } else {
            space = true;
        }
    }
    out
}

/// These words in a row, as the QML's \b…\b patterns find them (a word
/// ends at anything but an ASCII letter or digit: "calling…" is "calling")
fn has_word_seq(text: &str, words: &[&str]) -> bool {
    let t: Vec<&str> = text.split(|c: char| !c.is_ascii_alphanumeric()).filter(|w| !w.is_empty()).collect();
    t.windows(words.len()).any(|w| w == words)
}

/// A ringing call (Notifications.qml's isCall): category "call" (or an
/// x-…call one), or words like "Incoming call" / "is calling", never a
/// missed or ended one
pub fn is_call(n: &Note) -> bool {
    let cat = n.category.to_lowercase();
    if ["missed", "ended", "declined"].iter().any(|w| cat.contains(w)) {
        return false;
    }
    if cat.split(['.', '-']).any(|p| p == "call") {
        return true;
    }
    let text = format!("{} {}", norm(&n.summary), norm(&n.body));
    if has_word_seq(&text, &["missed"]) || has_word_seq(&text, &["ended"]) {
        return false;
    }
    let kinds = ["", "voice", "video", "audio", "group"];
    let incoming = kinds.iter().any(|k| if k.is_empty() { has_word_seq(&text, &["incoming", "call"]) } else { has_word_seq(&text, &["incoming", k, "call"]) });
    let started = ["", "voice", "video"].iter().any(|k| if k.is_empty() { has_word_seq(&text, &["started", "a", "call"]) } else { has_word_seq(&text, &["started", "a", k, "call"]) });
    incoming
        || started
        || has_word_seq(&text, &["is", "calling"])
        || has_word_seq(&text, &["calling", "you"])
        || has_word_seq(&text, &["voice", "call", "from"])
        || has_word_seq(&text, &["video", "call", "from"])
        || has_word_seq(&text, &["wants", "to", "call"])
        || has_word_seq(&text, &["wants", "to", "video", "call"])
}

/// The Focus mode on now, from focus.json (Focus.qml's file): its allowed
/// apps, or None when no mode is on (or its time ran out)
pub fn focus_allowed_apps(focus: &Value, now_ms: i64) -> Option<Vec<String>> {
    let active = focus["active"].as_str().unwrap_or("");
    if active.is_empty() {
        return None;
    }
    let until = focus["until"].as_f64().unwrap_or(0.0);
    if until > 0.0 && now_ms as f64 >= until {
        return None;
    }
    let modes = focus["modes"].as_array().cloned().unwrap_or_default();
    let mode = modes.iter().find(|m| m["id"].as_str() == Some(active)).cloned().or_else(|| {
        // (Focus.qml's own four, when the file has no list yet)
        ["dnd", "work", "personal", "sleep"].contains(&active).then(|| serde_json::json!({"id": active, "allowApps": []}))
    })?;
    Some(mode["allowApps"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default())
}

/// A banner may show (Focus.qml's allows): no mode on, critical, or from
/// an app the mode lets through
pub fn focus_allows(n: &Note, focus: &Value, now_ms: i64) -> bool {
    let Some(allowed) = focus_allowed_apps(focus, now_ms) else { return true };
    if n.urgency == CRITICAL {
        return true;
    }
    let names: Vec<String> = [&n.app_name, &n.desktop_entry].iter().map(|x| x.to_lowercase()).filter(|x| !x.is_empty()).collect();
    allowed.iter().any(|a| names.contains(&a.to_lowercase()))
}

/// Island.qml's wants: in island.events, or a kind newer than the setting
/// that the user hasn't touched (island.known)
pub fn island_wants(cfg: &Value, kind: &str) -> bool {
    const NEW_KINDS: &[&str] = &["usb", "displays", "audio", "temperature", "network", "transfers", "calls", "sysupdate"];
    let island = &cfg["island"];
    let has = |k: &str, v: &Value| v.as_array().is_some_and(|a| a.iter().any(|x| x.as_str() == Some(k)));
    if island["events"].is_array() {
        has(kind, &island["events"]) || (NEW_KINDS.contains(&kind) && !has(kind, &island["known"]))
    } else {
        // (the default list, Config.qml: no notifications, calls as a new kind)
        kind != "notifications" && (NEW_KINDS.contains(&kind) || ["music", "charging", "battery", "timers", "recording", "focus", "bluetooth", "resources"].contains(&kind))
    }
}

pub fn island_on(cfg: &Value) -> bool {
    cfg["island"]["enabled"].as_bool().unwrap_or(true)
}

/// Where a new notification goes. `shell`: the QML shell listens (it draws
/// the island; without it, the island's share comes as banners)
pub fn route(n: &Note, cfg: &Value, focus: &Value, now_ms: i64, shell: bool) -> Route {
    if !focus_allows(n, focus, now_ms) {
        return Route::Quiet;
    }
    let island = shell && island_on(cfg);
    if is_call(n) {
        return if island && island_wants(cfg, "calls") { Route::Call } else { Route::Banner };
    }
    if island && island_wants(cfg, "notifications") && n.urgency != CRITICAL {
        return Route::Island;
    }
    Route::Banner
}

/// How long a banner stays, ms (None: until dismissed: critical ones and a
/// ringing call). The app's own timeout when it gives one, else 5 s
pub fn banner_ms(n: &Note) -> Option<u64> {
    if n.urgency == CRITICAL || is_call(n) {
        return None;
    }
    Some(if n.expire_timeout > 0 { n.expire_timeout as u64 } else { 5000 })
}

/// The time on a card (Notifications.qml's timeLabel): "now", "5m ago",
/// then the clock time
pub fn time_label(at_ms: i64, now_ms: i64, clock: impl Fn(i64) -> String) -> String {
    if at_ms <= 0 {
        return String::new();
    }
    let mins = (now_ms - at_ms).max(0) / 60000;
    match mins {
        0 => "now".into(),
        1..=59 => format!("{mins}m ago"),
        _ => clock(at_ms),
    }
}

/// Notification Center's groups (NotificationGroups.js): by app, newest group
/// first, each newest first. `list` is oldest first, as the server keeps it
#[allow(dead_code)] // (Notification Center, when it moves to Rust; QML groups the mirror)
pub fn groups(list: &[Note]) -> Vec<(String, Vec<&Note>)> {
    let mut out: Vec<(String, Vec<&Note>)> = Vec::new();
    for n in list.iter().rev() {
        let app = if n.app_name.is_empty() { "Notification".to_string() } else { n.app_name.clone() };
        match out.iter_mut().find(|g| g.0 == app) {
            Some(g) => g.1.push(n),
            None => out.push((app, vec![n])),
        }
    }
    out
}

// ── The list ────────────────────────────────────────────────────────────────

/// What Notification Center lists: oldest first, at most KEEP (the oldest
/// go first). Transient ones aren't kept once their banner is gone
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Store {
    pub list: Vec<Note>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Added {
    /// a new one (and the ones pushed out by the limit, closed as expired)
    New(Vec<u32>),
    /// it replaced one with the same id, in place
    Replaced,
}

impl Store {
    pub fn get(&self, id: u32) -> Option<&Note> {
        self.list.iter().find(|n| n.id == id)
    }
    pub fn has(&self, id: u32) -> bool {
        self.get(id).is_some()
    }
    pub fn add(&mut self, n: Note) -> Added {
        if let Some(old) = self.list.iter_mut().find(|x| x.id == n.id) {
            // (keeps its place, its arrival time and where it went)
            let (at, route) = (old.at, old.route.clone());
            *old = n;
            old.at = at;
            old.route = route;
            return Added::Replaced;
        }
        self.list.push(n);
        let over = self.list.len().saturating_sub(KEEP);
        Added::New(self.list.drain(..over).map(|n| n.id).collect())
    }
    pub fn remove(&mut self, id: u32) -> Option<Note> {
        let i = self.list.iter().position(|n| n.id == id)?;
        Some(self.list.remove(i))
    }
    pub fn next_id(&self) -> u32 {
        self.list.iter().map(|n| n.id).max().unwrap_or(0).wrapping_add(1).max(1)
    }
}

/// The list's file, `$XDG_STATE_HOME/tessera/notifications.json`: kept so a
/// restart of tessera-ui on the same session bus (`bus`, the bus's id) finds
/// the list as it was, ids and all (apps still hold those ids). Another bus
/// (a new login) starts empty, as the QML server does. The shell's mirror
/// reads the same objects (Note's names are Quickshell's)
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct History {
    pub version: u32,
    pub bus: String,
    pub list: Vec<Note>,
}

pub fn history_json(bus: &str, store: &Store) -> String {
    let h = History { version: 1, bus: bus.to_string(), list: store.list.iter().filter(|n| !n.transient).cloned().collect() };
    serde_json::to_string(&h).unwrap_or_default()
}

/// The list a file holds for this bus (empty for another bus or a bad file)
pub fn history_read(text: &str, bus: &str) -> Store {
    match serde_json::from_str::<History>(text) {
        Ok(h) if h.version == 1 && h.bus == bus && !bus.is_empty() => {
            let mut list = h.list;
            let over = list.len().saturating_sub(KEEP);
            list.drain(..over);
            Store { list }
        }
        _ => Store::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn note(summary: &str, body: &str) -> Note {
        note_from(1, "Chat", "", summary, body, &[], &Hints::default(), -1, "", 1000)
    }

    #[test]
    fn notify_arguments() {
        let hints = Hints { urgency: Some(2), desktop_entry: "org.example.Chat".into(), value: Some(140), reply_placeholder: "Message".into(), ..Default::default() };
        let actions: Vec<String> = ["default", "Open", "inline-reply", "Reply…", "mark", "Mark read"].iter().map(|s| s.to_string()).collect();
        let n = note_from(7, "Chat", "chat", "Ann", "Hi", &actions, &hints, 3000, "", 5);
        assert_eq!(n.id, 7);
        assert_eq!(n.urgency, CRITICAL);
        assert_eq!(n.value, 100);
        assert!(n.has_inline_reply);
        // the action's label is the placeholder, ahead of the KDE hint
        assert_eq!(n.inline_reply_placeholder, "Reply…");
        assert_eq!(n.actions.len(), 2);
        assert_eq!(n.default_action().map(|a| a.text.as_str()), Some("Open"));
        assert_eq!(n.buttons().iter().map(|a| a.text.as_str()).collect::<Vec<_>>(), vec!["Mark read"]);
        // an odd list is ignored; urgency out of range is critical; no value: -1
        let n = note_from(1, "a", "", "s", "", &["only".into()], &Hints { urgency: Some(9), ..Default::default() }, -1, "", 0);
        assert!(n.actions.is_empty());
        assert_eq!(n.urgency, CRITICAL);
        assert_eq!(n.value, -1);
        // the KDE placeholder when the action gives none
        let n = note_from(1, "a", "", "s", "", &["inline-reply".into(), "".into()], &Hints { reply_placeholder: "Say hi".into(), ..Default::default() }, -1, "", 0);
        assert_eq!(n.inline_reply_placeholder, "Say hi");
        // a saved picture wins over image-path
        let n = note_from(1, "a", "", "s", "", &[], &Hints { image_path: "/p.png".into(), ..Default::default() }, -1, "/saved.png", 0);
        assert_eq!(n.image, "/saved.png");
        let n = note_from(1, "a", "", "s", "", &[], &Hints { image_path: "/p.png".into(), ..Default::default() }, -1, "", 0);
        assert_eq!(n.image, "/p.png");
        assert_eq!(n.urgency, NORMAL);
    }

    #[test]
    fn replaces_id() {
        let mut next = 5;
        let live = |id: u32| id == 3;
        assert_eq!(assign_id(live, &mut next, 3), (3, true));
        assert_eq!(next, 5);
        assert_eq!(assign_id(live, &mut next, 0), (5, false));
        // an id that's gone gets a new one
        assert_eq!(assign_id(live, &mut next, 4), (6, false));
        assert_eq!(next, 7);
        let mut next = u32::MAX;
        assert_eq!(assign_id(|_| false, &mut next, 0).0, u32::MAX);
        assert_eq!(next, 1);
    }

    #[test]
    fn calls() {
        assert!(is_call(&Note { category: "call.incoming".into(), ..note("Ann", "") }));
        assert!(is_call(&Note { category: "x-gnome.call".into(), ..note("Ann", "") }));
        assert!(!is_call(&Note { category: "call.missed".into(), ..note("Ann", "") }));
        assert!(!is_call(&Note { category: "x-recall".into(), ..note("Ann", "") }));
        assert!(is_call(&note("Incoming video call", "Ann")));
        assert!(is_call(&note("Ann", "is calling…")));
        assert!(is_call(&note("Ann started a call.", "")));
        assert!(is_call(&note("Ann", "wants to video call")));
        assert!(!is_call(&note("Missed call", "Ann is calling")));
        assert!(!is_call(&note("Call ended", "")));
        assert!(!is_call(&note("Recalling the meeting", "nothing incoming")));
    }

    #[test]
    fn timeouts_by_urgency() {
        assert_eq!(banner_ms(&note("a", "")), Some(5000));
        assert_eq!(banner_ms(&Note { expire_timeout: 0, ..note("a", "") }), Some(5000));
        assert_eq!(banner_ms(&Note { expire_timeout: 12000, ..note("a", "") }), Some(12000));
        assert_eq!(banner_ms(&Note { urgency: LOW, ..note("a", "") }), Some(5000));
        assert_eq!(banner_ms(&Note { urgency: CRITICAL, ..note("a", "") }), None);
        assert_eq!(banner_ms(&note("Incoming call", "Ann")), None);
    }

    #[test]
    fn focus_rules() {
        let now = 10_000;
        let off = json!({"active": ""});
        let dnd = json!({"active": "dnd", "until": 0, "modes": [{"id": "dnd", "allowApps": ["Signal"]}]});
        let n = note("a", "");
        assert!(focus_allows(&n, &off, now));
        assert!(focus_allows(&n, &Value::Null, now));
        assert!(!focus_allows(&n, &dnd, now));
        assert!(focus_allows(&Note { urgency: CRITICAL, ..n.clone() }, &dnd, now));
        assert!(focus_allows(&Note { app_name: "signal".into(), ..n.clone() }, &dnd, now));
        assert!(focus_allows(&Note { app_name: "x".into(), desktop_entry: "SIGNAL".into(), ..n.clone() }, &dnd, now));
        // its time ran out (Focus.qml turns it off on its next tick)
        assert!(focus_allows(&n, &json!({"active": "dnd", "until": 5000, "modes": []}), now));
        assert!(!focus_allows(&n, &json!({"active": "dnd", "until": 50000}), now));
        // a mode that isn't there (removed)
        assert!(focus_allows(&n, &json!({"active": "gone", "modes": [{"id": "dnd"}]}), now));
    }

    #[test]
    fn routes() {
        let now = 0;
        let off = Value::Null;
        let n = note("Ann", "Hi");
        let cfg = json!({});
        assert_eq!(route(&n, &cfg, &off, now, true), Route::Banner);
        let cards = json!({"island": {"enabled": true, "events": ["notifications"]}});
        assert_eq!(route(&n, &cards, &off, now, true), Route::Island);
        // critical ones are banners even then; without the shell, banners
        assert_eq!(route(&Note { urgency: CRITICAL, ..n.clone() }, &cards, &off, now, true), Route::Banner);
        assert_eq!(route(&n, &cards, &off, now, false), Route::Banner);
        assert_eq!(route(&n, &json!({"island": {"enabled": false, "events": ["notifications"]}}), &off, now, true), Route::Banner);
        // calls: the island's call card by default (a newer kind), a banner when switched off
        let call = note("Incoming call", "Ann");
        assert_eq!(route(&call, &cfg, &off, now, true), Route::Call);
        assert_eq!(route(&call, &json!({"island": {"events": [], "known": ["calls"]}}), &off, now, true), Route::Banner);
        // Focus holds it
        assert_eq!(route(&n, &cfg, &json!({"active": "dnd"}), now, true), Route::Quiet);
        assert!(island_wants(&json!({}), "calls") && !island_wants(&json!({}), "notifications"));
    }

    #[test]
    fn time_labels() {
        let clock = |_| "9:41 AM".to_string();
        assert_eq!(time_label(0, 5, clock), "");
        assert_eq!(time_label(1000, 30_000, clock), "now");
        assert_eq!(time_label(1000, 1000 + 5 * 60_000, clock), "5m ago");
        assert_eq!(time_label(1000, 1000 + 61 * 60_000, clock), "9:41 AM");
    }

    #[test]
    fn grouping() {
        let mk = |id, app: &str| Note { id, app_name: app.into(), ..Default::default() };
        let list = vec![mk(1, "Mail"), mk(2, "Chat"), mk(3, "Mail"), mk(4, "")];
        let g = groups(&list);
        assert_eq!(g.iter().map(|g| g.0.as_str()).collect::<Vec<_>>(), vec!["Notification", "Mail", "Chat"]);
        assert_eq!(g[1].1.iter().map(|n| n.id).collect::<Vec<_>>(), vec![3, 1]);
    }

    #[test]
    fn the_list_keeps_200_and_replaces_in_place() {
        let mut s = Store::default();
        for id in 1..=KEEP as u32 {
            assert_eq!(s.add(Note { id, at: id as i64, ..Default::default() }), Added::New(vec![]));
        }
        assert_eq!(s.add(Note { id: 999, ..Default::default() }), Added::New(vec![1]));
        assert_eq!(s.list.len(), KEEP);
        assert_eq!(s.add(Note { id: 50, summary: "new".into(), at: 77, route: "x".into(), ..Default::default() }), Added::Replaced);
        let n = s.get(50).unwrap();
        assert_eq!((n.summary.as_str(), n.at, n.route.as_str()), ("new", 50, ""));
        assert_eq!(s.list[48].id, 50);
        assert!(s.remove(50).is_some() && !s.has(50));
        assert_eq!(s.next_id(), 1000);
    }

    #[test]
    fn history_round_trip() {
        let mut s = Store::default();
        let n = note_from(3, "Chat", "chat", "Ann", "<b>Hi</b>", &["default".into(), "Open".into()], &Hints { urgency: Some(2), ..Default::default() }, -1, "/tmp/x.png", 42);
        s.add(Note { route: "banner".into(), ..n.clone() });
        s.add(Note { id: 4, transient: true, ..Default::default() });
        let text = history_json("bus-1", &s);
        let back = history_read(&text, "bus-1");
        assert_eq!(back.list, vec![Note { route: "banner".into(), ..n }]);
        // the names the shell's mirror reads (Quickshell's)
        let v: Value = serde_json::from_str(&text).unwrap();
        for k in ["id", "appName", "appIcon", "desktopEntry", "summary", "body", "image", "actions", "urgency", "expireTimeout", "resident", "hasInlineReply", "inlineReplyPlaceholder", "at"] {
            assert!(v["list"][0].get(k).is_some(), "{k}");
        }
        // another session's list, or a broken file: nothing
        assert!(history_read(&text, "bus-2").list.is_empty());
        assert!(history_read("{oops", "bus-1").list.is_empty());
        assert!(history_read(&text, "").list.is_empty());
        // a file from a later version with more fields still reads
        let more = text.replacen("\"bus\"", "\"extra\":1,\"bus\"", 1);
        assert_eq!(history_read(&more, "bus-1").list.len(), 1);
    }
}

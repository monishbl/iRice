//! Calls.qml: a call or meeting in progress (an app made for calls using the
//! microphone, or a browser with a meeting open) for the island's live
//! activity, and the ringing: an incoming call (a notification the
//! notifications surface routed "call") becomes a call card in the island
//! (kind "ringing"): the caller, Decline and Answer. It rings (the ringtone
//! every 2.6 s, about 30 s), and stays until answered, declined, or the app
//! takes it back; unanswered, it goes to the island's history as a missed
//! call. Answer uses the notification's own Answer/Accept, else its default
//! action, else opens the app; Decline its Decline/Reject, else dismisses it.
//!
//! The ringing card's buttons (the island's Answer / Decline) reach this by
//! the ipc target "calls" (`answer`, `decline`, `state`), as the QML's did.
//!
//! The call in progress needs the microphone's streams (Privacy.micStreams,
//! the system host): read from the shell state `privacy.micStreams`
//! ([{ bin, name }]) when that host says them; without, no call is shown.
//! Published as the state field `calls`: { active, name, leave, appIcon,
//! started, ringing }.

use super::common::*;
use serde_json::{json, Value};
use tessera_core::host::Host;
use tessera_core::services::apps::Apps;
use tessera_core::services::clients::{Client, Windows};
use tessera_core::services::notify::markup;
use tessera_core::services::notify::model::Note;

struct App {
    name: &'static str,
    id: fn(&str) -> bool,
    window: fn(&str) -> bool,
    call_title: Option<fn(&str) -> bool>,
}

fn has(s: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| s.contains(n))
}

fn apps() -> [App; 10] {
    [
        App { name: "Discord", id: |s| has(s, &["discord", "vesktop", "webcord"]), window: |s| has(s, &["discord", "vesktop"]), call_title: None },
        App { name: "Zoom", id: |s| s.starts_with("zoom") || has(s, &["zoom.real", "zoomwebview"]), window: |s| s == "zoom", call_title: Some(|t| t.starts_with("zoom meeting") || t == "meeting") },
        App { name: "Teams", id: |s| s.contains("teams"), window: |s| s.contains("teams"), call_title: None },
        App { name: "Slack", id: |s| s.contains("slack"), window: |s| s.contains("slack"), call_title: Some(|t| t.contains("huddle")) },
        App { name: "Skype", id: |s| s.contains("skype"), window: |s| s.contains("skype"), call_title: None },
        App { name: "Signal", id: |s| s.contains("signal"), window: |s| s.contains("signal"), call_title: None },
        App { name: "Telegram", id: |s| s.contains("telegram"), window: |s| s.contains("telegram"), call_title: None },
        App { name: "Element", id: |s| s.contains("element"), window: |s| s.contains("element"), call_title: None },
        App { name: "WhatsApp", id: |s| has(s, &["whatsapp", "zapzap"]), window: |s| has(s, &["whatsapp", "zapzap"]), call_title: None },
        App { name: "Webex", id: |s| s.contains("webex"), window: |s| s.contains("webex"), call_title: None },
    ]
}

fn is_browser(s: &str) -> bool {
    has(s, &["firefox", "librewolf", "zen", "floorp", "waterfox", "chrom", "brave", "vivaldi", "edge", "opera", "epiphany", "qutebrowser"])
}

fn meeting_in(title: &str) -> Option<&'static str> {
    let t = title.to_lowercase();
    if t.contains("google meet") || t.starts_with("meet -") || t.starts_with("meet \u{2013}") || t.contains("meet.google") {
        Some("Google Meet")
    } else if t.contains("zoom") {
        Some("Zoom")
    } else if t.contains("microsoft teams") || t.contains("| teams") {
        Some("Teams")
    } else if t.contains("jitsi") {
        Some("Jitsi Meet")
    } else if t.contains("discord") {
        Some("Discord")
    } else if t.contains("whereby") {
        Some("Whereby")
    } else if t.contains("webex") {
        Some("Webex")
    } else if t.contains("slack") {
        Some("Slack")
    } else if t.contains("whatsapp") {
        Some("WhatsApp")
    } else {
        None
    }
}

/// What describe() works out about the caller and the app
#[derive(Clone, Debug, Default)]
pub struct Info {
    pub name: String,
    pub app: String,
    pub app_icon: String,
    pub photo: String,
    pub initials: String,
    pub open_only: bool,
    pub can_answer: bool,
    /// the known app's window test (index into apps()), else by keys
    pub known: Option<usize>,
    pub keys: Vec<String>,
    pub entry_id: String,
}

pub struct Ringing {
    pub n: Note,
    pub uid: i64,
    pub info: Info,
    pub at: i64,
}

pub struct Answered {
    pub app: String,
    pub app_icon: String,
    pub known: Option<usize>,
    pub keys: Vec<String>,
    pub at: i64,
    pub before: Vec<String>,
}

#[derive(Default)]
pub struct Calls {
    pub ringing: Option<Ringing>,
    pub answered: Option<Answered>,
    pub rings: u32,
    pub ring_file: String,
    pub chime: slint::Timer,
    pub give_up: slint::Timer,
    /// since when a call is active (ms), 0 none
    pub started: i64,
    pub was_active: bool,
    pub answered_since: i64,
    last_published: String,
}

fn plain(x: &str) -> String {
    markup::plain(x).trim().to_string()
}

fn flat(s: &str) -> String {
    s.to_lowercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect()
}

fn phone_only(text: &str) -> bool {
    let t = format!(" {} ", text.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" "));
    ["check your phone", "check your mobile", "answer on your phone", "answer it on your phone", "use your phone", "on your phone"].iter().any(|p| t.contains(&format!(" {p} ")))
}

fn word_in(text: &str, words: &[&str]) -> bool {
    let t = text.to_lowercase();
    t.split(|c: char| !c.is_alphanumeric()).any(|w| words.contains(&w)) || words.iter().any(|w| w.contains(' ') && t.contains(w))
}

const ANSWER: &[&str] = &["answer", "accept", "pickup", "pick up", "join"];
const DECLINE: &[&str] = &["decline", "reject", "dismiss", "ignore", "hangup", "hang up", "deny", "busy"];

fn action_for<'a>(n: &'a Note, words: &[&str]) -> Option<&'a tessera_core::services::notify::model::Action> {
    n.actions.iter().find(|a| a.id != "inline-reply" && a.id != "default" && (word_in(&a.id, words) || word_in(&a.text, words)))
}

/// Calls.describe
pub fn describe(h: &Host, n: &Note) -> Info {
    let apps_list = h.st.shell.get::<Apps>();
    let entry = apps_list.and_then(|a| a.entry_for(if n.desktop_entry.is_empty() { &n.app_name } else { &n.desktop_entry }));
    let ids = format!("{} {}", n.desktop_entry, n.app_name).to_lowercase();
    let known = apps().iter().position(|a| (a.id)(&ids));
    let app = known.map(|i| apps()[i].name.to_string()).unwrap_or_else(|| {
        if !n.app_name.is_empty() {
            n.app_name.clone()
        } else {
            entry.map(|e| e.name.clone()).filter(|x| !x.is_empty()).unwrap_or_else(|| "Call".into())
        }
    });
    let (summary, body) = (plain(&n.summary), plain(&n.body));
    let sl = summary.to_lowercase();
    let name = if (sl.contains("call") || sl.contains("calling")) && !body.is_empty() && body.chars().count() <= 40 && !body.to_lowercase().contains("call") {
        body.clone()
    } else if !summary.is_empty() {
        summary.clone()
    } else {
        app.clone()
    };
    let initials: String = name
        .split_whitespace()
        .filter(|w| w.chars().next().is_some_and(|c| c.to_uppercase().ne(c.to_lowercase()) || c.is_ascii_digit()))
        .take(2)
        .filter_map(|w| w.chars().next())
        .flat_map(|c| c.to_uppercase())
        .collect();
    let keys: Vec<String> = [n.desktop_entry.as_str(), n.app_name.as_str(), entry.map_or("", |e| e.id.as_str())]
        .iter()
        .map(|k| flat(k.trim_end_matches(".desktop")))
        .filter(|k| k.len() >= 3)
        .collect();
    Info {
        name,
        app,
        app_icon: if !n.app_icon.is_empty() { n.app_icon.clone() } else { entry.map(|e| e.icon.clone()).unwrap_or_else(|| "dialog-information".into()) },
        photo: n.image.clone(),
        initials,
        open_only: phone_only(&format!("{body} {summary}")),
        can_answer: action_for(n, ANSWER).is_some(),
        known,
        keys,
        entry_id: entry.map(|e| e.id.clone()).unwrap_or_default(),
    }
}

fn window_of(known: Option<usize>, keys: &[String], c: &Client) -> bool {
    let cls = c.class.to_lowercase();
    if cls.is_empty() {
        return false;
    }
    if let Some(i) = known {
        return (apps()[i].window)(&cls);
    }
    let f = flat(&cls);
    keys.iter().any(|k| f.contains(k.as_str()) || (f.len() >= 4 && k.contains(f.as_str())))
}

impl Calls {
    fn windows<'a>(&self, h: &'a Host) -> Vec<&'a Client> {
        h.st.shell.get::<Windows>().map(|w| w.clients.iter().collect()).unwrap_or_default()
    }

    /// Is a call or meeting on right now: (name, window address, leave, appIcon)
    pub fn current(&self, h: &Host) -> Option<(String, String, bool, String)> {
        let on = tessera_core::services::notify::model::island_on(h.cfg()) && tessera_core::services::notify::model::island_wants(h.cfg(), "calls");
        if !on {
            return None;
        }
        let wins = self.windows(h);
        let streams = h.state("privacy.micStreams").as_array().cloned().unwrap_or_default();
        let list = apps();
        for s in &streams {
            let id = format!("{} {}", s["bin"].as_str().unwrap_or(""), s["name"].as_str().unwrap_or("")).trim().to_lowercase();
            if let Some(i) = list.iter().position(|a| (a.id)(&id)) {
                let a = &list[i];
                let own: Vec<&&Client> = wins.iter().filter(|c| (a.window)(&c.class.to_lowercase())).collect();
                let call_win = a.call_title.and_then(|f| own.iter().find(|c| f(&c.title.to_lowercase())));
                let mine = self.answered.as_ref().is_some_and(|x| x.app == a.name);
                let win = call_win.or(own.first()).map(|c| c.address.clone()).unwrap_or_default();
                let icon = if mine { self.answered.as_ref().map(|x| x.app_icon.clone()).unwrap_or_default() } else { String::new() };
                return Some((if mine { "Call".into() } else { a.name.into() }, win, call_win.is_some(), icon));
            }
            if is_browser(&id) {
                for t in &wins {
                    if !is_browser(&t.class.to_lowercase()) {
                        continue;
                    }
                    if let Some(m) = meeting_in(&t.title) {
                        return Some((m.into(), t.address.clone(), false, String::new()));
                    }
                }
            }
        }
        // Answered in the island, and the app opened a window for the call
        if let Some(a) = &self.answered {
            let own: Vec<&&Client> = wins.iter().filter(|c| window_of(a.known, &a.keys, c)).collect();
            let call_win = own.iter().find(|c| {
                let t = c.title.to_lowercase();
                word_in(&t, &["call", "calling", "meeting", "huddle"]) || (!a.before.is_empty() && !a.before.contains(&c.address))
            });
            if let Some(c) = call_win {
                return Some(("Call".into(), c.address.clone(), true, a.app_icon.clone()));
            }
        }
        None
    }

    /// Once a second: the live call, published for the island
    pub fn tick(&mut self, h: &mut Host) {
        let cur = self.current(h);
        let active = cur.is_some();
        if active != self.was_active {
            // (an answered call counts from the moment it was answered)
            self.started = if !active {
                0
            } else if self.answered.as_ref().is_some_and(|a| now() - a.at < 60_000) {
                self.answered.as_ref().map_or(now(), |a| a.at)
            } else {
                now()
            };
            if !active && self.answered.is_some() && self.answered_since > 0 {
                self.answered = None; // that call is over
            }
            self.answered_since = if active { now() } else { 0 };
            self.was_active = active;
        }
        // Answered, and nothing showed up for 20 s: given up
        if !active && self.answered.as_ref().is_some_and(|a| now() - a.at > 20_000) {
            self.answered = None;
        }
        let v = match &cur {
            Some((name, win, leave, icon)) => json!({"active": true, "name": name, "window": win, "leave": leave, "appIcon": icon, "started": self.started, "ringing": self.ringing.is_some()}),
            None => json!({"active": false, "started": 0, "ringing": self.ringing.is_some()}),
        };
        let s = v.to_string();
        if s != self.last_published {
            self.last_published = s;
            h.set_state("calls", v);
        }
    }

    // ── Ringing ─────────────────────────────────────────────────────────

    fn card(info: &Info, n: &Note) -> Value {
        json!({
            "kind": "ringing", "sticky": true, "nid": n.id, "title": info.name, "subtitle": info.app,
            "call": {"name": info.name, "app": info.app, "appIcon": info.app_icon, "photo": info.photo, "initials": info.initials, "openOnly": info.open_only, "canAnswer": info.can_answer},
        })
    }

    /// Shows a ringing call in the island; false when it can't (off), so it
    /// stays a banner
    pub fn ring(&mut self, h: &mut Host, n: &Note) -> bool {
        use tessera_core::services::notify::model as nm;
        if !nm::island_on(h.cfg()) || !nm::island_wants(h.cfg(), "calls") {
            return false;
        }
        let info = describe(h, n);
        let card = Self::card(&info, n);
        // The app changed it (same id): the card follows
        if let Some(r) = &mut self.ringing {
            if r.n.id == n.id {
                r.info = info;
                r.n = n.clone();
                island_update(h, r.uid, card);
                return true;
            }
        }
        if self.ringing.is_some() {
            self.missed(h); // a second call while one rings: the first one goes by
        }
        let uid = island_post(h, card, 120_000);
        self.ringing = Some(Ringing { n: n.clone(), uid, info, at: now() });
        self.rings = 0;
        self.ring_file = if n.suppress_sound {
            String::new()
        } else if n.sound_file.starts_with('/') {
            n.sound_file.clone()
        } else if sound_allowed(h) {
            ringtone_file(h.cfg())
        } else {
            String::new()
        };
        let send = h.sender(super::NAME);
        let s2 = send.clone();
        self.chime.start(slint::TimerMode::Repeated, std::time::Duration::from_millis(2600), move || send(Box::new(super::Msg::Ring)));
        self.give_up.start(slint::TimerMode::SingleShot, std::time::Duration::from_secs(120), move || s2(Box::new(super::Msg::RingGiveUp)));
        self.ring_tick();
        true
    }

    /// The island alert of the call that rings now (its buttons: 1 Answer, 2 Decline)
    pub fn ringing_uid(&self) -> Option<i64> {
        self.ringing.as_ref().map(|r| r.uid)
    }

    /// The ringtone: every 2.6 s, quiet after about 30 s (the card stays)
    pub fn ring_tick(&mut self) {
        if self.ringing.is_none() || self.ring_file.is_empty() {
            self.chime.stop();
            return;
        }
        self.rings += 1;
        if self.rings > 12 {
            self.chime.stop();
            return;
        }
        play_file(&self.ring_file);
    }

    fn stop_ringing(&mut self, h: &mut Host) -> Option<Ringing> {
        let r = self.ringing.take();
        self.chime.stop();
        self.give_up.stop();
        if let Some(r) = &r {
            island_remove(h, r.uid);
        }
        r
    }

    pub fn answer(&mut self, h: &mut Host) {
        let Some(r) = self.stop_ringing(h) else { return };
        if !r.info.open_only {
            // Its call window (or microphone) shows the live activity
            let before = self.windows(h).iter().filter(|c| window_of(r.info.known, &r.info.keys, c)).map(|c| c.address.clone()).collect();
            self.answered = Some(Answered { app: r.info.app.clone(), app_icon: r.info.app_icon.clone(), known: r.info.known, keys: r.info.keys.clone(), at: now(), before });
        }
        let a = if r.info.open_only { None } else { action_for(&r.n, ANSWER) };
        let def = r.n.actions.iter().find(|x| x.id == "default");
        match (a, def) {
            (Some(a), _) => h.tell("notifications", json!({"id": r.n.id, "do": "invoke", "action": a.id})),
            (None, Some(d)) => h.tell("notifications", json!({"id": r.n.id, "do": "invoke", "action": d.id})),
            _ => self.open_app(h, &r.info),
        }
    }

    pub fn decline(&mut self, h: &mut Host) {
        let Some(r) = self.stop_ringing(h) else { return };
        match action_for(&r.n, DECLINE) {
            Some(a) => h.tell("notifications", json!({"id": r.n.id, "do": "invoke", "action": a.id})),
            None => h.tell("notifications", json!({"id": r.n.id, "do": "dismiss"})),
        }
    }

    /// Unanswered (the caller hung up, or nothing happened for 2 minutes):
    /// a line in the island's history (Recent, Notification Center)
    pub fn missed(&mut self, h: &mut Host) {
        let Some(r) = self.stop_ringing(h) else { return };
        let who = if r.info.name != r.info.app { format!("{} · {}", r.info.name, r.info.app) } else { r.info.name.clone() };
        let red = accent(h, "red");
        // (the history line; its "Open <app>" button reaches `calls open`)
        h.say(json!({"src": "shell", "kind": "island", "op": "record", "event": {
            "kind": "missedCall", "icon": "phone-x", "title": "Missed call", "subtitle": who, "color": red, "action": format!("Open {}", r.info.app), "actionCall": ["calls", "openApp", r.info.entry_id, r.info.keys.first().cloned().unwrap_or_default()]
        }}));
    }

    /// Brings up the app's window, or starts it
    pub fn open_app(&mut self, h: &mut Host, info: &Info) {
        let wins = self.windows(h);
        if let Some(c) = wins.iter().rev().find(|c| window_of(info.known, &info.keys, c)) {
            let a = c.address.clone();
            h.ask("apps.show", json!([a]));
        } else if !info.entry_id.is_empty() {
            h.ask("apps.launch", json!([info.entry_id]));
        }
    }

    /// The notification went away while ringing: the app took it back (the
    /// caller hung up) is a missed call; you dismissing it is a decline
    pub fn note_gone(&mut self, h: &mut Host, id: u32, dismissed: bool) {
        if self.ringing.as_ref().is_some_and(|r| r.n.id == id) {
            if dismissed {
                self.stop_ringing(h);
            } else {
                self.missed(h);
            }
        }
    }

    /// Calls.open: bring up the call's window
    pub fn open(&mut self, h: &mut Host) {
        if let Some((_, win, _, _)) = self.current(h) {
            if !win.is_empty() {
                h.ask("apps.show", json!([win]));
            }
        }
    }

    /// Calls.leave
    pub fn leave(&mut self, h: &mut Host) {
        match self.current(h) {
            Some((_, win, true, _)) if !win.is_empty() => tessera_core::services::clients::close(&win),
            _ => self.open(h),
        }
    }

    pub fn ipc(&mut self, h: &mut Host, func: &str, args: &[String]) -> Option<String> {
        match func {
            "answer" => self.answer(h),
            "decline" => self.decline(h),
            "open" => self.open(h),
            "leave" => self.leave(h),
            "openApp" => {
                let info = Info { entry_id: args.first().cloned().unwrap_or_default(), keys: args.get(1).cloned().into_iter().collect(), ..Default::default() };
                self.open_app(h, &info);
            }
            "state" => {
                let cur = self.current(h);
                return Some(
                    json!({
                        "ringing": self.ringing.as_ref().map(|r| json!({"name": r.info.name, "app": r.info.app, "openOnly": r.info.open_only, "canAnswer": r.info.can_answer, "rings": self.rings})),
                        "answered": self.answered.as_ref().map(|a| a.app.clone()),
                        "call": cur.map(|(n, _, l, _)| json!({"name": n, "leave": l})),
                    })
                    .to_string(),
                );
            }
            _ => {}
        }
        None
    }
}

fn sound_allowed(h: &Host) -> bool {
    let s = &h.cfg()["sounds"];
    s["enabled"].as_bool().unwrap_or(true) && s["notifications"].as_bool().unwrap_or(true)
}

/// Sounds.file("ringtone") for the style playing now
fn ringtone_file(cfg: &Value) -> String {
    let packs = ["marimba", "taps", "soft", "keys", "minimal", "glass"];
    let material = cfg["appearance"]["look"] == "material" && cfg["appearance"]["materialSounds"].as_bool().unwrap_or(true);
    let pack = if material { "material" } else { cfg["sounds"]["pack"].as_str().filter(|p| packs.contains(p)).unwrap_or("marimba") };
    if let Some(custom) = cfg["sounds"]["notificationsFile"].as_str().filter(|f| !f.is_empty()) {
        return custom.to_string();
    }
    tessera_core::util::asset(&format!("shell/assets/sounds/{pack}/ringtone.ogg")).map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()
}

pub fn play_file(f: &str) {
    if f.is_empty() {
        return;
    }
    let vol = h_volume();
    if tessera_core::util::on_path("pw-play") {
        detached(&["pw-play", "--volume", &format!("{vol:.2}"), f]);
    } else if tessera_core::util::on_path("paplay") {
        detached(&["paplay", "--volume", &((vol * 65536.0).round() as i64).to_string(), f]);
    }
}

fn h_volume() -> f64 {
    cfg_get("sounds.volume").as_f64().unwrap_or(0.7).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apps_are_found_by_what_they_say() {
        let l = apps();
        assert!(l.iter().any(|a| a.name == "Discord" && (a.id)("vesktop")));
        assert!(l.iter().any(|a| a.name == "Zoom" && (a.id)("zoom.real")));
        assert!(!(l[1].window)("zoomwebview"));
        assert_eq!(meeting_in("Meet - abc-defg"), Some("Google Meet"));
        assert!(is_browser("firefox"));
    }

    #[test]
    fn phone_only_calls_get_open() {
        assert!(phone_only("Check your phone to answer"));
        assert!(!phone_only("Maya is calling"));
        assert!(word_in("Accept", ANSWER));
        assert!(word_in("Hang up", DECLINE));
    }
}

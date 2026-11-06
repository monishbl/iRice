//! tessera-events and what wants its sources: shell/services/Events.qml and
//! EventWatch.qml, and the startup queue (Startup.qml with StartupSounds.js).
//!
//! tessera-events (native/tessera-events) is one process that follows the
//! system for the whole shell (logind, udev, the sound server, the network,
//! badges, Cast devices, the camera, location, screen sharing, copies, the
//! clipboard history) and holds logind inhibitors. A service that wants a
//! source says so with a call; the events come back as messages to that service.
//!
//!   events.watch  [id, src, on, args?]   an EventWatch: `id` is the wisher's own
//!                 (unique); the source is on while anyone wants it, its args merge
//!                 (lists join, the rest: last). "inhibit:<what>" holds a logind
//!                 inhibitor (args.why)
//!   events.subscribe [service name]       that service's `message` gets every
//!                 event as a serde_json::Value `{"events": <src>, …the line…}`
//!   Events.covers(src) / fallsBack(src): state "events.ready", "events.decided",
//!                 "events.unsupported" ({src: true}), "events.pulseUp"
//!   startup.later [service name, tag]     a Value `{"startup": tag}` goes to that
//!                 service's `message` 15 s after the shell starts (one every
//!                 0.4 s), or at once when that time has passed

use crate::common::*;
use serde_json::{json, Map, Value};
use std::any::Any;
use std::collections::BTreeMap;
use tessera_core::host::{Host, Service};

enum Msg {
    /// a line tessera-events wrote
    Line(Value),
    /// it ended (started: it ran at all)
    Ended(bool),
    /// 3 s without "ready"
    Decide,
}

#[derive(Default)]
pub struct Events {
    found: Option<String>,
    kept: Option<Kept>,
    ready: bool,
    decided: bool,
    unsupported: Map<String, Value>,
    pulse_up: bool,
    quick_ends: u32,
    started_at: i64,
    /// id -> (src, args)
    wishes: BTreeMap<String, (String, Value)>,
    /// src -> the command sent for it last
    sent: BTreeMap<String, String>,
    subscribers: Vec<&'static str>,
}

/// The binary: $TESSERA_EVENTS, else <data>/bin/tessera-events, else the one
/// built while Tessera was called iRice
fn find_binary() -> Option<String> {
    let mut list: Vec<String> = Vec::new();
    if let Some(p) = tessera_core::env::var("EVENTS").filter(|p| !p.is_empty()) {
        list.push(p);
    }
    list.push(data_dir().join("bin/tessera-events").to_string_lossy().into_owned());
    list.push(data_dir().join("bin/irice-events").to_string_lossy().into_owned());
    use std::os::unix::fs::PermissionsExt;
    list.into_iter().find(|p| std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0))
}

/// Two wishes' args merged: lists join, the rest: last
pub fn merge_args(into: &mut Map<String, Value>, from: &Value) {
    let Value::Object(from) = from else { return };
    for (k, v) in from {
        match (v.as_array(), into.get_mut(k).and_then(Value::as_array_mut)) {
            (Some(new), Some(old)) => {
                for x in new {
                    if !old.contains(x) {
                        old.push(x.clone());
                    }
                }
            }
            _ => {
                into.insert(k.clone(), v.clone());
            }
        }
    }
}

impl Events {
    fn publish(&self, h: &mut Host) {
        h.set_state("events", json!({"ready": self.ready, "decided": self.decided, "unsupported": self.unsupported, "pulseUp": self.pulse_up}));
    }

    /// Events.sync: tell the process what a source should do now
    fn sync(&mut self, src: &str) {
        if !self.ready || src.is_empty() {
            return;
        }
        let mut on = false;
        let mut args = Map::new();
        for (s, a) in self.wishes.values() {
            if s == src {
                on = true;
                merge_args(&mut args, a);
            }
        }
        if !on && !self.sent.contains_key(src) {
            return; // (never turned on: nothing to turn off)
        }
        let cmd = match src.strip_prefix("inhibit:") {
            Some(what) => json!({"cmd": "inhibit", "what": what, "why": args.get("why").and_then(Value::as_str).unwrap_or(""), "on": on}),
            None => json!({"cmd": "watch", "src": src, "on": on, "args": args}),
        };
        let text = cmd.to_string();
        if self.sent.get(src) == Some(&text) {
            return;
        }
        self.sent.insert(src.to_string(), text.clone());
        if let Some(k) = &self.kept {
            k.write(&text);
        }
    }

    fn handle(&mut self, h: &mut Host, e: Value) {
        let src = e["src"].as_str().unwrap_or("").to_string();
        if src == "events" {
            match e["kind"].as_str().unwrap_or("") {
                "ready" => {
                    self.unsupported.clear();
                    self.sent.clear();
                    self.ready = true;
                    self.decided = true;
                    let srcs: std::collections::BTreeSet<String> = self.wishes.values().map(|w| w.0.clone()).collect();
                    for s in srcs {
                        self.sync(&s);
                    }
                    self.publish(h);
                }
                "error" => eprintln!("tessera-events: {}", e["msg"].as_str().unwrap_or("")),
                _ => {}
            }
            return;
        }
        if e["kind"] == "unsupported" {
            eprintln!("tessera-events: no {} here ({}): using its watcher", e["src"].as_str().unwrap_or(""), e["error"].as_str().unwrap_or(""));
            self.unsupported.insert(src, json!(true));
            self.publish(h);
            return;
        }
        if src == "pulse" {
            match e["kind"].as_str().unwrap_or("") {
                "up" => self.pulse_up = true,
                "down" => self.pulse_up = false,
                _ => {}
            }
            self.publish(h);
        }
        // every event goes to whoever subscribed (the QML signals)
        let mut out = e.clone();
        out["events"] = json!(src);
        for name in self.subscribers.clone() {
            h.sender(name)(Box::new(out.clone()));
        }
    }
}

impl Service for Events {
    fn name(&self) -> &'static str {
        "Events"
    }
    fn calls(&self) -> &'static [&'static str] {
        &["events"]
    }
    fn start(&mut self, h: &mut Host) {
        self.started_at = now_ms();
        self.found = find_binary();
        let send = h.sender("Events");
        // One that never says it's ready: watchers stand in meanwhile
        {
            let send = send.clone();
            slint::Timer::single_shot(std::time::Duration::from_secs(3), move || send(Box::new(Msg::Decide)));
        }
        match self.found.clone() {
            Some(bin) => {
                let (s1, s2) = (send.clone(), send);
                self.kept = Some(Kept::start(
                    vec![bin],
                    move |line| {
                        if let Ok(v) = serde_json::from_str::<Value>(&line) {
                            s1(Box::new(Msg::Line(v)));
                        }
                    },
                    move |started| s2(Box::new(Msg::Ended(started))),
                ));
            }
            None => {
                self.quick_ends = 3; // (not built)
                self.decided = true;
            }
        }
        self.publish(h);
    }
    fn call(&mut self, h: &mut Host, call: &str, args: &Value) {
        match call.strip_prefix("events.").unwrap_or("") {
            "watch" => {
                let id = args[0].as_str().map(String::from).unwrap_or_else(|| args[0].to_string());
                let src = args[1].as_str().unwrap_or("").to_string();
                let was = self.wishes.get(&id).map(|w| w.0.clone());
                if args[2].as_bool().unwrap_or(true) {
                    self.wishes.insert(id, (src.clone(), args.get(3).cloned().unwrap_or(json!({}))));
                } else {
                    self.wishes.remove(&id);
                }
                self.sync(&src);
                if let Some(w) = was.filter(|w| *w != src) {
                    self.sync(&w); // (its source changed: the old one may go off)
                }
            }
            "subscribe" => {
                let name = args[0].as_str().unwrap_or("");
                // (service names are static in the host's world; a subscriber's is kept for the run)
                if !name.is_empty() && !self.subscribers.iter().any(|s| *s == name) {
                    self.subscribers.push(Box::leak(name.to_string().into_boxed_str()));
                }
            }
            other => eprintln!("tessera-ui: host: no events.{other}"),
        }
        let _ = h;
    }
    fn message(&mut self, h: &mut Host, msg: Box<dyn Any + Send>) {
        let Ok(m) = msg.downcast::<Msg>() else { return };
        match *m {
            Msg::Line(v) => self.handle(h, v),
            Msg::Decide => {
                self.decided = true;
                self.publish(h);
            }
            Msg::Ended(started) => {
                // after three quick ends, or when it can't start at all, the shell gives up on it
                if !started {
                    self.quick_ends = 3;
                    if let Some(k) = &self.kept {
                        k.stop();
                    }
                } else if now_ms() - self.started_at < 60_000 {
                    self.quick_ends += 1;
                } else {
                    self.quick_ends = 0;
                }
                if self.quick_ends >= 3 {
                    if let Some(k) = &self.kept {
                        k.stop();
                    }
                }
                self.started_at = now_ms();
                self.ready = false;
                self.decided = true;
                self.pulse_up = false;
                self.publish(h);
            }
        }
    }
    fn quit(&mut self, _h: &mut Host) {
        if let Some(k) = &self.kept {
            k.stop();
        }
    }
}

// ── Startup.qml ─────────────────────────────────────────────────────────────

/// Startup work that can wait: run 15 s after the shell starts (the desktop
/// first), one every 0.4 s rather than all at once; asked for after that, it
/// runs straight away.
#[derive(Default)]
pub struct Startup {
    settled: bool,
    waiting: std::collections::VecDeque<(String, Value)>,
    next: Option<slint::Timer>,
}

enum Go {
    Settled,
    Next,
}

impl Startup {
    fn deliver(h: &Host, who: &str, tag: Value) {
        h.sender(Box::leak(who.to_string().into_boxed_str()))(Box::new(json!({"startup": tag})));
    }
}

impl Service for Startup {
    fn name(&self) -> &'static str {
        "Startup"
    }
    fn calls(&self) -> &'static [&'static str] {
        &["startup"]
    }
    fn start(&mut self, h: &mut Host) {
        let send = h.sender("Startup");
        slint::Timer::single_shot(std::time::Duration::from_secs(15), move || send(Box::new(Go::Settled)));
    }
    fn call(&mut self, h: &mut Host, _call: &str, args: &Value) {
        let who = args[0].as_str().unwrap_or("").to_string();
        if who.is_empty() {
            return;
        }
        if self.settled {
            Self::deliver(h, &who, args[1].clone());
        } else {
            self.waiting.push_back((who, args[1].clone()));
        }
    }
    fn message(&mut self, h: &mut Host, msg: Box<dyn Any + Send>) {
        let Ok(g) = msg.downcast::<Go>() else { return };
        match *g {
            Go::Settled => {
                self.settled = true;
                let send = h.sender("Startup");
                let t = slint::Timer::default();
                t.start(slint::TimerMode::Repeated, std::time::Duration::from_millis(400), move || send(Box::new(Go::Next)));
                self.next = Some(t);
            }
            Go::Next => match self.waiting.pop_front() {
                Some((who, tag)) => Self::deliver(h, &who, tag),
                None => self.next = None,
            },
        }
    }
}

// ── StartupSounds.js ────────────────────────────────────────────────────────

/// The sounds to log in to (Settings → Notifications → Logging in): the styles,
/// each with four motifs; "" is Tessera's own chime. The files are
/// assets/sounds/startup/<id>.ogg
pub const STARTUP_STYLES: &[(&str, &str)] = &[("soft", "Soft"), ("marimba", "Marimba"), ("keys", "Keys"), ("taps", "Taps"), ("minimal", "Minimal"), ("glass", "Glass"), ("material", "Material")];
pub const STARTUP_MOTIFS: &[&str] = &["rise", "hello", "dawn", "bell"];

/// Is this a sound to log in to ("" is Tessera's own)
pub fn startup_sound_exists(id: &str) -> bool {
    id.is_empty() || STARTUP_STYLES.iter().any(|(s, _)| STARTUP_MOTIFS.iter().any(|m| id == format!("{s}-{m}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_merge_lists_join_and_the_rest_is_last() {
        let mut m = Map::new();
        merge_args(&mut m, &json!({"subsystems": ["usb"], "why": "a"}));
        merge_args(&mut m, &json!({"subsystems": ["usb", "drm"], "why": "b"}));
        assert_eq!(m["subsystems"], json!(["usb", "drm"]));
        assert_eq!(m["why"], json!("b"));
    }

    #[test]
    fn startup_sounds_are_the_styles_and_motifs() {
        assert!(startup_sound_exists(""));
        assert!(startup_sound_exists("glass-bell"));
        assert!(!startup_sound_exists("glass-nothing"));
        assert!(!startup_sound_exists("../x"));
    }
}

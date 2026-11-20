//! What the dynamic island shows: shell/services/Island.qml (+ IslandCycle.js).
//!
//! The island itself is drawn by the menu bar's surface; this service is the
//! half that was QML: the alerts posted (and what their buttons do), the
//! history, the live activities, the hold while the pointer rests on the
//! alerts, the "kinds" a person switched on, the charger and low-battery
//! alerts and the hot-CPU watch. It says the same lines as Island.forward and
//! RustBridge.pushLive: `{"src":"shell","kind":"island","op":…}` and
//! `{"src":"shell","kind":"live","activities":[…]}`.
//!
//! For the other host services (calls, arguments an array):
//!   island.post     [event, ms?]            an alert; `event.gate` names the kind
//!                    in Settings → Notifications & Island it needs (dropped when off);
//!                    onAction, onAction2 and `more[i].act` are `{"call":…,"args":[…]}`
//!                    (what the QML closures did), run through Host::ask
//!   island.update   [uid, patch]
//!   island.remove   [uid]    island.dismiss [since?]    island.find [key] (answers by state "island.found")
//!   island.activity [kind, activity | null]  set or end a live activity (see `Live`)
//!   island.hud      [volume | brightness | mic, device?]  the level row (needs state "hud")
//!   island.temperature [celsius]   the CPU's reading (Resources): the hot watch
//!   island.resources   [cpu 0..1, memory 0..1, usedGb, totalGb]  the busy alerts
//!   island.hot [bool]  (preview)   island.act / hold / pin / live: the menu bar's buttons
//! and by `qs ipc call island …` (post, pin, hot, dismissNewest, holdAlerts,
//! history, clearHistory, timeLeft, demo).

use crate::common::*;
use serde_json::{json, Value};
use std::any::Any;
use std::collections::BTreeMap;
use tessera_core::host::{Host, Service};
use tessera_core::services::notify::live;

/// The alerts kinds that may be shown over a fullscreen app in "important" mode
const IMPORTANT: &[&str] = &["ringing", "battery", "temperature", "timer"];
/// Kinds added after the setting existed: on until the person has touched them
const NEW_KINDS: &[&str] = &["usb", "displays", "audio", "temperature", "network", "transfers", "calls", "sysupdate"];
/// Alerts that are not kept in the history
const UNRECORDED: &[&str] = &["notification", "hud", "music", "ringing"];
/// Alert uids host-desktop makes start here (desktop/src/today/common.rs UID_BASE): not this service's own
const DESKTOP_UID_BASE: i64 = 1_000_000_000;
const MAX_ALERTS: usize = 4;
const HISTORY_MAX: usize = 50;
const LONG_MS: i64 = 600_000;
const UNTIL_OVER: i64 = 86_400_000;

/// The order live activities are listed in (Island.activities)
const LIVE_ORDER: &[&str] = &["mirroring", "casting", "dictation", "captions", "recording", "timer", "music", "stopwatch", "call", "screenShare", "sysupdate", "transfer"];

/// Which pieces of a collapsed live activity are on by default (Island.liveDefaults)
fn live_defaults(kind: &str) -> Option<&'static [&'static str]> {
    Some(match kind {
        "transfer" => &["icon", "name", "time", "open", "cancel"],
        "call" => &["icon", "name", "time", "mute"],
        "screenShare" => &["icon", "name", "time", "stop"],
        "sysupdate" => &["icon", "name", "time"],
        "dictation" => &["icon", "name", "time", "stop"],
        "casting" => &["icon", "name", "time", "play", "stop"],
        "mirroring" => &["icon", "name", "stop"],
        "captions" => &["icon", "name", "stop"],
        _ => return None,
    })
}

#[derive(Clone, Debug, Default)]
struct Alert {
    uid: i64,
    /// what was posted, without the actions
    event: Value,
    on_action: Option<Value>,
    on_action2: Option<Value>,
    more: Vec<Option<Value>>,
    at: i64,
    expires: i64,
    hid: i64,
    sticky: bool,
}

/// Messages from its timers
enum Tick {
    Sweep,
    Live,
}

#[derive(Default)]
pub struct Island {
    /// resources sampled for the busy alerts (Island.qml's Binding on
    /// Resources.islandActive), as last asked
    res_asked: Option<bool>,
    alerts: Vec<Alert>,
    next_uid: i64,
    newest: i64,
    burst_at: i64,
    burst_index: i64,
    started_at: i64,
    history: Vec<Value>,
    next_hid: i64,
    /// the actions of history lines that keep a button (Open a download…)
    history_acts: BTreeMap<i64, (String, Value)>,
    holders: std::collections::BTreeSet<String>,
    held_at: i64,
    /// kind -> the activity as its owner gave it
    live: BTreeMap<String, Value>,
    live_sent: String,
    // the temperature watch
    temps: Vec<(f64, i64)>,
    hot: bool,
    hot_preview: bool,
    hot_uid: i64,
    hot_cpu: u32,
    hot_mem: u32,
    // charger and battery
    battery: Option<(bool, f64, bool)>,
    low_uid: i64,
    sweep: Option<slint::Timer>,
    live_timer: Option<slint::Timer>,
    scroll: Option<Value>,
    /// Focus as last seen (its on/off alert) and the track playing (the new-song alert)
    focus_was: Option<bool>,
    track_was: String,
}

/// A device's drawing (shell/assets/devices/<kind>.svg: Devices.pictureFor's files)
fn device_picture(kind: &str) -> String {
    tessera_core::env::root().join("shell/assets/devices").join(format!("{kind}.svg")).to_string_lossy().into_owned()
}

/// A live control's accessible name: its own, else from its piece ("Pause", "Skip"…)
fn ctl_name(b: &Value) -> String {
    if let Some(n) = b["name"].as_str().filter(|n| !n.is_empty()) {
        return n.to_string();
    }
    let icon = b["icon"].as_str().unwrap_or("");
    match b["piece"].as_str().unwrap_or("") {
        "play" | "pause" => if icon == "pause" { "Pause" } else { "Play" },
        "reset" => "Reset",
        "skip" => "Skip",
        "lap" => "Lap",
        "discard" => "Discard the recording",
        "mute" => if icon.ends_with("slash") { "Unmute the microphone" } else { "Mute the microphone" },
        "stop" => "Stop",
        p => p,
    }
    .to_string()
}

/// The button a history line keeps for an alert: (label, call), for the
/// kinds whose action still means something later (a download, a drive,
/// What's new, a missed call)
fn history_act(e: &Value, kind: &str, act: &Option<Value>) -> Option<(String, Value)> {
    if !e["action"].is_string() || !["transfer", "usb", "sysupdate", "missedCall"].contains(&kind) {
        return None;
    }
    act.clone().map(|a| (s(e, "action"), a))
}

/// Seconds-resolution milliseconds
fn s(v: &Value, k: &str) -> String {
    v[k].as_str().unwrap_or("").to_string()
}

impl Island {
    fn cfg<'a>(&self, h: &'a Host) -> &'a Value {
        h.cfg()
    }

    fn enabled(&self, h: &Host) -> bool {
        cfg_bool(h.cfg(), "island.enabled", true)
    }

    /// CPU, memory and temperature sampled (slowly) while their alerts are on
    fn ask_resources(&mut self, h: &mut Host) {
        let on = self.enabled(h) && (self.wants(h, "resources") || self.wants(h, "temperature"));
        if self.res_asked != Some(on) {
            self.res_asked = Some(on);
            h.ask("resources.use", json!([{"kind": "island", "on": on}]));
        }
    }

    /// Island.wants: an alert kind switched on (Settings → Notifications & Island)
    fn wants(&self, h: &Host, kind: &str) -> bool {
        let c = h.cfg();
        let listed = |path: &str, k: &str| at(c, path).as_array().is_some_and(|l| l.iter().any(|x| x == k));
        listed("island.events", kind) || (NEW_KINDS.contains(&kind) && !listed("island.known", kind))
    }

    fn fs_mode<'a>(&self, h: &'a Host) -> &'a str {
        let m = cfg_str(h.cfg(), "island.fullscreen", "show");
        if ["show", "important", "compact"].contains(&m) {
            m
        } else {
            "show"
        }
    }

    fn important(e: &Value) -> bool {
        e["important"].as_bool().unwrap_or(false) || IMPORTANT.contains(&e["kind"].as_str().unwrap_or(""))
    }

    /// A fullscreen window on the focused display (where you're looking)
    fn fullscreen_now(&self, h: &Host) -> bool {
        h.st.shell.get::<tessera_core::services::hypr::Snapshot>().is_some_and(|s| s.fullscreen())
    }

    fn fs_quiet(&self, h: &Host) -> bool {
        self.enabled(h) && self.fs_mode(h) == "important" && self.fullscreen_now(h)
    }

    fn held(&self) -> bool {
        !self.holders.is_empty()
    }

    /// Island.forward: the call, as a line for the island in Rust
    fn forward(&self, h: &mut Host, op: &str, uid: i64, e: &Value, ms: i64) {
        let mut c = serde_json::Map::new();
        if let Value::Object(m) = e {
            for (k, v) in m {
                if ["onAction", "onAction2", "gate"].contains(&k.as_str()) {
                    continue;
                }
                if k == "more" {
                    let items: Vec<Value> = v.as_array().map(|l| l.iter().map(|m| json!({"text": m["text"]})).collect()).unwrap_or_default();
                    c.insert(k.clone(), Value::Array(items));
                } else {
                    c.insert(k.clone(), v.clone());
                }
            }
        }
        let key = if op == "update" { "patch" } else { "event" };
        h.say(json!({"src": "shell", "kind": "island", "op": op, "uid": uid, "ms": ms, key: Value::Object(c)}));
    }

    // ── Posting ─────────────────────────────────────────────────────────

    /// Island.post: forwards it and keeps its actions; the uid (0 when it was not shown)
    pub fn post(&mut self, h: &mut Host, mut e: Value, ms: i64) -> i64 {
        let gate = e.get("gate").and_then(Value::as_str).map(String::from);
        if gate.as_deref().is_some_and(|g| !self.wants(h, g)) {
            return 0;
        }
        let (uid, quiet) = self.post_here(h, &mut e, ms);
        if uid != 0 {
            // (a live entry's quiet change: in place, no pop: live.rs)
            self.forward(h, if quiet { "update" } else { "post" }, uid, &e, ms);
        }
        uid
    }

    /// An event's state, as live.rs's rules see it: its `severity` (else its
    /// colour's), its `failure` (else what it says)
    fn event_state(e: &Value, at: i64) -> live::State {
        let (t, sub) = (e["title"].as_str().unwrap_or(""), e["subtitle"].as_str().unwrap_or(""));
        let text = if sub.is_empty() { t.to_string() } else { format!("{t} · {sub}") };
        let severity = e["severity"].as_u64().map_or_else(|| live::severity_of_color(e["color"].as_str().unwrap_or("")), |v| v.min(3) as u8);
        let failure = e["failure"].as_bool().unwrap_or_else(|| live::says_failure(&text));
        live::State { severity, failure, text, at }
    }

    /// A history line's (the entry's) latest state
    fn line_state(l: &Value) -> live::State {
        live::State {
            severity: l["severity"].as_u64().unwrap_or(0) as u8,
            failure: l["failure"].as_bool().unwrap_or(false),
            text: l["changes"].as_array().and_then(|c| c.last()).and_then(|c| c["text"].as_str()).unwrap_or("").to_string(),
            at: l["at"].as_i64().unwrap_or(0),
        }
    }

    fn take_actions(e: &mut Value) -> (Option<Value>, Option<Value>, Vec<Option<Value>>) {
        let a = e.get("onAction").filter(|x| x.is_object()).cloned();
        let a2 = e.get("onAction2").filter(|x| x.is_object()).cloned();
        let more = e["more"].as_array().map(|l| l.iter().map(|m| m.get("act").filter(|x| x.is_object()).or(m.get("call").map(|_| m)).cloned()).collect()).unwrap_or_default();
        (a, a2, more)
    }

    /// The uid it shows as (0: not shown), and whether that's a quiet
    /// change of a live entry (no pop)
    fn post_here(&mut self, h: &mut Host, e: &mut Value, ms: i64) -> (i64, bool) {
        if !self.enabled(h) || now_ms() - self.started_at < 4000 {
            return (0, false); // (ignore the initial values)
        }
        // A live entry (core notify/live.rs): Tessera's stateful alerts are
        // one entry each, by their key (or kind), changed in place
        let key = live::island_key(e["kind"].as_str().unwrap_or(""), e["key"].as_str().unwrap_or(""));
        if !key.is_empty() && e["key"].as_str().map_or(true, str::is_empty) {
            e["key"] = json!(key);
        }
        // (Settings' previews show every time: not an entry)
        if !key.is_empty() && !e["demo"].as_bool().unwrap_or(false) {
            let resolved = live::Resolve::from_str(e["resolved"].as_str().unwrap_or(""));
            if resolved == live::Resolve::Remove {
                // over: its card and its entry go
                let gone: Vec<i64> = self.alerts.iter().filter(|a| a.event["key"].as_str() == Some(key.as_str())).map(|a| a.uid).collect();
                for u in gone {
                    self.remove(h, u, 0);
                }
                if self.history.iter().any(|l| l["key"].as_str() == Some(key.as_str())) {
                    self.history.retain(|l| l["key"].as_str() != Some(key.as_str()));
                    self.publish_history(h);
                }
                return (0, false);
            }
            let now = now_ms();
            let new = Self::event_state(e, now);
            let was = self.history.iter().find(|l| l["key"].as_str() == Some(key.as_str())).map(Self::line_state);
            let up = self.alerts.iter().position(|x| x.event["key"].as_str() == Some(key.as_str()));
            // (its card gone a while: what happens now is news, shown)
            let again = was.as_ref().map_or(true, |w| live::realert(w, &new) || (up.is_none() && new.at - w.at >= live::THREAD_MS)) && resolved == live::Resolve::No;
            if !again && was.is_some() {
                let hid = self.record(h, e, 0);
                match up {
                    // its card is up: changed in place, quietly (its time as it was)
                    Some(i) => {
                        let (a, a2, more) = Self::take_actions(e);
                        let al = &mut self.alerts[i];
                        al.event = e.clone();
                        (al.on_action, al.on_action2, al.more, al.hid) = (a, a2, more, hid);
                        return (al.uid, true);
                    }
                    // not up: only its entry changes
                    None => return (0, false),
                }
            }
        }
        // Not now (a fullscreen app, "Only important"): only into the history
        if self.fs_quiet(h) && !Self::important(e) {
            self.record(h, e, 0);
            return (0, false);
        }
        let now = now_ms();
        let (a, a2, more) = Self::take_actions(e);
        let kind = s(e, "kind");
        // One thing, one card: an alert with the `key` of one that's up takes its place
        if let Some(key) = e["key"].as_str().filter(|k| !k.is_empty()).map(String::from) {
            if let Some(i) = self.alerts.iter().position(|x| x.event["key"].as_str() == Some(&key)) {
                let hid = self.record(h, e, self.alerts[i].hid);
                self.keep_act(h, hid, e, &kind, &a);
                let al = &mut self.alerts[i];
                al.event = e.clone();
                (al.on_action, al.on_action2, al.more) = (a, a2, more);
                (al.at, al.expires, al.hid) = (now, now + ms, hid);
                return (al.uid, false);
            }
        }
        // HUD and music update their row in place
        if kind == "hud" || kind == "music" {
            if let Some(al) = self.alerts.iter_mut().find(|x| x.event["kind"] == kind.as_str()) {
                al.event = e.clone();
                (al.at, al.expires) = (now, now + ms);
                return (al.uid, false);
            }
        }
        // The same thing again while its alert is up: that alert changes in place and stays a little longer
        if e["title"].as_str().is_some_and(|t| !t.is_empty()) {
            if let Some(i) = self.alerts.iter().position(|x| x.event["kind"] == kind.as_str() && x.event["title"] == e["title"] && x.expires > now) {
                let hid = self.record(h, e, 0);
                // (record() let its history button go: the same alert again keeps it)
                self.keep_act(h, hid, e, &kind, &a);
                let al = &mut self.alerts[i];
                al.event = e.clone();
                (al.on_action, al.on_action2, al.more) = (a, a2, more);
                (al.at, al.expires, al.hid) = (now, al.expires.max(now + ms), hid);
                return (al.uid, false);
            }
        }
        self.burst_index = if now - self.burst_at < 400 { self.burst_index + 1 } else { 0 };
        if self.burst_index == 0 {
            self.burst_at = now;
        }
        self.next_uid += 1;
        let uid = self.next_uid;
        let hid = self.record(h, e, 0);
        self.keep_act(h, hid, e, &kind, &a);
        let sticky = e["sticky"].as_bool().unwrap_or(false) || kind == "ringing";
        self.alerts.push(Alert { uid, event: e.clone(), on_action: a, on_action2: a2, more, at: now, expires: now + ms + self.burst_index * 600, hid, sticky });
        // (the oldest go first; a ringing call (sticky) stays)
        while self.alerts.len() > MAX_ALERTS {
            let i = (0..self.alerts.len() - 1).find(|&i| !self.alerts[i].sticky).unwrap_or(0);
            self.alerts.remove(i);
        }
        self.newest = uid;
        self.start_sweep(h);
        (uid, false)
    }

    /// Island.update: change a live alert in place; its line in the history follows
    fn update(&mut self, h: &mut Host, uid: i64, patch: Value) {
        let Some(al) = self.alerts.iter_mut().find(|a| a.uid == uid) else { return };
        if let Value::Object(p) = &patch {
            for (k, v) in p {
                al.event[k] = v.clone();
            }
        }
        let hid = al.hid;
        self.forward(h, "update", uid, &patch, 3000);
        if hid != 0 {
            for line in self.history.iter_mut().filter(|l| l["hid"] == hid) {
                for k in ["title", "subtitle", "icon", "battery", "value"] {
                    if !patch[k].is_null() {
                        line[k] = patch[k].clone();
                    }
                }
            }
            self.publish_history(h);
        }
    }

    /// Island.remove (`since`: unless it was posted or changed from then on)
    fn remove(&mut self, h: &mut Host, uid: i64, since: i64) {
        if since != 0 && self.alerts.iter().find(|a| a.uid == uid).is_some_and(|a| a.at >= since) {
            return;
        }
        self.forward(h, "remove", uid, &json!({}), 3000);
        if let Some(i) = self.alerts.iter().position(|a| a.uid == uid) {
            self.alerts.remove(i);
            if let Some(last) = self.alerts.last() {
                self.newest = last.uid;
            }
        }
    }

    /// Island.dismiss: close everything except a ringing call and, given
    /// `since`, alerts posted or changed from then on
    fn dismiss(&mut self, h: &mut Host, since: i64) {
        self.forward(h, "dismiss", 0, &json!({"since": since}), 3000);
        self.alerts.retain(|a| a.sticky || (since != 0 && a.at >= since));
    }

    // ── History ─────────────────────────────────────────────────────────

    /// Its history line's button (Open, What's new) for an alert that has
    /// one, kept after record() (which lets a line's old button go)
    fn keep_act(&mut self, h: &mut Host, hid: i64, e: &Value, kind: &str, act: &Option<Value>) {
        if let Some(b) = history_act(e, kind, act) {
            self.history_acts.insert(hid, b);
            self.publish_history(h);
        }
    }

    /// Island.record: a line for the history (the hid, 0 when it is not kept)
    fn record(&mut self, h: &mut Host, e: &Value, same: i64) -> i64 {
        let kind = s(e, "kind");
        if UNRECORDED.contains(&kind.as_str()) || e["title"].as_str().map_or(true, str::is_empty) {
            return 0;
        }
        let now = now_ms();
        let kept = (same != 0).then(|| self.history.iter().find(|l| l["hid"] == same).cloned()).flatten();
        let key = s(e, "key");
        let old = kept.clone().or_else(|| {
            self.history
                .iter()
                .find(|l| {
                    // (a live entry, by its key: whenever; else the same title within a minute)
                    if !key.is_empty() {
                        return l["key"] == key.as_str();
                    }
                    l["kind"] == kind.as_str() && l["title"] == e["title"] && now - l["at"].as_i64().unwrap_or(0) < 60_000 && (kind != "missedCall" || l["subtitle"].as_str().unwrap_or("") == e["subtitle"].as_str().unwrap_or(""))
                })
                .cloned()
        });
        let hid = match &old {
            Some(o) => o["hid"].as_i64().unwrap_or(0),
            None => {
                self.next_hid += 1;
                self.next_hid
            }
        };
        let times = old.as_ref().map_or(1, |o| o["times"].as_i64().unwrap_or(1) + if kept.is_some() { 0 } else { 1 });
        // its history (live.rs): every state it had, with its time
        let st = Self::event_state(e, now);
        let mut changes: Vec<live::Change> = old.as_ref().and_then(|o| serde_json::from_value(o["changes"].clone()).ok()).unwrap_or_default();
        live::push_change(&mut changes, live::Change { at: now, text: st.text.clone() });
        let mut line = json!({"hid": hid, "kind": kind, "icon": e["icon"].as_str().unwrap_or("bell"), "title": e["title"], "subtitle": e["subtitle"].as_str().unwrap_or(""), "color": e["color"].as_str().unwrap_or("#FFFFFF"), "at": now, "times": times,
            "severity": st.severity, "failure": st.failure, "changes": changes});
        for k in ["battery", "key", "picture"] {
            if !e[k].is_null() {
                line[k] = e[k].clone();
            }
        }
        self.history.retain(|l| l["hid"] != hid);
        self.history.insert(0, line);
        self.history.truncate(HISTORY_MAX);
        // (its button: Open a download or a drive, What's new, while it means something)
        let live: Vec<i64> = self.history.iter().filter_map(|l| l["hid"].as_i64()).collect();
        self.history_acts.retain(|k, _| live.contains(k) && *k != hid);
        self.publish_history(h);
        hid
    }

    fn publish_history(&self, h: &mut Host) {
        h.set_state("island.history", json!(self.history.len()));
        // (each line says whether its button is still remembered: a click on it runs it)
        let list: Vec<Value> = self
            .history
            .iter()
            .map(|l| {
                let mut l = l.clone();
                let label = l["hid"].as_i64().and_then(|hid| self.history_acts.get(&hid)).map(|a| a.0.clone());
                l["act"] = json!(label.is_some());
                l["actLabel"] = json!(label.unwrap_or_default());
                l
            })
            .collect();
        h.set_state("island.list", Value::Array(list));
    }

    // ── Hover hold ──────────────────────────────────────────────────────

    /// Island.hold: while the pointer rests on the alerts nothing times out;
    /// when it leaves, each alert gets back the time it had left, plus a second
    fn hold(&mut self, key: &str, on: bool) {
        if self.holders.contains(key) == on {
            return;
        }
        let was = self.held();
        if on {
            self.holders.insert(key.to_string());
        } else {
            self.holders.remove(key);
        }
        let now = now_ms();
        if !was && self.held() {
            self.held_at = now;
        } else if was && !self.held() {
            for a in self.alerts.iter_mut() {
                a.expires += now - self.held_at.max(a.at) + 1000;
            }
        }
    }

    fn start_sweep(&mut self, h: &mut Host) {
        if self.sweep.as_ref().is_some_and(slint::Timer::running) {
            return;
        }
        let send = h.sender("Island");
        let t = slint::Timer::default();
        t.start(slint::TimerMode::Repeated, std::time::Duration::from_millis(250), move || send(Box::new(Tick::Sweep)));
        self.sweep = Some(t);
    }

    /// Alerts whose time is up leave (the island plays their way out; they go here a moment later)
    fn sweep(&mut self, h: &mut Host) {
        if self.alerts.is_empty() {
            self.sweep = None;
            return;
        }
        if self.held() {
            return;
        }
        let now = now_ms();
        let _ = h;
        self.alerts.retain(|a| a.expires + 700 > now);
        if self.alerts.is_empty() {
            self.sweep = None;
        }
    }

    // ── Live activities ─────────────────────────────────────────────────

    fn customised(&self, h: &Host, kind: &str) -> bool {
        let c = h.cfg();
        live_defaults(kind).is_none()
            || at(c, "island.known").as_array().is_some_and(|l| l.iter().any(|x| x.as_str() == Some(&format!("live.{kind}"))))
            || at(c, "island.livePieces").as_array().is_some_and(|l| l.iter().any(|x| x.as_str().is_some_and(|p| p.starts_with(&format!("{kind}.")))))
    }

    /// Island.shows: is this piece of a collapsed live activity switched on?
    fn shows(&self, h: &Host, kind: &str, piece: &str) -> bool {
        if self.customised(h, kind) {
            at(h.cfg(), "island.livePieces").as_array().is_some_and(|l| l.iter().any(|x| x.as_str() == Some(&format!("{kind}.{piece}"))))
        } else {
            live_defaults(kind).is_some_and(|d| d.contains(&piece))
        }
    }

    /// Island.activities' own kinds, from what the host's services say:
    /// recording (Recorder), the timer and the stopwatch (Timers), with
    /// LiveInfo's icon, title, time, detail, progress and controls
    fn derive_live(&mut self, h: &mut Host) {
        let dark = h.st.shell.look.dark;
        let accent = |n: &str| tessera_core::theme::accent_hex(n, dark);
        let fmt = |secs: i64| {
            let s = secs.max(0);
            format!("{}:{:02}", s / 60, s % 60)
        };
        let ctl = |piece: &str, icon: &str, fill: bool, on: bool, danger: bool, act: Value| json!({"piece": piece, "icon": icon, "fill": fill, "on": on, "danger": danger, "act": act});
        let nc = |a: &str| json!({"call": "nc", "args": [{"act": a}]});
        let ipc = |t: &str, f: &str| json!({"ipc": [t, f]});
        let mut want: Vec<(&str, Option<Value>)> = Vec::new();
        // recording
        let r = h.state("recorder").clone();
        if r["recording"].as_bool().unwrap_or(false) {
            let paused = r["paused"].as_bool().unwrap_or(false);
            let now = now_ms();
            let started = r["started"].as_i64().unwrap_or(now);
            let upto = if paused { r["pausedAt"].as_i64().unwrap_or(now) } else { now };
            let secs = (upto - started) / 1000;
            let mut controls = vec![ctl("discard", "trash", false, true, false, ipc("recorder", "discard"))];
            if !cfg_str(h.cfg(), "recorder.mic", "").is_empty() {
                let muted = r["micMuted"].as_bool().unwrap_or(false);
                controls.push(ctl("mute", if muted { "microphone-slash" } else { "microphone" }, true, true, false, ipc("recorder", "toggleMic")));
            }
            if r["canPause"].as_bool().unwrap_or(false) {
                controls.push(ctl("pause", if paused { "play" } else { "pause" }, true, true, false, ipc("recorder", "pause")));
            }
            controls.push(ctl("stop", "stop", true, true, true, json!({"call": "screen.stopRecording", "args": []})));
            want.push(("recording", Some(json!({
                "icon": if paused { "pause-circle" } else { "record" },
                "color": if paused { "#8cffffff".to_string() } else { accent("red") },
                "title": if paused { "Recording paused" } else { "Recording" },
                "time": fmt(secs), "detail": fmt(secs), "progress": -1.0, "controls": controls,
            }))));
        } else {
            want.push(("recording", None));
        }
        // the timer and the stopwatch
        let t = h.state("timers").clone();
        let remaining = t["remaining"].as_i64().unwrap_or(0);
        let length = t["phaseLength"].as_i64().unwrap_or(0).max(1);
        let running = t["running"].as_bool().unwrap_or(false);
        if t["started"].as_bool().unwrap_or(false) {
            let focus = t["phase"].as_str().unwrap_or("focus") == "focus";
            want.push(("timer", Some(json!({
                "icon": if focus { "timer" } else { "coffee" }, "color": accent("orange"),
                "title": if focus { "Focus" } else { "Break" },
                "time": fmt(remaining), "detail": format!("{} left", fmt(remaining)),
                "progress": (1.0 - remaining as f64 / length as f64).clamp(0.0, 1.0),
                "controls": [
                    ctl("reset", "arrow-counter-clockwise", false, true, false, nc("timer.reset")),
                    ctl("play", if running { "pause" } else { "play" }, true, true, false, nc("timer.toggle")),
                    ctl("skip", "skip-forward", true, true, false, nc("timer.skip")),
                ],
            }))));
        } else {
            want.push(("timer", None));
        }
        let sw = &t["sw"];
        if sw["started"].as_bool().unwrap_or(false) {
            let on = sw["running"].as_bool().unwrap_or(false);
            let secs = sw["elapsed"].as_i64().unwrap_or(0) / 1000;
            want.push(("stopwatch", Some(json!({
                "icon": "timer", "color": accent("green"), "title": "Stopwatch",
                "time": fmt(secs), "detail": fmt(secs), "progress": -1.0,
                // (the open card counts hundredths on its own clock from here: IslandFocusCard's fineClock)
                "fine": {"ms": sw["elapsed"].as_i64().unwrap_or(0), "running": on},
                "controls": [
                    ctl("reset", "arrow-counter-clockwise", false, !on, false, nc("sw.reset")),
                    ctl("lap", "flag", false, on, false, nc("sw.lap")),
                    ctl("play", if on { "pause" } else { "play" }, true, true, false, nc("sw.toggle")),
                ],
            }))));
        } else {
            want.push(("stopwatch", None));
        }
        // the kinds other services own (island_live.rs: LiveInfo's mirroring … transfer)
        for (kind, a) in crate::island_live::derive(h, now_ms()) {
            let a = if kind == "sysupdate" && !self.wants(h, "sysupdate") { None } else { a };
            want.push((kind, a));
        }
        for (kind, a) in want {
            match a {
                Some(a) => {
                    self.live.insert(kind.to_string(), a);
                }
                None => {
                    self.live.remove(kind);
                }
            }
        }
    }

    /// RustBridge.pushLive: the activities as the pill shows them
    fn push_live(&mut self, h: &mut Host) {
        let mut acts = Vec::new();
        for kind in LIVE_ORDER {
            let Some(a) = self.live.get(*kind) else { continue };
            let music = *kind == "music";
            let controls: Vec<Value> = if music {
                vec![]
            } else {
                a["controls"].as_array().map(|l| l.iter().filter(|b| self.shows(h, kind, b["piece"].as_str().unwrap_or(""))).map(|b| json!({"icon": b["icon"], "fill": b["fill"].as_bool().unwrap_or(false), "on": b["on"].as_bool().unwrap_or(true), "danger": b["danger"].as_bool().unwrap_or(false), "name": ctl_name(b)})).collect()).unwrap_or_default()
            };
            // (every control, for the stacked rows and the open card: LiveInfo.controls)
            let all: Vec<Value> = if music {
                vec![]
            } else {
                a["controls"].as_array().map(|l| l.iter().map(|b| json!({"piece": b["piece"], "icon": b["icon"], "fill": b["fill"].as_bool().unwrap_or(false), "on": b["on"].as_bool().unwrap_or(true), "danger": b["danger"].as_bool().unwrap_or(false), "name": ctl_name(b)})).collect()).unwrap_or_default()
            };
            acts.push(json!({
                "kind": kind, "icon": a["icon"], "color": a["color"].as_str().unwrap_or(""),
                "title": if music { json!("") } else { a["title"].clone() },
                "time": if music { json!("") } else { a["time"].clone() },
                "detail": a["detail"].as_str().unwrap_or(""),
                "progress": a["progress"].as_f64().unwrap_or(-1.0),
                // (the open card: the caption over the big number, a call's app, the captions' lines, the stopwatch's hundredths)
                "caption": if music { String::new() } else { crate::island_live::caption(kind, a, h) },
                "appIcon": a["appIcon"].as_str().unwrap_or(""),
                "lines": a["lines"].clone(),
                "fine": a["fine"].clone(),
                "all": all,
                "lead": self.shows(h, kind, if music { "art" } else { "icon" }),
                "name": self.shows(h, kind, if music { "title" } else if *kind == "timer" { "label" } else { "name" }),
                "clock": !music && self.shows(h, kind, "time"),
                "controls": controls
            }));
        }
        let line = serde_json::to_string(&acts).unwrap_or_default();
        if line == self.live_sent {
            return;
        }
        self.live_sent = line;
        h.say(json!({"src": "shell", "kind": "live", "activities": acts}));
        // (each second while there are any, for their clocks)
        if acts.is_empty() {
            self.live_timer = None;
        } else if self.live_timer.is_none() {
            let send = h.sender("Island");
            let t = slint::Timer::default();
            t.start(slint::TimerMode::Repeated, std::time::Duration::from_secs(1), move || send(Box::new(Tick::Live)));
            self.live_timer = Some(t);
        }
    }

    // ── The watches: charger, battery, heat ─────────────────────────────

    /// Focus turned on or off (Island.qml's Connections on Focus): its mode's
    /// icon and name with its status, or "Focus off"
    fn watch_focus(&mut self, h: &mut Host) {
        let on = h.state("focus.on").as_bool();
        let Some(on) = on else { return };
        let was = self.focus_was.replace(on);
        if was.is_none() || was == Some(on) {
            return;
        }
        let e = if on {
            json!({"kind": "focus", "gate": "focus", "icon": h.state("focus.icon").as_str().unwrap_or("moon"), "title": h.state("focus.name").as_str().unwrap_or("Focus"), "subtitle": h.state("focus.status").as_str().unwrap_or(""), "color": tessera_core::theme::accent_hex("purple", h.st.shell.look.dark)})
        } else {
            json!({"kind": "focus", "gate": "focus", "icon": h.state("focus.icon").as_str().unwrap_or("moon"), "title": "Focus off", "subtitle": "", "color": tessera_core::theme::accent_hex("purple", h.st.shell.look.dark)})
        };
        self.post(h, e, 3000);
    }

    /// A new song while it plays (Island.qml's onTrackChanged): 3.5 s, with its art
    fn watch_track(&mut self, h: &mut Host) {
        let Some(p) = h.st.shell.get::<tessera_core::services::mpris::Players>().and_then(|p| p.chosen().cloned()) else { return };
        if p.track.title.is_empty() {
            return;
        }
        let track = format!("{}\n{}", p.track.title, p.track.artist);
        if track == self.track_was {
            return;
        }
        self.track_was = track;
        if p.playing && self.wants(h, "music") {
            self.post(h, json!({"kind": "music", "icon": "music-notes", "title": p.track.title, "subtitle": p.track.artist, "art": p.track.art_url, "color": ""}), 3500);
        }
    }

    fn level_pct(l: f64) -> i64 {
        (l * 100.0).round() as i64
    }

    /// Charger and battery (Island's onChargingChanged and onLevelChanged), on each new reading (Service::data)
    fn watch_battery(&mut self, h: &mut Host) {
        let Some(b) = h.st.shell.get::<tessera_core::services::status::Battery>().cloned() else { return };
        let level = if b.percent > 1.0 { b.percent / 100.0 } else { b.percent };
        let charging = b.present && (b.state == 1 || b.state == 4 || b.charging);
        let now = (b.present, level, charging);
        let Some(prev) = self.battery.replace(now) else { return };
        if !b.present {
            return;
        }
        if charging != prev.2 {
            // (plugged in: the low battery entry is resolved, card and all:
            // Power's or the demo's)
            if charging {
                let low: Vec<i64> = self.alerts.iter().filter(|a| a.uid == self.low_uid).map(|a| a.uid).collect();
                for u in low {
                    self.remove(h, u, 0);
                }
                self.low_uid = 0;
                self.post(h, json!({"kind": "battery", "key": "battery", "resolved": "remove"}), 0);
            }
            if self.wants(h, "charging") {
                let pct = Self::level_pct(level);
                self.post(h, json!({"kind": "charging", "icon": if charging { "lightning" } else { "plug" }, "title": if charging { "Charging" } else { "Charger removed" }, "subtitle": format!("{pct}%{}", if charging && level < 1.0 { " · on power" } else { "" }), "value": level, "color": if charging { "#30D158" } else { "rgba(255,255,255,0.6)" }}), 3000);
            }
        }
        // The low battery alerts themselves are Power's (one per mark:
        // power_policy::BatteryAlerts): the island posting its own 20/10/5%
        // ones as well showed two cards at 10%. Its card follows the level.
        if !charging && (level - prev.1).abs() >= f64::EPSILON {
            let pct = Self::level_pct(level);
            if let Some(u) = self.alerts.iter().find(|a| a.event["key"] == "battery" && a.event["kind"] == "battery").map(|a| a.uid) {
                let sub = self.alerts.iter().find(|a| a.uid == u).and_then(|a| a.event["subtitle"].as_str().map(String::from)).unwrap_or_default();
                let rest = sub.split_once(" · ").map(|(_, r)| r.to_string());
                let mut patch = json!({"value": level});
                if let Some(r) = rest.filter(|_| sub.contains("% left")) {
                    patch["subtitle"] = json!(format!("{pct}% left · {r}"));
                }
                self.update(h, u, patch);
            }
        }
    }

    /// The average over the last `ms`, or None when the readings don't cover that long yet
    fn average_over(&self, ms: i64, now: i64) -> Option<f64> {
        let from = now - ms;
        let inside: Vec<f64> = self.temps.iter().filter(|(_, at)| *at >= from).map(|(t, _)| *t).collect();
        if inside.is_empty() || self.temps[0].1 > from + 5000 {
            return None;
        }
        Some(inside.iter().sum::<f64>() / inside.len() as f64)
    }

    fn hot_text(&self, h: &Host, t: f64) -> String {
        let throttled = h.state("power.cooldown").as_bool().unwrap_or(false);
        let ppd_low = h.state("power.profile") == "power-saver";
        let ppd = h.state("power.ppd").as_bool().unwrap_or(true);
        format!(
            "CPU {} °C · {}",
            t.round() as i64,
            if throttled {
                if sandboxed() { "would switch to Low Power (sandbox)" } else { "switched to Low Power to cool down" }
            } else if ppd && ppd_low {
                "already on Low Power; close heavy apps"
            } else {
                "close heavy apps or let it cool"
            }
        )
    }

    /// Resources.temperature changed (Island's onTemperatureChanged): hot once
    /// the CPU has averaged at or above the threshold for 30 s, cool again once
    /// it has averaged 5 °C below it for a minute
    fn temperature(&mut self, h: &mut Host, t: f64) {
        if !self.wants(h, "temperature") {
            self.set_hot(h, false);
            self.temps.clear();
            return;
        }
        if t <= 0.0 {
            return; // a failed reading: ignore it rather than "cooling down"
        }
        let now = now_ms();
        let hot_temp = cfg_num(h.cfg(), "island.hotTemp", 90.0);
        self.temps.retain(|(_, at)| *at >= now - 60_000 - 5000);
        self.temps.push((t, now));
        let warm = self.average_over(30_000, now);
        let cool = self.average_over(60_000, now);
        let asking = cfg_bool(h.cfg(), "cooling.whenHot", true) && cfg_str(h.cfg(), "cooling.hotAction", "auto") == "ask";
        if !self.hot && warm.is_some_and(|w| w >= hot_temp) {
            self.set_hot(h, true);
            if asking {
                self.post(h, json!({"kind": "temperature", "icon": "thermometer-hot", "title": "Running hot", "subtitle": format!("CPU {} °C", t.round() as i64), "value": (t / 105.0).min(1.0), "color": "#FF453A",
                    "action": "Cool down", "onAction": {"call": "cooldown.answer", "args": [true]}, "action2": "Not now", "onAction2": {"call": "cooldown.answer", "args": [false]}}), UNTIL_OVER);
            } else {
                let text = self.hot_text(h, t);
                self.post(h, json!({"kind": "temperature", "icon": "thermometer-hot", "title": "Running hot", "subtitle": text, "value": (t / 105.0).min(1.0), "color": "#FF453A"}), UNTIL_OVER);
            }
            self.hot_uid = self.newest;
        } else if self.hot && cool.is_some_and(|c| c <= hot_temp - 5.0) {
            self.set_hot(h, false);
        } else if self.hot && self.hot_uid != 0 {
            let sub = if asking { format!("CPU {} °C", t.round() as i64) } else { self.hot_text(h, t) };
            let u = self.hot_uid;
            self.update(h, u, json!({"subtitle": sub, "value": (t / 105.0).min(1.0)}));
        }
    }

    /// Island.hot changing: Cool-down hears it, Low Power comes (or goes), the alert ends with the spell
    fn set_hot(&mut self, h: &mut Host, hot: bool) {
        if self.hot == hot {
            return;
        }
        self.hot = hot;
        h.ask("cooldown.hot", json!([hot]));
        let c = h.cfg();
        let throttle_on = hot && cfg_bool(c, "island.hotThrottle", true) && h.state("power.ppd").as_bool().unwrap_or(true);
        let asking = cfg_bool(c, "cooling.whenHot", true) && cfg_str(c, "cooling.hotAction", "auto") == "ask";
        if throttle_on && !asking {
            h.ask("power.ask", json!(["cooldown", "power-saver", format!("cooling down: CPU at {} °C", self.temps.last().map_or(0, |t| t.0.round() as i64))]));
        } else if !hot {
            h.ask("power.release", json!(["cooldown"]));
        }
        if !hot {
            if self.hot_uid != 0 {
                let u = self.hot_uid;
                self.remove(h, u, 0);
            }
            self.hot_uid = 0;
        }
        self.publish(h);
    }

    fn publish(&self, h: &mut Host) {
        // (field by field: the whole object would wipe island.list, the Recent lines)
        h.set_state("island.pinned", json!(cfg_bool(h.cfg(), "island.pinned", false)));
        h.set_state("island.hot", json!(self.hot || self.hot_preview));
        h.set_state("island.fsMode", json!(self.fs_mode(h)));
        h.set_state("island.history", json!(self.history.len()));
    }

    /// Island.hud: volume, brightness and mic, when the HUD lives in the island
    fn hud(&mut self, h: &mut Host, kind: &str, device: &str) {
        // (the levels as Osd.qml read them, Audio's and Brightness's: the
        // sound server's and the backlight's newest readings; the shell's
        // state "hud.*" only when those aren't there)
        use tessera_core::services::{audio::Sound, brightness::Backlight};
        let v = |p: &str| h.state(p).as_f64().unwrap_or(0.0);
        let snd = h.st.shell.get::<Sound>().filter(|s| s.up).cloned();
        let (value, icon, title) = match kind {
            "brightness" => {
                let level = h.st.shell.get::<Backlight>().filter(|b| b.present).map_or_else(|| v("hud.brightness"), |b| b.value);
                (level, "sun".to_string(), if device.is_empty() { "Display".to_string() } else { device.to_string() })
            }
            "mic" => {
                let muted = snd.as_ref().map_or_else(|| h.state("audio.micMuted").as_bool().unwrap_or(false), |s| s.mic_muted());
                let level = snd.as_ref().map_or_else(|| v("hud.mic"), |s| s.mic_volume());
                (if muted { 0.0 } else { level }, if muted { "microphone-slash".into() } else { "microphone".into() }, if !device.is_empty() { device.to_string() } else if muted { "Microphone off".into() } else { "Microphone on".into() })
            }
            _ => {
                let muted = snd.as_ref().map_or_else(|| h.state("audio.muted").as_bool().unwrap_or(false), |s| s.muted());
                let level = snd.as_ref().map_or_else(|| v("hud.volume"), |s| s.volume());
                let icon = match &snd {
                    // (Audio.icon: muted or silent, then by thirds)
                    Some(s) => (if s.muted() || s.volume() == 0.0 { "speaker-x" } else if s.volume() < 0.34 { "speaker-none" } else if s.volume() < 0.67 { "speaker-low" } else { "speaker-high" }).to_string(),
                    None => h.state("audio.icon").as_str().unwrap_or("speaker-high").to_string(),
                };
                (if muted { 0.0 } else { level.min(1.0) }, icon, if device.is_empty() { "Sound".into() } else { device.to_string() })
            }
        };
        self.post(h, json!({"kind": "hud", "hud": kind, "icon": icon, "title": title, "value": value, "device": device, "color": "#FFFFFF"}), if device.is_empty() { 1600 } else { 2500 });
    }

    /// Run an action stored with an alert (what the QML closure did)
    fn run(&self, h: &mut Host, act: &Option<Value>) {
        if let Some(a) = act {
            if let Some(call) = a["call"].as_str() {
                h.ask(call, a["args"].clone());
                // (a second step: a live control's "Details" shows them, then opens Settings)
                if a["then"].is_object() {
                    self.run(h, &Some(a["then"].clone()));
                }
            } else if let Some(ipc) = a["ipc"].as_array() {
                let args: Vec<String> = ipc.iter().map(|x| x.as_str().unwrap_or("").to_string()).collect();
                tessera_core::host::route_ipc(&args, None);
            }
        }
    }

    fn demo(&mut self, h: &mut Host, kind: &str) {
        let d = match kind {
            "device" => json!({"kind": "device", "icon": "headphones", "title": "AirPods Pro", "subtitle": "Connected", "color": "#0A84FF", "battery": 0.85}),
            "usb" => json!({"kind": "usb", "icon": "hard-drive", "title": "SanDisk Ultra", "subtitle": "Drive connected", "color": "#FF9F0A", "action": "Open"}),
            "display" => json!({"kind": "display", "icon": "monitor", "title": "Dell U2723QE", "subtitle": "Display connected · DP-1", "color": "#30D158"}),
            "audio" => json!({"kind": "audio", "icon": "headphones", "title": "Headphones", "subtitle": "Built-in Audio", "color": "#BF5AF2"}),
            "charging" => json!({"kind": "charging", "icon": "lightning", "title": "Charging", "subtitle": "64% · on power", "value": 0.64, "color": "#30D158"}),
            "battery" => json!({"kind": "battery", "icon": "battery-warning", "title": "Low battery", "subtitle": "10% left · plug in soon", "value": 0.1, "color": "#FF453A"}),
            "wifi" => json!({"kind": "network", "icon": "wifi-high", "title": "Home Wi-Fi", "subtitle": "Wi-Fi connected", "color": "#0A84FF"}),
            "vpn" => json!({"kind": "network", "icon": "shield-check", "title": "Work VPN", "subtitle": "VPN connected", "color": "#30D158", "ripple": true}),
            "temperature" => json!({"kind": "temperature", "icon": "thermometer-hot", "title": "Running hot", "subtitle": "CPU 94 °C · close heavy apps or let it cool", "value": 0.9, "color": "#FF453A"}),
            "notification" => json!({"kind": "notification", "headline": "Your order is on its way.", "card": {"name": "Parcel", "time": "now", "text": "Your earbuds ship today and arrive on Thursday.", "initials": "P", "appIcon": "org.gnome.Software"}}),
            "music" => json!({"kind": "music", "icon": "music-notes", "title": "Midnight City", "subtitle": "M83", "color": "#0A84FF", "art": ""}),
            "download" => json!({"kind": "transfer", "icon": "download-simple", "title": "Downloaded report.pdf", "subtitle": "In ~/Downloads", "color": "#30D158", "action": "Open", "action2": "Show in Folder"}),
            "connect" => json!({"kind": "custom", "key": "bt:demo", "icon": "headphones", "title": "Maya's earbuds", "subtitle": "Paired with this computer, not connected", "color": "#0A84FF", "action": "Connect",
                "onAction": {"call": "island.post", "args": [{"kind": "custom", "key": "bt:demo", "icon": "headphones", "title": "Maya's earbuds", "subtitle": "Connecting… keep the case open nearby", "color": "#0A84FF"}, 20000]}}),
            "linked" => json!({"kind": "device", "key": "bt:demo", "icon": "headphones", "title": "Maya's earbuds", "subtitle": "Connected", "color": "#0A84FF"}),
            "parts" => json!({"kind": "device", "key": "bt:demo", "icon": "headphones", "title": "Maya's earbuds", "subtitle": "Connected", "color": "#0A84FF", "batteries": [{"label": "L", "value": 0.8}, {"label": "R", "value": 0.76}, {"label": "Case", "value": 0.18}]}),
            "lowbuds" => json!({"kind": "device", "key": "bt:demo", "icon": "headphones", "title": "Maya's earbuds", "subtitle": "Low battery · 18% left", "color": "#FF453A", "battery": 0.18}),
            "connected" => json!({"kind": "device", "key": "bt:demo", "icon": "headphones", "title": "Maya's earbuds", "subtitle": "Connected", "color": "#0A84FF", "battery": 0.8}),
            "burst" => {
                for k in ["device", "charging", "audio"] {
                    self.demo(h, k);
                }
                return;
            }
            "output" => {
                self.hud(h, "volume", "Maya's earbuds");
                return;
            }
            // every device drawing, one after another (Devices.pictureFor's kinds)
            "pictures" => {
                let was = self.started_at;
                self.started_at = 0;
                for (pic, name) in [("headphones", "WH-CH720N"), ("speaker", "Speaker"), ("soundbar", "Samsung Soundbar"), ("controller", "Wireless Controller"), ("keyboard", "Keyboard"), ("mouse", "MX Master"), ("phone", "Pixel"), ("watch", "Galaxy Watch"), ("laptop", "Laptop"), ("bluetooth", "Tag")] {
                    self.post(h, json!({"kind": "device", "icon": "bluetooth", "picture": device_picture(pic), "title": name, "subtitle": "Connected", "color": "#0A84FF"}), 8000);
                }
                self.started_at = was;
                return;
            }
            _ => json!({"kind": "custom", "icon": "bell", "title": "Hello", "subtitle": "A custom alert", "color": "#0A84FF"}),
        };
        // (the earbuds' drawing, as Devices.pictureFor gives it)
        let mut d = d;
        if ["device", "connect", "linked", "parts", "lowbuds", "connected"].contains(&kind) {
            d["picture"] = json!(device_picture("earbuds"));
        }
        let ms = if kind == "battery" || kind == "temperature" { UNTIL_OVER } else if ["usb", "download", "connect"].contains(&kind) { LONG_MS } else { 5000 };
        // (the demo posts whatever the arming time: it shows at once)
        let was = self.started_at;
        self.started_at = 0;
        let mut d = d;
        d["demo"] = json!(true);
        self.post(h, d, ms);
        self.started_at = was;
        if kind == "temperature" {
            self.hot_uid = self.newest;
        }
        if kind == "battery" {
            self.low_uid = self.newest;
        }
    }
}

impl Service for Island {
    fn name(&self) -> &'static str {
        "Island"
    }
    fn calls(&self) -> &'static [&'static str] {
        &["island"]
    }
    fn ipc_targets(&self) -> &'static [&'static str] {
        &["island"]
    }
    fn start(&mut self, h: &mut Host) {
        self.started_at = now_ms();
        self.next_hid = 1;
        h.st.shell.want("status");
        h.st.shell.want("hypr");
        h.st.shell.want("mpris");
        self.publish(h);
        self.ask_resources(h);
        // (the battery now comes as data: Service::data below, no 3 s look)
    }
    fn settled(&mut self, h: &mut Host) {
        self.derive_live(h);
        self.push_live(h);
        self.watch_focus(h);
    }
    fn data(&mut self, h: &mut Host, what: std::any::TypeId) {
        if what == std::any::TypeId::of::<tessera_core::services::status::Battery>() {
            self.watch_battery(h);
        }
        if what == std::any::TypeId::of::<tessera_core::services::mpris::Players>() {
            self.watch_track(h);
        }
    }
    fn config_changed(&mut self, h: &mut Host) {
        self.publish(h);
        self.live_sent.clear();
        self.push_live(h);
        self.ask_resources(h);
    }
    fn message(&mut self, h: &mut Host, msg: Box<dyn Any + Send>) {
        match msg.downcast::<Tick>() {
            Ok(t) => match *t {
                Tick::Sweep => self.sweep(h),
                Tick::Live => {
                    self.live_sent.clear();
                    self.derive_live(h);
                    self.push_live(h);
                }
            },
            Err(_) => {}
        }
    }
    fn call(&mut self, h: &mut Host, call: &str, args: &Value) {
        // (host-sharing posts {event, ms}: the same as [event, ms])
        let a = if args.is_array() {
            args.clone()
        } else if args.get("event").is_some() {
            json!([args["event"], args["ms"]])
        } else {
            json!([])
        };
        let num = |i: usize| a.get(i).and_then(|x| x.as_i64().or_else(|| x.as_str().and_then(|s| s.parse().ok()))).unwrap_or(0);
        let text = |i: usize| a.get(i).and_then(Value::as_str).unwrap_or("").to_string();
        match call.strip_prefix("island.").unwrap_or("") {
            "post" => {
                let ms = a.get(1).and_then(Value::as_i64).unwrap_or(3000);
                self.post(h, a.get(0).cloned().unwrap_or(Value::Null), ms);
            }
            "update" => self.update(h, num(0), a.get(1).cloned().unwrap_or(Value::Null)),
            "remove" | "islandremove" => {
                // (by uid, or by the `key` of an alert: host-sharing's {key})
                let uid = match args["key"].as_str() {
                    Some(k) => self.alerts.iter().find(|x| x.event["key"].as_str() == Some(k)).map_or(0, |x| x.uid),
                    None => num(0),
                };
                self.remove(h, uid, 0)
            }
            "dismiss" => self.dismiss(h, num(0)),
            "find" => {
                let key = text(0);
                let u = self.alerts.iter().find(|x| x.event["key"].as_str() == Some(&key)).map_or(0, |x| x.uid);
                h.set_state("island.found", json!(u));
            }
            // the menu bar's buttons: act <uid> <1 | 2 | more> [index]
            "act" => {
                let uid = text(0).parse::<i64>().unwrap_or(0);
                let since = now_ms();
                let which = text(1);
                // an alert host-desktop made (Today, Calls, notifications, calendar: uids from
                // 1_000_000_000): its own table has the button's action, Answer/Decline included
                if uid >= DESKTOP_UID_BASE {
                    // ("more <i>" and the card's own "btn <i>": the index is the argument after)
                    let w = if which == "more" || which == "btn" { format!("{which} {}", text(2)) } else { which };
                    h.ask("today.islandAct", json!([format!("{uid} {w}")]));
                    self.dismiss(h, since);
                    return;
                }
                if let Some(al) = self.alerts.iter().find(|x| x.uid == uid).cloned() {
                    let f = match which.as_str() {
                        "1" => al.on_action.clone(),
                        "2" => al.on_action2.clone(),
                        "more" => al.more.get(text(2).parse::<usize>().unwrap_or(usize::MAX)).cloned().flatten(),
                        _ => None,
                    };
                    self.run(h, &f);
                } else if let Some((_, act)) = self.history_acts.get(&uid).cloned() {
                    self.run(h, &Some(act));
                }
                self.dismiss(h, since);
            }
            // Notification Center's alert rows (the history): one more press of its button, forget it, clear all
            "alertAct" => {
                if let Some((_, act)) = self.history_acts.get(&num(0)).cloned() {
                    self.run(h, &Some(act));
                }
            }
            "forget" => {
                let hid = num(0);
                self.history.retain(|l| l["hid"].as_i64() != Some(hid));
                self.history_acts.remove(&hid);
                self.publish_history(h);
            }
            "clear" => {
                self.history.clear();
                self.history_acts.clear();
                self.publish_history(h);
            }
            "hold" => self.hold("rust", text(0) == "1"),
            "pin" => {
                let on = !cfg_bool(h.cfg(), "island.pinned", false);
                set_config("island.pinned", json!(on));
                h.set_state("island.pinned", json!(on));
            }
            // <kind> <index>: a live activity's control
            "live" => {
                let (kind, i) = (text(0), text(1).parse::<usize>().unwrap_or(usize::MAX));
                let every = text(2) == "all";
                let c = self.live.get(&kind).and_then(|a| a["controls"].as_array().cloned()).unwrap_or_default().into_iter().filter(|b| every || self.shows(h, &kind, b["piece"].as_str().unwrap_or(""))).nth(i);
                if let Some(c) = c {
                    if c["on"].as_bool().unwrap_or(true) {
                        self.run(h, &c.get("act").cloned());
                    }
                }
            }
            "activity" => {
                let kind = text(0);
                match a.get(1) {
                    Some(v) if v.is_object() => {
                        self.live.insert(kind, v.clone());
                    }
                    _ => {
                        self.live.remove(&kind);
                    }
                }
                self.push_live(h);
            }
            // the "Running hot" question was answered: its alert goes
            "hotAnswered" => {
                let u = self.hot_uid;
                self.remove(h, u, 0);
            }
            "hud" => self.hud(h, &text(0), &text(1)),
            "temperature" => self.temperature(h, a.get(0).and_then(Value::as_f64).unwrap_or(0.0)),
            "resources" => {
                let (cpu, mem) = (a.get(0).and_then(Value::as_f64).unwrap_or(0.0), a.get(1).and_then(Value::as_f64).unwrap_or(0.0));
                self.hot_cpu = if cpu > 0.9 { self.hot_cpu + 1 } else { 0 };
                if self.hot_cpu == 4 {
                    self.post(h, json!({"kind": "resources", "gate": "resources", "icon": "cpu", "title": "CPU is busy", "subtitle": format!("{}% for a while", (cpu * 100.0).round() as i64), "value": cpu, "color": "#FF9F0A"}), 4500);
                }
                self.hot_mem = if mem > 0.9 { self.hot_mem + 1 } else { 0 };
                if self.hot_mem == 3 {
                    let (used, total) = (a.get(2).and_then(Value::as_f64).unwrap_or(0.0), a.get(3).and_then(Value::as_f64).unwrap_or(0.0));
                    self.post(h, json!({"kind": "resources", "gate": "resources", "icon": "memory", "title": "Memory is nearly full", "subtitle": format!("{used:.1} of {total:.0} GB"), "value": mem, "color": "#FF9F0A"}), 4500);
                }
            }
            "hot" => {
                self.hot_preview = a.get(0).and_then(Value::as_bool).unwrap_or(false);
                if !self.hot_preview && !self.hot && self.hot_uid != 0 {
                    let u = self.hot_uid;
                    self.remove(h, u, 0);
                    self.hot_uid = 0;
                }
                self.publish(h);
            }
            other => eprintln!("tessera-ui: host: no island.{other}"),
        }
        self.publish(h);
    }
    fn ipc(&mut self, h: &mut Host, _target: &str, func: &str, args: &[String]) -> Option<String> {
        let arg = |i: usize| args.get(i).cloned().unwrap_or_default();
        let on = |i: usize| args.get(i).is_some_and(|s| s == "true");
        let out = match func {
            // post <icon> <title> <subtitle>  (for scripts)
            "post" => {
                let icon = if arg(0).is_empty() { "bell".into() } else { arg(0) };
                self.post(h, json!({"kind": "custom", "icon": icon, "title": arg(1), "subtitle": arg(2), "color": "#0A84FF"}), 3000);
                None
            }
            // switching the live activity is the island's own (it holds the focus)
            "next" | "previous" => {
                h.tell("menubar", json!({"island": func}));
                None
            }
            "pin" => {
                self.call(h, "island.pin", &json!([]));
                None
            }
            "hot" => {
                self.call(h, "island.hot", &json!([on(0)]));
                None
            }
            // as a swipe up, the close button or Esc
            "dismissNewest" => {
                if let Some(al) = self.alerts.iter().find(|x| x.uid == self.newest).cloned() {
                    if al.event["kind"] == "ringing" {
                        h.ask("calls.decline", json!([])); // (Esc on a ringing call declines it)
                    } else {
                        self.remove(h, al.uid, 0);
                    }
                }
                None
            }
            "holdAlerts" => {
                self.hold("ipc", on(0));
                None
            }
            "history" => Some(serde_json::to_string(&self.history).unwrap_or_default()),
            "clearHistory" => {
                self.history.clear();
                self.history_acts.clear();
                self.publish_history(h);
                None
            }
            "timeLeft" => {
                let now = now_ms();
                Some(self.alerts.iter().map(|a| format!("{}: {} s", a.event["title"].as_str().unwrap_or(""), ((a.expires - now) as f64 / 1000.0).round() as i64)).collect::<Vec<_>>().join("\n"))
            }
            "demo" => {
                self.demo(h, &arg(0));
                None
            }
            other => {
                eprintln!("tessera-ui: host: no island {other}");
                None
            }
        };
        self.publish(h);
        out
    }
}

// ── IslandCycle.js ──────────────────────────────────────────────────────────

/// Scrolling over the island to move between its live activities. A wheel notch
/// is one step; a touchpad sends many small deltas per swipe, so deltas are
/// added up until they make a notch, then it waits for the switch to finish; a
/// touchpad swipe is one step however long it runs. Up or right is back, down
/// or left is next. (The island in Rust cycles on its own scroll; this keeps
/// the rule for it and for tests.)
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct Wheel {
    acc: f64,
    at: i64,
    locked_until: i64,
    swiped: bool,
}

const NOTCH: f64 = 120.0;
const LOCK_MS: i64 = 320;
const IDLE_MS: i64 = 400;

impl Wheel {
    /// dx, dy: angleDelta; the step taken (-1, 0 or 1)
    pub fn wheel(&mut self, dx: f64, dy: f64, now: i64) -> i32 {
        let d = if dx.abs() > dy.abs() { dx } else { dy };
        if d == 0.0 {
            return 0;
        }
        if now - self.at > IDLE_MS {
            self.acc = 0.0;
            self.swiped = false;
        } else if self.acc != 0.0 && (d > 0.0) != (self.acc > 0.0) {
            self.acc = 0.0;
        }
        self.at = now;
        if now < self.locked_until || (self.swiped && d.abs() < NOTCH) {
            return 0;
        }
        self.acc += d;
        if self.acc.abs() < NOTCH {
            return 0;
        }
        let step = if self.acc < 0.0 { 1 } else { -1 };
        self.swiped = d.abs() < NOTCH;
        self.acc = 0.0;
        self.locked_until = now + LOCK_MS;
        step
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_buttons_for_the_kinds_that_keep_one() {
        let e = json!({"kind": "transfer", "action": "Open"});
        let call = Some(json!({"call": "files.open"}));
        assert_eq!(history_act(&e, "transfer", &call), Some(("Open".to_string(), json!({"call": "files.open"}))));
        assert_eq!(history_act(&e, "music", &call), None);
        assert_eq!(history_act(&json!({"kind": "transfer"}), "transfer", &call), None);
        assert_eq!(history_act(&e, "transfer", &None), None);
    }

    #[test]
    fn a_wheel_notch_is_one_step_and_waits_for_the_switch() {
        let mut w = Wheel::default();
        assert_eq!(w.wheel(0.0, -120.0, 1000), 1);
        assert_eq!(w.wheel(0.0, -120.0, 1100), 0); // (the switch is still playing)
        assert_eq!(w.wheel(0.0, 120.0, 1500), -1);
    }

    #[test]
    fn a_touchpad_swipe_is_one_step() {
        let mut w = Wheel::default();
        let mut steps = 0;
        for i in 0..40 {
            steps += w.wheel(0.0, -10.0, 1000 + i * 20);
        }
        assert_eq!(steps, 1);
        // a pause starts a new gesture
        assert_eq!(w.wheel(0.0, -120.0, 5000), 1);
    }

    #[test]
    fn up_and_left_are_back_down_and_right_are_next() {
        assert_eq!(Wheel::default().wheel(120.0, 0.0, 10), -1);
        assert_eq!(Wheel::default().wheel(-120.0, 0.0, 10), 1);
        assert_eq!(Wheel::default().wheel(0.0, 0.0, 10), 0);
    }

    #[test]
    fn pieces_have_their_defaults() {
        assert!(live_defaults("call").unwrap().contains(&"mute"));
        assert!(live_defaults("music").is_none());
    }

    #[test]
    fn important_kinds_pass_a_fullscreen_app() {
        assert!(Island::important(&json!({"kind": "timer"})));
        assert!(Island::important(&json!({"kind": "custom", "important": true})));
        assert!(!Island::important(&json!({"kind": "device"})));
    }
}

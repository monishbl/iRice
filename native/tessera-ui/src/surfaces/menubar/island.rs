//! The dynamic island's state, as shell/services/Island.qml keeps it, and
//! what the island shows (shell/modules/menubar/IslandView.qml's decision).
//!
//! Alerts: short-lived cards (`post`) that pop the island open. Several stack
//! (four at most, the oldest go first, a ringing call stays); alerts posted
//! within 400 ms of each other are one burst that staggers in (110 ms apart)
//! and stays a little longer (600 ms each); the same thing again changes its
//! card in place (`key`, a HUD or music row, the same kind and title). Each
//! leaves when its time is up (sliding out for 0.7 s); while the pointer
//! rests on them nothing times out, and each gets back the time it had left
//! plus a second. The QML Island still decides which alerts there are (its
//! services post them); they arrive here as they're posted (services::bridge).
//!
//! Activities: ongoing things (music, a timer, recording…), most important
//! first; one is shown collapsed. A new one takes the focus (music coming and
//! going doesn't take it from captions); a wheel notch or one swipe moves it.

use serde_json::Value;

pub const MAX_ALERTS: usize = 4;
pub const BURST_MS: f64 = 400.0;
pub const LEAVE_MS: f64 = 700.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Alert {
    pub uid: u64,
    /// the QML Island's uid (its buttons run there), 0 for our own
    pub qml_uid: i64,
    pub event: Value,
    /// ms before it appears (a burst staggers in)
    pub delay: f64,
    /// ms since the epoch: posted (or changed), and when it's due to leave
    pub at: f64,
    pub expires: f64,
    pub leaving: bool,
}

impl Alert {
    pub fn kind(&self) -> &str {
        self.event["kind"].as_str().unwrap_or("")
    }
    pub fn sticky(&self) -> bool {
        self.event["sticky"].as_bool().unwrap_or(false) || self.kind() == "ringing"
    }
    pub fn is_card(&self) -> bool {
        self.event["card"].is_object()
    }
}

/// An ongoing activity (what the QML shell's LiveInfo says, or our player)
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Activity {
    pub kind: String,
    pub icon: String,
    pub color: String,
    pub title: String,
    pub time: String,
    /// (icon, fill, on, danger)
    pub controls: Vec<(String, bool, bool, bool)>,
    /// the collapsed pill's pieces (Island.shows): its icon, its name, its time
    pub show_lead: bool,
    pub show_title: bool,
    pub show_time: bool,
}

impl Activity {
    pub fn from_json(v: &Value) -> Activity {
        let s = |k: &str| v[k].as_str().unwrap_or("").to_string();
        Activity {
            kind: s("kind"),
            icon: s("icon"),
            color: s("color"),
            title: s("title"),
            time: s("time"),
            controls: v["controls"]
                .as_array()
                .map(|a| a.iter().map(|c| (c["icon"].as_str().unwrap_or("").to_string(), c["fill"].as_bool().unwrap_or(false), c["on"].as_bool().unwrap_or(true), c["danger"].as_bool().unwrap_or(false))).collect())
                .unwrap_or_default(),
            show_lead: v["lead"].as_bool().unwrap_or(true),
            show_title: v["name"].as_bool().unwrap_or(true),
            show_time: v["clock"].as_bool().unwrap_or(true),
        }
    }
}

/// IslandCycle.js: wheel deltas (QML's angleDelta units) into steps
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Wheel {
    acc: f64,
    at: f64,
    locked_until: f64,
    swiped: bool,
}

pub const NOTCH: f64 = 120.0;

impl Wheel {
    /// -1, 0 or 1: down or left moves on (1), up or right back
    pub fn step(&mut self, dx: f64, dy: f64, now: f64) -> i32 {
        let d = if dx.abs() > dy.abs() { dx } else { dy };
        if d == 0.0 {
            return 0;
        }
        if now - self.at > 400.0 {
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
        self.locked_until = now + 320.0;
        step
    }
}

#[derive(Clone, Debug, Default)]
pub struct Island {
    /// oldest first; the newest is the event shown
    pub alerts: Vec<Alert>,
    next_uid: u64,
    burst_at: f64,
    burst_index: i32,
    held: bool,
    held_at: f64,
    pub activities: Vec<Activity>,
    pub focus_kind: String,
    last_kinds: Vec<String>,
    pub wheel: Wheel,
}

impl Island {
    /// The newest alert (the one that pops it open)
    pub fn event(&self) -> Option<&Alert> {
        self.alerts.last()
    }
    /// A card or a ringing call: the island opens wider
    pub fn has_card(&self) -> bool {
        self.alerts.iter().any(|a| a.is_card() || a.kind() == "ringing")
    }
    pub fn ringing(&self) -> bool {
        self.alerts.iter().any(|a| a.kind() == "ringing")
    }
    pub fn held(&self) -> bool {
        self.held
    }

    fn replace(&mut self, i: usize, e: Value, ms: f64, now: f64, extend: bool, qml_uid: i64) -> u64 {
        let a = &mut self.alerts[i];
        a.expires = if extend { a.expires.max(now + ms) } else { now + ms };
        a.event = e;
        a.delay = 0.0;
        a.at = now;
        a.leaving = false;
        if qml_uid != 0 {
            a.qml_uid = qml_uid;
        }
        a.uid
    }

    /// Island.post: returns the alert's uid (a new one, or the one it changed)
    pub fn post(&mut self, e: Value, ms: f64, qml_uid: i64, now: f64) -> u64 {
        let kind = e["kind"].as_str().unwrap_or("").to_string();
        // the QML Island changed one of its own in place
        if qml_uid != 0 {
            if let Some(i) = self.alerts.iter().position(|a| a.qml_uid == qml_uid) {
                let extend = !e["key"].is_string() && kind != "hud" && kind != "music";
                return self.replace(i, e, ms, now, extend, qml_uid);
            }
        }
        // One thing, one card: the same `key` takes its place
        if let Some(key) = e["key"].as_str() {
            if let Some(i) = self.alerts.iter().position(|a| a.event["key"].as_str() == Some(key)) {
                return self.replace(i, e, ms, now, false, qml_uid);
            }
        }
        // the HUD and music update their row in place
        if kind == "hud" || kind == "music" {
            if let Some(i) = self.alerts.iter().position(|a| a.kind() == kind) {
                return self.replace(i, e, ms, now, false, qml_uid);
            }
        }
        // the same thing again while it's up: in place, a little longer
        if let Some(title) = e["title"].as_str().filter(|t| !t.is_empty()) {
            if let Some(i) = self.alerts.iter().position(|a| a.kind() == kind && a.event["title"].as_str() == Some(title) && a.expires > now) {
                return self.replace(i, e, ms, now, true, qml_uid);
            }
        }
        self.burst_index = if self.next_uid > 0 && now - self.burst_at < BURST_MS { self.burst_index + 1 } else { 0 };
        if self.burst_index == 0 {
            self.burst_at = now;
        }
        self.next_uid += 1;
        let uid = self.next_uid;
        let b = self.burst_index as f64;
        self.alerts.push(Alert { uid, qml_uid, event: e, delay: b * 110.0, at: now, expires: now + ms + b * 600.0, leaving: false });
        // the oldest go first (a ringing call stays)
        while self.alerts.len() > MAX_ALERTS {
            let mut i = 0;
            while i < self.alerts.len() - 1 && self.alerts[i].sticky() {
                i += 1;
            }
            self.alerts.remove(i);
        }
        uid
    }

    /// Island.update: change an alert in place (the QML uid)
    pub fn update(&mut self, qml_uid: i64, patch: &Value) {
        if let (Some(a), Some(p)) = (self.alerts.iter_mut().find(|a| a.qml_uid == qml_uid), patch.as_object()) {
            for (k, v) in p {
                a.event[k] = v.clone();
            }
        }
    }

    pub fn remove(&mut self, uid: u64) {
        self.alerts.retain(|a| a.uid != uid);
    }
    pub fn remove_qml(&mut self, qml_uid: i64) {
        self.alerts.retain(|a| a.qml_uid != qml_uid);
    }

    /// Close everything but a ringing call and, given `since`, what was
    /// posted or changed from then on (an action's own "Connecting…")
    pub fn dismiss(&mut self, since: f64) {
        self.alerts.retain(|a| a.sticky() || (since > 0.0 && a.at >= since));
    }

    /// The pointer rests on the alerts (true) or left (false)
    pub fn hold(&mut self, on: bool, now: f64) {
        if on == self.held {
            return;
        }
        self.held = on;
        if on {
            self.held_at = now;
        } else {
            for a in &mut self.alerts {
                a.expires += (now - self.held_at.max(a.at)) + 1000.0;
            }
        }
    }

    /// The sweep (every 250 ms while there are alerts and none is held):
    /// due ones start leaving, then go. Returns whether anything changed
    pub fn sweep(&mut self, now: f64) -> bool {
        if self.held {
            return false;
        }
        let before = (self.alerts.len(), self.alerts.iter().filter(|a| a.leaving).count());
        self.alerts.retain(|a| a.expires + LEAVE_MS > now);
        for a in &mut self.alerts {
            if a.expires <= now {
                a.leaving = true;
            }
        }
        before != (self.alerts.len(), self.alerts.iter().filter(|a| a.leaving).count())
    }

    /// The activities now (most important first): a new one takes the focus
    pub fn set_activities(&mut self, acts: Vec<Activity>) {
        let kinds: Vec<String> = acts.iter().map(|a| a.kind.clone()).collect();
        if let Some(added) = kinds.iter().find(|k| !self.last_kinds.contains(k)) {
            let captions_read = added == "music" && self.focus_kind == "captions" && kinds.iter().any(|k| k == "captions");
            if !captions_read {
                self.focus_kind = added.clone();
            }
        }
        self.last_kinds = kinds;
        self.activities = acts;
    }
    pub fn primary(&self) -> Option<&Activity> {
        self.activities.iter().find(|a| a.kind == self.focus_kind).or(self.activities.first())
    }
    pub fn focus_index(&self) -> usize {
        self.primary().and_then(|p| self.activities.iter().position(|a| a.kind == p.kind)).unwrap_or(0)
    }
    pub fn cycle(&mut self, step: i32) {
        let n = self.activities.len() as i32;
        if n < 2 {
            return;
        }
        let i = (self.focus_index() as i32 + step + n).rem_euclid(n);
        self.focus_kind = self.activities[i as usize].kind.clone();
    }
    /// A wheel or swipe over it; the step taken (for the swap's direction)
    pub fn scroll(&mut self, dx: f64, dy: f64, now: f64) -> i32 {
        if self.activities.len() < 2 {
            return 0;
        }
        let s = self.wheel.step(dx, dy, now);
        if s != 0 {
            self.cycle(s);
        }
        s
    }
}

/// Island.qml's activities, in its order, with the music one (when there
/// is music, playing or lingering a minute after pausing) at its place:
/// after a timer, before the stopwatch
pub fn with_music(mut acts: Vec<Activity>, music: Option<Activity>) -> Vec<Activity> {
    acts.retain(|a| a.kind != "music");
    if let Some(m) = music {
        let before = ["mirroring", "casting", "dictation", "captions", "recording", "timer"];
        let at = acts.iter().position(|a| !before.contains(&a.kind.as_str())).unwrap_or(acts.len());
        acts.insert(at, m);
    }
    acts
}

/// What the island is doing, IslandView's decision
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// the spaces' dots on the bar
    Idle,
    /// a live activity widens the pill
    Compact,
    /// open: alerts, or (hovered, pinned) the activities
    Expanded,
    /// over a fullscreen app, "compact" mode: the newest alert as a small pill
    Pill,
    /// covered by a fullscreen app with nothing to say
    Hidden,
}

pub struct View {
    pub mode: Mode,
    /// the alerts are in it (not a hover or the pin)
    pub event_inside: bool,
}

/// `fs_mode`: show | important | compact (Settings → island → in full screen)
pub fn view(i: &Island, hover_open: bool, pinned: bool, over_fs: bool, fs_mode: &str, fs_open: bool) -> View {
    let showing = i.event().is_some();
    let pill = over_fs && fs_mode == "compact" && showing && !fs_open && !i.ringing();
    let event_inside = showing;
    let expanded = (hover_open && !over_fs) || (event_inside && !pill) || (pinned && !over_fs);
    let mode = if pill {
        Mode::Pill
    } else if expanded {
        Mode::Expanded
    } else if over_fs {
        Mode::Hidden
    } else if i.primary().is_some() {
        Mode::Compact
    } else {
        Mode::Idle
    };
    View { mode, event_inside }
}

/// An SVG arc for a ring: from the top, clockwise, `frac` of the way round
/// (a battery's level, a timer's time left), in a viewbox around (cx, cy)
pub fn arc(cx: f32, cy: f32, r: f32, frac: f32) -> String {
    let f = frac.clamp(0.0, 1.0);
    if f >= 0.999 {
        return format!("M {cx} {} A {r} {r} 0 1 1 {} {}", cy - r, cx - 0.01, cy - r);
    }
    if f <= 0.001 {
        return format!("M {cx} {} L {cx} {}", cy - r, cy - r);
    }
    let a = (-90.0 + 360.0 * f).to_radians();
    let (x, y) = (cx + r * a.cos(), cy + r * a.sin());
    format!("M {cx} {} A {r} {r} 0 {} 1 {:.2} {:.2}", cy - r, (f > 0.5) as i32, x, y)
}

/// A colour as the QML shell writes one: #rrggbb or #aarrggbb (Qt's order)
pub fn color(s: &str) -> Option<slint::Color> {
    let h = s.strip_prefix('#')?;
    let v = u32::from_str_radix(h, 16).ok()?;
    match h.len() {
        6 => Some(slint::Color::from_rgb_u8((v >> 16) as u8, (v >> 8) as u8, v as u8)),
        8 => Some(slint::Color::from_argb_u8((v >> 24) as u8, (v >> 16) as u8, (v >> 8) as u8, v as u8)),
        _ => None,
    }
}

/// Is it the shell's red (IslandInk.isRed: what's urgent turns Material's error)
pub fn is_red(c: slint::Color) -> bool {
    c.red() as f32 / 255.0 > 0.8 && (c.green() as f32 / 255.0) < 0.45 && (c.blue() as f32 / 255.0) < 0.45
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rings_and_colours() {
        assert_eq!(arc(22.0, 22.0, 18.0, 0.25), "M 22 4 A 18 18 0 0 1 40.00 22.00");
        assert_eq!(arc(22.0, 22.0, 18.0, 0.75), "M 22 4 A 18 18 0 1 1 4.00 22.00");
        assert!(arc(17.0, 17.0, 15.0, 1.0).contains("0 1 1 16.99 2"));
        assert_eq!(arc(17.0, 17.0, 15.0, 0.0), "M 17 2 L 17 2");
        assert_eq!(color("#30D158"), Some(slint::Color::from_rgb_u8(0x30, 0xd1, 0x58)));
        assert_eq!(color("#99ffffff"), Some(slint::Color::from_argb_u8(0x99, 255, 255, 255)));
        assert_eq!(color("red"), None);
        assert!(is_red(color("#FF453A").unwrap()) && !is_red(color("#30D158").unwrap()));
    }

    fn ev(kind: &str, title: &str) -> Value {
        json!({"kind": kind, "title": title})
    }

    #[test]
    fn alerts_stack_and_the_newest_shows() {
        let mut i = Island::default();
        i.post(ev("charging", "Charging"), 3000.0, 1, 0.0);
        i.post(ev("device", "Earbuds"), 3000.0, 2, 1000.0);
        assert_eq!(i.event().unwrap().event["title"], "Earbuds");
        assert_eq!(i.alerts.len(), 2);
    }

    #[test]
    fn a_burst_staggers_in_and_stays_longer() {
        let mut i = Island::default();
        for (n, k) in ["device", "charging", "audio"].iter().enumerate() {
            i.post(ev(k, k), 5000.0, n as i64 + 1, 100.0 * n as f64);
        }
        let d: Vec<f64> = i.alerts.iter().map(|a| a.delay).collect();
        assert_eq!(d, vec![0.0, 110.0, 220.0]);
        let ex: Vec<f64> = i.alerts.iter().map(|a| a.expires).collect();
        assert_eq!(ex, vec![5000.0, 5700.0, 6400.0]);
        // a moment later it's a new burst
        i.post(ev("usb", "Drive"), 3000.0, 9, 2000.0);
        assert_eq!(i.event().unwrap().delay, 0.0);
    }

    #[test]
    fn four_at_most_and_a_ringing_call_stays() {
        let mut i = Island::default();
        i.post(json!({"kind": "ringing", "title": "Maya"}), 60000.0, 1, 0.0);
        for n in 0..5 {
            i.post(ev("custom", &format!("n{n}")), 3000.0, 10 + n, 1000.0 * (n + 1) as f64);
        }
        assert_eq!(i.alerts.len(), MAX_ALERTS);
        assert_eq!(i.alerts[0].kind(), "ringing");
        let t: Vec<&str> = i.alerts.iter().skip(1).map(|a| a.event["title"].as_str().unwrap()).collect();
        assert_eq!(t, vec!["n2", "n3", "n4"]);
        // dismiss leaves the call
        i.dismiss(0.0);
        assert_eq!(i.alerts.len(), 1);
        assert!(i.has_card() && i.ringing());
    }

    #[test]
    fn the_same_thing_changes_in_place() {
        let mut i = Island::default();
        // a key: Connect → Connecting… → Connected, one card, new time
        let a = i.post(json!({"kind": "custom", "key": "bt:x", "title": "Earbuds", "subtitle": "Connect"}), 600000.0, 1, 0.0);
        let b = i.post(json!({"kind": "device", "key": "bt:x", "title": "Earbuds", "subtitle": "Connected"}), 5000.0, 2, 1000.0);
        assert_eq!(a, b);
        assert_eq!(i.alerts.len(), 1);
        assert_eq!((i.alerts[0].expires, i.alerts[0].event["subtitle"].as_str()), (6000.0, Some("Connected")));
        // the HUD's row
        let h1 = i.post(json!({"kind": "hud", "value": 0.4}), 1600.0, 3, 2000.0);
        let h2 = i.post(json!({"kind": "hud", "value": 0.5}), 1600.0, 4, 2100.0);
        assert_eq!(h1, h2);
        // the same kind and title while up: stays at least as long as it had
        let mut j = Island::default();
        j.post(ev("device", "Earbuds"), 8000.0, 1, 0.0);
        j.post(ev("device", "Earbuds"), 3000.0, 1, 1000.0);
        assert_eq!((j.alerts.len(), j.alerts[0].expires), (1, 8000.0));
        // the QML uid it already has: in place too
        j.post(ev("device", "Earbuds · 80%"), 3000.0, 1, 2000.0);
        assert_eq!(j.alerts.len(), 1);
        j.update(1, &json!({"subtitle": "80%"}));
        assert_eq!(j.alerts[0].event["subtitle"], "80%");
        j.remove_qml(1);
        assert!(j.alerts.is_empty());
    }

    #[test]
    fn they_leave_on_time_unless_held() {
        let mut i = Island::default();
        i.post(ev("custom", "a"), 3000.0, 1, 0.0);
        assert!(!i.sweep(2900.0));
        assert!(i.sweep(3000.0));
        assert!(i.alerts[0].leaving);
        assert!(i.sweep(3700.0));
        assert!(i.alerts.is_empty());
        // held at 1 s for 5 s: it gets those 5 s back, and a second
        i.post(ev("custom", "b"), 3000.0, 2, 10000.0);
        i.hold(true, 11000.0);
        assert!(!i.sweep(14000.0));
        i.hold(false, 16000.0);
        assert_eq!(i.alerts[0].expires, 13000.0 + 5000.0 + 1000.0);
    }

    #[test]
    fn dismiss_keeps_what_an_action_just_posted() {
        let mut i = Island::default();
        i.post(ev("custom", "old"), 3000.0, 1, 0.0);
        i.post(ev("custom", "Connecting…"), 3000.0, 2, 500.0);
        i.dismiss(400.0);
        assert_eq!(i.alerts.len(), 1);
        assert_eq!(i.alerts[0].event["title"], "Connecting…");
    }

    fn act(k: &str) -> Activity {
        Activity { kind: k.into(), ..Default::default() }
    }

    #[test]
    fn a_new_activity_takes_the_focus() {
        let mut i = Island::default();
        i.set_activities(vec![act("timer")]);
        assert_eq!(i.primary().unwrap().kind, "timer");
        i.set_activities(vec![act("timer"), act("music")]);
        assert_eq!(i.primary().unwrap().kind, "music");
        i.cycle(1);
        assert_eq!(i.primary().unwrap().kind, "timer");
        // music coming back doesn't take it from captions
        i.set_activities(vec![act("captions")]);
        i.set_activities(vec![act("captions"), act("music")]);
        assert_eq!(i.primary().unwrap().kind, "captions");
        // the focused one gone: the first
        i.set_activities(vec![act("music")]);
        assert_eq!(i.primary().unwrap().kind, "music");
    }

    #[test]
    fn music_sits_at_its_place() {
        let m = Some(act("music"));
        let k = |v: Vec<Activity>| v.into_iter().map(|a| a.kind).collect::<Vec<_>>();
        assert_eq!(k(with_music(vec![act("recording"), act("timer"), act("stopwatch")], m.clone())), vec!["recording", "timer", "music", "stopwatch"]);
        assert_eq!(k(with_music(vec![act("music"), act("call")], m.clone())), vec!["music", "call"]);
        assert_eq!(k(with_music(vec![act("music")], None)), Vec::<String>::new());
    }

    #[test]
    fn the_wheel_steps_once_per_notch_or_swipe() {
        let mut w = Wheel::default();
        assert_eq!(w.step(0.0, -120.0, 0.0), 1);
        // locked while the swap plays
        assert_eq!(w.step(0.0, -120.0, 100.0), 0);
        assert_eq!(w.step(0.0, 120.0, 500.0), -1);
        // a touchpad swipe: small deltas add up to one step, then nothing more
        let mut w = Wheel::default();
        let steps: i32 = (0..30).map(|n| w.step(0.0, -20.0, 1000.0 + n as f64 * 16.0)).sum();
        assert_eq!(steps, 1);
        // a new gesture after a pause
        assert_eq!(w.step(0.0, -130.0, 3000.0), 1);
        let mut i = Island::default();
        i.set_activities(vec![act("timer"), act("music")]);
        assert_eq!(i.primary().unwrap().kind, "timer", "both new at once: the first");
        assert_eq!(i.scroll(0.0, -120.0, 0.0), 1);
        assert_eq!(i.primary().unwrap().kind, "music");
    }

    #[test]
    fn what_it_shows() {
        let mut i = Island::default();
        assert_eq!(view(&i, false, false, false, "show", false).mode, Mode::Idle);
        assert_eq!(view(&i, true, false, false, "show", false).mode, Mode::Expanded);
        assert_eq!(view(&i, false, true, false, "show", false).mode, Mode::Expanded);
        i.set_activities(vec![act("music")]);
        assert_eq!(view(&i, false, false, false, "show", false).mode, Mode::Compact);
        // over a fullscreen app: hidden, and the pin and hover don't open it
        assert_eq!(view(&i, true, true, true, "show", false).mode, Mode::Hidden);
        i.post(ev("charging", "Charging"), 3000.0, 1, 0.0);
        let v = view(&i, false, false, false, "show", false);
        assert!(v.event_inside && v.mode == Mode::Expanded);
        assert_eq!(view(&i, false, false, true, "show", false).mode, Mode::Expanded);
        assert_eq!(view(&i, false, false, true, "compact", false).mode, Mode::Pill);
        assert_eq!(view(&i, false, false, true, "compact", true).mode, Mode::Expanded);
    }
}

//! Mouse and trackpad (Settings → Mouse, Trackpad), replacing
//! shell/services/Mouse.qml (and FocusDecision.js, the pure part of it).
//!
//! Mice: applied to each mouse as a Hyprland device config (`hl.device`);
//! trackpads keep their own speed, acceleration and scrolling. Window focus
//! (click or hover) is Hyprland's input.follow_mouse. Re-applied after a
//! config reload and when the device list changes (udev says so: one
//! `udevadm monitor`; without it, every 60 s). Also the tessera-glass plugin's
//! pointer boundaries and pointer-follow, focus that follows the pointer after
//! scene changes, and apps asking to come forward ("urgent").
//!
//! Prefix "mouse": bringForward [address]. State for others: bus "mouse"
//! { mice, touchpads, boundaryAvailable } (RustBridgeInput's feed).
//!
//! Differences: the idle monitor that tells "the user just clicked or typed"
//! without the plugin (ext-idle-notify) isn't ported: without plugin v4 an
//! urgent window is announced, not brought forward (the plugin is the normal
//! case). Island's "Show" button comes back as `mouse.bringForward`.

use super::common::*;
use serde_json::{json, Value};
use std::any::Any;
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use tessera_core::host::{Host, Service};

// ── FocusDecision.js ──────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub enum Act {
    Warp(String),
    Refocus,
    None,
}

const SCENE_CHANGES: [&str; 7] = ["workspace", "workspacev2", "activespecial", "activespecialv2", "closewindow", "movewindowv2", "changefloatingmode"];

fn is_hex_addr(a: &str) -> bool {
    let b = a.strip_prefix("0x").unwrap_or(a);
    !b.is_empty() && b.chars().all(|c| c.is_ascii_hexdigit())
}

/// After a burst of Hyprland events: what to do with the pointer
pub fn after_events(events: &[Raw]) -> Act {
    let mut opened = String::new();
    let mut changed = false;
    for e in events {
        if e.name == "openwindow" {
            let a = e.data.split(',').next().unwrap_or("");
            if is_hex_addr(a) {
                opened = if a.starts_with("0x") { a.to_string() } else { format!("0x{a}") };
            }
        } else if SCENE_CHANGES.contains(&e.name.as_str()) {
            changed = true;
        }
    }
    if !opened.is_empty() {
        Act::Warp(opened)
    } else if changed {
        Act::Refocus
    } else {
        Act::None
    }
}

/// A window asking for attention: bring it forward right after the user
/// clicked or typed, else just show that it wants attention
pub fn on_urgent(input_age_ms: Option<i64>, already_active: bool) -> &'static str {
    if already_active {
        return "none";
    }
    match input_age_ms {
        Some(a) if (0..=3000).contains(&a) => "bring",
        _ => "notify",
    }
}

// ── Hyprland device config ────────────────────────────────────────────────

pub fn is_mouse(name: &str) -> bool {
    let n = name.to_lowercase();
    !["touchpad", "trackpad", "ydotool", "virtual", "wacom", "stylus", "pen"].iter().any(|k| n.contains(k))
}

fn is_pad(name: &str) -> bool {
    let n = name.to_lowercase();
    n.contains("touchpad") || n.contains("trackpad")
}

fn num(v: &Value, k: &str, d: f64) -> f64 {
    v[k].as_f64().unwrap_or(d)
}
fn flag(v: &Value, k: &str, d: bool) -> bool {
    v[k].as_bool().unwrap_or(d)
}
fn text<'a>(v: &'a Value, k: &str, d: &'a str) -> &'a str {
    v[k].as_str().unwrap_or(d)
}
fn tf(b: bool) -> &'static str {
    if b {
        "true"
    } else {
        "false"
    }
}

pub fn device_spec(name: &str, m: &Value) -> String {
    let sens = num(m, "speed", 0.0).clamp(-1.0, 1.0);
    format!(
        "hl.device({{ name = \"{}\", accel_profile = \"{}\", sensitivity = {:.2}, natural_scroll = {}, scroll_factor = {:.2}, left_handed = {}, middle_button_emulation = {} }})",
        name.replace('\\', "\\\\").replace('"', "\\\""),
        if text(m, "accelProfile", "adaptive") == "flat" { "flat" } else { "adaptive" },
        sens,
        tf(flag(m, "naturalScroll", false)),
        num(m, "scrollSpeed", 1.0).clamp(0.2, 3.0),
        tf(flag(m, "leftHanded", false)),
        tf(flag(m, "middleEmulation", false)),
    )
}

pub fn touchpad_spec(tp: &Value) -> String {
    format!(
        "hl.config({{ input = {{ touchpad = {{ tap_to_click = {}, tap_and_drag = {}, drag_lock = {}, natural_scroll = {}, scroll_factor = {:.2}, disable_while_typing = {}, clickfinger_behavior = {}, middle_button_emulation = {} }} }} }})",
        tf(flag(tp, "tapToClick", true)),
        tf(flag(tp, "tapAndDrag", true)),
        if flag(tp, "dragLock", false) { 1 } else { 0 },
        tf(flag(tp, "naturalScroll", true)),
        num(tp, "scrollSpeed", 0.7).clamp(0.2, 3.0),
        tf(flag(tp, "disableWhileTyping", false)),
        tf(text(tp, "rightClick", "twoFingers") != "corner"),
        tf(flag(tp, "middleClick", false)),
    )
}

pub fn touchpad_device(name: &str, tp: &Value) -> String {
    format!("hl.device({{ name = \"{}\", sensitivity = {:.2} }})", name.replace('\\', "\\\\").replace('"', "\\\""), num(tp, "speed", 0.0).clamp(-1.0, 1.0))
}

/// Any setting change (Mouse.signature)
fn signature(cfg: &Value) -> String {
    let m = &cfg["mouse"];
    let t = &cfg["trackpad"];
    let g = |v: &Value, k: &str| v[k].to_string();
    [
        g(m, "accelProfile"), g(m, "speed"), g(m, "naturalScroll"), g(m, "scrollSpeed"), g(m, "leftHanded"), g(m, "middleEmulation"), g(m, "focus"),
        g(t, "speed"), g(t, "tapToClick"), g(t, "tapAndDrag"), g(t, "dragLock"), g(t, "naturalScroll"), g(t, "scrollSpeed"), g(t, "disableWhileTyping"), g(t, "rightClick"), g(t, "middleClick"),
    ]
    .join("|")
}

enum M {
    Raw(Raw),
    Devices(Value),
    Plugin(String, i64),
    LookAgain,
    UdevLine,
    UdevEnded,
    ApplyLater(u64),
    FocusLater(u64),
    UrgentSettle(u64),
    UrgentInfo { address: String, found: bool, active: bool, class: String, title: String, age: Option<i64> },
    Lease(u64),
    Poll,
}

pub struct Mouse {
    mice: Vec<String>,
    touchpads: Vec<String>,
    sig: String,
    apply_gen: u64,
    look_gen: u64,
    plugin_name: String,
    plugin_version: i64,
    pending: Vec<Raw>,
    focus_gen: u64,
    urgent_window: String,
    urgent_gen: u64,
    opened_class: BTreeMap<String, (String, u64)>,
    boundary_sig: String,
    lease_gen: u64,
    udev: bool,
}

impl Default for Mouse {
    fn default() -> Self {
        Self {
            mice: vec![], touchpads: vec![], sig: String::new(), apply_gen: 0, look_gen: 0, plugin_name: "tessera-glass".into(), plugin_version: 0,
            pending: vec![], focus_gen: 0, urgent_window: String::new(), urgent_gen: 0, opened_class: BTreeMap::new(), boundary_sig: String::new(), lease_gen: 0, udev: false,
        }
    }
}

impl Mouse {
    fn boundary_available(&self) -> bool {
        self.plugin_version >= 3
    }
    fn follow_available(&self) -> bool {
        self.plugin_version >= 4
    }
    fn hover(h: &Host) -> bool {
        text(&h.cfg()["mouse"], "focus", "hover") == "hover"
    }
    fn locked(h: &Host) -> bool {
        h.state("locked").as_bool().unwrap_or(false)
    }
    fn selecting() -> String {
        crate::bus::get("screentools")["selecting"].as_str().unwrap_or("").to_string()
    }
    fn publish(&self) {
        crate::bus::put("mouse", json!({"mice": self.mice, "touchpads": self.touchpads, "boundaryAvailable": self.boundary_available()}));
    }

    fn apply(&mut self, h: &mut Host) {
        if h.cfg().is_null() {
            return;
        }
        let m = &h.cfg()["mouse"];
        let tp = &h.cfg()["trackpad"];
        let hover = Self::hover(h);
        let mut lua = format!("hl.config({{ input = {{ follow_mouse = {}, mouse_refocus = true, focus_on_close = {} }} }})\n", if hover { 1 } else { 2 }, if hover { 1 } else { 0 });
        for d in &self.mice {
            lua += &device_spec(d, m);
            lua.push('\n');
        }
        lua += &touchpad_spec(tp);
        lua.push('\n');
        for t in &self.touchpads {
            lua += &touchpad_device(t, tp);
            lua.push('\n');
        }
        hyprctl_eval(&lua);
        self.apply_boundaries(h);
    }

    fn plugin(&self, args: &[&str]) {
        let mut v = vec![self.plugin_name.as_str()];
        v.extend_from_slice(args);
        hyprctl(&v);
    }

    fn apply_boundaries(&mut self, h: &mut Host) {
        if h.cfg().is_null() {
            return;
        }
        if self.follow_available() {
            self.plugin(&["pointer-follow", if Self::hover(h) { "1" } else { "0" }]);
        }
        if !self.boundary_available() {
            return;
        }
        let m = &h.cfg()["mouse"];
        let mode = match text(m, "boundaryMode", "corners") {
            "corners" => "1",
            "resistance" => "2",
            _ => "0",
        };
        let size = (num(m, "cornerSize", 24.0).clamp(4.0, 100.0) as i64).to_string();
        self.plugin(&["pointer-policy", mode, &size]);
        self.confine_selection(h);
    }

    fn confine_selection(&mut self, h: &mut Host) {
        if !self.boundary_available() {
            return;
        }
        let off = text(&h.cfg()["mouse"], "boundaryMode", "corners") == "off";
        let sel = Self::selecting();
        let name = if !off && !Self::locked(h) && !sel.is_empty() { crate::bus::get("screentools")["selectingScreen"].as_str().unwrap_or("").to_string() } else { String::new() };
        self.plugin(&["pointer-confine", if name.is_empty() { "-" } else { &name }]);
        // A short lease releases confinement if the shell exits during selection
        if !off && !sel.is_empty() {
            self.lease_gen += 1;
            after(h.sender("Mouse"), 500, M::Lease(self.lease_gen));
        }
    }

    fn look(&mut self, h: &mut Host) {
        let send = h.sender("Mouse");
        std::thread::spawn(move || send(Box::new(M::Devices(hyprctl_json(&["devices"])))));
    }

    fn bring_forward(&self, h: &Host, address: &str) {
        let mut lua = format!("hl.dispatch(hl.dsp.focus({{ window = {} }}))\n", serde_json::to_string(&format!("address:{address}")).unwrap_or_default());
        if Self::hover(h) {
            lua += &format!("dofile({})({})\n", serde_json::to_string(&crate::util::script("pointer-focus.lua")).unwrap_or_default(), serde_json::to_string(address).unwrap_or_default());
        }
        hyprctl_eval(&lua);
    }

    fn urgent_decided(&mut self, h: &mut Host, address: String, found: bool, active: bool, class: String, title: String, age: Option<i64>) {
        if !found {
            return;
        }
        let opened = self.opened_class.get(&address).cloned();
        let fresh = opened.as_ref().is_some_and(|(_, at)| now_ms().saturating_sub(*at) < 2000);
        let cls = opened.as_ref().map(|(c, _)| c.clone()).filter(|c| !c.is_empty()).unwrap_or_else(|| class.clone());
        // Tessera's own windows are only opened on purpose: they come forward,
        // never as a "wants your attention" alert
        if fresh || cls == "org.quickshell" {
            if !active {
                self.bring_forward(h, &address);
            }
            return;
        }
        match on_urgent(age, active) {
            "bring" => self.bring_forward(h, &address),
            "notify" => {
                let name = h
                    .st
                    .shell
                    .get::<tessera_core::services::apps::Apps>()
                    .and_then(|a| a.entry_for(&class))
                    .map(|e| e.name.clone())
                    .filter(|n| !n.is_empty())
                    .or_else(|| Some(title.clone()).filter(|t| !t.is_empty()))
                    .or_else(|| Some(class.clone()).filter(|c| !c.is_empty()))
                    .unwrap_or_else(|| "An app".into());
                super::common_island::post(h, json!({"kind": "custom", "icon": "bell-ringing", "title": name, "subtitle": "Wants your attention", "color": "accent", "action": "Show", "onAction": {"call": "mouse.bringForward", "args": [address]}}), 6000);
            }
            _ => {}
        }
    }
}

impl Service for Mouse {
    fn name(&self) -> &'static str {
        "Mouse"
    }
    fn calls(&self) -> &'static [&'static str] {
        &["mouse"]
    }
    fn start(&mut self, h: &mut Host) {
        self.sig = signature(h.cfg());
        spawn_hypr_events(&[], h.sender("Mouse"));
        // the glass plugin: its name and version (a ping answers "<name> <version>")
        let send = h.sender("Mouse");
        std::thread::spawn(move || {
            for n in ["tessera-glass", "irice-glass"] {
                if let Some((0, o)) = crate::util::capture(&["hyprctl", n, "ping"], std::time::Duration::from_secs(3)) {
                    if o.trim().starts_with(n) {
                        let v = o.split_whitespace().nth(1).and_then(|v| v.parse().ok()).unwrap_or(1);
                        send(Box::new(M::Plugin(n.to_string(), v)));
                        return;
                    }
                }
            }
        });
        // udev says when a mouse comes or goes; without udevadm, every 60 s
        let send = h.sender("Mouse");
        self.udev = Command::new("udevadm").arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success());
        if self.udev {
            std::thread::spawn(move || loop {
                let child = Command::new("udevadm").args(["monitor", "--udev", "--subsystem-match=input"]).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn();
                if let Ok(mut c) = child {
                    if let Some(out) = c.stdout.take() {
                        for line in BufReader::new(out).lines().map_while(Result::ok) {
                            if line.split(|c: char| !c.is_alphanumeric() && c != '_').any(|w| w == "add" || w == "remove") {
                                send(Box::new(M::UdevLine));
                            }
                        }
                    }
                    let _ = c.wait();
                }
                send(Box::new(M::UdevEnded));
                std::thread::sleep(std::time::Duration::from_secs(10));
            });
        } else {
            after(h.sender("Mouse"), 60000, M::Poll);
        }
        self.look(h);
        self.apply_gen += 1;
        after(h.sender("Mouse"), 150, M::ApplyLater(self.apply_gen));
    }
    fn config_changed(&mut self, h: &mut Host) {
        let sig = signature(h.cfg());
        if sig != self.sig {
            self.sig = sig;
            self.apply_gen += 1;
            after(h.sender("Mouse"), 150, M::ApplyLater(self.apply_gen));
        }
        let m = &h.cfg()["mouse"];
        let bs = format!("{}|{}", text(m, "boundaryMode", "corners"), num(m, "cornerSize", 24.0));
        if bs != self.boundary_sig {
            self.boundary_sig = bs;
            self.apply_boundaries(h);
        }
    }
    fn call(&mut self, h: &mut Host, call: &str, args: &Value) {
        match call.split_once('.').map(|x| x.1).unwrap_or("") {
            "bringForward" => {
                if let Some(a) = args.get(0).and_then(Value::as_str) {
                    self.bring_forward(h, a);
                }
            }
            // The lock screen or a screen selection changed (Mouse.confineSelection)
            "confine" => self.confine_selection(h),
            _ => {}
        }
    }
    fn message(&mut self, h: &mut Host, msg: Box<dyn Any + Send>) {
        let msg = match msg.downcast::<Raw>() {
            Ok(e) => {
                if e.name == "configreloaded" {
                    self.apply_gen += 1;
                    after(h.sender("Mouse"), 150, M::ApplyLater(self.apply_gen));
                }
                if e.name == "openwindow" {
                    let f: Vec<&str> = e.data.split(',').collect();
                    let a = f[0].trim_start_matches("0x");
                    self.opened_class.insert(format!("0x{a}"), (f.get(2).unwrap_or(&"").to_string(), now_ms()));
                    if self.opened_class.len() > 64 {
                        if let Some(k) = self.opened_class.keys().next().cloned() {
                            self.opened_class.remove(&k);
                        }
                    }
                }
                if e.name == "urgent" {
                    let a = e.data.trim_start_matches("0x");
                    if !is_hex_addr(a) || Self::locked(h) {
                        return;
                    }
                    self.urgent_window = format!("0x{a}");
                    self.urgent_gen += 1;
                    after(h.sender("Mouse"), 250, M::UrgentSettle(self.urgent_gen));
                    return;
                }
                if !Self::hover(h) {
                    return;
                }
                if e.name == "openwindow" || SCENE_CHANGES.contains(&e.name.as_str()) {
                    self.pending.push((*e).clone());
                    self.focus_gen += 1;
                    after(h.sender("Mouse"), 180, M::FocusLater(self.focus_gen));
                }
                return;
            }
            Err(m) => m,
        };
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Raw(_) => {}
            M::Devices(d) => {
                let mice = d["mice"].as_array().cloned().unwrap_or_default();
                let names: Vec<String> = mice.iter().filter_map(|m| m["name"].as_str()).filter(|n| is_mouse(n)).map(String::from).collect();
                let pads: Vec<String> = mice.iter().filter_map(|m| m["name"].as_str()).filter(|n| is_pad(n)).map(String::from).collect();
                if d.is_object() && (names != self.mice || pads != self.touchpads) {
                    self.mice = names;
                    self.touchpads = pads;
                    self.publish();
                    self.apply_gen += 1;
                    after(h.sender("Mouse"), 150, M::ApplyLater(self.apply_gen));
                }
            }
            M::Plugin(n, v) => {
                self.plugin_name = n;
                self.plugin_version = v;
                self.publish();
                self.apply_boundaries(h);
            }
            M::UdevLine => {
                self.look_gen += 1;
                let g = self.look_gen;
                let _ = g;
                after(h.sender("Mouse"), 800, M::LookAgain);
            }
            M::LookAgain => self.look(h),
            M::UdevEnded => {}
            M::Poll => {
                self.look(h);
                after(h.sender("Mouse"), 60000, M::Poll);
            }
            M::ApplyLater(g) => {
                if g == self.apply_gen {
                    self.apply(h);
                }
            }
            M::FocusLater(g) => {
                if g != self.focus_gen {
                    return;
                }
                let act = after_events(&self.pending);
                self.pending.clear();
                if act == Act::None || h.cfg().is_null() || !Self::hover(h) || Self::locked(h) || !h.state("open").as_str().unwrap_or("").is_empty() || !Self::selecting().is_empty() {
                    return;
                }
                if act == Act::Refocus && self.boundary_available() {
                    self.plugin(&["pointer-refocus"]);
                    return;
                }
                let addr = if let Act::Warp(a) = &act { a.clone() } else { String::new() };
                let script = serde_json::to_string(&crate::util::script("pointer-focus.lua")).unwrap_or_default();
                hyprctl_eval(&format!("local focus = dofile({script}); focus({})", serde_json::to_string(&addr).unwrap_or_default()));
            }
            M::UrgentSettle(g) => {
                if g != self.urgent_gen || self.urgent_window.is_empty() {
                    return;
                }
                let address = std::mem::take(&mut self.urgent_window);
                let follow = self.follow_available();
                let name = self.plugin_name.clone();
                let send = h.sender("Mouse");
                std::thread::spawn(move || {
                    let age = if follow {
                        crate::util::capture(&["hyprctl", &name, "input-age"], std::time::Duration::from_secs(3)).and_then(|(_, o)| o.trim().parse::<i64>().ok())
                    } else {
                        None
                    };
                    let clients = hyprctl_json(&["clients"]);
                    let cl = clients.as_array().and_then(|a| a.iter().find(|c| c["address"] == address.as_str()).cloned());
                    let active = hyprctl_json(&["activewindow"])["address"] == address.as_str();
                    send(Box::new(M::UrgentInfo {
                        found: cl.is_some(),
                        active,
                        class: cl.as_ref().and_then(|c| c["class"].as_str()).unwrap_or("").into(),
                        title: cl.as_ref().and_then(|c| c["title"].as_str()).unwrap_or("").into(),
                        address,
                        age,
                    }));
                });
            }
            M::UrgentInfo { address, found, active, class, title, age } => self.urgent_decided(h, address, found, active, class, title, age),
            M::Lease(g) => {
                if g == self.lease_gen {
                    self.confine_selection(h);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(n: &str, d: &str) -> Raw {
        Raw { name: n.into(), data: d.into() }
    }

    #[test]
    fn focus_decisions() {
        assert_eq!(after_events(&[]), Act::None);
        assert_eq!(after_events(&[ev("workspacev2", "1,1")]), Act::Refocus);
        assert_eq!(after_events(&[ev("workspacev2", "1,1"), ev("openwindow", "55aa,1,kitty,kitty")]), Act::Warp("0x55aa".into()));
        assert_eq!(after_events(&[ev("openwindow", "zz,1,k,k")]), Act::None);
        assert_eq!(on_urgent(Some(500), false), "bring");
        assert_eq!(on_urgent(Some(5000), false), "notify");
        assert_eq!(on_urgent(None, false), "notify");
        assert_eq!(on_urgent(Some(1), true), "none");
    }

    #[test]
    fn device_lua() {
        assert!(is_mouse("logitech-mx") && !is_mouse("Synaptics Touchpad") && !is_mouse("ydotoold virtual device"));
        let s = device_spec("m\"x", &json!({"speed": 3, "accelProfile": "flat", "scrollSpeed": 9}));
        assert_eq!(s, "hl.device({ name = \"m\\\"x\", accel_profile = \"flat\", sensitivity = 1.00, natural_scroll = false, scroll_factor = 3.00, left_handed = false, middle_button_emulation = false })");
        assert!(touchpad_spec(&json!({"rightClick": "corner"})).contains("clickfinger_behavior = false"));
        assert!(touchpad_spec(&json!({})).contains("tap_to_click = true, tap_and_drag = true, drag_lock = 0, natural_scroll = true, scroll_factor = 0.70"));
    }
}

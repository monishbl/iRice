//! Cool-down and what the glass follows that only the shell knows:
//! shell/common/CoolDown.qml, and RustUi.qml's `glassLine`
//! (`{"src":"shell","kind":"glass","tone":"#rrggbb","cooldown":"keep"}`: the
//! wallpaper's tone and Cool-down's cheaper glass; said at the start and on
//! each change). `qs ipc call cooldown toggle | on | off`.
//!
//!   cooldown.hot [bool]       the island's temperature watch (island.rs)
//!   cooldown.battery [bool]   on battery (the power service)
//!   cooldown.answer [bool]    the "Running hot" alert's Cool down / Not now
//!   cooldown.toggle           the Control Center control
//! State: "cooldown" {active, why, glass, motionScale, pauseLive}.
//!
//! While it is on, as set: the glass turns frosted or solid, Hyprland's blur
//! gets lighter (fewer, smaller passes), animations get shorter (Theme.motion),
//! a live wallpaper pauses, and (if asked) the laptop's own panel drops to its
//! lowest refresh rate. All of it comes back when cool-down ends, also after a
//! restart in the middle (what it changed is kept in state.json's coolDown).

use crate::common::*;
use serde_json::{json, Value};
use std::any::Any;
use tessera_core::host::{Host, Service};

enum Msg {
    Tone(String, Option<String>),
    /// the current blur, read: (passes, size)
    Blur(i64, i64),
    /// hyprctl -j monitors
    Monitors(Value),
    #[allow(dead_code)]
    Reapply,
}

#[derive(Default)]
pub struct Appearance {
    cool: Cool,
    tone: Option<String>,
    tone_for: String,
    sent: String,
    last_active: bool,
    /// the blur the config had, to put back
    saved_blur: Option<(i64, i64)>,
    reading_blur: bool,
    /// output -> the refresh rate before cool-down
    saved_rates: serde_json::Map<String, Value>,
}

/// The (width, height, rate) modes a monitor lists: "2560x1600@165.00Hz"
pub fn parse_mode(m: &str) -> Option<(i64, i64, f64)> {
    let (wh, rest) = m.split_once('@')?;
    let (w, h) = wh.split_once('x')?;
    Some((w.parse().ok()?, h.parse().ok()?, rest.strip_suffix("Hz")?.parse().ok()?))
}

/// The lowest rate (40 Hz or more) at the monitor's own resolution, if it differs
pub fn lowest_rate(modes: &[String], width: i64, height: i64, current: f64) -> Option<f64> {
    let pick = modes.iter().filter_map(|m| parse_mode(m)).filter(|(w, h, r)| *w == width && *h == height && *r >= 40.0).map(|(_, _, r)| r).fold(f64::INFINITY, f64::min);
    (pick.is_finite() && (pick - current).abs() >= 0.5).then_some(pick)
}

/// Is this output the laptop's own panel (eDP, LVDS, DSI)
pub fn internal(name: &str) -> bool {
    ["eDP", "LVDS", "DSI"].iter().any(|p| name.starts_with(p))
}

fn hypr_eval(lua: &str) {
    run(&["hyprctl", "eval", lua]);
}

impl Appearance {
    fn cfg_cool<'a>(&self, h: &'a Host) -> &'a Value {
        h.cfg()
    }

    fn glass_line(&self, h: &Host) -> Value {
        json!({"src": "shell", "kind": "glass", "tone": self.tone.clone().unwrap_or_default(), "cooldown": self.cool.glass(self.cfg_cool(h))})
    }

    /// Say the glass line when it changed
    fn say_glass(&mut self, h: &mut Host) {
        let line = self.glass_line(h);
        let text = line.to_string();
        if text != self.sent {
            self.sent = text;
            h.say(line);
        }
    }

    fn publish(&self, h: &mut Host) {
        let c = self.cfg_cool(h);
        let active = self.cool.active(c);
        let why = if !active {
            if self.cool.snoozed || self.cool.declined { "Off until it cools or you plug in" } else { "" }
        } else if self.cool.manual {
            "Turned on"
        } else if self.cool.hot_on(c) {
            "The CPU is hot"
        } else {
            "On battery"
        };
        let pause = active && cfg_bool(c, "cooling.pauseLiveWallpaper", true);
        let state = json!({"active": active, "why": why, "glass": self.cool.glass(c), "motionScale": self.cool.motion_scale(c), "pauseLive": pause, "hot": self.cool.hot});
        h.set_state("cooldown", state);
        // what the Settings pages and the lock read of the wallpaper and the look
        h.set_state("wallpaper.tone", json!(self.tone.clone().unwrap_or_default()));
        h.set_state("wallpaper.path", json!(Self::picture(h)));
        h.set_state("dark", json!(dark(c)));
    }

    /// The wallpaper showing now (for its tone)
    fn picture(h: &Host) -> String {
        tessera_core::wallpaper::showing_now(h.cfg(), material(h.cfg()))
    }

    fn read_tone(&mut self, h: &mut Host) {
        let pic = Self::picture(h);
        if pic == self.tone_for || pic.is_empty() {
            return;
        }
        self.tone_for = pic.clone();
        let send = h.sender("Appearance");
        std::thread::spawn(move || {
            let tone = wallpaper_tone(&pic);
            send(Box::new(Msg::Tone(pic, tone)));
        });
    }

    /// Something changed: say it, and do what Cool-down changes
    fn changed(&mut self, h: &mut Host) {
        let active = self.cool.active(h.cfg());
        if !active && self.last_active {
            // coming back
            self.restore(h);
        }
        if active != self.last_active || self.light_blur(h) != (self.saved_blur.is_some() || self.reading_blur) {
            self.apply_blur(h);
            self.apply_rates(h);
        }
        self.last_active = active;
        self.say_glass(h);
        self.publish(h);
    }

    fn light_blur(&self, h: &Host) -> bool {
        self.cool.active(h.cfg()) && cfg_bool(h.cfg(), "cooling.lightBlur", true)
    }

    fn apply_blur(&mut self, h: &mut Host) {
        if self.light_blur(h) {
            match self.saved_blur {
                None if !self.reading_blur => {
                    self.reading_blur = true;
                    let send = h.sender("Appearance");
                    std::thread::spawn(move || {
                        let out = capture(&["sh", "-c", "hyprctl -j getoption decoration:blur:passes; hyprctl -j getoption decoration:blur:size"]).unwrap_or_default();
                        let nums: Vec<i64> = out.split("\"int\":").skip(1).filter_map(|p| p.trim().split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()).collect();
                        send(Box::new(Msg::Blur(nums.first().copied().unwrap_or(3), nums.get(1).copied().unwrap_or(8))));
                    });
                }
                Some(_) => hypr_eval("hl.config({ decoration = { blur = { passes = 1, size = 5 } } })"),
                None => {}
            }
        } else if let Some((passes, size)) = self.saved_blur.take() {
            hypr_eval(&format!("hl.config({{ decoration = {{ blur = {{ passes = {passes}, size = {size} }} }} }})"));
            self.remember();
        }
    }

    fn remember(&self) {
        let v = if self.saved_blur.is_none() && self.saved_rates.is_empty() { Value::Null } else { json!({"blur": self.saved_blur.map(|(p, s)| json!({"passes": p, "size": s})), "rates": self.saved_rates}) };
        ui_set("coolDown", v);
    }

    /// The laptop's own panel at its lowest rate while cool-down is on (only if asked for)
    fn apply_rates(&mut self, h: &mut Host) {
        let low = self.cool.active(h.cfg()) && cfg_bool(h.cfg(), "cooling.refreshDrop", false);
        if !low && self.saved_rates.is_empty() {
            return;
        }
        let send = h.sender("Appearance");
        std::thread::spawn(move || {
            if let Some(out) = capture(&["hyprctl", "-j", "monitors"]) {
                if let Ok(v) = serde_json::from_str::<Value>(&out) {
                    send(Box::new(Msg::Monitors(v)));
                }
            }
        });
    }

    fn monitors(&mut self, h: &mut Host, mons: Value) {
        let low = self.cool.active(h.cfg()) && cfg_bool(h.cfg(), "cooling.refreshDrop", false);
        let set_rate = |m: &Value, rate: f64| {
            let mut lua = format!("hl.monitor({{ output = \"{}\", mode = \"{}x{}@{:.2}\", position = \"{}x{}\", scale = {}", m["name"].as_str().unwrap_or(""), m["width"], m["height"], rate, m["x"], m["y"], m["scale"]);
            if m["transform"].as_i64().unwrap_or(0) != 0 {
                lua.push_str(&format!(", transform = {}", m["transform"]));
            }
            lua.push_str(" })");
            hypr_eval(&lua);
        };
        let mut saved = self.saved_rates.clone();
        for m in mons.as_array().cloned().unwrap_or_default() {
            let name = m["name"].as_str().unwrap_or("").to_string();
            if !internal(&name) || m["mirrorOf"].as_str().is_some_and(|x| x != "none") {
                continue;
            }
            let current = m["refreshRate"].as_f64().unwrap_or(0.0);
            if !low {
                // Only put back what cool-down changed
                if let Some(was) = saved.get(&name).and_then(Value::as_f64) {
                    if (was - current).abs() >= 0.5 {
                        set_rate(&m, was);
                    }
                }
                saved.remove(&name);
                continue;
            }
            let modes: Vec<String> = m["availableModes"].as_array().map(|l| l.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
            let Some(pick) = lowest_rate(&modes, m["width"].as_i64().unwrap_or(0), m["height"].as_i64().unwrap_or(0), current) else { continue };
            saved.entry(name).or_insert(json!(current));
            set_rate(&m, pick);
        }
        self.saved_rates = saved;
        self.remember();
    }

    fn restore(&mut self, h: &mut Host) {
        let _ = h;
    }
}

impl Service for Appearance {
    fn name(&self) -> &'static str {
        "Appearance"
    }
    fn calls(&self) -> &'static [&'static str] {
        &["cooldown"]
    }
    fn ipc_targets(&self) -> &'static [&'static str] {
        &["cooldown"]
    }
    fn start(&mut self, h: &mut Host) {
        // what a restart in the middle of cool-down left (to put back)
        let s = ui_get("coolDown");
        if s.is_object() {
            if let (Some(p), Some(z)) = (s["blur"]["passes"].as_i64(), s["blur"]["size"].as_i64()) {
                self.saved_blur = Some((p, z));
            }
            if let Some(r) = s["rates"].as_object() {
                self.saved_rates = r.clone();
            }
        }
        h.st.shell.want("status");
        self.read_tone(h);
        self.changed(h);
        // (once: puts back what a restart left, if cool-down has ended meanwhile)
        self.apply_blur(h);
        self.apply_rates(h);
    }
    fn config_changed(&mut self, h: &mut Host) {
        self.read_tone(h);
        self.changed(h);
    }
    fn call(&mut self, h: &mut Host, call: &str, args: &Value) {
        match call.strip_prefix("cooldown.").unwrap_or("") {
            "hot" => {
                self.cool.hot = args[0].as_bool().unwrap_or(false);
                if !self.cool.hot {
                    self.cool.accepted = false;
                    self.cool.declined = false;
                }
            }
            "battery" => {
                self.cool.on_battery = args[0].as_bool().unwrap_or(false);
                if !self.cool.automatic(h.cfg()) {
                    self.cool.snoozed = false;
                }
            }
            "answer" => {
                if args[0].as_bool().unwrap_or(false) {
                    self.cool.accepted = true;
                    // Low Power comes with the yes
                    h.ask("power.ask", json!(["cooldown", "power-saver", "cooling down"]));
                } else {
                    self.cool.declined = true;
                }
                h.ask("island.hotAnswered", json!([]));
            }
            "toggle" => {
                let c = h.cfg().clone();
                self.cool.toggle(&c);
            }
            other => eprintln!("tessera-ui: host: no cooldown.{other}"),
        }
        self.changed(h);
    }
    fn ipc(&mut self, h: &mut Host, _target: &str, func: &str, _args: &[String]) -> Option<String> {
        match func {
            "toggle" => {
                let c = h.cfg().clone();
                self.cool.toggle(&c);
            }
            "on" => {
                self.cool.snoozed = false;
                self.cool.manual = true;
            }
            "off" => {
                self.cool.manual = false;
                if self.cool.automatic(h.cfg()) {
                    self.cool.snoozed = true;
                }
            }
            _ => {}
        }
        self.changed(h);
        None
    }
    fn message(&mut self, h: &mut Host, msg: Box<dyn Any + Send>) {
        let Ok(m) = msg.downcast::<Msg>() else { return };
        match *m {
            Msg::Tone(pic, tone) => {
                if pic == self.tone_for {
                    self.tone = tone;
                    self.say_glass(h);
                }
            }
            Msg::Blur(passes, size) => {
                self.reading_blur = false;
                // Cool-down may have ended while this was reading: then nothing was changed
                if !self.light_blur(h) {
                    return;
                }
                self.saved_blur = Some((passes, size));
                self.remember();
                self.apply_blur(h);
            }
            Msg::Monitors(v) => self.monitors(h, v),
            Msg::Reapply => {
                self.saved_blur = None;
                self.apply_blur(h);
                self.apply_rates(h);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_parse() {
        assert_eq!(parse_mode("2560x1600@165.00Hz"), Some((2560, 1600, 165.0)));
        assert_eq!(parse_mode("1920x1080"), None);
    }

    #[test]
    fn the_lowest_rate_at_the_same_resolution() {
        let modes: Vec<String> = ["2560x1600@165.00Hz", "2560x1600@48.00Hz", "2560x1600@30.00Hz", "1920x1080@40.00Hz"].iter().map(|s| s.to_string()).collect();
        assert_eq!(lowest_rate(&modes, 2560, 1600, 165.0), Some(48.0));
        // already there
        assert_eq!(lowest_rate(&modes, 2560, 1600, 48.0), None);
        // no mode of 40 Hz or more at that size
        assert_eq!(lowest_rate(&modes, 1280, 800, 60.0), None);
    }

    #[test]
    fn only_the_laptop_panel() {
        assert!(internal("eDP-1") && internal("LVDS-1") && internal("DSI-1"));
        assert!(!internal("DP-1") && !internal("HDMI-A-1"));
    }
}

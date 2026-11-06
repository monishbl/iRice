//! Timers.qml (Pomodoro focus/break cycles and a stopwatch) and Alarms.qml
//! (countdown timers and alarms: "timer 10 min", "alarm 7am"). They keep
//! running with the panels closed. A phase change posts a notification when
//! the island doesn't take it; an alarm plays the alarm sound, notifies and
//! posts an island alert. Alarms live in state.json's `alarms`
//! ([{ id, at (ms), label, kind: "timer" | "alarm" }]).
//!
//! `qs ipc call timer toggle | skip | reset | stopwatch | stopwatchReset`
//! is the "timer" ipc target. Calls: alarms.addTimer [seconds, label],
//! alarms.addAlarm [at ms, label], alarms.cancel [id].
//! The state it keeps for the island (its live activities read Timers) is
//! said as the shell's state fields `timers` and `alarms`.

use super::common::*;
use serde_json::{json, Value};
use tessera_core::host::Host;
use tessera_core::services::notify::model as nm;

pub struct Timers {
    pub focus_minutes: i64,
    pub break_minutes: i64,
    pub long_break_minutes: i64,
    pub phase: &'static str,
    pub running: bool,
    pub remaining: i64,
    pub deadline: i64,
    pub completed: i64,
    pub sw_running: bool,
    pub sw_started: i64,
    pub sw_accum: i64,
    pub laps: Vec<i64>,
    /// views showing hundredths (Notification Center's stopwatch)
    pub sw_fine: i32,
}

impl Default for Timers {
    fn default() -> Self {
        Self { focus_minutes: 25, break_minutes: 5, long_break_minutes: 15, phase: "focus", running: false, remaining: 25 * 60, deadline: 0, completed: 0, sw_running: false, sw_started: 0, sw_accum: 0, laps: vec![], sw_fine: 0 }
    }
}

fn island_takes(h: &Host) -> bool {
    nm::island_on(h.cfg()) && nm::island_wants(h.cfg(), "timers")
}

impl Timers {
    pub fn phase_length(&self) -> i64 {
        (match self.phase {
            "focus" => self.focus_minutes,
            "break" => self.break_minutes,
            _ => self.long_break_minutes,
        }) * 60
    }
    pub fn progress(&self) -> f64 {
        1.0 - self.remaining as f64 / self.phase_length().max(1) as f64
    }
    pub fn sw_elapsed(&self, now: i64) -> i64 {
        self.sw_accum + if self.sw_running { now - self.sw_started } else { 0 }
    }

    pub fn toggle(&mut self, now: i64) {
        self.running = !self.running;
        // (onRunningChanged)
        if self.running {
            self.deadline = now + self.remaining * 1000;
        } else if self.deadline > 0 {
            self.remaining = ((self.deadline - now) as f64 / 1000.0).ceil().max(0.0) as i64;
        }
    }
    pub fn reset(&mut self, now: i64) {
        if self.running {
            self.toggle(now);
        }
        self.phase = "focus";
        self.remaining = self.focus_minutes * 60;
    }
    /// A phase ended (by time or Skip; not by Reset)
    pub fn advance(&mut self, h: &mut Host, now: i64) {
        let island = island_takes(h);
        if self.phase == "focus" {
            self.completed += 1;
            self.phase = if self.completed % 4 == 0 { "longBreak" } else { "break" };
            if !island {
                let mins = if self.phase == "longBreak" { self.long_break_minutes } else { self.break_minutes };
                notify_send("Focus", None, None, "Time for a break", &format!("{mins} minutes. Stand up and stretch."));
            }
        } else {
            self.phase = "focus";
            if !island {
                notify_send("Focus", None, None, "Break's over", &format!("{} minutes of focus starting now.", self.focus_minutes));
            }
        }
        self.remaining = self.phase_length();
        self.deadline = now + self.remaining * 1000;
        self.publish(h, now);
    }
    pub fn sw_toggle(&mut self, now: i64) {
        if self.sw_running {
            self.sw_accum += now - self.sw_started;
            self.sw_running = false;
        } else {
            self.sw_started = now;
            self.sw_running = true;
        }
    }
    pub fn sw_lap(&mut self, now: i64) {
        let e = self.sw_elapsed(now);
        self.laps.insert(0, e);
        self.laps.truncate(20);
    }
    pub fn sw_reset(&mut self) {
        self.sw_running = false;
        self.sw_accum = 0;
        self.laps.clear();
    }

    /// The 1 s tick
    pub fn tick(&mut self, h: &mut Host, now: i64) {
        if self.running {
            self.remaining = ((self.deadline - now) as f64 / 1000.0).ceil().max(0.0) as i64;
            if self.remaining <= 0 {
                self.advance(h, now);
            }
        }
        self.publish(h, now);
    }

    /// What the island's activities read (Timers.running, remaining…)
    pub fn publish(&self, h: &mut Host, now: i64) {
        h.set_state(
            "timers",
            json!({
                "running": self.running, "phase": self.phase, "remaining": self.remaining, "phaseLength": self.phase_length(),
                "completed": self.completed,
                "started": self.running || (self.remaining > 0 && self.remaining < self.phase_length()),
                "sw": {"running": self.sw_running, "accum": self.sw_accum, "elapsed": self.sw_elapsed(now), "started": self.sw_running || self.sw_accum > 0},
            }),
        );
    }

    /// `qs ipc call timer <fn>`
    pub fn ipc(&mut self, h: &mut Host, func: &str, now: i64) -> Option<String> {
        match func {
            "toggle" => self.toggle(now),
            "skip" => self.advance(h, now),
            "reset" => self.reset(now),
            "stopwatch" => self.sw_toggle(now),
            "stopwatchReset" => self.sw_reset(),
            _ => return None,
        }
        self.publish(h, now);
        None
    }
}

/// Timers.format: "24:59"
pub fn format(seconds: i64) -> String {
    let s = seconds.max(0);
    format!("{}:{:02}", s / 60, s % 60)
}

/// Timers.formatMs: "1:02.35"
pub fn format_ms(ms: i64) -> String {
    let cs = ms.max(0) / 10;
    format!("{}:{:02}.{:02}", cs / 6000, cs / 100 % 60, cs % 100)
}

// ── Alarms ──────────────────────────────────────────────────────────────

pub fn alarms() -> Vec<Value> {
    let mut l = state_get("alarms").as_array().cloned().unwrap_or_default();
    l.sort_by_key(|a| a["at"].as_f64().unwrap_or(0.0) as i64);
    l
}

fn add_alarm(kind: &str, at: i64, label: &str) -> Value {
    let e = json!({"id": new_id(), "at": at, "label": label, "kind": kind});
    let mut l = state_get("alarms").as_array().cloned().unwrap_or_default();
    l.push(e.clone());
    state_set("alarms", Value::Array(l));
    e
}

pub fn add_timer(seconds: i64, label: &str) -> Value {
    add_alarm("timer", now() + seconds * 1000, label)
}

pub fn cancel(id: &str) {
    let l: Vec<Value> = state_get("alarms").as_array().cloned().unwrap_or_default().into_iter().filter(|a| a["id"].as_str() != Some(id)).collect();
    state_set("alarms", Value::Array(l));
}

/// The 1 s tick: what's due goes off (the QML waits for the exact moment;
/// half a second early is as close as a second's tick gets)
pub fn check(h: &mut Host) {
    let now = now();
    let l = alarms();
    let at = |a: &Value| a["at"].as_f64().unwrap_or(0.0) as i64;
    let due: Vec<Value> = l.iter().filter(|a| at(a) <= now + 500).cloned().collect();
    if due.is_empty() {
        return;
    }
    state_set("alarms", Value::Array(l.into_iter().filter(|a| at(a) > now + 500).collect()));
    for a in due {
        let timer = a["kind"] == "timer";
        let title = if timer { "Timer done" } else { "Alarm" };
        let label = a["label"].as_str().unwrap_or("");
        let body = if !label.is_empty() {
            label.to_string()
        } else if timer {
            "Your timer has finished.".to_string()
        } else {
            hm(at(&a))
        };
        // (even with interface sounds off: it's an alarm)
        detached(&["sh", "-c", "f=/usr/share/sounds/freedesktop/stereo/alarm-clock-elapsed.oga; pw-play \"$f\" 2>/dev/null || paplay \"$f\""]);
        notify_send("Clock", Some("alarm-clock"), Some("critical"), title, &body);
        let color = accent(h, "orange");
        island_post(h, json!({"kind": "custom", "important": true, "icon": "alarm", "title": title, "subtitle": body, "color": color}), 3000);
    }
}

/// The next one, for the island and the menu bar
pub fn publish_alarms(h: &mut Host) {
    let l = alarms();
    h.set_state("alarms", json!({"next": l.first().cloned().unwrap_or(Value::Null), "count": l.len()}));
}

/// Calls: alarms.addTimer, alarms.addAlarm, alarms.cancel
pub fn call(call: &str, args: &Value) {
    let a = args_of(args);
    match call {
        "alarms.addTimer" => {
            let s = a.first().and_then(Value::as_f64).unwrap_or(0.0) as i64;
            if s > 0 {
                add_timer(s, a.get(1).and_then(Value::as_str).unwrap_or(""));
            }
        }
        "alarms.addAlarm" => {
            let at = a.first().and_then(Value::as_f64).unwrap_or(0.0) as i64;
            if at > 0 {
                add_alarm("alarm", at, a.get(1).and_then(Value::as_str).unwrap_or(""));
            }
        }
        "alarms.cancel" => cancel(a.first().and_then(Value::as_str).unwrap_or("")),
        other => eprintln!("tessera-ui: host: no alarms call {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clocks_read_as_the_qml() {
        assert_eq!(format(1499), "24:59");
        assert_eq!(format(-3), "0:00");
        assert_eq!(format_ms(62_350), "1:02.35");
    }

    #[test]
    fn pomodoro_cycles() {
        let mut t = Timers::default();
        assert_eq!(t.phase_length(), 1500);
        t.toggle(1000);
        assert!(t.running && t.deadline == 1000 + 1_500_000);
        t.toggle(61_000);
        assert_eq!(t.remaining, 1440);
        t.sw_toggle(0);
        t.sw_toggle(2500);
        assert_eq!(t.sw_elapsed(9999), 2500);
        t.sw_lap(0);
        assert_eq!(t.laps, vec![2500]);
    }
}

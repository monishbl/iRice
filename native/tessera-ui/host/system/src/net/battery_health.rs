//! The laptop battery's health and power draw (replaces services/BatteryHealth.qml),
//! read from the kernel (/sys/class/power_supply/BAT*), which knows more than
//! UPower reports: maximum capacity (full charge vs the design's), cycle
//! count, and the watts going in or out. Read when asked (the battery menu
//! opening, a hover), never on a timer of its own.
//!
//! Calls (prefix "batteryhealth"): refresh. State on the bus as
//! "batteryhealth": {capacity (0..1, -1 unknown), cycles (-1 unknown),
//! watts (-1 unknown), condition}. `summary` is the hover line.

use super::common::*;
use serde_json::{json, Value};
use std::any::Any;
use std::collections::BTreeMap;
use tessera_core::host::{Host, Service};
use tessera_core::services::status::Battery;

const NAME: &str = "BatteryHealth";

const READER: &str = "for b in /sys/class/power_supply/BAT*; do [ -d \"$b\" ] || continue; for f in energy_full energy_full_design charge_full charge_full_design cycle_count power_now current_now voltage_now; do [ -r \"$b/$f\" ] && echo \"$f=$(cat \"$b/$f\")\"; done; break; done";

/// (capacity, cycles, watts) from the reader's `name=number` lines
pub fn parse(text: &str) -> (f64, i64, f64) {
    let v: BTreeMap<&str, f64> = text.lines().filter_map(|l| {
        let (k, n) = l.split_once('=')?;
        Some((k, n.parse().ok()?))
    }).collect();
    let full = v.get("energy_full").or(v.get("charge_full")).copied();
    let design = v.get("energy_full_design").or(v.get("charge_full_design")).copied();
    let capacity = match (full, design) {
        (Some(f), Some(d)) if f > 0.0 && d > 0.0 => (f / d).min(1.0),
        _ => -1.0,
    };
    let cycles = v.get("cycle_count").filter(|c| **c > 0.0).map_or(-1, |c| *c as i64);
    // power_now in µW, or current (µA) × voltage (µV)
    let watts = match (v.get("power_now"), v.get("current_now"), v.get("voltage_now")) {
        (Some(p), _, _) => p / 1e6,
        (None, Some(c), Some(vo)) => c * vo / 1e12,
        _ => -1.0,
    };
    (capacity, cycles, watts)
}

pub fn condition(capacity: f64) -> &'static str {
    if capacity < 0.0 { "" } else if capacity >= 0.8 { "Normal" } else { "Service recommended" }
}

/// "1:05" for seconds
pub fn hm(secs: i64) -> String {
    let m = (secs as f64 / 60.0).round() as i64;
    format!("{}:{:02}", m / 60, m % 60)
}

/// One line for a hover: "84% · Charging · Balanced", "84% · 3:20 left · Low Power"
pub fn summary(b: &Battery, mode: Option<&str>) -> String {
    let level = if b.percent > 1.0 { b.percent / 100.0 } else { b.percent };
    let state = match b.state {
        4 => "Fully charged".to_string(),
        1 => if b.time_to_full > 0 { format!("Full in {}", hm(b.time_to_full)) } else { "Charging".into() },
        5 => "Not charging".into(),
        _ => if b.time_to_empty > 0 { format!("{} left", hm(b.time_to_empty)) } else { "On battery".into() },
    };
    format!("{}% · {}{}", (level * 100.0).round(), state, mode.map(|m| format!(" · {}", super::power::mode_name(m))).unwrap_or_default())
}

enum M {
    Read(String),
}

#[derive(Default)]
pub struct BatteryHealth {
    busy: bool,
}

impl Service for BatteryHealth {
    fn name(&self) -> &'static str {
        NAME
    }
    fn calls(&self) -> &'static [&'static str] {
        &["batteryhealth"]
    }
    fn start(&mut self, _h: &mut Host) {
        crate::bus::put(NAME, json!({"capacity": -1, "cycles": -1, "watts": -1, "condition": ""}));
    }
    fn call(&mut self, h: &mut Host, call: &str, _args: &Value) {
        if call.ends_with(".refresh") && !self.busy {
            self.busy = true;
            let send = h.sender(NAME);
            capture_then(sh_cmd(READER, &[]), None, move |_, t| send(Box::new(M::Read(t))));
        }
    }
    fn message(&mut self, _h: &mut Host, msg: Box<dyn Any + Send>) {
        let Ok(m) = msg.downcast::<M>() else { return };
        let M::Read(t) = *m;
        self.busy = false;
        let (capacity, cycles, watts) = parse(&t);
        crate::bus::put(NAME, json!({"capacity": capacity, "cycles": cycles, "watts": watts, "condition": condition(capacity)}));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_from_the_kernel() {
        let (c, y, w) = parse("energy_full=40000000\nenergy_full_design=50000000\ncycle_count=212\npower_now=7500000\n");
        assert_eq!((c, y, w), (0.8, 212, 7.5));
        assert_eq!(condition(c), "Normal");
        let (c, y, w) = parse("charge_full=3000\ncharge_full_design=4000\ncycle_count=0\ncurrent_now=1000000\nvoltage_now=12000000\n");
        assert_eq!((c, y), (0.75, -1));
        assert_eq!(w, 12.0);
        assert_eq!(condition(c), "Service recommended");
        assert_eq!(parse(""), (-1.0, -1, -1.0));
        assert_eq!(hm(12000), "3:20");
    }

    #[test]
    fn the_hover_line() {
        let b = Battery { present: true, percent: 84.0, charging: false, state: 2, time_to_empty: 12000, time_to_full: 0 };
        assert_eq!(summary(&b, Some("power-saver")), "84% · 3:20 left · Low Power");
        assert_eq!(summary(&Battery { state: 1, ..b }, None), "84% · Charging");
    }
}

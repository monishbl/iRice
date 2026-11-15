//! The laptop battery's health and power draw (replaces services/BatteryHealth.qml),
//! read from the kernel (/sys/class/power_supply/BAT*), which knows more than
//! UPower reports: maximum capacity (full charge vs the design's), cycle
//! count, and the watts going in or out. Read when asked (the battery menu
//! opening, a hover), never on a timer of its own.
//!
//! Calls (prefix "batteryhealth"): refresh. State on the bus as
//! "batteryhealth": {capacity (0..1, -1 unknown), cycles (-1 unknown),
//! watts (-1 unknown), condition}. `summary` is the hover line.

use serde_json::{json, Value};
use std::any::Any;
use std::collections::BTreeMap;
use tessera_core::host::{Host, Service};
use tessera_core::services::status::Battery;

const NAME: &str = "BatteryHealth";

const FIELDS: [&str; 8] = ["energy_full", "energy_full_design", "charge_full", "charge_full_design", "cycle_count", "power_now", "current_now", "voltage_now"];

/// The computer's own batteries' readings under `root` (the power supply
/// class, or a test's): `name=number` lines, a "--" line between batteries.
/// Any name (BAT0, BAT1, CMB0…), never a mouse's or headset's battery.
pub fn reader_text(root: &std::path::Path) -> String {
    let mut out = String::new();
    for b in tessera_core::power_supply::read(root).iter().filter(|s| s.system_battery()) {
        if !out.is_empty() {
            out.push_str("--\n");
        }
        for f in FIELDS {
            if let Ok(v) = std::fs::read_to_string(b.path.join(f)) {
                out.push_str(&format!("{f}={}\n", v.trim()));
            }
        }
    }
    out
}

/// One battery's (full, design, cycles, watts) from its lines
fn one(text: &str) -> (Option<f64>, Option<f64>, i64, f64) {
    let v: BTreeMap<&str, f64> = text.lines().filter_map(|l| {
        let (k, n) = l.split_once('=')?;
        Some((k, n.parse().ok()?))
    }).collect();
    let energy = v.contains_key("energy_full") && v.contains_key("energy_full_design");
    let full = if energy { v.get("energy_full") } else { v.get("charge_full") }.copied().filter(|f| *f > 0.0);
    let design = if energy { v.get("energy_full_design") } else { v.get("charge_full_design") }.copied().filter(|d| *d > 0.0);
    let cycles = v.get("cycle_count").filter(|c| **c > 0.0).map_or(-1, |c| *c as i64);
    // power_now in µW, or current (µA) × voltage (µV); signed on some
    // drivers (negative: discharging)
    let watts = match (v.get("power_now"), v.get("current_now"), v.get("voltage_now")) {
        (Some(p), _, _) => p.abs() / 1e6,
        (None, Some(c), Some(vo)) => (c * vo).abs() / 1e12,
        _ => -1.0,
    };
    (full, design, cycles, watts)
}

/// (capacity, cycles, watts) from the reader's lines: over all batteries
/// (two in a ThinkPad), capacity as what they hold together against their
/// design, the most cycles, the watts summed
pub fn parse(text: &str) -> (f64, i64, f64) {
    let bats: Vec<_> = text.split("--\n").filter(|b| !b.trim().is_empty()).map(one).collect();
    let both: Vec<(f64, f64)> = bats.iter().filter_map(|(f, d, _, _)| Some(((*f)?, (*d)?))).collect();
    let capacity = if both.is_empty() { -1.0 } else { (both.iter().map(|b| b.0).sum::<f64>() / both.iter().map(|b| b.1).sum::<f64>()).min(1.0) };
    let cycles = bats.iter().map(|b| b.2).max().unwrap_or(-1);
    let known: Vec<f64> = bats.iter().map(|b| b.3).filter(|w| *w >= 0.0).collect();
    let watts = if known.is_empty() { -1.0 } else { known.iter().sum() };
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
            std::thread::spawn(move || send(Box::new(M::Read(reader_text(std::path::Path::new(tessera_core::power_supply::CLASS))))));
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
    fn every_own_battery_counts_and_nothing_else() {
        let root = std::env::temp_dir().join(format!("tessera-bh-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let put = |n: &str, files: &[(&str, &str)]| {
            std::fs::create_dir_all(root.join(n)).unwrap();
            for (f, v) in files {
                std::fs::write(root.join(n).join(f), format!("{v}\n")).unwrap();
            }
        };
        // a desktop: nothing
        std::fs::create_dir_all(&root).unwrap();
        assert_eq!(reader_text(&root), "");
        assert_eq!(parse(&reader_text(&root)), (-1.0, -1, -1.0));
        // a ThinkPad's two (not named BAT* only), an adapter, a mouse's battery
        put("AC", &[("type", "Mains"), ("online", "0")]);
        put("BAT0", &[("type", "Battery"), ("energy_full", "18000000"), ("energy_full_design", "24000000"), ("cycle_count", "300"), ("power_now", "4000000")]);
        put("CMB1", &[("type", "Battery"), ("energy_full", "22000000"), ("energy_full_design", "24000000"), ("cycle_count", "120"), ("power_now", "-2000000")]);
        put("hidpp_battery_0", &[("type", "Battery"), ("scope", "Device"), ("energy_full", "1"), ("energy_full_design", "100"), ("cycle_count", "9999")]);
        let (c, y, w) = parse(&reader_text(&root));
        assert!((c - 40.0 / 48.0).abs() < 1e-9);
        assert_eq!((y, w), (300, 6.0));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_hover_line() {
        let b = Battery { present: true, percent: 84.0, charging: false, state: 2, time_to_empty: 12000, time_to_full: 0 };
        assert_eq!(summary(&b, Some("power-saver")), "84% · 3:20 left · Low Power");
        assert_eq!(summary(&Battery { state: 1, ..b }, None), "84% · Charging");
    }
}

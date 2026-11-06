//! Display brightness (replaces services/Brightness.qml). The built-in
//! panel's backlight is core's services::brightness (sysfs + logind, what
//! brightnessctl does without a udev rule); the other half, external
//! monitors through ddcutil (DDC/CI), is here. `value`, `set()` and
//! `available` act on the display that has focus when it's an external with
//! DDC, else on the built-in backlight, so the keys, the slider and the OSD
//! follow the display you're working on.
//!
//! DDC: `ddcutil detect` takes seconds, so it runs in the background a little
//! after start and again when displays come and go; each display's value is
//! read once when it's found and when it gets focus. Writes are coalesced per
//! display (a DDC write takes about 50-200 ms); reads and writes run one at a
//! time.
//!
//! Calls (prefix "brightness"): set(v, quiet), setFor(id, v, quiet), rescan.
//! IPC `brightness rescan | status`. State on the bus as "brightness": {value,
//! available, external, displayName, displays:[{id,name,value,builtIn}],
//! ddcDisplays:[{output,bus,model}], ddcValues, ddcInstalled, ddcProblem,
//! ddcScanned, ddcScanning, backlight}.

use super::common::*;
use serde_json::{json, Map, Value};
use std::any::Any;
use std::collections::BTreeMap;
use std::time::Duration;
use tessera_core::host::{Host, Service};
use tessera_core::services::brightness::{self, Backlight};
use tessera_core::services::hypr::Snapshot;

const NAME: &str = "Brightness";

#[derive(Clone, Debug, PartialEq)]
pub struct Ddc {
    pub output: String,
    pub bus: u32,
    pub model: String,
}

/// `ddcutil detect --brief` (or the probe's "PROBLEM module|access"): the
/// problem ("", module, access) and the usable displays
pub fn parse_detect(text: &str) -> (String, Vec<Ddc>) {
    if let Some(r) = text.strip_prefix("PROBLEM ") {
        let w: String = r.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
        if !w.is_empty() {
            return (w, vec![]);
        }
    }
    let low = text.to_lowercase();
    let problem = if low.contains("permission denied") || low.contains("ddcutil requires module i2c-dev") {
        if low.contains("i2c-dev") { "module" } else { "access" }
    } else {
        ""
    };
    // Blocks start with "Display N" (usable) or "Invalid display"
    let mut blocks: Vec<Vec<&str>> = vec![vec![]];
    for l in text.lines() {
        if l.trim().is_empty() {
            blocks.push(vec![]);
        } else {
            blocks.last_mut().unwrap().push(l);
        }
    }
    let after = |b: &[&str], key: &str| -> Option<String> { b.iter().find_map(|l| l.trim_start().strip_prefix(key).map(|r| r.trim().to_string())) };
    let mut found = vec![];
    for b in blocks {
        let usable = b.iter().any(|l| {
            let t = l.trim_start();
            t.strip_prefix("Display ").is_some_and(|r| r.starts_with(|c: char| c.is_ascii_digit()))
        });
        if !usable {
            continue;
        }
        let bus = after(&b, "I2C bus:").and_then(|r| r.strip_prefix("/dev/i2c-").and_then(|n| n.split(|c: char| !c.is_ascii_digit()).next().and_then(|n| n.parse().ok())));
        let conn = after(&b, "DRM connector:").or_else(|| after(&b, "DRM_connector:")).and_then(|r| {
            let r = r.strip_prefix("card")?;
            let (_, rest) = r.split_once('-')?;
            rest.split_whitespace().next().map(str::to_string)
        });
        let (Some(bus), Some(conn)) = (bus, conn) else { continue };
        let mon = after(&b, "Monitor:").unwrap_or_default();
        let model = mon.split(':').nth(1).filter(|m| !m.trim().is_empty()).unwrap_or(&conn).trim().to_string();
        found.push(Ddc { output: conn, bus, model });
    }
    (problem.to_string(), found)
}

/// "VCP 10 C 50 100" (current, max)
pub fn parse_vcp(text: &str) -> Option<(u32, u32)> {
    let t: Vec<&str> = text.split_whitespace().collect();
    t.windows(5).find_map(|w| (w[0] == "VCP" && w[1] == "10" && w[2] == "C").then(|| Some((w[3].parse().ok()?, w[4].parse().ok()?)))?).filter(|(_, m)| *m > 0)
}

const DETECT: &str = "set -- /dev/i2c-*; [ -e \"$1\" ] || { echo 'PROBLEM module'; exit 0; }; ok=; for d; do [ -r \"$d\" ] && [ -w \"$d\" ] && ok=1; done; [ -n \"$ok\" ] || { echo 'PROBLEM access'; exit 0; }; exec ddcutil detect --brief 2>&1";

enum M {
    Tick,
    StartScan,
    Detected(String),
    Read(String, String),
    Written,
}

#[derive(Default)]
pub struct Brightness {
    ddc: Vec<Ddc>,
    values: BTreeMap<String, f64>,
    max: BTreeMap<String, u32>,
    problem: String,
    scanned: bool,
    detecting: bool,
    detect_again: bool,
    /// a DDC read or write is running (they go one at a time)
    op_busy: bool,
    read_queue: Vec<String>,
    read_at: BTreeMap<String, f64>,
    pending: BTreeMap<String, f64>,
    reading: String,
    focused: String,
    scan: slint::Timer,
    tick: slint::Timer,
    last: Value,
    installed: bool,
}

impl Brightness {
    fn target(&self) -> Option<&Ddc> {
        self.ddc.iter().find(|d| d.output == self.focused)
    }

    fn backlight(h: &Host) -> (bool, f64) {
        h.st.shell.get::<Backlight>().map(|b| (b.present, b.value)).unwrap_or((false, 0.5))
    }

    fn publish(&mut self, h: &Host) {
        let (present, bl) = Self::backlight(h);
        let t = self.target();
        let value = t.map(|d| self.values.get(&d.output).copied().unwrap_or(0.5)).unwrap_or(bl);
        let mut displays = vec![];
        if present {
            displays.push(json!({"id": "backlight", "name": "Built-in display", "value": bl, "builtIn": true}));
        }
        for d in &self.ddc {
            displays.push(json!({"id": d.output, "name": if d.model.is_empty() { &d.output } else { &d.model }, "value": self.values.get(&d.output).copied().unwrap_or(0.5), "builtIn": false}));
        }
        let vals: Map<String, Value> = self.values.iter().map(|(k, v)| (k.clone(), json!(v))).collect();
        let v = json!({
            "value": value, "available": t.is_some() || present, "external": t.is_some(), "displayName": t.map(|d| d.model.clone()).unwrap_or_default(),
            "displays": displays, "ddcDisplays": self.ddc.iter().map(|d| json!({"output": d.output, "bus": d.bus, "model": d.model})).collect::<Vec<_>>(),
            "ddcValues": vals, "ddcInstalled": self.installed, "ddcProblem": self.problem, "ddcScanned": self.scanned, "ddcScanning": self.detecting,
            "backlight": present, "target": t.map(|d| json!({"output": d.output, "bus": d.bus, "model": d.model})),
        });
        if v != self.last {
            crate::bus::put(NAME, v.clone());
            self.last = v;
        }
    }

    fn hush(h: &mut Host) {
        h.tell("osd", json!({"hush": "brightness"})); // (tessera-ui's OSD, when it draws it)
    }

    fn set_ddc(&mut self, h: &mut Host, output: &str, v: f64) {
        let x = v.clamp(0.0, 1.0);
        self.values.insert(output.into(), x);
        if tessera_core::env::sandboxed() {
            return; // (real monitors: the sandbox never writes)
        }
        self.pending.insert(output.into(), x);
        self.flush_ddc(h);
    }

    fn flush_ddc(&mut self, h: &mut Host) {
        if self.op_busy {
            return;
        }
        let Some(out) = self.pending.keys().next().cloned() else { return self.next_read(h) };
        let v = self.pending.remove(&out).unwrap_or(0.0);
        let Some(d) = self.ddc.iter().find(|d| d.output == out).cloned() else { return self.flush_ddc(h) };
        self.op_busy = true;
        let send = h.sender(NAME);
        let val = (v * self.max.get(&out).copied().unwrap_or(100) as f64).round() as i64;
        capture_then(
            vec!["ddcutil".into(), "--bus".into(), d.bus.to_string(), "--noverify".into(), "setvcp".into(), "10".into(), val.to_string()],
            None,
            move |_, _| send(Box::new(M::Written)),
        );
    }

    fn read_ddc(&mut self, h: &mut Host, output: &str) {
        if self.op_busy && self.reading == output {
            return;
        }
        if !self.read_queue.iter().any(|o| o == output) {
            self.read_queue.push(output.into());
        }
        self.next_read(h);
    }

    fn next_read(&mut self, h: &mut Host) {
        if self.op_busy || self.read_queue.is_empty() {
            return;
        }
        let out = self.read_queue.remove(0);
        let Some(d) = self.ddc.iter().find(|d| d.output == out).cloned() else { return self.next_read(h) };
        self.op_busy = true;
        self.reading = d.output.clone();
        self.read_at.insert(d.output.clone(), now_ms());
        let send = h.sender(NAME);
        let o = d.output.clone();
        capture_then(
            vec!["ddcutil".into(), "--bus".into(), d.bus.to_string(), "--brief".into(), "getvcp".into(), "10".into()],
            None,
            move |_, t| send(Box::new(M::Read(o, t))),
        );
    }

    fn rescan(&mut self, h: &mut Host) {
        if !self.installed {
            return;
        }
        if self.detecting {
            self.detect_again = true;
        } else {
            self.detecting = true;
            let send = h.sender(NAME);
            capture_then(sh_cmd(DETECT, &[]), None, move |_, t| send(Box::new(M::Detected(t))));
        }
    }

    fn schedule_scan(&mut self, h: &Host) {
        if self.installed {
            let send = h.sender(NAME);
            self.scan.start(slint::TimerMode::SingleShot, Duration::from_millis(4000), move || send(Box::new(M::StartScan)));
        }
    }

    fn set_backlight(&self, v: f64) {
        brightness::set(v);
    }
}

impl Service for Brightness {
    fn name(&self) -> &'static str {
        NAME
    }
    fn calls(&self) -> &'static [&'static str] {
        &["brightness"]
    }
    fn ipc_targets(&self) -> &'static [&'static str] {
        &["brightness"]
    }
    fn start(&mut self, h: &mut Host) {
        h.st.shell.want("brightness");
        h.st.shell.want("hypr");
        self.installed = has_tool("ddcutil");
        // A few seconds after start (not in the startup rush)
        self.schedule_scan(h);
        let send = h.sender(NAME);
        self.tick.start(slint::TimerMode::Repeated, Duration::from_millis(500), move || send(Box::new(M::Tick)));
    }
    fn outputs_changed(&mut self, h: &mut Host) {
        self.schedule_scan(h);
    }
    fn call(&mut self, h: &mut Host, call: &str, args: &Value) {
        let a = args_list(args);
        let f = |i: usize| a.get(i).and_then(Value::as_f64).unwrap_or(0.0);
        let q = |i: usize| a.get(i).and_then(Value::as_bool).unwrap_or(false);
        match call.strip_prefix("brightness.").unwrap_or("") {
            "set" => {
                if q(1) {
                    Self::hush(h);
                }
                if let Some(o) = self.target().map(|d| d.output.clone()) {
                    self.set_ddc(h, &o, f(0));
                } else if Self::backlight(h).0 {
                    self.set_backlight(f(0));
                }
            }
            "setFor" => {
                let id = str_arg(&a, 0);
                if q(2) {
                    Self::hush(h);
                }
                if id != "backlight" {
                    self.set_ddc(h, &id, f(1));
                } else if Self::backlight(h).0 {
                    self.set_backlight(f(1));
                }
            }
            "hush" => Self::hush(h),
            "rescan" | "rescanDdc" => self.rescan(h),
            // Settings -> Displays' DDC slider: [output, value]
            "setDdc" => self.set_ddc(h, &str_arg(&a, 0), f(1)),
            _ => {}
        }
        self.publish(h);
    }
    fn ipc(&mut self, h: &mut Host, _target: &str, func: &str, _args: &[String]) -> Option<String> {
        match func {
            "rescan" => {
                self.rescan(h);
                None
            }
            "status" => {
                let (present, bl) = Self::backlight(h);
                let t = self.target();
                Some(json!({"target": t.map(|d| json!({"output": d.output, "bus": d.bus, "model": d.model})), "value": t.map(|d| self.values.get(&d.output).copied().unwrap_or(0.5)).unwrap_or(bl),
                            "backlight": present, "ddc": self.ddc.iter().map(|d| json!({"output": d.output, "bus": d.bus, "model": d.model})).collect::<Vec<_>>(),
                            "problem": self.problem, "installed": self.installed}).to_string())
            }
            _ => None,
        }
    }
    fn message(&mut self, h: &mut Host, msg: Box<dyn Any + Send>) {
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Tick => {
                // Focus moved to an external: read it again (its own buttons may
                // have changed it), not again right after a read
                let focused = h.st.shell.get::<Snapshot>().map(|s| s.focused.clone()).unwrap_or_default();
                if focused != self.focused {
                    self.focused = focused;
                    if let Some(o) = self.target().map(|d| d.output.clone()) {
                        if now_ms() - self.read_at.get(&o).copied().unwrap_or(0.0) > 10000.0 {
                            self.read_ddc(h, &o);
                        }
                    }
                }
            }
            M::StartScan => self.rescan(h),
            M::Detected(t) => {
                self.detecting = false;
                self.scanned = true;
                let (problem, found) = parse_detect(&t);
                self.problem = problem;
                self.ddc = found;
                if self.detect_again {
                    self.detect_again = false;
                    self.rescan(h);
                }
                for o in self.ddc.iter().map(|d| d.output.clone()).collect::<Vec<_>>() {
                    self.read_ddc(h, &o);
                }
            }
            M::Read(output, t) => {
                if let Some((cur, max)) = parse_vcp(&t) {
                    self.max.insert(output.clone(), max);
                    // (don't undo a change made while this read was running)
                    if !self.pending.contains_key(&output) {
                        self.values.insert(output, cur as f64 / max as f64);
                    }
                }
                self.op_busy = false;
                self.flush_ddc(h);
                self.next_read(h);
            }
            M::Written => {
                self.op_busy = false;
                self.flush_ddc(h);
            }
        }
        self.publish(h);
    }
    fn quit(&mut self, _h: &mut Host) {
        self.tick.stop();
        self.scan.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_displays_and_problems() {
        assert_eq!(parse_detect("PROBLEM module\n"), ("module".into(), vec![]));
        assert_eq!(parse_detect("PROBLEM access").0, "access");
        let t = "Display 1\n   I2C bus:  /dev/i2c-12\n   DRM_connector:  card1-DP-1\n   Monitor:  BNQ:BenQ GW2786TC:12345\n\nInvalid display\n   I2C bus:  /dev/i2c-3\n\nDisplay 2\n   I2C bus:  /dev/i2c-5\n   DRM connector:  card1-HDMI-A-1\n   Monitor:  :\n";
        let (p, d) = parse_detect(t);
        assert_eq!(p, "");
        assert_eq!(d, vec![Ddc { output: "DP-1".into(), bus: 12, model: "BenQ GW2786TC".into() }, Ddc { output: "HDMI-A-1".into(), bus: 5, model: "HDMI-A-1".into() }]);
        assert_eq!(parse_detect("ddcutil requires module i2c-dev\n").0, "module");
        assert_eq!(parse_detect("open: Permission denied\n").0, "access");
    }

    #[test]
    fn reads_vcp() {
        assert_eq!(parse_vcp("VCP 10 C 50 100\n"), Some((50, 100)));
        assert_eq!(parse_vcp("VCP 10 C 5 0"), None);
        assert_eq!(parse_vcp("nothing"), None);
    }
}

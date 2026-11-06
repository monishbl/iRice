//! CPU, memory, swap, temperature, network and GPU, sampled while something
//! shows it (replaces services/Resources.qml), and the hover that opens the
//! resources popover (replaces modules/resources/ResourceHover.qml).
//!
//! Resources: sampling runs while any consumer shows it, once a second when a
//! graph shows (`fast`), every 5 seconds otherwise; the last minute of each is
//! kept. CPU, memory, swap, temperature and network are core's
//! sysstat::Sampler (the Rust popover and widgets use the same); the memory
//! sizes and the GPU (amdgpu's gpu_busy_percent, Intel's RC6 / idle
//! residency, NVIDIA through nvidia-smi) are added here.
//!
//! Calls (prefix "resources"): use({kind: overlay|menubar|island|desktop|graph,
//! on: bool}) (desktop and graph count users: on = +1, off = -1), and
//! hover({key, x, y, screen, hovered, gap}): a figure being hovered; half a
//! second's rest opens the popover under (x, y + gap) (screen coordinates:
//! the centre and bottom of the figure), 120 ms after leaving it closes
//! (`{"to":"resources","open":true,"key":…,"x":…,"y":…,"screen":…}`).
//! State on the bus as "resources": {cpu, memory, swap, memUsedGb, memTotalGb,
//! temperature, netDown, netUp, gpu, gpuAvailable, gpuAsleep, gpuTemp, active,
//! fast, stats:[{key,label,icon,tone,value,text,detail,history,max}],
//! groups}.

use super::common::*;
use serde_json::{json, Value};
use std::any::Any;
use std::time::Duration;
use tessera_core::host::{Host, Service};
use tessera_core::sysstat::{self, Sampler};

const NAME: &str = "Resources";

const FIND_GPU: &str = "nv=; if command -v nvidia-smi >/dev/null 2>&1 && [ -d /proc/driver/nvidia ]; then nv=present; for c in /sys/class/drm/card*; do case $c in *-*) continue;; esac; case \"$(readlink \"$c/device/driver\")\" in */nvidia) for s in \"$c\"-*/status; do [ \"$(cat \"$s\" 2>/dev/null)\" = connected ] && nv=display; done;; esac; done; fi; [ \"$nv\" = display ] && { echo 'nvidia nvidia-smi'; exit; }; for f in /sys/class/drm/card*/device/gpu_busy_percent; do [ -r \"$f\" ] && { echo \"busy $f\"; exit; }; done; for f in /sys/class/drm/card*/gt/gt0/rc6_residency_ms /sys/class/drm/card*/device/tile0/gt0/gtidle/idle_residency_ms; do [ -r \"$f\" ] && { echo \"idle $f\"; exit; }; done; [ -n \"$nv\" ] && echo 'nvidia nvidia-smi'; true";
const NVIDIA: &str = "for d in /sys/bus/pci/drivers/nvidia/0000:*; do [ \"$(cat \"$d/power/runtime_status\" 2>/dev/null)\" = suspended ] && { echo asleep; exit 0; }; done; exec nvidia-smi --query-gpu=utilization.gpu,temperature.gpu --format=csv,noheader,nounits";

/// /proc/meminfo: (used GB, total GB)
pub fn mem_gb(info: &str) -> (f64, f64) {
    let kb = |k: &str| -> f64 { info.lines().find_map(|l| l.strip_prefix(k).and_then(|r| r.strip_prefix(':'))).and_then(|r| r.split_whitespace().next()?.parse().ok()).unwrap_or(0.0) };
    let (total, avail) = (kb("MemTotal"), kb("MemAvailable"));
    ((total - avail) / 1048576.0, total / 1048576.0)
}

/// nvidia-smi's "utilization, temperature" line: Asleep, or (0..1, °C)
pub fn parse_nvidia(text: &str) -> Option<(f64, f64, bool)> {
    let l = text.trim().lines().next().unwrap_or("");
    if l == "asleep" {
        return Some((0.0, 0.0, true));
    }
    let f: Vec<f64> = l.split(',').map(|x| x.trim().parse().unwrap_or(f64::NAN)).collect();
    let u = *f.first()?;
    if !(u >= 0.0) {
        return None;
    }
    let t = f.get(1).copied().filter(|t| *t > 0.0).unwrap_or(0.0);
    Some((u.min(100.0) / 100.0, t, false))
}

enum M {
    Tick,
    GpuFound(String),
    Nvidia(i32, String),
    OpenFire,
    CloseFire,
}

#[derive(Default)]
struct Hover {
    key: String,
    x: f64,
    y: f64,
    screen: String,
    open: bool,
    open_t: slint::Timer,
    close_t: slint::Timer,
}

#[derive(Default)]
pub struct Resources {
    sampler: Sampler,
    tick: slint::Timer,
    ticks: u64,
    overlay: bool,
    menubar: bool,
    island: bool,
    desktop_users: i32,
    graph_users: i32,
    mem_used_gb: f64,
    mem_total_gb: f64,
    gpu_path: String,
    gpu_kind: String,
    gpu_searched: bool,
    gpu_searching: bool,
    gpu: f64,
    gpu_temp: f64,
    gpu_asleep: bool,
    gpu_history: Vec<f64>,
    last_gpu: Option<(f64, f64)>,
    nvidia_busy: bool,
    hover: Hover,
    last: Value,
}

impl Resources {
    fn active(&self) -> bool {
        self.overlay || self.menubar || self.island || self.desktop_users > 0 || self.graph_users > 0
    }
    fn fast(&self) -> bool {
        self.overlay || self.menubar || self.graph_users > 0
    }

    fn push_gpu(&mut self) {
        self.gpu_history.push(self.gpu);
        if self.gpu_history.len() > sysstat::HISTORY {
            self.gpu_history.remove(0);
        }
    }

    fn sample(&mut self, h: &mut Host) {
        self.sampler.sample(now_ms() / 1000.0);
        let (u, t) = mem_gb(&std::fs::read_to_string("/proc/meminfo").unwrap_or_default());
        self.mem_used_gb = u;
        self.mem_total_gb = t;
        if self.gpu_kind == "nvidia" {
            // a process per sample: only while a graph shows
            if self.fast() && !self.nvidia_busy {
                self.nvidia_busy = true;
                let send = h.sender(NAME);
                capture_then(sh_cmd(NVIDIA, &[]), None, move |c, t| send(Box::new(M::Nvidia(c, t))));
            }
        } else if !self.gpu_path.is_empty() {
            match std::fs::read_to_string(&self.gpu_path).ok().map(|t| t.trim().parse::<f64>().unwrap_or(0.0)) {
                Some(n) if self.gpu_kind == "busy" => {
                    self.gpu = n.min(100.0) / 100.0;
                    self.push_gpu();
                }
                Some(n) => {
                    let now = now_ms();
                    let last = self.last_gpu.replace((n, now));
                    if let Some((ms, t)) = last.filter(|(_, t)| now > *t) {
                        self.gpu = (1.0 - (n - ms) / (now - t)).clamp(0.0, 1.0);
                        self.push_gpu();
                    }
                }
                None => {
                    self.gpu_path.clear();
                    self.gpu_searched = false;
                }
            }
        } else if !self.gpu_searched && !self.gpu_searching {
            self.gpu_searching = true;
            let send = h.sender(NAME);
            capture_then(sh_cmd(FIND_GPU, &[]), None, move |_, t| send(Box::new(M::GpuFound(t))));
        }
    }

    fn publish(&mut self) {
        let st = self.sampler.stats();
        let last = |k: &str| st.iter().find(|s| s.key == k).and_then(|s| s.history.last().copied()).unwrap_or(0.0);
        let hist = |k: &str| st.iter().find(|s| s.key == k).map(|s| s.history.clone()).unwrap_or_default();
        let (cpu, memory, swap, temp, down, up) = (last("cpu"), last("memory"), last("swap"), last("temperature"), last("netDown"), last("netUp"));
        let gpu_available = !self.gpu_path.is_empty();
        let pct = |v: f64| format!("{}%", (v * 100.0).round());
        let mut stats = vec![
            json!({"key": "cpu", "label": "CPU", "icon": "cpu", "tone": "blue", "value": cpu, "text": pct(cpu), "detail": if temp > 0.0 { format!("{}°C", temp.round()) } else { String::new() }, "history": hist("cpu"), "max": 1}),
            json!({"key": "memory", "label": "Memory", "icon": "memory", "tone": "green", "value": memory, "text": pct(memory), "detail": format!("{:.1} of {:.1} GB", self.mem_used_gb, self.mem_total_gb), "history": hist("memory"), "max": 1}),
            json!({"key": "swap", "label": "Swap", "icon": "swap", "tone": "purple", "value": swap, "text": pct(swap), "detail": "", "history": hist("swap"), "max": 1}),
        ];
        if self.sampler.has_temp() {
            stats.push(json!({"key": "temperature", "label": "Temperature", "icon": "thermometer-simple", "tone": "orange", "value": temp / 100.0, "text": format!("{}°", temp.round()), "detail": "", "history": hist("temperature"), "max": 100}));
        }
        stats.push(json!({"key": "netDown", "label": "Download", "icon": "arrow-down", "tone": "teal", "value": 0, "text": sysstat::rate(down), "detail": "", "history": hist("netDown"), "max": 0}));
        stats.push(json!({"key": "netUp", "label": "Upload", "icon": "arrow-up", "tone": "pink", "value": 0, "text": sysstat::rate(up), "detail": "", "history": hist("netUp"), "max": 0}));
        if gpu_available {
            stats.push(json!({"key": "gpu", "label": "GPU", "icon": "graphics-card", "tone": "indigo", "value": self.gpu,
                              "text": if self.gpu_asleep { "Asleep".to_string() } else { pct(self.gpu) },
                              "detail": if self.gpu_temp > 0.0 && !self.gpu_asleep { format!("{}°C", self.gpu_temp.round()) } else { String::new() }, "history": self.gpu_history, "max": 1}));
        }
        let v = json!({
            "cpu": cpu, "memory": memory, "swap": swap, "memUsedGb": self.mem_used_gb, "memTotalGb": self.mem_total_gb, "temperature": temp, "netDown": down, "netUp": up,
            "gpu": self.gpu, "gpuAvailable": gpu_available, "gpuAsleep": self.gpu_asleep, "gpuTemp": self.gpu_temp, "active": self.active(), "fast": self.fast(), "stats": stats,
            "groups": if gpu_available { json!(["processor", "graphics", "memory", "network"]) } else { json!(["processor", "memory", "network"]) },
        });
        if v != self.last {
            crate::bus::put(NAME, v.clone());
            self.last = v;
        }
    }

    fn tell_hover(&self, h: &mut Host, open: bool) {
        if open {
            h.tell("resources", json!({"open": true, "key": self.hover.key, "x": self.hover.x, "y": self.hover.y, "screen": self.hover.screen}));
        } else {
            h.tell("resources", json!({"open": false, "key": self.hover.key, "screen": self.hover.screen}));
        }
    }
}

impl Service for Resources {
    fn name(&self) -> &'static str {
        NAME
    }
    fn calls(&self) -> &'static [&'static str] {
        &["resources"]
    }
    fn start(&mut self, h: &mut Host) {
        let send = h.sender(NAME);
        self.tick.start(slint::TimerMode::Repeated, Duration::from_secs(1), move || send(Box::new(M::Tick)));
        self.publish();
    }
    fn call(&mut self, h: &mut Host, call: &str, args: &Value) {
        let a = args_list(args);
        let r = a.first().cloned().unwrap_or(Value::Null);
        match call.strip_prefix("resources.").unwrap_or("") {
            "use" => {
                let on = r["on"].as_bool().unwrap_or(false);
                let d = if on { 1 } else { -1 };
                match r["kind"].as_str().unwrap_or("") {
                    "overlay" => self.overlay = on,
                    "menubar" => self.menubar = on,
                    "island" => self.island = on,
                    "desktop" => self.desktop_users = (self.desktop_users + d).max(0),
                    "graph" => self.graph_users = (self.graph_users + d).max(0),
                    _ => {}
                }
                if on {
                    self.sample(h); // (triggeredOnStart)
                }
            }
            "hover" => {
                let hovered = r["hovered"].as_bool().unwrap_or(false);
                let send = h.sender(NAME);
                if hovered {
                    self.hover.key = r["key"].as_str().unwrap_or("cpu").into();
                    self.hover.x = r["x"].as_f64().unwrap_or(0.0);
                    self.hover.y = r["y"].as_f64().unwrap_or(0.0) + r["gap"].as_f64().unwrap_or(8.0);
                    self.hover.screen = r["screen"].as_str().unwrap_or("").into();
                    self.hover.close_t.stop();
                    self.hover.open_t.start(slint::TimerMode::SingleShot, Duration::from_millis(500), move || send(Box::new(M::OpenFire)));
                } else {
                    self.hover.open_t.stop();
                    self.hover.close_t.start(slint::TimerMode::SingleShot, Duration::from_millis(120), move || send(Box::new(M::CloseFire)));
                }
            }
            _ => {}
        }
        self.publish();
    }
    fn message(&mut self, h: &mut Host, msg: Box<dyn Any + Send>) {
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Tick => {
                self.ticks += 1;
                if self.active() && (self.fast() || self.ticks % 5 == 0) {
                    self.sample(h);
                    self.publish();
                }
            }
            M::GpuFound(t) => {
                self.gpu_searching = false;
                self.gpu_searched = true;
                let mut f = t.trim().split(' ');
                self.gpu_kind = f.next().unwrap_or("").into();
                self.gpu_path = f.next().unwrap_or("").into();
                if self.gpu_kind == "nvidia" {
                    self.gpu_path = "nvidia-smi".into();
                }
            }
            M::Nvidia(code, t) => {
                self.nvidia_busy = false;
                if code != 0 {
                    // nvidia-smi doesn't work (driver/library mismatch after an update):
                    // no GPU graph rather than a failing process every second
                    self.gpu_path.clear();
                    self.gpu_kind.clear();
                } else if let Some((g, temp, asleep)) = parse_nvidia(&t) {
                    self.gpu_asleep = asleep;
                    self.gpu = g;
                    if !asleep {
                        self.gpu_temp = temp;
                    }
                    self.push_gpu();
                }
            }
            M::OpenFire => {
                self.hover.open = true;
                self.tell_hover(h, true);
            }
            M::CloseFire => {
                self.hover.open = false;
                self.tell_hover(h, false);
            }
        }
        self.publish();
    }
    fn quit(&mut self, _h: &mut Host) {
        self.tick.stop();
        self.hover.open_t.stop();
        self.hover.close_t.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_sizes_and_nvidia() {
        let (u, t) = mem_gb("MemTotal:       16777216 kB\nMemAvailable:    8388608 kB\n");
        assert_eq!((u, t), (8.0, 16.0));
        assert_eq!(parse_nvidia("35, 61\n"), Some((0.35, 61.0, false)));
        assert_eq!(parse_nvidia("asleep"), Some((0.0, 0.0, true)));
        assert_eq!(parse_nvidia("[N/A], 40"), None);
        assert_eq!(parse_nvidia("120, 0"), Some((1.0, 0.0, false)));
    }
}

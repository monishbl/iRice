//! The System widget's last minute (services/Resources.qml's samples):
//! CPU, memory, swap, the CPU package temperature and the network, read
//! from /proc and /sys once a second while a System widget shows. (No GPU
//! graph yet: Resources' GPU probing, nvidia-smi included, isn't here.)

use std::collections::VecDeque;

pub const HISTORY: usize = 60;

#[derive(Clone, Debug, PartialEq)]
pub struct Stat {
    pub key: &'static str,
    pub label: &'static str,
    /// a Theme accent (Glass) / ResourceTones' role (Material)
    pub tone: &'static str,
    pub text: String,
    pub history: Vec<f64>,
    /// the graph's top (0: scale to the largest value)
    pub max: f64,
}

#[derive(Default)]
pub struct Sampler {
    last_cpu: Option<(u64, u64)>,
    last_net: Option<(u64, u64, f64)>,
    temp_path: Option<Option<String>>,
    cpu: VecDeque<f64>,
    mem: VecDeque<f64>,
    swap: VecDeque<f64>,
    temp: VecDeque<f64>,
    down: VecDeque<f64>,
    up: VecDeque<f64>,
    last_tick: f64,
}

fn push(q: &mut VecDeque<f64>, v: f64) {
    q.push_back(v);
    while q.len() > HISTORY {
        q.pop_front();
    }
}

/// /proc/stat's first line: (idle, total)
pub fn cpu_times(stat: &str) -> Option<(u64, u64)> {
    let f: Vec<u64> = stat.lines().next()?.split_whitespace().skip(1).filter_map(|x| x.parse().ok()).collect();
    if f.len() < 5 {
        return None;
    }
    Some((f[3] + f[4], f.iter().sum()))
}

/// /proc/meminfo: (memory used 0..1, swap used 0..1)
pub fn mem_use(info: &str) -> (f64, f64) {
    let kb = |k: &str| -> f64 { info.lines().find_map(|l| l.strip_prefix(k).and_then(|r| r.strip_prefix(':'))).and_then(|r| r.split_whitespace().next()?.parse().ok()).unwrap_or(0.0) };
    let (total, avail, st, sf) = (kb("MemTotal"), kb("MemAvailable"), kb("SwapTotal"), kb("SwapFree"));
    (if total > 0.0 { 1.0 - avail / total } else { 0.0 }, if st > 0.0 { 1.0 - sf / st } else { 0.0 })
}

/// /proc/net/dev: bytes in and out over the real interfaces (no loopback,
/// containers, bridges or tunnels: a VPN's traffic would count twice)
pub fn net_bytes(dev: &str) -> (u64, u64) {
    let (mut rx, mut tx) = (0u64, 0u64);
    for line in dev.lines().skip(2) {
        let Some((name, rest)) = line.split_once(':') else { continue };
        let name = name.trim();
        if ["lo", "veth", "docker", "br-", "virbr", "vnet", "tun", "tap", "wg", "tailscale", "zt"].iter().any(|p| name.starts_with(p)) {
            continue;
        }
        let f: Vec<u64> = rest.split_whitespace().map(|x| x.parse().unwrap_or(0)).collect();
        rx += f.first().copied().unwrap_or(0);
        tx += f.get(8).copied().unwrap_or(0);
    }
    (rx, tx)
}

/// "512 B/s", "3.4 KB/s", "12 KB/s", "1.2 MB/s" (Resources.rate)
pub fn rate(b: f64) -> String {
    if b < 1024.0 {
        format!("{} B/s", b.round())
    } else if b < 1_048_576.0 {
        if b < 10240.0 { format!("{:.1} KB/s", b / 1024.0) } else { format!("{:.0} KB/s", b / 1024.0) }
    } else {
        format!("{:.1} MB/s", b / 1_048_576.0)
    }
}

fn read(p: &str) -> String {
    std::fs::read_to_string(p).unwrap_or_default()
}

/// The CPU package temperature's file: Intel's x86_pkg_temp zone, else
/// coretemp / k10temp / zenpower's package reading, else the hottest
/// CPU-ish zone (Resources' findTemp)
fn find_temp() -> Option<String> {
    let zones: Vec<_> = std::fs::read_dir("/sys/class/thermal").map(|d| d.flatten().map(|e| e.path()).filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("thermal_zone"))).collect()).unwrap_or_default();
    let ty = |p: &std::path::Path| read(&p.join("type").to_string_lossy()).trim().to_string();
    if let Some(z) = zones.iter().find(|z| ty(z) == "x86_pkg_temp") {
        return Some(z.join("temp").to_string_lossy().into_owned());
    }
    if let Ok(d) = std::fs::read_dir("/sys/class/hwmon") {
        for h in d.flatten().map(|e| e.path()) {
            if matches!(read(&h.join("name").to_string_lossy()).trim(), "coretemp" | "k10temp" | "zenpower") && h.join("temp1_input").exists() {
                return Some(h.join("temp1_input").to_string_lossy().into_owned());
            }
        }
    }
    zones
        .iter()
        .filter(|z| matches!(ty(z).as_str(), t if t.starts_with("cpu") || t == "acpitz" || t == "TCPU"))
        .filter_map(|z| Some((read(&z.join("temp").to_string_lossy()).trim().parse::<i64>().ok()?, z.join("temp").to_string_lossy().into_owned())))
        .max_by_key(|(t, _)| *t)
        .map(|(_, p)| p)
}

impl Sampler {
    /// One sample (now: seconds); after a pause the old minute is dropped
    pub fn sample(&mut self, now: f64) {
        if now - self.last_tick > 15.0 {
            *self = Sampler { temp_path: self.temp_path.take(), ..Default::default() };
        }
        self.last_tick = now;
        if let Some((idle, total)) = cpu_times(&read("/proc/stat")) {
            if let Some((li, lt)) = self.last_cpu {
                let dt = total.saturating_sub(lt);
                let v = if dt > 0 { 1.0 - idle.saturating_sub(li) as f64 / dt as f64 } else { 0.0 };
                push(&mut self.cpu, v.clamp(0.0, 1.0));
            }
            self.last_cpu = Some((idle, total));
        }
        let (m, s) = mem_use(&read("/proc/meminfo"));
        push(&mut self.mem, m);
        push(&mut self.swap, s);
        let (rx, tx) = net_bytes(&read("/proc/net/dev"));
        if let Some((lr, lt, t)) = self.last_net {
            if now > t {
                push(&mut self.down, rx.saturating_sub(lr) as f64 / (now - t));
                push(&mut self.up, tx.saturating_sub(lt) as f64 / (now - t));
            }
        }
        self.last_net = Some((rx, tx, now));
        if self.temp_path.is_none() {
            self.temp_path = Some(find_temp());
        }
        if let Some(Some(p)) = &self.temp_path {
            match read(p).trim().parse::<f64>() {
                Ok(v) => push(&mut self.temp, v / 1000.0),
                Err(_) => self.temp_path = None, // (the sensor went away: look again)
            }
        }
    }

    pub fn has_temp(&self) -> bool {
        matches!(self.temp_path, Some(Some(_)))
    }

    /// Resources.stats, in its order
    pub fn stats(&self) -> Vec<Stat> {
        let last = |q: &VecDeque<f64>| q.back().copied().unwrap_or(0.0);
        let pct = |v: f64| format!("{}%", (v * 100.0).round());
        let mut v = vec![
            Stat { key: "cpu", label: "CPU", tone: "blue", text: pct(last(&self.cpu)), history: self.cpu.iter().copied().collect(), max: 1.0 },
            Stat { key: "memory", label: "Memory", tone: "green", text: pct(last(&self.mem)), history: self.mem.iter().copied().collect(), max: 1.0 },
            Stat { key: "swap", label: "Swap", tone: "purple", text: pct(last(&self.swap)), history: self.swap.iter().copied().collect(), max: 1.0 },
        ];
        if self.has_temp() {
            v.push(Stat { key: "temperature", label: "Temperature", tone: "orange", text: format!("{}°", last(&self.temp).round()), history: self.temp.iter().copied().collect(), max: 100.0 });
        }
        v.push(Stat { key: "netDown", label: "Download", tone: "teal", text: rate(last(&self.down)), history: self.down.iter().copied().collect(), max: 0.0 });
        v.push(Stat { key: "netUp", label: "Upload", tone: "pink", text: rate(last(&self.up)), history: self.up.iter().copied().collect(), max: 0.0 });
        v
    }
}

/// Which stats a System widget shows (SystemWidget.qml)
pub fn keys_for(size: &str, have: &[&str]) -> Vec<String> {
    match size {
        "s" => vec!["cpu".into(), "memory".into()],
        "m" => vec!["cpu".into(), "memory".into(), if have.contains(&"temperature") { "temperature" } else { "swap" }.into(), "netDown".into()],
        _ => have.iter().map(|s| s.to_string()).collect(),
    }
}

/// A graph as SVG path commands in a `w` × `h` box (ResourceGraph.qml):
/// (the area under the line, the line), or bars; newest at the right, a
/// minute across
pub fn graph(values: &[f64], max: f64, w: f32, h: f32, style: &str, line_width: f32) -> (String, String) {
    let slots = HISTORY;
    let n = values.len().min(slots);
    if n == 0 || w <= 0.0 || h <= 0.0 {
        return (String::new(), String::new());
    }
    let vals = &values[values.len() - n..];
    let ceiling = if max > 0.0 { max } else { (vals.iter().cloned().fold(0.0, f64::max) * 1.15).max(65536.0) };
    let frac = |v: f64| (v / ceiling).clamp(0.0, 1.0) as f32;
    if style == "bars" {
        let bw = w / slots as f32;
        let gap = if bw >= 4.0 { 1.0 } else { 0.0 };
        let mut d = String::new();
        for (i, v) in vals.iter().enumerate() {
            let bh = (frac(*v) * h).max(1.0);
            let x = w - (n - i) as f32 * bw + gap / 2.0;
            d += &format!("M {x:.2} {:.2} h {:.2} v {bh:.2} h {:.2} Z ", h - bh, bw - gap, -(bw - gap));
        }
        return (d, String::new());
    }
    if n < 2 {
        return (String::new(), String::new());
    }
    let step = w / (slots - 1) as f32;
    let pad = line_width / 2.0;
    let pts: Vec<(f32, f32)> = vals.iter().enumerate().map(|(i, v)| (w - (n - 1 - i) as f32 * step, h - pad - frac(*v) * (h - 2.0 * pad))).collect();
    // a curve through each segment's midpoint
    let mut line = format!("M {:.2} {:.2} ", pts[0].0, pts[0].1);
    for i in 1..n {
        let (mx, my) = ((pts[i - 1].0 + pts[i].0) / 2.0, (pts[i - 1].1 + pts[i].1) / 2.0);
        line += &format!("Q {:.2} {:.2} {mx:.2} {my:.2} ", pts[i - 1].0, pts[i - 1].1);
    }
    line += &format!("L {:.2} {:.2} ", pts[n - 1].0, pts[n - 1].1);
    let area = if style == "line" { String::new() } else { format!("{line}L {:.2} {h:.2} L {:.2} {h:.2} Z", pts[n - 1].0, pts[0].0) };
    (area, line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_proc() {
        assert_eq!(cpu_times("cpu  10 0 10 70 10 0 0 0 0 0\ncpu0 1 2 3 4 5"), Some((80, 100)));
        assert_eq!(cpu_times(""), None);
        let (m, s) = mem_use("MemTotal:  1000 kB\nMemFree: 1 kB\nMemAvailable:  250 kB\nSwapTotal: 0 kB\nSwapFree: 0 kB\n");
        assert_eq!((m, s), (0.75, 0.0));
        let dev = "Inter-|   Receive\n face |bytes\n    lo: 100 0 0 0 0 0 0 0 100 0\n  eth0: 1000 5 0 0 0 0 0 0 2000 4\ndocker0: 9 0 0 0 0 0 0 0 9 0\n wlan0: 10 0 0 0 0 0 0 0 20 0\n";
        assert_eq!(net_bytes(dev), (1010, 2020));
    }

    #[test]
    fn rates_read_as_the_shell_writes_them() {
        assert_eq!(rate(512.0), "512 B/s");
        assert_eq!(rate(3481.6), "3.4 KB/s");
        assert_eq!(rate(12288.0), "12 KB/s");
        assert_eq!(rate(1258291.2), "1.2 MB/s");
    }

    #[test]
    fn which_stats_by_size() {
        let have = ["cpu", "memory", "swap", "netDown", "netUp"];
        assert_eq!(keys_for("s", &have), vec!["cpu", "memory"]);
        assert_eq!(keys_for("m", &have), vec!["cpu", "memory", "swap", "netDown"]);
        assert_eq!(keys_for("m", &["cpu", "temperature"]), vec!["cpu", "memory", "temperature", "netDown"]);
        assert_eq!(keys_for("l", &have).len(), 5);
    }

    #[test]
    fn graphs() {
        assert_eq!(graph(&[], 1.0, 100.0, 40.0, "area", 1.5), (String::new(), String::new()));
        let (area, line) = graph(&[0.0, 1.0], 1.0, 59.0, 40.0, "area", 2.0);
        assert!(line.starts_with("M 58.00 39.00") && line.contains("L 59.00 1.00"), "{line}");
        assert!(area.ends_with('Z'));
        let (_, line) = graph(&[0.5, 0.5], 1.0, 59.0, 40.0, "line", 2.0);
        assert!(!line.is_empty());
        assert!(graph(&[0.5, 0.5], 1.0, 59.0, 40.0, "line", 2.0).0.is_empty());
        let (bars, _) = graph(&[1.0], 1.0, 60.0, 40.0, "bars", 1.5);
        assert!(bars.starts_with("M 59.00 0.00"), "{bars}");
        let mut s = Sampler::default();
        s.sample(100.0);
        s.sample(101.0);
        let st = s.stats();
        assert_eq!(st[0].key, "cpu");
        assert!(st.iter().any(|x| x.key == "netUp"));
    }
}

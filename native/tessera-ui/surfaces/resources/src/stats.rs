//! What the resources popover shows (ResourceRow.qml on Resources.qml's
//! stats): the samples come from the desktop's sampler (desktop::sysstat:
//! CPU, memory, swap, the package temperature, the network), plus memory in
//! GB and the GPU's load where the driver reports it (amdgpu's
//! gpu_busy_percent, Intel's idle residency), as Resources.qml reads them.

use crate::sysstat::{self, Stat};

/// A row: a group of stats (ResourceRow.qml)
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub group: &'static str,
    pub title: &'static str,
    pub icon: &'static str,
    /// the figure on the right, and the network's second one
    pub value: String,
    pub value2: String,
    /// the line under the title
    pub detail: String,
    pub tone: &'static str,
    pub tone2: &'static str,
    pub main: Vec<f64>,
    pub second: Vec<f64>,
    /// the main graph's top (0: scale to the largest)
    pub max: f64,
}

/// The group a hovered figure belongs to (ResourcePopover.group)
pub fn group_of(key: &str) -> &'static str {
    match key {
        "cpu" | "temperature" => "processor",
        "gpu" => "graphics",
        "memory" | "swap" => "memory",
        k if k.starts_with("net") => "network",
        _ => "processor",
    }
}

/// The groups shown (Resources.groups): graphics only where there's a GPU reading
pub fn groups(gpu: bool) -> Vec<&'static str> {
    if gpu { vec!["processor", "graphics", "memory", "network"] } else { vec!["processor", "memory", "network"] }
}

/// Material's role for a stat's tone (ResourceTones.qml): 0 primary,
/// 1 secondary, 2 tertiary
pub fn role(tone: &str) -> i32 {
    match tone {
        "green" | "teal" => 2,
        "purple" | "orange" => 1,
        _ => 0,
    }
}

/// The network's shared top: a little room over the peak of both lines
pub fn net_top(a: &[f64], b: &[f64]) -> f64 {
    a.iter().chain(b.iter()).cloned().fold(65536.0, f64::max) * 1.15
}

/// The rows, from the stats (Resources.stats' keys) and the memory's
/// "used of total GB"
pub fn rows(stats: &[Stat], mem_detail: &str, gpu: Option<&Stat>) -> Vec<Row> {
    let find = |k: &str| stats.iter().find(|s| s.key == k);
    let empty = Stat { key: "cpu", label: "", tone: "blue", text: String::new(), history: Vec::new(), max: 1.0 };
    let mut out = Vec::new();
    for g in groups(gpu.is_some()) {
        let (main, second) = match g {
            "processor" => (find("cpu"), find("temperature")),
            "graphics" => (gpu, None),
            "memory" => (find("memory"), find("swap")),
            _ => (find("netDown"), find("netUp")),
        };
        let main = main.unwrap_or(&empty);
        let detail = match g {
            "processor" => second.map(|s| s.text.replace('°', "°C")).unwrap_or_default(),
            "memory" => {
                let swap = second.and_then(|s| s.history.last().copied()).unwrap_or(0.0);
                let t = second.map(|s| s.text.clone()).unwrap_or_default();
                format!("{mem_detail}{}", if swap > 0.005 { format!(" · swap {t}") } else { String::new() })
            }
            _ => String::new(),
        };
        let net = g == "network";
        out.push(Row {
            group: g,
            title: match g {
                "processor" => "Processor",
                "graphics" => "Graphics",
                "memory" => "Memory",
                _ => "Network",
            },
            icon: match g {
                "processor" => "cpu",
                "graphics" => "graphics-card",
                "memory" => "memory",
                _ => "arrows-down-up",
            },
            value: if net { format!("↓ {}", main.text) } else { main.text.clone() },
            value2: if net { format!("↑ {}", second.map(|s| s.text.as_str()).unwrap_or("")) } else { String::new() },
            detail,
            tone: main.tone,
            tone2: second.map_or(main.tone, |s| s.tone),
            main: main.history.clone(),
            second: if net { second.map(|s| s.history.clone()).unwrap_or_default() } else { Vec::new() },
            max: main.max,
        });
    }
    out
}

/// /proc/meminfo: "6.1 of 15.4 GB" (in core: the overlay shows it too)
pub use crate::surfaces::kit::mem_detail;

/// The GPU's reading: amdgpu's busy percentage, or Intel's idle residency
/// (busy = 1 - idle time over the interval)
#[derive(Default)]
pub struct Gpu {
    found: Option<Option<(bool, String)>>, // (busy kind, file)
    last_idle: Option<(f64, f64)>,
    history: Vec<f64>,
}

fn glob_first(pattern_dirs: &[&str]) -> Option<String> {
    let cards: Vec<_> = std::fs::read_dir("/sys/class/drm").ok()?.flatten().map(|e| e.path()).filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("card") && !n.to_string_lossy().contains('-'))).collect();
    for rel in pattern_dirs {
        for c in &cards {
            let f = c.join(rel);
            if std::fs::File::open(&f).is_ok() {
                return Some(f.to_string_lossy().into_owned());
            }
        }
    }
    None
}

impl Gpu {
    fn find() -> Option<(bool, String)> {
        if let Some(f) = glob_first(&["device/gpu_busy_percent"]) {
            return Some((true, f));
        }
        glob_first(&["gt/gt0/rc6_residency_ms", "device/tile0/gt0/gtidle/idle_residency_ms"]).map(|f| (false, f))
    }

    pub fn sample(&mut self, now: f64) {
        let found = self.found.get_or_insert_with(Self::find).clone();
        let Some((busy, path)) = found else { return };
        let Ok(text) = std::fs::read_to_string(&path) else { return };
        let Ok(v) = text.trim().parse::<f64>() else { return };
        let load = if busy {
            Some((v / 100.0).clamp(0.0, 1.0))
        } else {
            let prev = self.last_idle.replace((v, now));
            prev.filter(|(_, t)| now > *t).map(|(pv, t)| (1.0 - (v - pv) / ((now - t) * 1000.0)).clamp(0.0, 1.0))
        };
        if let Some(l) = load {
            self.history.push(l);
            if self.history.len() > sysstat::HISTORY {
                self.history.remove(0);
            }
        }
    }

    pub fn stat(&self) -> Option<Stat> {
        if !matches!(self.found, Some(Some(_))) {
            return None;
        }
        let last = self.history.last().copied().unwrap_or(0.0);
        Some(Stat { key: "gpu", label: "GPU", tone: "indigo", text: format!("{}%", (last * 100.0).round()), history: self.history.clone(), max: 1.0 })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stat(key: &'static str, tone: &'static str, text: &str, history: Vec<f64>, max: f64) -> Stat {
        Stat { key, label: "", tone, text: text.into(), history, max }
    }

    #[test]
    fn groups_and_tones() {
        assert_eq!(group_of("temperature"), "processor");
        assert_eq!(group_of("netUp"), "network");
        assert_eq!(group_of("swap"), "memory");
        assert_eq!(group_of("gpu"), "graphics");
        assert_eq!(groups(false), vec!["processor", "memory", "network"]);
        assert_eq!((role("blue"), role("purple"), role("teal")), (0, 1, 2));
        assert_eq!(net_top(&[], &[]), 65536.0 * 1.15);
        assert_eq!(net_top(&[1e6], &[2e6]), 2e6 * 1.15);
    }

    #[test]
    fn rows_read_as_resource_row() {
        let stats = vec![
            stat("cpu", "blue", "12%", vec![0.1, 0.12], 1.0),
            stat("memory", "green", "40%", vec![0.4], 1.0),
            stat("swap", "purple", "3%", vec![0.03], 1.0),
            stat("temperature", "orange", "52°", vec![52.0], 100.0),
            stat("netDown", "teal", "1.2 KB/s", vec![1200.0], 0.0),
            stat("netUp", "pink", "300 B/s", vec![300.0], 0.0),
        ];
        let r = rows(&stats, "6.1 of 15.4 GB", None);
        assert_eq!(r.len(), 3);
        assert_eq!((r[0].title, r[0].value.as_str(), r[0].detail.as_str()), ("Processor", "12%", "52°C"));
        assert_eq!(r[1].detail, "6.1 of 15.4 GB · swap 3%");
        assert_eq!((r[2].value.as_str(), r[2].value2.as_str(), r[2].tone2), ("↓ 1.2 KB/s", "↑ 300 B/s", "pink"));
        // (no swap in use: none said)
        let mut s2 = stats.clone();
        s2[2].history = vec![0.0];
        assert_eq!(rows(&s2, "1.0 of 2.0 GB", None)[1].detail, "1.0 of 2.0 GB");
        let g = stat("gpu", "indigo", "7%", vec![0.07], 1.0);
        let r = rows(&stats, "", Some(&g));
        assert_eq!((r[1].title, r[1].icon, r.len()), ("Graphics", "graphics-card", 4));
        assert_eq!(mem_detail("MemTotal: 16148844 kB\nMemAvailable: 9759008 kB\n"), "6.1 of 15.4 GB");
    }
}

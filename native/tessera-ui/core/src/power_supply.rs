//! The kernel's power supplies (/sys/class/power_supply), read directly where
//! UPower isn't asked: which are the computer's own batteries (BAT0, BAT1,
//! CMB0…: any name, type Battery, not a mouse's or headset's, which say scope
//! Device), whether it runs on mains (an adapter of any name: AC, ACAD, ADP1,
//! or a USB-C port), and the level over all its batteries. A desktop has
//! none; a UPS (type UPS) isn't the computer's battery either.

use std::path::{Path, PathBuf};

pub const CLASS: &str = "/sys/class/power_supply";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Supply {
    pub name: String,
    pub path: PathBuf,
    /// Battery, Mains, USB, UPS…
    pub kind: String,
    /// a peripheral's (scope Device: a mouse, a headset, a phone)
    pub peripheral: bool,
    /// an adapter's `online`
    pub online: Option<bool>,
    /// a battery's Charging, Discharging, Full, Not charging, Unknown
    pub status: String,
    pub capacity: Option<f64>,
    /// energy (µWh) or charge (µAh) now and when full, whichever it reports
    pub now: Option<f64>,
    pub full: Option<f64>,
}

impl Supply {
    /// One of the computer's own batteries
    pub fn system_battery(&self) -> bool {
        self.kind == "Battery" && !self.peripheral
    }
    /// An adapter that can power the computer (not a UPS: that one feeds it
    /// mains through its own Mains supply, when it says anything)
    pub fn adapter(&self) -> bool {
        !self.peripheral && matches!(self.kind.as_str(), "Mains" | "USB" | "USB_C" | "USB_PD" | "USB_PD_DRP" | "BrickID" | "Wireless")
    }
}

fn text(p: &Path) -> Option<String> {
    std::fs::read_to_string(p).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}
fn num(p: &Path) -> Option<f64> {
    text(p)?.parse().ok()
}

/// Every supply under `root` (CLASS, or a test's folder), sorted by name;
/// none when it can't be read
pub fn read(root: &Path) -> Vec<Supply> {
    let Ok(rd) = std::fs::read_dir(root) else { return Vec::new() };
    let mut out: Vec<Supply> = rd
        .flatten()
        .map(|e| {
            let p = e.path();
            Supply {
                name: e.file_name().to_string_lossy().into_owned(),
                kind: text(&p.join("type")).unwrap_or_default(),
                peripheral: text(&p.join("scope")).is_some_and(|s| s == "Device"),
                online: num(&p.join("online")).map(|v| v > 0.0),
                status: text(&p.join("status")).unwrap_or_default(),
                capacity: num(&p.join("capacity")),
                now: num(&p.join("energy_now")).or_else(|| num(&p.join("charge_now"))),
                full: num(&p.join("energy_full")).or_else(|| num(&p.join("charge_full"))),
                path: p,
            }
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Summary {
    pub has_battery: bool,
    /// running on its batteries (no adapter online, one discharging)
    pub on_battery: bool,
    /// 0..100 over all its batteries; -1 without one (or unknown)
    pub percent: i32,
}

pub fn summary(supplies: &[Supply]) -> Summary {
    let bats: Vec<&Supply> = supplies.iter().filter(|s| s.system_battery()).collect();
    if bats.is_empty() {
        return Summary { has_battery: false, on_battery: false, percent: -1 };
    }
    let adapters: Vec<&Supply> = supplies.iter().filter(|s| s.adapter()).collect();
    let discharging = bats.iter().any(|b| b.status == "Discharging");
    // (an adapter that says it's online wins; without any adapter listed,
    // the batteries' own word)
    let on_battery = if adapters.iter().any(|a| a.online.is_some()) { !adapters.iter().any(|a| a.online == Some(true)) && !bats.iter().any(|b| b.status == "Charging") } else { discharging };
    // two batteries (ThinkPads): by what they hold, else their levels' mean
    let percent = if bats.iter().all(|b| b.now.is_some() && b.full.is_some_and(|f| f > 0.0)) {
        let (n, f) = bats.iter().fold((0.0, 0.0), |(n, f), b| (n + b.now.unwrap(), f + b.full.unwrap()));
        Some(100.0 * n / f)
    } else {
        let caps: Vec<f64> = bats.iter().filter_map(|b| b.capacity).collect();
        (!caps.is_empty()).then(|| caps.iter().sum::<f64>() / caps.len() as f64)
    };
    Summary { has_battery: true, on_battery, percent: percent.map_or(-1, |p| p.round().clamp(0.0, 100.0) as i32) }
}

#[cfg(test)]
pub mod fixture {
    use std::path::{Path, PathBuf};

    /// A fake /sys/class/power_supply: each (name, [(file, value)])
    pub fn class(tag: &str, supplies: &[(&str, &[(&str, &str)])]) -> PathBuf {
        let root = std::env::temp_dir().join(format!("tessera-ps-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for (name, files) in supplies {
            let d = root.join(name);
            std::fs::create_dir_all(&d).unwrap();
            for (f, v) in *files {
                std::fs::write(d.join(f), format!("{v}\n")).unwrap();
            }
        }
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    pub fn clean(root: &Path) {
        let _ = std::fs::remove_dir_all(root);
    }
}

#[cfg(test)]
mod tests {
    use super::fixture::{class, clean};
    use super::*;

    #[test]
    fn a_desktop_has_no_battery() {
        let r = class("desktop", &[]);
        assert_eq!(summary(&read(&r)), Summary { has_battery: false, on_battery: false, percent: -1 });
        assert!(read(&r.join("missing")).is_empty());
        clean(&r);
        // only a UPS and a wireless mouse's battery: still none of its own
        let r = class("ups", &[("ups0", &[("type", "UPS"), ("status", "Discharging"), ("capacity", "90")]), ("hidpp_battery_0", &[("type", "Battery"), ("scope", "Device"), ("status", "Discharging"), ("capacity", "40")])]);
        assert_eq!(summary(&read(&r)), Summary { has_battery: false, on_battery: false, percent: -1 });
        clean(&r);
    }

    #[test]
    fn two_batteries_and_odd_adapter_names() {
        // a ThinkPad: BAT0 + BAT1 by energy, adapter "AC" offline
        let r = class(
            "tp",
            &[
                ("AC", &[("type", "Mains"), ("online", "0")]),
                ("BAT0", &[("type", "Battery"), ("status", "Discharging"), ("energy_now", "10000000"), ("energy_full", "20000000"), ("capacity", "50")]),
                ("BAT1", &[("type", "Battery"), ("status", "Unknown"), ("energy_now", "20000000"), ("energy_full", "20000000"), ("capacity", "100")]),
            ],
        );
        assert_eq!(summary(&read(&r)), Summary { has_battery: true, on_battery: true, percent: 75 });
        clean(&r);
        // "ACAD" plugged in, the battery reports charge, not energy; a mouse's battery is left out
        let r = class(
            "acad",
            &[
                ("ACAD", &[("type", "Mains"), ("online", "1")]),
                ("BAT1", &[("type", "Battery"), ("status", "Not charging"), ("capacity", "80")]),
                ("hid-mouse-battery", &[("type", "Battery"), ("scope", "Device"), ("capacity", "5")]),
            ],
        );
        assert_eq!(summary(&read(&r)), Summary { has_battery: true, on_battery: false, percent: 80 });
        clean(&r);
        // "ADP1" offline, a USB-C port online: on mains
        let r = class("adp", &[("ADP1", &[("type", "Mains"), ("online", "0")]), ("ucsi-source-psy-USBC000:001", &[("type", "USB"), ("online", "1")]), ("CMB0", &[("type", "Battery"), ("status", "Charging"), ("capacity", "33")])]);
        assert_eq!(summary(&read(&r)), Summary { has_battery: true, on_battery: false, percent: 33 });
        clean(&r);
        // no adapter listed at all: the battery's own status
        let r = class("noac", &[("BAT0", &[("type", "Battery"), ("status", "Discharging"), ("capacity", "61")])]);
        assert_eq!(summary(&read(&r)), Summary { has_battery: true, on_battery: true, percent: 61 });
        clean(&r);
    }
}

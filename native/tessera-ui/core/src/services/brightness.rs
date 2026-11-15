//! The built-in display's brightness (service "brightness"): the backlight
//! in /sys/class/backlight, the device with the finest steps when there are
//! several (as Brightness.qml picks brightnessctl's), and its changes. The
//! kernel wakes a poll() on `actual_brightness` (sysfs_notify) whenever the
//! level changes, whoever changed it (keys, brightnessctl, logind), so this
//! thread sleeps until then. `set` writes it through logind's
//! SetBrightness (what brightnessctl does without a udev rule): no root.
//!
//! In the dev sandbox (TESSERA_SANDBOX=1) the backlight is the laptop's own:
//! there it's never written, the level only moves in memory (as the QML
//! shell's). TESSERA_BACKLIGHT=<a folder like /sys/class/backlight/x> reads
//! (and, in the sandbox, writes) that one instead, to test with.
//!
//! External monitors (DDC/CI through ddcutil, Brightness.qml's other half)
//! aren't here yet.

use crate::Msg;
use smithay_client_toolkit::reexports::calloop::channel::Sender;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Mutex, OnceLock};
use std::time::Duration;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Backlight {
    /// a backlight exists
    pub present: bool,
    /// its name (intel_backlight, amdgpu_bl0…)
    pub device: String,
    /// 0..1
    pub value: f64,
}

/// The backlight with the finest steps (the largest max_brightness), among
/// (name, max_brightness)
pub fn pick(devices: &[(String, u64)]) -> Option<String> {
    devices.iter().filter(|(_, m)| *m > 0).max_by_key(|(_, m)| *m).map(|(n, _)| n.clone())
}

fn read_num(p: &Path) -> Option<u64> {
    std::fs::read_to_string(p).ok()?.trim().parse().ok()
}

/// The folder of the backlight in use
fn find() -> Option<PathBuf> {
    if let Some(d) = crate::env::var("BACKLIGHT").filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(d));
    }
    find_in(Path::new("/sys/class/backlight"))
}

/// The backlight to use among a backlight class folder's devices (none:
/// a desktop, external monitors only)
pub fn find_in(base: &Path) -> Option<PathBuf> {
    let mut devices: Vec<(String, u64, bool)> = std::fs::read_dir(base)
        .ok()?
        .flatten()
        .filter_map(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            // (a raw backlight hangs off its panel's connector: one whose
            // connector is disabled lights nothing, as intel_backlight on a
            // laptop whose NVIDIA card drives the panel)
            let off = std::fs::read_to_string(base.join(&n).join("device/enabled")).is_ok_and(|s| s.trim() == "disabled");
            Some((n.clone(), read_num(&base.join(&n).join("max_brightness"))?, off))
        })
        .collect();
    devices.sort();
    if devices.iter().any(|d| !d.2 && d.1 > 0) {
        devices.retain(|d| !d.2);
    }
    let devices: Vec<(String, u64)> = devices.into_iter().map(|(n, m, _)| (n, m)).collect();
    pick(&devices).map(|n| base.join(n))
}

fn reading(dir: &Path) -> Backlight {
    let max = read_num(&dir.join("max_brightness")).unwrap_or(0);
    let now = read_num(&dir.join("brightness")).unwrap_or(0);
    Backlight {
        present: max > 0,
        device: dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        value: if max > 0 { (now as f64 / max as f64).clamp(0.0, 1.0) } else { 0.0 },
    }
}

struct Shared {
    dir: PathBuf,
    tx: Sender<Msg>,
    /// the level last sent, so a write shows at once
    now: Backlight,
}

static SHARED: Mutex<Option<Shared>> = Mutex::new(None);

/// The backlight went: said once (no slider, no OSD); false when the
/// interface is gone
fn gone(dir: &Path) -> bool {
    let mut shared = SHARED.lock().unwrap();
    let Some(s) = shared.as_mut() else { return false };
    if s.now.present {
        s.now = Backlight { present: false, device: dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), value: 0.0 };
        return s.tx.send(Msg::Data(Box::new(s.now.clone()))).is_ok();
    }
    true
}

pub fn spawn(tx: Sender<Msg>) {
    let Some(dir) = find() else {
        let _ = tx.send(Msg::Data(Box::new(Backlight::default())));
        return;
    };
    let first = reading(&dir);
    if let Ok(mut s) = SHARED.lock() {
        *s = Some(Shared { dir: dir.clone(), tx: tx.clone(), now: first.clone() });
    }
    let _ = tx.send(Msg::Data(Box::new(first)));
    std::thread::spawn(move || {
        // (a sysfs file wakes poll() with POLLPRI on a change; a plain file,
        // a test folder, never does: read that one twice a second)
        let sysfs = dir.starts_with("/sys");
        let watch = Some(dir.join("actual_brightness")).filter(|p| p.exists()).unwrap_or_else(|| dir.join("brightness"));
        loop {
            let f = std::fs::File::open(&watch);
            let Ok(mut f) = f else {
                // (the device went, a GPU driver unloaded or a panel switched
                // off with its output: no backlight until it's back, and no
                // busy loop on a file that's gone)
                if !dir.join("max_brightness").exists() && !gone(&dir) {
                    return;
                }
                std::thread::sleep(Duration::from_secs(5));
                continue;
            };
            loop {
                use std::io::Read;
                let mut buf = [0u8; 32];
                let _ = f.read(&mut buf); // (reading arms the next notification)
                if sysfs {
                    let mut p = libc::pollfd { fd: std::os::fd::AsRawFd::as_raw_fd(&f), events: libc::POLLPRI | libc::POLLERR, revents: 0 };
                    if unsafe { libc::poll(&mut p, 1, -1) } < 0 {
                        std::thread::sleep(Duration::from_millis(500));
                        break;
                    }
                    // (a key held down: one reading per burst)
                    std::thread::sleep(Duration::from_millis(30));
                    // (its file removed: poll() would wake at once, forever)
                    if !watch.exists() {
                        break;
                    }
                } else {
                    std::thread::sleep(Duration::from_millis(500));
                }
                let now = reading(&dir);
                let mut shared = SHARED.lock().unwrap();
                let Some(s) = shared.as_mut() else { return };
                if (now.value - s.now.value).abs() > 1e-6 || now.present != s.now.present {
                    s.now = now.clone();
                    if s.tx.send(Msg::Data(Box::new(now))).is_err() {
                        return;
                    }
                }
                drop(shared);
                if std::io::Seek::seek(&mut f, std::io::SeekFrom::Start(0)).is_err() {
                    break;
                }
            }
        }
    });
}

fn sandboxed() -> bool {
    crate::env::var("SANDBOX").as_deref() == Some("1")
}

/// Set the level (0.01..1, never fully dark, as Brightness.qml); the new
/// level is sent at once, the write happens on a thread of its own
pub fn set(v: f64) {
    let v = v.clamp(0.01, 1.0);
    let mut shared = SHARED.lock().unwrap();
    let Some(s) = shared.as_mut() else { return };
    if !s.now.present {
        return;
    }
    s.now.value = v;
    let _ = s.tx.send(Msg::Data(Box::new(s.now.clone())));
    let dir = s.dir.clone();
    drop(shared);
    let test_folder = !dir.starts_with("/sys");
    if sandboxed() && !test_folder {
        return; // (the laptop's real backlight: only the level moves)
    }
    static WRITES: OnceLock<Mutex<mpsc::Sender<(PathBuf, f64)>>> = OnceLock::new();
    let tx = WRITES.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<(PathBuf, f64)>();
        std::thread::spawn(move || {
            let mut bus: Option<zbus::blocking::Connection> = None;
            while let Ok(first) = rx.recv() {
                let (dir, v) = rx.try_iter().last().unwrap_or(first); // (a drag: its newest level)
                let max = read_num(&dir.join("max_brightness")).unwrap_or(0);
                let raw = (v * max as f64).round() as u32;
                if !dir.starts_with("/sys") {
                    let _ = std::fs::write(dir.join("brightness"), format!("{raw}\n"));
                    continue;
                }
                if bus.is_none() {
                    bus = zbus::blocking::Connection::system().ok();
                }
                let Some(b) = bus.as_ref() else { continue };
                let name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let r = b.call_method(
                    Some("org.freedesktop.login1"),
                    "/org/freedesktop/login1/session/auto",
                    Some("org.freedesktop.login1.Session"),
                    "SetBrightness",
                    &("backlight", name.as_str(), raw),
                );
                if let Err(e) = r {
                    // (logind finds the session by the caller: a shell started
                    // outside one is refused; brightnessctl, as Brightness.qml
                    // uses, may still be allowed through its udev rule)
                    let ok = std::process::Command::new("brightnessctl")
                        .args(["-q", "-d", &name, "set", &raw.to_string()])
                        .stdin(std::process::Stdio::null())
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .status()
                        .is_ok_and(|s| s.success());
                    if !ok {
                        eprintln!("tessera-ui: brightness: {e}");
                    }
                    bus = None;
                }
            }
        });
        Mutex::new(tx)
    });
    if let Ok(tx) = tx.lock() {
        let _ = tx.send((dir, v));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_finest_backlight_is_used() {
        let d = |n: &str, m: u64| (n.to_string(), m);
        assert_eq!(pick(&[d("acpi_video0", 15), d("intel_backlight", 19200)]).as_deref(), Some("intel_backlight"));
        assert_eq!(pick(&[d("broken", 0)]), None);
        assert_eq!(pick(&[]), None);
    }

    #[test]
    fn reads_a_backlight_folder() {
        let d = std::env::temp_dir().join(format!("tessera-ui-bl-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("max_brightness"), "400\n").unwrap();
        std::fs::write(d.join("brightness"), "100\n").unwrap();
        let r = reading(&d);
        assert!(r.present);
        assert_eq!(r.value, 0.25);
        std::fs::write(d.join("max_brightness"), "0\n").unwrap();
        assert!(!reading(&d).present);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A fake /sys/class/backlight: (name, max, its connector disabled)
    fn class(tag: &str, devs: &[(&str, &str, bool)]) -> PathBuf {
        let base = std::env::temp_dir().join(format!("tessera-ui-blc-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        for (n, max, off) in devs {
            let d = base.join(n);
            std::fs::create_dir_all(d.join("device")).unwrap();
            std::fs::write(d.join("max_brightness"), format!("{max}\n")).unwrap();
            std::fs::write(d.join("brightness"), "1\n").unwrap();
            if *off {
                std::fs::write(d.join("device/enabled"), "disabled\n").unwrap();
            }
        }
        base
    }

    #[test]
    fn several_none_or_odd_backlights() {
        // a desktop: the class folder is empty (or missing)
        let b = class("none", &[]);
        assert_eq!(find_in(&b), None);
        assert_eq!(find_in(&b.join("missing")), None);
        // intel + ACPI + NVIDIA's: the finest steps
        let b = class("three", &[("acpi_video0", "15", false), ("intel_backlight", "96000", false), ("nvidia_0", "100", false)]);
        assert_eq!(find_in(&b).unwrap().file_name().unwrap(), "intel_backlight");
        // the NVIDIA card drives the panel: intel's connector is disabled
        let b = class("mux", &[("intel_backlight", "96000", true), ("nvidia_0", "100", false)]);
        assert_eq!(find_in(&b).unwrap().file_name().unwrap(), "nvidia_0");
        // every one says disabled (a lid closed): still one
        let b = class("alloff", &[("intel_backlight", "96000", true)]);
        assert_eq!(find_in(&b).unwrap().file_name().unwrap(), "intel_backlight");
        // a broken driver's max of 0 or garbage: none
        let b = class("broken", &[("amdgpu_bl0", "0", false), ("x", "garbage", false)]);
        assert_eq!(find_in(&b), None);
        for t in ["none", "three", "mux", "alloff", "broken"] {
            let _ = std::fs::remove_dir_all(std::env::temp_dir().join(format!("tessera-ui-blc-{t}-{}", std::process::id())));
        }
    }
}

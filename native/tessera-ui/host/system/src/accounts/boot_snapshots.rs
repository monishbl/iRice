//! BootSnapshots.qml: snapshots at startup (Settings → Software Update →
//! Going back): whether this boot started from a snapshot (GRUB's snapshot
//! menu, grub-btrfs) and whether that menu is kept up to date. Started from
//! one, the island says so once, 15 s after the desktop is up.
//!
//! Bus "bootsnapshots": checked, booted, btrfs, grub, grubBtrfs, menuOn,
//! snapper, assistant, canMenu, busy, error. Calls: bootsnapshots.refresh|
//! setMenu [on]|openAssistant|updatesFinished.
//! The probe runs at start + 5 s (Startup.later: after the desktop is up).

use super::common::*;
use serde_json::{json, Value};
use std::any::Any;
use std::time::Duration;
use tessera_core::host::{Host, Service};

enum M {
    Probe(String),
    Menu(i32, String),
    Start,
    Say,
}

#[derive(Default)]
pub struct BootSnapshots {
    tx: Option<Tx>,
    checked: bool,
    booted: String,
    flags: [bool; 6], // btrfs grub grubBtrfs menuOn snapper assistant
    busy: bool,
    error: String,
    probing: bool,
    start_t: slint::Timer,
    say_t: slint::Timer,
}

const KEYS: [&str; 6] = ["btrfs", "grub", "grubBtrfs", "menuOn", "snapper", "assistant"];

/// The snapshot a boot started from: snapper's number, or a timeshift date
pub fn booted_from(where_: &str) -> String {
    if let Some(i) = where_.find("snapshots/") {
        let rest = &where_[i + 10..];
        let num: String = rest.chars().take_while(char::is_ascii_digit).collect();
        if !num.is_empty() && rest[num.len()..].starts_with("/snapshot") {
            return num;
        }
    }
    if let Some(i) = where_.find("timeshift-btrfs/snapshots/") {
        let rest = &where_[i + 26..];
        if let Some(end) = rest.find(['/', ',']).filter(|e| *e > 0) {
            let name = &rest[..end];
            if !name.contains(char::is_whitespace) && rest[end..].starts_with('/') {
                return name.to_string();
            }
        }
    }
    String::new()
}

impl BootSnapshots {
    fn tx(&self) -> Tx {
        self.tx.clone().expect("started")
    }
    fn probe(&mut self) {
        if self.probing {
            return;
        }
        self.probing = true;
        let script = [
            "echo \"cmdline:$(cat /proc/cmdline 2>/dev/null)\"",
            "echo \"root:$(findmnt -no FSTYPE,OPTIONS / 2>/dev/null | tr -s ' ' ' ')\"",
            "{ findmnt -no FSTYPE / | grep -q btrfs || grep -q 'subvol=' /proc/cmdline; } 2>/dev/null && echo btrfs",
            "{ test -d /boot/grub || test -d /boot/grub2; } && command -v grub-mkconfig >/dev/null && echo grub",
            // (grub-btrfsd itself is 0744, root's only: its GRUB script says it's there)
            "{ test -e /etc/grub.d/41_snapshots-btrfs || test -e /usr/bin/grub-btrfsd; } && echo grubBtrfs",
            "systemctl is-enabled --quiet grub-btrfsd 2>/dev/null && echo menuOn",
            "test -f /etc/snapper/configs/root && echo snapper",
            "command -v btrfs-assistant >/dev/null && echo assistant",
            ":",
        ]
        .join("; ");
        let tx = self.tx();
        std::thread::spawn(move || {
            let (_, out, _) = run_full(&strs(&["sh", "-c", &script]), None);
            tx.send(M::Probe(out));
        });
    }
    fn publish(&self) {
        let [btrfs, grub, grub_btrfs, menu_on, snapper, assistant] = self.flags;
        put(
            "bootsnapshots",
            json!({"checked": self.checked, "booted": self.booted, "btrfs": btrfs, "grub": grub, "grubBtrfs": grub_btrfs, "menuOn": menu_on,
                   "snapper": snapper, "assistant": assistant, "canMenu": btrfs && grub && snapper, "busy": self.busy, "error": self.error}),
        );
    }
}

impl Service for BootSnapshots {
    fn name(&self) -> &'static str {
        "BootSnapshots"
    }
    fn calls(&self) -> &'static [&'static str] {
        &["bootsnapshots"]
    }
    fn start(&mut self, h: &mut Host) {
        let tx = Tx::new(h.sender("BootSnapshots"));
        self.tx = Some(tx.clone());
        self.publish();
        self.start_t.start(slint::TimerMode::SingleShot, Duration::from_secs(5), move || tx.send(M::Start));
    }
    fn call(&mut self, _h: &mut Host, call: &str, args: &Value) {
        match call.split('.').nth(1).unwrap_or("") {
            "refresh" | "updatesFinished" => self.probe(), // (grub-btrfs or Btrfs Assistant may have come)
            "setMenu" => {
                if self.busy {
                    return;
                }
                self.error.clear();
                if !has_tool("pkexec") {
                    self.error = NO_PKEXEC.into();
                    self.publish();
                    return;
                }
                self.busy = true;
                let on = args.get(0).and_then(Value::as_bool).unwrap_or(false);
                let cmd = root_command(&["bootmenu", if on { "on" } else { "off" }]);
                let tx = self.tx();
                std::thread::spawn(move || {
                    let (code, _, err) = run_full(&cmd, None);
                    tx.send(M::Menu(code, err));
                });
                self.publish();
            }
            // Btrfs Assistant restores the snapshot you started from for good
            "openAssistant" => {
                std::thread::spawn(|| {
                    let _ = std::process::Command::new("btrfs-assistant").stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn();
                });
            }
            _ => {}
        }
    }
    fn message(&mut self, h: &mut Host, msg: Box<dyn Any + Send>) {
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Start => self.probe(),
            M::Probe(text) => {
                self.probing = false;
                let lines: Vec<&str> = text.lines().collect();
                for (i, k) in KEYS.iter().enumerate() {
                    self.flags[i] = lines.contains(k);
                }
                let where_ = lines.iter().filter(|l| l.starts_with("cmdline:") || l.starts_with("root:")).cloned().collect::<Vec<_>>().join(" ");
                let was = self.booted.clone();
                self.booted = booted_from(&where_);
                if was.is_empty() && !self.booted.is_empty() {
                    // Said once a boot, 15 s after the check
                    let tx = self.tx();
                    self.say_t.start(slint::TimerMode::SingleShot, Duration::from_secs(15), move || tx.send(M::Say));
                }
                self.checked = true;
            }
            M::Say => {
                if self.booted.is_empty() {
                    return;
                }
                island_post(
                    h,
                    json!({"kind": "custom", "icon": "clock-counter-clockwise", "title": "Started from a snapshot",
                           "subtitle": format!("Your system is as it was at snapshot {}. Keep it, or restart to go back to now.", self.booted),
                           "color": "accent", "action": "Open Settings", "onAction": {"ask": "panels.openSettings", "args": ["update"]}}),
                    15000,
                );
            }
            M::Menu(code, err) => {
                self.busy = false;
                if code != 0 && code != 126 {
                    self.error = if code == 127 {
                        "Not allowed (an administrator's password is needed).".into()
                    } else {
                        let m = root_message(&err);
                        if m.is_empty() { "That didn't change. Update Tessera, then try again.".into() } else { m }
                    };
                }
                self.probe();
            }
        }
        self.publish();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn which_snapshot_a_boot_started_from() {
        assert_eq!(booted_from("cmdline:root=x rootflags=subvol=@/.snapshots/412/snapshot ro"), "412");
        assert_eq!(booted_from("root:btrfs rw,subvol=/timeshift-btrfs/snapshots/2024-01-02_10-00-00/@"), "2024-01-02_10-00-00");
        assert_eq!(booted_from("cmdline:root=x subvol=@"), "");
    }
}

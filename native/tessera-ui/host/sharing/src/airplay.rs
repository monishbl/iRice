//! AirPlay speakers (HomePod, Apple TV, AirPlay speakers) as outputs, with
//! PipeWire's own AirPlay support (RAOP, the pipewire-zeroconf package)
//! (shell/services/AirPlay.qml): a small PipeWire client of its own (assets/
//! pipewire/tessera-airplay.conf) finds them over mDNS and adds each as an
//! output. It runs as a user unit (tessera-airplay, so it outlives a shell
//! restart), or as the shell's own child without systemd. Off until Settings
//! -> Sound turns it on (audio.airplay). The speakers themselves are
//! PipeWire outputs whose node name starts with `raop_sink.` (`is_airplay`):
//! Audio's output list is host-system's.

use crate::proc::{spawn, Kept};
use crate::{Job, Sharing};
use serde_json::{json, Value};
use tessera_core::host::Host;

#[derive(Default)]
pub struct AirPlay {
    pub checked: bool,
    /// pipewire-zeroconf's raop modules
    pub installed: bool,
    pub running: bool,
    pub error: String,
    pub child: Kept,
    pub starting: bool,
    pub install_error: String,
    pub last_wanted: bool,
}

/// A PipeWire node made by the raop module
pub fn is_airplay(node_name: &str) -> bool {
    node_name.starts_with("raop_sink.")
}

const UNIT: &str = "tessera-airplay";

impl Sharing {
    fn air_wanted(&self) -> bool {
        self.cfg_bool("audio.airplay", false)
    }
    fn air_conf(&self) -> String {
        tessera_core::util::asset("shell/assets/pipewire/tessera-airplay.conf").map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|| "tessera-airplay.conf".into())
    }
    pub fn air_check(&mut self, h: &mut Host) {
        let cmd = crate::proc::s(&["sh", "-c", "test -e /usr/lib/pipewire-0.3/libpipewire-module-raop-discover.so && echo installed; systemctl --user is-active --quiet \"$1\" 2>/dev/null && echo running; :", "sh", UNIT]);
        self.run(h, Job::AirCheck, &cmd, None);
    }
    pub fn air_checked(&mut self, h: &mut Host, text: &str) {
        self.air.installed = text.contains("installed");
        self.air.running = text.contains("running") || self.air.child.running;
        self.air.checked = true;
        self.air_apply(h);
    }
    /// onWantedChanged (each turn)
    pub fn air_sync(&mut self, h: &mut Host) {
        let w = self.air_wanted();
        if w != self.air.last_wanted {
            self.air.last_wanted = w;
            self.air_apply(h);
        }
    }
    /// On when wanted and installed, off otherwise
    fn air_apply(&mut self, h: &mut Host) {
        if !self.air.checked {
            return;
        }
        let wanted = self.air_wanted();
        if wanted && self.air.installed && !self.air.running && !self.air.starting {
            self.air_start(h);
        } else if !wanted && self.air.running {
            self.air_stop();
        }
    }
    fn air_start(&mut self, h: &mut Host) {
        self.air.error.clear();
        self.air.starting = true;
        let conf = self.air_conf();
        // (100: no systemd user manager. A unit left over, restarting or
        // stopping, is cleared first: one copy only)
        let cmd = crate::proc::s(&["sh", "-c", "systemctl --user show-environment >/dev/null 2>&1 || exit 100; \
            systemctl --user stop \"$1\" irice-airplay 2>/dev/null; systemctl --user reset-failed \"$1\" 2>/dev/null; \
            exec systemd-run --user --quiet --collect --unit=\"$1\" -p Restart=on-failure -p RestartSec=5 pipewire -c \"$2\"", "sh", UNIT, &conf]);
        self.run(h, Job::AirStart, &cmd, None);
    }
    pub fn air_started(&mut self, h: &mut Host, stderr: &str, code: i32) {
        self.air.starting = false;
        if code == 0 {
            self.air.running = true;
        } else if code == 100 {
            // (the shell runs it itself)
            let conf = self.air_conf();
            let gen = crate::proc::next_gen();
            if let Some(k) = spawn(h.sender("Sharing"), "airplay", gen, &crate::proc::s(&["pipewire", "-c", &conf]), &[], false, true) {
                self.air.child = k;
            }
            self.air.running = true;
        } else {
            let last = stderr.trim().lines().last().unwrap_or("").to_string();
            self.air.error = if last.is_empty() { "AirPlay couldn't start.".into() } else { last };
        }
    }
    pub fn air_child_exit(&mut self, gen: u64, code: i32, stderr: &str) {
        if gen != self.air.child.gen {
            return;
        }
        self.air.child.ended();
        if self.air_wanted() && code != 0 {
            let last = stderr.trim().lines().last().unwrap_or("").trim().to_string();
            let msg = if last.is_empty() { "AirPlay stopped.".to_string() } else { last };
            // "[E][12345.6] " log prefix off
            self.air.error = strip_log_prefix(&msg);
        }
        self.air.running = false;
    }
    fn air_stop(&mut self) {
        self.air.child.signal(libc::SIGTERM);
        // (irice-airplay: its unit when it was started before iRice became Tessera)
        crate::proc::detach(&["systemctl", "--user", "stop", UNIT, "irice-airplay"]);
        self.air.running = false;
    }
    /// pipewire-zeroconf, through Software Update's job
    pub fn air_install(&mut self) {
        if self.upd.running() || self.air.installed {
            return;
        }
        self.air.install_error.clear();
        self.update_run("airplay", &["--install", "pipewire-zeroconf"]);
    }
    pub fn air_state(&self) -> Value {
        json!({"wanted": self.air_wanted(), "checked": self.air.checked, "installed": self.air.installed, "running": self.air.running, "error": self.air.error,
            "installing": self.upd.installing("airplay"), "installError": self.air.install_error})
    }
}

/// `[E][123.4] text` -> `text`
fn strip_log_prefix(s: &str) -> String {
    let mut rest = s.trim_start();
    for _ in 0..2 {
        if let Some(r) = rest.strip_prefix('[') {
            if let Some(end) = r.find(']') {
                rest = &r[end + 1..];
                continue;
            }
        }
        return s.to_string();
    }
    rest.trim_start().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speakers_are_raop_sinks() {
        assert!(is_airplay("raop_sink.HomePod.local.192.168.0.4.7000"));
        assert!(!is_airplay("alsa_output.pci-0000"));
    }

    #[test]
    fn pipewire_log_prefix_is_dropped() {
        assert_eq!(strip_log_prefix("[E][00123.456] pw.conf: can't load"), "pw.conf: can't load");
        assert_eq!(strip_log_prefix("plain"), "plain");
    }
}

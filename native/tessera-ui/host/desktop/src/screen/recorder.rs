//! Screen recording (shell/services/Recorder.qml). gpu-screen-recorder
//! (hardware encoding, system audio and the microphone mixed into one track,
//! pause) when it's installed, else wf-recorder (one audio source, no
//! pause). Started from the recording toolbar (Super+Shift+5, Control
//! Center's Record), Super+Shift+R (an area), Spotlight; the island shows it
//! live with Stop, Pause, mic mute and Discard (the host state `recorder`).
//!
//! `qs ipc call recorder start|stop|pause|discard|undo|toolbar|options|screen`
//! (here also `toggleMic`, `open <file>`, `showInFolder <file>`).
//! Differences: the microphone's level meter (PipeWire peak) isn't read.

use super::service::*;
use super::util;
use serde_json::{json, Value};
use std::process::{Command, Stdio};
use tessera_core::host::Host;

/// Is this stderr gpu-screen-recorder's "can't encode on the GPU"?
pub fn hw_encode_failure(text: &str) -> bool {
    let t = text.to_lowercase();
    ["vaapi", "va-api", "libva", "failed to create", "no h264", "nvenc", "cuda", "vulkan video", "encoder", "encoding"].iter().any(|k| t.contains(k))
        || t.lines().any(|l| ["h264", "hevc", "av1"].iter().any(|c| l.contains(c)) && (l.contains("not supported") || l.contains("unavailable")))
}

/// "x,y wxh" → gpu-screen-recorder's -region WxH+X+Y
pub fn gsr_region(region: &str) -> String {
    let parse = || -> Option<String> {
        let (pos, size) = region.split_once(' ')?;
        let (x, y) = pos.split_once(',')?;
        let (w, h) = size.split_once('x')?;
        for n in [x, y, w, h] {
            n.parse::<i64>().ok()?;
        }
        Some(format!("{w}x{h}+{x}+{y}"))
    };
    parse().unwrap_or_else(|| region.to_string())
}

const CPU_NOTE: &str = "Hardware video encoding isn't available (install intel-media-driver for Intel or libva-nvidia-driver for NVIDIA); recorded with the CPU instead";

impl ScreenSvc {
    fn rec_cfg_str(h: &Host, k: &str, d: &str) -> String {
        h.cfg()["recorder"][k].as_str().unwrap_or(d).to_string()
    }

    /// Spotlight / Control Center's Record: the toolbar with the recording tools
    pub(super) fn open_toolbar(&mut self, h: &mut Host) {
        if self.recording {
            return self.stop(h);
        }
        self.toolbar_record = true;
        self.open_toolbar_panel(h);
    }

    /// The recording options (frame rate, format, quality, sound, countdown,
    /// folder) live in the toolbar's Options: this opens it with them showing
    pub(super) fn open_options(&mut self, h: &mut Host) {
        if self.recording {
            return;
        }
        self.toolbar_options = true;
        self.toolbar_record = true;
        self.open_toolbar_panel(h);
    }

    fn open_toolbar_panel(&mut self, h: &mut Host) {
        // said before the panel opens (RustBridge.qml's onOpenChanged)
        h.say(json!({"cmd": "options", "surface": "screentools", "open": self.toolbar_options, "record": self.toolbar_record}));
        self.toolbar_options = false;
        self.refresh_mics(h);
        let scr = focused(h).map(|s| s.name).unwrap_or_default();
        h.ask("panels.show", json!(["screenTools", scr]));
    }

    /// Start what the toolbar has selected
    pub(super) fn record_area(&mut self, h: &mut Host, area: &str) {
        util::cfg_set("recorder.area", json!(area));
        if area == "screen" {
            self.record_screen(h);
        } else {
            self.select(h, if area == "window" { "record-window" } else { "record" });
        }
    }

    pub(super) fn record_screen(&mut self, h: &mut Host) {
        h.ask("panels.closeOpen", json!([]));
        let out = focused(h).map(|s| s.name).unwrap_or_default();
        self.start(h, json!({"output": out}));
    }

    /// A region was chosen to record (ScreenTools.startRecording)
    pub(super) fn start_recording(&mut self, h: &mut Host, geo: String) {
        self.start(h, json!({"region": geo}));
    }

    // ── Microphones ─────────────────────────────────────────────────────
    pub(super) fn refresh_mics(&mut self, h: &mut Host) {
        let send = h.sender("ScreenTools");
        std::thread::spawn(move || {
            let out = Command::new("pactl").args(["-f", "json", "list", "sources"]).stdin(Stdio::null()).stderr(Stdio::null()).output();
            let list: Vec<Value> = out
                .ok()
                .and_then(|o| serde_json::from_slice::<Value>(&o.stdout).ok())
                .and_then(|v| v.as_array().cloned())
                .map(|a| {
                    a.into_iter()
                        .filter(|s| s["monitor_source"].as_str().map_or(true, |m| m.is_empty()) && !s["name"].as_str().unwrap_or("").ends_with(".monitor"))
                        .map(|s| {
                            let name = s["name"].as_str().unwrap_or("").to_string();
                            let d = s["description"].as_str().filter(|d| !d.is_empty()).unwrap_or(&name).to_string();
                            json!({"name": name, "description": d})
                        })
                        .collect()
                })
                .unwrap_or_default();
            send(Box::new(Mics(list)));
        });
    }

    /// Mute / unmute the microphone being recorded (restored when it ends)
    pub(super) fn toggle_mic(&mut self, h: &mut Host) {
        if !self.recording || Self::rec_cfg_str(h, "mic", "").is_empty() {
            return;
        }
        let mic = Self::rec_cfg_str(h, "mic", "");
        let target = if mic == "default" { "@DEFAULT_AUDIO_SOURCE@".to_string() } else { mic };
        util::detached(&["wpctl", "set-mute", &target, "toggle"]);
        self.mic_muted = !self.mic_muted;
    }

    fn restore_mic(&mut self, h: &mut Host) {
        // (what it was before we touched it: unmuted if we muted it)
        if self.mic_muted {
            let mic = Self::rec_cfg_str(h, "mic", "");
            let target = if mic == "default" || mic.is_empty() { "@DEFAULT_AUDIO_SOURCE@".to_string() } else { mic };
            util::detached(&["wpctl", "set-mute", &target, "0"]);
        }
        self.mic_muted = false;
    }

    // ── Countdown ───────────────────────────────────────────────────────
    pub(super) fn cancel_countdown(&mut self) {
        self.tick_gen += 1;
        self.countdown = 0;
        self.pending = None;
    }

    pub(super) fn tick(&mut self, h: &mut Host, gen: u64) {
        if gen != self.tick_gen || self.countdown <= 0 {
            return;
        }
        self.countdown -= 1;
        if self.countdown <= 0 {
            self.tick_gen += 1;
            if let Some(p) = self.pending.take() {
                self.launch(h, p, false, false);
            }
        } else {
            util::later(h.sender("ScreenTools"), 1000, Box::new(Tick(gen)));
        }
    }

    // ── Start / stop ────────────────────────────────────────────────────
    /// what: { output: "DP-1" } (a whole display) or { region: "x,y wxh" }
    pub(super) fn start(&mut self, h: &mut Host, what: Value) {
        if self.recording || self.countdown > 0 || !self.available() {
            return;
        }
        h.ask("panels.closeOpen", json!([]));
        let n = h.cfg()["recorder"]["countdown"].as_i64().unwrap_or(0).max(0);
        if n > 0 {
            self.pending = Some(what);
            self.countdown = n;
            self.tick_gen += 1;
            util::later(h.sender("ScreenTools"), 1000, Box::new(Tick(self.tick_gen)));
        } else {
            util::later(h.sender("ScreenTools"), 0, Box::new(StartRec(what)));
        }
    }

    /// Without hardware encoding (Intel without intel-media-driver, NVIDIA
    /// without libva-nvidia-driver, a VM) gpu-screen-recorder exits at once;
    /// launch() then runs it again once with the CPU encoder (H.264 only)
    pub(super) fn launch(&mut self, h: &mut Host, what: Value, cpu: bool, keep_file: bool) {
        if what.is_null() {
            return;
        }
        self.last_what = Some(what.clone());
        self.cpu_encoding = cpu;
        let folder = self.folder(h);
        if !keep_file {
            self.file = format!("{}/Screen Recording {}.mp4", folder, util::stamp(false));
        }
        let cfg = h.cfg()["recorder"].clone();
        let mic = cfg["mic"].as_str().unwrap_or("").to_string();
        let fps = cfg["fps"].as_i64().filter(|f| *f > 0).unwrap_or(60).to_string();
        let mut audio: Vec<String> = Vec::new();
        if cfg["systemAudio"].as_bool().unwrap_or(true) {
            audio.push("default_output".into());
        }
        if !mic.is_empty() {
            audio.push(if mic == "default" { "default_input".into() } else { mic.clone() });
        }
        let mut cmd: Vec<String>;
        if self.gsr() {
            cmd = vec!["gpu-screen-recorder".into()];
            if let Some(region) = what["region"].as_str() {
                cmd.extend(["-w".into(), "region".into(), "-region".into(), gsr_region(region)]);
            } else {
                cmd.extend(["-w".into(), what["output"].as_str().filter(|o| !o.is_empty()).unwrap_or("screen").to_string()]);
            }
            let codec = if cpu { "h264".to_string() } else { cfg["codec"].as_str().filter(|c| !c.is_empty()).unwrap_or("h264").to_string() };
            // the CPU encoder only does H.264
            cmd.extend([
                "-f".into(), fps, "-k".into(), codec, "-q".into(), cfg["quality"].as_str().filter(|c| !c.is_empty()).unwrap_or("high").to_string(),
                "-c".into(), "mp4".into(), "-cursor".into(), if cfg["cursor"].as_bool().unwrap_or(true) { "yes".into() } else { "no".into() },
            ]);
            if cpu {
                cmd.extend(["-encoder".into(), "cpu".into()]);
            }
            if !audio.is_empty() {
                cmd.extend(["-a".into(), audio.join("|")]); // one track, mixed
            }
            cmd.extend(["-o".into(), self.file.clone()]);
        } else {
            // wf-recorder: one audio source (the microphone if chosen, else
            // what's playing, through the output's monitor)
            cmd = vec!["wf-recorder".into(), "-r".into(), fps, "-f".into(), self.file.clone()];
            if let Some(region) = what["region"].as_str() {
                cmd.extend(["-g".into(), region.into()]);
            } else if let Some(o) = what["output"].as_str().filter(|o| !o.is_empty()) {
                cmd.extend(["-o".into(), o.into()]);
            }
            if !mic.is_empty() {
                cmd.push(format!("--audio={}", if mic == "default" { "default" } else { mic.as_str() }));
            } else if cfg["systemAudio"].as_bool().unwrap_or(true) {
                let sink = Command::new("pactl").arg("get-default-sink").stdin(Stdio::null()).stderr(Stdio::null()).output().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
                cmd.push(format!("--audio={}", if sink.is_empty() { "default".to_string() } else { format!("{sink}.monitor") }));
            }
        }
        // (setpriv: if the shell dies, the recording is finished and stops
        // with it, rather than carrying on with no indicator and filling the disk)
        let mut full: Vec<String> = ["setpriv", "--pdeathsig", "INT", "--", "sh", "-c", "mkdir -p \"$1\" && shift && exec \"$@\"", "sh"].iter().map(|s| s.to_string()).collect();
        full.push(folder);
        full.extend(cmd);
        self.discarding = false;
        self.paused = false;
        let child = Command::new(&full[0]).args(&full[1..]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped()).spawn();
        let mut child = match child {
            Ok(c) => c,
            Err(_) => return self.rec_failed(h, "The recorder couldn't be started. Try again, or check that the display is still connected."),
        };
        self.rec_gen += 1;
        let gen = self.rec_gen;
        self.rec_pid = Some(child.id() as i32);
        self.recording = true;
        self.started = tessera_core::util::now_ms();
        if self.island_says(h) {
            util::island_post(h, json!({"kind": "recording", "icon": "record", "title": "Recording", "subtitle": "Stop it here or with Super+Shift+R", "color": "danger"}), 3000);
        }
        let send = h.sender("ScreenTools");
        std::thread::spawn(move || {
            use std::io::Read;
            let mut err = String::new();
            if let Some(mut e) = child.stderr.take() {
                let _ = e.read_to_string(&mut err);
            }
            let code = child.wait().ok().and_then(|s| s.code()).unwrap_or(-1);
            send(Box::new(RecExit(gen, code, err)));
        });
    }

    pub(super) fn stop(&mut self, _h: &mut Host) {
        if self.countdown > 0 {
            return self.cancel_countdown();
        }
        let Some(p) = self.rec_pid else { return };
        if self.paused && self.gsr() {
            unsafe { libc::kill(p, libc::SIGUSR2) }; // resume first, so the end is written
        }
        unsafe { libc::kill(p, libc::SIGINT) }; // stop and finish the file
    }

    pub(super) fn toggle_pause(&mut self, _h: &mut Host) {
        let Some(p) = self.rec_pid else { return };
        if !self.gsr() {
            return;
        }
        unsafe { libc::kill(p, libc::SIGUSR2) }; // pause / resume
        let now = tessera_core::util::now_ms();
        if self.paused {
            self.started += now - self.paused_at;
        } else {
            self.paused_at = now;
        }
        self.paused = !self.paused;
    }

    /// Discard: stop, and throw the file away unless Undo is pressed within 8 s
    pub(super) fn discard(&mut self, h: &mut Host) {
        if self.rec_pid.is_none() {
            return;
        }
        self.discarding = true;
        self.stop(h);
    }

    pub(super) fn undo_discard(&mut self, h: &mut Host) {
        let f = std::mem::take(&mut self.discarded);
        if !f.is_empty() {
            self.saved(h, &f);
        }
    }

    pub(super) fn drop_now(&mut self, file: String) {
        if self.discarded == file && !file.is_empty() {
            util::detached(&["rm", "-f", "--", &file]);
            self.discarded.clear();
        }
    }

    fn island_says(&self, h: &Host) -> bool {
        let i = &h.cfg()["island"];
        i["enabled"].as_bool().unwrap_or(true) && i["events"].as_array().is_some_and(|e| e.iter().any(|x| x == "recording"))
    }

    pub(super) fn rec_exited(&mut self, h: &mut Host, gen: u64, code: i32, err: String) {
        if gen != self.rec_gen {
            return;
        }
        self.rec_pid = None;
        self.restore_mic(h);
        self.recording = false;
        self.paused = false;
        let f = self.file.clone();
        if self.discarding {
            self.discarding = false;
            // a second Discard within the Undo time: the first one is final now
            if !self.discarded.is_empty() && self.discarded != f {
                util::detached(&["rm", "-f", "--", &self.discarded.clone()]);
            }
            self.discarded = f.clone();
            util::later(h.sender("ScreenTools"), 8000, Box::new(DropNow(f.clone())));
            self.dropped(h, &f);
        } else if code != 0 && code != 130 && code != 255 {
            // no hardware encoder: once more on the CPU, same file
            if self.gsr() && !self.cpu_encoding && tessera_core::util::now_ms() - self.started < 10000 && hw_encode_failure(&err) {
                eprintln!("Recorder: hardware encoding failed, retrying on the CPU: {}", err.trim().lines().last().unwrap_or(""));
                if let Some(w) = self.last_what.clone() {
                    return self.launch(h, w, true, true);
                }
            }
            let e = err.lines().filter(|l| !l.trim().is_empty()).last().unwrap_or("").to_string();
            if self.cpu_encoding && hw_encode_failure(&err) {
                self.rec_failed(h, "Hardware video encoding isn't available and the CPU encoder failed too. Install intel-media-driver (Intel) or libva-nvidia-driver (NVIDIA), or install wf-recorder.");
            } else if !e.is_empty() {
                self.rec_failed(h, &e);
            } else {
                self.rec_failed(h, &format!("The recorder stopped (code {code}). Try again, or check that the display is still connected."));
            }
        } else {
            self.saved(h, &f);
            if self.cpu_encoding {
                if self.island_says(h) {
                    util::island_post(h, json!({"kind": "recording", "icon": "info", "title": "Recorded with the CPU", "subtitle": "Install intel-media-driver or libva-nvidia-driver", "color": "orange"}), 8000);
                } else {
                    util::notify("Screen Recording", "Recorded with the CPU", CPU_NOTE);
                }
            }
        }
    }

    fn base(f: &str) -> &str {
        f.rsplit('/').next().unwrap_or(f)
    }

    /// Recorder.saved: the island says it, else a notification with Open / Show in Folder
    fn saved(&mut self, h: &mut Host, f: &str) {
        if self.island_says(h) {
            util::island_post(h, json!({"kind": "recording", "icon": "record", "title": "Recording saved", "subtitle": Self::base(f), "color": "danger",
                "more": [{"text": "Open", "call": "recorder.open", "args": [f]}, {"text": "Show in Folder", "call": "recorder.showInFolder", "args": [f]}]}), 6000);
            return;
        }
        let m = util::file_manager(h.cfg());
        util::detached(&[
            "sh", "-c",
            "a=$(timeout 600 notify-send -a 'Screen Recording' -i video-x-generic -A open=Open -A folder='Show in Folder' 'Recording saved' \"$(basename \"$1\")\"); [ \"$a\" = open ] && xdg-open \"$1\"; [ \"$a\" = folder ] && ${2:-xdg-open} \"$(dirname \"$1\")\"",
            "sh", f, &m,
        ]);
    }

    fn rec_failed(&mut self, h: &mut Host, why: &str) {
        if self.island_says(h) {
            util::island_post(h, json!({"kind": "recording", "icon": "warning-circle", "title": "Recording didn't work", "subtitle": why, "color": "danger"}), 6000);
        } else {
            util::detached(&["notify-send", "-a", "Screen Recording", "-u", "critical", "Recording didn't work", why]);
        }
    }

    fn dropped(&mut self, h: &mut Host, f: &str) {
        if self.island_says(h) {
            util::island_post(h, json!({"kind": "recording", "icon": "trash", "title": "Recording discarded", "subtitle": Self::base(f), "color": "danger",
                "more": [{"text": "Undo", "call": "recorder.undo", "args": []}]}), 8000);
            return;
        }
        // (Undo asks the shell back: shell-ipc.sh call recorder undo)
        let ipc = util::script("shell-ipc.sh");
        util::detached(&[
            "sh", "-c",
            "a=$(timeout 600 notify-send -a 'Screen Recording' -t 8000 -A undo=Undo 'Recording discarded' \"$(basename \"$1\")\"); [ \"$a\" = undo ] && \"$2\" call recorder undo",
            "sh", f, &ipc,
        ]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regions_become_gsr_regions() {
        assert_eq!(gsr_region("100,50 800x600"), "800x600+100+50");
        assert_eq!(gsr_region("-1920,0 1920x1080"), "1920x1080+-1920+0");
        assert_eq!(gsr_region("odd"), "odd");
    }

    #[test]
    fn missing_encoders_are_told_apart() {
        assert!(hw_encode_failure("Error: failed to create vaapi context"));
        assert!(!hw_encode_failure("permission denied"));
    }
}

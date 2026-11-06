//! An Android phone on this computer (shell/services/Phone.qml; assets/
//! scripts/phone.py): its screen in a window you control with the mouse and
//! keyboard (scrcpy; drop files on it to copy them to the phone's
//! Downloads), its camera as a webcam, in the Dock while it's connected, and
//! paired without a cable by QR code. Settings -> Phone sets it up.
//! Calls (all `share.phone*`, see lib.rs): phoneMirror, phoneWebcam,
//! phoneSetUpCam, phoneCheckCam, phonePair, phoneCancelPair, phoneInstall.

use crate::localsend::js;
use crate::proc::{json_line, spawn, Keep, Kept};
use crate::{Job, Sharing};
use serde_json::{json, Value};
use tessera_core::host::Host;

pub struct Phone {
    /// [{ serial, state, wireless, model, android, battery, charging }]
    pub devices: Vec<Value>,
    pub watch: Keep,
    /// serial -> its scrcpy
    pub mirrors: Vec<(String, Kept)>,
    /// the virtual camera exists
    pub cam_ready: bool,
    pub cam_module: bool,
    /// linux, linux-lts, linux-zen, linux-cachyos...
    pub kernel_package: String,
    pub cam: Kept,
    pub cam_error: String,
    pub cam_setting: bool,
    /// "" | starting | waiting (the QR is up) | pairing | connecting | done | error
    pub pair_stage: String,
    pub pair_qr: String,
    pub pair_error: String,
    pub pairer: Kept,
    pub install_error: String,
}

impl Default for Phone {
    fn default() -> Self {
        Self {
            devices: vec![],
            watch: Keep::new("phone-watch", "phone_rest"),
            mirrors: vec![],
            cam_ready: false,
            cam_module: false,
            kernel_package: String::new(),
            cam: Kept::default(),
            cam_error: String::new(),
            cam_setting: false,
            pair_stage: String::new(),
            pair_qr: String::new(),
            pair_error: String::new(),
            pairer: Kept::default(),
            install_error: String::new(),
        }
    }
}

fn last_line(s: &str, fallback: &str) -> String {
    let l = s.trim().lines().last().unwrap_or("").to_string();
    let l = l.strip_prefix("ERROR: ").or_else(|| l.strip_prefix("WARN: ")).unwrap_or(&l).to_string();
    if l.is_empty() { fallback.into() } else { l }
}

impl Sharing {
    pub fn has_adb(&self) -> bool {
        tessera_core::util::on_path("adb")
    }
    pub fn has_scrcpy(&self) -> bool {
        tessera_core::util::on_path("scrcpy")
    }
    pub fn phone_dev(&self) -> Option<Value> {
        self.phone.devices.iter().find(|d| d["state"] == "device").cloned()
    }
    pub fn phone_waiting(&self) -> Option<Value> {
        self.phone.devices.iter().find(|d| d["state"] == "unauthorized").cloned()
    }

    pub fn phone_sync(&mut self, h: &mut Host) {
        // (only once scrcpy is installed: until then nobody asked for it, and
        // adb's server would run for nothing)
        let wanted = self.cfg_bool("phone.enabled", true) && self.has_adb() && self.has_scrcpy();
        let cmd = vec!["python3".to_string(), "-I".into(), self.script("phone.py"), "watch".into()];
        self.phone.watch.sync(&mut self.sched, h.sender("Sharing"), wanted, &cmd, 5000, true);
        if !self.phone.watch.kept.running {
            self.phone.devices.clear(); // (it went away: nothing is known now)
        }
    }
    pub fn phone_watch_line(&mut self, h: &mut Host, gen: u64, line: &str) {
        if !self.phone.watch.is(gen) {
            return;
        }
        let Some(m) = json_line(line) else { return };
        let before: Vec<String> = self.phone.devices.iter().map(|d| format!("{}{}", js(&d["serial"]), js(&d["state"]))).collect();
        self.phone.devices = m["devices"].as_array().cloned().unwrap_or_default();
        for d in self.phone.devices.clone() {
            if !before.contains(&format!("{}{}", js(&d["serial"]), js(&d["state"]))) {
                self.phone_arrived(h, &d);
            }
        }
    }
    pub fn phone_watch_exit(&mut self, gen: u64) {
        let wanted = self.cfg_bool("phone.enabled", true) && self.has_adb() && self.has_scrcpy();
        if self.phone.watch.ended(&mut self.sched, gen, wanted, 5000) {
            self.phone.devices.clear();
        }
    }

    /// Said once per connection
    fn phone_arrived(&mut self, h: &mut Host, d: &Value) {
        if d["state"] == "unauthorized" {
            self.island(h, json!({"kind": "custom", "icon": "device-mobile", "title": "Allow this computer on your phone", "subtitle": "Tap Allow on the phone's USB debugging prompt"}), 8000);
        } else if d["state"] == "device" {
            let model = js(&d["model"]);
            let sub = match d["battery"].as_f64() {
                Some(b) => format!("{b}% battery"),
                None => "Connected".into(),
            };
            let scrcpy = self.has_scrcpy();
            self.island(
                h,
                json!({"kind": "custom", "icon": "device-mobile", "title": if model.is_empty() { "Phone connected".to_string() } else { model }, "subtitle": sub,
                    "action": if scrcpy {"Show Screen"} else {""}, "onAction": crate::act("share.phoneMirror", json!([js(&d["serial"])]))}),
                6000,
            );
        }
    }

    // ── Its screen (scrcpy) ─────────────────────────────────────────────
    pub fn phone_mirroring(&self, serial: &str) -> bool {
        self.phone.mirrors.iter().any(|(s, _)| s == serial)
    }
    pub fn phone_mirror(&mut self, h: &mut Host, serial: &str) {
        if !self.has_scrcpy() {
            h.ask("panels.openSettings", json!(["phone"]));
            return;
        }
        let Some(d) = self.phone.devices.iter().find(|d| js(&d["serial"]) == serial).cloned().or_else(|| self.phone_dev()) else { return };
        let ser = js(&d["serial"]);
        if let Some((_, k)) = self.phone.mirrors.iter().find(|(s, _)| *s == ser) {
            // (already up: bring it forward, found by its process)
            tessera_core::services::hypr::dispatch(format!("hl.dsp.focus({{ window = {} }})", json!(format!("pid:{}", k.pid))));
            return;
        }
        let model = js(&d["model"]);
        let mut cmd = crate::proc::s(&["scrcpy", "-s", &ser, "--window-title", if model.is_empty() { "Phone" } else { &model }, "--stay-awake", "--shortcut-mod=lalt,ralt"]);
        if self.cfg_bool("phone.screenOff", true) {
            cmd.push("--turn-screen-off".into());
        }
        let gen = crate::proc::next_gen();
        // (it ends with the shell, like the camera below)
        match spawn(h.sender("Sharing"), "scrcpy", gen, &cmd, &[], false, true) {
            Some(k) => self.phone.mirrors.push((ser, k)),
            None => {
                self.island(h, json!({"kind": "custom", "icon": "device-mobile", "title": "Couldn't show the phone's screen", "subtitle": "scrcpy stopped", "color": crate::DANGER}), 7000);
            }
        }
    }
    pub fn phone_mirror_exit(&mut self, h: &mut Host, gen: u64, code: i32, stderr: &str) {
        let Some(i) = self.phone.mirrors.iter().position(|(_, k)| k.gen == gen) else { return };
        self.phone.mirrors.remove(i);
        if code != 0 && code != 2 {
            // (2: the phone went away)
            self.island(h, json!({"kind": "custom", "icon": "device-mobile", "title": "Couldn't show the phone's screen", "subtitle": last_line(stderr, "scrcpy stopped"), "color": crate::DANGER}), 7000);
        }
    }

    // ── Its camera as a webcam (scrcpy into /dev/video42) ───────────────
    pub fn phone_webcam(&self) -> bool {
        self.phone.cam.running
    }
    pub fn phone_check_cam(&mut self, h: &mut Host) {
        // (the running kernel's package: its headers build v4l2loopback-dkms)
        let cmd = crate::proc::s(&["sh", "-c", "test -e /dev/video42 && echo ready; modinfo v4l2loopback >/dev/null 2>&1 && echo module; k=$(cat \"/usr/lib/modules/$(uname -r)/pkgbase\" 2>/dev/null || pacman -Qqo \"/usr/lib/modules/$(uname -r)/vmlinux\" 2>/dev/null) && echo \"kernel:$k\"; :"]);
        self.run(h, Job::CamCheck, &cmd, None);
    }
    pub fn phone_cam_checked(&mut self, text: &str) {
        self.phone.cam_ready = text.contains("ready");
        self.phone.cam_module = text.contains("module");
        self.phone.kernel_package = text.lines().find_map(|l| l.strip_prefix("kernel:")).map(|k| k.trim().to_string()).filter(|k| !k.contains(' ')).unwrap_or_default();
    }
    pub fn phone_set_webcam(&mut self, h: &mut Host, on: bool) {
        if !on {
            self.phone.cam.signal(libc::SIGTERM);
            return;
        }
        let Some(p) = self.phone_dev() else { return };
        if !self.phone.cam_ready || self.phone.cam.running {
            return;
        }
        let facing = self.cfg_str("phone.camera", "back");
        // (the phone's camera has no window: if the shell died it stayed on, unseen)
        let cmd = crate::proc::s(&["scrcpy", "-s", &js(&p["serial"]), "--video-source=camera", &format!("--camera-facing={}", if facing.is_empty() { "back" } else { &facing }), "--v4l2-sink=/dev/video42", "--no-playback", "--no-audio", "--no-window"]);
        let gen = crate::proc::next_gen();
        if let Some(k) = spawn(h.sender("Sharing"), "phone-cam", gen, &cmd, &[], false, true) {
            self.phone.cam = k;
            self.phone.cam_error.clear();
        }
    }
    pub fn phone_cam_exit(&mut self, gen: u64, code: i32, stderr: &str) {
        if gen != self.phone.cam.gen {
            return;
        }
        self.phone.cam.ended();
        if code != 0 && code != 2 {
            let low = stderr.to_lowercase();
            let api = low.contains("android 12") || low.contains("api") || (low.contains("camera") && low[low.find("camera").unwrap_or(0)..].contains("not supported"));
            self.phone.cam_error = if api { "The phone's camera can be used from Android 12 on.".into() } else { last_line(stderr, "The camera stopped.") };
        }
    }
    /// The virtual camera: one password (tessera-root phonecam on)
    pub fn phone_set_up_cam(&mut self, h: &mut Host) {
        if self.phone.cam_setting {
            return;
        }
        self.phone.cam_error.clear();
        if !tessera_core::util::on_path("pkexec") {
            self.phone.cam_error = "Changing this needs polkit (pkexec), which isn't installed. Install polkit, then try again.".into();
            return;
        }
        self.phone.cam_setting = true;
        let cmd = Self::root_command(&["phonecam", "on"]);
        self.run(h, Job::CamSetup, &cmd, None);
    }
    pub fn phone_cam_setup_done(&mut self, h: &mut Host, stderr: &str, code: i32) {
        self.phone.cam_setting = false;
        if code != 0 && code != 126 {
            let said = stderr.trim().trim_start_matches("tessera-root: ").trim_start_matches("irice-root: ").trim().to_string();
            self.phone.cam_error = if said.is_empty() { "The virtual camera couldn't be set up. Update Tessera, then try again.".into() } else { said };
        }
        self.phone_check_cam(h);
    }

    // ── Pairing without a cable (Android 11+) ───────────────────────────
    pub fn phone_pair(&mut self, h: &mut Host) {
        if self.phone.pairer.running {
            return;
        }
        self.phone.pair_stage = "starting".into();
        self.phone.pair_qr.clear();
        self.phone.pair_error.clear();
        let cmd = vec!["python3".to_string(), "-I".into(), self.script("phone.py"), "pair".into()];
        let gen = crate::proc::next_gen();
        match spawn(h.sender("Sharing"), "phone-pair", gen, &cmd, &[], false, true) {
            Some(k) => self.phone.pairer = k,
            None => self.phone.pair_stage.clear(),
        }
    }
    pub fn phone_cancel_pair(&mut self) {
        self.phone.pairer.signal(libc::SIGTERM);
        self.phone.pair_stage.clear();
    }
    pub fn phone_pair_line(&mut self, gen: u64, line: &str) {
        if gen != self.phone.pairer.gen {
            return;
        }
        let Some(m) = json_line(line) else { return };
        if let Some(qr) = m["qr"].as_str().filter(|q| !q.is_empty()) {
            self.phone.pair_qr = qr.into();
            self.phone.pair_stage = "waiting".into();
        } else if let Some(st) = m["stage"].as_str().filter(|q| !q.is_empty()) {
            self.phone.pair_stage = st.into();
        } else if let Some(ok) = m["ok"].as_bool() {
            self.phone.pair_stage = if ok { "done" } else { "error" }.into();
            self.phone.pair_error = js(&m["error"]);
        }
    }
    pub fn phone_pair_exit(&mut self, gen: u64) {
        if gen != self.phone.pairer.gen {
            return;
        }
        self.phone.pairer.ended();
        let st = self.phone.pair_stage.as_str();
        if st == "waiting" || st == "starting" {
            self.phone.pair_stage.clear();
        } else if st == "pairing" || st == "connecting" {
            // (it ended without saying how it went)
            self.phone.pair_stage = "error".into();
            self.phone.pair_error = "Pairing stopped. Try again.".into();
        }
    }

    // ── Installing scrcpy (and adb, and the camera module) ──────────────
    pub fn phone_install(&mut self, with_camera: bool) {
        if self.upd.running() {
            return;
        }
        let mut pkgs: Vec<String> = Vec::new();
        if !self.has_adb() {
            pkgs.push("android-tools".into());
        }
        if !self.has_scrcpy() {
            pkgs.push("scrcpy".into());
        }
        if !tessera_core::util::on_path("qrencode") {
            pkgs.push("qrencode".into()); // (pairing by QR code)
        }
        if with_camera && !self.phone.cam_module {
            // (not in this kernel: built for it, with its headers)
            pkgs.push("v4l2loopback-dkms".into());
            if !self.phone.kernel_package.is_empty() {
                pkgs.push(format!("{}-headers", self.phone.kernel_package));
            }
        }
        if pkgs.is_empty() {
            return;
        }
        self.phone.install_error.clear();
        let list = pkgs.join(",");
        self.update_run("phone", &["--install", &list]);
    }

    pub fn phone_state(&self) -> Value {
        json!({"devices": self.phone.devices, "phone": self.phone_dev(), "waiting": self.phone_waiting(), "hasAdb": self.has_adb(), "hasScrcpy": self.has_scrcpy(),
            "mirrors": self.phone.mirrors.iter().map(|(s, _)| s.clone()).collect::<Vec<_>>(), "camReady": self.phone.cam_ready, "camModule": self.phone.cam_module,
            "kernelPackage": self.phone.kernel_package, "webcam": self.phone_webcam(), "camError": self.phone.cam_error, "camSetting": self.phone.cam_setting,
            "pairStage": self.phone.pair_stage, "pairQr": self.phone.pair_qr, "pairError": self.phone.pair_error, "installing": self.upd.installing("phone"), "installError": self.phone.install_error})
    }
}

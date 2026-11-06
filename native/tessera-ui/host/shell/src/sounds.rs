//! Short UI sounds: shell/services/Sounds.qml. A chime when the session starts
//! (not when the shell reloads), a blip when the charger goes in, two-tones when
//! a display or USB device comes or goes, a latch for locking, glass taps for
//! notifications, a pop for the volume keys, a knock for a wrong password, the
//! shutter for screenshots, paper for emptying the Trash.
//!
//!   sound.play    [name, preview?]       (Settings' Play buttons preview a sound that is off)
//!   sound.file    [path, volume?]        a file, whatever its name
//!   sound.notify  [{urgency, hints}]     a banner about to show (Notifications calls it)
//!   sound.playStartup [id]               a sound to log in to ("" is Tessera's own)
//! The charger, lock, recording and display sounds follow the shell's state
//! once it has settled (5 s).

use crate::common::*;
use serde_json::{json, Value};
use std::any::Any;
use std::collections::BTreeMap;
use tessera_core::host::{Host, Service};

const DIR: &str = "/usr/share/sounds/freedesktop/stereo/";
const PACKS: &[&str] = &["marimba", "taps", "soft", "keys", "minimal", "glass", "material"];

/// The set playing now: the Material set (in the Material look, unless
/// "Material sounds" is off), or the style chosen for both looks
pub fn pack(cfg: &Value) -> &'static str {
    if material(cfg) && cfg_bool(cfg, "appearance.materialSounds", true) {
        return "material";
    }
    let p = cfg_str(cfg, "sounds.pack", "marimba");
    PACKS.iter().find(|x| **x == p && **x != "material").copied().unwrap_or("marimba")
}

/// The setting a sound follows (and whose custom file replaces it), when it isn't its own name
pub fn setting(name: &str) -> &str {
    match name {
        "displaysOff" | "displaysLeft" | "displaysRight" | "displaysOffLeft" | "displaysOffRight" => "displays",
        "unlock" => "lock",
        "usbOff" => "usb",
        "recordingOff" => "recording",
        "ringtone" => "notifications",
        n => n,
    }
}

/// Sounds that may only play this often (ms): the volume keys repeat
pub fn spacing(name: &str) -> i64 {
    match name {
        "volumeKeys" => 70,
        "notifications" => 600,
        "usb" | "usbOff" => 400,
        _ => 0,
    }
}

/// The file a sound is, in the set playing now
pub fn file_of(cfg: &Value, name: &str) -> String {
    let own = shell_dir().join("assets/sounds").join(pack(cfg));
    let in_set = |f: &str| own.join(f).to_string_lossy().into_owned();
    let default = match name {
        "startup" => {
            let id = cfg_str(cfg, "sounds.startupSound", "");
            if !id.is_empty() && crate::events::startup_sound_exists(id) {
                shell_dir().join("assets/sounds/startup").join(format!("{id}.ogg")).to_string_lossy().into_owned()
            } else {
                in_set("startup.ogg")
            }
        }
        "charging" => in_set("charging.ogg"),
        "displays" => in_set("display-connect.ogg"),
        "displaysOff" => in_set("display-disconnect.ogg"),
        "displaysLeft" => in_set("display-connect-left.ogg"),
        "displaysRight" => in_set("display-connect-right.ogg"),
        "displaysOffLeft" => in_set("display-disconnect-left.ogg"),
        "displaysOffRight" => in_set("display-disconnect-right.ogg"),
        "lock" => in_set("lock.ogg"),
        "unlock" => in_set("unlock.ogg"),
        "notifications" => in_set("notification.ogg"),
        "battery" => in_set("battery.ogg"),
        "volumeKeys" => in_set("volume.ogg"),
        "usb" => in_set("usb-connect.ogg"),
        "usbOff" => in_set("usb-disconnect.ogg"),
        "received" => in_set("received.ogg"),
        "recording" => in_set("record-start.ogg"),
        "recordingOff" => in_set("record-stop.ogg"),
        "error" => in_set("error.ogg"),
        "today" => in_set("today.ogg"),
        "ringtone" => in_set("ringtone.ogg"),
        "screenshot" => format!("{DIR}camera-shutter.oga"),
        "trash" => format!("{DIR}trash-empty.oga"),
        _ => String::new(),
    };
    // yours if you picked one
    let key = setting(name);
    let custom = cfg_str(cfg, &format!("sounds.{key}File"), "");
    if !custom.is_empty() && name == key {
        custom.to_string()
    } else {
        default
    }
}

/// The player command for a file ([] when there's no player)
pub fn command(cfg: &Value, f: &str, volume: Option<f64>) -> Vec<String> {
    if f.is_empty() {
        return vec![];
    }
    let v = volume.unwrap_or_else(|| cfg_num(cfg, "sounds.volume", 0.7)).clamp(0.0, 1.0);
    if on_path("pw-play") {
        vec!["pw-play".into(), "--volume".into(), format!("{v:.2}"), f.into()]
    } else if on_path("paplay") {
        vec!["paplay".into(), "--volume".into(), format!("{}", (v * 65536.0).round() as i64), f.into()]
    } else if on_path("canberra-gtk-play") {
        vec!["canberra-gtk-play".into(), "-f".into(), f.into()]
    } else {
        vec![]
    }
}

#[derive(Default)]
pub struct Sounds {
    last_played: BTreeMap<String, i64>,
    armed: bool,
    // what the sounds follow
    charging: Option<bool>,
    locked: bool,
    recording: bool,
    screens: usize,
    heard: usize,
    settle_gen: u64,
    poll: Option<slint::Timer>,
}

enum Tick {
    Armed,
    Poll,
    Settle(u64),
}

impl Sounds {
    /// Sounds.play
    pub fn play(&mut self, h: &Host, name: &str, preview: bool) {
        let cfg = h.cfg();
        let key = setting(name);
        let own = name == "startup";
        if !preview && ((!own && !cfg_bool(cfg, "sounds.enabled", true)) || at(cfg, &format!("sounds.{key}")) == &Value::Bool(false)) {
            return;
        }
        let now = now_ms();
        if !preview && now - self.last_played.get(name).copied().unwrap_or(0) < spacing(name) {
            return;
        }
        self.last_played.insert(name.to_string(), now);
        let vol = own.then(|| cfg_num(cfg, "sounds.startupVolume", 0.7));
        let cmd = command(cfg, &file_of(cfg, name), vol);
        if !cmd.is_empty() {
            run_owned(cmd);
        }
    }

    /// Is this sound switched on (Settings → Notifications → Sound effects)?
    pub fn allowed(cfg: &Value, name: &str) -> bool {
        cfg_bool(cfg, "sounds.enabled", true) && at(cfg, &format!("sounds.{}", setting(name))) != &Value::Bool(false)
    }

    /// A banner that's about to show: low urgency stays quiet, and so do apps
    /// that ask for silence; an app that names its own sound file gets that instead
    fn notify(&mut self, h: &Host, n: &Value) {
        let cfg = h.cfg();
        if !cfg_bool(cfg, "sounds.enabled", true) || !cfg_bool(cfg, "sounds.notifications", true) || n["urgency"].as_i64() == Some(0) {
            return;
        }
        let hints = &n["hints"];
        if hints["suppress-sound"].as_bool().unwrap_or(false) {
            return;
        }
        let own = hints["sound-file"].as_str().unwrap_or("");
        if own.starts_with('/') {
            let now = now_ms();
            if now - self.last_played.get("notifications").copied().unwrap_or(0) < spacing("notifications") {
                return;
            }
            self.last_played.insert("notifications".into(), now);
            let cmd = command(cfg, own, None);
            if !cmd.is_empty() {
                run_owned(cmd);
            }
            return;
        }
        self.play(h, "notifications", false);
    }

    /// Startup: once per compositor session. The mark carries the Hyprland
    /// instance, because the runtime dir outlives a session on another console
    fn startup(&mut self, h: &mut Host) {
        let sig = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").unwrap_or_else(|_| "none".into());
        let dir = runtime_dir();
        let mark = dir.join(format!("tessera-session-started-{sig}"));
        let old = dir.join(format!("irice-session-started-{sig}"));
        if mark.exists() {
            return;
        }
        let _ = std::fs::write(&mark, b"");
        if old.exists() {
            return; // (a session that started before iRice became Tessera)
        }
        self.play(h, "startup", false);
        h.ask("lock.sessionStarted", json!([]));
    }
}

impl Service for Sounds {
    fn name(&self) -> &'static str {
        "Sounds"
    }
    fn calls(&self) -> &'static [&'static str] {
        &["sound"]
    }
    fn start(&mut self, h: &mut Host) {
        h.st.shell.want("status");
        self.screens = h.st.outputs_now().len();
        self.heard = self.screens;
        self.startup(h);
        let send = h.sender("Sounds");
        slint::Timer::single_shot(std::time::Duration::from_secs(5), move || send(Box::new(Tick::Armed)));
        let send = h.sender("Sounds");
        let t = slint::Timer::default();
        t.start(slint::TimerMode::Repeated, std::time::Duration::from_secs(1), move || send(Box::new(Tick::Poll)));
        self.poll = Some(t);
    }
    fn call(&mut self, h: &mut Host, call: &str, args: &Value) {
        let a = if args.is_array() { args.clone() } else { json!([]) };
        match call {
            "sound" | "sound.play" => {
                let name = a[0].as_str().unwrap_or("").to_string();
                self.play(h, &name, a[1].as_bool().unwrap_or(false));
            }
            "sound.file" => {
                let cmd = command(h.cfg(), a[0].as_str().unwrap_or(""), a[1].as_f64());
                if !cmd.is_empty() {
                    run_owned(cmd);
                }
            }
            // (the notification itself, as host-desktop asks it: not wrapped in a list)
            "sound.notify" => self.notify(h, if args.is_array() { &a[0] } else { args }),
            // a sound to log in to: "" is Tessera's own (only the known ids)
            "sound.playStartup" => {
                let id = a[0].as_str().unwrap_or("");
                if crate::events::startup_sound_exists(id) {
                    let f = if id.is_empty() { shell_dir().join("assets/sounds").join(pack(h.cfg())).join("startup.ogg") } else { shell_dir().join("assets/sounds/startup").join(format!("{id}.ogg")) };
                    let cmd = command(h.cfg(), &f.to_string_lossy(), Some(cfg_num(h.cfg(), "sounds.startupVolume", 0.7)));
                    if !cmd.is_empty() {
                        run_owned(cmd);
                    }
                }
            }
            other => eprintln!("tessera-ui: host: no {other}"),
        }
    }
    fn outputs_changed(&mut self, h: &mut Host) {
        // A hotplug can add and remove outputs several times in a second;
        // one sound for where it ends up
        self.screens = h.st.outputs_now().len();
        if !self.armed {
            self.heard = self.screens;
            return;
        }
        self.settle_gen += 1;
        let (send, gen) = (h.sender("Sounds"), self.settle_gen);
        slint::Timer::single_shot(std::time::Duration::from_millis(800), move || send(Box::new(Tick::Settle(gen))));
    }
    fn message(&mut self, h: &mut Host, msg: Box<dyn Any + Send>) {
        let Ok(t) = msg.downcast::<Tick>() else { return };
        match *t {
            Tick::Armed => self.armed = true,
            Tick::Settle(g) => {
                if g != self.settle_gen {
                    return;
                }
                // Directional sound on: DisplayFx plays it, panned toward the display
                if !cfg_bool(h.cfg(), "displays.directionalSound", false) {
                    if self.screens > self.heard {
                        self.play(h, "displays", false);
                    } else if self.screens < self.heard {
                        self.play(h, "displaysOff", false);
                    }
                }
                self.heard = self.screens;
            }
            Tick::Poll => {
                let charging = h.st.shell.get::<tessera_core::services::status::Battery>().map(|b| b.charging);
                let locked = h.state("locked").as_bool().unwrap_or(false);
                let recording = h.state("recording.on").as_bool().unwrap_or(false);
                if self.armed {
                    if let (Some(was), Some(now)) = (self.charging, charging) {
                        if was != now && now {
                            self.play(h, "charging", false);
                        }
                    }
                    if locked != self.locked {
                        self.play(h, if locked { "lock" } else { "unlock" }, false);
                    }
                    if recording != self.recording {
                        self.play(h, if recording { "recording" } else { "recordingOff" }, false);
                    }
                }
                self.charging = charging.or(self.charging);
                self.locked = locked;
                self.recording = recording;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_set_follows_the_look_and_the_choice() {
        assert_eq!(pack(&json!({})), "marimba");
        assert_eq!(pack(&json!({"sounds": {"pack": "glass"}})), "glass");
        // (a chosen "material" set is only the Material look's own)
        assert_eq!(pack(&json!({"sounds": {"pack": "material"}})), "marimba");
        assert_eq!(pack(&json!({"appearance": {"look": "material"}})), "material");
        assert_eq!(pack(&json!({"appearance": {"look": "material", "materialSounds": false}, "sounds": {"pack": "keys"}})), "keys");
    }

    #[test]
    fn sounds_follow_their_settings() {
        assert_eq!(setting("unlock"), "lock");
        assert_eq!(setting("displaysOffLeft"), "displays");
        assert_eq!(setting("error"), "error");
        assert!(!Sounds::allowed(&json!({"sounds": {"enabled": false}}), "error"));
        assert!(!Sounds::allowed(&json!({"sounds": {"lock": false}}), "unlock"));
        assert!(Sounds::allowed(&json!({}), "unlock"));
    }

    #[test]
    fn a_custom_file_replaces_only_its_own_sound() {
        let cfg = json!({"sounds": {"lockFile": "/home/me/click.ogg"}});
        assert_eq!(file_of(&cfg, "lock"), "/home/me/click.ogg");
        assert_ne!(file_of(&cfg, "unlock"), "/home/me/click.ogg");
        assert!(file_of(&cfg, "trash").ends_with("trash-empty.oga"));
    }

    #[test]
    fn the_keys_and_notifications_do_not_repeat_at_once() {
        assert_eq!(spacing("volumeKeys"), 70);
        assert_eq!(spacing("lock"), 0);
    }
}

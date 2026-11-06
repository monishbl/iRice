//! shell/services/System.qml: the small system switches of Control Center
//! (Caffeine, night light, game mode, anti-flashbang, EasyEffects, WARP),
//! power actions (sleep waits for the lock), the lock mark file, and what
//! the shell tells Hyprland from Settings → Appearance (corners, gaps,
//! window transparency, pointer theme, window tabs reload).
//!
//! One Service answers the prefix "system": System.qml's functions
//! (`system.setKeepAwake [on, minutes]`, `system.power [action]`,
//! `system.repairShell`, `system.toggleDarkMode`, `system.setEasyEffects
//! [on]`, `system.setWarp [on]`, `system.set [prop, value]` for nightLight,
//! nightLightTemp, gameMode, antiFlashbang, `system.run [cmd…]`,
//! `system.hyprEval [lua]`, `system.identifySong`, `system.locked [bool]`,
//! `system.lockSecure [bool]`, `system.cursorTheme [name]`), and
//! everything Settings' System page asks (bridge_system.rs: `system.fpEnroll`
//! …, `system.live [bool]`, `system.page [id]`). IPC target `caffeine`
//! (toggle | on <minutes> | off). State is on the bus as "system" and in the
//! shell's state as caffeine.{on,status}.

use super::bridge_system::SystemBridge;
use super::helpers::{self, Tick, Ticker, Tx};
use crate::util;
use serde_json::{json, Value};
use std::any::Any;
use std::process::{Child, Command, Stdio};
use tessera_core::host::{Host, Service};

const ORANGE: &str = "#FF9F0A";

/// What the shell last told Hyprland from Settings (so only changes apply)
#[derive(Default, PartialEq, Clone, Debug)]
struct Applied {
    radius: i64,
    material: bool,
    gaps: String,
    opacity: String,
    custom: (u64, u64), // f64 bits would do; ×1000 rounded is enough for a slider
    cursor: String,
    tabs: String,
}

pub struct System {
    tx: Option<Tx>,
    keep_awake: bool,
    keep_until: f64,
    keep_minutes: i64,
    night_light: bool,
    night_temp: i64,
    game_mode: bool,
    anti_flash: bool,
    easy_effects: bool,
    warp: bool,
    locked: bool,
    lock_secure: bool,
    sleep_pending: bool,
    inhibit: Option<Child>,
    sunset: Option<Child>,
    t_until: Ticker,
    t_sleep: Ticker,
    t_tabs: Ticker,
    t_opacity: Ticker,
    applied: Option<Applied>,
    bridge: SystemBridge,
}

impl Default for System {
    fn default() -> Self {
        Self {
            tx: None,
            keep_awake: false,
            keep_until: 0.0,
            keep_minutes: 0,
            night_light: false,
            night_temp: 4500,
            game_mode: false,
            anti_flash: false,
            easy_effects: true,
            warp: false,
            locked: false,
            lock_secure: false,
            sleep_pending: false,
            inhibit: None,
            sunset: None,
            t_until: Ticker::new("keepUntil"),
            t_sleep: Ticker::new("sleepGuard"),
            t_tabs: Ticker::new("gestureReload"),
            t_opacity: Ticker::new("opacityTimer"),
            applied: None,
            bridge: SystemBridge::default(),
        }
    }
}

/// "Staying awake for 90 min" / "…2 hours" / "…until you turn it off"
pub fn awake_subtitle(minutes: i64) -> String {
    if minutes > 0 {
        let m = if minutes >= 60 {
            let h = minutes as f64 / 60.0;
            format!("{} {}", h, if minutes == 60 { "hour" } else { "hours" })
        } else {
            format!("{minutes} min")
        };
        format!("Staying awake for {m}")
    } else {
        "Staying awake until you turn it off".into()
    }
}

fn sandboxed() -> bool {
    helpers::env_is("SANDBOX")
}

fn tty() -> bool {
    helpers::env_is("TTY")
}

fn lock_marks() -> (String, String) {
    let run = std::env::var("XDG_RUNTIME_DIR").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "/tmp".into());
    let sig = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "none".into());
    (format!("{run}/tessera-locked-{sig}"), format!("{run}/irice-locked-{sig}"))
}

fn gap_levels(name: &str) -> (i64, i64) {
    match name {
        "none" => (0, 0),
        "comfortable" => (5, 10),
        _ => (3, 4), // compact, and what's unknown
    }
}

fn opacity_levels(name: &str) -> Option<(f64, f64)> {
    match name {
        "off" => Some((1.0, 1.0)),
        "subtle" => Some((0.97, 0.93)),
        "more" => Some((0.92, 0.86)),
        _ => None,
    }
}

impl System {
    fn tx(&self) -> &Tx {
        self.tx.as_ref().unwrap()
    }

    fn clock24(h: &Host) -> bool {
        h.cfg()["appearance"]["clock24"].as_bool().unwrap_or(false)
    }

    fn status(&self, h: &Host) -> String {
        if !self.keep_awake {
            String::new()
        } else if self.keep_until > 0.0 {
            format!("Until {}", helpers::fmt_hm((self.keep_until / 1000.0) as i64, Self::clock24(h)))
        } else {
            "Until off".into()
        }
    }

    fn publish(&mut self, h: &mut Host) {
        let status = self.status(h);
        crate::bus::put(
            "system",
            json!({
                "keepAwake": self.keep_awake, "keepAwakeUntil": self.keep_until, "keepAwakeMinutes": self.keep_minutes, "keepAwakeStatus": status,
                "nightLight": self.night_light, "nightLightTemp": self.night_temp, "gameMode": self.game_mode, "antiFlashbang": self.anti_flash,
                "easyEffects": self.easy_effects, "warp": self.warp, "locked": self.locked, "lockSecure": self.lock_secure, "sandboxed": sandboxed(),
            }),
        );
        h.set_state("caffeine.on", json!(self.keep_awake));
        h.set_state("caffeine.status", json!(status));
        self.bridge.system_changed(h);
    }

    fn hypr_eval(lua: &str) {
        tessera_core::services::bridge::run(&["hyprctl", "eval", lua]);
    }

    fn reload_config() {
        tessera_core::services::bridge::run(&["sh", "-c", "hyprctl reload config-only >/dev/null"]);
    }

    pub fn set_keep_awake(&mut self, h: &mut Host, on: bool, minutes: i64) {
        self.keep_until = if on && minutes > 0 { helpers::now_ms() + minutes as f64 * 60000.0 } else { 0.0 };
        self.keep_minutes = if on { minutes.max(0) } else { 0 };
        // (UiState.data.caffeine: it survives shell restarts)
        h.ask("uistate.set", json!(["caffeine", if on { json!({"until": self.keep_until, "minutes": self.keep_minutes}) } else { Value::Null }]));
        if self.keep_awake == on && !on {
            return;
        }
        self.keep_awake = on;
        helpers::island_post(
            h,
            json!({
                "kind": "custom", "icon": "coffee",
                "title": if on { "Caffeine on" } else { "Caffeine off" },
                "subtitle": if on { awake_subtitle(minutes) } else { "Sleeps when idle again".to_string() },
                "color": ORANGE,
            }),
            3000,
        );
        self.after_keep_awake();
        self.publish(h);
    }

    /// The expiry timer, and the idle inhibitor while it's held
    fn after_keep_awake(&mut self) {
        if self.keep_awake && self.keep_until > 0.0 {
            self.t_until.once(self.tx(), ((self.keep_until - helpers::now_ms()).max(1000.0)) as u64);
        } else {
            self.t_until.stop();
        }
        // A logind "idle" inhibitor while Caffeine is on (held by a child that
        // dies with the shell, and so does the sleep it holds it for). (The
        // in-use monitor of the tty session isn't ported: notes)
        let hold = self.keep_awake && !(sandboxed() && !tty());
        if hold && self.inhibit.is_none() {
            self.inhibit = Command::new("setpriv")
                .args(["--pdeathsig", "TERM", "--", "systemd-inhibit", "--what=idle", "--who=Tessera", "--why=In use (Caffeine, or this console session)", "--mode=block", "setpriv", "--pdeathsig", "TERM", "--", "sleep", "infinity"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .ok();
        } else if !hold {
            if let Some(mut c) = self.inhibit.take() {
                let _ = c.kill();
                let _ = c.wait();
            }
        }
    }

    fn sunset_update(&mut self) {
        if let Some(mut c) = self.sunset.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        if self.night_light {
            self.sunset = Command::new("setpriv").args(["--pdeathsig", "TERM", "--", "hyprsunset", "-t", &self.night_temp.to_string()]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().ok();
        }
    }

    fn set_locked(&mut self, h: &mut Host, on: bool) {
        if self.locked == on {
            return;
        }
        self.locked = on;
        if !on {
            self.sleep_pending = false;
            self.t_sleep.stop();
        }
        // remembered for this Hyprland session, so a shell that restarts while
        // the screen is locked locks again straight away
        let (mark, old) = lock_marks();
        if on {
            tessera_core::services::bridge::run(&["touch", &mark]);
        } else {
            tessera_core::services::bridge::run(&["rm", "-f", &mark, &old]);
        }
        // (the lock screen follows System.locked)
        h.ask("lock.set", json!([on]));
        self.publish(h);
    }

    fn finish_sleep(&mut self) {
        if !self.sleep_pending || !self.lock_secure || !self.locked {
            return;
        }
        self.sleep_pending = false;
        self.t_sleep.stop();
        tessera_core::services::bridge::run(&["systemctl", "suspend"]);
    }

    pub fn power(&mut self, h: &mut Host, action: &str) {
        if action == "logout" {
            tessera_core::services::bridge::run(&["hyprctl", "dispatch", "hl.dsp.exit()"]);
            return;
        }
        if action == "lock" {
            self.set_locked(h, true);
            return;
        }
        let cmd: &[&str] = match action {
            "sleep" => &["systemctl", "suspend"],
            "restart" => &["systemctl", "reboot"],
            "shutdown" => &["systemctl", "poweroff"],
            _ => return,
        };
        if sandboxed() {
            // (systemctl would reach the real machine through the shared bus)
            helpers::notify_send(&["-a", "Tessera sandbox", &format!("Skipped {action}"), &cmd.join(" ")]);
        } else if action == "sleep" {
            self.sleep_pending = true;
            self.set_locked(h, true);
            if self.lock_secure {
                self.finish_sleep();
            } else {
                self.t_sleep.once(self.tx(), 3000);
            }
        } else {
            tessera_core::services::bridge::run(cmd);
        }
    }

    /// Settings → Appearance, as Hyprland should have it (only what changed)
    fn apply_config(&mut self, h: &mut Host) {
        let a = &h.cfg()["appearance"];
        if !a.is_object() {
            return; // (Config isn't ready)
        }
        let now = Applied {
            radius: a["windowRadius"].as_i64().unwrap_or(20),
            material: a["look"].as_str() == Some("material"),
            gaps: a["gaps"].as_str().unwrap_or("compact").to_string(),
            opacity: a["windowOpacity"].as_str().unwrap_or("subtle").to_string(),
            custom: (
                (a["windowOpacityActive"].as_f64().unwrap_or(0.97) * 1000.0).round() as u64,
                (a["windowOpacityInactive"].as_f64().unwrap_or(0.93) * 1000.0).round() as u64,
            ),
            cursor: if (a["look"].as_str() == Some("material") && a["materialCursor"].as_bool().unwrap_or(false)) || a["cursor"].as_str().unwrap_or("bibata") == "bibata" { "Bibata-Modern-Classic".into() } else { "macOS".into() },
            tabs: a["windowTabs"].as_str().unwrap_or("hyprland").to_string(),
        };
        let was = self.applied.clone();
        if was.as_ref() == Some(&now) {
            return;
        }
        let first = was.is_none();
        let w = was.unwrap_or_default();
        if first || now.radius != w.radius || now.material != w.material {
            self.apply_radius(&now);
        }
        if first || now.gaps != w.gaps {
            self.apply_gaps(&now.gaps);
        }
        if first || now.opacity != w.opacity {
            self.apply_opacity(&now);
        } else if now.opacity == "custom" && now.custom != w.custom {
            self.t_opacity.once(self.tx(), 80); // while a slider moves
        }
        if first || now.cursor != w.cursor {
            Self::apply_cursor(&now.cursor);
        }
        // The window tab style is read by the Hyprland config (tabs.lua):
        // reload it once the change is saved
        if !first && now.tabs != w.tabs {
            self.t_tabs.once(self.tx(), 800);
        }
        self.applied = Some(now);
    }

    fn apply_radius(&self, a: &Applied) {
        if a.radius >= 0 && !self.game_mode {
            // (Glass: continuous corners; Material: round ones)
            Self::hypr_eval(&format!("hl.config({{ decoration = {{ rounding = {}, rounding_power = {} }} }})", a.radius, if a.material { "2.0" } else { "4.0" }));
        }
    }
    fn apply_gaps(&self, level: &str) {
        if !self.game_mode {
            let g = gap_levels(level);
            Self::hypr_eval(&format!("hl.config({{ general = {{ gaps_in = {}, gaps_out = {} }} }})", g.0, g.1));
        }
    }
    fn apply_opacity(&self, a: &Applied) {
        let c = |v: u64| (v as f64 / 1000.0).clamp(0.3, 1.0);
        let o = if a.opacity == "custom" { Some((c(a.custom.0), c(a.custom.1))) } else { opacity_levels(&a.opacity) };
        let Some(o) = o else { return };
        if self.game_mode {
            return;
        }
        Self::hypr_eval(&format!("hl.config({{ decoration = {{ active_opacity = {}, inactive_opacity = {}, fullscreen_opacity = 1.0 }} }})", o.0, o.1));
    }
    fn apply_cursor(theme: &str) {
        if theme.is_empty() {
            return;
        }
        // (Hyprland draws macOS-hypr, the sharper hyprcursor build, when it's installed)
        tessera_core::services::bridge::run(&["sh", "-c", "h=$1; [ \"$1\" = macOS ] && [ -d /usr/share/icons/macOS-hypr ] && h=macOS-hypr; hyprctl setcursor \"$h\" 24 >/dev/null; gsettings set org.gnome.desktop.interface cursor-theme \"$1\" 2>/dev/null", "sh", theme]);
    }
    fn apply_shader(&self) {
        let path = if self.anti_flash { util::root().join("shell/assets/hypr/antiflashbang.glsl").to_string_lossy().into_owned() } else { String::new() };
        Self::hypr_eval(&format!("hl.config({{ decoration = {{ screen_shader = \"{path}\" }} }})"));
    }

    fn set_prop(&mut self, h: &mut Host, prop: &str, v: &Value) {
        match prop {
            "nightLight" => {
                let on = v.as_bool().unwrap_or(false);
                if on != self.night_light {
                    self.night_light = on;
                    self.sunset_update();
                }
            }
            "nightLightTemp" => {
                let t = v.as_i64().unwrap_or(4500);
                if t != self.night_temp {
                    self.night_temp = t;
                    if self.night_light {
                        self.sunset_update();
                    }
                }
            }
            "gameMode" => {
                let on = v.as_bool().unwrap_or(false);
                if on != self.game_mode {
                    self.game_mode = on;
                    if on {
                        Self::hypr_eval("hl.config({ animations = { enabled = false }, decoration = { blur = { enabled = false }, shadow = { enabled = false }, rounding = 0, active_opacity = 1.0, inactive_opacity = 1.0 }, general = { gaps_in = 0, gaps_out = 0 } })");
                    } else {
                        Self::reload_config(); // (configreloaded then re-applies corners and the shader)
                    }
                }
            }
            "antiFlashbang" => {
                let on = v.as_bool().unwrap_or(false);
                if on != self.anti_flash {
                    self.anti_flash = on;
                    self.apply_shader();
                }
            }
            "easyEffects" => self.easy_effects = v.as_bool().unwrap_or(true),
            "warp" => self.warp = v.as_bool().unwrap_or(false),
            _ => {}
        }
        self.publish(h);
    }
}

fn arg_bool(a: &Value, i: usize, key: &str) -> bool {
    a.get(i).and_then(Value::as_bool).or_else(|| a.get(key).and_then(Value::as_bool)).unwrap_or(false)
}
fn arg_i(a: &Value, i: usize, key: &str) -> i64 {
    a.get(i).and_then(Value::as_i64).or_else(|| a.get(key).and_then(Value::as_i64)).unwrap_or(0)
}

impl Service for System {
    fn name(&self) -> &'static str {
        "System"
    }
    fn calls(&self) -> &'static [&'static str] {
        &["system"]
    }
    fn ipc_targets(&self) -> &'static [&'static str] {
        &["caffeine"]
    }

    fn start(&mut self, h: &mut Host) {
        let tx = helpers::tx(h, "System");
        self.tx = Some(tx.clone());
        self.bridge.start(h, tx.clone());
        // Caffeine stays on across shell restarts and reloads, unless it ran out
        let state = util::read_json(&util::state_dir().join("tessera/state.json"));
        let c = &state["caffeine"];
        if c.is_object() {
            let until = c["until"].as_f64().unwrap_or(0.0);
            if until > 0.0 && until <= helpers::now_ms() {
                h.ask("uistate.set", json!(["caffeine", Value::Null]));
            } else {
                self.keep_until = until;
                self.keep_minutes = c["minutes"].as_i64().unwrap_or(0);
                self.keep_awake = true;
                self.after_keep_awake();
            }
        }
        // A shell that restarts while the screen is locked locks again
        let (mark, old) = lock_marks();
        if std::path::Path::new(&mark).exists() || std::path::Path::new(&old).exists() {
            self.locked = true;
            h.ask("lock.set", json!([true]));
        }
        // WARP: is it connected?
        let t = tx.clone();
        std::thread::spawn(move || {
            let on = util::sh("command -v warp-cli >/dev/null && warp-cli status 2>/dev/null | grep -qi 'status update: connected' && echo on", std::time::Duration::from_secs(5)).map(|(_, o)| o.trim() == "on").unwrap_or(false);
            t(Box::new(Warp(on)));
        });
        // A config reload (saving a config file, or leaving game mode) puts back
        // the file's values; re-apply what the shell sets
        helpers::hypr_events(&["configreloaded"], move |_, _| tx(Box::new(Reloaded)));
        self.apply_config(h);
        self.publish(h);
    }

    fn config_changed(&mut self, h: &mut Host) {
        self.apply_config(h);
        self.publish(h); // (the clock format, in the status)
    }

    fn call(&mut self, h: &mut Host, call: &str, args: &Value) {
        let f = call.strip_prefix("system.").unwrap_or(call);
        match f {
            "setKeepAwake" => {
                let (on, m) = (arg_bool(args, 0, "on"), arg_i(args, 1, "minutes"));
                self.set_keep_awake(h, on, m);
            }
            "toggleDarkMode" => {
                // Config.options.appearance.mode = Theme.dark ? "light" : "dark"
                let mode = h.cfg()["appearance"]["mode"].as_str().unwrap_or("dark").to_string();
                let dark = match mode.as_str() {
                    "dark" => true,
                    "light" => false,
                    _ => crate::bus::field("theme", "dark").as_bool().unwrap_or(true),
                };
                util::config_set("appearance.mode", json!(if dark { "light" } else { "dark" }));
            }
            "power" => {
                let a = args.get(0).and_then(Value::as_str).or(args.as_str()).unwrap_or("").to_string();
                self.power(h, &a);
            }
            "repairShell" => {
                // a separate script (it outlives this shell): removes leftover and
                // duplicate shells and starts one fresh; apps keep running
                h.ask("panels.close", json!([]));
                tessera_core::services::bridge::run(&["setsid", "-f", &util::script("repair-shell.sh")]);
            }
            "locked" => {
                let on = arg_bool(args, 0, "locked");
                self.set_locked(h, on);
            }
            "lockSecure" => {
                self.lock_secure = arg_bool(args, 0, "lockSecure");
                if self.lock_secure {
                    self.finish_sleep();
                }
                self.publish(h);
            }
            "set" => {
                let p = args.get(0).and_then(Value::as_str).unwrap_or("").to_string();
                let v = args.get(1).cloned().unwrap_or(Value::Null);
                self.set_prop(h, &p, &v);
            }
            "setEasyEffects" => {
                let on = arg_bool(args, 0, "on");
                self.easy_effects = on;
                tessera_core::services::bridge::run(&["easyeffects", "--bypass", if on { "2" } else { "1" }]);
                self.publish(h);
            }
            "setWarp" => {
                let on = arg_bool(args, 0, "on");
                self.warp = on;
                tessera_core::services::bridge::run(&["warp-cli", if on { "connect" } else { "disconnect" }]);
                self.publish(h);
            }
            "run" => {
                let c: Vec<String> = args.as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default();
                if !c.is_empty() {
                    let c: Vec<&str> = c.iter().map(String::as_str).collect();
                    tessera_core::services::bridge::run(&c);
                }
            }
            "hyprEval" => {
                if let Some(l) = args.get(0).and_then(Value::as_str).or(args.as_str()) {
                    Self::hypr_eval(l);
                }
            }
            "cursorTheme" => {
                if let Some(t) = args.get(0).and_then(Value::as_str).or(args.as_str()) {
                    Self::apply_cursor(t);
                }
            }
            "identifySong" => {
                // listen for a few seconds and name the song that's playing (SongRec)
                tessera_core::services::bridge::run(&["sh", "-c", "command -v songrec >/dev/null || { notify-send -a 'Identify Song' 'SongRec is not installed' 'Install the songrec package to identify music.'; exit; }; notify-send -a 'Identify Song' 'Listening…' 'Play the song near your microphone.'; r=$(timeout 25 songrec recognize 2>/dev/null); if [ -n \"$r\" ]; then notify-send -a 'Identify Song' \"$r\" 'Copied to the clipboard'; printf %s \"$r\" | wl-copy; else notify-send -a 'Identify Song' 'No match' 'Try again with the music louder.'; fi"]);
            }
            // the shortcuts, menus and Spotlight's calls, each to the service that does it
            "toggleCaffeine" => self.set_keep_awake(h, !self.keep_awake, 0),
            "cyclePowerProfile" => h.ask("power.cycleProfile", json!([])),
            "toggleMic" => h.ask("audio.toggleMic", args.clone()),
            "installUpdates" => h.ask("updates.install", json!([])),
            "update" => h.ask("updates.update", args.clone()),
            // Spotlight's "Check for Updates": install what is waiting, else say there is nothing
            "checkUpdates" => {
                if crate::bus::field("updates", "count").as_i64().unwrap_or(0) > 0 {
                    h.ask("updates.install", json!([]));
                } else {
                    tessera_core::services::bridge::run(&["notify-send", "-a", "Software Update", "Your system is up to date"]);
                }
            }
            "bootRefresh" => h.ask("bootsplash.refresh", json!([])),
            "bootSet" => h.ask("bootsplash.set", args.clone()),
            "previewLogin" => h.ask("loginscreen.preview", json!([])),
            // [light | dark]
            "setAppearance" => {
                if let Some(m) = args.get(0).and_then(Value::as_str).filter(|m| *m == "light" || *m == "dark") {
                    util::config_set("appearance.mode", json!(m));
                }
            }
            // [on]; with none, the other way round
            "setNightLight" => {
                let on = args.get(0).and_then(Value::as_bool).unwrap_or(!self.night_light);
                self.set_prop(h, "nightLight", &json!(on));
            }
            // [style id]: the font of an emoji style, from the AUR in a terminal (EmojiFont.install)
            "installEmoji" => {
                let pkg = match args.get(0).and_then(Value::as_str).unwrap_or("") {
                    "android" => "noto-fonts-emoji",
                    "apple" => "ttf-apple-emoji",
                    "windows" => "ttf-fluent-emoji",
                    _ => return,
                };
                h.ask("launch.terminal", json!([["sh", "-c", "h=$(command -v yay || command -v paru) || { echo 'Install yay or paru first (an AUR helper), then try again.'; read -p 'Press Enter to close' _; exit 1; }; \"$h\" -S --needed \"$1\" && fc-cache -f; echo; read -p 'Done. Press Enter to close' _", "sh", pkg], false]));
            }
            // everything else is Settings' System page (bridge_system.rs)
            _ => {
                let mut out = Vec::new();
                self.bridge.call(h, f, args, &mut out);
                for (c, a) in out {
                    if c == "system.setKeepAwake" {
                        self.set_keep_awake(h, false, 0);
                    } else {
                        h.ask(&c, a);
                    }
                }
            }
        }
    }

    fn ipc(&mut self, h: &mut Host, target: &str, func: &str, args: &[String]) -> Option<String> {
        if target != "caffeine" {
            return None;
        }
        match func {
            "toggle" => self.set_keep_awake(h, !self.keep_awake, 0),
            "on" => {
                let m = args.first().and_then(|s| s.parse().ok()).unwrap_or(0);
                self.set_keep_awake(h, true, m)
            }
            "off" => self.set_keep_awake(h, false, 0),
            _ => {}
        }
        None
    }

    fn message(&mut self, h: &mut Host, msg: Box<dyn Any + Send>) {
        let msg = match msg.downcast::<Tick>() {
            Ok(t) => {
                match t.0 {
                    "keepUntil" => self.set_keep_awake(h, false, 0),
                    "sleepGuard" => {
                        // Sleep only once the compositor confirms the lock: never sleep
                        // with the desktop open
                        if self.sleep_pending {
                            self.sleep_pending = false;
                            helpers::notify_send(&["-a", "Tessera", "-i", "system-lock-screen", "Didn't sleep", "The screen couldn't be locked, so the computer stayed awake. Lock it first (Super+L), then choose Sleep."]);
                        }
                    }
                    "gestureReload" => Self::reload_config(),
                    "opacityTimer" => {
                        if let Some(a) = self.applied.clone() {
                            self.apply_opacity(&a);
                        }
                    }
                    other => self.bridge.tick(h, other),
                }
                return;
            }
            Err(m) => m,
        };
        let msg = match msg.downcast::<Warp>() {
            Ok(w) => {
                self.warp = w.0;
                return self.publish(h);
            }
            Err(m) => m,
        };
        match msg.downcast::<Reloaded>() {
            Ok(_) => {
                if let Some(a) = self.applied.clone() {
                    self.apply_radius(&a);
                    self.apply_gaps(&a.gaps);
                    self.apply_opacity(&a);
                }
                if self.anti_flash {
                    self.apply_shader();
                }
            }
            Err(m) => self.bridge.message(h, m),
        }
    }

    fn quit(&mut self, _h: &mut Host) {
        for c in [self.inhibit.take(), self.sunset.take()].into_iter().flatten() {
            let mut c = c;
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

struct Warp(bool);
struct Reloaded;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn caffeine_words() {
        assert_eq!(awake_subtitle(0), "Staying awake until you turn it off");
        assert_eq!(awake_subtitle(30), "Staying awake for 30 min");
        assert_eq!(awake_subtitle(60), "Staying awake for 1 hour");
        assert_eq!(awake_subtitle(120), "Staying awake for 2 hours");
        assert_eq!(awake_subtitle(90), "Staying awake for 1.5 hours");
    }
    #[test]
    fn levels() {
        assert_eq!(gap_levels("comfortable"), (5, 10));
        assert_eq!(gap_levels("weird"), (3, 4));
        assert_eq!(opacity_levels("more"), Some((0.92, 0.86)));
        assert_eq!(opacity_levels("custom"), None);
    }
}

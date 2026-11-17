//! Battery & Power (replaces services/Power.qml; the decisions are
//! power_policy.rs, services/PowerPolicy.js): one place that decides the
//! energy mode, and the battery's idle, lid and critical-battery behaviour.
//!
//! Energy mode: everyone who wants a say asks here instead of setting the
//! power daemon themselves, and one answer wins (power_policy::resolve):
//! Cool-down when hot, then the battery saver, then your own choice or Game
//! Mode, whichever came last. A change made outside Tessera (powerprofilesctl)
//! counts as your choice. The dev sandbox never sets the mode. Without
//! power-profiles-daemon there are no modes to set.
//!
//! The battery is core's services::status::Battery (UPower); the daemon's
//! profile is read over D-Bus on a thread (net.hadess.PowerProfiles, or
//! UPower's own name for it) and set with `powerprofilesctl set`.
//!
//! Battery saver (the shell's, not the energy mode): on battery at or below
//! power.saverLevel (Auto), always on battery, or only by hand; plugging in
//! ends it (power_policy::Saver). It sets core's saver flag (saver.rs: 60 Hz,
//! Reduce motion, slower background refreshes) and has every part read the
//! settings again; with power.saverProfile it also asks for Low Power.
//!
//! Calls (prefix "power"): setMode(mode), ask({owner, mode, why, until}),
//! release(owner), saverOn, saverToggle, saverOff, energyUsers(delta),
//! clearEnergy, syncGuard, idleLater. From System (read off the bus under "system"): gameMode,
//! keepAwake. Asks of others: system.power(["sleep"|"lock"]), sound.play
//! (["battery"]), island.post. State on the bus as "power": {hasBattery,
//! percent, onBattery, mode, modeName, why, winner, requests, ppd,
//! hasPerformance, profile, stage, guardActive, guardError, ownHypridle,
//! userHypridle, energyApps, energyMinutes, lidMine, saver {on, mode,
//! level, byHand}}.

use super::common::*;
use super::power_policy as policy;
use super::power_policy::Request;
use serde_json::{json, Value};
use std::any::Any;
use std::collections::BTreeMap;
use std::time::Duration;
use tessera_core::host::{Host, Service};
use tessera_core::services::status::Battery;

const NAME: &str = "Power";

pub fn mode_name(mode: &str) -> String {
    let name = match mode {
        "power-saver" => "Low Power",
        "balanced" => "Balanced",
        "performance" => "Performance",
        m => m,
    };
    name.to_string()
}

/// Config.options.power with the QML's defaults filled in
pub fn cfg_with_defaults(raw: &Value) -> Value {
    let mut c = json!({
        "mode": "balanced", "batterySaver": true, "batterySaverAuto": false, "batterySaverLevel": 20, "gamePerformance": true, "critical": true,
        "idleDimAc": 300, "idleLockAc": 600, "idleOffAc": 600, "idleSleepAc": 0,
        "idleDimBattery": 120, "idleLockBattery": 300, "idleOffBattery": 300, "idleSleepBattery": 900,
        "lidAc": "sleep", "lidBattery": "sleep", "lidExternal": "nothing", "lidCaffeine": "lock", "passwordAfterSleep": 0, "appEnergy": true,
        "saver": "auto", "saverLevel": 20, "saverProfile": false,
    });
    if let Some(o) = raw["power"].as_object() {
        for (k, v) in o {
            if !v.is_null() {
                c[k] = v.clone();
            }
        }
    }
    c
}

/// The settings that rewrite the idle listeners
fn idle_key(c: &Value) -> String {
    let keys = ["idleDimAc", "idleLockAc", "idleOffAc", "idleSleepAc", "idleDimBattery", "idleLockBattery", "idleOffBattery", "idleSleepBattery", "passwordAfterSleep"];
    keys.iter().map(|k| c[*k].to_string()).collect::<Vec<_>>().join(",")
}

// ── The power daemon, over D-Bus ───────────────────────────────────────────

enum M {
    Tick,
    /// (profile, has a Performance profile); None: no daemon
    Ppd(Option<(String, bool)>),
    GuardDone(String),
    IdleDone(String),
    IdleWrite,
    LidClosed,
    LidOpened,
    Report(String),
    /// the notification's Turn off
    SaverOff,
}

fn ppd_read(conn: &zbus::blocking::Connection, dest: &'static str, path: &'static str) -> zbus::Result<(String, bool)> {
    use zbus::proxy::CacheProperties;
    let p = zbus::blocking::proxy::Builder::<zbus::blocking::Proxy>::new(conn).destination(dest)?.path(path)?.interface(dest)?.cache_properties(CacheProperties::No).build()?;
    let active: String = p.get_property("ActiveProfile")?;
    let profiles: Vec<std::collections::HashMap<String, zbus::zvariant::OwnedValue>> = p.get_property("Profiles").unwrap_or_default();
    let perf = profiles.iter().any(|m| m.get("Profile").and_then(|v| String::try_from(v.clone()).ok()).as_deref() == Some("performance"));
    Ok((active, perf))
}

/// Set the energy mode over D-Bus, as powerprofilesctl does (TuneD's
/// tuned-ppd and older daemons answer the same names, without always
/// shipping powerprofilesctl); powerprofilesctl only when neither answers.
/// On a thread
fn set_profile(mode: String) {
    std::thread::spawn(move || {
        let names = [("net.hadess.PowerProfiles", "/net/hadess/PowerProfiles"), ("org.freedesktop.UPower.PowerProfiles", "/org/freedesktop/UPower/PowerProfiles")];
        let set = zbus::blocking::Connection::system().ok().is_some_and(|conn| {
            names.iter().any(|(dest, path)| {
                conn.call_method(Some(*dest), *path, Some("org.freedesktop.DBus.Properties"), "Set", &(*dest, "ActiveProfile", zbus::zvariant::Value::from(mode.as_str()))).is_ok()
            })
        });
        if !set && tessera_core::util::on_path("powerprofilesctl") {
            let _ = std::process::Command::new("powerprofilesctl").args(["set", &mode]).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
        }
    });
}

fn spawn_ppd(send: impl Fn(Box<dyn Any + Send>) + Send + 'static) {
    std::thread::spawn(move || loop {
        let run = || -> zbus::Result<()> {
            let conn = zbus::blocking::Connection::system()?;
            let names = [("net.hadess.PowerProfiles", "/net/hadess/PowerProfiles"), ("org.freedesktop.UPower.PowerProfiles", "/org/freedesktop/UPower/PowerProfiles")];
            let (dest, path, first) = names
                .iter()
                .find_map(|(d, p)| ppd_read(&conn, *d, *p).ok().map(|r| (*d, *p, r)))
                .ok_or_else(|| zbus::Error::Failure("no power-profiles-daemon".into()))?;
            let rule = zbus::MatchRule::builder().msg_type(zbus::message::Type::Signal).sender(dest)?.interface("org.freedesktop.DBus.Properties")?.member("PropertiesChanged")?.build();
            let mut signals = zbus::blocking::MessageIterator::for_match_rule(rule, &conn, Some(16))?;
            let mut last = first.clone();
            send(Box::new(M::Ppd(Some(first))));
            while signals.next().is_some() {
                let now = ppd_read(&conn, dest, path)?;
                if now != last {
                    send(Box::new(M::Ppd(Some(now.clone()))));
                    last = now;
                }
            }
            Ok(())
        };
        if run().is_err() {
            send(Box::new(M::Ppd(None)));
        }
        std::thread::sleep(Duration::from_secs(30));
    });
}

/// (no battery reading yet)
const NO_STATE: u32 = u32::MAX;

pub struct Power {
    cfg: Value,
    requests: BTreeMap<String, Request>,
    manual_since: f64,
    /// power-profiles-daemon answered (true until it is known not to)
    ppd: bool,
    profile: String,
    has_perf: bool,
    applied: String,
    applied_at: f64,
    last_mode: String,
    battery: Battery,
    prev_low: bool,
    prev_on_battery: bool,
    prev_state: u32,
    prev_percent: f64,
    saver_dismissed: bool,
    warned: u8,
    guard_active: bool,
    guard_error: String,
    guard_synced: bool,
    own_hypridle: bool,
    idle_key: String,
    idle_timer: slint::Timer,
    tick: slint::Timer,
    lid: Vec<Watch>,
    /// logind's lid inhibitor, held by this process (closes with it)
    lid_fd: Option<zbus::zvariant::OwnedFd>,
    /// the lid's state while Tessera handles it, and the closed-lid guard
    lid_is_closed: bool,
    lid_sleep_asked: f64,
    /// asks to sleep since the lid closed (the third sleeps without waiting for the lock)
    lid_sleep_tries: u32,
    hot_since: f64,
    energy_users: i32,
    energy_apps: Value,
    energy_minutes: i64,
    ticks: u64,
    reporting: bool,
    last: Value,
    /// the shell's battery saver
    saver: policy::Saver,
}

impl Default for Power {
    fn default() -> Self {
        Self {
            cfg: cfg_with_defaults(&Value::Null),
            requests: BTreeMap::new(),
            manual_since: 0.0,
            ppd: true,
            profile: String::new(),
            has_perf: false,
            applied: String::new(),
            applied_at: 0.0,
            last_mode: String::new(),
            battery: Battery::default(),
            prev_low: false,
            prev_on_battery: false,
            prev_state: NO_STATE,
            prev_percent: -1.0,
            saver_dismissed: false,
            warned: 0,
            guard_active: false,
            guard_error: String::new(),
            guard_synced: false,
            own_hypridle: false,
            idle_key: String::new(),
            idle_timer: Default::default(),
            tick: Default::default(),
            lid: vec![],
            lid_fd: None,
            lid_is_closed: false,
            lid_sleep_asked: 0.0,
            lid_sleep_tries: 0,
            hot_since: 0.0,
            energy_users: 0,
            energy_apps: json!([]),
            energy_minutes: 0,
            ticks: 0,
            reporting: false,
            last: Value::Null,
            saver: policy::Saver::default(),
        }
    }
}

impl Power {
    fn has_battery(&self) -> bool {
        self.battery.present
    }
    fn percent(&self) -> f64 {
        if !self.has_battery() {
            -1.0
        } else if self.battery.percent > 1.0 {
            self.battery.percent
        } else {
            self.battery.percent * 100.0
        }
    }
    fn on_battery(&self) -> bool {
        self.has_battery() && (self.battery.state == 2 || self.battery.state == 3)
    }
    fn caffeine() -> bool {
        crate::bus::field("system", "keepAwake") == true
    }
    fn game_mode() -> bool {
        crate::bus::field("system", "gameMode") == true
    }

    fn winner(&self) -> Request {
        let mut all: Vec<Request> = self.requests.values().cloned().collect();
        all.push(Request { owner: "manual".into(), mode: self.cfg["mode"].as_str().unwrap_or("balanced").into(), priority: 20, since: self.manual_since, until: 0.0, why: String::new() });
        policy::resolve(&all, now_ms())
    }
    fn mode(&self) -> String {
        policy::fit(&self.winner().mode, self.has_perf)
    }

    /// Power.ask(owner, mode, why, until)
    fn ask_mode(&mut self, owner: &str, mode: &str, why: &str, until: f64) {
        let since = match self.requests.get(owner) {
            Some(r) if r.mode == mode => r.since,
            _ => now_ms(),
        };
        self.requests.insert(owner.into(), Request { owner: owner.into(), mode: mode.into(), priority: policy::priority(owner), since, until, why: why.into() });
    }
    fn release(&mut self, owner: &str) {
        self.requests.remove(owner);
    }

    /// Your choice: saved, and newer than Game Mode. It also ends the battery
    /// saver until you plug in again.
    fn set_mode(&mut self, mode: &str) {
        self.manual_since = now_ms();
        self.cfg["mode"] = json!(mode);
        crate::util::config_set("power.mode", json!(mode));
        self.release("battery");
    }

    fn saver_on(&mut self) {
        let why = format!("battery at {}%", self.percent().round());
        self.ask_mode("battery", "power-saver", &why, 0.0);
    }

    fn offer_saver(&mut self, h: &mut Host) {
        if !self.cfg["batterySaver"].as_bool().unwrap_or(true) || self.saver_dismissed || self.requests.contains_key("battery") || self.mode() == "power-saver" || !self.ppd {
            return;
        }
        // One alert at the low mark, not two: while the shell's own battery
        // saver comes on by itself (its notice says so, and its "Also use Low
        // Power" setting covers the profile), this offer stays quiet
        if policy::SaverMode::from_cfg(&self.cfg) != policy::SaverMode::Off && !self.cfg["batterySaverAuto"].as_bool().unwrap_or(false) {
            return;
        }
        self.saver_dismissed = true;
        let p = format!("{}%", self.percent().round());
        if self.cfg["batterySaverAuto"].as_bool().unwrap_or(false) {
            self.saver_on();
            island_post(h, json!({"kind": "custom", "icon": "leaf", "title": "Low Power on", "subtitle": format!("Battery at {p}"), "color": accent::YELLOW, "action": "Undo", "actionCall": "power.release", "actionArgs": ["battery"]}), 8000);
        } else {
            island_post(h, json!({"kind": "custom", "icon": "leaf", "title": format!("Battery at {p}"), "subtitle": "Turn on Low Power?", "color": accent::YELLOW, "action": "Turn on", "actionCall": "power.saverOn", "actionArgs": []}), 15000);
        }
    }

    /// Applied when the answer changes; what we set is remembered, so a
    /// different profile showing up means someone else changed it
    fn apply(&mut self) {
        let mode = self.mode();
        if tessera_core::env::sandboxed() || !self.ppd {
            return;
        }
        if self.applied.is_empty() {
            self.applied_at = now_ms();
        }
        self.applied = mode.clone();
        if self.profile != mode {
            set_profile(mode);
        }
    }

    fn warn(&mut self, h: &mut Host, icon: &str, title: &str, text: &str, action: Option<&str>) {
        h.ask("sound.play", json!(["battery"]));
        if island_enabled(h.cfg()) {
            let mut ev = json!({"kind": "custom", "icon": icon, "title": title, "subtitle": text, "color": accent::DANGER});
            if let Some(a) = action {
                ev["action"] = json!(a);
                ev["actionCall"] = json!("system.power");
                ev["actionArgs"] = json!(["sleep"]);
            }
            island_post(h, ev, 15000);
        } else {
            tessera_core::util::notify("Tessera", "", "critical", title, text);
        }
    }

    fn sync_guard(&mut self, h: &mut Host) {
        if tessera_core::env::sandboxed() || !self.has_battery() {
            return;
        }
        let send = h.sender(NAME);
        let enable = if self.cfg["critical"].as_bool().unwrap_or(true) { "enable" } else { "disable" };
        capture_then(vec!["bash".into(), crate::util::script("battery-guard-install.sh"), enable.into()], None, move |_, t| send(Box::new(M::GuardDone(t))));
    }

    fn write_idle(&mut self, h: &mut Host) {
        let mut lines: Vec<String> = policy::idle_listeners(&self.cfg).into_iter().map(|(s, a, t)| format!("{s} {a} {t}")).collect();
        lines.push(format!("grace {}", self.cfg["passwordAfterSleep"].as_f64().unwrap_or(0.0).max(0.0)));
        let idle_file = tessera_core::theme::config_path().parent().map(|p| p.join("idle-listeners")).unwrap_or_default();
        let user_hypridle = tessera_core::util::xdg("XDG_CONFIG_HOME", ".config").join("hypr/hypridle.conf");
        let own = crate::util::script("own-hypridle.sh");
        let script = "printf '%s\\n' \"$2\" > \"$1.tmp\" && mv \"$1.tmp\" \"$1\"; for n in tessera irice; do d=\"${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/$n-idle-$HYPRLAND_INSTANCE_SIGNATURE\"; p=$(cat \"$d/hypridle.pid\" 2>/dev/null); [ -n \"$p\" ] && [ \"$(cat /proc/$p/comm 2>/dev/null)\" = hypridle ] && kill \"$p\"; done; \"$4\" \"$3\" && echo own || echo ok";
        let send = h.sender(NAME);
        let args = [idle_file.to_string_lossy().into_owned(), lines.join("\n"), user_hypridle.to_string_lossy().into_owned(), own];
        let a: Vec<&str> = args.iter().map(String::as_str).collect();
        capture_then(sh_cmd(script, &a), None, move |_, t| send(Box::new(M::IdleDone(t))));
    }

    fn idle_later(&mut self, h: &Host) {
        let send = h.sender(NAME);
        self.idle_timer.start(slint::TimerMode::SingleShot, Duration::from_millis(800), move || send(Box::new(M::IdleWrite))); // after a slider settles
    }

    /// Hold (or let go of) logind's lid switch, and hear the lid
    fn lid_watch(&mut self, h: &Host) {
        let mine = !tessera_core::env::sandboxed() && self.has_battery() && policy::lid_needs_inhibit(&self.cfg, Self::caffeine());
        if mine && self.lid.is_empty() {
            let send = h.sender(NAME);
            // (held by this process: it can't outlive the shell, so logind
            // handles the lid again the moment Tessera isn't there to)
            self.lid_fd = tessera_core::inhibit::hold("handle-lid-switch", "Lid settings (Battery & Power)");
            self.lid_is_closed = tessera_core::inhibit::lid_closed().unwrap_or(false);
            self.lid.push(Watch::start(
                &["gdbus", "monitor", "--system", "--dest", "org.freedesktop.login1", "--object-path", "/org/freedesktop/login1"],
                None,
                move |l| {
                    if l.contains("'LidClosed': <true>") {
                        send(Box::new(M::LidClosed));
                    } else if l.contains("'LidClosed': <false>") {
                        send(Box::new(M::LidOpened));
                    }
                },
                |_, _| {},
            ));
        } else if !mine && !self.lid.is_empty() {
            self.lid.drain(..).for_each(|w| w.stop());
            self.lid_fd = None;
            self.lid_is_closed = false;
        }
    }

    fn built_in_screens(h: &Host, on: bool) {
        for (_, name) in h.st.outputs_now() {
            if is_built_in(&name) {
                tessera_core::services::hypr::dispatch(format!("hl.dsp.dpms({{ action = \"{}\", monitor = \"{}\" }})", if on { "enable" } else { "disable" }, name));
            }
        }
    }

    fn lid_closed(&mut self, h: &mut Host) {
        self.lid_is_closed = true;
        let external = h.st.outputs_now().iter().filter(|(_, n)| !is_built_in(n)).count();
        let act = policy::lid_action(!self.on_battery(), external, &self.cfg, Self::caffeine());
        if act == "sleep" {
            self.lid_sleep_asked = (tessera_core::util::now_ms() as f64);
            h.ask("system.power", json!(["sleep"]));
            return;
        }
        if Self::caffeine() && !self.on_battery() {
            eprintln!("tessera-ui: lid closed on the charger with Caffeine on: {act}");
        }
        if act == "lock" {
            h.ask("system.power", json!(["lock"]));
        }
        Self::built_in_screens(h, false);
    }

    /// The lid is shut and Tessera holds it from logind, yet it's awake: on
    /// battery it sleeps (an unplug after closing, a resume with the lid
    /// shut, Caffeine, a missed event: none keep it awake in a bag), on the
    /// charger when it runs hot for a minute and isn't docked. At most once a
    /// minute.
    fn closed_lid_guard(&mut self, h: &mut Host, on_battery: bool) {
        if !self.lid_is_closed || self.lid_fd.is_none() {
            self.hot_since = 0.0;
            return;
        }
        let now = (tessera_core::util::now_ms() as f64);
        let hot = cpu_temp_c().is_some_and(|t| t >= 90.0);
        if hot && self.hot_since == 0.0 {
            self.hot_since = now;
        } else if !hot {
            self.hot_since = 0.0;
        }
        let hot_for = if self.hot_since > 0.0 { (now - self.hot_since) / 1000.0 } else { 0.0 };
        let external = h.st.outputs_now().iter().filter(|(_, n)| !is_built_in(n)).count();
        if !policy::lid_should_sleep_now(!on_battery, external, hot_for) {
            self.lid_sleep_tries = 0;
            return;
        }
        if now - self.lid_sleep_asked > 60_000.0 {
            self.lid_sleep_asked = now;
            self.lid_sleep_tries += 1;
            if self.lid_sleep_tries <= 2 {
                eprintln!("tessera-ui: the lid is closed and it's still awake ({}): sleeping", if on_battery { "on battery" } else { "running hot" });
                // (locks first, sleeps once the lock is confirmed)
                h.ask("system.power", json!(["sleep"]));
            } else {
                // Still awake a minute after asking twice: the lock never
                // confirmed. The lid is shut, so nobody sees the desktop, and
                // the lock was asked for: sleep now rather than run the battery
                // down in a bag (17 Nov: 2 hours, 99% → 20%, hot)
                eprintln!("tessera-ui: the lid is closed, asked to sleep twice and still awake: sleeping without waiting for the lock");
                h.ask("system.power", json!(["lock"]));
                tessera_core::services::bridge::run(&["systemctl", "suspend"]);
            }
        }
    }

    /// The shell's battery saver, for the battery as it is now: on or off,
    /// core's flag with it (every part reads the settings again: Reduce
    /// motion), Low Power with it if asked, and a notice when it came on by
    /// itself
    fn saver_step(&mut self, h: &mut Host) {
        let mode = policy::SaverMode::from_cfg(&self.cfg);
        let level = policy::saver_level(&self.cfg);
        let changed = self.saver.step(mode, level, self.percent(), self.on_battery());
        // (Low Power with it: only when asked for, and only while it's on)
        let profile = self.saver.on && self.cfg["saverProfile"].as_bool().unwrap_or(false) && self.ppd;
        if profile && !self.requests.contains_key("saver") {
            self.ask_mode("saver", "power-saver", "battery saver", 0.0);
        } else if !profile {
            self.release("saver");
        }
        if tessera_core::saver::set(self.saver.on) {
            eprintln!("tessera-ui: battery saver {} (battery at {:.0}%, {})", if self.saver.on { "on" } else { "off" }, self.percent(), if self.on_battery() { "on battery" } else { "plugged in" });
            let _ = h.st.shell.tx.send(tessera_core::Msg::ConfigChanged);
        }
        if changed == Some(true) && self.saver.came_on_by_itself(mode) {
            let (title, text) = ("Battery saver is on", "The shell runs lighter until you plug in.");
            if island_enabled(h.cfg()) {
                island_post(h, json!({"kind": "custom", "icon": "leaf", "title": title, "subtitle": text, "color": accent::YELLOW, "action": "Turn off", "actionCall": "power.saverOff", "actionArgs": []}), 8000);
            } else {
                // (a notification with its Turn off: notify-send waits for the answer)
                let send = h.sender(NAME);
                std::thread::spawn(move || {
                    let out = std::process::Command::new("notify-send").args(["-a", "Tessera", "-i", "battery-caution", "-A", "off=Turn off", "--wait", title, text]).stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null()).output();
                    if out.is_ok_and(|o| String::from_utf8_lossy(&o.stdout).trim() == "off") {
                        send(Box::new(M::SaverOff));
                    }
                });
            }
        }
    }

    /// A pass of the one-second tick: the battery, what depends on it
    fn tick(&mut self, h: &mut Host) {
        self.ticks += 1;
        if let Some(b) = h.st.shell.get::<Battery>() {
            self.battery = b.clone();
        }
        self.saver_step(h);
        if self.has_battery() && !self.guard_synced {
            self.guard_synced = true;
            self.sync_guard(h);
            if self.low() {
                self.offer_saver(h);
            }
        }
        let (on_battery, percent, low) = (self.on_battery(), self.percent(), self.low());
        if low && !self.prev_low {
            self.offer_saver(h);
        }
        self.prev_low = low;
        if self.prev_on_battery && !on_battery {
            self.saver_dismissed = false;
            self.release("battery");
        }
        self.prev_on_battery = on_battery;
        self.closed_lid_guard(h, on_battery);
        if percent != self.prev_percent {
            self.prev_percent = percent;
            if self.requests.contains_key("battery") {
                self.saver_on(); // (the reason shows the level)
            }
        }
        // Game Mode asks for Performance
        let game_wanted = Self::game_mode() && self.cfg["gamePerformance"].as_bool().unwrap_or(true) && self.has_perf && self.ppd;
        if game_wanted && !self.requests.contains_key("game") {
            self.ask_mode("game", "performance", "Game Mode until you turn it off", 0.0);
        } else if !game_wanted {
            self.release("game");
        }
        // Critical battery
        let stage = if self.cfg["critical"].as_bool().unwrap_or(true) { policy::battery_stage(percent, on_battery) } else { 0 };
        if stage == 0 {
            self.warned = 0;
        } else if stage > self.warned {
            self.warned = stage;
            let p = format!("{}%", percent.round());
            match stage {
                1 => self.warn(h, "battery-low", "Battery low", &format!("{p} left. Plug in soon."), None),
                2 => self.warn(h, "battery-warning", "Battery very low", &format!("{p} left. It sleeps at 3% to keep your work."), Some("Sleep now")),
                _ => {
                    if !self.guard_active && !tessera_core::env::sandboxed() {
                        h.ask("system.power", json!(["sleep"])); // the guard does it when it's running
                    }
                }
            }
        }
        // A note when it's full (the charger can come out): when the state
        // changes to full (Power.qml's onStateChanged), not on the first
        // reading, which posted it at every start while full
        if self.has_battery() {
            if self.battery.state == 4 && self.prev_state != 4 && self.prev_state != NO_STATE {
                tessera_core::util::notify("Battery", "battery-full-charged", "", "Fully charged", "You can unplug your computer.");
            }
            self.prev_state = self.battery.state;
        }
        self.lid_watch(h);
        // the energy mode, as it is now
        let mode = self.mode();
        if mode != self.last_mode {
            self.last_mode = mode;
            self.apply();
        }
        // Apps using significant energy: a look every 5 minutes, the report only while Settings shows it
        if self.cfg["appEnergy"].as_bool().unwrap_or(true) {
            if self.ticks % 300 == 1 {
                crate::util::run_async(&["python3", "-I", &crate::util::script("app-energy.py"), "sample"]);
            }
            if self.energy_users > 0 && self.ticks % 10 == 1 && !self.reporting {
                self.reporting = true;
                let send = h.sender(NAME);
                capture_then(script_cmd("app-energy.py", &["report"]), None, move |_, t| send(Box::new(M::Report(t))));
            }
        }
        self.publish(h);
    }

    fn low(&self) -> bool {
        self.on_battery() && self.percent() >= 0.0 && self.percent() <= self.cfg["batterySaverLevel"].as_f64().unwrap_or(20.0)
    }

    fn publish(&mut self, h: &Host) {
        let w = self.winner();
        let mode = self.mode();
        let why = if w.owner == "manual" || w.owner == "default" { String::new() } else { w.why.clone() };
        let reqs: Vec<Value> = self.requests.values().map(|r| json!({"owner": r.owner, "mode": r.mode, "priority": r.priority, "since": r.since, "until": r.until, "why": r.why})).collect();
        let v = json!({
            "hasBattery": self.has_battery(), "percent": self.percent(), "onBattery": self.on_battery(), "mode": mode, "modeName": mode_name(&mode), "why": why,
            "winner": w.owner, "requests": reqs, "ppd": self.ppd, "hasPerformance": self.has_perf, "profile": self.profile, "stage": self.warned,
            "guardActive": self.guard_active, "guardError": self.guard_error, "ownHypridle": self.own_hypridle,
            "userHypridle": tessera_core::util::xdg("XDG_CONFIG_HOME", ".config").join("hypr/hypridle.conf").to_string_lossy(),
            "energyApps": self.energy_apps, "energyMinutes": self.energy_minutes, "lidMine": !self.lid.is_empty(), "sandboxed": tessera_core::env::sandboxed(),
            "saver": {"on": self.saver.on, "mode": self.cfg["saver"], "level": policy::saver_level(&self.cfg), "byHand": self.saver.forced.is_some()},
            "names": {"power-saver": "Low Power", "balanced": "Balanced", "performance": "Performance"},
        });
        if v != self.last {
            crate::bus::put(NAME, v.clone());
            self.last = v;
        }
        let _ = h;
    }
}

fn is_built_in(name: &str) -> bool {
    ["eDP", "LVDS", "DSI"].iter().any(|p| name.starts_with(p))
}

impl Service for Power {
    fn name(&self) -> &'static str {
        NAME
    }
    fn calls(&self) -> &'static [&'static str] {
        &["power"]
    }
    fn start(&mut self, h: &mut Host) {
        h.st.shell.want("status");
        self.cfg = cfg_with_defaults(h.cfg());
        self.idle_key = idle_key(&self.cfg);
        spawn_ppd(h.sender(NAME));
        self.idle_later(h);
        let send = h.sender(NAME);
        self.tick.start(slint::TimerMode::Repeated, Duration::from_secs(1), move || send(Box::new(M::Tick)));
        self.last_mode = self.mode();
    }
    fn config_changed(&mut self, h: &mut Host) {
        let old = std::mem::replace(&mut self.cfg, cfg_with_defaults(h.cfg()));
        if old["critical"] != self.cfg["critical"] {
            self.sync_guard(h);
        }
        let k = idle_key(&self.cfg);
        if k != self.idle_key {
            self.idle_key = k;
            self.idle_later(h);
        }
        // (a new battery saver setting applies now; not before the first
        // battery reading, as the tick does)
        if self.ticks > 0 {
            self.saver_step(h);
        }
        self.publish(h);
    }
    fn call(&mut self, h: &mut Host, call: &str, args: &Value) {
        let a = args_list(args);
        match call.strip_prefix("power.").unwrap_or("") {
            "setMode" => {
                let m = str_arg(&a, 0);
                if ["power-saver", "balanced", "performance"].contains(&m.as_str()) {
                    self.set_mode(&m);
                }
            }
            // the menu bar's energy button: Low Power -> Balanced -> Performance (if there is one) -> Low Power
            "cycleProfile" => {
                let next = match self.profile.as_str() {
                    "power-saver" => "balanced",
                    "balanced" if self.has_perf => "performance",
                    _ => "power-saver",
                };
                self.set_mode(next);
            }
            "ask" => {
                let r = a.first().cloned().unwrap_or(Value::Null);
                if let (Some(o), Some(m)) = (r["owner"].as_str(), r["mode"].as_str()) {
                    self.ask_mode(o, m, r["why"].as_str().unwrap_or(""), r["until"].as_f64().unwrap_or(0.0));
                }
            }
            "release" => self.release(&str_arg(&a, 0)),
            "saverOn" => self.saver_on(),
            "energyUsers" => self.energy_users = (self.energy_users + a.first().and_then(Value::as_i64).unwrap_or(0) as i32).max(0),
            "clearEnergy" => {
                self.energy_apps = json!([]);
                crate::util::run_async(&["python3", "-I", &crate::util::script("app-energy.py"), "clear"]);
            }
            "syncGuard" => self.sync_guard(h),
            "idleLater" => self.idle_later(h),
            "saverToggle" => {
                if self.saver.toggle(self.on_battery()) {
                    self.saver_step(h);
                }
            }
            "saverOff" => {
                self.saver.turn_off();
                self.saver_step(h);
            }
            _ => {}
        }
        // (a choice applies now, not on the next second's tick; the menus show
        // the daemon's profile, so it shows the new one straight away: it was
        // a beat late and looked like the click hadn't taken)
        let mode = self.mode();
        if mode != self.last_mode {
            self.last_mode = mode.clone();
            self.apply();
            if self.ppd && !tessera_core::env::sandboxed() {
                self.profile = mode;
            }
        }
        self.publish(h);
    }
    fn message(&mut self, h: &mut Host, msg: Box<dyn Any + Send>) {
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Tick => self.tick(h),
            M::Ppd(None) => {
                self.ppd = false;
            }
            M::Ppd(Some((profile, perf))) => {
                self.ppd = true;
                self.has_perf = perf;
                self.profile = profile.clone();
                if self.applied.is_empty() {
                    self.apply(); // (Tessera's own answer is put back: the daemon's first report is left from before)
                } else if !tessera_core::env::sandboxed() && profile != self.applied {
                    if now_ms() - self.applied_at < 10000.0 {
                        self.apply(); // (left from before this start)
                    } else {
                        // Changed outside Tessera: your choice now
                        self.applied = profile.clone();
                        self.set_mode(&profile);
                    }
                }
            }
            M::GuardDone(t) => {
                self.guard_active = t.trim() == "active";
                self.guard_error = if self.cfg["critical"].as_bool().unwrap_or(true) && !self.guard_active {
                    "The battery guard isn't running. Check that systemd's user session is working (systemctl --user status tessera-battery-guard).".into()
                } else {
                    String::new()
                };
            }
            M::IdleWrite => self.write_idle(h),
            M::IdleDone(t) => self.own_hypridle = t.trim() == "own",
            M::LidClosed => self.lid_closed(h),
            M::LidOpened => {
                self.lid_is_closed = false;
                self.hot_since = 0.0;
                Self::built_in_screens(h, true)
            }
            M::SaverOff => {
                self.saver.turn_off();
                self.saver_step(h);
            }
            M::Report(t) => {
                self.reporting = false;
                if let Ok(r) = serde_json::from_str::<Value>(&t) {
                    self.energy_apps = r["apps"].clone().as_array().map(|_| r["apps"].clone()).unwrap_or(json!([]));
                    self.energy_minutes = r["minutes"].as_i64().unwrap_or(0);
                }
            }
        }
        self.publish(h);
    }
    fn quit(&mut self, _h: &mut Host) {
        self.tick.stop();
        self.lid.drain(..).for_each(|w| w.stop());
    }
}

/// The hottest CPU sensor, °C (x86_pkg_temp, k10temp/coretemp-like zones, or
/// any thermal zone when those aren't named)
fn cpu_temp_c() -> Option<f64> {
    let mut best: Option<f64> = None;
    for e in std::fs::read_dir("/sys/class/thermal").ok()?.flatten() {
        let d = e.path();
        let kind = std::fs::read_to_string(d.join("type")).unwrap_or_default();
        let kind = kind.trim();
        if !(kind.contains("pkg") || kind.contains("cpu") || kind.contains("x86") || kind.contains("k10") || kind.starts_with("acpitz")) {
            continue;
        }
        if let Some(t) = std::fs::read_to_string(d.join("temp")).ok().and_then(|t| t.trim().parse::<f64>().ok()) {
            let c = t / 1000.0;
            if c > 0.0 && c < 150.0 {
                best = Some(best.map_or(c, |b: f64| b.max(c)));
            }
        }
    }
    best
}

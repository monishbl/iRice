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
//! Calls (prefix "power"): setMode(mode), ask({owner, mode, why, until}),
//! release(owner), saverOn, energyUsers(delta), clearEnergy, syncGuard,
//! idleLater. From System (read off the bus under "system"): gameMode,
//! keepAwake. Asks of others: system.power(["sleep"|"lock"]), sound.play
//! (["battery"]), island.post. State on the bus as "power": {hasBattery,
//! percent, onBattery, mode, modeName, why, winner, requests, ppd,
//! hasPerformance, profile, stage, guardActive, guardError, ownHypridle,
//! userHypridle, energyApps, energyMinutes, lidMine}.

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
}

fn ppd_read(conn: &zbus::blocking::Connection, dest: &'static str, path: &'static str) -> zbus::Result<(String, bool)> {
    use zbus::proxy::CacheProperties;
    let p = zbus::blocking::proxy::Builder::<zbus::blocking::Proxy>::new(conn).destination(dest)?.path(path)?.interface(dest)?.cache_properties(CacheProperties::No).build()?;
    let active: String = p.get_property("ActiveProfile")?;
    let profiles: Vec<std::collections::HashMap<String, zbus::zvariant::OwnedValue>> = p.get_property("Profiles").unwrap_or_default();
    let perf = profiles.iter().any(|m| m.get("Profile").and_then(|v| String::try_from(v.clone()).ok()).as_deref() == Some("performance"));
    Ok((active, perf))
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
    energy_users: i32,
    energy_apps: Value,
    energy_minutes: i64,
    ticks: u64,
    reporting: bool,
    last: Value,
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
            prev_state: 0,
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
            energy_users: 0,
            energy_apps: json!([]),
            energy_minutes: 0,
            ticks: 0,
            reporting: false,
            last: Value::Null,
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
            crate::util::run_async(&["powerprofilesctl", "set", &mode]);
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
            crate::util::run_async(&["notify-send", "-u", "critical", "-a", "Tessera", title, text]);
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
        let user_hypridle = std::env::var_os("XDG_CONFIG_HOME").map(std::path::PathBuf::from).unwrap_or_else(|| crate::util::home().join(".config")).join("hypr/hypridle.conf");
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
            self.lid.push(Watch::start(
                &["setpriv", "--pdeathsig", "TERM", "--", "systemd-inhibit", "--what=handle-lid-switch", "--who=Tessera", "--why=Lid settings (Battery & Power)", "--mode=block", "setpriv", "--pdeathsig", "TERM", "--", "sleep", "infinity"],
                None,
                |_| {},
                |_, _| {},
            ));
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
        let external = h.st.outputs_now().iter().filter(|(_, n)| !is_built_in(n)).count();
        let act = policy::lid_action(!self.on_battery(), external, &self.cfg, Self::caffeine());
        if act == "sleep" {
            h.ask("system.power", json!(["sleep"]));
            return;
        }
        if act == "lock" {
            h.ask("system.power", json!(["lock"]));
        }
        Self::built_in_screens(h, false);
    }

    /// A pass of the one-second tick: the battery, what depends on it
    fn tick(&mut self, h: &mut Host) {
        self.ticks += 1;
        if let Some(b) = h.st.shell.get::<Battery>() {
            self.battery = b.clone();
        }
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
        // A note when it's full (the charger can come out)
        if self.has_battery() && self.battery.state == 4 && self.prev_state != 4 {
            crate::util::run_async(&["notify-send", "-a", "Battery", "-i", "battery-full-charged", "Fully charged", "You can unplug your computer."]);
        }
        self.prev_state = self.battery.state;
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
            "userHypridle": std::env::var_os("XDG_CONFIG_HOME").map(std::path::PathBuf::from).unwrap_or_else(|| crate::util::home().join(".config")).join("hypr/hypridle.conf").to_string_lossy(),
            "energyApps": self.energy_apps, "energyMinutes": self.energy_minutes, "lidMine": !self.lid.is_empty(), "sandboxed": tessera_core::env::sandboxed(),
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
            _ => {}
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
            M::LidOpened => Self::built_in_screens(h, true),
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

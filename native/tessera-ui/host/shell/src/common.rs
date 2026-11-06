//! The shell's shared logic from shell/common/: Paths, Config's reading and
//! writing, UiState, Launch, SettingsMap's aliases, TimeFormat, Daylight,
//! CoolDown's rules, KeptProcess. (The widgets there are Slint's now: PORT-NOTES.md.)

use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

// ── Paths.qml ───────────────────────────────────────────────────────────────

/// $XDG_CONFIG_HOME/tessera (Tessera's settings folder; iRice's while only that one has them)
pub fn config_dir() -> PathBuf {
    tessera_core::theme::config_path().parent().map(Path::to_path_buf).unwrap_or_default()
}

/// $XDG_STATE_HOME/tessera
pub fn state_dir() -> PathBuf {
    tessera_core::util::state_dir()
}

/// $XDG_CACHE_HOME/tessera
pub fn cache_dir() -> PathBuf {
    tessera_core::wallpaper::cache_dir()
}

/// $XDG_DATA_HOME/tessera
pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| home().join(".local/share"));
    let old = config_dir().file_name().is_some_and(|n| n == "irice");
    base.join(if old { "irice" } else { "tessera" })
}

pub fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
}

/// $XDG_RUNTIME_DIR, else /tmp
pub fn runtime_dir() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"))
}

/// The shell's folder (shell/), where assets/scripts/ are
pub fn shell_dir() -> PathBuf {
    tessera_core::wallpaper::shell_dir()
}

/// A script of the shell's (assets/scripts/<name>)
pub fn script(name: &str) -> PathBuf {
    shell_dir().join("assets/scripts").join(name)
}

pub fn sandboxed() -> bool {
    tessera_core::env::sandboxed()
}

pub fn now_ms() -> i64 {
    tessera_core::util::now_ms()
}

// ── Config.qml: reading and writing the settings ────────────────────────────

/// `a.b.c` in a JSON value (Null when any part is missing)
pub fn at<'a>(v: &'a Value, path: &str) -> &'a Value {
    path.split('.').fold(v, |v, k| &v[k])
}

pub fn cfg_str<'a>(cfg: &'a Value, path: &str, default: &'a str) -> &'a str {
    at(cfg, path).as_str().unwrap_or(default)
}

pub fn cfg_bool(cfg: &Value, path: &str, default: bool) -> bool {
    at(cfg, path).as_bool().unwrap_or(default)
}

pub fn cfg_num(cfg: &Value, path: &str, default: f64) -> f64 {
    at(cfg, path).as_f64().unwrap_or(default)
}

/// Set `a.b.c` in a value, making the objects on the way
pub fn put(v: &mut Value, path: &str, x: Value) {
    let keys: Vec<&str> = path.split('.').collect();
    let mut at = v;
    for k in &keys[..keys.len() - 1] {
        if !at[*k].is_object() {
            at[*k] = json!({});
        }
        at = &mut at[*k];
    }
    at[keys[keys.len() - 1]] = x;
}

/// `Config.options.<path> = value`: read config.json, change that key, write
/// it whole (Config.qml writes the adapter 300 ms after a change; the shell
/// and this process both watch the file, so the change comes back as
/// config_changed). Returns whether it was written.
pub fn set_config(path: &str, x: Value) -> bool {
    let file = tessera_core::theme::config_path();
    let mut cfg = std::fs::read_to_string(&file).ok().and_then(|s| serde_json::from_str::<Value>(&s).ok()).unwrap_or_else(|| json!({}));
    if !cfg.is_object() {
        cfg = json!({});
    }
    if at(&cfg, path) == &x {
        return true;
    }
    put(&mut cfg, path, x);
    let text = serde_json::to_string_pretty(&cfg).unwrap_or_default();
    tessera_core::util::write_atomic(&file, text.as_bytes()).is_ok()
}

/// Config.qml's welcome / defaultsVersion upgrades that standalone needs when
/// it reads a file written by an older shell (once, at start)
pub fn upgrade_config() {
    let cfg = tessera_core::theme::read_json();
    if !cfg.is_object() {
        return;
    }
    let version = cfg["defaultsVersion"].as_i64().unwrap_or(0);
    if version >= 3 {
        return;
    }
    if version < 1 {
        set_config("sidebar.booru", json!(false));
    }
    if version < 2 && cfg.get("welcome").is_none() {
        set_config("welcome.done", json!(true));
    }
    if version < 3 {
        if cfg_str(&cfg, "appearance.iconStyle", "default") == "default" {
            set_config("appearance.iconStyle", json!("look"));
        }
        if cfg_str(&cfg, "appearance.cursor", "") == "macos" {
            set_config("appearance.cursor", json!("bibata"));
        }
    }
    set_config("defaultsVersion", json!(3));
}

// ── Theme.qml's dark, Daylight.qml, TimeFormat.qml ──────────────────────────

pub fn dark(cfg: &Value) -> bool {
    let (r, s) = tessera_core::wallpaper::sun();
    tessera_core::wallpaper::dark(cfg, &tessera_core::wallpaper::Day::now(r, s))
}

pub fn material(cfg: &Value) -> bool {
    cfg_str(cfg, "appearance.look", "glass") == "material"
}

/// TimeFormat.hm: "9:41 AM" or "09:41" (Settings → Appearance → 24-hour time)
pub fn hm(cfg: &Value, secs: i64) -> String {
    tessera_core::util::strftime(if cfg_bool(cfg, "appearance.clock24", false) { "%H:%M" } else { "%-I:%M %p" }, secs)
}

// ── Processes ───────────────────────────────────────────────────────────────

/// A command on a thread, output ignored (Quickshell.execDetached)
pub fn run(cmd: &[&str]) {
    let cmd: Vec<String> = cmd.iter().map(|s| s.to_string()).collect();
    run_owned(cmd);
}

pub fn run_owned(cmd: Vec<String>) {
    if cmd.is_empty() {
        return;
    }
    std::thread::spawn(move || {
        let _ = Command::new(&cmd[0]).args(&cmd[1..]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status();
    });
}

/// A command run to its end, its stdout (None when it can't run or fails)
pub fn capture(cmd: &[&str]) -> Option<String> {
    let out = Command::new(cmd.first()?).args(&cmd[1..]).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// A command run to its end on a thread; `done` gets (exit code, stdout)
pub fn run_then(cmd: Vec<String>, done: impl FnOnce(i32, String) + Send + 'static) {
    std::thread::spawn(move || {
        let out = Command::new(&cmd[0]).args(&cmd[1..]).stdin(Stdio::null()).stderr(Stdio::null()).output();
        match out {
            Ok(o) => done(o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned()),
            Err(_) => done(127, String::new()),
        }
    });
}

pub fn on_path(bin: &str) -> bool {
    tessera_core::util::on_path(bin)
}

// ── Launch.qml ──────────────────────────────────────────────────────────────

/// The variables the tty session sandboxes, dropped so what Tessera starts gets
/// the user's real folders (Launch.scrub)
fn scrub() -> Vec<String> {
    if tessera_core::env::var("TTY").as_deref() != Some("1") {
        return Vec::new();
    }
    let mut v = vec!["env".to_string()];
    for k in ["XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_STATE_HOME", "XDG_CACHE_HOME", "TESSERA_SANDBOX", "TESSERA_ROOT", "TESSERA_TTY", "IRICE_SANDBOX", "IRICE_ROOT", "IRICE_TTY", "QML_IMPORT_PATH", "QML2_IMPORT_PATH"] {
        v.push("-u".into());
        v.push(k.into());
    }
    v
}

/// A command line in a scope of its own (assets/scripts/app-scope.sh)
pub fn scoped(cmd: &[String], id: &str) -> Vec<String> {
    let mut v = vec![script("app-scope.sh").to_string_lossy().into_owned(), "--id".into(), id.into(), "--".into()];
    v.extend(scrub());
    v.extend(cmd.iter().cloned());
    v
}

/// Launch.run
pub fn launch_run(cmd: &[&str], id: &str) {
    let cmd: Vec<String> = cmd.iter().map(|s| s.to_string()).collect();
    run_owned(scoped(&cmd, id));
}

/// Launch.open: a link, file or folder in its default app
pub fn launch_open(url: &str) {
    launch_run(&["xdg-open", url], "open");
}

/// Launch.linkAllowed: only web and mail links from outside
pub fn link_allowed(url: &str) -> bool {
    let u = url.trim().to_ascii_lowercase();
    u.starts_with("http://") || u.starts_with("https://") || u.starts_with("mailto:")
}

/// Launch.terminal: a command in the chosen terminal (assets/scripts/open-app.sh)
pub fn launch_terminal(args: &[&str], hold: bool) {
    let mut cmd: Vec<String> = vec!["env".into(), format!("TESSERA_USER_DIR={}", config_dir().display()), "bash".into(), script("open-app.sh").to_string_lossy().into_owned(), "terminal".into()];
    if hold {
        cmd.push("--hold".into());
    }
    cmd.push("--".into());
    cmd.extend(args.iter().map(|s| s.to_string()));
    run_owned(scoped(&cmd, "terminal"));
}

// ── SettingsMap.js: where an older page id goes now ─────────────────────────

/// (page, part) an older settings page id is now; None when it is current
pub fn settings_alias(page: &str) -> Option<(&'static str, &'static str)> {
    Some(match page {
        "dock" => ("menubar", "Dock"),
        "controlcenter" => ("menubar", "Quick Settings"),
        "menubar/island" => ("notifications/island", ""),
        "appearance/windows" => ("windowrules", "Windows, Stages and Overview"),
        "appearance/cooldown" => ("power/cooldown", ""),
        "bluetooth" => ("network", "Bluetooth"),
        "trackpad" => ("mouse", "Trackpad"),
        "trackpad/gestures" => ("mouse/gestures", ""),
        "dictation" => ("shortcuts", "Dictation"),
        "sharing" => ("phone", "Sharing"),
        "google" => ("accounts", "Google"),
        "icloud" => ("accounts", "iCloud"),
        "microsoft" => ("accounts", "Microsoft 365"),
        "sidebar" => ("ai", "Sidebar"),
        "loginitems" => ("about/loginitems", ""),
        "bugreport" => ("about/bugreport", ""),
        _ => return None,
    })
}

// ── UiState.qml: state.json, changed through journals and state-merge.py ────

/// One key of state.json (Null when there is none)
pub fn ui_get(key: &str) -> Value {
    std::fs::read_to_string(state_dir().join("state.json")).ok().and_then(|s| serde_json::from_str::<Value>(&s).ok()).map(|v| v[key].clone()).unwrap_or(Value::Null)
}

/// `UiState.data.<key> = value`: a journal first (a crash can't lose it), then
/// the merge (assets/scripts/state-merge.py, which two shells can share)
pub fn ui_set(key: &str, value: Value) {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    let id = format!("rs{}-{}", std::process::id(), now_ms());
    let dir = state_dir();
    let journal = json!({ key: { "revision": [now_ms(), id, seq], "value": value } });
    let _ = std::fs::create_dir_all(dir.join("pending"));
    if tessera_core::util::write_atomic(&dir.join("pending").join(format!("{id}-{seq}.json")), journal.to_string().as_bytes()).is_err() {
        return;
    }
    let merger = script("state-merge.py").to_string_lossy().into_owned();
    run_owned(vec!["python3".into(), "-I".into(), merger, dir.to_string_lossy().into_owned()]);
}

/// UiState.widget(id): an overlay widget's state, with the defaults
pub fn overlay_widget(id: &str) -> Value {
    let mut w = json!({ "open": false, "pinned": false, "clickthrough": false, "x": 100, "y": 100, "w": 320, "h": 240 });
    let defaults = overlay_defaults();
    if let Some(d) = defaults.get(id) {
        w = d.clone();
    }
    if let Value::Object(saved) = &ui_get("overlay")[id] {
        for (k, v) in saved {
            w[k] = v.clone();
        }
    }
    w
}

/// UiState's overlay defaults (id -> { open, pinned, clickthrough, x, y, w, h })
pub fn overlay_defaults() -> Value {
    json!({
        "notes": { "open": false, "pinned": false, "clickthrough": false, "x": 80, "y": 120, "w": 340, "h": 300 },
        "resources": { "open": true, "pinned": false, "clickthrough": false, "x": 440, "y": 120, "w": 320, "h": 260 },
        "recorder": { "open": false, "pinned": false, "clickthrough": false, "x": 780, "y": 120, "w": 300, "h": 170 },
        "mixer": { "open": false, "pinned": false, "clickthrough": false, "x": 1100, "y": 120, "w": 340, "h": 300 },
        "image": { "open": false, "pinned": false, "clickthrough": false, "x": 80, "y": 460, "w": 360, "h": 280 },
        "crosshair": { "open": false, "pinned": false, "clickthrough": false, "x": 460, "y": 420, "w": 300, "h": 230 },
        "fps": { "open": false, "pinned": false, "clickthrough": false, "x": 780, "y": 330, "w": 300, "h": 170 }
    })
}

/// UiState.data.overlay as the QML builds it: the defaults, each widget's saved fields on top
pub fn overlay_all() -> Value {
    let mut all = overlay_defaults();
    if let Value::Object(saved) = ui_get("overlay") {
        for (id, w) in saved {
            if let Value::Object(w) = w {
                for (k, v) in w {
                    all[id.as_str()][k] = v;
                }
            }
        }
    }
    all
}

pub fn crosshair_state() -> Value {
    let mut c = json!({ "enabled": false, "color": "#30D158", "size": 12, "thickness": 2, "gap": 4, "dot": true });
    if let Value::Object(saved) = ui_get("crosshair") {
        for (k, v) in saved {
            c[k] = v;
        }
    }
    c
}

// ── CoolDown.qml's rules ────────────────────────────────────────────────────

/// Cool-down's state: what the glass, motion and live wallpaper read
#[derive(Default, Clone, Debug, PartialEq)]
pub struct Cool {
    /// set by the island's temperature watch
    pub hot: bool,
    pub accepted: bool,
    pub declined: bool,
    /// the Control Center control
    pub manual: bool,
    /// turned off by hand while hot or on battery
    pub snoozed: bool,
    pub on_battery: bool,
}

impl Cool {
    pub fn hot_on(&self, cfg: &Value) -> bool {
        cfg_bool(cfg, "cooling.whenHot", true) && self.hot && (cfg_str(cfg, "cooling.hotAction", "auto") != "ask" || self.accepted)
    }
    pub fn automatic(&self, cfg: &Value) -> bool {
        self.hot_on(cfg) || (cfg_bool(cfg, "cooling.onBattery", false) && self.on_battery)
    }
    pub fn active(&self, cfg: &Value) -> bool {
        self.manual || (self.automatic(cfg) && !self.snoozed)
    }
    /// CoolDown.toggle
    pub fn toggle(&mut self, cfg: &Value) {
        if self.active(cfg) {
            self.manual = false;
            if self.automatic(cfg) {
                self.snoozed = true;
            }
        } else {
            self.snoozed = false;
            self.manual = true;
        }
    }
    /// keep | frosted | solid
    pub fn glass(&self, cfg: &Value) -> String {
        if self.active(cfg) {
            let g = cfg_str(cfg, "cooling.glass", "frosted");
            if ["keep", "frosted", "solid"].contains(&g) {
                return g.to_string();
            }
        }
        "keep".into()
    }
    pub fn motion_scale(&self, cfg: &Value) -> f64 {
        if self.active(cfg) && cfg_bool(cfg, "cooling.shortAnimations", true) {
            0.6
        } else {
            1.0
        }
    }
}

// ── KeptProcess.qml ─────────────────────────────────────────────────────────

/// A long-lived process kept running: started again after it ends (or fails to
/// start), waiting longer each time it ends quickly (10 s, 20 s… up to 5 min;
/// back to 10 s after a run of a minute or more). Its stdout lines go to `on_line`
/// (on its thread); `write` feeds its stdin.
pub struct Kept {
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    stdin: std::sync::mpsc::Sender<String>,
    /// how many times it ended within a minute of starting, in a row
    pub quick_ends: std::sync::Arc<std::sync::atomic::AtomicU32>,
}

/// The wait before the next start after `quick_ends` quick ends (10 s, 20 s… 5 min)
pub fn kept_wait(quick_ends: u32) -> std::time::Duration {
    std::time::Duration::from_millis((10_000u64 << quick_ends.min(10)).min(300_000))
}

impl Kept {
    pub fn start(cmd: Vec<String>, on_line: impl Fn(String) + Send + 'static, on_exit: impl Fn(bool) + Send + 'static) -> Kept {
        use std::io::{BufRead, Write};
        use std::sync::atomic::Ordering;
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let quick = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        // (in a Mutex: each run's writer thread borrows it, and a Receiver alone isn't Sync)
        let rx = std::sync::Mutex::new(rx);
        let (stop2, quick2) = (stop.clone(), quick.clone());
        std::thread::spawn(move || {
            while !stop2.load(Ordering::SeqCst) {
                let began = std::time::Instant::now();
                let child = Command::new(&cmd[0]).args(&cmd[1..]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn();
                let started = child.is_ok();
                if let Ok(mut child) = child {
                    let mut stdin = child.stdin.take();
                    let stdout = child.stdout.take();
                    // (what is written to us goes on to its stdin)
                    let done = std::sync::atomic::AtomicBool::new(false);
                    std::thread::scope(|sc| {
                        sc.spawn(|| loop {
                            let got = rx.lock().map(|r| r.recv_timeout(std::time::Duration::from_millis(300)));
                            let Ok(got) = got else { break };
                            match got {
                                Ok(l) => {
                                    if let Some(i) = stdin.as_mut() {
                                        let _ = i.write_all(l.as_bytes());
                                        let _ = i.flush();
                                    }
                                }
                                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                                    if done.load(Ordering::SeqCst) {
                                        break;
                                    }
                                }
                                Err(_) => break,
                            }
                        });
                        if let Some(o) = stdout {
                            for l in std::io::BufReader::new(o).lines().map_while(Result::ok) {
                                on_line(l);
                            }
                        }
                        done.store(true, Ordering::SeqCst);
                    });
                    let _ = child.wait();
                }
                if stop2.load(Ordering::SeqCst) {
                    break;
                }
                on_exit(started);
                if began.elapsed() >= std::time::Duration::from_secs(60) {
                    quick2.store(0, Ordering::SeqCst);
                }
                let wait = kept_wait(quick2.fetch_add(1, Ordering::SeqCst));
                let until = std::time::Instant::now() + wait;
                while std::time::Instant::now() < until && !stop2.load(Ordering::SeqCst) {
                    std::thread::sleep(std::time::Duration::from_millis(250));
                }
            }
        });
        Kept { stop, stdin: tx, quick_ends: quick }
    }
    pub fn write(&self, line: &str) {
        let _ = self.stdin.send(format!("{line}\n"));
    }
    pub fn stop(&self) {
        self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

// ── The wallpaper's tone (Wallpaper.qml's ColorQuantizer + pickAccent) ──────

/// The picture's most colourful shade as "#rrggbb": the picture shrunk to 64 px,
/// its colours binned (4 levels a channel), the busiest eight scored by
/// saturation × (0.4 + value), the best kept. None when it can't be read.
pub fn wallpaper_tone(path: &str) -> Option<String> {
    let img = image::open(path).ok()?.thumbnail(64, 64).to_rgb8();
    let mut bins: std::collections::HashMap<(u8, u8, u8), (u64, [u64; 3])> = std::collections::HashMap::new();
    for p in img.pixels() {
        let e = bins.entry((p[0] >> 6, p[1] >> 6, p[2] >> 6)).or_insert((0, [0; 3]));
        e.0 += 1;
        for i in 0..3 {
            e.1[i] += p[i] as u64;
        }
    }
    let mut top: Vec<(u64, [f64; 3])> = bins.into_values().map(|(n, s)| (n, [s[0] as f64 / n as f64, s[1] as f64 / n as f64, s[2] as f64 / n as f64])).collect();
    top.sort_by(|a, b| b.0.cmp(&a.0));
    top.truncate(8);
    let mut best = ([128.0, 128.0, 128.0], -1.0f64);
    for (_, c) in top {
        let (mx, mn) = (c.iter().cloned().fold(0.0, f64::max), c.iter().cloned().fold(255.0, f64::min));
        let (value, sat) = (mx / 255.0, if mx == 0.0 { 0.0 } else { (mx - mn) / mx });
        let score = sat * (0.4 + value);
        if score > best.1 {
            best = (c, score);
        }
    }
    let c = best.0;
    Some(format!("#{:02x}{:02x}{:02x}", c[0].round() as u8, c[1].round() as u8, c[2].round() as u8))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_in_a_value() {
        let mut v = json!({});
        put(&mut v, "a.b.c", json!(3));
        assert_eq!(at(&v, "a.b.c"), &json!(3));
        assert_eq!(at(&v, "a.x.c"), &Value::Null);
        assert_eq!(cfg_str(&v, "a.b.c", "d"), "d");
        assert!(cfg_bool(&v, "nothing", true));
    }

    #[test]
    fn old_settings_pages_go_to_new_ones() {
        assert_eq!(settings_alias("dock"), Some(("menubar", "Dock")));
        assert_eq!(settings_alias("appearance"), None);
    }

    #[test]
    fn only_web_and_mail_links_open() {
        assert!(link_allowed(" https://example.org"));
        assert!(link_allowed("MAILTO:a@b.c"));
        assert!(!link_allowed("file:///etc/passwd"));
        assert!(!link_allowed("smb://x"));
    }

    #[test]
    fn cool_down_toggles_like_the_control() {
        let cfg = json!({"cooling": {"whenHot": true, "hotAction": "auto", "glass": "frosted", "shortAnimations": true}});
        let mut c = Cool::default();
        assert_eq!(c.glass(&cfg), "keep");
        c.toggle(&cfg);
        assert!(c.active(&cfg));
        assert_eq!(c.glass(&cfg), "frosted");
        assert_eq!(c.motion_scale(&cfg), 0.6);
        c.hot = true;
        c.toggle(&cfg); // off by hand while hot: stays off until it cools
        assert!(!c.active(&cfg) && c.snoozed);
        // ask mode: hot alone does nothing until accepted
        let ask = json!({"cooling": {"whenHot": true, "hotAction": "ask"}});
        let mut d = Cool { hot: true, ..Default::default() };
        assert!(!d.active(&ask));
        d.accepted = true;
        assert!(d.active(&ask));
    }

    #[test]
    fn kept_waits_grow_then_stop_at_five_minutes() {
        assert_eq!(kept_wait(0).as_secs(), 10);
        assert_eq!(kept_wait(1).as_secs(), 20);
        assert_eq!(kept_wait(2).as_secs(), 40);
        assert_eq!(kept_wait(9).as_secs(), 300);
    }

    #[test]
    fn overlay_defaults_are_complete() {
        let d = overlay_defaults();
        for id in ["notes", "resources", "recorder", "mixer", "image", "crosshair", "fps"] {
            assert!(d[id]["w"].is_number(), "{id}");
        }
    }
}

//! The small surfaces' bridges to the shell: what each QML module did beside
//! its Rust twin. Welcome (modules/welcome/Welcome.qml), Overlay
//! (modules/overlay/Overlay.qml), Lock and the login intro (modules/lock/Lock.qml,
//! LoginIntro.qml, common/LockFx.qml), the hot corners (modules/hotcorners/HotCorners.qml),
//! the OSD's and the password prompt's IPC (modules/osd/Osd.qml, modules/polkit/Polkit.qml),
//! the background's sunrise (modules/background/Background.qml), Launch.qml's
//! opening of links, FilePicker.qml, Wallpaper.qml's setters, and Settings'
//! "look" and "notify" page groups (modules/settings/RustBridgeLook.qml,
//! RustBridgeNotify.qml).
//!
//! Calls (arguments an array), by prefix:
//!   welcome.mode/wall/pick/later/finish/open   overlay.widget/crosshair/record/pasteImage/fps
//!   hotcorners.run [action, screen]            lock.set [bool] / lock / unlock / sessionStarted / playIntro / ui
//!   launch.open / openLink [url]               picker.pick [{reply, id, folder, multiple, title, start, filter}]
//!   wallpaper.set [path] / openFolder / useSet [set]     background.sun [sunrise, sunset]
//!   osd.preview [kind]  polkit.preview [finger?]  open / reveal [path]  show [panel]

use crate::common::*;
use crate::panels::{drawing, first_output, focused, Model};
use serde_json::{json, Value};
use std::any::Any;
use std::collections::BTreeMap;
use tessera_core::host::{Host, Service};

/// What the settings page groups may change besides their own state
pub struct Ctx<'a> {
    pub panels: &'a mut Model,
}

// ── Wallpaper.qml's setters ─────────────────────────────────────────────────

/// Wallpaper.set: choosing a picture turns dynamic and live wallpapers off
pub fn wallpaper_set(path: &str) {
    set_config("wallpaper.dynamic", json!("off"));
    set_config("wallpaper.live", json!(""));
    set_config("wallpaper.path", json!(path));
}

/// Wallpaper.useSet: a pair (light and dark) or a folder through the day
pub fn wallpaper_use_set(s: &Value) {
    set_config("wallpaper.live", json!(""));
    if s["kind"] == "pair" {
        set_config("wallpaper.lightPath", s["light"].clone());
        set_config("wallpaper.darkPath", s["dark"].clone());
        set_config("wallpaper.dynamic", json!("appearance"));
    } else {
        set_config("wallpaper.dynamicFolder", s["folder"].clone());
        set_config("wallpaper.dynamic", json!("time"));
    }
}

/// The sets in the wallpaper folder (assets/scripts/wallpaper-sets.sh): newest macOS-style names first, then by name
pub fn parse_sets(text: &str) -> Vec<Value> {
    const ORDER: &[&str] = &["tahoe beach", "tahoe", "sequoia", "sonoma", "ventura", "monterey", "big sur", "catalina", "mojave"];
    let rank = |n: &str| ORDER.iter().position(|o| *o == n.to_lowercase()).unwrap_or(ORDER.len());
    let mut sets: Vec<Value> = text
        .lines()
        .filter(|l| !l.is_empty())
        .filter_map(|l| {
            let mut p = l.split('\t');
            let (kind, name, a, b) = (p.next()?, p.next()?, p.next()?, p.next().unwrap_or(""));
            Some(if kind == "pair" { json!({"kind": kind, "name": name, "light": a, "dark": b, "preview": a}) } else { json!({"kind": kind, "name": name, "folder": a, "preview": b}) })
        })
        .collect();
    sets.sort_by(|x, y| {
        let (nx, ny) = (x["name"].as_str().unwrap_or(""), y["name"].as_str().unwrap_or(""));
        rank(nx).cmp(&rank(ny)).then_with(|| nx.to_lowercase().cmp(&ny.to_lowercase()))
    });
    sets
}

/// The welcome screen's wallpapers: the Tahoe set through the day, three of
/// its pictures, and Material's light and dark pair (where it's there)
pub fn welcome_walls(tahoe: &str, material: &str, dark: bool) -> Vec<Value> {
    let mut w = vec![json!({"id": "time", "name": "Through the day", "preview": if tahoe.is_empty() { String::new() } else { format!("{tahoe}/tahoe-dawn.jpg") }})];
    for (n, name) in [("day", "Day"), ("dusk", "Dusk"), ("night", "Night")] {
        let p = if tahoe.is_empty() { String::new() } else { format!("{tahoe}/tahoe-{n}.jpg") };
        w.push(json!({"id": n, "name": name, "preview": p, "path": p}));
    }
    if !material.is_empty() {
        w.push(json!({"id": "material", "name": "Material, light and dark", "preview": format!("{material}/material-{}.jpg", if dark { "dark" } else { "light" }), "light": format!("{material}/material-light.jpg"), "dark": format!("{material}/material-dark.jpg")}));
    }
    w
}

// ── Settings: the "look" and "notify" page groups ───────────────────────────

/// `act` of a page group (cx.ask("look", what, args))
pub fn settings_act(h: &mut Host, ctx: &mut Ctx, group: &str, what: &str, a: &[Value]) {
    let s = |i: usize| a.get(i).and_then(Value::as_str).unwrap_or("").to_string();
    if group == "notify" {
        match what {
            // [sound] (a preview, played even when that sound is off)
            "play" => h.ask("sound.play", json!([s(0), true])),
            // [id] (a sound to log in to: "" is Tessera's own)
            "playStartup" => h.ask("sound.playStartup", json!([s(0)])),
            "focusOn" => h.ask("focus.activate", json!([if s(0).is_empty() { "dnd".to_string() } else { s(0) }, 0, false])),
            "focusOff" => h.ask("focus.deactivate", json!([])),
            "clearAll" => h.ask("notify.clearAll", json!([])),
            "setMode" => {
                if a.get(0).is_some_and(Value::is_string) && a.get(1).is_some_and(Value::is_object) {
                    h.ask("focus.setMode", json!([s(0), a[1]]));
                }
            }
            "addMode" => {
                if !s(0).trim().is_empty() {
                    h.ask("focus.addMode", json!([s(0).trim()]));
                }
            }
            "removeMode" => h.ask("focus.removeMode", json!([s(0)])),
            "restoreMode" => h.ask("focus.restoreMode", json!([a.get(0).cloned().unwrap_or(Value::Null)])),
            "installHyprbars" => h.ask("decorations.installPlugin", json!([])),
            other => eprintln!("tessera-ui: host: settings (notify): no act {other}"),
        }
        return;
    }
    match what {
        // (Quick Settings' Edit Controls)
        "editControls" => {
            ctx.panels.settings_open = false;
            ctx.panels.cc_editing = true;
            let (f, first) = (focused(h), first_output(h));
            ctx.panels.show("controlCenter", None, None, None, &f, &first);
        }
        // a picture of the desktop in each; switch-look.sh restarts the shell in it
        "switchLook" => {
            let look = s(0);
            if (look == "glass" || look == "material") && cfg_str(h.cfg(), "appearance.look", "glass") != look {
                set_config("appearance.look", json!(look));
                run_owned(vec!["setsid".into(), "-f".into(), script("switch-look.sh").to_string_lossy().into_owned(), look]);
            }
        }
        "repairShell" => h.ask("look.later", json!([800, "system.repairShell", []])),
        "installEmoji" => h.ask("system.installEmoji", json!([s(0)])),
        "buildPlugin" => {
            let root = tessera_core::env::var("ROOT").unwrap_or_else(|| data_dir().join("src").to_string_lossy().into_owned());
            launch_terminal(&["sh", "-c", "\"$1\" --load", "sh", &format!("{root}/tools/build-plugin.sh")], true);
        }
        "setWallpaper" => {
            if s(0).starts_with('/') {
                wallpaper_set(&s(0));
            }
        }
        "loginWallpaper" => h.ask("look.loginWallpaper", json!([])),
        "editWidgets" => {
            ctx.panels.settings_open = false;
            ctx.panels.screen = first_output(h);
            ctx.panels.widget_editing = true;
        }
        "shelf" => h.ask("shelf.toggle", json!([])),
        "previewLogin" => h.ask("system.previewLogin", json!([])),
        "bootRefresh" => h.ask("system.bootRefresh", json!([])),
        "bootSet" => h.ask("system.bootSet", json!([s(0)])),
        "playIntro" => {
            if !cfg_bool(h.cfg(), "appearance.reduceMotion", false) {
                h.ask("lock.playIntro", json!([]));
            }
        }
        "lock" => h.ask("lock.lock", json!([])),
        other => eprintln!("tessera-ui: host: settings (look): no act {other}"),
    }
}

/// What only the shell knows for the "look" pages (their `cx.feed["look"]`),
/// the part this crate owns; the other host crates add theirs with `settings.feed`
pub fn look_feed(h: &Host) -> Value {
    let cfg = h.cfg();
    let (sunrise, sunset) = tessera_core::wallpaper::sun();
    let tone = h.state("wallpaper.tone").as_str().unwrap_or("").to_string();
    let dark = dark(cfg);
    let accent = if tone.len() >= 7 { tessera_core::theme::snap_accent(&tone, dark) } else { String::new() };
    let screens: Vec<Value> = h
        .st
        .outputs_now()
        .into_iter()
        .map(|(o, name)| {
            let info = h.st.outputs.info(&o);
            let (w, hgt) = info.as_ref().and_then(|i| i.logical_size).unwrap_or((0, 0));
            json!({"name": name, "model": info.as_ref().map(|i| i.model.clone()).unwrap_or_default(), "width": w, "height": hgt})
        })
        .collect();
    json!({
        "cooldownGlass": h.state("cooldown.glass"),
        "sunrise": sunrise, "sunset": sunset,
        "path": h.state("wallpaper.path"),
        "seed": tone, "accent": accent, "tone": h.state("wallpaper.tone"),
        "glassPlugin": h.st.shell.plugin,
        "pickerBusy": h.state("picker.busy"),
        "sets": h.state("wallpaper.sets"),
        "directory": tessera_core::wallpaper::directory(cfg),
        "rootHelper": h.state("look.rootHelper"),
        "loginStatus": h.state("look.loginStatus"),
        "screens": screens
    })
}

/// A page group's feed with the other crates' fields on top of this crate's
pub fn merged(mine: Value, others: Option<&Value>) -> Value {
    let mut m = mine;
    if let (Value::Object(m), Some(Value::Object(o))) = (&mut m, others) {
        for (k, v) in o {
            m.insert(k.clone(), v.clone());
        }
    }
    m
}

// ── The service ─────────────────────────────────────────────────────────────

#[derive(Clone)]
enum Msg {
    Timer(&'static str, u64),
    Delayed(String, Value),
    Dirs(String, String),
    Sets(Vec<Value>),
    Picked(Value, Option<Value>),
    LoginStatus(String),
    RootHelper(bool),
}

#[derive(Default)]
pub struct Bridges {
    gens: BTreeMap<&'static str, u64>,
    // welcome
    w_shown: bool,
    w_step: i64,
    w_look: String,
    w_where: String,
    tahoe_dir: String,
    material_dir: String,
    // lock (Lock.qml) and what it keeps of LockFx
    locked: bool,
    held: bool,
    leaving: bool,
    rust_lock: bool,
    desktop_zero: bool,
    lock_zoom: bool,
    // login intro
    intro_shown: bool,
    intro_playing: bool,
    chimed: bool,
    // background
    sun: Option<(i64, i64)>,
    // overlay
    image_stamp: i64,
    // wallpaper sets, the root helper, the login screen's status
    sets: Vec<Value>,
    sets_for: String,
    root_helper: bool,
    pub login_status: String,
    picking: bool,
    fps_busy: bool,
    sent_overlay: String,
}

impl Bridges {
    fn later(&mut self, h: &Host, kind: &'static str, ms: u64) {
        let g = self.gens.entry(kind).or_insert(0);
        *g += 1;
        let (gen, send) = (*g, h.sender("Bridges"));
        slint::Timer::single_shot(std::time::Duration::from_millis(ms), move || send(Box::new(Msg::Timer(kind, gen))));
    }

    fn cancel(&mut self, kind: &'static str) {
        *self.gens.entry(kind).or_insert(0) += 1;
    }

    // ── LockFx: the steps the Rust background settles with ──────────────

    fn lockfx(&mut self, h: &mut Host, step: &str, ms: i64) {
        match step {
            "prepare" => self.desktop_zero = true,
            _ => self.desktop_zero = false,
        }
        let at = now_ms();
        h.tell("background", json!({"settle": step, "zoom": self.lock_zoom, "ms": ms, "at": at}));
    }

    // ── Lock.qml: lock and unlock, with their transitions ───────────────

    fn set_locked(&mut self, h: &mut Host, locked: bool) {
        if self.locked == locked {
            return;
        }
        self.locked = locked;
        h.set_state("locked", json!(locked));
        self.cancel("leave");
        if self.intro_shown {
            self.finish_intro(h); // (the lock plays its own)
        }
        if locked {
            self.leaving = false;
            if self.held {
                return;
            }
            // Acquire the compositor lock immediately. Animation must never
            // leave an unlocked, input-transparent password-entry interval.
            self.set_held(h, true);
            self.lock_zoom = true;
            self.lockfx(h, "rest", 0);
        } else {
            if !self.held {
                // locked and unlocked again before the lock showed
                self.lockfx(h, "reveal", 300);
                return;
            }
            let t = cfg_str(h.cfg(), "lock.unlockTransition", "zoom").to_string();
            if t == "none" {
                self.lockfx(h, "rest", 0);
                self.set_held(h, false);
                return;
            }
            // Lock → desktop: while the lock's content lifts away, the desktop
            // is set back behind it, so the first frame without the lock looks
            // just like the last one
            self.lock_zoom = t == "zoom";
            self.lockfx(h, "prepare", 0);
            self.leaving = true;
            self.tell_lock(h);
            self.later(h, "leave", 220);
        }
    }

    fn set_held(&mut self, h: &mut Host, held: bool) {
        if self.held == held {
            return;
        }
        self.held = held;
        // (whoever draws the lock as it begins keeps it)
        self.rust_lock = held && drawing(h, "lock");
        self.tell_lock(h);
    }

    fn tell_lock(&self, h: &mut Host) {
        let (finger, face) = (h.state("fingerprint.lockScreen").as_bool().unwrap_or(false), h.state("face.lockScreen").as_bool().unwrap_or(false));
        h.tell("lock", json!({"held": self.held && self.rust_lock, "leaving": self.leaving, "finger": finger, "face": face}));
    }

    /// What the lock screen (Rust) says: secure, unlocked, sound, refused
    fn lock_event(&mut self, h: &mut Host, e: &Value) {
        if let Some(secure) = e["secure"].as_bool() {
            h.set_state("lockSecure", json!(secure));
            if secure && self.locked {
                self.lockfx(h, "rest", 0);
            } else if !secure && !self.locked {
                self.later(h, "reveal", 60);
            }
        }
        if e["unlocked"] == true && self.rust_lock {
            self.set_locked(h, false);
        }
        if e["sound"] == "error" {
            h.ask("sound.play", json!(["error"]));
        }
        if e["refused"] == true && self.rust_lock {
            eprintln!("tessera-ui: lock: couldn't lock the session; trying again");
            self.rust_lock = false;
        }
    }

    // ── LoginIntro.qml ──────────────────────────────────────────────────

    fn intro_length(&self, h: &Host) -> u64 {
        if material(h.cfg()) { 1000 } else { 1300 }
    }

    fn pace() -> u64 {
        tessera_core::env::var("INTRO_PACE").and_then(|p| p.parse::<f64>().ok()).map_or(1, |p| p.max(1.0) as u64)
    }

    fn arm_intro(&mut self, h: &mut Host) {
        self.lock_zoom = false;
        self.lockfx(h, "prepare", 0); // (fading in: zooming the wallpaper itself redraws every glass surface each frame)
        self.intro_shown = true;
        self.later(h, "introHold", 900);
        if drawing(h, "lock") {
            h.tell("lock", json!({"intro": "arm"}));
        }
    }

    fn maybe_play(&mut self, h: &mut Host) {
        if self.intro_shown && !self.intro_playing && self.chimed {
            self.play_intro(h);
        }
    }

    fn play_intro(&mut self, h: &mut Host) {
        self.cancel("introHold");
        self.intro_playing = true;
        let pace = Self::pace();
        let mat = material(h.cfg());
        self.later(h, "introReveal", (if mat { 150 } else { 350 }) * pace);
        let len = self.intro_length(h) * pace;
        // (a second timer kind for the end: the clock)
        let g = self.gens.entry("introDone").or_insert(0);
        *g += 1;
        let (gen, send) = (*g, h.sender("Bridges"));
        slint::Timer::single_shot(std::time::Duration::from_millis(len), move || send(Box::new(Msg::Timer("introDone", gen))));
        h.tell("lock", json!({"intro": "play", "pace": pace}));
    }

    fn finish_intro(&mut self, h: &mut Host) {
        for k in ["introHold", "introReveal", "introDone"] {
            self.cancel(k);
        }
        self.intro_playing = false;
        self.intro_shown = false;
        h.tell("lock", json!({"intro": "stop"}));
        if !self.locked {
            self.lockfx(h, "rest", 0);
        }
    }

    fn preview_intro(&mut self, h: &mut Host) {
        if self.intro_shown {
            return;
        }
        self.arm_intro(h);
        self.chimed = true;
        self.maybe_play(h);
    }

    // ── Welcome.qml ─────────────────────────────────────────────────────

    fn material_available(&self, h: &Host) -> bool {
        material(h.cfg()) || !self.material_dir.is_empty()
    }

    fn welcome_open(&mut self, h: &mut Host) {
        if self.w_shown {
            return;
        }
        self.w_look = cfg_str(h.cfg(), "appearance.look", "glass").to_string();
        self.w_step = 0;
        let f = focused(h);
        self.w_where = if f.is_empty() { first_output(h) } else { f };
        self.w_shown = true;
        self.tell_welcome(h);
    }

    fn tell_welcome(&self, h: &mut Host) {
        if self.w_shown {
            let avail = self.material_available(h);
            h.tell("welcome", json!({"open": true, "screen": self.w_where, "look": self.w_look, "materialAvailable": avail, "tahoeDir": self.tahoe_dir, "materialDir": self.material_dir}));
        } else {
            h.tell("welcome", json!({"open": false}));
        }
    }

    fn welcome_finish(&mut self, h: &mut Host) {
        set_config("welcome.done", json!(true));
        self.w_shown = false;
        let look = self.w_look.clone();
        if look != cfg_str(h.cfg(), "appearance.look", "glass") && (look == "glass" || self.material_available(h)) {
            set_config("appearance.look", json!(look));
            run_owned(vec!["setsid".into(), "-f".into(), script("switch-look.sh").to_string_lossy().into_owned(), look]);
        }
        self.tell_welcome(h);
    }

    fn welcome_call(&mut self, h: &mut Host, what: &str, a: &Value) {
        let s = |i: usize| a.get(i).and_then(Value::as_str).unwrap_or("").to_string();
        match what {
            "mode" => {
                if ["light", "dark", "auto"].contains(&s(0).as_str()) {
                    set_config("appearance.mode", json!(s(0)));
                }
            }
            "wall" => {
                let walls = welcome_walls(&self.tahoe_dir, &self.material_dir, dark(h.cfg()));
                if let Some(w) = walls.iter().find(|w| w["id"] == s(0).as_str()) {
                    if w["id"] == "time" {
                        set_config("wallpaper.live", json!(""));
                        set_config("wallpaper.dynamicFolder", json!(""));
                        set_config("wallpaper.dynamic", json!("time"));
                    } else if w["id"] == "material" {
                        wallpaper_use_set(&json!({"kind": "pair", "light": w["light"], "dark": w["dark"]}));
                    } else if let Some(p) = w["path"].as_str().filter(|p| !p.is_empty()) {
                        wallpaper_set(p);
                    }
                }
            }
            "pick" => h.ask("picker.pick", json!([{"title": "Choose a wallpaper", "start": "~/Pictures", "filter": "Images|*.jpg *.jpeg *.png *.webp", "reply": "wallpaper"}])),
            "later" => {
                if self.w_shown {
                    set_config("welcome.done", json!(true));
                    self.w_shown = false;
                    self.tell_welcome(h);
                }
            }
            "finish" => {
                if self.w_shown {
                    if s(0) == "glass" || (s(0) == "material" && self.material_available(h)) {
                        self.w_look = s(0);
                    }
                    self.welcome_finish(h);
                }
            }
            // (Settings -> About's "Show the welcome screen": Panels.welcomeRequested)
            "open" | "request" => {
                h.ask("panels.set", json!(["settingsOpen", false])); // (it opens over everything; Settings would wait behind it)
                self.welcome_open(h);
            }
            "resend" => self.tell_welcome(h),
            other => eprintln!("tessera-ui: host: no welcome.{other}"),
        }
    }

    // ── Overlay.qml ─────────────────────────────────────────────────────

    fn overlay_state(&self, h: &Host) -> Value {
        json!({
            "widgets": overlay_all(), "crosshair": crosshair_state(), "floatingImage": ui_get("floatingImage").as_str().unwrap_or(""),
            "imageStamp": self.image_stamp, "recording": h.state("recording.on").as_bool().unwrap_or(false),
            "notes": state_dir().join("notes.md").to_string_lossy()
        })
    }

    fn tell_overlay(&mut self, h: &mut Host, force: bool) {
        let state = self.overlay_state(h);
        let text = state.to_string();
        if force || text != self.sent_overlay {
            self.sent_overlay = text;
            h.tell("overlay", json!({"state": state}));
        }
    }

    fn overlay_call(&mut self, h: &mut Host, what: &str, a: &Value) {
        let gaming = cfg_bool(h.cfg(), "overlays.gaming", false);
        match what {
            "widget" => {
                let id = a[0].as_str().unwrap_or("");
                let allowed = ["notes", "resources", "recorder", "mixer"].contains(&id) || (gaming && ["image", "crosshair", "fps"].contains(&id));
                let p = &a[1];
                if !allowed || !p.is_object() {
                    return;
                }
                let mut patch = serde_json::Map::new();
                for k in ["open", "pinned", "clickthrough"] {
                    if let Some(b) = p[k].as_bool() {
                        patch.insert(k.into(), json!(b));
                    }
                }
                for k in ["x", "y", "w", "h"] {
                    if let Some(n) = p[k].as_f64().filter(|n| n.is_finite()) {
                        patch.insert(k.into(), json!(n.round() as i64));
                    }
                }
                let mut all = overlay_all();
                let mut w = overlay_widget(id);
                for (k, v) in patch {
                    w[k] = v;
                }
                all[id] = w;
                ui_set("overlay", all);
            }
            "crosshair" => {
                let p = &a[0];
                let mut c = crosshair_state();
                for k in ["enabled", "dot"] {
                    if let Some(b) = p[k].as_bool() {
                        c[k] = json!(b);
                    }
                }
                for k in ["size", "thickness", "gap"] {
                    if let Some(n) = p[k].as_f64().filter(|n| n.is_finite()) {
                        c[k] = json!(n.round() as i64);
                    }
                }
                if let Some(col) = p["color"].as_str().filter(|c| c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|x| x.is_ascii_hexdigit())) {
                    c["color"] = json!(col);
                }
                ui_set("crosshair", c);
            }
            "record" => match a[0].as_str().unwrap_or("") {
                "stop" => h.ask("screen.stopRecording", json!([])),
                "screen" => h.ask("screen.recordScreen", json!([])),
                "area" => h.ask("screen.select", json!(["record"])),
                _ => {}
            },
            "pasteImage" => {
                let file = state_dir().join("floating.png");
                let send = h.sender("Bridges");
                let f = file.to_string_lossy().into_owned();
                run_then(vec!["sh".into(), "-c".into(), "wl-paste --type image/png > \"$1\" && echo ok".into(), "sh".into(), f.clone()], move |_, out| {
                    if out.trim() == "ok" {
                        ui_set("floatingImage", json!(f));
                        send(Box::new(Msg::Delayed("overlay.stamp".into(), json!([]))));
                    }
                });
            }
            // MangoHud's fps_limit (the FPS widget's)
            "fps" => {
                let n = a[0].as_i64().unwrap_or(-1);
                if [0, 30, 60, 90, 120].contains(&n) {
                    run_owned(vec!["sh".into(), "-c".into(), "for f in \"$HOME/.config/MangoHud/MangoHud.conf\"; do mkdir -p \"$(dirname \"$f\")\"; touch \"$f\"; if grep -q '^fps_limit=' \"$f\"; then sed -i \"s/^fps_limit=.*/fps_limit=$1/\" \"$f\"; else echo \"fps_limit=$1\" >> \"$f\"; fi; done; pkill -SIGUSR2 -x mangohud 2>/dev/null; true".into(), "sh".into(), n.to_string()]);
                }
            }
            "stamp" => self.image_stamp += 1,
            "resend" => {}
            other => eprintln!("tessera-ui: host: no overlay.{other}"),
        }
        // (the files change under a moment's merge: said again shortly)
        self.later(h, "overlay", 250);
    }

    // ── FilePicker.qml ──────────────────────────────────────────────────

    fn pick(&mut self, h: &mut Host, o: &Value) {
        if self.picking {
            return;
        }
        let home = home().to_string_lossy().into_owned();
        let mut cmd = vec!["python3".to_string(), script("pick-file.py").to_string_lossy().into_owned()];
        let folder = o["folder"].as_bool().unwrap_or(false);
        let multiple = o["multiple"].as_bool().unwrap_or(false) && !folder;
        if folder {
            cmd.push("--folder".into());
        }
        if multiple {
            cmd.push("--multiple".into());
        }
        for (flag, key) in [("--title", "title"), ("--start", "start"), ("--filter", "filter")] {
            if let Some(v) = o[key].as_str().filter(|v| !v.is_empty()) {
                cmd.push(flag.into());
                cmd.push(if key == "start" && v.starts_with('~') { format!("{home}{}", &v[1..]) } else { v.to_string() });
            }
        }
        self.picking = true;
        h.set_state("picker.busy", json!(true));
        // (the tty session's sandboxed folders are dropped for what it starts)
        let mut full = scrub_env();
        full.extend(cmd);
        let (send, o) = (h.sender("Bridges"), o.clone());
        run_then(full, move |code, out| {
            let out = out.trim().to_string();
            let path = if code == 0 && !out.is_empty() {
                if multiple {
                    serde_json::from_str::<Value>(&out).ok().filter(|v| v.as_array().is_some_and(|l| !l.is_empty()))
                } else {
                    Some(json!(out))
                }
            } else {
                None
            };
            send(Box::new(Msg::Picked(o, path)));
        });
    }

    // ── Hot corners ─────────────────────────────────────────────────────

    fn hot_corner(&mut self, h: &mut Host, a: &Value) {
        if self.locked {
            return;
        }
        let action = a[0].as_str().unwrap_or("");
        let screen = a[1].as_str().unwrap_or("");
        match action {
            "overview" | "missionControl" | "notificationCenter" | "launchpad" => h.ask("panels.toggle", json!([action, screen])),
            "desktop" => h.ask("desktop.showDesktop", json!([])),
            // The lock screen turns into StandBy on its own while charging
            "lock" | "standby" => self.set_locked(h, true),
            "quickNote" => h.ask("panels.show", json!(["quickNote", screen])),
            _ => {}
        }
    }

    fn timer(&mut self, h: &mut Host, kind: &'static str) {
        match kind {
            // the lock's content has lifted away: the compositor lets go
            "leave" => {
                self.later(h, "reveal", 150); // (in case the lock doesn't report letting go)
                self.leaving = false;
                self.set_held(h, false);
            }
            "reveal" => {
                if !self.locked && self.desktop_zero {
                    let ms = if cfg_str(h.cfg(), "lock.unlockTransition", "zoom") == "zoom" { 560 } else { 360 };
                    self.lockfx(h, "reveal", ms);
                }
            }
            // once, on a new setup: when the desktop is up and unlocked
            "welcome" => {
                if !cfg_bool(h.cfg(), "welcome.done", false) && !self.locked && !self.w_shown && drawing(h, "welcome") {
                    self.welcome_open(h);
                } else if !cfg_bool(h.cfg(), "welcome.done", false) {
                    self.later(h, "welcome", 5000);
                }
            }
            // the surfaces are up: what they are told at "ready"
            "initial" => self.resend(h),
            "overlay" => self.tell_overlay(h, false),
            "introHold" => {
                if self.intro_shown && !self.intro_playing {
                    self.play_intro(h);
                }
            }
            "introReveal" => {
                let mat = material(h.cfg());
                self.lockfx(h, "reveal", ((if mat { 800 } else { 900 }) * Self::pace()) as i64);
            }
            "introDone" => self.finish_intro(h),
            _ => {}
        }
    }

    /// What the Rust surfaces are told when they come up (RustUi's `started`)
    fn resend(&mut self, h: &mut Host) {
        self.tell_lock(h);
        self.tell_welcome(h);
        self.tell_overlay(h, true);
        let (r, s) = self.sun.unwrap_or_else(|| {
            let (r, s) = tessera_core::wallpaper::sun();
            (r as i64, s as i64)
        });
        h.tell("background", json!({"sunrise": r, "sunset": s}));
        h.tell("wallpaper", json!({"sunrise": r, "sunset": s}));
    }
}

/// The variables the tty session sandboxes, dropped for what the shell starts
fn scrub_env() -> Vec<String> {
    if tessera_core::env::var("TTY").as_deref() != Some("1") {
        return Vec::new();
    }
    let mut v = vec!["env".to_string()];
    for k in ["XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_STATE_HOME", "XDG_CACHE_HOME", "TESSERA_SANDBOX", "TESSERA_ROOT", "TESSERA_TTY", "IRICE_SANDBOX", "IRICE_ROOT", "IRICE_TTY"] {
        v.push("-u".into());
        v.push(k.into());
    }
    v
}

impl Service for Bridges {
    fn name(&self) -> &'static str {
        "Bridges"
    }
    fn calls(&self) -> &'static [&'static str] {
        &["welcome", "overlay", "hotcorners", "lock", "launch", "wallpaper", "picker", "background", "osd", "polkit", "look", "open", "reveal", "show", "menubar", "session"]
    }
    fn ipc_targets(&self) -> &'static [&'static str] {
        &["lock", "loginIntro", "osd", "polkit", "welcome"]
    }
    fn start(&mut self, h: &mut Host) {
        upgrade_config();
        self.w_look = cfg_str(h.cfg(), "appearance.look", "glass").to_string();
        // Where the bundled pictures really are (the Material look runs from a folder of links)
        let send = h.sender("Bridges");
        let shell = shell_dir().to_string_lossy().into_owned();
        run_then(
            vec!["sh".into(), "-c".into(), "s=\"$1\"; r=$(dirname \"$(readlink -f \"$s\")\"); for d in \"$s/assets/wallpapers\" \"$r/shell/assets/wallpapers\"; do [ -f \"$d/tahoe-day.jpg\" ] && { printf 'tahoe\\t%s\\n' \"$(dirname \"$(readlink -f \"$d/tahoe-day.jpg\")\")\"; break; }; done; for d in \"$s/assets/wallpapers\" \"$r/material/assets/wallpapers\"; do [ -f \"$d/material-light.jpg\" ] && { printf 'material\\t%s\\n' \"$(dirname \"$(readlink -f \"$d/material-light.jpg\")\")\"; break; }; done; true".into(), "sh".into(), shell],
            move |_, out| {
                let (mut t, mut m) = (String::new(), String::new());
                for l in out.lines() {
                    match l.split_once('\t') {
                        Some(("tahoe", v)) => t = v.to_string(),
                        Some(("material", v)) => m = v.to_string(),
                        _ => {}
                    }
                }
                send(Box::new(Msg::Dirs(t, m)));
            },
        );
        // Tessera's root helper installed? (the login screen's wallpaper goes through it)
        let send = h.sender("Bridges");
        std::thread::spawn(move || {
            let ok = ["/usr/lib/tessera/tessera-root", "/usr/lib/irice/irice-root"].iter().any(|p| std::path::Path::new(p).exists());
            send(Box::new(Msg::RootHelper(ok)));
        });
        self.rescan_sets(h);
        // The login intro: the first start in this session, the setting on (the
        // session's mark is made by Sounds, which starts after this)
        let sig = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").unwrap_or_else(|_| "none".into());
        let started = runtime_dir().join(format!("tessera-session-started-{sig}")).exists() || runtime_dir().join(format!("irice-session-started-{sig}")).exists();
        if !started && !self.locked && cfg_bool(h.cfg(), "lock.loginIntro", false) && !cfg_bool(h.cfg(), "appearance.reduceMotion", false) {
            self.arm_intro(h);
        }
        self.later(h, "welcome", 2500);
        self.later(h, "initial", 1000);
        h.set_state("locked", json!(false));
        h.set_state("menubar.resources", json!(cfg_bool(h.cfg(), "menubar.resources", false)));
    }
    fn config_changed(&mut self, h: &mut Host) {
        self.rescan_sets(h);
        h.set_state("menubar.resources", json!(cfg_bool(h.cfg(), "menubar.resources", false)));
        self.tell_overlay(h, false);
    }
    fn call(&mut self, h: &mut Host, call: &str, args: &Value) {
        let a = if args.is_array() { args.clone() } else { json!([]) };
        let (prefix, func) = call.split_once('.').unwrap_or((call, ""));
        let s = |i: usize| a.get(i).and_then(Value::as_str).unwrap_or("").to_string();
        match prefix {
            "welcome" => self.welcome_call(h, func, &a),
            "overlay" => self.overlay_call(h, func, &a),
            "hotcorners" => self.hot_corner(h, &a),
            "lock" => match func {
                "set" => self.set_locked(h, a[0].as_bool().unwrap_or(false)),
                "lock" | "" => self.set_locked(h, true),
                "unlock" => self.set_locked(h, false),
                "sessionStarted" => {
                    self.chimed = true;
                    self.maybe_play(h);
                }
                "playIntro" => self.preview_intro(h),
                "ui" => self.lock_event(h, &a[0]),
                "resend" => self.resend(h),
                other => eprintln!("tessera-ui: host: no lock.{other}"),
            },
            // the links a Rust surface opens (https only), and those from outside (web and mail only)
            "launch" => match func {
                "open" => {
                    if s(0).to_ascii_lowercase().starts_with("https://") {
                        launch_open(&s(0));
                    }
                }
                "openLink" => {
                    if link_allowed(&s(0)) {
                        launch_open(s(0).trim());
                    }
                }
                // [[argv], hold]: a command in the chosen terminal (Launch.terminal)
                "terminal" => {
                    let argv: Vec<String> = a[0].as_array().map(|l| l.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
                    if !argv.is_empty() {
                        launch_terminal(&argv.iter().map(String::as_str).collect::<Vec<_>>(), a.get(1).and_then(Value::as_bool).unwrap_or(false));
                    }
                }
                // [[argv]] (Launch.run: a detached command in its own scope)
                "run" => {
                    let argv: Vec<String> = a[0].as_array().map(|l| l.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
                    if !argv.is_empty() {
                        launch_run(&argv.iter().map(String::as_str).collect::<Vec<_>>(), "run");
                    }
                }
                other => eprintln!("tessera-ui: host: no launch.{other}"),
            },
            "wallpaper" => match func {
                "set" => {
                    if s(0).starts_with('/') {
                        wallpaper_set(&s(0));
                    }
                }
                "useSet" => wallpaper_use_set(&a[0]),
                "openFolder" => launch_open(&tessera_core::wallpaper::directory(h.cfg())),
                other => eprintln!("tessera-ui: host: no wallpaper.{other}"),
            },
            "picker" => self.pick(h, &a[0]),
            // the day's sunrise and sunset (the weather service knows them): the time of day's picture
            "background" => {
                if func == "sun" {
                    let (r, ss) = (a[0].as_i64().unwrap_or(390), a[1].as_i64().unwrap_or(1110));
                    if self.sun != Some((r, ss)) {
                        self.sun = Some((r, ss));
                        h.tell("background", json!({"sunrise": r, "sunset": ss}));
                        h.tell("wallpaper", json!({"sunrise": r, "sunset": ss}));
                    }
                } else if func == "resend" {
                    self.resend(h);
                }
            }
            "osd" => {
                if func == "preview" {
                    h.tell("osd", json!({"preview": s(0)}));
                }
            }
            "polkit" => {
                if func == "preview" {
                    h.tell("polkit", json!({"preview": true, "finger": a[0].as_bool().unwrap_or(false)}));
                }
            }
            // Settings' "look" page group: the login screen's wallpaper
            "look" => match func {
                "later" => {
                    let ms = a[0].as_u64().unwrap_or(0);
                    let (c, args) = (s(1), a.get(2).cloned().unwrap_or(json!([])));
                    let send = h.sender("Bridges");
                    slint::Timer::single_shot(std::time::Duration::from_millis(ms), move || send(Box::new(Msg::Delayed(c.clone(), args.clone()))));
                }
                "loginWallpaper" => {
                    // (the dev sandbox must not change the real login screen)
                    if sandboxed() {
                        self.login_status = "Sandbox: would copy this wallpaper to the login screen.".into();
                        h.set_state("look.loginStatus", json!(self.login_status));
                    } else {
                        let pic = h.state("wallpaper.path").as_str().unwrap_or("").to_string();
                        let helper = self.root_helper;
                        let send = h.sender("Bridges");
                        let cmd: Vec<String> = if helper {
                            vec!["sh".into(), "-c".into(), "h=/usr/lib/tessera/tessera-root; [ -x \"$h\" ] || h=/usr/lib/irice/irice-root; exec pkexec \"$h\" \"$@\"".into(), "sh".into(), "wallpaper".into(), pic]
                        } else {
                            vec!["sh".into(), "-c".into(), "d=/usr/share/tessera; [ -d \"$d\" ] || d=/usr/share/irice; exec pkexec cp \"$1\" \"$d/wallpaper.jpg\"".into(), "sh".into(), pic]
                        };
                        run_then(cmd, move |code, _| {
                            send(Box::new(Msg::LoginStatus(if code == 0 { "The login screen now uses this wallpaper.".into() } else if helper { "Didn't change the login screen. Pick a JPEG, PNG or WebP wallpaper and try again.".into() } else { "Didn't change the login screen. Approve the password prompt to try again.".into() })));
                        });
                    }
                }
                other => eprintln!("tessera-ui: host: no look.{other}"),
            },
            "open" | "reveal" => {
                // Files.open / Files.reveal: a path in its default app, or its folder
                if s(0).starts_with('/') {
                    let target = if prefix == "reveal" { std::path::Path::new(&s(0)).parent().map_or(s(0), |p| p.to_string_lossy().into_owned()) } else { s(0) };
                    launch_open(&target);
                }
            }
            "show" => {
                if func.is_empty() && s(0) == "wallpaper" {
                    h.ask("panels.show", json!(["wallpaper"]));
                }
            }
            // the menu bar's pieces that are the state's (resources sampling), and the power menu's choices
            "menubar" | "session" => {}
            other => eprintln!("tessera-ui: host: nothing answers {other}"),
        }
    }
    fn ipc(&mut self, h: &mut Host, target: &str, func: &str, args: &[String]) -> Option<String> {
        let arg = |i: usize| args.get(i).cloned().unwrap_or_default();
        match (target, func) {
            ("lock", "lock") => self.set_locked(h, true),
            // Sandbox only: the sandbox user has no password
            ("lock", "unlock") => {
                if sandboxed() {
                    self.set_locked(h, false);
                }
            }
            // Sandbox only, for films and screenshots (it has no charger, reader or camera)
            ("lock", "demo") => {
                if sandboxed() {
                    h.tell("lock", json!({"demo": arg(0)}));
                }
            }
            ("loginIntro", "play") => self.preview_intro(h),
            ("osd", "preview") => h.tell("osd", json!({"preview": arg(0)})),
            ("polkit", "preview") => h.tell("polkit", json!({"preview": true, "finger": false})),
            ("polkit", "previewFinger") => h.tell("polkit", json!({"preview": true, "finger": true})),
            ("welcome", "open") => self.welcome_open(h),
            (t, f) => eprintln!("tessera-ui: host: no ipc {t} {f}"),
        }
        None
    }
    fn message(&mut self, h: &mut Host, msg: Box<dyn Any + Send>) {
        let Ok(m) = msg.downcast::<Msg>() else { return };
        match *m {
            Msg::Timer(kind, gen) => {
                if self.gens.get(kind).copied() == Some(gen) {
                    self.timer(h, kind);
                }
            }
            Msg::Delayed(call, args) => {
                if call == "overlay.stamp" {
                    self.image_stamp += 1;
                    self.tell_overlay(h, false);
                } else {
                    h.ask(&call, args);
                }
            }
            Msg::Dirs(t, m) => {
                self.tahoe_dir = t;
                self.material_dir = m;
                if self.w_shown {
                    self.tell_welcome(h);
                }
            }
            Msg::Sets(s) => {
                h.set_state("wallpaper.sets", json!(s));
                self.sets = s;
            }
            Msg::RootHelper(ok) => {
                self.root_helper = ok;
                h.set_state("look.rootHelper", json!(ok));
            }
            Msg::LoginStatus(t) => {
                h.set_state("look.loginStatus", json!(t));
                self.login_status = t;
            }
            Msg::Picked(o, path) => {
                self.picking = false;
                h.set_state("picker.busy", json!(false));
                let Some(path) = path else { return };
                match o["reply"].as_str().unwrap_or("") {
                    "settings" => h.tell("settings", json!({"picked": {"id": o["id"], "path": path}})),
                    "wallpaper" => {
                        if let Some(p) = path.as_str() {
                            wallpaper_set(p);
                        }
                    }
                    // Panels.pickShare: the chosen files go to the share sheet
                    "share" => h.ask("panels.share", json!([path, ""])),
                    // a service that asked: its message gets the answer
                    name if !name.is_empty() => {
                        let name: &'static str = Box::leak(name.to_string().into_boxed_str());
                        h.sender(name)(Box::new(json!({"picked": {"id": o["id"], "path": path}})));
                    }
                    _ => {}
                }
            }
        }
    }
}

impl Bridges {
    fn rescan_sets(&mut self, h: &mut Host) {
        let dir = tessera_core::wallpaper::directory(h.cfg());
        if dir == self.sets_for {
            return;
        }
        self.sets_for = dir.clone();
        let send = h.sender("Bridges");
        run_then(vec![script("wallpaper-sets.sh").to_string_lossy().into_owned(), dir], move |_, out| send(Box::new(Msg::Sets(parse_sets(&out)))));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sets_come_newest_first_then_by_name() {
        let text = "pair\tZebra\t/w/Zebra Light.jpg\t/w/Zebra Dark.jpg\ntime\tApple\t/w/Apple\t/w/Apple/1.jpg\npair\tSonoma\t/w/Sonoma Light.jpg\t/w/Sonoma Dark.jpg\npair\tTahoe\t/w/Tahoe Light.jpg\t/w/Tahoe Dark.jpg\n";
        let sets = parse_sets(text);
        let names: Vec<&str> = sets.iter().map(|s| s["name"].as_str().unwrap()).collect();
        assert_eq!(names, vec!["Tahoe", "Sonoma", "Apple", "Zebra"]);
        assert_eq!(sets[0]["light"], "/w/Tahoe Light.jpg");
        assert_eq!(sets[2]["folder"], "/w/Apple");
        assert_eq!(sets[2]["preview"], "/w/Apple/1.jpg");
    }

    #[test]
    fn the_welcome_walls() {
        let w = welcome_walls("/t", "/m", true);
        assert_eq!(w.len(), 5);
        assert_eq!(w[0]["preview"], "/t/tahoe-dawn.jpg");
        assert_eq!(w[4]["preview"], "/m/material-dark.jpg");
        assert_eq!(welcome_walls("/t", "", false).len(), 4);
        // nothing found yet: no pictures to preview
        assert_eq!(welcome_walls("", "", false)[1]["path"], "");
    }

    #[test]
    fn a_page_group_feed_takes_the_other_crates_fields() {
        let m = merged(json!({"a": 1, "b": 2}), Some(&json!({"b": 3, "c": 4})));
        assert_eq!(m, json!({"a": 1, "b": 3, "c": 4}));
        assert_eq!(merged(json!({"a": 1}), None), json!({"a": 1}));
    }
}

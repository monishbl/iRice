//! The bar's right side, as MenuBar.qml lists it, in its order: the extra
//! buttons, recording, the shelf, Caffeine, the apps' tray icons (behind an
//! arrow when collapsed), Now Playing, resources, weather, the keyboard
//! layout, (Material: the clock), battery, Bluetooth, Wi-Fi, sound, Focus,
//! the sidebar, Search, the privacy dots, Control Center, (Glass: the clock).
//! Each entry says what it shows, its hover tip, and the panel it opens (the
//! QML shell's popovers, until they're in Rust too).

use super::settings::Bar;
use crate::icons::glyph;
use crate::services::bluez::Bluetooth;
use crate::services::bridge::ShellState;
use crate::services::mpris::Players;
use crate::services::status::{Battery, Wifi};
use crate::services::tray::Tray;

#[derive(Clone, Debug, Default, PartialEq)]
pub enum Glyph {
    #[default]
    Icon,
    Battery {
        level: f32,
        charging: bool,
        plugged: bool,
        low_power: bool,
        show_percent: bool,
    },
    Wifi {
        on: bool,
        connected: bool,
        strength: f32,
    },
    /// camera, mic, location, screen shared
    Privacy([bool; 4]),
    /// a tray item's picture (its key)
    Tray(String),
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Tone {
    #[default]
    Ink,
    Danger,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Entry {
    /// what a click does (mod.rs: `clicked`)
    pub id: String,
    /// the QML panel it opens ("" none): its highlight while open
    pub panel: String,
    pub icon: &'static str,
    pub fill: bool,
    pub bold: bool,
    pub text: String,
    pub tip: String,
    pub glyph: Glyph,
    pub tone: Tone,
    pub active: bool,
    pub badge: i32,
    /// unread notifications before the clock: bellDot | dot | count | bellCount | off
    pub indicator: String,
    pub unread: i32,
}

pub struct Inputs<'a> {
    pub bar: &'a Bar,
    pub material: bool,
    pub screen: &'a str,
    pub shell: &'a ShellState,
    pub battery: &'a Battery,
    pub wifi: &'a Wifi,
    pub bt: &'a Bluetooth,
    pub players: &'a Players,
    pub tray: &'a Tray,
    pub tray_open: bool,
    /// the tray item whose menu is open here ("" none)
    pub tray_menu: &'a str,
    pub clock: &'a str,
    pub clock_long: &'a str,
    /// ms since the epoch
    pub now_ms: f64,
}

/// "3:07"
pub fn elapsed(secs: i64) -> String {
    let s = secs.max(0);
    format!("{}:{:02}", s / 60, s % 60)
}

/// The Wi-Fi item's tip (MenuBar.qml)
pub fn wifi_tip(w: &Wifi, elsewhere: bool, short: &str) -> String {
    if elsewhere {
        return format!("Wi-Fi · {short}");
    }
    if !w.enabled {
        return "Wi-Fi off".into();
    }
    if w.ssid.is_empty() {
        return "Wi-Fi on · not connected".into();
    }
    let s = w.strength as f32 / 100.0;
    let q = if s > 0.75 {
        "Excellent"
    } else if s > 0.5 {
        "Good"
    } else if s > 0.3 {
        "Fair"
    } else {
        "Weak"
    };
    format!("{} · {q} signal", w.ssid)
}

/// Battery: the level beside it when asked, and always at 20% or less on
/// battery (unless it's already inside)
pub fn battery_text(level: f32, charging: bool, percent: &str) -> String {
    if percent == "outside" || (percent != "inside" && level <= 0.2 && !charging) {
        format!("{}%", (level * 100.0).round() as i32)
    } else {
        String::new()
    }
}

fn hm(secs: i64) -> String {
    let m = (secs as f64 / 60.0).round() as i64;
    if m >= 60 {
        format!("{} h {} min", m / 60, m % 60)
    } else {
        format!("{m} min")
    }
}

/// Without the QML shell's BatteryHealth: the level and what it's doing
pub fn battery_tip(b: &Battery) -> String {
    let pct = b.percent.round() as i32;
    match b.state {
        4 => format!("{pct}% · fully charged"),
        1 if b.time_to_full > 0 => format!("{pct}% · full in {}", hm(b.time_to_full)),
        1 => format!("{pct}% · charging"),
        _ if b.time_to_empty > 0 => format!("{pct}% · {} left", hm(b.time_to_empty)),
        _ => format!("{pct}% · on battery"),
    }
}

/// The util buttons' look and tips (Settings → Menu Bar → extra buttons)
fn util(name: &str, s: &ShellState) -> Option<(&'static str, String)> {
    let profile = s.str("power.profile");
    Some(match name {
        "screenshot" => ("camera", "Screenshot".into()),
        "colorPicker" => ("eyedropper", "Pick a colour".into()),
        "mic" => (if s.bool("audio.micMuted") { "microphone-slash" } else { "microphone" }, if s.bool("audio.micMuted") { "Microphone off" } else { "Microphone on" }.into()),
        "keyboard" => ("keyboard", "On-screen keyboard".into()),
        "darkMode" => ("circle-half", if s.get("dark").as_bool().unwrap_or(true) { "Dark appearance" } else { "Light appearance" }.into()),
        "powerProfile" => (
            match profile {
                "power-saver" => "leaf",
                "performance" => "lightning",
                _ => "scales",
            },
            format!(
                "Energy mode: {}",
                match profile {
                    "power-saver" => "Low Power",
                    "performance" => "Performance",
                    _ => "Balanced",
                }
            ),
        ),
        "record" => ("record", "Record the screen".into()),
        _ => return None,
    })
}

pub fn entries(i: &Inputs) -> Vec<Entry> {
    let s = i.shell;
    let bar = i.bar;
    let open = |panel: &str| s.open(panel, i.screen);
    let mut out: Vec<Entry> = Vec::new();
    let e = |id: &str, panel: &str, icon: &str| Entry { id: id.into(), panel: panel.into(), icon: glyph(icon), active: !panel.is_empty() && open(panel), ..Default::default() };

    // Extra buttons
    for b in &bar.util_buttons {
        // (the colour picker needs hyprpicker, energy modes power-profiles-daemon)
        if (b == "colorPicker" && !s.bool("tools.hyprpicker")) || (b == "powerProfile" && !s.get("power.ppd").as_bool().unwrap_or(true)) {
            continue;
        }
        if let Some((icon, tip)) = util(b, s) {
            let panel = match b.as_str() {
                "screenshot" => "screenTools",
                "keyboard" => "keyboard",
                _ => "",
            };
            out.push(Entry { tip, active: false, ..e(&format!("util:{b}"), panel, icon) });
        }
    }
    // Recording in progress: click to stop
    if s.bool("recording.on") {
        let t = elapsed(((i.now_ms - s.num("recording.started")) / 1000.0).floor() as i64);
        out.push(Entry { fill: true, tone: Tone::Danger, tip: format!("Recording · {t} · click to stop"), text: t, ..e("recording", "", "record") });
    }
    // The shelf: while it holds something, or files are being dragged
    if s.bool("shelf.on") && (s.num("shelf.count") > 0.0 || s.bool("shelf.zone")) {
        let n = s.num("shelf.count") as i32;
        out.push(Entry {
            fill: s.bool("shelf.opened"),
            badge: n,
            tip: if n > 0 { format!("Shelf · {n} {}", if n == 1 { "item" } else { "items" }) } else { "Drop here to keep it on the shelf".into() },
            ..e("shelf", "", "tray")
        });
    }
    // Caffeine on
    if s.bool("caffeine.on") {
        out.push(Entry { fill: true, tip: format!("Keeping awake · {}", s.str("caffeine.status").to_lowercase()), ..e("caffeine", "", "coffee") });
    }
    // The tray (behind its arrow when collapsed)
    let collapses = bar.collapse_tray && !i.tray.items.is_empty();
    if !collapses || i.tray_open {
        for t in &i.tray.items {
            out.push(Entry { id: format!("tray:{}", t.key), glyph: Glyph::Tray(t.key.clone()), tip: t.label().to_string(), active: i.tray_menu == t.key, ..Default::default() });
        }
    }
    if collapses {
        let n = i.tray.items.len();
        out.push(Entry {
            bold: true,
            tip: if i.tray_open { "Hide app icons".into() } else { format!("Show app icons · {n}") },
            ..e("trayArrow", "", if i.tray_open { "caret-right" } else { "caret-left" })
        });
    }
    // Now Playing (the island shows music itself when it's on)
    if !bar.island {
        if let Some(p) = i.players.chosen().filter(|p| !p.track.title.is_empty()) {
            let t = &p.track.title;
            let text = if t.chars().count() > 28 { t.chars().take(27).collect::<String>() + "…" } else { t.clone() };
            out.push(Entry {
                fill: true,
                text,
                tip: if p.track.artist.is_empty() { t.clone() } else { format!("{t} · {}", p.track.artist) },
                ..e("nowPlaying", "nowPlaying", if p.playing { "waveform" } else { "music-note" })
            });
        }
    }
    // Resources
    if bar.resources {
        let (cpu, mem) = (s.num("resources.cpu"), s.num("resources.memory"));
        out.push(Entry {
            tone: if cpu > 0.9 || mem > 0.95 { Tone::Danger } else { Tone::Ink },
            text: format!("{}% · {}%", (cpu * 100.0).round(), (mem * 100.0).round()),
            tip: "CPU · memory".into(),
            ..e("resources", "resourcesMenu", "cpu")
        });
    }
    // Weather
    if bar.weather && s.bool("weather.ready") {
        let w = |k: &str| s.get(&format!("weather.{k}")).to_string().trim_matches('"').to_string();
        out.push(Entry {
            fill: true,
            text: format!("{}°", w("temp")),
            tip: format!("{} · H {}° L {}°{}", w("condition"), w("high"), w("low"), if s.str("weather.place").is_empty() { String::new() } else { format!(" · {}", s.str("weather.place")) }),
            ..e("weather", "weatherMenu", s.str("weather.icon"))
        });
    }
    // Keyboard layout (two or more)
    if bar.keyboard_layout && s.bool("keyboard.multiple") {
        let short = s.str("keyboard.short");
        let name = if s.str("keyboard.name").is_empty() { short } else { s.str("keyboard.name") };
        out.push(Entry { text: short.into(), tip: format!("{name} · click to switch"), ..e("keyboard", "", "") });
    }
    let clock = |out: &mut Vec<Entry>| {
        if !bar.clock {
            return;
        }
        let unread = s.num("unread") as i32;
        out.push(Entry {
            text: i.clock.into(),
            indicator: if s.bool("focus.on") { "off".into() } else { bar.notification_indicator.clone() },
            unread,
            tip: i.clock_long.split(',').next().unwrap_or("").to_string() + &if unread > 0 { format!(" · {unread} {}", if unread == 1 { "notification" } else { "notifications" }) } else { String::new() },
            ..e("clock", "notificationCenter", "")
        });
    };
    // Material: the clock leads the status icons, as on Android
    if i.material {
        clock(&mut out);
    }
    // Battery
    if bar.battery && i.battery.present {
        let b = i.battery;
        let level = (b.percent / 100.0) as f32;
        out.push(Entry {
            text: battery_text(level, b.charging, &bar.battery_percent),
            glyph: Glyph::Battery { level, charging: b.state == 1, plugged: b.state == 4 || b.state == 5, low_power: s.bool("battery.lowPower"), show_percent: bar.battery_percent == "inside" },
            tip: if s.str("battery.summary").is_empty() { battery_tip(b) } else { s.str("battery.summary").into() },
            ..e("battery", "batteryMenu", "")
        });
    }
    // Bluetooth
    if bar.bluetooth && i.bt.present {
        out.push(Entry { bold: true, tip: i.bt.summary(), ..e("bluetooth", "bluetoothMenu", i.bt.icon()) });
    }
    // Wi-Fi
    let elsewhere = s.bool("wifi.elsewhere");
    if bar.wifi && (i.wifi.present || elsewhere) {
        out.push(Entry {
            glyph: Glyph::Wifi {
                on: i.wifi.enabled || elsewhere,
                connected: !i.wifi.ssid.is_empty() || (elsewhere && s.bool("wifi.linkUp")),
                strength: if elsewhere { 1.0 } else { i.wifi.strength as f32 / 100.0 },
            },
            tip: wifi_tip(i.wifi, elsewhere, s.str("wifi.short")),
            ..e("wifi", "wifiMenu", "")
        });
    }
    // Sound
    if bar.sound {
        let vol = if s.bool("audio.muted") { "Muted".to_string() } else { format!("{}%", (s.num("audio.volume") * 100.0).round()) };
        let sink = if s.str("audio.sink").is_empty() { "Sound" } else { s.str("audio.sink") };
        out.push(Entry { fill: true, tip: format!("{vol} · {sink}"), ..e("sound", "soundMenu", if s.str("audio.icon").is_empty() { "speaker-high" } else { s.str("audio.icon") }) });
    }
    // Focus
    if s.bool("focus.on") {
        out.push(Entry { fill: true, tip: format!("{} · {}", s.str("focus.name"), s.str("focus.status")), ..e("focus", "", if s.str("focus.icon").is_empty() { "moon" } else { s.str("focus.icon") }) });
    }
    if bar.ai {
        let on = open("sidebar");
        out.push(Entry { fill: on, tip: "AI assistant".into(), ..e("ai", "sidebar", "sparkle") });
    }
    if bar.search {
        out.push(Entry { bold: true, tip: "Search".into(), ..e("search", "spotlight", "magnifying-glass") });
    }
    if s.bool("privacy.any") {
        out.push(Entry { glyph: Glyph::Privacy([s.bool("privacy.camera"), s.bool("privacy.mic"), s.bool("privacy.location"), s.bool("privacy.share")]), tip: "In use".into(), ..e("privacy", "privacyMenu", "") });
    }
    if bar.control_center {
        out.push(Entry { bold: true, tip: "Quick Settings".into(), ..e("controlCenter", "controlCenter", "sliders-horizontal") });
    }
    // Glass: the clock ends the bar, as on macOS
    if !i.material {
        clock(&mut out);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::tray::{Icon, TrayItem};
    use serde_json::json;

    fn ids(v: &[Entry]) -> Vec<&str> {
        v.iter().map(|e| e.id.as_str()).collect()
    }

    struct World {
        bar: Bar,
        shell: ShellState,
        battery: Battery,
        wifi: Wifi,
        bt: Bluetooth,
        players: Players,
        tray: Tray,
    }

    fn world() -> World {
        World {
            bar: super::super::settings::bar(&json!({})),
            shell: ShellState(json!({"open": "", "screen": ""})),
            battery: Battery { present: true, percent: 64.0, charging: false, state: 2, time_to_empty: 5400, time_to_full: 0 },
            wifi: Wifi { present: true, enabled: true, ssid: "Home".into(), strength: 80 },
            bt: Bluetooth::default(),
            players: Players::default(),
            tray: Tray::default(),
        }
    }

    fn run(w: &World, material: bool, tray_open: bool) -> Vec<Entry> {
        entries(&Inputs { bar: &w.bar, material, screen: "DP-1", shell: &w.shell, battery: &w.battery, wifi: &w.wifi, bt: &w.bt, players: &w.players, tray: &w.tray, tray_open, tray_menu: "", clock: "Wed 4 Nov  7:26 PM", clock_long: "Wednesday 4 November 2026, 7:26 PM", now_ms: 0.0 })
    }

    #[test]
    fn the_default_bar_in_each_look() {
        let w = world();
        // (the island is on by default: no Now Playing; weather waits for its reading)
        assert_eq!(ids(&run(&w, false, false)), vec!["util:screenshot", "battery", "wifi", "ai", "search", "controlCenter", "clock"]);
        assert_eq!(ids(&run(&w, true, false)), vec!["util:screenshot", "clock", "battery", "wifi", "ai", "search", "controlCenter"]);
    }

    #[test]
    fn the_tray_waits_behind_its_arrow() {
        let mut w = world();
        let item = |k: &str| TrayItem { key: k.into(), id: k.into(), title: "".into(), tooltip: "".into(), status: "Active".into(), icon: Icon::None, menu: "".into(), item_is_menu: false };
        w.tray.items = vec![item("a"), item("b")];
        let v = run(&w, false, false);
        assert!(ids(&v).contains(&"trayArrow") && !ids(&v).contains(&"tray:a"));
        assert_eq!(v.iter().find(|e| e.id == "trayArrow").unwrap().tip, "Show app icons · 2");
        let v = run(&w, false, true);
        let i = ids(&v);
        assert!(i.iter().position(|x| *x == "tray:a").unwrap() < i.iter().position(|x| *x == "trayArrow").unwrap());
        w.bar.collapse_tray = false;
        assert!(!ids(&run(&w, false, false)).contains(&"trayArrow"));
    }

    #[test]
    fn what_the_shell_says_shows() {
        let mut w = world();
        w.shell = ShellState(json!({
            "open": "wifiMenu", "screen": "DP-1",
            "focus": {"on": true, "icon": "briefcase", "name": "Work", "status": "On"},
            "caffeine": {"on": true, "status": "Until off"},
            "recording": {"on": true, "started": -125000.0},
            "unread": 3,
            "privacy": {"any": true, "mic": true},
            "weather": {"ready": true, "icon": "sun", "temp": 21, "condition": "Clear", "high": 24, "low": 12, "place": "Lisbon"},
        }));
        let v = run(&w, false, false);
        assert_eq!(ids(&v), vec!["util:screenshot", "recording", "caffeine", "weather", "battery", "wifi", "focus", "ai", "search", "privacy", "controlCenter", "clock"]);
        let get = |id: &str| v.iter().find(|e| e.id == id).unwrap().clone();
        assert_eq!(get("recording").text, "2:05");
        assert_eq!(get("recording").tone, Tone::Danger);
        assert!(get("wifi").active && !get("battery").active);
        assert_eq!(get("caffeine").tip, "Keeping awake · until off");
        assert_eq!(get("weather").text, "21°");
        assert_eq!(get("weather").tip, "Clear · H 24° L 12° · Lisbon");
        assert_eq!(get("focus").tip, "Work · On");
        assert_eq!(get("focus").icon, glyph("briefcase"));
        // Focus on: no unread mark on the clock
        assert_eq!((get("clock").indicator.as_str(), get("clock").unread), ("off", 3));
        assert_eq!(get("clock").tip, "Wednesday 4 November 2026 · 3 notifications");
        assert_eq!(get("privacy").glyph, Glyph::Privacy([false, true, false, false]));
    }

    #[test]
    fn battery_and_wifi_say_it_as_the_qml_bar() {
        assert_eq!(battery_text(0.64, false, "off"), "");
        assert_eq!(battery_text(0.64, false, "outside"), "64%");
        assert_eq!(battery_text(0.18, false, "off"), "18%");
        assert_eq!(battery_text(0.18, true, "off"), "");
        assert_eq!(battery_text(0.18, false, "inside"), "");
        let b = Battery { present: true, percent: 64.0, charging: false, state: 2, time_to_empty: 5400, time_to_full: 0 };
        assert_eq!(battery_tip(&b), "64% · 1 h 30 min left");
        assert_eq!(battery_tip(&Battery { state: 4, percent: 100.0, ..b.clone() }), "100% · fully charged");
        let w = Wifi { present: true, enabled: true, ssid: "Home".into(), strength: 45 };
        assert_eq!(wifi_tip(&w, false, ""), "Home · Fair signal");
        assert_eq!(wifi_tip(&Wifi { enabled: false, ..w.clone() }, false, ""), "Wi-Fi off");
        assert_eq!(wifi_tip(&Wifi { ssid: "".into(), ..w.clone() }, false, ""), "Wi-Fi on · not connected");
        assert_eq!(wifi_tip(&w, true, "Managed by iwd"), "Wi-Fi · Managed by iwd");
        // the battery's glyph: plugged when full or holding
        let mut wd = world();
        wd.battery.state = 4;
        wd.bar.battery_percent = "inside".into();
        let v = run(&wd, false, false);
        assert_eq!(v.iter().find(|e| e.id == "battery").unwrap().glyph, Glyph::Battery { level: 0.64, charging: false, plugged: true, low_power: false, show_percent: true });
    }

    #[test]
    fn now_playing_only_without_the_island() {
        let mut w = world();
        w.players.list.push(crate::services::mpris::Player { bus: "x".into(), playing: true, track: crate::services::mpris::Track { title: "A very long song title that goes on and on".into(), artist: "Band".into(), ..Default::default() }, ..Default::default() });
        assert!(!ids(&run(&w, false, false)).contains(&"nowPlaying"));
        w.bar.island = false;
        let v = run(&w, false, false);
        let np = v.iter().find(|e| e.id == "nowPlaying").unwrap();
        assert_eq!(np.text.chars().count(), 28);
        assert!(np.text.ends_with('…'));
        assert_eq!(np.icon, glyph("waveform"));
    }

    #[test]
    fn extra_buttons_need_their_tools() {
        let mut w = world();
        w.bar.util_buttons = vec!["colorPicker".into(), "powerProfile".into(), "mic".into()];
        w.shell = ShellState(json!({"power": {"ppd": false}, "tools": {"hyprpicker": false}}));
        assert_eq!(ids(&run(&w, false, false))[0], "util:mic");
        w.shell = ShellState(json!({"power": {"ppd": true, "profile": "power-saver"}, "tools": {"hyprpicker": true}}));
        let v = run(&w, false, false);
        assert_eq!(ids(&v)[..3], ["util:colorPicker", "util:powerProfile", "util:mic"]);
        assert_eq!(v[1].tip, "Energy mode: Low Power");
        assert_eq!(v[1].icon, glyph("leaf"));
    }
}

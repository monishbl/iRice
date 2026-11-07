//! What the status menus show, worked out from the readings (no Slint here,
//! so it's tested on its own): the menus' sizes, Bluetooth's page as the
//! Control Center's blocks, the battery's lines, the privacy rows, the
//! resource bars, the weather's hours and Now Playing's times.

use serde_json::Value;
use std::collections::HashSet;
use tessera_controlcenter::bridge::{Block, BlockKind, MenuItem, Page};
use tessera_core::services::bluez::{Bluetooth, Device};
use tessera_core::services::status::Battery;

/// The menus this surface draws: (Panels name, content width, how far left
/// of the item it starts). StatusMenus.qml's `width_` and `targetX:
/// Panels.anchorX - 20`; NowPlaying.qml's `contentWidth: 320` and
/// `Panels.anchorX - 120`.
pub const PANELS: &[(&str, f32, f32)] = &[
    ("bluetoothMenu", 300.0, 20.0),
    ("soundMenu", 300.0, 20.0),
    ("privacyMenu", 260.0, 20.0),
    ("batteryMenu", 280.0, 20.0),
    ("resourcesMenu", 280.0, 20.0),
    ("weatherMenu", 300.0, 20.0),
    ("nowPlaying", 320.0, 120.0),
];

pub fn panel(name: &str) -> Option<(&'static str, f32, f32)> {
    PANELS.iter().find(|p| p.0 == name).copied()
}

fn s(v: &Value, k: &str) -> String {
    v[k].as_str().unwrap_or("").to_string()
}

// ── Bluetooth (the Control Center's Bluetooth page, cc.rs / RustBridge.qml's btPage) ──

/// btIcon(d): BlueZ's icon name → a Phosphor one
pub fn bt_icon(icon: &str) -> &'static str {
    if icon.contains("headset") || icon.contains("headphone") {
        "headphones"
    } else if icon.contains("phone") {
        "device-mobile"
    } else if icon.contains("keyboard") {
        "keyboard"
    } else if icon.contains("mouse") {
        "mouse"
    } else if icon.contains("audio") {
        "speaker-hifi"
    } else {
        "bluetooth"
    }
}

/// The devices worth listing: paired or named, connected first, then paired,
/// then by name; one row per name for the unpaired; at most 40
pub fn bt_devices(bt: &Bluetooth) -> Vec<Device> {
    let mut devices: Vec<Device> = bt.devices.iter().filter(|d| d.paired || !d.name.is_empty()).cloned().collect();
    devices.sort_by(|a, b| b.connected.cmp(&a.connected).then(b.paired.cmp(&a.paired)).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    let mut seen = HashSet::new();
    devices.retain(|d| d.paired || d.name.is_empty() || seen.insert(d.name.clone()));
    devices.truncate(40);
    devices
}

/// What a device's menu item does
#[derive(Clone, Debug, PartialEq)]
pub enum BtAct {
    /// bluetoothctl <verb> <address>
    Ctl(&'static str),
    Copy,
    Settings,
}

/// A device's right-click menu (cc.rs bt_menu): None is a separator
pub fn bt_menu(d: &Device, busy: bool) -> Vec<Option<(&'static str, BtAct, bool)>> {
    let mut m = vec![];
    if d.connected {
        m.push(Some(("Disconnect", BtAct::Ctl("disconnect"), false)));
    } else if d.paired {
        m.push(Some(("Connect", BtAct::Ctl("connect"), false)));
    }
    if busy && !d.paired {
        m.push(Some(("Stop pairing", BtAct::Ctl("cancel-pairing"), false)));
    } else if !d.paired {
        m.push(Some(("Pair", BtAct::Ctl("pair"), false)));
    }
    if d.paired {
        m.push(Some(("Connect automatically", BtAct::Ctl("trust"), false)));
        m.push(Some(("Don't connect automatically", BtAct::Ctl("untrust"), false)));
    }
    m.push(None);
    m.push(Some(("Copy address", BtAct::Copy, false)));
    if d.paired {
        m.push(Some(("Forget device", BtAct::Ctl("remove"), true)));
    }
    m.push(None);
    m.push(Some(("Bluetooth settings…", BtAct::Settings, false)));
    m
}

pub fn menu_items(m: &[Option<(&'static str, BtAct, bool)>]) -> Vec<MenuItem> {
    m.iter()
        .map(|e| match e {
            Some((t, _, danger)) => MenuItem { text: (*t).into(), danger: *danger, ..Default::default() },
            None => MenuItem { separator: true, ..Default::default() },
        })
        .collect()
}

/// A click on a device's row: disconnect, connect or pair
pub fn bt_click(d: &Device) -> &'static str {
    if d.connected {
        "disconnect"
    } else if d.paired {
        "connect"
    } else {
        "pair"
    }
}

/// The Bluetooth page (BluetoothPage.qml with showBack: false): the notice
/// when BlueZ can't be used (`backends`: the shell's Backends state), why
/// it's empty, the devices (`busy`: pairing; `pictures`: address → file)
pub fn bt_page(bt: &Bluetooth, backends: &Value, busy: &HashSet<String>, pictures: &Value) -> Page {
    let state = s(backends, "btState");
    let mut blocks = vec![];
    let note = |t: &str| Block { kind: BlockKind::Note, title: t.into(), center: true, ..Default::default() };
    if !state.is_empty() && state != "ok" {
        blocks.push(Block {
            kind: BlockKind::Notice,
            id: "fix".into(),
            icon: "bluetooth-slash".into(),
            title: s(backends, "btTitle"),
            status: s(backends, "btReason"),
            action: s(backends, "btFixLabel"),
            busy: backends["btFixing"].as_bool().unwrap_or(false),
            ..Default::default()
        });
    } else {
        let devices = bt_devices(bt);
        if !bt.present {
            blocks.push(note("No Bluetooth adapter. If this computer has one, check that it isn't switched off in its settings or with a key."));
        } else if !bt.powered {
            blocks.push(note("Bluetooth is off"));
        } else if devices.is_empty() {
            blocks.push(note("Looking for devices…"));
        }
        if bt.powered {
            for d in devices {
                let pairing = busy.contains(&d.address);
                let status = if d.connected {
                    d.battery.map_or_else(|| "Connected".to_string(), |p| format!("Connected · {p}%"))
                } else if pairing {
                    "Pairing…".into()
                } else if d.paired {
                    "Not connected".into()
                } else {
                    String::new()
                };
                blocks.push(Block {
                    kind: BlockKind::Row,
                    id: format!("d:{}", d.address),
                    icon: bt_icon(&d.icon).into(),
                    picture: pictures[&d.address].as_str().unwrap_or("").trim_start_matches("file://").to_string(),
                    title: if d.name.is_empty() { d.address.clone() } else { d.name.clone() },
                    status,
                    highlighted: d.connected,
                    busy: pairing,
                    ..Default::default()
                });
            }
        }
    }
    let usable = state == "ok" || (state.is_empty() && bt.present);
    Page { name: "bluetooth".into(), title: "Bluetooth".into(), switch: usable, switch_on: bt.powered, switch_label: "Use Bluetooth".into(), refresh: usable, blocks }
}

// ── Battery ──────────────────────────────────────────────────────────────

/// The battery menu's hm(): "1 h 5 min", "42 min"
pub fn hm(secs: i64) -> String {
    let m = (secs as f64 / 60.0).round() as i64;
    if m >= 60 {
        format!("{} h {} min", m / 60, m % 60)
    } else {
        format!("{m} min")
    }
}

/// 0..1
pub fn level(b: &Battery) -> f64 {
    (if b.percent > 1.0 { b.percent / 100.0 } else { b.percent }).clamp(0.0, 1.0)
}

/// UPower's states: 1 charging, 4 fully charged
pub fn charging(b: &Battery) -> bool {
    b.state == 1
}
pub fn full(b: &Battery) -> bool {
    b.state == 4
}

/// The line under the level
pub fn battery_status(b: &Battery) -> String {
    if !b.present {
        "No battery. This computer runs on mains power.".into()
    } else if full(b) {
        "Fully charged".into()
    } else if charging(b) {
        if b.time_to_full > 0 { format!("Charging · full in {}", hm(b.time_to_full)) } else { "Charging".into() }
    } else if b.time_to_empty > 0 {
        format!("On battery · {} left", hm(b.time_to_empty))
    } else {
        "On battery".into()
    }
}

/// Power going in or out ("" when there's nothing to say)
pub fn watts_line(b: &Battery, health: &Value) -> String {
    let w = health["watts"].as_f64().unwrap_or(-1.0);
    if !b.present || w <= 0.05 || full(b) {
        return String::new();
    }
    format!("{}{:.1} W", if charging(b) { "Charging at " } else { "Using " }, w)
}

/// Battery health's rows (label, value), only those known (BatteryHealth's
/// kernel readings; UPower's health percentage isn't read here)
pub fn health_rows(health: &Value) -> Vec<(String, String)> {
    let cap = health["capacity"].as_f64().unwrap_or(-1.0);
    let cycles = health["cycles"].as_i64().unwrap_or(-1);
    let cond = health["condition"].as_str().unwrap_or("");
    let mut out = vec![];
    if cap >= 0.0 {
        out.push(("Maximum capacity".to_string(), format!("{}%", (cap * 100.0).round())));
    }
    if cycles > 0 {
        out.push(("Cycle count".to_string(), cycles.to_string()));
    }
    if !cond.is_empty() {
        out.push(("Condition".to_string(), cond.to_string()));
    }
    out
}

/// The energy modes the segmented control offers, and which is on
pub fn energy_modes(performance: bool, profile: &str) -> (Vec<&'static str>, i32) {
    let opts = if performance { vec!["Low Power", "Balanced", "Performance"] } else { vec!["Low Power", "Balanced"] };
    let cur = match profile {
        "power-saver" => 0,
        "performance" if performance => 2,
        "balanced" => 1,
        _ => 0,
    };
    (opts, cur)
}

pub const MODES: [&str; 3] = ["power-saver", "balanced", "performance"];

// ── Privacy ──────────────────────────────────────────────────────────────

/// One app using something: app, what, its Phosphor icon, its colour (#rrggbb)
#[derive(Clone, Debug, PartialEq)]
pub struct InUse {
    pub app: String,
    pub what: String,
    pub icon: &'static str,
    pub color: u32,
}

/// The privacy menu's rows: cameras, microphones, location, shared screens
pub fn in_use(p: &Value) -> Vec<InUse> {
    let apps = |k: &str| p[k].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect::<Vec<_>>()).unwrap_or_default();
    let mut out = vec![];
    for a in apps("cameraApps") {
        out.push(InUse { app: a, what: "Camera".into(), icon: "video-camera", color: 0x30D158 });
    }
    for a in apps("micApps") {
        out.push(InUse { app: a, what: "Microphone".into(), icon: "microphone", color: 0xFF9F0A });
    }
    for a in apps("locationApps") {
        out.push(InUse { app: a, what: "Location".into(), icon: "navigation-arrow", color: 0x0A84FF });
    }
    for x in p["shares"].as_array().into_iter().flatten() {
        let window = x["window"].as_bool().unwrap_or(false);
        out.push(InUse { app: s(x, "app"), what: format!("Sharing {}", s(x, "target")), icon: if window { "app-window" } else { "screencast" }, color: 0xBF5AF2 });
    }
    out
}

/// "<app> is still sharing. Stop it in <app>." ("" unless stopping failed)
pub fn stop_failed(p: &Value) -> String {
    if !p["stopFailed"].as_bool().unwrap_or(false) {
        return String::new();
    }
    let app = s(p, "screenApp");
    format!("{app} is still sharing. Stop it in {app}.")
}

// ── Resources ────────────────────────────────────────────────────────────

/// CPU, Memory, Swap: (label, 0..1, what it says)
pub fn resource_rows(r: &Value) -> Vec<(&'static str, f64, String)> {
    let f = |k: &str| r[k].as_f64().unwrap_or(0.0);
    let pct = |v: f64| format!("{}%", (v * 100.0).round());
    let temp = f("temperature");
    vec![
        ("CPU", f("cpu"), format!("{}{}", pct(f("cpu")), if temp > 0.0 { format!(" · {}°C", temp.round()) } else { String::new() })),
        ("Memory", f("memory"), format!("{:.1} of {:.1} GB", f("memUsedGb"), f("memTotalGb"))),
        ("Swap", f("swap"), pct(f("swap"))),
    ]
}

// ── Weather ──────────────────────────────────────────────────────────────

/// TimeFormat.hOnly: "9 AM", or "09"
pub fn hour(h: u32, h24: bool) -> String {
    if h24 {
        format!("{h:02}")
    } else {
        format!("{} {}", (h + 11) % 12 + 1, if h < 12 { "AM" } else { "PM" })
    }
}

/// Before the first reading: loading, or why there's none
pub fn weather_note(ready: bool, error: &str) -> String {
    if ready {
        String::new()
    } else if error.is_empty() {
        "Getting the weather…".into()
    } else {
        format!("{error}. It's tried again in a few minutes.")
    }
}

// ── Now Playing ──────────────────────────────────────────────────────────

/// NowPlaying.qml's fmt(): "3:07"
pub fn mmss(secs: f64) -> String {
    let s = secs.max(0.0).floor() as i64;
    format!("{}:{:02}", s / 60, s % 60)
}

/// Artist — album (what's known)
pub fn byline(artist: &str, album: &str) -> String {
    [artist, album].iter().filter(|x| !x.is_empty()).copied().collect::<Vec<_>>().join(" — ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn dev(name: &str, addr: &str, paired: bool, connected: bool) -> Device {
        Device { name: name.into(), address: addr.into(), paired, connected, icon: "audio-headset".into(), ..Default::default() }
    }

    #[test]
    fn menus_sizes_and_places() {
        assert_eq!(panel("privacyMenu"), Some(("privacyMenu", 260.0, 20.0)));
        assert_eq!(panel("nowPlaying").map(|p| (p.1, p.2)), Some((320.0, 120.0)));
        assert_eq!(panel("wifiMenu"), None);
        assert_eq!(PANELS.len(), 7);
    }

    #[test]
    fn bluetooth_in_every_state() {
        let none = HashSet::new();
        // no adapter, off, on and empty
        let p = bt_page(&Bluetooth::default(), &json!(null), &none, &json!(null));
        assert!(!p.switch && p.blocks.len() == 1 && p.blocks[0].title.starts_with("No Bluetooth adapter"));
        let off = Bluetooth { present: true, powered: false, devices: vec![dev("Buds", "A", true, false)] };
        let p = bt_page(&off, &json!({}), &none, &json!({}));
        assert!(p.switch && !p.switch_on);
        assert_eq!(p.blocks.iter().map(|b| b.title.as_str()).collect::<Vec<_>>(), vec!["Bluetooth is off"]);
        let empty = Bluetooth { present: true, powered: true, devices: vec![] };
        assert_eq!(bt_page(&empty, &json!({}), &none, &json!({})).blocks[0].title, "Looking for devices…");
        // the devices: connected first, with its battery; pairing; unnamed strangers left out
        let mut d1 = dev("Headphones", "B", true, true);
        d1.battery = Some(80);
        let on = Bluetooth { present: true, powered: true, devices: vec![dev("Buds", "A", true, false), d1, dev("", "C", false, false), dev("Speaker", "D", false, false)] };
        let busy: HashSet<String> = ["D".to_string()].into();
        let p = bt_page(&on, &json!({"btState": "ok"}), &busy, &json!({"B": "file:///x/headphones.svg"}));
        let rows: Vec<(&str, &str)> = p.blocks.iter().map(|b| (b.title.as_str(), b.status.as_str())).collect();
        assert_eq!(rows, vec![("Headphones", "Connected · 80%"), ("Buds", "Not connected"), ("Speaker", "Pairing…")]);
        assert!(p.blocks[0].highlighted && p.blocks[2].busy);
        assert_eq!((p.blocks[0].icon.as_str(), p.blocks[0].picture.as_str()), ("headphones", "/x/headphones.svg"));
        // BlueZ not usable: the notice and its fix
        let p = bt_page(&on, &json!({"btState": "off", "btTitle": "Bluetooth is blocked", "btReason": "rfkill", "btFixLabel": "Unblock"}), &none, &json!({}));
        assert!(!p.switch && p.blocks.len() == 1 && p.blocks[0].kind == BlockKind::Notice && p.blocks[0].action == "Unblock");
    }

    #[test]
    fn bluetooth_menus_follow_the_device() {
        let m = bt_menu(&dev("X", "A", true, true), false);
        assert_eq!(m[0].as_ref().unwrap().0, "Disconnect");
        assert!(m.iter().flatten().any(|e| e.0 == "Forget device" && e.2));
        let m = bt_menu(&dev("X", "A", false, false), true);
        assert_eq!(m[0].as_ref().unwrap().1, BtAct::Ctl("cancel-pairing"));
        assert!(!m.iter().flatten().any(|e| e.0 == "Forget device"));
        assert_eq!(menu_items(&m).iter().filter(|i| i.separator).count(), 2);
        assert_eq!((bt_click(&dev("X", "A", true, true)), bt_click(&dev("X", "A", true, false)), bt_click(&dev("X", "A", false, false))), ("disconnect", "connect", "pair"));
    }

    #[test]
    fn battery_lines() {
        let b = Battery { present: true, percent: 84.0, charging: false, state: 2, time_to_empty: 12000, time_to_full: 0 };
        assert_eq!(battery_status(&b), "On battery · 3 h 20 min left");
        assert_eq!(battery_status(&Battery { state: 1, time_to_full: 1500, ..b.clone() }), "Charging · full in 25 min");
        assert_eq!(battery_status(&Battery { state: 4, ..b.clone() }), "Fully charged");
        assert!(battery_status(&Battery::default()).starts_with("No battery"));
        assert_eq!(level(&b), 0.84);
        assert_eq!(watts_line(&b, &json!({"watts": 7.46})), "Using 7.5 W");
        assert_eq!(watts_line(&Battery { state: 4, ..b.clone() }, &json!({"watts": 7.46})), "");
        assert_eq!(watts_line(&b, &json!({"watts": -1})), "");
        assert_eq!(health_rows(&json!({"capacity": 0.784, "cycles": 212, "condition": "Service recommended"})).len(), 3);
        assert_eq!(health_rows(&json!({"capacity": -1, "cycles": -1, "condition": ""})), vec![]);
        assert_eq!(energy_modes(true, "performance"), (vec!["Low Power", "Balanced", "Performance"], 2));
        assert_eq!(energy_modes(false, "balanced"), (vec!["Low Power", "Balanced"], 1));
    }

    #[test]
    fn privacy_rows() {
        let p = json!({"cameraApps": ["Zoom"], "micApps": ["Zoom", "Discord"], "locationApps": [], "shares": [{"app": "OBS", "target": "DP-1", "window": false}], "screenApp": "OBS", "stopFailed": true});
        let rows = in_use(&p);
        assert_eq!(rows.iter().map(|r| r.what.as_str()).collect::<Vec<_>>(), vec!["Camera", "Microphone", "Microphone", "Sharing DP-1"]);
        assert_eq!((rows[3].icon, rows[3].color), ("screencast", 0xBF5AF2));
        assert_eq!(stop_failed(&p), "OBS is still sharing. Stop it in OBS.");
        assert!(in_use(&json!(null)).is_empty() && stop_failed(&json!({})).is_empty());
    }

    #[test]
    fn resources_weather_and_time() {
        let r = resource_rows(&json!({"cpu": 0.42, "memory": 0.5, "swap": 0.0, "memUsedGb": 7.84, "memTotalGb": 15.6, "temperature": 54.6}));
        assert_eq!(r[0].2, "42% · 55°C");
        assert_eq!(r[1].2, "7.8 of 15.6 GB");
        assert_eq!(resource_rows(&json!(null))[0].2, "0%");
        assert_eq!((hour(0, false), hour(13, false), hour(9, true)), ("12 AM".to_string(), "1 PM".to_string(), "09".to_string()));
        assert_eq!(weather_note(true, ""), "");
        assert_eq!(weather_note(false, ""), "Getting the weather…");
        assert!(weather_note(false, "Offline").starts_with("Offline."));
        assert_eq!((mmss(187.9), mmss(-3.0)), ("3:07".to_string(), "0:00".to_string()));
        assert_eq!(byline("A", ""), "A");
        assert_eq!(byline("A", "B"), "A — B");
    }
}

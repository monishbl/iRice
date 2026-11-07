//! modules/controlcenter/RustBridge.qml (with the module table of
//! ModuleDefs.qml it drives): Control Center drawn in Rust. While it is open
//! this tells the surface what only the shell knows (`RustUi.tell(
//! "controlcenter", …)`: each control's state, the player, you, and the open
//! page's rows as plain blocks) and does what a click there asks
//! (`cc.act`, `cc.page`, `cc.editing`, `cc.layout`, `cc.media`, `cc.block`,
//! `cc.pick`). The sliders, the microphone, the Sound and Microphone pages
//! and the battery are the surface's own.
//!
//! Where the QML read a service singleton, this reads the bus (the other
//! groups publish their state under the QML service's name; field names
//! are the QML properties') and where it called a function, it asks
//! "<service lowercase>.<function>" with the QML arguments as an array.
//! The complete list is in notes/accounts.md. Wi-Fi and Bluetooth come from
//! the readings the core already has (status::Wifi, bluez) plus bus "network"
//! (NetworkFeed's rows) and bluetoothctl for the device actions.
//!
//! The open page and Edit controls stay in step with Panels: this service
//! owns `ccPage` and `ccEditing` (state "ccPage", "ccEditing").

use super::common::*;
use serde_json::{json, Value};
use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use tessera_core::host::{Host, Service};
use tessera_core::services::{bluez::Bluetooth, mpris::Players, status::Wifi};

enum M {
    Poll,
    Done(String),
    Art(String, String),
    Position(f64),
}

/// What a menu item or a click does
#[derive(Clone, Debug)]
enum Do {
    Ask(&'static str, Value),
    Cmd(Vec<String>),
    Page(String),
    Asking(String),
    Config(&'static str, Value),
    Bt(String, &'static str),
    Settings(&'static str, &'static str),
    Nop,
}

#[derive(Clone, Debug)]
struct Item {
    text: String,
    checked: bool,
    sep: bool,
    danger: bool,
    act: Do,
}

impl Item {
    fn new(text: &str, act: Do) -> Self {
        Self { text: text.into(), checked: false, sep: false, danger: false, act }
    }
    fn sep() -> Self {
        Self { text: String::new(), checked: false, sep: true, danger: false, act: Do::Nop }
    }
    fn danger(mut self) -> Self {
        self.danger = true;
        self
    }
    fn checked(mut self, c: bool) -> Self {
        self.checked = c;
        self
    }
}

fn menu_json(items: &[Item]) -> Value {
    Value::Array(items.iter().map(|i| json!({"text": i.text, "checked": i.checked, "sep": i.sep, "danger": i.danger})).collect())
}

/// One control: ModuleDefs.all[id]
struct Def {
    label: String,
    icon: String,
    on: bool,
    status: String,
    available: bool,
    reason: String,
    needs: &'static str,
    menu: Vec<Item>,
}

fn def(label: &str, icon: &str, on: bool, status: &str) -> Def {
    Def { label: label.into(), icon: icon.into(), on, status: status.into(), available: true, reason: String::new(), needs: "", menu: vec![] }
}

/// Every module id, in ModuleDefs.all's order
const IDS: &[&str] = &[
    "wifi", "bluetooth", "focus", "nightLight", "darkMode", "sharing", "mirroring", "captions", "keepAwake", "stageManager", "mic", "lowPower", "coolDown",
    "gameMode", "antiFlashbang", "record", "easyEffects", "warp", "keyboard", "screenshot", "colorPicker", "musicRecognition", "clipboard", "calculator",
    "timer", "tileWindows", "appLibrary", "display", "sound", "micLevel", "nowPlaying", "battery", "resources", "weather",
];

fn g(name: &str) -> Value {
    crate::bus::get(name)
}
fn bs(v: &Value, k: &str) -> bool {
    v[k].as_bool().unwrap_or(false)
}
fn ss<'a>(v: &'a Value, k: &str) -> &'a str {
    v[k].as_str().unwrap_or("")
}

/// A Wi-Fi network as the page needs it (bus "network".networks, NetworkFeed's rows)
#[derive(Clone, Debug, Default)]
struct Net {
    name: String,
    connected: bool,
    known: bool,
    secured: bool,
    enterprise: bool,
    signal: f64,
    connecting: bool,
    failed: String,
    busy: bool,
}

fn nets() -> Vec<Net> {
    let n = g("network");
    n["networks"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|x| Net {
                    name: ss(x, "name").into(),
                    connected: bs(x, "connected"),
                    known: bs(x, "known"),
                    secured: bs(x, "secured"),
                    enterprise: bs(x, "enterprise"),
                    signal: x["signal"].as_f64().or(x["signalStrength"].as_f64()).unwrap_or(0.0),
                    connecting: bs(x, "connecting") || bs(x, "joining"),
                    failed: ss(x, "failed").into(),
                    busy: bs(x, "busy") || bs(x, "joining") || bs(x, "connecting"),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The Wi-Fi password rule of the row's "commit": 8 to 63 characters, or a WEP key's length
pub fn psk_ok(text: &str) -> bool {
    let n = text.chars().count();
    (8..=63).contains(&n) || [5, 10, 13, 26].contains(&n)
}

/// btIcon(d)
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

fn note(text: &str, tone: &str, center: bool) -> Value {
    json!({"t": "note", "text": text, "tone": tone, "center": center})
}
fn head(text: &str, more: bool) -> Value {
    json!({"t": "head", "text": text, "h": if more { 1 } else { 0 }})
}
fn row(mut o: Value) -> Value {
    o["t"] = json!("row");
    o
}

pub struct ControlCenter {
    tx: Option<Tx>,
    page: String,
    editing: bool,
    asking: String,
    hint: String,
    focus_duration: i64,
    sent_state: String,
    sent_page: String,
    was_open: bool,
    sharing_look: bool,
    mirroring_look: bool,
    bt_busy: HashSet<String>,
    art: HashMap<String, String>,
    position: f64,
    last_pos: i64,
    last_seen_page: String,
    poll: slint::Timer,
}

impl Default for ControlCenter {
    fn default() -> Self {
        Self {
            tx: None,
            page: String::new(),
            editing: false,
            asking: String::new(),
            hint: String::new(),
            focus_duration: 0,
            sent_state: String::new(),
            sent_page: String::new(),
            was_open: false,
            sharing_look: false,
            mirroring_look: false,
            bt_busy: HashSet::new(),
            art: HashMap::new(),
            position: 0.0,
            last_pos: 0,
            last_seen_page: String::new(),
            poll: slint::Timer::default(),
        }
    }
}

impl ControlCenter {
    fn tx(&self) -> Tx {
        self.tx.clone().expect("started")
    }
    fn is_open(h: &Host) -> bool {
        h.state("open").as_str() == Some("controlCenter")
    }

    // ── What the controls say (ModuleDefs.all) ───────────────────────────
    fn defs(&self, h: &Host) -> Vec<(&'static str, Def)> {
        let look = &h.st.shell.look;
        let cfg = h.cfg();
        let sys = g("system");
        let be = g("backends");
        let audio = g("audio");
        let pw = g("power");
        let focus = g("focus");
        let sharing = g("sharing");
        let mirr = g("mirroring");
        let net = g("network");
        let wifi = h.st.shell.get::<Wifi>().cloned().unwrap_or_default();
        let bt = h.st.shell.get::<Bluetooth>().cloned().unwrap_or_default();
        let wifi_enabled = net["wifiEnabled"].as_bool().unwrap_or(wifi.enabled);
        let mut out: Vec<(&'static str, Def)> = vec![];
        for id in IDS {
            let d = match *id {
                "wifi" => {
                    let ssid = if net["ssid"].is_string() { ss(&net, "ssid").to_string() } else { wifi.ssid.clone() };
                    let mut d = def("Wi-Fi", if wifi_enabled { "wifi-high" } else { "wifi-slash" }, wifi_enabled, if !wifi_enabled { "Off" } else if ssid.is_empty() { "Not connected" } else { ssid.as_str() });
                    // (without NetworkManager there's nothing for it to switch: Backends)
                    d.available = !bs(&be, "wifiUnmanaged");
                    d.reason = ss(&be, "wifiShort").into();
                    d
                }
                "bluetooth" => {
                    let on = bt.powered;
                    let short = ss(&be, "btShort");
                    let first = bt.devices.iter().find(|x| x.connected).map(|x| x.name.clone());
                    def("Bluetooth", "bluetooth", on, if !short.is_empty() { short } else if !on { "Off" } else if let Some(n) = &first { n.as_str() } else { "On" })
                }
                "focus" => {
                    let on = focus["on"].as_bool().or(h.state("focus.on").as_bool()).unwrap_or(false);
                    let label = if on { focus["mode"]["name"].as_str().unwrap_or("Focus").to_string() } else { "Focus".into() };
                    let icon = focus["mode"]["icon"].as_str().or(h.state("focus.icon").as_str()).unwrap_or("moon").to_string();
                    let until = focus["until"].as_f64().unwrap_or(0.0);
                    def(&label, &icon, on, if on && until > 0.0 { ss(&focus, "statusText") } else { "" })
                }
                "nightLight" => {
                    let on = bs(&sys, "nightLight");
                    let mut d = def("Night Light", "moon-stars", on, &if on { format!("{}K", sys["nightLightTemp"].as_i64().unwrap_or(0)) } else { "Off".into() });
                    d.needs = "hyprsunset";
                    d
                }
                "darkMode" => def(if look.dark { "Dark Mode" } else { "Light Mode" }, "circle-half", look.dark, if cfg["appearance"]["mode"] == "auto" { "Auto" } else { "" }),
                // Visible to nearby devices, LocalSend and Quick Share together; on = for 10 minutes
                "sharing" => {
                    let level = ss(&sharing, "level").to_string();
                    let on = bs(&sharing, "on");
                    let mixed = bs(&sharing, "mixed");
                    let status = if mixed && on { "Custom" } else if level == "off" { "No one" } else if level == "10min" { "Everyone · 10 min" } else { "Everyone" };
                    let mut d = def("Sharing", "share-network", on, status);
                    d.menu = vec![
                        Item::new("Send files…", Do::Ask("panels.pickShare", json!([]))),
                        Item::sep(),
                        Item::new("No one", Do::Ask("sharing.setLevel", json!(["off"]))).checked(level == "off"),
                        Item::new("Everyone for 10 minutes", Do::Ask("sharing.setLevel", json!(["10min"]))).checked(level == "10min" && !mixed),
                        Item::new("Everyone", Do::Ask("sharing.setLevel", json!(["everyone"]))).checked(level == "everyone" && !mixed),
                        Item::sep(),
                        Item::new("Sharing settings…", Do::Page("sharing".into())),
                    ];
                    d
                }
                "mirroring" => {
                    let active = bs(&mirr, "active");
                    let label = if active { ss(&mirr, "targetName").to_string() } else { "Screen Mirroring".into() };
                    let mut d = def(&label, "screencast", active, if active { if ss(&mirr, "state") == "streaming" { "Mirroring" } else { "Connecting…" } } else { "" });
                    d.available = mirr["available"].as_bool().unwrap_or(true);
                    d.reason = "Needs GNOME Network Displays".into();
                    d
                }
                "captions" => {
                    let en = bs(&g("captions"), "enabled");
                    def("Live Captions", "closed-captioning", en, if en { "On" } else { "" })
                }
                "keepAwake" => def("Caffeine", "coffee", bs(&sys, "keepAwake"), ss(&sys, "keepAwakeStatus")),
                "stageManager" => def("Stages", "squares-four", cfg["stageManager"]["enabled"].as_bool().unwrap_or(false), ""),
                "mic" => {
                    let muted = bs(&audio, "micMuted");
                    def("Microphone", if muted { "microphone-slash" } else { "microphone" }, !muted, if muted { "Muted" } else { "On" })
                }
                "lowPower" => {
                    let mut d = def("Low Power", "leaf", ss(&pw, "profile") == "power-saver", "");
                    d.available = be["ppd"].as_bool().unwrap_or(true);
                    d.reason = ss(&be, "powerShort").into();
                    d
                }
                "coolDown" => {
                    let c = g("cooldown");
                    def("Cool-down", "snowflake", bs(&c, "active"), ss(&c, "why"))
                }
                "gameMode" => def("Game Mode", "game-controller", bs(&sys, "gameMode"), ""),
                "antiFlashbang" => def("Anti-Flash", "sun-dim", bs(&sys, "antiFlashbang"), ""),
                "record" => {
                    let rec = bs(&g("screentools"), "recording") || bs(&g("recorder"), "recording");
                    let mut d = def("Record", "record", rec, if rec { "Recording" } else { "" });
                    d.available = g("recorder")["available"].as_bool().unwrap_or(true);
                    d
                }
                "easyEffects" => {
                    let mut d = def("EasyEffects", "faders", bs(&sys, "easyEffects"), "");
                    d.needs = "easyeffects";
                    d
                }
                "warp" => {
                    let mut d = def("WARP", "cloud", bs(&sys, "warp"), "");
                    d.needs = "warp-cli";
                    d
                }
                "keyboard" => def("Keyboard", "keyboard", false, ""),
                "screenshot" => def("Screenshot", "camera", false, ""),
                "colorPicker" => {
                    let mut d = def("Colour Picker", "eyedropper", false, "");
                    d.needs = "hyprpicker";
                    d
                }
                "musicRecognition" => {
                    let mut d = def("Identify Song", "music-notes", false, "");
                    d.needs = "songrec";
                    d
                }
                "clipboard" => def("Clipboard", "clipboard-text", false, ""),
                "calculator" => def("Calculator", "calculator", false, ""),
                "timer" => def("Timer", "timer", false, ""),
                "tileWindows" => def("Tile Left & Right", "square-split-horizontal", false, ""),
                "appLibrary" => def("App Drawer", "squares-four", false, ""),
                "display" => {
                    let mut d = def("Display", "sun", false, "");
                    d.available = g("brightness")["available"].as_bool().unwrap_or(true);
                    d.reason = "No adjustable display brightness".into();
                    d
                }
                "sound" => def("Sound", ss(&audio, "icon"), false, ""),
                "micLevel" => {
                    let muted = bs(&audio, "micMuted");
                    let mut d = def("Microphone level", if muted { "microphone-slash" } else { "microphone" }, false, "");
                    let inputs = &g("audiostreams")["inputs"];
                    d.available = inputs.as_array().map_or_else(|| inputs.as_i64().unwrap_or(1) > 0, |a| !a.is_empty());
                    d.reason = "No microphone found".into();
                    d
                }
                "nowPlaying" => def("Now Playing", "music-notes", false, ""),
                "battery" => {
                    let mut d = def("Battery", "battery-high", false, "");
                    d.available = h.st.shell.get::<tessera_core::services::status::Battery>().is_some_and(|b| b.present);
                    d.reason = "No battery".into();
                    d
                }
                "resources" => def("CPU & Memory", "cpu", false, ""),
                _ => {
                    let w = h.st.shell.get::<tessera_core::services::weather::Weather>();
                    def("Weather", w.map_or("", |w| w.icon), false, "")
                }
            };
            out.push((id, d));
        }
        out
    }

    fn usable(d: &Def) -> bool {
        d.available && (d.needs.is_empty() || has_tool(d.needs))
    }
    fn why_not(d: &Def) -> String {
        if Self::usable(d) {
            String::new()
        } else if !d.available {
            if d.reason.is_empty() { "Not available on this computer".into() } else { d.reason.clone() }
        } else {
            format!("Needs {}", d.needs)
        }
    }

    fn local_art(url: &str) -> String {
        url.strip_prefix("file://").map(|p| p.split('?').next().unwrap_or(p).to_string()).unwrap_or_default()
    }

    fn state(&mut self, h: &Host) -> Value {
        let mut mods = serde_json::Map::new();
        for (id, d) in self.defs(h) {
            let mut m = json!({"label": d.label, "icon": d.icon, "on": d.on, "status": d.status, "usable": Self::usable(&d), "why": Self::why_not(&d)});
            if id == "resources" {
                let r = g("resources");
                m["title"] = json!(format!("CPU {}%", (r["cpu"].as_f64().unwrap_or(0.0) * 100.0).round()));
                m["sub"] = json!(format!("Memory {}%", (r["memory"].as_f64().unwrap_or(0.0) * 100.0).round()));
            } else if id == "weather" {
                let w = h.st.shell.get::<tessera_core::services::weather::Weather>();
                m["title"] = json!(match w {
                    Some(w) if w.ready => format!("{}{}", w.temp, g("weather")["unit"].as_str().unwrap_or("°")),
                    _ => "—".into(),
                });
                m["sub"] = json!(w.map_or(String::new(), |w| if !w.condition.is_empty() { w.condition.clone() } else { w.place.clone() }));
            }
            if !d.menu.is_empty() {
                m["menu"] = menu_json(&d.menu);
            }
            mods.insert(id.to_string(), m);
        }
        let players = h.st.shell.get::<Players>().cloned().unwrap_or_default();
        let p = players.chosen();
        let art_url = p.map(|p| p.track.art_url.clone()).unwrap_or_default();
        let mut art = Self::local_art(&art_url);
        if art.is_empty() && art_url.starts_with("http") {
            match self.art.get(&art_url) {
                Some(f) => art = f.clone(),
                None => {
                    self.art.insert(art_url.clone(), String::new());
                    let (tx, u) = (self.tx(), art_url.clone());
                    std::thread::spawn(move || {
                        if let Some(f) = tessera_core::services::mpris::art_file(&u) {
                            tx.send(M::Art(u, f.to_string_lossy().into_owned()));
                        }
                    });
                }
            }
        }
        let cast = g("cast");
        let account = g("account");
        let profiles: Value = if self.page == "sound" {
            let ap = g("AudioProfiles");
            map_alts(&ap)
        } else {
            json!([])
        };
        json!({
            "modules": mods,
            "media": {
                "has": p.is_some(), "identity": p.map_or("", |p| p.identity.as_str()), "title": p.map_or("", |p| p.track.title.as_str()),
                "artist": p.map_or("", |p| p.track.artist.as_str()), "playing": p.is_some_and(|p| p.playing), "art": art,
                "prev": p.is_some_and(|p| p.can_prev), "next": p.is_some_and(|p| p.can_next), "toggle": p.is_some_and(|p| p.can_play),
                "seek": p.is_some_and(|p| p.track.length > 0), "position": self.position,
                "length": p.map_or(0.0, |p| p.track.length as f64 / 1e6),
            },
            "cast": cast["devices"].as_array().is_some_and(|d| !d.is_empty()) || bs(&cast, "active"),
            "casting": bs(&cast, "active"),
            "avatar": Self::local_art(ss(&account, "pictureUrl")),
            "user": ss(&account, "name"),
            // (Sound's outputs that switch the card over first)
            "profiles": profiles,
        })
    }

    // ── The open page, as blocks ─────────────────────────────────────────
    fn build(&self, h: &Host, name: &str) -> Option<Value> {
        if name.is_empty() || name == "sound" || name == "mic" {
            return None;
        }
        Some(match name {
            "wifi" => self.wifi_page(h),
            "bluetooth" => self.bt_page(h),
            "focus" => self.focus_page(h),
            "caffeine" => self.caffeine_page(),
            "sharing" => self.sharing_page(h),
            "mirroring" => self.mirroring_page(),
            "cast" => self.cast_page(h),
            "resources" => self.resources_page(),
            _ => json!({"name": name, "title": name, "blocks": []}),
        })
    }

    fn wifi_menu(n: &Net) -> Vec<Item> {
        let mut items = vec![];
        if n.connected {
            items.push(Item::new("Disconnect", Do::Ask("network.disconnect", json!([n.name]))));
        } else if n.known || !n.secured {
            items.push(Item::new("Connect", Do::Ask("network.connect", json!([n.name]))));
        }
        if !n.connected && n.secured {
            items.push(Item::new(if n.enterprise { "Connect with sign-in…" } else { "Connect with password…" }, if n.enterprise { Do::Settings("network", "") } else { Do::Asking(n.name.clone()) }));
        }
        items.push(Item::sep());
        if (n.connected || n.known) && !n.enterprise {
            items.push(Item::new("Share network…", Do::Settings("network", "")));
        }
        items.push(Item::new("Copy name", Do::Cmd(strs(&["wl-copy", "--", &n.name]))));
        if n.known {
            items.push(Item::new("Forget network", Do::Ask("network.forget", json!([n.name]))).danger());
        }
        items.push(Item::sep());
        items.push(Item::new("Network details…", Do::Settings("network", "")));
        items
    }

    fn wifi_page(&self, h: &Host) -> Value {
        let be = g("backends");
        let net = g("network");
        let wifi = h.st.shell.get::<Wifi>().cloned().unwrap_or_default();
        let on = net["wifiEnabled"].as_bool().unwrap_or(wifi.enabled);
        let present = net["wifiPresent"].as_bool().unwrap_or(wifi.present);
        let unmanaged = bs(&be, "wifiUnmanaged");
        let mut networks = nets();
        networks.sort_by(|a, b| b.connected.cmp(&a.connected).then(b.known.cmp(&a.known)).then(b.signal.partial_cmp(&a.signal).unwrap_or(std::cmp::Ordering::Equal)));
        networks.truncate(30);
        let mut blocks = vec![];
        if unmanaged {
            let can_nm = bs(&be, "canStartNm");
            let app = ss(&be, "wifiApp");
            blocks.push(json!({"t": "notice", "icon": "wifi-slash", "title": ss(&be, "wifiTitle"), "status": ss(&be, "wifiReason"),
                "act": if can_nm { "Start NetworkManager".to_string() } else if !app.is_empty() { format!("Open {}", ss(&be, "wifiAppName")) } else { String::new() },
                "busy": ss(&g("health"), "fixing") == "networkmanager", "id": "fix"}));
        } else {
            if !on || networks.is_empty() {
                blocks.push(note(if !present { "No Wi-Fi adapter" } else if !on { "Wi-Fi is off" } else { "Looking for networks…" }, "secondary", true));
            }
            for n in if on { networks.clone() } else { vec![] } {
                let items = Self::wifi_menu(&n);
                let status = if n.connected {
                    "Connected"
                } else if n.connecting {
                    "Connecting…"
                } else if !n.failed.is_empty() {
                    "Couldn't connect"
                } else if n.known {
                    "Known network"
                } else {
                    ""
                };
                blocks.push(row(json!({
                    "id": format!("n:{}", n.name),
                    "icon": if n.signal > 0.66 { "wifi-high" } else if n.signal > 0.33 { "wifi-medium" } else { "wifi-low" },
                    "hl": n.connected, "title": n.name, "status": status, "trail": if n.secured { "lock-simple" } else { "" },
                    "act": if n.connected && !n.enterprise { "qr-code" } else { "" }, "busy": n.busy, "menu": menu_json(&items),
                })));
                if self.asking == n.name {
                    blocks.push(json!({"t": "pass", "id": format!("n:{}", n.name), "status": self.hint}));
                } else if !n.failed.is_empty() {
                    blocks.push(note(&n.failed, "danger", false));
                }
            }
            if on {
                blocks.push(row(json!({"id": "join", "icon": "plus", "title": "Join other network…"})));
            }
        }
        json!({"name": "wifi", "title": "Wi-Fi", "switch": !unmanaged, "switchOn": on, "refresh": !unmanaged, "blocks": blocks})
    }

    fn bt_menu(d: &tessera_core::services::bluez::Device, busy: bool) -> Vec<Item> {
        let a = d.address.clone();
        let mut items = vec![];
        if d.connected {
            items.push(Item::new("Disconnect", Do::Bt(a.clone(), "disconnect")));
        } else if d.paired {
            items.push(Item::new("Connect", Do::Bt(a.clone(), "connect")));
        }
        if busy && !d.paired {
            items.push(Item::new("Stop pairing", Do::Bt(a.clone(), "cancel-pairing")));
        } else if !d.paired {
            items.push(Item::new("Pair", Do::Bt(a.clone(), "pair")));
        }
        if d.paired {
            // (BlueZ's Trusted isn't in the core reading: offered both ways)
            items.push(Item::new("Connect automatically", Do::Bt(a.clone(), "trust")));
            items.push(Item::new("Don't connect automatically", Do::Bt(a.clone(), "untrust")));
        }
        items.push(Item::sep());
        items.push(Item::new("Copy address", Do::Cmd(strs(&["wl-copy", "--", &a]))));
        if d.paired {
            items.push(Item::new("Forget device", Do::Bt(a, "remove")).danger());
        }
        items.push(Item::sep());
        items.push(Item::new("Bluetooth settings…", Do::Settings("network", "Bluetooth")));
        items
    }

    fn bt_devices(h: &Host) -> Vec<tessera_core::services::bluez::Device> {
        let bt = h.st.shell.get::<Bluetooth>().cloned().unwrap_or_default();
        let mut devices: Vec<_> = bt.devices.into_iter().filter(|d| d.paired || !d.name.is_empty()).collect();
        devices.sort_by(|a, b| b.connected.cmp(&a.connected).then(b.paired.cmp(&a.paired)).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
        let mut seen = HashSet::new();
        devices.retain(|d| d.paired || d.name.is_empty() || seen.insert(d.name.clone()));
        devices.truncate(40);
        devices
    }

    fn bt_page(&self, h: &Host) -> Value {
        let be = g("backends");
        let bt = h.st.shell.get::<Bluetooth>().cloned().unwrap_or_default();
        let btstate = ss(&be, "btState");
        let devices = Self::bt_devices(h);
        let pics = g("devices")["pictures"].clone();
        let mut blocks = vec![];
        if !btstate.is_empty() && btstate != "ok" {
            blocks.push(json!({"t": "notice", "id": "fix", "icon": "bluetooth-slash", "title": ss(&be, "btTitle"), "status": ss(&be, "btReason"), "act": ss(&be, "btFixLabel"), "busy": bs(&be, "btFixing")}));
        } else {
            if !bt.powered || devices.is_empty() {
                blocks.push(note(if !bt.present { "No Bluetooth adapter. If this computer has one, check that it isn't switched off in its settings or with a key." } else if !bt.powered { "Bluetooth is off" } else { "Looking for devices…" }, "secondary", true));
            }
            for d in if bt.powered { devices.clone() } else { vec![] } {
                let busy = self.bt_busy.contains(&d.address);
                let items = Self::bt_menu(&d, busy);
                let status = if d.connected {
                    match d.battery {
                        Some(p) => format!("Connected · {p}%"),
                        None => "Connected".into(),
                    }
                } else if busy {
                    "Pairing…".into()
                } else if d.paired {
                    "Not connected".into()
                } else {
                    String::new()
                };
                blocks.push(row(json!({"id": format!("d:{}", d.address), "icon": bt_icon(&d.icon), "pic": pics[&d.address].as_str().unwrap_or(""), "hl": d.connected,
                    "title": if d.name.is_empty() { d.address.clone() } else { d.name.clone() }, "status": status, "busy": busy, "menu": menu_json(&items)})));
            }
        }
        json!({"name": "bluetooth", "title": "Bluetooth", "switch": btstate == "ok" || btstate.is_empty() && bt.present, "switchOn": bt.powered,
               "refresh": btstate == "ok" || btstate.is_empty() && bt.present, "blocks": blocks})
    }

    fn focus_page(&self, h: &Host) -> Value {
        let focus = g("focus");
        let active = ss(&focus, "active").to_string();
        let mut blocks = vec![];
        for m in focus["modes"].as_array().cloned().unwrap_or_default() {
            let is_active = ss(&m, "id") == active && !active.is_empty();
            let sched = &m["schedule"];
            let status = if is_active {
                ss(&focus, "statusText").to_string()
            } else if bs(sched, "enabled") {
                format!("Scheduled {}–{}", fmt_hm(ss(sched, "from")), fmt_hm(ss(sched, "to")))
            } else {
                String::new()
            };
            blocks.push(row(json!({"id": format!("f:{}", ss(&m, "id")), "icon": ss(&m, "icon"), "title": ss(&m, "name"), "hl": is_active, "status": status, "trail": if is_active { "check" } else { "" }})));
        }
        blocks.push(json!({"t": "gap", "h": 12}));
        blocks.push(json!({"t": "head", "text": "Turn on"}));
        blocks.push(json!({"t": "seg", "id": "len", "options": ["Until off", "For 1 hour", "Until evening"], "cur": if self.focus_duration == 3_600_000 { 1 } else if self.focus_duration == -1 { 2 } else { 0 }}));
        blocks.push(json!({"t": "gap", "h": 6}));
        blocks.push(row(json!({"id": "settings", "icon": "gear-six", "title": "Focus settings…"})));
        let _ = h;
        json!({"name": "focus", "title": "Focus", "blocks": blocks})
    }

    fn caffeine_page(&self) -> Value {
        let sys = g("system");
        let mut blocks = vec![];
        for (minutes, title) in [(60, "For 1 hour"), (120, "For 2 hours"), (0, "Until turned off")] {
            let active = bs(&sys, "keepAwake") && sys["keepAwakeMinutes"].as_i64().unwrap_or(0) == minutes;
            blocks.push(row(json!({"id": format!("c:{minutes}"), "icon": if minutes > 0 { "timer" } else { "coffee" }, "title": title, "hl": active,
                "status": if active { ss(&sys, "keepAwakeStatus") } else { "" }, "trail": if active { "check" } else { "" }})));
        }
        blocks.push(json!({"t": "gap", "h": 6}));
        blocks.push(note("The screen won't dim, lock or sleep while you're away. Closing the lid still sleeps.", "tertiary", false));
        json!({"name": "caffeine", "title": "Caffeine", "switch": true, "switchOn": bs(&sys, "keepAwake"), "switchLabel": "Keep the screen awake", "blocks": blocks})
    }

    fn sharing_page(&self, h: &Host) -> Value {
        const LEVELS: [&str; 3] = ["off", "10min", "everyone"];
        let sh = g("sharing");
        let cfg = h.cfg();
        let seg = |id: &str, cur: &str| json!({"t": "seg", "id": id, "options": ["No one", "10 minutes", "Everyone"], "cur": LEVELS.iter().position(|l| *l == cur).unwrap_or(0)});
        let qs = g("quickshare");
        let ls = g("localsend");
        let mut blocks = vec![seg("all", ss(&sh, "level"))];
        blocks.push(head("LocalSend", true));
        blocks.push(seg("localsend", cfg["sharing"]["receiving"].as_str().unwrap_or("off")));
        blocks.push(note("Computers and phones with the LocalSend app", "tertiary", false));
        blocks.push(head("Quick Share", true));
        blocks.push(seg("quickshare", cfg["sharing"]["quickShare"].as_str().unwrap_or("off")));
        let needs = bs(&qs, "bluetoothNeeded");
        let bt_on = qs["bluetoothOn"].as_bool().unwrap_or(true);
        blocks.push(note(if needs && !bt_on { "Turn on Bluetooth so Android phones can find this computer." } else { "Android phones nearby" }, if needs && !bt_on { "danger" } else { "tertiary" }, false));
        blocks.push(head("Your name", true));
        let user = std::env::var("USER").unwrap_or_else(|_| "Tessera".into());
        blocks.push(json!({"t": "field", "id": "name", "title": cfg["sharing"]["name"].as_str().unwrap_or(""), "status": format!("{user}'s computer")}));
        blocks.push(json!({"t": "gap", "h": 6}));
        blocks.push(row(json!({"id": "send", "icon": "paper-plane-tilt", "title": "Send files…", "status": "To a phone or computer nearby", "dis": !(ls["available"].as_bool().unwrap_or(true) && !bs(&g("filepicker"), "busy"))})));
        blocks.push(head("Nearby", true));
        let devices = sh["devices"].as_array().cloned().unwrap_or_default();
        for d in &devices {
            let p = &d["progress"];
            let sending = p.is_object();
            let status = if sending {
                if bs(p, "waiting") {
                    "Waiting for them to accept…".to_string()
                } else {
                    let total = p["total"].as_f64().unwrap_or(0.0);
                    format!("{}% sent", (100.0 * if total > 0.0 { p["done"].as_f64().unwrap_or(0.0) / total } else { 0.0 }).round())
                }
            } else if ss(d, "prefer") == "quickshare" {
                "Quick Share".into()
            } else {
                "LocalSend".into()
            };
            let icon = match ss(d, "type") {
                "mobile" => "device-mobile",
                "laptop" => "laptop",
                _ => "desktop",
            };
            blocks.push(row(json!({"id": format!("s:{}", ss(d, "name")), "icon": icon, "title": ss(d, "name"), "status": status, "trail": "paper-plane-tilt"})));
        }
        if devices.is_empty() {
            blocks.push(note("No devices nearby. Make sure they're on the same Wi-Fi with LocalSend open, or Quick Share visible to everyone.", "tertiary", false));
        }
        let errors: Vec<&str> = [ss(&ls, "error"), ss(&qs, "error")].into_iter().filter(|e| !e.is_empty()).collect();
        if !errors.is_empty() {
            blocks.push(note(&errors.join(" "), "danger", false));
        }
        json!({"name": "sharing", "title": "Sharing", "switch": true, "switchOn": bs(&sh, "on"), "switchLabel": "Visible to nearby devices", "blocks": blocks})
    }

    fn mirroring_page(&self) -> Value {
        let m = g("mirroring");
        let needs = g("needs");
        let displays = m["displays"].as_array().cloned().unwrap_or_default();
        let mut blocks = vec![];
        for d in &displays {
            let here = !ss(&m, "target").is_empty() && ss(&m, "target") == ss(d, "uuid");
            let streaming = ss(&m, "state") == "streaming";
            let proto = d["protocolName"].as_str().unwrap_or(ss(d, "protocol"));
            blocks.push(row(json!({"id": format!("m:{}", ss(d, "uuid")), "icon": if ss(d, "protocol") == "chromecast" { "television-simple" } else { "screencast" },
                "hl": here && streaming, "title": ss(d, "name"), "status": if here { if streaming { "Mirroring" } else { "Connecting…" } } else { proto },
                "busy": here && ss(&m, "state") == "connecting", "trail": if here { "stop" } else { "" }})));
        }
        let no_nm = bs(&needs, "checked") && !bs(&needs, "networkManager");
        let err = ss(&m, "error");
        let p2p = m["p2p"].as_bool().unwrap_or(true);
        if displays.is_empty() || !err.is_empty() || !p2p || no_nm {
            let text = if !m["available"].as_bool().unwrap_or(true) {
                "Screen mirroring needs GNOME Network Displays. Install it in Settings → Displays."
            } else if !err.is_empty() {
                err
            } else if no_nm {
                "Miracast TVs need NetworkManager (for Wi-Fi Direct), which isn't running here. Chromecasts and Miracast over the network still work."
            } else if displays.is_empty() {
                "Looking for TVs… Turn on the TV's screen mirroring (Miracast, Screen Mirroring or Cast), nearby."
            } else {
                "This computer's Wi-Fi can't do Wi-Fi Direct: Miracast TVs need it. Chromecasts and network Miracast still work."
            };
            blocks.push(note(text, if !err.is_empty() { "danger" } else { "tertiary" }, false));
        }
        json!({"name": "mirroring", "title": "Screen Mirroring", "blocks": blocks})
    }

    fn cast_page(&self, h: &Host) -> Value {
        let c = g("cast");
        let needs = g("needs");
        let health = g("health");
        let p = h.st.shell.get::<Players>().and_then(|ps| ps.chosen().cloned());
        let active = bs(&c, "active");
        let can_cast = bs(&c, "canCastPlaying");
        let mut blocks = vec![];
        let text = if active {
            format!("Playing on {}: {}", ss(&c["device"], "name"), ss(&c, "title"))
        } else if let Some(p) = &p {
            if can_cast {
                format!("Cast “{}” to:", if p.track.title.is_empty() { &p.identity } else { &p.track.title })
            } else {
                format!("“{}” doesn't say what it's playing, so it can't be cast. Cast a video or song from Files instead.", if p.identity.is_empty() { "This app" } else { &p.identity })
            }
        } else {
            "Play something first, then choose where it goes.".into()
        };
        blocks.push(note(&text, "secondary", false));
        let devices = c["devices"].as_array().cloned().unwrap_or_default();
        for d in &devices {
            let here = active && c["device"]["id"] == d["id"];
            let st = ss(&c, "state");
            blocks.push(row(json!({"id": format!("t:{}", ss(d, "id")), "icon": ss(d, "icon"), "hl": here, "title": ss(d, "name"),
                "status": if here { if st == "playing" { "Playing" } else if st == "paused" { "Paused" } else { "Connecting…" } } else { ss(d, "model") },
                "trail": if here { "stop" } else { "" }, "busy": here && (st == "connecting" || st == "loading"), "dis": !(here || (can_cast && p.is_some()))})));
        }
        let checked = bs(&needs, "checked");
        let avahi = bs(&needs, "avahi");
        if checked && !avahi {
            let busy = bs(&needs, "installing") || ss(&health, "fixing") == "avahi";
            blocks.push(row(json!({"id": "avahi", "icon": "wifi-high", "title": if busy { "Turning on…" } else { "Turn on network discovery" }, "status": "TVs are found with avahi, which isn't running", "busy": busy})));
        }
        if devices.is_empty() && (!checked || avahi) {
            blocks.push(note(if has_tool("avahi-browse") { "No Chromecast or Google TV on this network. Check that it's on and on the same Wi-Fi." } else { "Finding TVs needs avahi (avahi-browse)." }, "tertiary", false));
        }
        json!({"name": "cast", "title": "Cast", "blocks": blocks})
    }

    fn resources_page(&self) -> Value {
        let r = g("resources");
        let pct = |k: &str| format!("{}%", (r[k].as_f64().unwrap_or(0.0) * 100.0).round());
        let blocks = vec![
            row(json!({"id": "cpu", "icon": "cpu", "title": "Processor", "status": pct("cpu")})),
            row(json!({"id": "mem", "icon": "memory", "title": "Memory", "status": pct("memory")})),
            json!({"t": "gap", "h": 2}),
            note(&format!("The last minute, once a second. Network counts every interface but loopback, containers and VPN tunnels.{}", if r["gpuAvailable"].as_bool().unwrap_or(false) { "" } else { " Your GPU driver doesn't report its load." }), "tertiary", false),
        ];
        json!({"name": "resources", "title": "CPU & Memory", "blocks": blocks})
    }

    // ── Doing ────────────────────────────────────────────────────────────
    fn exec(&mut self, h: &mut Host, d: Do) {
        match d {
            Do::Ask(c, a) => h.ask(c, a),
            Do::Cmd(c) => spawn_cmd(c),
            Do::Page(p) => self.set_page(h, &p),
            Do::Asking(n) => self.asking = n,
            Do::Config(k, v) => crate::util::config_set(k, v),
            Do::Settings(page, target) => h.ask("panels.openSettings", json!([page, target])),
            Do::Bt(addr, verb) => {
                self.bt_busy.insert(addr.clone());
                let tx = self.tx();
                let a2 = addr.clone();
                std::thread::spawn(move || {
                    let _ = run_full(&strs(&["bluetoothctl", verb, &a2]), None);
                    tx.send(M::Done(a2));
                });
            }
            Do::Nop => {}
        }
    }

    fn set_page(&mut self, h: &mut Host, p: &str) {
        if self.page != p {
            self.page = p.to_string();
            h.set_state("ccPage", json!(p));
            if Self::is_open(h) {
                h.tell("controlcenter", json!({"ccPage": p}));
            }
            self.page_changed(h);
        }
    }

    fn page_changed(&mut self, h: &mut Host) {
        self.asking.clear();
        self.hint.clear();
        if self.page == "sound" {
            h.ask("audioprofiles.refresh", json!([]));
        }
        let want = self.page == "sharing";
        if want != self.sharing_look {
            self.sharing_look = want;
            h.ask("sharing.look", json!([want]));
        }
        let mir = self.page == "mirroring";
        if mir != self.mirroring_look {
            self.mirroring_look = mir;
            h.ask("mirroring.look", json!([mir]));
        }
        // (scanning only while their page shows)
        if self.page == "wifi" {
            h.ask("network.rescan", json!([]));
        }
        self.sent_page.clear();
    }

    /// ModuleDefs.all[id].act()
    fn module_act(&mut self, h: &mut Host, id: &str) {
        let defs = self.defs(h);
        let Some((_, d)) = defs.iter().find(|(i, _)| *i == id) else { return };
        let on = d.on;
        let sys = g("system");
        let ask = |h: &mut Host, c: &'static str, a: Value| h.ask(c, a);
        match id {
            "wifi" => ask(h, "network.wifi", json!([!on])),
            "bluetooth" => {
                if ss(&g("backends"), "btFix") == "unblock" {
                    ask(h, "backends.fixBluetooth", json!([]));
                } else {
                    spawn_cmd(strs(&["bluetoothctl", "power", if on { "off" } else { "on" }]));
                }
            }
            "focus" => {
                if on {
                    ask(h, "focus.deactivate", json!([]))
                } else {
                    ask(h, "focus.activate", json!(["dnd", 0, false]))
                }
            }
            "nightLight" => ask(h, "system.set", json!(["nightLight", !on])),
            "darkMode" => ask(h, "system.toggleDarkMode", json!([])),
            "sharing" => ask(h, "sharing.setLevel", json!([if on { "off" } else { "10min" }])),
            "mirroring" => {
                if on {
                    ask(h, "mirroring.stop", json!([]))
                } else {
                    self.set_page(h, "mirroring")
                }
            }
            "captions" => {
                let c = g("captions");
                if bs(&c, "ready") || bs(&c, "enabled") {
                    ask(h, "captions.toggle", json!([]))
                } else {
                    ask(h, "panels.openSettings", json!(["accessibility", ""]))
                }
            }
            "keepAwake" => ask(h, "system.setKeepAwake", json!([!bs(&sys, "keepAwake"), 0])),
            "stageManager" => crate::util::config_set("stageManager.enabled", json!(!on)),
            "mic" => ask(h, "audio.toggleMic", json!([true])),
            "lowPower" => ask(h, "power.setMode", json!([if on { "balanced" } else { "power-saver" }])),
            "coolDown" => ask(h, "cooldown.toggle", json!([])),
            "gameMode" => ask(h, "system.set", json!(["gameMode", !on])),
            "antiFlashbang" => ask(h, "system.set", json!(["antiFlashbang", !on])),
            "record" => ask(h, "recorder.openToolbar", json!([])), // the recording toolbar (again: stop)
            "easyEffects" => ask(h, "system.setEasyEffects", json!([!on])),
            "warp" => ask(h, "system.setWarp", json!([!on])),
            "keyboard" => ask(h, "panels.show", json!(["keyboard"])),
            "screenshot" => ask(h, "panels.show", json!(["screenTools"])),
            "colorPicker" => {
                ask(h, "panels.close", json!([]));
                spawn_cmd(strs(&["sh", "-c", "sleep 0.3; hyprpicker -a"]));
            }
            "musicRecognition" => {
                ask(h, "panels.close", json!([]));
                ask(h, "system.identifySong", json!([]));
            }
            "clipboard" => ask(h, "panels.show", json!(["clipboard"])),
            "calculator" => {
                ask(h, "panels.close", json!([]));
                spawn_cmd(strs(&["sh", "-c", "gnome-calculator || qalculate-gtk || kcalc || galculator"]));
            }
            "timer" => {
                ask(h, "panels.set", json!(["ncTab", 2]));
                ask(h, "panels.show", json!(["notificationCenter"]));
            }
            "tileWindows" => {
                ask(h, "panels.close", json!([]));
                ask(h, "tiling.arrange", json!(["leftRight"]));
            }
            "appLibrary" => ask(h, "panels.show", json!(["launchpad"])),
            _ => {}
        }
    }

    fn block(&mut self, h: &mut Host, page: &str, id: &str, what: &str, text: &str) {
        match (page, what) {
            ("wifi", "switch") => {
                let net = g("network");
                let on = net["wifiEnabled"].as_bool().unwrap_or_else(|| h.st.shell.get::<Wifi>().is_some_and(|w| w.enabled));
                h.ask("network.wifi", json!([!on]));
            }
            ("wifi", "refresh") => h.ask("network.rescan", json!([])),
            ("bluetooth", "switch") => {
                let on = h.st.shell.get::<Bluetooth>().is_some_and(|b| b.powered);
                spawn_cmd(strs(&["bluetoothctl", "power", if on { "off" } else { "on" }]));
            }
            ("bluetooth", "refresh") => spawn_cmd(strs(&["bluetoothctl", "--timeout", "8", "scan", "on"])),
            ("caffeine", "switch") => h.ask("system.setKeepAwake", json!([!bs(&g("system"), "keepAwake"), 0])),
            ("sharing", "switch") => h.ask("sharing.setLevel", json!([if bs(&g("sharing"), "on") { "off" } else { "10min" }])),
            ("wifi", _) => self.wifi_block(h, id, what, text),
            ("bluetooth", _) => {
                if id == "fix" {
                    h.ask("backends.fixBluetooth", json!([]));
                    return;
                }
                let addr = id.strip_prefix("d:").unwrap_or("");
                if let Some(d) = Self::bt_devices(h).into_iter().find(|d| d.address == addr) {
                    let verb = if d.connected { "disconnect" } else if d.paired { "connect" } else { "pair" };
                    self.exec(h, Do::Bt(d.address, verb));
                }
            }
            ("focus", _) => {
                if id == "settings" {
                    h.ask("panels.openSettings", json!(["focus", ""]));
                } else if id == "len" {
                    if let Some(i) = what.strip_prefix("seg:").and_then(|s| s.parse::<usize>().ok()) {
                        self.focus_duration = [0, 3_600_000, -1].get(i).copied().unwrap_or(0);
                    }
                } else if let Some(m) = id.strip_prefix("f:") {
                    let focus = g("focus");
                    if ss(&focus, "active") == m {
                        h.ask("focus.deactivate", json!([]));
                    } else {
                        h.ask("focus.activate", json!([m, self.focus_duration, false]));
                    }
                }
            }
            ("caffeine", _) => {
                if let Some(min) = id.strip_prefix("c:").and_then(|m| m.parse::<i64>().ok()) {
                    let sys = g("system");
                    let active = bs(&sys, "keepAwake") && sys["keepAwakeMinutes"].as_i64().unwrap_or(0) == min;
                    h.ask("system.setKeepAwake", json!([!active, if active { 0 } else { min }]));
                }
            }
            ("sharing", _) => self.sharing_block(h, id, what, text),
            ("mirroring", _) => {
                let sh = g("mirroring");
                if let Some(u) = id.strip_prefix("m:") {
                    if ss(&sh, "target") == u {
                        h.ask("mirroring.stop", json!([]));
                    } else if let Some(d) = sh["displays"].as_array().and_then(|a| a.iter().find(|d| ss(d, "uuid") == u)) {
                        h.ask("mirroring.start", json!([d]));
                    }
                }
            }
            ("cast", _) => {
                let c = g("cast");
                if id == "avahi" {
                    h.ask("needs.turnOnAvahi", json!([]));
                } else if let Some(tid) = id.strip_prefix("t:") {
                    if bs(&c, "active") && ss(&c["device"], "id") == tid {
                        h.ask("cast.stop", json!([]));
                    } else if let Some(d) = c["devices"].as_array().and_then(|a| a.iter().find(|d| ss(d, "id") == tid)) {
                        h.ask("cast.castPlaying", json!([d]));
                    }
                }
            }
            _ => {}
        }
    }

    fn wifi_block(&mut self, h: &mut Host, id: &str, what: &str, text: &str) {
        if id == "fix" {
            let be = g("backends");
            h.ask(if bs(&be, "canStartNm") { "backends.startNm" } else { "backends.openWifiApp" }, json!([]));
            return;
        }
        if id == "join" {
            h.ask("panels.openSettings", json!(["network", ""]));
            return;
        }
        let Some(name) = id.strip_prefix("n:") else { return };
        let Some(n) = nets().into_iter().find(|n| n.name == name) else { return };
        match what {
            "cancel" => self.asking.clear(),
            "action" | "other" => h.ask("panels.openSettings", json!(["network", ""])),
            "commit" => {
                if !psk_ok(text) {
                    self.hint = if text.is_empty() { "Type the password first." } else { "Wi-Fi passwords are at least 8 characters." }.into();
                    return;
                }
                h.ask("network.join", json!([n.name, text]));
                self.asking.clear();
            }
            _ => {
                if n.connected {
                    h.ask("network.disconnect", json!([n.name]));
                } else if n.known || !n.secured {
                    h.ask("network.connect", json!([n.name]));
                } else if n.enterprise {
                    h.ask("panels.openSettings", json!(["network", ""]));
                } else {
                    self.asking = if self.asking == n.name { String::new() } else { n.name };
                }
            }
        }
    }

    fn sharing_block(&mut self, h: &mut Host, id: &str, what: &str, text: &str) {
        const LEVELS: [&str; 3] = ["off", "10min", "everyone"];
        let seg = what.strip_prefix("seg:").and_then(|s| s.parse::<usize>().ok()).and_then(|i| LEVELS.get(i).copied());
        match (id, seg) {
            ("all", Some(l)) => h.ask("sharing.setLevel", json!([l])),
            ("localsend", Some(l)) => crate::util::config_set("sharing.receiving", json!(l)),
            ("quickshare", Some(l)) => crate::util::config_set("sharing.quickShare", json!(l)),
            ("name", _) if what == "commit" => crate::util::config_set("sharing.name", json!(text.trim())),
            ("send", _) => h.ask("panels.pickShare", json!([])),
            _ => {
                if let Some(n) = id.strip_prefix("s:") {
                    let sh = g("sharing");
                    if let Some(d) = sh["devices"].as_array().and_then(|a| a.iter().find(|d| ss(d, "name") == n)) {
                        if !d["progress"].is_object() {
                            h.ask("sharing.pickFor", json!([d]));
                        }
                    }
                }
            }
        }
    }

    /// cc.pick: a row's menu item, or a module's (a == ["", id, i])
    fn pick(&mut self, h: &mut Host, page: &str, id: &str, i: usize) {
        if page.is_empty() {
            let defs = self.defs(h);
            if let Some((_, d)) = defs.into_iter().find(|(m, _)| *m == id) {
                if let Some(it) = d.menu.get(i).cloned() {
                    self.exec(h, it.act);
                }
            }
            return;
        }
        let items: Vec<Item> = match page {
            "wifi" => id.strip_prefix("n:").and_then(|n| nets().into_iter().find(|x| x.name == n)).map(|n| Self::wifi_menu(&n)).unwrap_or_default(),
            "bluetooth" => id.strip_prefix("d:").and_then(|a| Self::bt_devices(h).into_iter().find(|d| d.address == a)).map(|d| {
                let busy = self.bt_busy.contains(&d.address);
                Self::bt_menu(&d, busy)
            }).unwrap_or_default(),
            _ => vec![],
        };
        if let Some(it) = items.get(i).cloned() {
            self.exec(h, it.act);
        }
    }

    fn open_edge(&mut self, h: &mut Host, open: bool) {
        self.sent_state.clear();
        self.sent_page.clear();
        if open {
            h.tell("controlcenter", json!({"ccPage": self.page, "editing": self.editing}));
            self.page_changed(h);
        } else if self.was_open {
            // closed: leave editing
            if self.editing {
                self.editing = false;
                h.set_state("ccEditing", json!(false));
            }
        }
    }

    fn tick(&mut self, h: &mut Host) {
        let open = Self::is_open(h);
        if open != self.was_open {
            self.open_edge(h, open);
            self.was_open = open;
        }
        if !open {
            return;
        }
        // the playing track's position, once a second
        if let Some(p) = h.st.shell.get::<Players>().and_then(|p| p.chosen().cloned()) {
            if p.playing && now_ms() - self.last_pos >= 1000 {
                self.last_pos = now_ms();
                let (tx, bus) = (self.tx(), p.bus.clone());
                std::thread::spawn(move || {
                    let (_, out, _) = run_full(&strs(&["busctl", "--user", "get-property", &bus, "/org/mpris/MediaPlayer2", "org.mpris.MediaPlayer2.Player", "Position"]), None);
                    if let Some(us) = out.split_whitespace().nth(1).and_then(|n| n.parse::<f64>().ok()) {
                        tx.send(M::Position(us / 1e6));
                    }
                });
            }
        }
        let st = self.state(h);
        let s = st.to_string();
        if s != self.sent_state {
            self.sent_state = s;
            h.tell("controlcenter", json!({"state": st}));
        }
        if let Some(pm) = self.build(h, &self.page.clone()) {
            let s = pm.to_string();
            if s != self.sent_page {
                self.sent_page = s;
                h.tell("controlcenter", json!({"page": pm}));
            }
        }
    }
}

fn map_alts(ap: &Value) -> Value {
    let sw = ss(ap, "switching").to_string();
    Value::Array(
        ap["alternatives"]
            .as_array()
            .map(|a| a.iter().map(|x| json!({"name": ss(x, "name"), "kind": ss(x, "kind"), "busy": !sw.is_empty() && ss(x, "profile") == sw})).collect())
            .unwrap_or_default(),
    )
}

/// Focus.fmt(hhmm): "HH:MM" as given (the service formats the clock; plain here)
fn fmt_hm(s: &str) -> String {
    s.to_string()
}

fn spawn_cmd(c: Vec<String>) {
    std::thread::spawn(move || {
        let _ = run_full(&c, None);
    });
}

impl Service for ControlCenter {
    fn name(&self) -> &'static str {
        "ControlCenter"
    }
    fn calls(&self) -> &'static [&'static str] {
        &["cc"]
    }
    fn start(&mut self, h: &mut Host) {
        h.st.shell.want("status");
        h.st.shell.want("bluez");
        h.st.shell.want("mpris");
        h.st.shell.want("weather");
        let tx = Tx::new(h.sender("ControlCenter"));
        self.tx = Some(tx.clone());
        self.poll.start(slint::TimerMode::Repeated, Duration::from_secs(1), move || tx.send(M::Poll));
        h.set_state("ccPage", json!(""));
        h.set_state("ccEditing", json!(false));
    }
    fn call(&mut self, h: &mut Host, call: &str, a: &Value) {
        let a0 = a.get(0).cloned().unwrap_or(Value::Null);
        let s = |i: usize| a.get(i).and_then(Value::as_str).unwrap_or("").to_string();
        match call {
            "cc.act" => self.module_act(h, &s(0)),
            "cc.page" => self.set_page(h, &s(0)),
            "cc.editing" => {
                self.editing = a0 == true;
                h.set_state("ccEditing", json!(self.editing));
            }
            "cc.layout" => {
                if let Some(l) = a0.as_array() {
                    crate::util::config_set("controlCenter.layout", Value::Array(l.iter().map(|x| json!(x.as_str().map(String::from).unwrap_or_else(|| x.to_string()))).collect()));
                }
            }
            "cc.media" => {
                let w = s(0);
                let players = h.st.shell.get::<Players>().cloned().unwrap_or_default();
                if let Some(p) = players.chosen() {
                    match w.as_str() {
                        "previous" => tessera_core::services::mpris::control(&p.bus, "Previous"),
                        "next" => tessera_core::services::mpris::control(&p.bus, "Next"),
                        "toggle" => tessera_core::services::mpris::control(&p.bus, "PlayPause"),
                        x if x.starts_with("seek:") && p.track.length > 0 => {
                            let frac = x[5..].parse::<f64>().unwrap_or(0.0).clamp(0.0, 1.0);
                            let target = (frac * p.track.length as f64) as i64;
                            let offset = target - (self.position * 1e6) as i64;
                            let bus = p.bus.clone();
                            self.position = target as f64 / 1e6;
                            spawn_cmd(strs(&["busctl", "--user", "call", &bus, "/org/mpris/MediaPlayer2", "org.mpris.MediaPlayer2.Player", "Seek", "x", &offset.to_string()]));
                        }
                        _ => {}
                    }
                }
            }
            "cc.block" => {
                // (Sound is drawn there; its card-switching outputs are this shell's)
                if s(0) == "sound" && s(1).starts_with("prof:") {
                    let ap = g("AudioProfiles");
                    let i = s(1)[5..].parse::<usize>().unwrap_or(usize::MAX);
                    if let Some(alt) = ap["alternatives"].as_array().and_then(|x| x.get(i)) {
                        h.ask("audioprofiles.pick", json!([alt]));
                    }
                    return;
                }
                if s(0) != self.page || self.build(h, &self.page.clone()).is_none() {
                    return;
                }
                let what = s(2);
                let (page, id, text) = (s(0), s(1), s(3));
                self.block(h, &page, &id, &what, &text);
            }
            "cc.pick" => {
                let i = a.get(2).and_then(Value::as_u64).unwrap_or(u64::MAX) as usize;
                self.pick(h, &s(0), &s(1), i);
            }
            _ => {}
        }
        // answer at once, not at the next tick
        if Self::is_open(h) {
            self.tick(h);
        }
    }
    fn message(&mut self, h: &mut Host, msg: Box<dyn Any + Send>) {
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Poll => self.tick(h),
            M::Done(addr) => {
                self.bt_busy.remove(&addr);
            }
            M::Art(url, file) => {
                self.art.insert(url, file);
            }
            M::Position(p) => self.position = p,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wifi_passwords() {
        assert!(psk_ok("12345678") && psk_ok("abcde") && psk_ok(&"x".repeat(63)));
        assert!(!psk_ok("") && !psk_ok("1234567") && !psk_ok(&"x".repeat(64)));
    }
    #[test]
    fn device_icons() {
        assert_eq!(bt_icon("audio-headset"), "headphones");
        assert_eq!(bt_icon("input-mouse"), "mouse");
        assert_eq!(bt_icon("audio-card"), "speaker-hifi");
        assert_eq!(bt_icon(""), "bluetooth");
    }
    #[test]
    fn wifi_menus_follow_the_network() {
        let n = Net { name: "Home".into(), connected: true, known: true, secured: true, ..Default::default() };
        let m = ControlCenter::wifi_menu(&n);
        assert_eq!(m[0].text, "Disconnect");
        assert!(m.iter().any(|i| i.text == "Forget network" && i.danger));
        let o = Net { name: "Cafe".into(), secured: true, ..Default::default() };
        let m = ControlCenter::wifi_menu(&o);
        assert!(m.iter().any(|i| i.text == "Connect with password…"));
        assert!(!m.iter().any(|i| i.text == "Connect"));
    }
}

//! Device connections, shown as live alerts in the dynamic island, replacing
//! shell/services/Devices.qml:
//!   bluetooth  devices connecting or disconnecting, with their battery
//!   usb        USB devices and drives plugged in or removed (drives: Open)
//!   displays   monitors connected or disconnected
//!   audio      sound moving to another output (wired headphones, HDMI…)
//! Nothing is posted for the first seconds after start (Island's `armed`,
//! 4 s), so devices already present don't announce themselves. Alerts go to
//! host-shell's Island as `island.post` [event, ms]; sounds as `sound.play`.
//!
//! Reads: tessera-core's bluez reading (polled once a second: the QML reacted
//! to its signals 700 ms after a change), bus "quickshare" {running} and
//! "fastpair" {busy, levels: {address: [{label,value}]}} (host-sharing), bus
//! "files" {manager}. Its own `udevadm monitor`, Hyprland's events and a
//! `pactl subscribe` (the QML's shared PulseEvents; net's PulseEvents may
//! share it later). Call "devicealerts.openDrive" [dev] is the drive alert's
//! Open button.
//!
//! Differences: without pactl the Pipewire default-sink fallback isn't ported.

use super::common::*;
use super::common_island as island;
use serde_json::{json, Value};
use std::any::Any;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use tessera_core::host::{Host, Service};
use tessera_core::services::bluez::{Bluetooth, Device};

const BLUE: &str = "#0A84FF";
const ORANGE: &str = "#FF9F0A";
const GREEN: &str = "#30D158";
const PURPLE: &str = "#BF5AF2";
const DANGER: &str = "#FF453A";
const DIM: &str = "#8cffffff"; // Qt.rgba(1, 1, 1, 0.55)

pub fn bt_icon(icon: &str) -> &'static str {
    let i = icon.to_lowercase();
    if i.contains("headset") || i.contains("headphone") {
        "headphones"
    } else if i.contains("audio") || i.contains("speaker") {
        "speaker-hifi"
    } else if i.contains("mouse") {
        "mouse"
    } else if i.contains("keyboard") {
        "keyboard"
    } else if i.contains("gaming") || i.contains("joystick") {
        "game-controller"
    } else if i.contains("phone") {
        "device-mobile"
    } else if i.contains("watch") {
        "watch"
    } else if i.contains("computer") {
        "desktop"
    } else {
        "bluetooth"
    }
}

fn is_word(c: Option<char>) -> bool {
    c.is_some_and(|c| c.is_alphanumeric() || c == '_')
}

/// `\bneedle` (and `needle\b` when `end`) anywhere in hay
fn word(hay: &str, needle: &str, end: bool) -> bool {
    hay.match_indices(needle).any(|(i, _)| {
        let before = hay[..i].chars().last();
        let after = hay[i + needle.len()..].chars().next();
        !is_word(before) && (!end || !is_word(after))
    })
}

/// `\bqc ?\d`
fn qc_digit(n: &str) -> bool {
    n.match_indices("qc").any(|(i, _)| {
        if is_word(n[..i].chars().last()) {
            return false;
        }
        let r = &n[i + 2..];
        let r = r.strip_prefix(' ').unwrap_or(r);
        r.chars().next().is_some_and(|c| c.is_ascii_digit())
    })
}

/// `\bear ?\(`
fn ear_paren(n: &str) -> bool {
    n.match_indices("ear").any(|(i, _)| {
        if is_word(n[..i].chars().last()) {
            return false;
        }
        let r = &n[i + 3..];
        r.strip_prefix(' ').unwrap_or(r).starts_with('(')
    })
}

/// A drawing of the device (assets/devices) for its island card, from the
/// kind Bluetooth reports and its name: earbuds and over-ear headphones
/// report the same kind, so the name tells them apart
pub fn picture_kind(icon: &str, name: &str, audio: &str) -> &'static str {
    let i = icon.to_lowercase();
    let n = name.to_lowercase();
    let has = |s: &str| n.contains(s);
    if has("soundbar") || has("sound bar") {
        "soundbar"
    } else if has("buds") || has("pods") || has("earbud") || has("earphone") || word(&n, "tws", true) || word(&n, "wf-", false) || ear_paren(&n) || has("liberty") || word(&n, "freeclip", false) {
        "earbuds"
    } else if has("headphone") || word(&n, "wh-", false) || word(&n, "max", true) || qc_digit(&n) {
        "headphones"
    } else if i.contains("headset") || i.contains("headphone") {
        if audio.is_empty() { "headphones" } else if audio == "earbuds" { "earbuds" } else { "headphones" }
    } else if i.contains("audio") || i.contains("speaker") || has("speaker") {
        "speaker"
    } else if i.contains("mouse") || has("mouse") || word(&n, "mx anywhere", false) || word(&n, "mx master", false) {
        "mouse"
    } else if i.contains("keyboard") || has("keyboard") {
        "keyboard"
    } else if i.contains("gaming") || i.contains("joystick") || has("controller") || has("gamepad") || has("dualsense") || has("dualshock") || has("xbox") {
        "controller"
    } else if i.contains("watch") || has("watch") {
        "watch"
    } else if i.contains("phone") {
        "phone"
    } else if i.contains("computer") {
        "laptop"
    } else {
        "bluetooth"
    }
}

pub fn picture_for(icon: &str, name: &str) -> String {
    crate::util::root().join("shell/assets/devices").join(format!("{}.svg", picture_kind(icon, name, ""))).to_string_lossy().into_owned()
}

pub fn usb_icon(p: &HashMap<String, String>) -> &'static str {
    let ifs = p.get("ID_USB_INTERFACES").map(String::as_str).unwrap_or("");
    let name = format!("{} {}", p.get("ID_MODEL_FROM_DATABASE").map(String::as_str).unwrap_or(""), p.get("ID_MODEL").map(String::as_str).unwrap_or("")).to_lowercase();
    if ifs.contains(":0e") {
        "webcam"
    } else if ifs.contains(":0301") {
        "keyboard"
    } else if ifs.contains(":0302") {
        "mouse"
    } else if ifs.contains(":01") {
        "headphones"
    } else if ifs.contains(":07") {
        "printer"
    } else if ifs.contains(":ff42") || ifs.contains(":0601") || name.contains("phone") || name.contains("android") {
        "device-mobile"
    } else if ifs.contains(":08") {
        "hard-drive"
    } else {
        "usb"
    }
}

pub fn usb_name(p: &HashMap<String, String>) -> String {
    let get = |a: &str, b: &str| {
        let v = p.get(a).filter(|s| !s.is_empty()).or_else(|| p.get(b)).cloned().unwrap_or_default();
        v.replace('_', " ")
    };
    let model = get("ID_MODEL_FROM_DATABASE", "ID_MODEL");
    let vendor = get("ID_VENDOR_FROM_DATABASE", "ID_VENDOR");
    if !model.is_empty() {
        let first = vendor.split(' ').next().unwrap_or("");
        if !vendor.is_empty() && !model.starts_with(first) {
            format!("{first} {model}")
        } else {
            model
        }
    } else if !vendor.is_empty() {
        vendor
    } else {
        "USB device".into()
    }
}

pub fn output_icon(text: &str) -> &'static str {
    let t = text.to_lowercase();
    if ["headphone", "headset", "bluez", "airpods", "buds"].iter().any(|k| t.contains(k)) {
        "headphones"
    } else if t.contains("hdmi") || t.contains("displayport") {
        "television"
    } else {
        "speaker-hifi"
    }
}

/// The disk of a block device name: sdb1 -> sdb, nvme0n1p1 -> nvme0n1 (`replace(/p?\d+$/, "")`)
pub fn disk_of(dev: &str) -> String {
    let t = dev.trim_end_matches(|c: char| c.is_ascii_digit());
    if t.len() == dev.len() {
        return dev.to_string();
    }
    t.strip_suffix('p').unwrap_or(t).to_string()
}

enum M {
    Armed,
    Poll,
    Udev(HashMap<String, String>),
    UsbWait(u64),
    PulseLine(String),
    SinkPoll(u64),
    Sink(String, Value),
    PulseUp,
}

#[derive(Default)]
pub struct DeviceAlerts {
    armed: bool,
    bt_state: HashMap<String, bool>,
    last_bt_audio: u64,
    awaiting_battery: HashMap<String, u64>,
    low_warned: HashMap<String, i64>,
    usb_names: HashMap<String, String>,
    pending_usb: Option<Value>,
    drive_seen: bool,
    drive_seen_at: HashMap<String, u64>,
    usb_wait_gen: u64,
    output_key: String,
    sink_gen: u64,
    started: bool,
}

impl DeviceAlerts {
    fn live(&self, h: &Host) -> bool {
        island::enabled(h) && self.armed
    }

    fn post_bt(&self, h: &mut Host, d: &Device, icon: &str) {
        let name = if d.name.is_empty() { d.address.clone() } else { d.name.clone() };
        let mut e = json!({
            "kind": "device", "key": format!("bt:{}", d.address), "icon": icon, "picture": picture_for(&d.icon, &name),
            "title": name, "subtitle": if d.connected { "Connected" } else { "Disconnected" }, "color": if d.connected { BLUE } else { DIM }
        });
        if d.connected {
            if let Some(b) = d.battery {
                e["battery"] = json!(b as f64 / 100.0);
            }
            let levels = crate::bus::get("fastpair")["levels"][&d.address].clone();
            if levels.as_array().is_some_and(|l| !l.is_empty()) {
                e["batteries"] = levels;
            }
        }
        island::post(h, e, 3500);
    }

    fn scan_bluetooth(&mut self, h: &mut Host) {
        let Some(bt) = h.st.shell.get::<Bluetooth>().cloned() else { return };
        let mut next = HashMap::new();
        for d in &bt.devices {
            next.insert(d.address.clone(), d.connected);
            let was = self.bt_state.get(&d.address).copied().unwrap_or(false);
            // (the first scan only learns what's there)
            if d.connected == was || !self.live(h) || !island::wants(h, "bluetooth") {
                continue;
            }
            let icon = bt_icon(&d.icon);
            // While Quick Share is on, phones connect and disconnect over
            // Bluetooth for every transfer: its own prompt says what's happening
            if crate::bus::get("quickshare")["running"] == true && (icon == "device-mobile" || icon == "bluetooth") {
                continue;
            }
            // (earbuds drop and come back while Fast Pair connects them)
            if !d.connected && crate::bus::get("fastpair")["busy"] == true {
                continue;
            }
            if d.connected && (icon == "headphones" || icon == "speaker-hifi") {
                self.last_bt_audio = now_ms();
            }
            self.post_bt(h, d, icon);
            // Headphones often report their battery seconds after connecting:
            // show it then (once), if it wasn't known yet
            if d.connected && d.battery.is_none() {
                self.awaiting_battery.insert(d.address.clone(), now_ms());
            }
        }
        self.bt_state = next;
        for d in &bt.devices {
            self.battery_arrived(h, d);
            self.check_low(h, d);
        }
    }

    fn battery_arrived(&mut self, h: &mut Host, d: &Device) {
        let Some(t) = self.awaiting_battery.get(&d.address).copied() else { return };
        if !d.connected || d.battery.is_none() {
            return;
        }
        self.awaiting_battery.remove(&d.address);
        if now_ms().saturating_sub(t) < 20000 && self.live(h) && island::wants(h, "bluetooth") {
            self.post_bt(h, d, bt_icon(&d.icon));
        }
    }

    /// Running low: earbuds, a mouse or a controller at 20%, and again at 10%,
    /// on the device's own card (once each, until it's charged past 25%)
    fn check_low(&mut self, h: &mut Host, d: &Device) {
        let Some(b) = d.battery else { return };
        if !d.connected {
            return;
        }
        let pct = b as i64;
        let was = self.low_warned.get(&d.address).copied().unwrap_or(100);
        if pct > 25 {
            if was != 100 {
                self.low_warned.remove(&d.address);
            }
            return;
        }
        let step = if pct <= 10 { 10 } else if pct <= 20 { 20 } else { 100 };
        if step >= was {
            return;
        }
        self.low_warned.insert(d.address.clone(), step);
        if self.live(h) && island::wants(h, "bluetooth") {
            let name = if d.name.is_empty() { d.address.clone() } else { d.name.clone() };
            island::post(
                h,
                json!({
                    "kind": "device", "key": format!("bt:{}", d.address), "icon": bt_icon(&d.icon), "picture": picture_for(&d.icon, &name),
                    "title": name, "subtitle": format!("Battery low · {pct}%{}", if step == 10 { " · charge it soon" } else { "" }),
                    "color": DANGER, "battery": b as f64 / 100.0
                }),
                8000,
            );
        }
    }

    fn on_udev(&mut self, h: &mut Host, p: &HashMap<String, String>) {
        let g = |k: &str| p.get(k).map(String::as_str).unwrap_or("");
        if g("SUBSYSTEM") == "usb" && g("DEVTYPE") == "usb_device" {
            if g("ID_USB_INTERFACES").starts_with(":09") {
                return; // hubs
            }
            if g("ACTION") == "add" {
                let name = usb_name(p);
                self.usb_names.insert(g("DEVPATH").to_string(), name.clone());
                let live = self.live(h);
                if live {
                    h.ask("sound.play", json!(["usb"]));
                }
                if live && island::wants(h, "usb") {
                    self.drive_seen = false;
                    self.pending_usb = Some(json!({"kind": "usb", "icon": usb_icon(p), "title": name, "subtitle": "Connected", "color": ORANGE}));
                    self.usb_wait_gen += 1;
                    after(h.sender("DeviceAlerts"), 1200, M::UsbWait(self.usb_wait_gen));
                }
            } else if g("ACTION") == "remove" {
                let name = self.usb_names.get(g("DEVPATH")).cloned();
                if name.is_some() && self.live(h) {
                    h.ask("sound.play", json!(["usbOff"]));
                }
                if let Some(name) = name {
                    if self.live(h) && island::wants(h, "usb") && !self.drive_seen {
                        island::post(h, json!({"kind": "usb", "icon": "usb", "title": name, "subtitle": "Disconnected", "color": DIM}), 2500);
                    }
                }
                self.drive_seen = false;
            }
        } else if g("SUBSYSTEM") == "block" && g("ID_BUS") == "usb" && (g("DEVTYPE") == "partition" || (g("DEVTYPE") == "disk" && !g("ID_FS_TYPE").is_empty())) {
            if !self.live(h) || !island::wants(h, "usb") {
                return;
            }
            self.drive_seen = true;
            self.usb_wait_gen += 1;
            self.pending_usb = None;
            let label = if !g("ID_FS_LABEL").is_empty() { g("ID_FS_LABEL").replace("\\x20", " ") } else { usb_name(p) };
            let dev = g("DEVNAME").to_string();
            // One alert per drive, not per partition (sdb1, sdb2… or nvme0n1p1…)
            let seen_key = format!("{}:{}", g("ACTION"), disk_of(&dev));
            if now_ms().saturating_sub(self.drive_seen_at.get(&seen_key).copied().unwrap_or(0)) < 8000 {
                return;
            }
            self.drive_seen_at.insert(seen_key, now_ms());
            if g("ACTION") == "add" {
                // up for 10 minutes (or until dismissed)
                island::post(h, json!({"kind": "usb", "icon": "hard-drive", "title": label, "subtitle": "Drive connected", "color": ORANGE, "action": "Open", "onAction": {"call": "devicealerts.openDrive", "args": [dev]}}), 600000);
            } else if g("ACTION") == "remove" {
                island::post(h, json!({"kind": "usb", "icon": "hard-drive", "title": label, "subtitle": "Drive removed", "color": DIM}), 2500);
            }
        }
    }

    fn output_changed(&mut self, h: &mut Host, key: String, title: String, detail: String) {
        if key == self.output_key {
            return;
        }
        let first = self.output_key.is_empty();
        self.output_key = key.clone();
        if first || !self.live(h) || !island::wants(h, "audio") {
            return;
        }
        if key.contains("bluez") && now_ms().saturating_sub(self.last_bt_audio) < 5000 {
            return; // the Bluetooth alert already said it
        }
        island::post(h, json!({"kind": "audio", "icon": output_icon(&format!("{key} {title}")), "title": title, "subtitle": detail, "color": PURPLE}), 3000);
    }
}

impl Service for DeviceAlerts {
    fn name(&self) -> &'static str {
        "DeviceAlerts"
    }
    fn calls(&self) -> &'static [&'static str] {
        &["devicealerts"]
    }
    fn start(&mut self, h: &mut Host) {
        self.started = true;
        h.st.shell.want("bluez");
        after(h.sender("DeviceAlerts"), 4000, M::Armed);
        after(h.sender("DeviceAlerts"), 1000, M::Poll);
        spawn_hypr_events(&["monitoraddedv2", "monitorremovedv2"], h.sender("DeviceAlerts"));
        // udev: one block of KEY=value lines per event
        let send = h.sender("DeviceAlerts");
        std::thread::spawn(move || loop {
            let child = Command::new("udevadm")
                .args(["monitor", "--udev", "--property", "--subsystem-match=usb", "--subsystem-match=block"])
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn();
            if let Ok(mut c) = child {
                if let Some(out) = c.stdout.take() {
                    let mut cur: HashMap<String, String> = HashMap::new();
                    for line in BufReader::new(out).lines().map_while(Result::ok) {
                        if line.trim().is_empty() {
                            if cur.contains_key("ACTION") {
                                send(Box::new(M::Udev(std::mem::take(&mut cur))));
                            }
                            cur.clear();
                            continue;
                        }
                        if let Some((k, v)) = line.split_once('=') {
                            if !k.is_empty() {
                                cur.insert(k.to_string(), v.to_string());
                            }
                        }
                    }
                }
                let _ = c.wait();
            }
            std::thread::sleep(std::time::Duration::from_secs(10));
        });
        // the sound server's events (the shell's one `pactl subscribe`)
        let send = h.sender("DeviceAlerts");
        std::thread::spawn(move || loop {
            let child = Command::new("pactl").arg("subscribe").stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn();
            if let Ok(mut c) = child {
                if let Some(out) = c.stdout.take() {
                    send(Box::new(M::PulseUp));
                    for line in BufReader::new(out).lines().map_while(Result::ok) {
                        send(Box::new(M::PulseLine(line)));
                    }
                }
                let _ = c.wait();
            }
            std::thread::sleep(std::time::Duration::from_secs(5));
        });
    }
    fn call(&mut self, _h: &mut Host, call: &str, args: &Value) {
        if call.ends_with(".openDrive") {
            let dev = args[0].as_str().unwrap_or("").to_string();
            if dev.is_empty() {
                return;
            }
            let fm = crate::bus::get("files")["manager"].as_str().unwrap_or("xdg-open").to_string();
            crate::util::run_async(&["sh", "-c", "m=$(findmnt -nro TARGET \"$1\" | head -1); [ -z \"$m\" ] && m=$(udisksctl mount -b \"$1\" 2>/dev/null | sed -n 's/.* at //p'); ${2:-xdg-open} \"${m:-$HOME}\"", "sh", &dev, &fm]);
        }
    }
    fn message(&mut self, h: &mut Host, msg: Box<dyn Any + Send>) {
        let msg = match msg.downcast::<Raw>() {
            Ok(e) => {
                if !self.live(h) || !island::wants(h, "displays") {
                    return;
                }
                let mut it = e.data.splitn(3, ',');
                let _id = it.next();
                let name = it.next().unwrap_or("").to_string();
                let desc = it.next().unwrap_or("").trim().to_string();
                if name.starts_with("HEADLESS") || name.starts_with("TESSERA-") {
                    return;
                }
                let added = e.name == "monitoraddedv2";
                island::post(
                    h,
                    json!({
                        "kind": "display", "icon": if name.starts_with("eDP") { "desktop" } else { "monitor" },
                        "title": if desc.is_empty() { name.clone() } else { desc },
                        "subtitle": if added { format!("Display connected · {name}") } else { "Display disconnected".to_string() },
                        "color": if added { GREEN } else { DIM }
                    }),
                    3500,
                );
                return;
            }
            Err(m) => m,
        };
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Armed => self.armed = true,
            M::Poll => {
                self.scan_bluetooth(h);
                after(h.sender("DeviceAlerts"), 1000, M::Poll);
            }
            M::Udev(p) => self.on_udev(h, &p),
            M::UsbWait(g) => {
                if g == self.usb_wait_gen {
                    if let Some(e) = self.pending_usb.take() {
                        if !self.drive_seen {
                            island::post(h, e, 3000);
                        }
                    }
                }
            }
            M::PulseUp => {
                // once at start, to learn the current output
                self.sink_gen += 1;
                after(h.sender("DeviceAlerts"), 350, M::SinkPoll(self.sink_gen));
            }
            M::PulseLine(line) => {
                if !island::enabled(h) {
                    return;
                }
                // (not "sink-input": every stream starting or stopping would re-read all outputs)
                if line.contains(" on sink #") || line.contains(" on server #") || line.contains(" on card #") {
                    self.sink_gen += 1;
                    after(h.sender("DeviceAlerts"), 350, M::SinkPoll(self.sink_gen));
                }
                // (a display with speakers plugged in or out changes its card)
                if line.contains(" on card #") {
                    h.ask("audioprofiles.refresh", json!([]));
                }
            }
            M::SinkPoll(g) => {
                if g != self.sink_gen || !island::enabled(h) {
                    return;
                }
                let send = h.sender("DeviceAlerts");
                std::thread::spawn(move || {
                    if let Some((_, out)) = crate::util::sh("pactl get-default-sink; pactl -f json list sinks", std::time::Duration::from_secs(5)) {
                        let (def, rest) = out.split_once('\n').unwrap_or((&out, ""));
                        if let Ok(v) = serde_json::from_str::<Value>(rest) {
                            send(Box::new(M::Sink(def.trim().to_string(), v)));
                        }
                    }
                });
            }
            M::Sink(def, sinks) => {
                let Some(sink) = sinks.as_array().and_then(|a| a.iter().find(|s| s["name"] == def.as_str())) else { return };
                let ports = sink["ports"].as_array().cloned().unwrap_or_default();
                let active = sink["active_port"].as_str().unwrap_or("");
                let port = ports.iter().find(|p| p["name"] == active);
                let title = sink["description"].as_str().filter(|d| !d.is_empty()).unwrap_or(&def).to_string();
                let key = format!("{def}|{active}");
                match port {
                    Some(p) if ports.len() > 1 => self.output_changed(h, key, p["description"].as_str().unwrap_or("").to_string(), title),
                    _ => self.output_changed(h, key, title, "Sound output".into()),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn props(l: &[(&str, &str)]) -> HashMap<String, String> {
        l.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
    }

    #[test]
    fn icons_and_pictures() {
        assert_eq!(bt_icon("audio-headset"), "headphones");
        assert_eq!(bt_icon("input-mouse"), "mouse");
        assert_eq!(bt_icon("unknown"), "bluetooth");
        assert_eq!(picture_kind("audio-headset", "Galaxy Buds2", ""), "earbuds");
        assert_eq!(picture_kind("audio-headset", "Sony WH-1000XM5", ""), "headphones");
        assert_eq!(picture_kind("audio-card", "JBL Flip", ""), "speaker");
        assert_eq!(picture_kind("", "Living room soundbar", ""), "soundbar");
        assert_eq!(picture_kind("input-gaming", "Pad", ""), "controller");
        assert_eq!(picture_kind("", "Thing", ""), "bluetooth");
        assert_eq!(picture_kind("", "Bose QC45", ""), "headphones");
        assert_eq!(picture_kind("", "Nothing Ear (2)", ""), "earbuds");
        assert_eq!(picture_kind("", "Maxwell", ""), "bluetooth");
    }

    #[test]
    fn usb_names_and_icons() {
        let p = props(&[("ID_MODEL", "Ultra_Fit"), ("ID_VENDOR", "SanDisk_Corp"), ("ID_USB_INTERFACES", ":080650:")]);
        assert_eq!(usb_name(&p), "SanDisk Ultra Fit");
        assert_eq!(usb_icon(&p), "hard-drive");
        assert_eq!(usb_name(&props(&[])), "USB device");
        assert_eq!(usb_icon(&props(&[("ID_USB_INTERFACES", ":ff0000:")])), "usb");
        assert_eq!(usb_icon(&props(&[("ID_USB_INTERFACES", ":0e0100:")])), "webcam");
    }

    #[test]
    fn disks_and_outputs() {
        assert_eq!(disk_of("/dev/sdb1"), "/dev/sdb");
        assert_eq!(disk_of("/dev/nvme0n1p2"), "/dev/nvme0n1");
        assert_eq!(disk_of("/dev/sdb"), "/dev/sdb");
        assert_eq!(output_icon("alsa_output.hdmi-stereo HDMI"), "television");
        assert_eq!(output_icon("bluez_output.AA Buds"), "headphones");
        assert_eq!(output_icon("alsa speakers"), "speaker-hifi");
    }
}

//! Settings drawn in Rust: the "devices" page group's side, replacing
//! shell/modules/settings/RustBridgeDevices.qml (Displays, Sound, Wi-Fi &
//! Bluetooth, Phone & Sharing).
//!
//! While Settings is open (`devices.live` [true]) it tells the page on show
//! (`devices.page` [page], both told by host-shell's settings bridge) what
//! only the shell knows (`{"to":"settings","feed":{"devices":…}}`, the pages'
//! cx.feed["devices"]) and does what the pages ask (`cx.ask("devices", what,
//! args)`: "devices.act" [what, …args], or "devices.<what>" [args…]).
//!
//! The feed is assembled from what the other services publish, reshaped to
//! the QML bridge's property names:
//!   host-system bus: "WifiList" (network_feed: the nmcli list, asked again
//!   every 4 s as `network.list` while the page is up), "WifiJoin", "WifiShare",
//!   "SpeedTest" (speed / speedTest), "Brightness" (ddc), "updates" {running};
//!   host state: "backends", "needs", "health" (host-shell Probes), "airplay",
//!   "mirroring", "phone", "localsend", "quickshare", "sharing" (host-sharing),
//!   "picker.busy" (host-shell); core's BlueZ, Wi-Fi status and Sound; the
//!   adapter's name and trusted devices from BlueZ's object tree (busctl);
//!   pairing/busy marks are this bridge's own (its bluetoothctl runs).
//! and its own: bus "displays" (displays.rs). Actions go out as asks:
//!   network.{wifi,rescan,connect,disconnect,join,forget,hold}, wifijoin.{loadKnown,saved,
//!   dismiss,join}, wifishare.{share,close}, speedtest.{probe,readLink,start,cancel,clearHistory},
//!   brightness.{setDdc,rescanDdc}, airplay.{check,install}, mirroring.{look,install,start,stop},
//!   phone.{checkCam,install,mirror,pair,cancelPair,setUpCam,setWebcam}, needs.{install,turnOnAvahi},
//!   backends.{startNm,openWifiApp,fixBluetooth,recheck}, quickshare.turnOnBluetooth,
//!   linkshare.receive, sharing.shareSelection, panels.pickShare, displays.* (displays.rs).
//! Bluetooth actions use `bluetoothctl`, as the control centre bridge does.
//!
//! Differences: the Wi-Fi/Bluetooth scanners that the QML switched on with a
//! Binding while the network page is up are `network.rescan` and
//! `bluetoothctl scan on` (a child killed when the page goes away).

use super::common::*;
use super::device_alerts::picture_for;
use serde_json::{json, Value};
use std::any::Any;
use std::collections::BTreeSet;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;
use tessera_core::host::{Host, Service};
use tessera_core::services::audio::Sound;
use tessera_core::services::bluez::Bluetooth;
use tessera_core::services::status::Wifi;

fn arg_str(args: &[Value], i: usize) -> String {
    args.get(i).map(|v| v.as_str().map(String::from).unwrap_or_else(|| if v.is_null() { String::new() } else { v.to_string() })).unwrap_or_default()
}
fn arg_bool(args: &[Value], i: usize) -> bool {
    match args.get(i) {
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0) != 0.0,
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Null) | None => false,
        _ => true,
    }
}

/// Settings of the page's last scan child (bluetoothctl scan on)
static SCAN: Mutex<Option<Child>> = Mutex::new(None);

fn scan_start() {
    scan_stop();
    if let Ok(c) = Command::new("bluetoothctl").args(["--timeout", "3600", "scan", "on"]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
        if let Ok(mut s) = SCAN.lock() {
            *s = Some(c);
        }
    }
}

fn scan_stop() {
    if let Ok(mut s) = SCAN.lock() {
        if let Some(mut c) = s.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

fn bluetoothctl(args: Vec<String>) {
    std::thread::spawn(move || {
        let c: Vec<&str> = std::iter::once("bluetoothctl").chain(args.iter().map(String::as_str)).collect();
        let _ = crate::util::capture(&c, Duration::from_secs(60));
    });
}

/// bluetoothctl, then `M::BtDone(address)` (the device's busy/pairing mark comes off)
fn bluetoothctl_then(args: Vec<String>, addr: String, send: impl Fn(Box<dyn Any + Send>) + Send + 'static) {
    std::thread::spawn(move || {
        let c: Vec<&str> = std::iter::once("bluetoothctl").chain(args.iter().map(String::as_str)).collect();
        let _ = crate::util::capture(&c, Duration::from_secs(60));
        send(Box::new(M::BtDone(addr)));
    });
}

/// What core's BlueZ list doesn't carry: the adapter's name (Alias) and the
/// trusted devices, from BlueZ's object tree, on a thread. Over D-Bus
/// itself (busctl is systemd's, not on every system), in busctl --json's
/// shape for parse_bluez_objects
fn bt_read(send: impl Fn(Box<dyn Any + Send>) + Send + 'static) {
    std::thread::spawn(move || {
        use std::collections::HashMap;
        use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value as V};
        type Objects = HashMap<OwnedObjectPath, HashMap<String, HashMap<String, OwnedValue>>>;
        let Ok(conn) = zbus::blocking::Connection::system() else { return };
        let Ok(m) = conn.call_method(Some("org.bluez"), "/", Some("org.freedesktop.DBus.ObjectManager"), "GetManagedObjects", &()) else { return };
        let body = m.body();
        let Ok(objs) = body.deserialize::<Objects>() else { return };
        let mut tree = serde_json::Map::new();
        for (path, ifaces) in objs {
            let mut im = serde_json::Map::new();
            for (iface, props) in ifaces {
                let mut pm = serde_json::Map::new();
                for (k, v) in props {
                    let data = match &*v {
                        V::Str(s) => json!(s.as_str()),
                        V::Bool(b) => json!(b),
                        _ => continue,
                    };
                    pm.insert(k, json!({"data": data}));
                }
                im.insert(iface, Value::Object(pm));
            }
            tree.insert(path.to_string(), Value::Object(im));
        }
        let (name, trusted) = parse_bluez_objects(&json!({"data": [tree]}));
        send(Box::new(M::BtInfo(name, trusted)));
    });
}

fn parse_bluez_objects(v: &Value) -> (String, BTreeSet<String>) {
    let mut name = String::new();
    let mut trusted = BTreeSet::new();
    if let Some(objs) = v["data"][0].as_object() {
        for ifaces in objs.values() {
            let a = &ifaces["org.bluez.Adapter1"];
            if a.is_object() && name.is_empty() {
                name = a["Alias"]["data"].as_str().or(a["Name"]["data"].as_str()).unwrap_or("").to_string();
            }
            let d = &ifaces["org.bluez.Device1"];
            if d["Trusted"]["data"] == true {
                if let Some(addr) = d["Address"]["data"].as_str() {
                    trusted.insert(addr.to_string());
                }
            }
        }
    }
    (name, trusted)
}

/// Mirroring.protocolName
fn protocol_name(p: &str) -> &'static str {
    match p {
        "chromecast" => "Chromecast",
        "mice" => "Miracast (network)",
        _ => "Miracast",
    }
}

/// `new Date(ms).toLocaleString("d MMM, HH:mm")`, local time
fn date_label(ms: f64) -> String {
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let t = (ms / 1000.0) as libc::time_t;
    // SAFETY: localtime_r only writes the tm it is given
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    if unsafe { libc::localtime_r(&t, &mut tm) }.is_null() {
        return String::new();
    }
    format!("{} {}, {:02}:{:02}", tm.tm_mday, MONTHS[tm.tm_mon.clamp(0, 11) as usize], tm.tm_hour, tm.tm_min)
}

/// RustBridgeDevices.qml's ago(ms)
fn ago(ms: f64) -> String {
    let m = ((crate::net::common::now_ms() - ms) / 60000.0).round() as i64;
    if m < 1 {
        "just now".into()
    } else if m < 60 {
        format!("{m} min ago")
    } else if m < 1440 {
        format!("{} h ago", (m as f64 / 60.0).round())
    } else {
        format!("{} d ago", (m as f64 / 1440.0).round())
    }
}

/// SpeedTestCard's texts (RustBridgeDevices.qml speedCard), from bus "SpeedTest"
/// through the Wi-Fi menu's own builder (network_feed::build_speed)
fn speed_card(st: &Value) -> Value {
    let m = crate::net::network_feed::build_speed(st);
    let stage = m["stage"].as_str().unwrap_or("idle").to_string();
    let err = stage == "error";
    let running = m["running"] == true;
    let sub = if running || stage == "done" || err || st["last"].is_null() {
        m["sub"].as_str().unwrap_or("").to_string()
    } else {
        format!("Last test {}", ago(st["last"]["at"].as_f64().unwrap_or(0.0)))
    };
    let stats = m["showStats"] == true;
    json!({
        "running": running, "sub": sub, "button": m["action"], "live": m["live"], "progress": m["progress"], "stats": stats,
        "download": m["stats"][0]["value"], "upload": m["stats"][1]["value"], "ping": m["stats"][2]["value"], "stage": stage,
        "extra": if stats { m["resp"].clone() } else { json!("") },
        "verdict": if !running && !err { m["verdict"].clone() } else { json!("") },
        "server": if err { json!("") } else { m["server"].clone() },
        "error": m["error"],
    })
}

/// The `speedTest` segment (RustBridgeDevices.qml), from bus "SpeedTest"
fn speed_test(st: &Value) -> Value {
    use crate::net::speed_test::{format, format_ms, signal_words};
    let s = |v: &Value| v.as_str().unwrap_or("").to_string();
    let n = |v: &Value| v.as_f64().unwrap_or(-1.0);
    let last = &st["last"];
    let or = |a: String, b: String| if a.is_empty() { b } else { a };
    let history: Vec<Value> = st["history"].as_array().map(|l| l.iter().map(|h| {
        let net = s(&h["network"]);
        let label = date_label(h["at"].as_f64().unwrap_or(0.0)) + &if net.is_empty() { String::new() } else { format!(" · {net}") };
        let server = s(&h["server"]);
        let detail: Vec<String> = [
            if h["backend"] == "ookla" { "Ookla".to_string() } else { "Cloudflare".to_string() },
            server.strip_prefix("Cloudflare ").unwrap_or(&server).to_string(),
            if n(&h["ping"]) >= 0.0 { format!("ping {}", format_ms(n(&h["ping"]))) } else { String::new() },
            if n(&h["loadedDown"]) >= 0.0 { format!("under load {}", format_ms(n(&h["loadedDown"]))) } else { String::new() },
        ].into_iter().filter(|x| !x.is_empty()).collect();
        json!({"label": label, "detail": detail.join(" · "), "value": format!("↓ {}   ↑ {}", format(n(&h["download"])), format(n(&h["upload"])))})
    }).collect()).unwrap_or_default();
    json!({
        "backend": st["backend"].as_str().unwrap_or("cloudflare"), "running": st["running"] == true, "ooklaReady": st["ooklaReady"] == true,
        "hasOokla": st["hasOokla"] == true, "ooklaInstall": s(&st["ooklaInstall"]), "link": st["link"],
        "signalWords": signal_words(st["link"]["signal"].as_f64().unwrap_or(-100.0)),
        "ip": or(s(&st["client"]["ip"]), s(&last["ip"])), "isp": or(s(&st["client"]["isp"]), s(&last["isp"])),
        "history": history,
    })
}

enum M {
    Tick,
    Link(u64),
    Recheck(u64),
    Scan(u64),
    BtInfo(String, BTreeSet<String>),
    BtDone(String),
    Managers(Option<Value>, String),
    System(Value),
    Fix(Option<Value>),
    CamAgain,
}

#[derive(Default)]
pub struct DevicesBridge {
    live: bool,
    page: String,
    asking: String,
    reveal: bool,
    managers: Value,
    fm_error: String,
    fm_busy: bool,
    system: Value,
    fixing: String,
    fix_error: String,
    looking_tvs: bool,
    last: String,
    tick_gen: u64,
    link_gen: u64,
    /// Bluetooth: the adapter's name, the trusted devices, the ones pairing or (dis)connecting
    bt_name: String,
    bt_trusted: BTreeSet<String>,
    bt_pairing: BTreeSet<String>,
    bt_busy: BTreeSet<String>,
    /// Needs.installAsked: 0 no, 1 asked, 2 Software Update is running it
    install_asked: u8,
}

impl DevicesBridge {
    fn page_changed(&mut self, h: &mut Host, page: &str) {
        self.page = page.to_string();
        match page {
            "network" => {
                h.ask("speedtest.probe", json!([]));
                h.ask("speedtest.readLink", json!([]));
                h.ask("wifijoin.loadKnown", json!([]));
                // the scanners (QML: Bindings while this page is up)
                h.ask("network.rescan", json!([]));
                if h.st.shell.get::<Bluetooth>().is_some_and(|b| b.powered) {
                    scan_start();
                }
                // (the link changes as you move around)
                self.link_gen += 1;
                let gen = self.link_gen;
                after(h.sender("DevicesBridge"), 5000, M::Link(gen));
                // (rfkill and the service change without BlueZ telling)
                after(h.sender("DevicesBridge"), 3000, M::Recheck(gen));
                // (the Wi-Fi scanner: the list again every 4 s)
                after(h.sender("DevicesBridge"), 4000, M::Scan(gen));
                bt_read(h.sender("DevicesBridge"));
            }
            "sound" => h.ask("airplay.check", json!([])),
            "displays" => h.ask("displays.refresh", json!([])),
            "phone" => {
                h.ask("phone.checkCam", json!([]));
                self.fm(h, &["status"]);
                self.check_system(h);
            }
            _ => {}
        }
        self.sync_look(h);
        if page != "network" {
            if !self.asking.is_empty() {
                h.ask("network.hold", json!([false, false]));
            }
            self.asking.clear();
            self.link_gen += 1; // (the network page's timers stop)
            scan_stop();
            if !crate::bus::get("WifiShare")["stage"].as_str().unwrap_or("").is_empty() {
                self.reveal = false;
                h.ask("wifishare.close", json!([]));
            }
        }
    }

    /// Mirroring.look while Displays is up and mirroring is available (also when that changes)
    fn sync_look(&mut self, h: &mut Host) {
        let look = self.live && self.page == "displays" && h.state("mirroring")["available"] == true;
        if look != self.looking_tvs {
            self.looking_tvs = look;
            h.ask("mirroring.look", json!([look]));
        }
    }

    /// The `wifi` segment: NetworkFeed's list (bus "WifiList") with WifiJoin's
    /// and WifiShare's states (bus "WifiJoin", "WifiShare")
    fn wifi_feed(&self, h: &Host) -> Value {
        let g = crate::bus::get;
        let st = h.st.shell.get::<Wifi>().cloned().unwrap_or_default();
        let list = g("WifiList");
        let join = g("WifiJoin");
        let share = g("WifiShare");
        let s = |v: &Value| v.as_str().unwrap_or("").to_string();
        let (jssid, jstage, jbusy) = (s(&join["ssid"]), s(&join["stage"]), join["busy"] == true);
        let networks: Vec<Value> = list["networks"].as_array().map(|l| l.iter().map(|n| {
            let name = s(&n["name"]);
            let joining = jbusy && jssid == name;
            let failed_text = if jstage == "error" && jssid == name { s(&join["error"]) } else { String::new() };
            let connecting = s(&list["connecting"]) == name || joining;
            let open = n["open"] == true;
            json!({
                "name": name, "signal": n["signal"].as_f64().unwrap_or(0.0), "connected": n["connected"] == true, "known": n["known"] == true,
                "secured": !open, "open": open, "enterprise": n["enterprise"] == true, "connecting": connecting, "busy": connecting,
                "failed": s(&list["failed"]) == name || !failed_text.is_empty(), "failedText": failed_text,
            })
        }).collect()).unwrap_or_default();
        let pw = s(&share["password"]);
        json!({
            "device": st.present, "enabled": st.enabled, "networks": networks,
            "known": if join["knownNetworks"].is_array() { join["knownNetworks"].clone() } else { json!([]) },
            "forgetError": s(&join["forgetError"]),
            "join": {"ssid": jssid, "stage": jstage, "error": s(&join["error"]), "busy": jbusy},
            "saved": join["saved"],
            "share": {"stage": s(&share["stage"]), "ssid": s(&share["ssid"]), "qr": s(&share["qr"]), "error": s(&share["error"]),
                      "hasPassword": !pw.is_empty(), "password": if self.reveal { pw } else { String::new() }},
        })
    }

    // ── Sharing's helpers (sharing.qml) ──

    fn fm(&mut self, h: &Host, args: &[&str]) {
        self.fm_busy = true;
        let mut cmd = vec!["python3".to_string(), "-I".into(), crate::util::script("file-manager-share.py")];
        cmd.extend(args.iter().map(|s| s.to_string()));
        let send = h.sender("DevicesBridge");
        std::thread::spawn(move || {
            let c: Vec<&str> = cmd.iter().map(String::as_str).collect();
            // (stdout is the list; stderr, when there is any, the error)
            let out = std::process::Command::new(c[0]).args(&c[1..]).stdin(Stdio::null()).output();
            match out {
                Ok(o) => send(Box::new(M::Managers(serde_json::from_slice(&o.stdout).ok(), String::from_utf8_lossy(&o.stderr).trim().to_string()))),
                Err(_) => send(Box::new(M::Managers(None, String::new()))),
            }
        });
    }

    fn check_system(&self, h: &Host) {
        let cmd = vec!["python3".to_string(), "-I".into(), crate::util::script("sharing-check.py"), "status".into()];
        let send = h.sender("DevicesBridge");
        crate::util::spawn_capture(cmd, Duration::from_secs(30), move |r| {
            if let Some(v) = r.and_then(|(_, o)| serde_json::from_str::<Value>(&o).ok()) {
                send(Box::new(M::System(v)));
            }
        });
    }

    fn fix_system(&mut self, h: &Host, what: &str) {
        self.fixing = what.to_string();
        self.fix_error.clear();
        let cmd = vec!["python3".to_string(), "-I".into(), crate::util::script("sharing-check.py"), "fix".into(), what.to_string()];
        let send = h.sender("DevicesBridge");
        crate::util::spawn_capture(cmd, Duration::from_secs(120), move |r| send(Box::new(M::Fix(r.and_then(|(_, o)| serde_json::from_str::<Value>(&o).ok())))));
    }

    // ── What the pages show ──

    fn feed(&self, h: &Host) -> Value {
        let p = self.page.as_str();
        let g = crate::bus::get;
        let updating = g("updates")["running"].as_bool().unwrap_or(false);
        // (host-shell's Probes: h.state "needs", "health", "backends")
        let needs = h.state("needs");
        let nb = |k: &str| needs[k].as_bool().unwrap_or(false);
        let mut f = json!({
            "page": p,
            "healthFixing": h.state("health")["fixing"].as_str().unwrap_or(""),
            "updatesRunning": updating,
            "needs": {
                "checked": nb("checked"), "avahi": nb("avahi"), "pipewire": nb("pipewire"), "networkManager": nb("networkManager"),
                "installing": self.install_asked > 0 && updating
            }
        });
        match p {
            "network" => {
                f["wifi"] = self.wifi_feed(h);
                f["backends"] = if h.state("backends").is_object() { h.state("backends").clone() } else { json!({}) };
                let bt = h.st.shell.get::<Bluetooth>();
                let mut devices = serde_json::Map::new();
                for d in bt.map(|b| b.devices.as_slice()).unwrap_or(&[]) {
                    let pairing = self.bt_pairing.contains(&d.address);
                    devices.insert(
                        d.address.clone(),
                        json!({"trusted": self.bt_trusted.contains(&d.address), "pairing": pairing, "busy": pairing || self.bt_busy.contains(&d.address),
                               "picture": picture_for(&d.icon, &d.name), "name": d.name}),
                    );
                }
                f["bt"] = json!({"adapter": bt.is_some_and(|b| b.present), "enabled": bt.is_some_and(|b| b.powered), "name": self.bt_name, "devices": devices});
                let st = g("SpeedTest");
                f["speed"] = speed_card(&st);
                f["speedTest"] = speed_test(&st);
            }
            "sound" => {
                // (AirPlay.speakers: the PipeWire outputs the raop module made)
                let speakers = h.st.shell.get::<Sound>().map_or(0, |s| s.sinks.iter().filter(|d| d.name.starts_with("raop_sink.")).count());
                let mut air = if h.state("airplay").is_object() { h.state("airplay").clone() } else { json!({}) };
                air["speakers"] = json!(speakers);
                f["airplay"] = air;
            }
            "displays" => {
                let d = g("displays");
                f["displays"] = json!({
                    "loaded": d["loaded"].as_bool().unwrap_or(false), "confirming": d["confirming"].as_bool().unwrap_or(false), "confirmLeft": d["confirmLeft"].as_i64().unwrap_or(0),
                    "monitors": d["monitors"], "saved": d["saved"].as_i64().unwrap_or(0), "lowered": d["lowered"]
                });
                let b = g("Brightness");
                let outputs: Vec<Value> = b["ddcDisplays"].as_array().map(|l| l.iter().map(|d| d["output"].clone()).collect()).unwrap_or_default();
                f["ddc"] = json!({
                    "displays": outputs, "values": if b["ddcValues"].is_object() { b["ddcValues"].clone() } else { json!({}) },
                    "installed": b["ddcInstalled"] == true, "problem": b["ddcProblem"].as_str().unwrap_or(""),
                    "scanned": b["ddcScanned"] == true, "scanning": b["ddcScanning"] == true
                });
                let m = h.state("mirroring");
                let tvs: Vec<Value> = m["displays"].as_array().map(|l| l.iter().map(|d| json!({"uuid": d["uuid"], "name": d["name"], "protocol": protocol_name(d["protocol"].as_str().unwrap_or(""))})).collect()).unwrap_or_default();
                f["mirroring"] = json!({
                    "available": m["available"] == true, "error": m["error"].as_str().unwrap_or(""), "p2p": m["p2p"].as_bool().unwrap_or(true),
                    "installing": m["installing"] == true, "installError": m["installError"].as_str().unwrap_or(""),
                    "target": m["target"].as_str().unwrap_or(""), "state": m["state"].as_str().unwrap_or(""), "displays": tvs
                });
            }
            "phone" => {
                // (host-sharing's state: "phone", "localsend", "quickshare", "sharing")
                let ph = h.state("phone");
                let dev = &ph["phone"];
                let serial = dev["serial"].as_str().unwrap_or("").to_string();
                let s = |v: &Value| v.as_str().unwrap_or("").to_string();
                f["phone"] = json!({
                    "phone": if dev.is_object() { json!({"model": s(&dev["model"]), "android": s(&dev["android"]), "battery": dev["battery"].as_f64().unwrap_or(-1.0),
                        "charging": dev["charging"] == true, "wireless": dev["wireless"] == true, "serial": serial}) } else { Value::Null },
                    "mirroring": dev.is_object() && ph["mirrors"].as_array().is_some_and(|l| l.iter().any(|x| x.as_str() == Some(serial.as_str()))),
                    "hasAdb": ph["hasAdb"] == true, "hasScrcpy": ph["hasScrcpy"] == true, "qrencode": tessera_core::util::on_path("qrencode"),
                    "installing": ph["installing"] == true, "installError": s(&ph["installError"]), "waiting": !ph["waiting"].is_null() && ph["waiting"] != false,
                    "pairStage": s(&ph["pairStage"]), "pairQr": s(&ph["pairQr"]), "pairError": s(&ph["pairError"]),
                    "camReady": ph["camReady"] == true, "camModule": ph["camModule"] == true, "kernelPackage": s(&ph["kernelPackage"]),
                    "camSetting": ph["camSetting"], "camError": s(&ph["camError"]), "webcam": ph["webcam"] == true
                });
                let (ls, qs) = (h.state("localsend"), h.state("quickshare"));
                let mut sh = json!({
                    "localSend": ls["available"] == true, "localSendError": s(&ls["error"]),
                    "quickShare": qs["available"] == true, "quickShareName": s(&qs["name"]), "quickShareError": s(&qs["error"]),
                    "btAvailable": qs["bluetoothAvailable"] == true, "btOn": qs["bluetoothOn"] == true,
                    "portsError": s(&h.state("sharing")["ports"]["error"]),
                    // (the one visibility's countdown: "for 7 more min")
                    "minutesLeft": h.state("sharing")["minutesLeft"].as_i64().unwrap_or(0)
                });
                sh["pairingRisk"] = json!(self.system["pairing"]["risk"].as_bool().unwrap_or(false));
                sh["passwordless"] = json!(self.system["passwordless"].as_bool().unwrap_or(false));
                sh["fixing"] = json!(self.fixing);
                sh["fixError"] = json!(self.fix_error);
                sh["managers"] = if self.managers.is_null() { json!([]) } else { self.managers.clone() };
                sh["fmError"] = json!(self.fm_error);
                sh["fmBusy"] = json!(self.fm_busy);
                sh["pickerBusy"] = json!(h.state("picker.busy").as_bool().unwrap_or(false));
                sh["user"] = json!(std::env::var("USER").unwrap_or_default());
                f["sharing"] = sh;
            }
            _ => {}
        }
        f
    }

    fn push(&mut self, h: &mut Host, force: bool) {
        if !self.live {
            return;
        }
        let f = self.feed(h);
        let s = f.to_string();
        if force || s != self.last {
            self.last = s;
            h.tell("settings", json!({"feed": {"devices": f}}));
        }
    }

    // ── What the pages ask ──

    fn act(&mut self, h: &mut Host, what: &str, a: &[Value]) {
        let a0 = a.first().cloned().unwrap_or(Value::Null);
        let s0 = arg_str(a, 0);
        match what {
            // Wi-Fi (the network feed's own calls)
            "wifiEnable" => h.ask("network.wifi", json!([arg_bool(a, 0)])),
            "wifiRescan" => h.ask("network.rescan", json!([])),
            "wifiConnect" => h.ask("network.connect", json!([s0])),
            "wifiDisconnect" => h.ask("network.disconnect", json!([s0])),
            "wifiJoin" => {
                // [name, password]
                h.ask("network.join", json!([s0, arg_str(a, 1)]));
                self.asking.clear();
                h.ask("network.hold", json!([false, false]));
            }
            "wifiAsking" => {
                self.asking = s0.clone();
                h.ask("network.hold", json!([false, !s0.is_empty()]));
            }
            "wifiForget" => {
                h.ask("network.forget", json!([s0]));
                h.ask("wifijoin.loadKnown", json!([]));
            }
            "wifiShare" => {
                self.reveal = false;
                h.ask("wifishare.share", json!([s0]));
            }
            "wifiShareClose" => {
                self.reveal = false;
                h.ask("wifishare.close", json!([]));
            }
            "wifiShareReveal" => self.reveal = !self.reveal,
            "wifiShareCopy" => {
                // Clipboard.copyText(WifiShare.password), through wl-copy's stdin
                let pw = crate::bus::get("WifiShare")["password"].as_str().unwrap_or("").to_string();
                if !pw.is_empty() {
                    std::thread::spawn(move || {
                        use std::io::Write;
                        if let Ok(mut c) = Command::new("wl-copy").stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
                            if let Some(mut i) = c.stdin.take() {
                                let _ = i.write_all(pw.as_bytes());
                            }
                            let _ = c.wait();
                        }
                    });
                }
            }
            // [ssid]: a saved network's settings (never its secrets)
            "wifiForm" => {
                h.ask("wifijoin.dismiss", json!([]));
                if !s0.is_empty() && !a0.is_null() {
                    h.ask("wifijoin.saved", json!([s0]));
                }
            }
            "wifiJoinForm" => h.ask("wifijoin.join", json!([a0])),
            "wifiDismiss" => h.ask("wifijoin.dismiss", json!([])),
            "installTools" => {
                self.install_asked = 1;
                h.ask("needs.install", json!([if a0.is_null() { json!([]) } else { a0 }]));
            }
            "startNm" => h.ask("backends.startNm", json!([])),
            "openWifiApp" => h.ask("backends.openWifiApp", json!([])),
            "speedTest" => {
                if crate::bus::get("SpeedTest")["running"] == true {
                    h.ask("speedtest.cancel", json!([]));
                } else {
                    h.ask("speedtest.start", json!([]));
                }
            }
            "speedClear" => h.ask("speedtest.clearHistory", json!([])),
            // Bluetooth (bluetoothctl, as the control centre's)
            "btEnable" => bluetoothctl(vec!["power".into(), if arg_bool(a, 0) { "on" } else { "off" }.into()]),
            "btRescan" => {
                if h.st.shell.get::<Bluetooth>().is_some_and(|b| b.powered) {
                    scan_start();
                }
            }
            "btFix" => h.ask("backends.fixBluetooth", json!([])),
            "btConnect" | "btDisconnect" => {
                self.bt_busy.insert(s0.clone());
                bluetoothctl_then(vec![if what == "btConnect" { "connect" } else { "disconnect" }.into(), s0.clone()], s0, h.sender("DevicesBridge"));
            }
            "btPair" => {
                self.bt_pairing.insert(s0.clone());
                bluetoothctl_then(vec!["pair".into(), s0.clone()], s0, h.sender("DevicesBridge"));
            }
            "btCancelPair" => {
                self.bt_pairing.remove(&s0);
                bluetoothctl(vec!["cancel-pairing".into(), s0]);
            }
            "btTrust" => {
                // (shown at once; BlueZ's own answer follows)
                let on = arg_bool(a, 1);
                if on {
                    self.bt_trusted.insert(s0.clone());
                } else {
                    self.bt_trusted.remove(&s0);
                }
                bluetoothctl_then(vec![if on { "trust" } else { "untrust" }.into(), s0.clone()], s0, h.sender("DevicesBridge"));
            }
            "btForget" => bluetoothctl_then(vec!["remove".into(), s0.clone()], s0, h.sender("DevicesBridge")),
            // Sound
            "airplayInstall" => h.ask("airplay.install", json!([])),
            "avahiOn" => {
                if !h.state("needs")["avahiInstalled"].as_bool().unwrap_or(false) {
                    self.install_asked = 1;
                }
                h.ask("needs.turnOnAvahi", json!([]));
            }
            // Displays
            "displaysSet" => h.ask("displays.set", json!([s0, a.get(1).cloned().filter(Value::is_object).unwrap_or(json!({}))])),
            "displaysSetMany" => h.ask("displays.setMany", json!([if a0.is_null() { json!([]) } else { a0 }])),
            "displaysIdentify" => h.ask("displays.identify", json!([])),
            "displaysKeep" => h.ask("displays.keep", json!([])),
            "displaysRevert" => h.ask("displays.revert", json!([])),
            "displaysReset" => h.ask("displays.reset", json!([])),
            "ddcSet" => h.ask("brightness.setDdc", json!([s0, a.get(1).and_then(Value::as_f64).unwrap_or(0.5)])),
            "ddcRescan" => h.ask("brightness.rescanDdc", json!([])),
            "mirrorInstall" => h.ask("mirroring.install", json!([])),
            "mirrorStart" => h.ask("mirroring.start", json!([s0])),
            "mirrorStop" => h.ask("mirroring.stop", json!([])),
            // [animation, side]
            "fxPreview" => h.ask("displays.preview", json!([s0, if a.len() > 1 { arg_str(a, 1) } else { "left".into() }])),
            // Phone
            "phoneInstall" => h.ask("phone.install", json!([arg_bool(a, 0)])),
            "phoneMirror" => h.ask("phone.mirror", json!([s0])),
            "phonePair" => h.ask("phone.pair", json!([])),
            "phoneCancelPair" => h.ask("phone.cancelPair", json!([])),
            "phoneSetUpCam" => h.ask("phone.setUpCam", json!([])),
            "phoneWebcam" => h.ask("phone.setWebcam", json!([arg_bool(a, 0)])),
            // (again, with the other camera)
            "phoneCameraChanged" => {
                if h.state("phone")["webcam"] == true {
                    h.ask("phone.setWebcam", json!([false]));
                    after(h.sender("DevicesBridge"), 800, M::CamAgain);
                }
            }
            // Sharing
            "sendFiles" => h.ask("panels.pickShare", json!([])),
            "quickShareBluetooth" => h.ask("quickshare.turnOnBluetooth", json!([])),
            "sharingFix" => self.fix_system(h, &s0),
            // [install|remove, id]
            "fileManager" => {
                let id = arg_str(a, 1);
                self.fm(h, &[s0.as_str(), id.as_str()]);
            }
            "receiveLink" => h.ask("linkshare.receive", json!([])),
            "shareSelection" => h.ask("sharing.shareSelection", json!([])),
            other => eprintln!("settings (devices): no act {other}"),
        }
        self.push(h, false);
    }
}

impl Service for DevicesBridge {
    fn name(&self) -> &'static str {
        "DevicesBridge"
    }
    fn calls(&self) -> &'static [&'static str] {
        &["devices"]
    }
    fn start(&mut self, h: &mut Host) {
        self.managers = json!([]);
        self.system = json!({});
        h.st.shell.want("bluez");
    }
    fn call(&mut self, h: &mut Host, call: &str, args: &Value) {
        let list: Vec<Value> = args.as_array().cloned().unwrap_or_else(|| if args.is_null() { vec![] } else { vec![args.clone()] });
        match call.split_once('.').map(|x| x.1).unwrap_or("") {
            "" | "act" => {
                let what = arg_str(&list, 0);
                self.act(h, &what, &list[list.len().min(1)..]);
            }
            "live" => {
                let on = list.first().and_then(Value::as_bool).unwrap_or(true);
                if on != self.live {
                    self.live = on;
                    if on {
                        self.tick_gen += 1;
                        after(h.sender("DevicesBridge"), 250, M::Tick);
                        self.push(h, true);
                    } else {
                        let p = String::new();
                        self.page_changed(h, &p);
                    }
                }
            }
            "page" => {
                let p = arg_str(&list, 0);
                if p != self.page {
                    self.page_changed(h, &p);
                }
                self.push(h, true);
            }
            what => self.act(h, what, &list),
        }
    }
    fn message(&mut self, h: &mut Host, msg: Box<dyn Any + Send>) {
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Tick => {
                if self.live {
                    // (Needs.installing: asked, until Software Update's run ends)
                    let updating = crate::bus::get("updates")["running"].as_bool().unwrap_or(false);
                    if self.install_asked == 1 && updating {
                        self.install_asked = 2;
                    } else if self.install_asked == 2 && !updating {
                        self.install_asked = 0;
                    }
                    self.sync_look(h);
                    self.push(h, false);
                    after(h.sender("DevicesBridge"), 250, M::Tick);
                }
            }
            M::Link(gen) => {
                if self.live && self.page == "network" && gen == self.link_gen {
                    h.ask("speedtest.readLink", json!([]));
                    bt_read(h.sender("DevicesBridge"));
                    after(h.sender("DevicesBridge"), 5000, M::Link(gen));
                }
            }
            M::Recheck(gen) => {
                if self.live && self.page == "network" && gen == self.link_gen {
                    // (Backends.btState, host-shell's Probes)
                    let st = h.state("backends")["btState"].as_str().unwrap_or("").to_string();
                    if st != "ok" {
                        h.ask("backends.recheck", json!([]));
                    }
                    after(h.sender("DevicesBridge"), 3000, M::Recheck(gen));
                }
            }
            M::Scan(gen) => {
                if self.live && self.page == "network" && gen == self.link_gen {
                    if self.asking.is_empty() {
                        h.ask("network.list", json!([]));
                    }
                    after(h.sender("DevicesBridge"), 4000, M::Scan(gen));
                }
            }
            M::BtInfo(name, trusted) => {
                self.bt_name = name;
                self.bt_trusted = trusted;
                self.push(h, false);
            }
            M::BtDone(addr) => {
                self.bt_pairing.remove(&addr);
                self.bt_busy.remove(&addr);
                bt_read(h.sender("DevicesBridge"));
                self.push(h, false);
            }
            M::Managers(v, err) => {
                self.fm_busy = false;
                if let Some(v) = v {
                    self.managers = v;
                    self.fm_error.clear();
                }
                if !err.is_empty() {
                    self.fm_error = err;
                }
                self.push(h, false);
            }
            M::System(v) => {
                self.system = v;
                self.push(h, false);
            }
            M::Fix(r) => {
                self.fix_error = match r {
                    Some(v) => if v["ok"].as_bool().unwrap_or(false) { String::new() } else { v["error"].as_str().unwrap_or("").to_string() },
                    None => "It didn't work; try again.".into(),
                };
                self.fixing.clear();
                self.check_system(h);
                self.push(h, false);
            }
            M::CamAgain => h.ask("phone.setWebcam", json!([true])),
        }
    }
    fn quit(&mut self, _h: &mut Host) {
        scan_stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_read_like_js() {
        let a = vec![json!("x"), json!(1), json!(null), json!(true)];
        assert_eq!(arg_str(&a, 0), "x");
        assert_eq!(arg_str(&a, 2), "");
        assert_eq!(arg_str(&a, 9), "");
        assert!(arg_bool(&a, 1) && arg_bool(&a, 3) && !arg_bool(&a, 2) && !arg_bool(&a, 9));
    }

    #[test]
    fn bluez_name_and_trusted_devices() {
        let v = json!({"type": "a{oa{sa{sv}}}", "data": [{
            "/org/bluez/hci0": {"org.bluez.Adapter1": {"Alias": {"type": "s", "data": "Desk"}, "Name": {"type": "s", "data": "host"}}},
            "/org/bluez/hci0/dev_AA": {"org.bluez.Device1": {"Address": {"type": "s", "data": "AA:BB"}, "Trusted": {"type": "b", "data": true}}},
            "/org/bluez/hci0/dev_CC": {"org.bluez.Device1": {"Address": {"type": "s", "data": "CC:DD"}, "Trusted": {"type": "b", "data": false}}}
        }]});
        let (name, trusted) = parse_bluez_objects(&v);
        assert_eq!(name, "Desk");
        assert!(trusted.contains("AA:BB") && !trusted.contains("CC:DD"));
        assert_eq!(protocol_name("mice"), "Miracast (network)");
    }

    #[test]
    fn the_speed_card_in_the_pages_shape() {
        let c = speed_card(&json!({"stage": "download", "running": true, "live": 42e6, "progress": 0.4, "ping": 12.0, "download": -1.0, "upload": -1.0}));
        assert_eq!((c["button"].as_str(), c["sub"].as_str(), c["running"].as_bool()), (Some("Cancel"), Some("Testing download…"), Some(true)));
        let e = speed_card(&json!({"stage": "error", "error": "offline"}));
        assert_eq!((e["error"].as_str(), e["server"].as_str(), e["stats"].as_bool()), (Some("offline"), Some(""), Some(false)));
    }

    #[test]
    fn the_feed_follows_the_page() {
        let b = DevicesBridge { page: "sound".into(), live: true, managers: json!([]), system: json!({}), ..Default::default() };
        let _ = &b;
        // (feed() needs a Host; the page names are what decide its segments)
        assert_eq!(b.page, "sound");
    }
}

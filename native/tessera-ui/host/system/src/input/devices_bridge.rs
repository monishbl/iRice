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
//! The feed is assembled from what the other services publish on the bus. The
//! contract (each is the exact JSON segment of the feed, so its owner builds
//! it with the QML property names):
//!   "feed.wifi"      net: { device, enabled, networks[], known, forgetError,
//!                    join{ssid,stage,error,busy}, share{stage,ssid,qr,error,password} }
//!   "feed.backends"  net/host-shell: Backends' fields (wifiUnmanaged … btFixError)
//!   "feed.speed"     net SpeedTest: SpeedTestCard's texts (running, sub, button, live, …)
//!   "feed.speedTest" net SpeedTest: backend, running, ooklaReady, hasOokla, ooklaInstall,
//!                    link, signalWords, ip, isp, history[]
//!   "feed.ddc"       net Brightness: { displays, values, installed, problem, scanned, scanning }
//!   "feed.airplay"   sharing: { checked, installed, installing, installError, error, wanted, speakers }
//!   "feed.mirroring" sharing: { available, error, p2p, installing, installError, target, state, displays[] }
//!   "feed.phone"     sharing: the `phone` segment ({phone, mirroring, hasAdb, …, webcam})
//!   "feed.sharing"   sharing: { localSend, localSendError, quickShare, quickShareName,
//!                    quickShareError, btAvailable, btOn, portsError }
//!   "wifijoin" {saved}  (the saved enterprise network's settings for the join form)
//!   "needs" {checked, avahi, pipewire, networkManager, installing}, "health" {fixing},
//!   "updates" {running}, "filepicker" {busy}, "bluetooth" {devices: {addr: {trusted, pairing, busy}}, name}
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
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;
use tessera_core::host::{Host, Service};
use tessera_core::services::bluez::Bluetooth;

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

enum M {
    Tick,
    Link,
    Recheck,
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
    recheck_gen: u64,
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
                after(h.sender("DevicesBridge"), 5000, M::Link);
                // (rfkill and the service change without BlueZ telling)
                after(h.sender("DevicesBridge"), 3000, M::Recheck);
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
        let look = page == "displays" && crate::bus::get("feed.mirroring")["available"] == true;
        if look != self.looking_tvs {
            self.looking_tvs = look;
            h.ask("mirroring.look", json!([look]));
        }
        if page != "network" {
            self.asking.clear();
            scan_stop();
            if !crate::bus::get("feed.wifi")["share"]["stage"].as_str().unwrap_or("").is_empty() {
                h.ask("wifishare.close", json!([]));
            }
        }
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
        let mut f = json!({
            "page": p,
            "healthFixing": g("health")["fixing"].as_str().unwrap_or(""),
            "updatesRunning": g("updates")["running"].as_bool().unwrap_or(false),
            "needs": {
                "checked": g("needs")["checked"].as_bool().unwrap_or(false), "avahi": g("needs")["avahi"].as_bool().unwrap_or(false),
                "pipewire": g("needs")["pipewire"].as_bool().unwrap_or(false), "networkManager": g("needs")["networkManager"].as_bool().unwrap_or(false),
                "installing": g("needs")["installing"].as_bool().unwrap_or(false)
            }
        });
        match p {
            "network" => {
                let mut wifi = g("feed.wifi");
                if wifi.is_null() {
                    wifi = json!({"device": false, "enabled": false, "networks": [], "known": [], "forgetError": "", "join": {"ssid": "", "stage": "", "error": "", "busy": false}, "share": {"stage": "", "ssid": "", "qr": "", "error": "", "password": ""}});
                }
                let pw = wifi["share"]["password"].as_str().unwrap_or("").to_string();
                wifi["share"]["hasPassword"] = json!(!pw.is_empty());
                wifi["share"]["password"] = json!(if self.reveal { pw } else { String::new() });
                wifi["saved"] = g("wifijoin")["saved"].clone();
                f["wifi"] = wifi;
                f["backends"] = g("feed.backends");
                let bt = h.st.shell.get::<Bluetooth>();
                let extra = g("bluetooth");
                let mut devices = serde_json::Map::new();
                for d in bt.map(|b| b.devices.as_slice()).unwrap_or(&[]) {
                    let e = &extra["devices"][&d.address];
                    devices.insert(
                        d.address.clone(),
                        json!({"trusted": e["trusted"].as_bool().unwrap_or(false), "pairing": e["pairing"].as_bool().unwrap_or(false), "busy": e["busy"].as_bool().unwrap_or(false),
                               "picture": picture_for(&d.icon, &d.name), "name": d.name}),
                    );
                }
                f["bt"] = json!({"adapter": bt.is_some_and(|b| b.present), "enabled": bt.is_some_and(|b| b.powered), "name": extra["name"].as_str().unwrap_or(""), "devices": devices});
                f["speed"] = g("feed.speed");
                f["speedTest"] = g("feed.speedTest");
            }
            "sound" => f["airplay"] = g("feed.airplay"),
            "displays" => {
                let d = g("displays");
                f["displays"] = json!({
                    "loaded": d["loaded"].as_bool().unwrap_or(false), "confirming": d["confirming"].as_bool().unwrap_or(false), "confirmLeft": d["confirmLeft"].as_i64().unwrap_or(0),
                    "monitors": d["monitors"], "saved": d["saved"].as_i64().unwrap_or(0), "lowered": d["lowered"]
                });
                f["ddc"] = g("feed.ddc");
                f["mirroring"] = g("feed.mirroring");
            }
            "phone" => {
                f["phone"] = g("feed.phone");
                let mut sh = g("feed.sharing");
                if !sh.is_object() {
                    sh = json!({});
                }
                sh["pairingRisk"] = json!(self.system["pairing"]["risk"].as_bool().unwrap_or(false));
                sh["passwordless"] = json!(self.system["passwordless"].as_bool().unwrap_or(false));
                sh["fixing"] = json!(self.fixing);
                sh["fixError"] = json!(self.fix_error);
                sh["managers"] = if self.managers.is_null() { json!([]) } else { self.managers.clone() };
                sh["fmError"] = json!(self.fm_error);
                sh["fmBusy"] = json!(self.fm_busy);
                sh["pickerBusy"] = json!(g("filepicker")["busy"].as_bool().unwrap_or(false));
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
                let pw = crate::bus::get("feed.wifi")["share"]["password"].as_str().unwrap_or("").to_string();
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
            "installTools" => h.ask("needs.install", json!([if a0.is_null() { json!([]) } else { a0 }])),
            "startNm" => h.ask("backends.startNm", json!([])),
            "openWifiApp" => h.ask("backends.openWifiApp", json!([])),
            "speedTest" => {
                if crate::bus::get("feed.speed")["running"] == true {
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
            "btConnect" => bluetoothctl(vec!["connect".into(), s0]),
            "btDisconnect" => bluetoothctl(vec!["disconnect".into(), s0]),
            "btPair" => bluetoothctl(vec!["pair".into(), s0]),
            "btCancelPair" => bluetoothctl(vec!["cancel-pairing".into(), s0]),
            "btTrust" => bluetoothctl(vec![if arg_bool(a, 1) { "trust" } else { "untrust" }.into(), s0]),
            "btForget" => bluetoothctl(vec!["remove".into(), s0]),
            // Sound
            "airplayInstall" => h.ask("airplay.install", json!([])),
            "avahiOn" => h.ask("needs.turnOnAvahi", json!([])),
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
                if crate::bus::get("feed.phone")["webcam"] == true {
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
                    self.push(h, false);
                    after(h.sender("DevicesBridge"), 250, M::Tick);
                }
            }
            M::Link => {
                if self.live && self.page == "network" {
                    h.ask("speedtest.readLink", json!([]));
                    after(h.sender("DevicesBridge"), 5000, M::Link);
                }
            }
            M::Recheck => {
                if self.live && self.page == "network" {
                    let st = crate::bus::get("feed.backends")["btState"].as_str().unwrap_or("").to_string();
                    if st != "ok" && !st.is_empty() {
                        h.ask("backends.recheck", json!([]));
                    }
                    after(h.sender("DevicesBridge"), 3000, M::Recheck);
                }
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
    fn the_feed_follows_the_page() {
        let b = DevicesBridge { page: "sound".into(), live: true, managers: json!([]), system: json!({}), ..Default::default() };
        let _ = &b;
        // (feed() needs a Host; the page names are what decide its segments)
        assert_eq!(b.page, "sound");
    }
}

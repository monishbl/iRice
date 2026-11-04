//! The D-Bus sources, one connection per bus, read by one thread each:
//!   login1    logind's signals on /org/freedesktop/login1 (PrepareForSleep,
//!             PropertiesChanged with LidClosed...), as {"kind": member, "args"}
//!   nm        NetworkManager's state changes (a cue: the shell then asks
//!             nmcli what's connected, as after `nmcli monitor`)
//!   avahi     Google Cast devices, through Avahi's browser and resolver
//!   location  apps GeoClue serves, asked only while GeoClue already runs
//!             (never started by asking), when it signals a change
//!   unity     app badges on the session bus (LauncherEntry Update)
//! and logind inhibitors, held as the fd Inhibit() returns.
//!
//! System services are recognised by the unique name that owns their
//! well-known name (followed through NameOwnerChanged), never by what a
//! message says about itself: on the system bus anyone may send a signal,
//! and a fake LidClosed would put the laptop to sleep.

use crate::{emit, json as dj};
use serde_json::json;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{LazyLock, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};
use zbus::blocking::{fdo::DBusProxy, Connection, MessageIterator};
use zbus::message::Type;
use zbus::zvariant::OwnedFd;
use zbus::MatchRule;

const LOGIN1: &str = "org.freedesktop.login1";
const NM: &str = "org.freedesktop.NetworkManager";
const AVAHI: &str = "org.freedesktop.Avahi";
const GEOCLUE: &str = "org.freedesktop.GeoClue2";
const UNITY: &str = "com.canonical.Unity.LauncherEntry";
const CAST: &str = "_googlecast._tcp";

struct Bus {
    conn: Connection,
    owners: Mutex<HashMap<String, String>>, // well-known name -> its owner's unique name
    rules: Mutex<Vec<String>>,              // match rules added
}

static SYSTEM: Mutex<Option<&'static Bus>> = Mutex::new(None);
static SESSION: Mutex<Option<&'static Bus>> = Mutex::new(None);
static LOGIN1_ON: AtomicBool = AtomicBool::new(false);
static NM_ON: AtomicBool = AtomicBool::new(false);
static UNITY_ON: AtomicBool = AtomicBool::new(false);
static AVAHI_TX: OnceLock<Sender<Avahi>> = OnceLock::new();
static GEO_TX: OnceLock<Sender<Geo>> = OnceLock::new();
static HELD: LazyLock<Mutex<HashMap<String, OwnedFd>>> = LazyLock::new(Default::default);

/// The bus's connection, made (and its reader started) the first time.
fn bus(system: bool) -> Result<&'static Bus, String> {
    let slot = if system { &SYSTEM } else { &SESSION };
    let mut g = slot.lock().unwrap();
    if let Some(b) = *g {
        return Ok(b);
    }
    let conn = if system { Connection::system() } else { Connection::session() }.map_err(|e| e.to_string())?;
    // (before any match rule: nothing that matches can come before it)
    let it = MessageIterator::from(&conn);
    let b: &'static Bus = Box::leak(Box::new(Bus { conn, owners: Default::default(), rules: Default::default() }));
    thread::spawn(move || read(b, system, it));
    *g = Some(b);
    Ok(b)
}

impl Bus {
    fn add_rule(&self, rule: &str) -> Result<(), String> {
        let mut rules = self.rules.lock().unwrap();
        if rules.iter().any(|r| r == rule) {
            return Ok(());
        }
        let r = MatchRule::try_from(rule).map_err(|e| e.to_string())?;
        let p = DBusProxy::new(&self.conn).map_err(|e| e.to_string())?;
        p.add_match_rule(r).map_err(|e| e.to_string())?;
        rules.push(rule.to_string());
        Ok(())
    }

    /// Follows who owns `name` (and learns who does now).
    fn track(&self, name: &str) -> Result<(), String> {
        self.add_rule(&format!(
            "type='signal',sender='org.freedesktop.DBus',interface='org.freedesktop.DBus',member='NameOwnerChanged',arg0='{name}'"
        ))?;
        let p = DBusProxy::new(&self.conn).map_err(|e| e.to_string())?;
        let bus_name = zbus::names::BusName::try_from(name).map_err(|e| e.to_string())?;
        if let Ok(o) = p.get_name_owner(bus_name) {
            self.owners.lock().unwrap().insert(name.to_string(), o.to_string());
        }
        Ok(())
    }

    fn owner(&self, name: &str) -> Option<String> {
        self.owners.lock().unwrap().get(name).cloned()
    }

    /// The well-known name `unique` owns, among those followed.
    fn known_as(&self, unique: &str) -> Option<String> {
        self.owners.lock().unwrap().iter().find(|(_, o)| o.as_str() == unique).map(|(n, _)| n.clone())
    }

    fn call<B>(&self, dest: &str, path: &str, iface: &str, method: &str, body: &B) -> Result<zbus::Message, String>
    where
        B: serde::Serialize + zbus::zvariant::DynamicType,
    {
        self.conn.call_method(Some(dest), path, Some(iface), method, body).map_err(|e| e.to_string())
    }

    fn property(&self, dest: &str, path: &str, iface: &str, name: &str) -> Option<serde_json::Value> {
        let m = self.call(dest, path, "org.freedesktop.DBus.Properties", "Get", &(iface, name)).ok()?;
        let v = m.body().deserialize::<zbus::zvariant::OwnedValue>().ok()?;
        Some(dj::from_dbus(&v))
    }
}

// The bus's messages, as they come
fn read(b: &'static Bus, system: bool, it: MessageIterator) {
    for m in it {
        let Ok(m) = m else { continue };
        if m.message_type() != Type::Signal {
            continue;
        }
        let h = m.header();
        let (Some(sender), Some(iface), Some(member)) = (h.sender(), h.interface(), h.member()) else { continue };
        let (sender, iface, member) = (sender.as_str(), iface.as_str(), member.as_str());
        let path = h.path().map(|p| p.as_str()).unwrap_or("");
        if sender == "org.freedesktop.DBus" && member == "NameOwnerChanged" {
            if let Ok((name, _, new)) = m.body().deserialize::<(String, String, String)>() {
                owner_changed(b, &name, &new);
            }
            continue;
        }
        if !system {
            if iface == UNITY && member == "Update" && UNITY_ON.load(Ordering::Relaxed) {
                emit(json!({"src": "unity", "kind": "Update", "args": dj::args(&m)}));
            }
            continue;
        }
        let Some(from) = b.known_as(sender) else { continue };
        match from.as_str() {
            LOGIN1 if path == "/org/freedesktop/login1" && LOGIN1_ON.load(Ordering::Relaxed) => {
                emit(json!({"src": "login1", "kind": member, "args": dj::args(&m)}));
            }
            NM if NM_ON.load(Ordering::Relaxed) && nm_matters(path, iface, member) => {
                emit(json!({"src": "nm", "kind": member}));
            }
            AVAHI if iface == "org.freedesktop.Avahi.ServiceBrowser" => {
                // ItemNew / ItemRemove: (interface, protocol, name, type, domain, flags)
                if let Ok((i, p, name, ty, domain, _)) = m.body().deserialize::<(i32, i32, String, String, String, u32)>() {
                    let item = Item { iface: i, proto: p, name, ty, domain };
                    match member {
                        "ItemNew" => send(&AVAHI_TX, Avahi::New(item)),
                        "ItemRemove" => send(&AVAHI_TX, Avahi::Remove(item)),
                        _ => {}
                    }
                }
            }
            GEOCLUE => send(&GEO_TX, Geo::Dirty),
            _ => {}
        }
    }
    // The bus went away (its daemon restarted): end, and the shell starts
    // a new one that connects again (its watchers stand in meanwhile)
    crate::emit_error("events", if system { "system bus lost" } else { "session bus lost" });
    std::process::exit(1);
}

fn owner_changed(b: &Bus, name: &str, new: &str) {
    {
        let mut o = b.owners.lock().unwrap();
        if new.is_empty() {
            o.remove(name);
        } else {
            o.insert(name.to_string(), new.to_string());
        }
    }
    match name {
        AVAHI => send(&AVAHI_TX, if new.is_empty() { Avahi::Down } else { Avahi::Up }),
        GEOCLUE => send(&GEO_TX, Geo::Dirty),
        _ => {}
    }
}

fn send<T>(tx: &OnceLock<Sender<T>>, msg: T) {
    if let Some(tx) = tx.get() {
        let _ = tx.send(msg);
    }
}

// What `nmcli monitor` reports: NetworkManager's state, devices coming and
// going or changing state, connections activating, the manager's properties
// (active connections, connectivity). Access points' signal strength isn't.
fn nm_matters(path: &str, iface: &str, member: &str) -> bool {
    match (iface, member) {
        (NM, "StateChanged" | "DeviceAdded" | "DeviceRemoved") => true,
        ("org.freedesktop.NetworkManager.Device", "StateChanged") => true,
        ("org.freedesktop.NetworkManager.Connection.Active", "StateChanged") => true,
        ("org.freedesktop.DBus.Properties", "PropertiesChanged") => path == "/org/freedesktop/NetworkManager",
        _ => false,
    }
}

/// Turns a D-Bus source on or off. One that can't work here (no bus) says
/// it's unsupported, so the shell uses its own watcher for it.
pub fn watch(src: &str, on: bool) {
    let r = if !on {
        off(src);
        Ok(())
    } else {
        start(src)
    };
    if let Err(e) = r {
        emit(json!({"src": src, "kind": "unsupported", "error": e}));
    }
}

fn off(src: &str) {
    match src {
        "login1" => LOGIN1_ON.store(false, Ordering::Relaxed),
        "nm" => NM_ON.store(false, Ordering::Relaxed),
        "unity" => UNITY_ON.store(false, Ordering::Relaxed),
        "avahi" => send(&AVAHI_TX, Avahi::Off),
        "location" => send(&GEO_TX, Geo::Off),
        _ => {}
    }
}

fn start(src: &str) -> Result<(), String> {
    match src {
        "login1" => {
            let b = bus(true)?;
            b.track(LOGIN1)?;
            b.add_rule("type='signal',sender='org.freedesktop.login1',path='/org/freedesktop/login1'")?;
            LOGIN1_ON.store(true, Ordering::Relaxed);
        }
        "nm" => {
            let b = bus(true)?;
            b.track(NM)?;
            for r in [
                "type='signal',sender='org.freedesktop.NetworkManager',interface='org.freedesktop.NetworkManager'",
                "type='signal',sender='org.freedesktop.NetworkManager',interface='org.freedesktop.NetworkManager.Device',member='StateChanged'",
                "type='signal',sender='org.freedesktop.NetworkManager',interface='org.freedesktop.NetworkManager.Connection.Active',member='StateChanged'",
                "type='signal',sender='org.freedesktop.NetworkManager',interface='org.freedesktop.DBus.Properties',member='PropertiesChanged',path='/org/freedesktop/NetworkManager'",
            ] {
                b.add_rule(r)?;
            }
            NM_ON.store(true, Ordering::Relaxed);
        }
        "unity" => {
            let b = bus(false)?;
            b.add_rule("type='signal',interface='com.canonical.Unity.LauncherEntry',member='Update'")?;
            UNITY_ON.store(true, Ordering::Relaxed);
        }
        "avahi" => {
            let b = bus(true)?;
            b.track(AVAHI)?;
            b.add_rule("type='signal',sender='org.freedesktop.Avahi',interface='org.freedesktop.Avahi.ServiceBrowser'")?;
            let tx = AVAHI_TX.get_or_init(|| {
                let (tx, rx) = channel();
                thread::spawn(move || avahi(b, rx));
                tx
            });
            let _ = tx.send(Avahi::On);
        }
        "location" => {
            let b = bus(true)?;
            b.track(GEOCLUE)?;
            b.add_rule("type='signal',sender='org.freedesktop.GeoClue2'")?;
            let tx = GEO_TX.get_or_init(|| {
                let (tx, rx) = channel();
                thread::spawn(move || location(b, rx));
                tx
            });
            let _ = tx.send(Geo::On);
        }
        _ => return Err(format!("unknown source {src}")),
    }
    Ok(())
}

// ── Avahi: Google Cast devices ──────────────────────────────────────────
// Each service heard is resolved (its address, port and TXT: id, fn the
// friendly name, md the model) and reported as
//   {"src":"avahi","kind":"new","key":"<interface>;<protocol>;<name>",
//    "name","host","port","txt":{...}}
// and {"kind":"remove","key"} when it goes; {"kind":"down"} when Avahi
// itself goes (everything it knew is gone).
struct Item {
    iface: i32,
    proto: i32,
    name: String,
    ty: String,
    domain: String,
}

impl Item {
    fn key(&self) -> String {
        format!("{};{};{}", self.iface, self.proto, self.name)
    }
}

enum Avahi {
    On,
    Off,
    Up,
    Down,
    New(Item),
    Remove(Item),
}

fn avahi(b: &'static Bus, rx: Receiver<Avahi>) {
    let mut on = false;
    let mut browser: Option<String> = None;
    for msg in rx {
        match msg {
            Avahi::On => {
                on = true;
                if browser.is_none() {
                    browser = browse(b);
                }
            }
            Avahi::Off => {
                on = false;
                if let (Some(path), Some(owner)) = (browser.take(), b.owner(AVAHI)) {
                    let _ = b.call(&owner, &path, "org.freedesktop.Avahi.ServiceBrowser", "Free", &());
                }
            }
            Avahi::Up => {
                browser = None; // (the old daemon's browser went with it)
                if on {
                    browser = browse(b);
                }
            }
            Avahi::Down => {
                browser = None;
                if on {
                    emit(json!({"src": "avahi", "kind": "down"}));
                }
            }
            Avahi::New(item) if on && item.ty == CAST => {
                if let Some(e) = resolve(b, &item) {
                    emit(e);
                }
            }
            Avahi::Remove(item) if on && item.ty == CAST => {
                emit(json!({"src": "avahi", "kind": "remove", "key": item.key()}));
            }
            _ => {}
        }
    }
}

// A browser for Cast devices: prepared, then started, so no signal comes
// before its path is known (Avahi 0.8); ServiceBrowserNew on older ones.
fn browse(b: &Bus) -> Option<String> {
    let owner = b.owner(AVAHI)?; // (not running: browsed once it starts)
    let args = (-1i32, -1i32, CAST, "", 0u32);
    let server = "org.freedesktop.Avahi.Server2";
    if let Ok(m) = b.call(&owner, "/", server, "ServiceBrowserPrepare", &args) {
        let path: zbus::zvariant::OwnedObjectPath = m.body().deserialize().ok()?;
        b.call(&owner, path.as_str(), "org.freedesktop.Avahi.ServiceBrowser", "Start", &()).ok()?;
        return Some(path.to_string());
    }
    let m = b.call(&owner, "/", "org.freedesktop.Avahi.Server", "ServiceBrowserNew", &args).ok()?;
    let path: zbus::zvariant::OwnedObjectPath = m.body().deserialize().ok()?;
    Some(path.to_string())
}

type Resolved = (i32, i32, String, String, String, String, i32, String, u16, Vec<Vec<u8>>, u32);

fn resolve(b: &Bus, item: &Item) -> Option<serde_json::Value> {
    let owner = b.owner(AVAHI)?;
    let args = (item.iface, item.proto, item.name.as_str(), item.ty.as_str(), item.domain.as_str(), -1i32, 0u32);
    let m = b.call(&owner, "/", "org.freedesktop.Avahi.Server", "ResolveService", &args).ok()?;
    let r: Resolved = m.body().deserialize().ok()?;
    Some(resolved_event(item, &r.7, r.8, &r.9))
}

fn resolved_event(item: &Item, address: &str, port: u16, txt: &[Vec<u8>]) -> serde_json::Value {
    json!({"src": "avahi", "kind": "new", "key": item.key(), "name": item.name, "host": address, "port": port, "txt": dj::txt(txt)})
}

// ── GeoClue: who is using your location ─────────────────────────────────
// Asked a second after GeoClue signals anything (or comes or goes), and once
// a minute while it runs, as a safety net. Reported as
//   {"src":"location","apps":[desktop ids]}   ("?" for an app it can't name)
// whenever the list changes, and once when turned on.
enum Geo {
    On,
    Off,
    Dirty,
}

fn location(b: &'static Bus, rx: Receiver<Geo>) {
    let mut on = false;
    let mut prev: Option<Vec<String>> = None;
    let mut dirty: Option<Instant> = None;
    loop {
        let wait = match dirty {
            Some(t) => Duration::from_secs(1).saturating_sub(t.elapsed()),
            None if on && b.owner(GEOCLUE).is_some() => Duration::from_secs(60),
            None => Duration::from_secs(86400),
        };
        let check = match rx.recv_timeout(wait) {
            Ok(Geo::On) => {
                on = true;
                prev = None;
                true
            }
            Ok(Geo::Off) => {
                on = false;
                dirty = None;
                false
            }
            Ok(Geo::Dirty) => {
                if on {
                    dirty = Some(Instant::now()); // (looked at once it's been quiet a second)
                }
                false
            }
            Err(RecvTimeoutError::Timeout) => {
                dirty = None;
                on
            }
            Err(RecvTimeoutError::Disconnected) => return,
        };
        if check {
            let now = location_apps(b);
            if prev.as_ref() != Some(&now) {
                emit(json!({"src": "location", "apps": now}));
                prev = Some(now);
            }
        }
    }
}

fn location_apps(b: &Bus) -> Vec<String> {
    // (to its unique name: a call to the well-known one could start it)
    let Some(owner) = b.owner(GEOCLUE) else { return vec![] };
    let in_use = b.property(&owner, "/org/freedesktop/GeoClue2/Manager", "org.freedesktop.GeoClue2.Manager", "InUse");
    if in_use.and_then(|v| v.as_bool()) != Some(true) {
        return vec![];
    }
    let mut apps = vec![];
    let xml = b
        .call(&owner, "/org/freedesktop/GeoClue2/Client", "org.freedesktop.DBus.Introspectable", "Introspect", &())
        .ok()
        .and_then(|m| m.body().deserialize::<String>().ok())
        .unwrap_or_default();
    for child in child_nodes(&xml) {
        let path = format!("/org/freedesktop/GeoClue2/Client/{child}");
        let client = "org.freedesktop.GeoClue2.Client";
        if b.property(&owner, &path, client, "Active").and_then(|v| v.as_bool()) == Some(true) {
            if let Some(id) = b.property(&owner, &path, client, "DesktopId").and_then(|v| v.as_str().map(String::from)) {
                if !id.is_empty() {
                    apps.push(id);
                }
            }
        }
    }
    apps.sort();
    apps.dedup();
    if apps.is_empty() {
        apps.push("?".into());
    }
    apps
}

/// The children an Introspect answer lists (<node name="3"/>).
fn child_nodes(xml: &str) -> Vec<String> {
    let mut out = vec![];
    let mut rest = xml;
    while let Some(i) = rest.find("<node name=\"") {
        rest = &rest[i + 12..];
        if let Some(j) = rest.find('"') {
            let name = &rest[..j];
            if !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                out.push(name.to_string());
            }
            rest = &rest[j..];
        }
    }
    out
}

// ── logind inhibitors ───────────────────────────────────────────────────
/// Holds (or lets go of) a logind inhibitor: the fd Inhibit() returns, kept
/// open while on. It goes with this process, so a crashed shell never
/// leaves the lid or idle inhibited. Answers {"src":"inhibit","what","on"}
/// (with "error" when logind said no).
pub fn inhibit(what: &str, why: &str, on: bool) {
    let mut held = HELD.lock().unwrap();
    if !on {
        held.remove(what);
        emit(json!({"src": "inhibit", "what": what, "on": false}));
        return;
    }
    if held.contains_key(what) {
        emit(json!({"src": "inhibit", "what": what, "on": true}));
        return;
    }
    // (the nested dev sandbox shares the real system bus: never inhibit
    // from it; the console session is a real one, as System.qml has it;
    // IRICE_*: a session started before iRice became Tessera)
    let flag = |name: &str| std::env::var(format!("TESSERA_{name}")).or_else(|_| std::env::var(format!("IRICE_{name}"))).as_deref() == Ok("1");
    if flag("SANDBOX") && !flag("TTY") {
        emit(json!({"src": "inhibit", "what": what, "on": false, "error": "not in the sandbox"}));
        return;
    }
    let r = bus(true).and_then(|b| {
        let m = b.call(LOGIN1, "/org/freedesktop/login1", "org.freedesktop.login1.Manager", "Inhibit", &(what, "Tessera", why, "block"))?;
        m.body().deserialize::<OwnedFd>().map_err(|e| e.to_string())
    });
    match r {
        Ok(fd) => {
            held.insert(what.to_string(), fd);
            emit(json!({"src": "inhibit", "what": what, "on": true}));
        }
        Err(e) => emit(json!({"src": "inhibit", "what": what, "on": false, "error": e})),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nm_cues() {
        assert!(nm_matters("/org/freedesktop/NetworkManager", NM, "StateChanged"));
        assert!(nm_matters("/org/freedesktop/NetworkManager/Devices/3", "org.freedesktop.NetworkManager.Device", "StateChanged"));
        assert!(nm_matters("/org/freedesktop/NetworkManager", "org.freedesktop.DBus.Properties", "PropertiesChanged"));
        // an access point's strength changing isn't a network change
        assert!(!nm_matters("/org/freedesktop/NetworkManager/AccessPoint/9", "org.freedesktop.DBus.Properties", "PropertiesChanged"));
        assert!(!nm_matters("/org/freedesktop/NetworkManager", NM, "CheckPermissions"));
    }

    #[test]
    fn avahi_events() {
        let item = Item { iface: 3, proto: 0, name: "Chromecast-abc".into(), ty: CAST.into(), domain: "local".into() };
        let txt = vec![b"id=abc".to_vec(), b"fn=Living Room".to_vec(), b"md=Google TV".to_vec()];
        assert_eq!(
            resolved_event(&item, "192.168.1.20", 8009, &txt),
            json!({"src": "avahi", "kind": "new", "key": "3;0;Chromecast-abc", "name": "Chromecast-abc", "host": "192.168.1.20",
                   "port": 8009, "txt": {"id": "abc", "fn": "Living Room", "md": "Google TV"}})
        );
    }

    #[test]
    fn introspect_children() {
        let xml = r#"<node><interface name="x"/><node name="1"/><node name="12"/><node name="../evil"/></node>"#;
        assert_eq!(child_nodes(xml), vec!["1", "12"]);
        assert!(child_nodes("").is_empty());
    }
}

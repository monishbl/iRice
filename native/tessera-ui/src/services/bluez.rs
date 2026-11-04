//! Bluetooth (BlueZ) over the system bus, read-only for now
//! (services::bluez::Bluetooth): the first adapter and every known device,
//! connected or not, with its battery (org.bluez.Battery1) when it reports
//! one. A thread reads BlueZ's objects (ObjectManager) and reads again
//! after each burst of its signals (properties, devices added or removed).
//! (Turning it on or off and connecting are the Control Center's, still in
//! QML: Bluetooth acts on the real hardware, even from the sandbox.)

use crate::Msg;
use smithay_client_toolkit::reexports::calloop::channel::Sender;
use std::collections::HashMap;
use std::time::Duration;
use zbus::blocking::{Connection, MessageIterator};
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};
use zbus::MatchRule;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Device {
    pub path: String,
    pub address: String,
    pub name: String,
    /// BlueZ's icon name (audio-headset, input-mouse…)
    pub icon: String,
    pub paired: bool,
    pub connected: bool,
    /// 0..100, when it says
    pub battery: Option<u8>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Bluetooth {
    /// an adapter exists
    pub present: bool,
    pub powered: bool,
    pub devices: Vec<Device>,
}

type Objects = HashMap<OwnedObjectPath, HashMap<String, HashMap<String, OwnedValue>>>;

fn s(p: &HashMap<String, OwnedValue>, k: &str) -> String {
    match p.get(k).map(|v| &**v) {
        Some(Value::Str(s)) => s.to_string(),
        _ => String::new(),
    }
}
fn b(p: &HashMap<String, OwnedValue>, k: &str) -> bool {
    matches!(p.get(k).map(|v| &**v), Some(Value::Bool(true)))
}

/// BlueZ's objects as the bar needs them (devices by name)
pub fn from_objects(objs: &Objects) -> Bluetooth {
    let mut out = Bluetooth::default();
    let mut adapters: Vec<(&str, bool)> = objs
        .iter()
        .filter_map(|(path, ifs)| ifs.get("org.bluez.Adapter1").map(|a| (path.as_str(), b(a, "Powered"))))
        .collect();
    adapters.sort();
    if let Some((_, powered)) = adapters.first() {
        out.present = true;
        out.powered = *powered;
    }
    for (path, ifs) in objs {
        let Some(d) = ifs.get("org.bluez.Device1") else { continue };
        let battery = ifs.get("org.bluez.Battery1").and_then(|bat| match bat.get("Percentage").map(|v| &**v) {
            Some(Value::U8(p)) => Some(*p),
            _ => None,
        });
        let name = [s(d, "Alias"), s(d, "Name"), s(d, "Address")].into_iter().find(|n| !n.is_empty()).unwrap_or_default();
        out.devices.push(Device { path: path.to_string(), address: s(d, "Address"), name, icon: s(d, "Icon"), paired: b(d, "Paired"), connected: b(d, "Connected"), battery });
    }
    out.devices.sort_by(|a, b| b.connected.cmp(&a.connected).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    out
}

impl Bluetooth {
    pub fn any_connected(&self) -> bool {
        self.devices.iter().any(|d| d.connected)
    }
    /// The menu bar item's icon (MenuBar.qml)
    pub fn icon(&self) -> &'static str {
        if !self.powered {
            "bluetooth-slash"
        } else if self.any_connected() {
            "bluetooth-connected"
        } else {
            "bluetooth"
        }
    }
    /// Its hover tip: off, nothing connected, or who is (with batteries)
    pub fn summary(&self) -> String {
        if !self.powered {
            return "Bluetooth off".into();
        }
        let on: Vec<String> = self.devices.iter().filter(|d| d.connected).map(|d| d.name.clone() + &d.battery.map(|p| format!(" · {p}%")).unwrap_or_default()).collect();
        if on.is_empty() {
            "Bluetooth on · nothing connected".into()
        } else {
            on.join(", ")
        }
    }
}

fn read(conn: &Connection) -> zbus::Result<Bluetooth> {
    let m = conn.call_method(Some("org.bluez"), "/", Some("org.freedesktop.DBus.ObjectManager"), "GetManagedObjects", &())?;
    let objs: Objects = m.body().deserialize()?;
    Ok(from_objects(&objs))
}

pub fn spawn(tx: Sender<Msg>) {
    std::thread::spawn(move || loop {
        let run = || -> zbus::Result<()> {
            let conn = Connection::system()?;
            let rule = MatchRule::builder().msg_type(zbus::message::Type::Signal).sender("org.bluez")?.build();
            let mut signals = MessageIterator::for_match_rule(rule, &conn, Some(64))?;
            let mut last: Option<Bluetooth> = None;
            loop {
                // (no BlueZ at all: an empty reading, and wait for it)
                let now = read(&conn).unwrap_or_default();
                if last.as_ref() != Some(&now) {
                    tx.send(Msg::Data(Box::new(now.clone()))).map_err(|_| zbus::Error::Failure("ui gone".into()))?;
                    last = Some(now);
                }
                if signals.next().is_none() {
                    return Ok(());
                }
                // coalesce a burst (a device connecting changes several properties)
                std::thread::sleep(Duration::from_millis(80));
            }
        };
        if let Err(zbus::Error::Failure(_)) = run() {
            return;
        }
        std::thread::sleep(Duration::from_secs(5));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn props(list: &[(&str, Value<'static>)]) -> HashMap<String, OwnedValue> {
        list.iter().map(|(k, v)| (k.to_string(), OwnedValue::try_from(v.clone()).unwrap())).collect()
    }

    fn objects() -> Objects {
        let mut o: Objects = HashMap::new();
        let path = |p: &str| OwnedObjectPath::try_from(p.to_string()).unwrap();
        o.insert(path("/org/bluez/hci0"), [("org.bluez.Adapter1".to_string(), props(&[("Powered", Value::from(true))]))].into());
        o.insert(
            path("/org/bluez/hci0/dev_AA"),
            [
                ("org.bluez.Device1".to_string(), props(&[("Alias", Value::from("Maya's earbuds")), ("Address", Value::from("AA")), ("Connected", Value::from(true)), ("Icon", Value::from("audio-headset"))])),
                ("org.bluez.Battery1".to_string(), props(&[("Percentage", Value::from(80u8))])),
            ]
            .into(),
        );
        o.insert(path("/org/bluez/hci0/dev_BB"), [("org.bluez.Device1".to_string(), props(&[("Name", Value::from("Keyboard")), ("Address", Value::from("BB")), ("Paired", Value::from(true))]))].into());
        o
    }

    #[test]
    fn devices_from_bluez_objects() {
        let bt = from_objects(&objects());
        assert!(bt.present && bt.powered);
        assert_eq!(bt.devices.len(), 2);
        // connected ones first
        assert_eq!(bt.devices[0].name, "Maya's earbuds");
        assert_eq!(bt.devices[0].battery, Some(80));
        assert_eq!(bt.devices[1].name, "Keyboard");
        assert!(bt.devices[1].paired && !bt.devices[1].connected);
        assert_eq!(bt.icon(), "bluetooth-connected");
        assert_eq!(bt.summary(), "Maya's earbuds · 80%");
    }

    #[test]
    fn off_and_idle_say_so() {
        let mut bt = from_objects(&objects());
        bt.devices.iter_mut().for_each(|d| d.connected = false);
        assert_eq!(bt.icon(), "bluetooth");
        assert_eq!(bt.summary(), "Bluetooth on · nothing connected");
        bt.powered = false;
        assert_eq!((bt.icon(), bt.summary().as_str()), ("bluetooth-slash", "Bluetooth off"));
        assert!(!from_objects(&HashMap::new()).present);
    }
}

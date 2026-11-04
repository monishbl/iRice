//! Battery (UPower's display device) and Wi-Fi (NetworkManager) over the
//! system bus with zbus, read-only. Each runs on a thread that blocks on the
//! service's PropertiesChanged signals and sends a fresh reading.

use crate::Msg;
use smithay_client_toolkit::reexports::calloop::channel::Sender;
use std::time::Duration;
use zbus::blocking::{proxy::Builder, Connection, MessageIterator, Proxy};
use zbus::proxy::CacheProperties;
use zbus::zvariant::OwnedObjectPath;
use zbus::MatchRule;

#[derive(Clone, Debug, Default)]
pub struct Battery {
    pub present: bool,
    pub percent: f64,
    pub charging: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Wifi {
    pub present: bool, // a Wi-Fi device exists
    pub enabled: bool,
    pub ssid: String,  // "" not connected
    pub strength: u8,  // 0..100
}

fn watch<T: Send + 'static>(
    tx: Sender<Msg>,
    sender: &'static str,
    read: fn(&Connection) -> zbus::Result<T>,
    wrap: fn(T) -> Msg,
) {
    std::thread::spawn(move || loop {
        let run = || -> zbus::Result<()> {
            let conn = Connection::system()?;
            let rule = MatchRule::builder()
                .msg_type(zbus::message::Type::Signal)
                .sender(sender)?
                .interface("org.freedesktop.DBus.Properties")?
                .member("PropertiesChanged")?
                .build();
            let mut signals = MessageIterator::for_match_rule(rule, &conn, Some(16))?;
            tx.send(wrap(read(&conn)?)).map_err(|_| zbus::Error::Failure("ui gone".into()))?;
            while signals.next().is_some() {
                tx.send(wrap(read(&conn)?)).map_err(|_| zbus::Error::Failure("ui gone".into()))?;
            }
            Ok(())
        };
        if let Err(zbus::Error::Failure(_)) = run() {
            return;
        }
        std::thread::sleep(Duration::from_secs(5));
    });
}

// Plain property reads: no cache (each reading is fresh, and a cached proxy
// would add its own match rules)
fn proxy<'a>(conn: &Connection, path: &'a str, iface: &'a str, dest: &'a str) -> zbus::Result<Proxy<'a>> {
    Builder::new(conn).destination(dest)?.path(path)?.interface(iface)?.cache_properties(CacheProperties::No).build()
}

fn battery(conn: &Connection) -> zbus::Result<Battery> {
    let p = proxy(conn, "/org/freedesktop/UPower/devices/DisplayDevice", "org.freedesktop.UPower.Device", "org.freedesktop.UPower")?;
    let kind: u32 = p.get_property("Type")?;
    let state: u32 = p.get_property("State")?;
    Ok(Battery {
        present: kind == 2 && p.get_property::<bool>("IsPresent")?,
        percent: p.get_property("Percentage")?,
        charging: state == 1 || state == 4, // charging, fully charged
    })
}

const NM: &str = "org.freedesktop.NetworkManager";

fn wifi(conn: &Connection) -> zbus::Result<Wifi> {
    let nm = proxy(conn, "/org/freedesktop/NetworkManager", "org.freedesktop.NetworkManager", NM)?;
    let mut out = Wifi { enabled: nm.get_property("WirelessEnabled")?, ..Default::default() };
    let devices: Vec<OwnedObjectPath> = nm.call("GetDevices", &())?;
    for d in devices {
        let dev = proxy(conn, d.as_str(), "org.freedesktop.NetworkManager.Device", NM)?;
        if dev.get_property::<u32>("DeviceType")? != 2 {
            continue; // not Wi-Fi
        }
        out.present = true;
        let w = proxy(conn, d.as_str(), "org.freedesktop.NetworkManager.Device.Wireless", NM)?;
        let ap: OwnedObjectPath = w.get_property("ActiveAccessPoint")?;
        if ap.as_str() != "/" {
            let ap = proxy(conn, ap.as_str(), "org.freedesktop.NetworkManager.AccessPoint", NM)?;
            out.strength = ap.get_property("Strength")?;
            let ssid: Vec<u8> = ap.get_property("Ssid")?;
            out.ssid = String::from_utf8_lossy(&ssid).into_owned();
        }
        break;
    }
    Ok(out)
}

pub fn spawn(tx: Sender<Msg>) {
    watch(tx.clone(), "org.freedesktop.UPower", battery, Msg::Battery);
    watch(tx, "org.freedesktop.NetworkManager", wifi, Msg::Wifi);
}

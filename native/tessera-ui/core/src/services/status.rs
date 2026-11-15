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

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Battery {
    pub present: bool,
    pub percent: f64,
    pub charging: bool,
    /// UPower's state: 1 charging, 2 discharging, 4 fully charged, 5 pending charge
    pub state: u32,
    /// seconds, 0 unknown
    pub time_to_empty: i64,
    pub time_to_full: i64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Wifi {
    pub present: bool, // a Wi-Fi device exists
    pub enabled: bool,
    pub ssid: String,  // "" not connected
    pub strength: u8,  // 0..100
}

fn watch<T: Send + Clone + PartialEq + 'static>(
    tx: Sender<Msg>,
    sender: &'static str,
    read: fn(&Connection) -> zbus::Result<T>,
    wrap: fn(T) -> Msg,
) {
    // (a computer without the service, UPower or NetworkManager not
    // installed: asked again less and less often, up to once a minute)
    let mut wait = 5;
    let mut first = true;
    std::thread::spawn(move || loop {
        let mut up = false;
        let mut run = || -> zbus::Result<()> {
            let conn = Connection::system()?;
            // (a computer without it, NetworkManager under iwd or ConnMan, a
            // desktop without UPower: sleep until it appears, not a new bus
            // connection every few seconds, forever)
            wait_for_owner(&conn, sender, first)?;
            let rule = MatchRule::builder()
                .msg_type(zbus::message::Type::Signal)
                .sender(sender)?
                .interface("org.freedesktop.DBus.Properties")?
                .member("PropertiesChanged")?
                .build();
            let mut signals = MessageIterator::for_match_rule(rule, &conn, Some(16))?;
            // (only what changed: NetworkManager and UPower signal often,
            // for properties these readings don't hold)
            let mut last = read(&conn)?;
            up = true;
            tx.send(wrap(last.clone())).map_err(|_| zbus::Error::Failure("ui gone".into()))?;
            while signals.next().is_some() {
                let now = read(&conn)?;
                if now != last {
                    tx.send(wrap(now.clone())).map_err(|_| zbus::Error::Failure("ui gone".into()))?;
                    last = now;
                }
            }
            Ok(())
        };
        if let Err(zbus::Error::Failure(_)) = run() {
            return;
        }
        first = false;
        wait = if up { 5 } else { (wait * 2).min(60) };
        std::thread::sleep(Duration::from_secs(wait));
    });
}

/// Return once `name` has an owner on the bus: at once when it has, else
/// when NameOwnerChanged says it got one (the system bus never activates
/// NetworkManager or UPower for a property read here)
fn wait_for_owner(conn: &Connection, name: &'static str, activate: bool) -> zbus::Result<()> {
    let rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .sender("org.freedesktop.DBus")?
        .interface("org.freedesktop.DBus")?
        .member("NameOwnerChanged")?
        .arg(0, name)?
        .build();
    // (the rule first, so an owner that appears between the two isn't missed)
    let mut changes = MessageIterator::for_match_rule(rule, conn, Some(8))?;
    let dbus = zbus::blocking::fdo::DBusProxy::new(conn)?;
    let has = |n: &str| zbus::names::BusName::try_from(n).ok().map(|b| dbus.name_has_owner(b).unwrap_or(false)).unwrap_or(false);
    if has(name) {
        return Ok(());
    }
    // (activatable over D-Bus: the first read starts it, as before; a
    // masked one that never starts is then waited for like a missing one)
    if activate && dbus.list_activatable_names().is_ok_and(|l| l.iter().any(|n| n.as_str() == name)) {
        return Ok(());
    }
    while let Some(m) = changes.next() {
        let m = m?;
        let body = m.body();
        if let Ok((_, _, new)) = body.deserialize::<(String, String, String)>() {
            if !new.is_empty() {
                return Ok(());
            }
        }
    }
    Ok(())
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
        state,
        time_to_empty: p.get_property("TimeToEmpty").unwrap_or(0),
        time_to_full: p.get_property("TimeToFull").unwrap_or(0),
    })
}

const NM: &str = "org.freedesktop.NetworkManager";

/// One Wi-Fi device's reading
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Radio {
    pub connected: bool,
    pub strength: u8,
    pub ssid: String,
}

/// Several Wi-Fi devices (a built-in card and a USB stick): the connected
/// one, the strongest of those; else the first. None: no Wi-Fi device.
pub fn pick_radio(radios: &[Radio]) -> Option<&Radio> {
    radios.iter().filter(|r| r.connected).max_by_key(|r| r.strength).or(radios.first())
}

fn radio(conn: &Connection, d: &OwnedObjectPath) -> zbus::Result<Option<Radio>> {
    let dev = proxy(conn, d.as_str(), "org.freedesktop.NetworkManager.Device", NM)?;
    if dev.get_property::<u32>("DeviceType")? != 2 {
        return Ok(None); // not Wi-Fi
    }
    let w = proxy(conn, d.as_str(), "org.freedesktop.NetworkManager.Device.Wireless", NM)?;
    let ap: OwnedObjectPath = w.get_property("ActiveAccessPoint")?;
    let mut r = Radio::default();
    if ap.as_str() != "/" {
        // (the access point can go between two reads: still a device)
        if let Ok(ap) = proxy(conn, ap.as_str(), "org.freedesktop.NetworkManager.AccessPoint", NM) {
            if let (Ok(strength), Ok(ssid)) = (ap.get_property::<u8>("Strength"), ap.get_property::<Vec<u8>>("Ssid")) {
                r = Radio { connected: true, strength, ssid: String::from_utf8_lossy(&ssid).into_owned() };
            }
        }
    }
    Ok(Some(r))
}

fn wifi(conn: &Connection) -> zbus::Result<Wifi> {
    let nm = proxy(conn, "/org/freedesktop/NetworkManager", "org.freedesktop.NetworkManager", NM)?;
    let mut out = Wifi { enabled: nm.get_property("WirelessEnabled")?, ..Default::default() };
    let devices: Vec<OwnedObjectPath> = nm.call("GetDevices", &())?;
    // (a device that goes while it's read, a USB stick pulled out, is
    // skipped: the others still count, and its removal brings a new reading)
    let radios: Vec<Radio> = devices.iter().filter_map(|d| radio(conn, d).ok().flatten()).collect();
    if let Some(r) = pick_radio(&radios) {
        out.present = true;
        out.strength = r.strength;
        out.ssid = r.ssid.clone();
    }
    Ok(out)
}

pub fn spawn(tx: Sender<Msg>) {
    watch(tx.clone(), "org.freedesktop.UPower", battery, |b| Msg::Data(Box::new(b)));
    watch(tx, "org.freedesktop.NetworkManager", wifi, |w| Msg::Data(Box::new(w)));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(connected: bool, strength: u8, ssid: &str) -> Radio {
        Radio { connected, strength, ssid: ssid.into() }
    }

    #[test]
    fn the_connected_wifi_device_is_the_one_shown() {
        // Ethernet only: no Wi-Fi device
        assert_eq!(pick_radio(&[]), None);
        // one card, not connected
        assert_eq!(pick_radio(&[r(false, 0, "")]), Some(&r(false, 0, "")));
        // a built-in card idle and a USB stick connected: the stick
        assert_eq!(pick_radio(&[r(false, 0, ""), r(true, 40, "Cafe")]).unwrap().ssid, "Cafe");
        // both connected: the stronger
        assert_eq!(pick_radio(&[r(true, 30, "A"), r(true, 70, "B")]).unwrap().ssid, "B");
    }
}

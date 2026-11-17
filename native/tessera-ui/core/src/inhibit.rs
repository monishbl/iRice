//! logind inhibitors held by this process itself: the file descriptor logind
//! hands back, kept open for as long as it's held. It closes when the
//! process ends, however it ends (a crash, a kill), so an inhibitor can never
//! outlive the shell. (16 Nov: systemd-inhibit helpers ignored SIGTERM while
//! their child ran, outlived crashed shells, and a closed lid didn't sleep a
//! laptop for 7 hours.)

use zbus::zvariant::OwnedFd;

const DEST: &str = "org.freedesktop.login1";
const PATH: &str = "/org/freedesktop/login1";
const IFACE: &str = "org.freedesktop.login1.Manager";

/// Hold a "block" inhibitor for `what` ("handle-lid-switch", "idle",
/// "sleep"…); drop the fd to let go. None: no logind (or it refused).
pub fn hold(what: &str, why: &str) -> Option<OwnedFd> {
    let conn = zbus::blocking::Connection::system().ok()?;
    let reply = conn.call_method(Some(DEST), PATH, Some(IFACE), "Inhibit", &(what, "Tessera", why, "block")).ok()?;
    reply.body().deserialize::<OwnedFd>().ok()
}

/// logind's LidClosed now (None: no lid, or no logind)
pub fn lid_closed() -> Option<bool> {
    let conn = zbus::blocking::Connection::system().ok()?;
    let reply = conn
        .call_method(Some(DEST), PATH, Some("org.freedesktop.DBus.Properties"), "Get", &(IFACE, "LidClosed"))
        .ok()?;
    let v: zbus::zvariant::OwnedValue = reply.body().deserialize().ok()?;
    bool::try_from(v).ok()
}

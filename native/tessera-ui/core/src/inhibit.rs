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
    take(what, why, "block")
}

/// Hold a "delay" inhibitor for `what` ("sleep", "shutdown"): logind waits
/// (up to its InhibitDelayMaxSec, 5 s by default) after PrepareForSleep
/// until it's dropped, so a part can tidy up first. Drop it once done, and
/// take it again after the wake.
pub fn delay(what: &str, why: &str) -> Option<OwnedFd> {
    take(what, why, "delay")
}

/// `hold` on the pool: `done` gets the answer there (send it to the part
/// by message). For a caller on the UI thread, which a system-bus round
/// trip right after a wake could stall (audit #48)
pub fn hold_then(what: &str, why: &str, done: impl FnOnce(Option<OwnedFd>) + Send + 'static) {
    let (what, why) = (what.to_string(), why.to_string());
    crate::work::spawn(move || done(hold(&what, &why)));
}

/// `lid_closed` on the pool, the same way
pub fn lid_closed_then(done: impl FnOnce(Option<bool>) + Send + 'static) {
    crate::work::spawn(move || done(lid_closed()));
}

static CONN: std::sync::Mutex<Option<zbus::blocking::Connection>> = std::sync::Mutex::new(None);

/// One system-bus connection for these calls, made once (each call made
/// its own before: a connect, auth and Hello every time, and a zbus
/// executor thread started and dropped), made again after it dies.
/// Shared with other one-off system-bus calls (host/system's energy mode,
/// audit #135); call it off the UI thread
pub fn system_bus() -> Option<zbus::blocking::Connection> {
    system()
}

fn system() -> Option<zbus::blocking::Connection> {
    let mut c = CONN.lock().unwrap_or_else(|e| e.into_inner());
    if c.is_none() {
        *c = zbus::blocking::Connection::system().ok();
    }
    c.clone()
}

/// A call's answer; an I/O error (the socket closed) drops the connection
fn answer<T>(r: zbus::Result<T>) -> Option<T> {
    match r {
        Ok(v) => Some(v),
        Err(e) => {
            if matches!(e, zbus::Error::InputOutput(_)) {
                *CONN.lock().unwrap_or_else(|e| e.into_inner()) = None;
            }
            None
        }
    }
}

fn take(what: &str, why: &str, mode: &str) -> Option<OwnedFd> {
    let conn = system()?;
    let reply = answer(conn.call_method(Some(DEST), PATH, Some(IFACE), "Inhibit", &(what, "Tessera", why, mode)))?;
    reply.body().deserialize::<OwnedFd>().ok()
}

/// logind's LidClosed now (None: no lid, or no logind)
pub fn lid_closed() -> Option<bool> {
    let conn = system()?;
    let reply = answer(conn.call_method(Some(DEST), PATH, Some("org.freedesktop.DBus.Properties"), "Get", &(IFACE, "LidClosed")))?;
    let v: zbus::zvariant::OwnedValue = reply.body().deserialize().ok()?;
    bool::try_from(v).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dead_system_connection_is_made_again() {
        assert_eq!(answer(Ok(3u8)), Some(3));
        assert_eq!(answer::<u8>(Err(zbus::Error::Failure("refused".into()))), None);
        // (an I/O error: the kept connection goes, the next call makes one)
        assert_eq!(answer::<u8>(Err(zbus::Error::InputOutput(std::sync::Arc::new(std::io::Error::other("closed"))))), None);
        assert!(CONN.lock().unwrap().is_none());
    }
}

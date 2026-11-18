//! The fingerprint reader on the lock: when it listens, how many tries, a
//! sleep, and what's done when fprintd is stuck.
//!
//! 18 Nov (Dell, ControlVault 3, USB): a check left running into a sleep
//! kept fprintd's reader open; the reader dropped off USB in s2idle, fprintd
//! couldn't close it ("Error closing device after disconnect: The device is
//! still busy") and from then on every claim said "Open failed with error:
//! The device has already been opened!" until fprintd restarted. The lock
//! retried every 5 s for good; only the password worked.
//!
//! The rules (the user's choice, 18 Nov):
//!   Listening   a fresh scan (any old helper ended first, then the reader
//!               confirmed: fprintd's GetDevices and a claim that works) on
//!               every wake, screen on, lid open, and a key, pointer or touch
//!               while the lock shows. One scan always runs while the lock
//!               shows, renewed every 30 s (pam_fprintd's timeout). After a
//!               wake the reader is waited for (up to 10 s) while USB brings
//!               it back.
//!   Tries       3 real misses: "Not recognised. Place your finger again", a
//!               2 s pause, then 3 more; after 6, the password only until the
//!               next wake or unlock. (A "no match" faster than a touch isn't
//!               a miss: Lock.qml's rule, in the lock.)
//!   Idle        no finger, key, pointer or touch for 2 minutes: no scan (the
//!               screen dims then); a key, the pointer, the lid or the screen
//!               coming on starts one at once.
//!   Sleep       a logind "delay" inhibitor while a scan can run; on
//!               PrepareForSleep(true) `before_sleep` ends the scans (SIGTERM,
//!               waits for the helpers), waits until fprintd has let the
//!               reader go (a claim of ours that works), then lets the sleep
//!               go, within ~2 s.
//!   Stuck       "already been opened", fprintd's Internal error, or scans
//!               that keep failing at once (after a wake: the reader never
//!               came back): fprintd is restarted through systemd's bus
//!               (RestartUnit, never interactive) where Tessera's polkit rule
//!               allows it (49-tessera.rules, installed by the root helper's
//!               one-time setup), at most once in 2 minutes, then a fresh
//!               scan. Without the rule: "Fingerprint needs a restart. Use your
//!               password" under the field, a look every 30 s, and after the
//!               unlock a notification whose button restarts it (pkexec).
//!
//! fprintd's and systemd's buses are behind `Fprint`: all of it is tested
//! without them.

use crate::auth::{Conv, Kind};
use std::time::{Duration, Instant};

/// What the lock says under the field while fprintd is stuck
pub const STUCK_TEXT: &str = "Fingerprint needs a restart. Use your password";
/// A miss
pub const MISS_TEXT: &str = "Not recognised. Try again";
/// The third miss (and the pause after it)
pub const AGAIN_TEXT: &str = "Not recognised. Place your finger again";

/// Real misses before the pause, and in all
pub const PAUSE_AFTER: u32 = 3;
pub const MAX_MISSES: u32 = 6;
pub const PAUSE_MS: u64 = 2000;
/// No finger, key, pointer or touch this long: no scan
pub const IDLE_MS: i64 = 120_000;
/// A scan is renewed this often (pam_fprintd's timeout=30)
pub const RENEW_MS: i64 = 30_000;
/// A key or the pointer leaves a scan younger than this that's listening
pub const FRESH_MS: i64 = 10_000;
/// fprintd is restarted at most this often
pub const RESTART_EVERY_MS: i64 = 120_000;
/// A stuck reader is looked at again this often
pub const STUCK_EVERY_MS: u64 = 30_000;
/// Scans over at once in a row before fprintd is asked what's wrong; and
/// after which it's stuck whatever fprintd says
pub const CHECK_AFTER: u32 = 3;
pub const STUCK_AFTER: u32 = 8;

/// What fprintd says about the reader now
#[derive(Clone, Debug, PartialEq)]
pub enum Probe {
    /// claimed and let go: a scan can start
    Ready,
    /// no reader (not back on USB yet)
    NoReader,
    /// in use, or busy for a moment (another claim, a close under way)
    Busy(String),
    /// fprintd holds it open and won't let go: only a restart helps
    Stuck(String),
    /// no fprintd (not running, restarting) or no system bus
    Gone(String),
}

/// fprintd's answer to a claim, by its error
pub fn classify(err: &str) -> Probe {
    let l = err.to_lowercase();
    if l.contains("already been opened") {
        Probe::Stuck(err.to_string())
    } else if l.contains("nosuchdevice") || l.contains("no devices") {
        Probe::NoReader
    } else if l.contains("serviceunknown") || l.contains("name has no owner") || l.contains("noreply") || l.contains("no reply") || l.contains("not activat") {
        Probe::Gone(err.to_string())
    } else if l.contains("fprint.error.internal") && !l.contains("busy") {
        // (fprintd's own trouble: a claim that fails inside it)
        Probe::Stuck(err.to_string())
    } else {
        // (AlreadyInUse, "still busy with another operation": a moment
        // later is another answer)
        Probe::Busy(err.to_string())
    }
}

/// fprintd and its service, as far as the lock asks
pub trait Fprint: Send + Sync {
    /// The first reader's object path (GetDevices; None: no reader)
    fn device(&self) -> Result<Option<String>, String>;
    /// Claim it for the current user and release it at once
    fn claim_release(&self, device: &str) -> Result<(), String>;
    /// Restart fprintd.service without asking anyone (Err: not allowed, or it failed)
    fn restart(&self) -> Result<(), String>;
}

/// Ask fprintd once
pub fn probe(f: &dyn Fprint) -> Probe {
    match f.device() {
        Ok(None) => Probe::NoReader,
        Err(e) => classify(&e),
        Ok(Some(dev)) => match f.claim_release(&dev) {
            Ok(()) => Probe::Ready,
            Err(e) => classify(&e),
        },
    }
}

/// Ask until the reader is there (or stuck), up to `within`, every `every`
pub fn wait_for_reader(f: &dyn Fprint, within: Duration, every: Duration) -> Probe {
    let start = Instant::now();
    loop {
        let p = probe(f);
        if matches!(p, Probe::Ready | Probe::Stuck(_)) || start.elapsed() + every > within {
            return p;
        }
        std::thread::sleep(every);
    }
}

/// Why the lock asked fprintd (a fresh scan is about to start)
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Why {
    /// a key, the pointer, the lid, the screen on, the renewal, the pause over
    Fresh,
    /// just woke: wait for the reader to be back
    Wake,
    /// the scans kept failing at once
    Failing,
    /// stuck: looking again (every 30 s)
    Stuck,
    /// fprintd was restarted: wait for it and the reader
    Restarted,
}

impl Why {
    /// How long to wait for the reader
    pub fn within(self) -> Duration {
        match self {
            Why::Wake | Why::Restarted => Duration::from_secs(10),
            Why::Fresh => Duration::from_millis(1500),
            Why::Failing | Why::Stuck => Duration::ZERO,
        }
    }
}

/// Off the UI thread, before a fresh scan: end the old one (SIGTERM, then
/// wait for it), then wait for the reader
pub fn fresh_scan(old: Option<Conv>, f: &dyn Fprint, why: Why) -> Probe {
    if let Some(c) = old {
        c.stop(Duration::from_secs(1));
    }
    wait_for_reader(f, why.within(), Duration::from_millis(300))
}

/// What the lock does next
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Next {
    /// start the scan now
    Start,
    /// try again after this long (the quick failures keep counting; 0: ask
    /// fprintd now)
    Later(u64),
    /// restart fprintd (if allowed now: `can_restart`; else `Hint`)
    Restart,
    /// say so quietly; look again in 30 s
    Hint,
}

/// After fprintd answered
pub fn after_probe(p: &Probe, why: Why, quick_fails: u32) -> Next {
    match (p, why) {
        // (failing at once though fprintd is fine: back off, never a fast loop)
        (Probe::Ready, Why::Failing) if quick_fails >= STUCK_AFTER => Next::Restart,
        (Probe::Ready, Why::Failing) => Next::Later(backoff_ms(quick_fails)),
        (Probe::Ready, _) => Next::Start,
        // (a restart that didn't help: the hint; while stuck, another
        // restart once 2 minutes have passed: `can_restart`)
        (Probe::Stuck(_), Why::Restarted) => Next::Hint,
        (Probe::Stuck(_), _) => Next::Restart,
        // the reader never came back after a wake: stuck
        (_, Why::Wake) => Next::Restart,
        (_, Why::Restarted | Why::Stuck) => Next::Hint,
        _ if quick_fails >= STUCK_AFTER => Next::Restart,
        _ => Next::Later(backoff_ms(quick_fails)),
    }
}

/// A scan that ended at once, before it asked for a finger (the reader
/// refused): the `quick_fails`th in a row
pub fn after_quick_fail(quick_fails: u32) -> Next {
    if quick_fails >= STUCK_AFTER {
        Next::Restart
    } else if quick_fails % CHECK_AFTER == 0 {
        Next::Later(0)
    } else {
        Next::Later(backoff_ms(quick_fails))
    }
}

/// The wait before the next try while the reader refuses: 0.3 s doubling,
/// up to 5 s
pub fn backoff_ms(quick_fails: u32) -> u64 {
    (300u64 << quick_fails.min(5)).min(5000)
}

/// fprintd may be restarted now (at most once in 2 minutes)
pub fn can_restart(now: i64, last: Option<i64>) -> bool {
    last.is_none_or(|t| now - t >= RESTART_EVERY_MS)
}

/// What a real miss leads to
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AfterMiss {
    /// the scan goes on ("Not recognised. Try again")
    Again,
    /// the scan ends; "Place your finger again"; a fresh one in 2 s
    Pause,
    /// the password only, until the next wake or unlock
    Done,
}

pub fn after_miss(misses: u32) -> AfterMiss {
    if misses >= MAX_MISSES {
        AfterMiss::Done
    } else if misses == PAUSE_AFTER {
        AfterMiss::Pause
    } else {
        AfterMiss::Again
    }
}

/// A scan should run now (the lock shown, the reader on: the lock's part)
pub fn listening(misses: u32, now: i64, activity_at: i64) -> bool {
    misses < MAX_MISSES && now - activity_at < IDLE_MS
}

/// The running scan is due for its renewal
pub fn renew_due(running_since: Option<i64>, now: i64) -> bool {
    running_since.is_some_and(|t| now - t >= RENEW_MS)
}

/// A key, the pointer or a touch: a fresh scan, unless one that's young and
/// listening runs (the pointer says so many times a second)
pub fn activity_restarts(running_since: Option<i64>, asked: bool, now: i64) -> bool {
    match running_since {
        None => true,
        Some(t) => !asked || now - t >= FRESH_MS,
    }
}

/// The lock holds the sleep inhibitor while a scan can run
pub fn hold_inhibitor(real: bool, held: bool, sleeping: bool, finger_on: bool, face_on: bool) -> bool {
    real && held && !sleeping && (finger_on || face_on)
}

/// How `before_sleep` went (for the log)
#[derive(Debug, PartialEq)]
pub struct Settled {
    /// every helper ended by itself in time (else it was killed)
    pub ended: bool,
    /// the reader's state when the sleep was let go (None: no reader check ran)
    pub reader: Option<Probe>,
}

/// Before a sleep (off the UI thread): end the scans, wait until fprintd
/// lets the reader go, then drop `inhibitor` (the sleep goes on), all within
/// `within`
pub fn before_sleep<I>(convs: Vec<Conv>, inhibitor: Option<I>, f: &dyn Fprint, within: Duration) -> Settled {
    let start = Instant::now();
    let left = || within.saturating_sub(start.elapsed()).max(Duration::from_millis(50));
    let finger = convs.iter().any(|c| c.kind == Kind::Finger);
    let mut ended = true;
    for c in &convs {
        ended &= c.stop(left());
    }
    let mut reader = None;
    if finger {
        // (the helper's bus connection closed with it: fprintd stops the read
        // and closes the reader on its own; a claim of ours works once it has)
        loop {
            let p = probe(f);
            let done = !matches!(p, Probe::Busy(_) | Probe::Gone(_));
            reader = Some(p);
            if done || start.elapsed() + Duration::from_millis(150) >= within {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    drop(convs);
    drop(inhibitor);
    Settled { ended, reader }
}

/// The screens' power (DRM connectors: name, on), as the kernel has it
pub fn screens() -> Vec<(String, bool)> {
    let mut out = Vec::new();
    let Ok(dir) = std::fs::read_dir("/sys/class/drm") else { return out };
    for e in dir.flatten() {
        let p = e.path();
        let status = std::fs::read_to_string(p.join("status")).unwrap_or_default();
        if status.trim() != "connected" {
            continue;
        }
        if let Ok(d) = std::fs::read_to_string(p.join("dpms")) {
            out.push((e.file_name().to_string_lossy().into_owned(), d.trim() == "On"));
        }
    }
    out.sort();
    out
}

/// A screen came on (it was off, or wasn't there, and is on now)
pub fn screen_came_on(before: &[(String, bool)], now: &[(String, bool)]) -> bool {
    now.iter().any(|(n, on)| *on && !before.iter().any(|(b, was)| b == n && *was))
        && !before.is_empty()
}

// ── The real fprintd and systemd ───────────────────────────────────────────

const FPRINT: &str = "net.reactivated.Fprint";

pub struct SystemFprint;

impl Fprint for SystemFprint {
    fn device(&self) -> Result<Option<String>, String> {
        let conn = zbus::blocking::Connection::system().map_err(|e| e.to_string())?;
        match conn.call_method(Some(FPRINT), "/net/reactivated/Fprint/Manager", Some("net.reactivated.Fprint.Manager"), "GetDevices", &()) {
            Ok(r) => r.body().deserialize::<Vec<zbus::zvariant::OwnedObjectPath>>().map(|v| v.first().map(|p| p.to_string())).map_err(|e| e.to_string()),
            Err(e) if e.to_string().contains("NoSuchDevice") => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    fn claim_release(&self, device: &str) -> Result<(), String> {
        let conn = zbus::blocking::Connection::system().map_err(|e| e.to_string())?;
        let dev = "net.reactivated.Fprint.Device";
        conn.call_method(Some(FPRINT), device, Some(dev), "Claim", &("",)).map_err(|e| e.to_string())?;
        conn.call_method(Some(FPRINT), device, Some(dev), "Release", &()).map(|_| ()).map_err(|e| e.to_string())
    }

    fn restart(&self) -> Result<(), String> {
        // systemd's manager over the system bus, never interactive: allowed
        // by Tessera's polkit rule (fprintd.service, restart, the person at
        // the computer), or refused at once; never a prompt over the lock
        let conn = zbus::blocking::Connection::system().map_err(|e| e.to_string())?;
        conn.call_method(Some("org.freedesktop.systemd1"), "/org/freedesktop/systemd1", Some("org.freedesktop.systemd1.Manager"), "RestartUnit", &("fprintd.service", "replace"))
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

/// After the unlock, while fprintd is still stuck: a notification whose
/// button restarts it (pkexec: the password, once)
pub fn offer_restart() {
    let title = "Fingerprint reader needs a restart";
    let body = "It stopped answering after the computer slept. Restart it to unlock with your finger again. To have Tessera do this by itself, run the one-time setup in Settings → Health.";
    if !tessera_core::util::on_path("notify-send") {
        return tessera_core::util::notify("Tessera", "auth-fingerprint-symbolic", "normal", title, body);
    }
    std::thread::spawn(move || {
        let out = std::process::Command::new("timeout")
            .args(["3600", "notify-send", "-a", "Tessera", "-i", "auth-fingerprint-symbolic", "-A", "restart=Restart the fingerprint reader", "--", title, body])
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output();
        if !out.is_ok_and(|o| String::from_utf8_lossy(&o.stdout).trim() == "restart") {
            return;
        }
        let ok = std::process::Command::new("pkexec")
            .args(["systemctl", "restart", "fprintd.service"])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
        if ok {
            tessera_core::util::notify("Tessera", "auth-fingerprint-symbolic", "low", "Fingerprint reader restarted", "You can unlock with your finger again.");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::sync::{Arc, Mutex};

    /// fprintd as a list of answers, and a log of what was asked
    #[derive(Default)]
    struct Fake {
        devices: Mutex<Vec<Result<Option<String>, String>>>,
        claims: Mutex<Vec<Result<(), String>>>,
        log: Arc<Mutex<Vec<String>>>,
    }

    impl Fake {
        fn new(devices: Vec<Result<Option<String>, String>>, claims: Vec<Result<(), String>>) -> Self {
            Self { devices: Mutex::new(devices), claims: Mutex::new(claims), log: Default::default() }
        }
        fn next<T: Clone>(v: &Mutex<Vec<T>>) -> T {
            let mut v = v.lock().unwrap();
            if v.len() > 1 { v.remove(0) } else { v[0].clone() }
        }
    }

    impl Fprint for Fake {
        fn device(&self) -> Result<Option<String>, String> {
            self.log.lock().unwrap().push("device".into());
            Self::next(&self.devices)
        }
        fn claim_release(&self, d: &str) -> Result<(), String> {
            self.log.lock().unwrap().push(format!("claim {d}"));
            Self::next(&self.claims)
        }
        fn restart(&self) -> Result<(), String> {
            self.log.lock().unwrap().push("restart".into());
            Ok(())
        }
    }

    const DEV: &str = "/net/reactivated/Fprint/Device/0";
    const STUCK: &str = "net.reactivated.Fprint.Error.Internal: Open failed with error: The device has already been opened!";

    #[test]
    fn fprintds_answers_classified() {
        assert_eq!(classify(STUCK), Probe::Stuck(STUCK.into()));
        assert!(matches!(classify("net.reactivated.Fprint.Error.Internal: Open failed with error: The device is still busy with another operation, please try again later."), Probe::Busy(_)));
        assert!(matches!(classify("net.reactivated.Fprint.Error.Internal: Claim failed"), Probe::Stuck(_)));
        assert!(matches!(classify("net.reactivated.Fprint.Error.AlreadyInUse: Device was already claimed"), Probe::Busy(_)));
        assert_eq!(classify("net.reactivated.Fprint.Error.NoSuchDevice: No devices available"), Probe::NoReader);
        assert!(matches!(classify("org.freedesktop.DBus.Error.ServiceUnknown: The name is not activatable"), Probe::Gone(_)));
        assert!(matches!(classify("org.freedesktop.DBus.Error.NoReply: Did not receive a reply"), Probe::Gone(_)));
    }

    #[test]
    fn a_probe_claims_and_lets_go() {
        assert_eq!(probe(&Fake::new(vec![Ok(Some(DEV.into()))], vec![Ok(())])), Probe::Ready);
        assert_eq!(probe(&Fake::new(vec![Ok(None)], vec![Ok(())])), Probe::NoReader);
        assert_eq!(probe(&Fake::new(vec![Ok(Some(DEV.into()))], vec![Err(STUCK.into())])), Probe::Stuck(STUCK.into()));
        // (no reader: no claim)
        let f = Fake::new(vec![Ok(None)], vec![Ok(())]);
        probe(&f);
        assert_eq!(*f.log.lock().unwrap(), vec!["device"]);
    }

    #[test]
    fn after_a_wake_it_waits_for_the_reader() {
        // off USB twice, busy once, then there
        let f = Fake::new(vec![Ok(None), Ok(None), Ok(Some(DEV.into()))], vec![Err("AlreadyInUse".into()), Ok(())]);
        assert_eq!(wait_for_reader(&f, Duration::from_secs(5), Duration::from_millis(5)), Probe::Ready);
        assert_eq!(f.log.lock().unwrap().iter().filter(|l| *l == "device").count(), 4);
        // stuck: says so at once, no more waiting
        let f = Fake::new(vec![Ok(Some(DEV.into()))], vec![Err(STUCK.into())]);
        let t = Instant::now();
        assert!(matches!(wait_for_reader(&f, Duration::from_secs(5), Duration::from_millis(200)), Probe::Stuck(_)));
        assert!(t.elapsed() < Duration::from_millis(150));
        // never back: gives up when the time is up, with the last answer
        let f = Fake::new(vec![Ok(None)], vec![Ok(())]);
        let t = Instant::now();
        assert_eq!(wait_for_reader(&f, Duration::from_millis(100), Duration::from_millis(10)), Probe::NoReader);
        assert!(t.elapsed() < Duration::from_millis(500));
        // (no wait asked: one look)
        let f = Fake::new(vec![Ok(None)], vec![Ok(())]);
        wait_for_reader(&f, Duration::ZERO, Duration::from_millis(500));
        assert_eq!(f.log.lock().unwrap().len(), 1);
    }

    #[test]
    fn a_fresh_scan_ends_the_old_one_before_it_asks_fprintd() {
        let f = Fake::new(vec![Ok(Some(DEV.into()))], vec![Ok(())]);
        let old = Conv::of_child(Kind::Finger, 3, Command::new("sleep").arg("30").spawn().unwrap());
        let t = Instant::now();
        assert_eq!(fresh_scan(Some(old), &f, Why::Fresh), Probe::Ready);
        assert!(t.elapsed() < Duration::from_secs(1));
        assert_eq!(f.log.lock().unwrap().len(), 2);
        assert_eq!(Why::Wake.within(), Duration::from_secs(10));
        assert_eq!(Why::Stuck.within(), Duration::ZERO);
    }

    /// An inhibitor that notes when it's let go
    struct Inhibitor(Arc<Mutex<Vec<String>>>);
    impl Drop for Inhibitor {
        fn drop(&mut self) {
            self.0.lock().unwrap().push("sleep let go".into());
        }
    }

    #[test]
    fn before_a_sleep_the_scan_ends_and_the_reader_is_let_go_first() {
        let f = Fake::new(vec![Ok(Some(DEV.into()))], vec![Err("net.reactivated.Fprint.Error.AlreadyInUse: Device was already claimed".into()), Ok(())]);
        let log = f.log.clone();
        // (a helper that notes its end, through a file)
        let dir = std::env::temp_dir().join(format!("tessera-reader-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mark = dir.join("ended");
        let helper = Command::new("sh").args(["-c", &format!("trap 'touch {}; exit 0' TERM; while :; do sleep 0.05; done", mark.display())]).spawn().unwrap();
        std::thread::sleep(Duration::from_millis(100));
        let conv = Conv::of_child(Kind::Finger, 7, helper);
        let s = before_sleep(vec![conv], Some(Inhibitor(log.clone())), &f, Duration::from_secs(2));
        assert!(s.ended);
        assert_eq!(s.reader, Some(Probe::Ready));
        assert!(mark.exists(), "the helper had SIGTERM and ended by itself");
        // the helper ended, then fprintd was asked (still claimed once, then free), then the sleep went on
        let l = log.lock().unwrap().clone();
        let claim = format!("claim {DEV}");
        assert_eq!(l, vec!["device", claim.as_str(), "device", claim.as_str(), "sleep let go"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn before_a_sleep_it_never_holds_the_sleep_long() {
        // fprintd busy the whole time: let go anyway within the time
        let f = Fake::new(vec![Ok(Some(DEV.into()))], vec![Err("still busy with another operation".into())]);
        let log = f.log.clone();
        let t = Instant::now();
        let conv = Conv::of_child(Kind::Finger, 1, Command::new("sleep").arg("30").spawn().unwrap());
        let s = before_sleep(vec![conv], Some(Inhibitor(log.clone())), &f, Duration::from_millis(600));
        assert!(t.elapsed() < Duration::from_millis(900));
        assert!(matches!(s.reader, Some(Probe::Busy(_))));
        assert_eq!(log.lock().unwrap().last().unwrap(), "sleep let go");
        // a face scan only: no reader check, let go at once
        let f = Fake::new(vec![Ok(Some(DEV.into()))], vec![Ok(())]);
        let log = f.log.clone();
        let conv = Conv::of_child(Kind::Face, 2, Command::new("sleep").arg("30").spawn().unwrap());
        let s = before_sleep(vec![conv], Some(Inhibitor(log.clone())), &f, Duration::from_secs(2));
        assert_eq!(s.reader, None);
        assert_eq!(*log.lock().unwrap(), vec!["sleep let go"]);
        // nothing running: let go at once
        let f = Fake::new(vec![Ok(Some(DEV.into()))], vec![Ok(())]);
        let log = f.log.clone();
        let s = before_sleep(Vec::new(), Some(Inhibitor(log.clone())), &f, Duration::from_secs(2));
        assert_eq!(s, Settled { ended: true, reader: None });
        assert_eq!(*log.lock().unwrap(), vec!["sleep let go"]);
    }

    #[test]
    fn the_inhibitor_only_while_a_scan_can_run() {
        assert!(hold_inhibitor(true, true, false, true, false));
        assert!(hold_inhibitor(true, true, false, false, true));
        assert!(!hold_inhibitor(true, true, true, true, true)); // (going to sleep: let go)
        assert!(!hold_inhibitor(true, false, false, true, true)); // (not locked)
        assert!(!hold_inhibitor(false, true, false, true, true)); // (demo, sandbox)
        assert!(!hold_inhibitor(true, true, false, false, false)); // (no reader, no face)
    }

    #[test]
    fn three_misses_a_pause_three_more_then_the_password() {
        let seq: Vec<AfterMiss> = (1..=6).map(after_miss).collect();
        assert_eq!(seq, vec![AfterMiss::Again, AfterMiss::Again, AfterMiss::Pause, AfterMiss::Again, AfterMiss::Again, AfterMiss::Done]);
        assert_eq!(after_miss(9), AfterMiss::Done);
        assert_eq!(PAUSE_MS, 2000);
        // six: no scan, however recent the activity
        assert!(listening(5, 1000, 1000));
        assert!(!listening(6, 1000, 1000));
    }

    #[test]
    fn two_idle_minutes_stop_the_scan() {
        assert!(listening(0, 119_999, 0));
        assert!(!listening(0, 120_000, 0));
        // (any activity starts it again: activity_at moves)
        assert!(listening(0, 120_000, 119_000));
    }

    #[test]
    fn one_scan_renewed_every_30_s() {
        assert!(!renew_due(None, 100_000));
        assert!(!renew_due(Some(0), 29_999));
        assert!(renew_due(Some(0), 30_000));
    }

    #[test]
    fn a_key_or_the_pointer_brings_a_fresh_scan() {
        // none running, or one that never asked for a finger: fresh
        assert!(activity_restarts(None, false, 0));
        assert!(activity_restarts(Some(0), false, 500));
        // a young one that's listening stays (the pointer says so often)
        assert!(!activity_restarts(Some(0), true, 9_999));
        assert!(activity_restarts(Some(0), true, 10_000));
    }

    #[test]
    fn a_screen_coming_on() {
        let s = |v: &[(&str, bool)]| v.iter().map(|(n, o)| (n.to_string(), *o)).collect::<Vec<_>>();
        assert!(screen_came_on(&s(&[("card1-eDP-1", false)]), &s(&[("card1-eDP-1", true)])));
        assert!(!screen_came_on(&s(&[("card1-eDP-1", true)]), &s(&[("card1-eDP-1", true)])));
        assert!(!screen_came_on(&s(&[("card1-eDP-1", true)]), &s(&[("card1-eDP-1", false)])));
        // a monitor plugged in, already on
        assert!(screen_came_on(&s(&[("card1-eDP-1", true)]), &s(&[("card1-eDP-1", true), ("card1-DP-1", true)])));
        // (the first reading isn't a change)
        assert!(!screen_came_on(&[], &s(&[("card1-eDP-1", true)])));
    }

    #[test]
    fn backing_off_never_loops_fast_for_good() {
        assert_eq!(backoff_ms(1), 600);
        assert_eq!(backoff_ms(3), 2400);
        assert_eq!(backoff_ms(5), 5000);
        assert_eq!(backoff_ms(40), 5000);
        // every few quick failures fprintd is asked; then it's stuck
        let seq: Vec<Next> = (1..=STUCK_AFTER).map(after_quick_fail).collect();
        assert_eq!(seq[0], Next::Later(600));
        assert_eq!(seq[CHECK_AFTER as usize - 1], Next::Later(0));
        assert_eq!(*seq.last().unwrap(), Next::Restart);
        assert_eq!(STUCK_EVERY_MS, 30_000);
    }

    #[test]
    fn restarts_at_most_once_in_two_minutes() {
        assert!(can_restart(5_000, None));
        assert!(!can_restart(100_000, Some(0)));
        assert!(can_restart(120_000, Some(0)));
    }

    #[test]
    fn what_follows_fprintds_answer() {
        let stuck = Probe::Stuck(STUCK.into());
        assert_eq!(after_probe(&Probe::Ready, Why::Wake, 0), Next::Start);
        assert_eq!(after_probe(&Probe::Ready, Why::Fresh, 0), Next::Start);
        // stuck: a restart (when allowed), then the hint
        assert_eq!(after_probe(&stuck, Why::Wake, 0), Next::Restart);
        assert_eq!(after_probe(&stuck, Why::Fresh, 0), Next::Restart);
        assert_eq!(after_probe(&stuck, Why::Failing, 3), Next::Restart);
        assert_eq!(after_probe(&stuck, Why::Restarted, 0), Next::Hint);
        // the reader never came back after a wake (10 s): stuck too
        assert_eq!(after_probe(&Probe::NoReader, Why::Wake, 0), Next::Restart);
        assert_eq!(after_probe(&Probe::Busy("x".into()), Why::Wake, 0), Next::Restart);
        assert_eq!(after_probe(&Probe::NoReader, Why::Restarted, 0), Next::Hint);
        // while stuck: only a working claim ends it
        assert_eq!(after_probe(&Probe::Busy("x".into()), Why::Stuck, 8), Next::Hint);
        assert_eq!(after_probe(&Probe::Ready, Why::Stuck, 8), Next::Start);
        // failing at once though fprintd is fine: back off (no fast loop), then stuck
        assert_eq!(after_probe(&Probe::Ready, Why::Failing, 3), Next::Later(2400));
        assert_eq!(after_probe(&Probe::Ready, Why::Failing, STUCK_AFTER), Next::Restart);
        // busy for a moment on a fresh start: a little later
        assert_eq!(after_probe(&Probe::Busy("x".into()), Why::Fresh, 1), Next::Later(600));
    }

    #[test]
    fn the_18_nov_story() {
        // woke; fprintd holds the reader open; restarted (allowed); back
        assert_eq!(after_probe(&Probe::Stuck(STUCK.into()), Why::Wake, 0), Next::Restart);
        assert!(can_restart(1_000_000, None));
        assert_eq!(after_probe(&Probe::Ready, Why::Restarted, 0), Next::Start);
        // without the rule (refused): the hint, a look every 30 s (a restart
        // asked again at most every 2 minutes), the finger back once a claim works
        assert_eq!(after_probe(&Probe::Stuck("org.freedesktop.DBus.Error.InteractiveAuthorizationRequired".into()), Why::Restarted, 0), Next::Hint);
        assert_eq!(after_probe(&Probe::Stuck(STUCK.into()), Why::Stuck, 0), Next::Restart);
        assert!(!can_restart(1_030_000, Some(1_000_000)));
        assert_eq!(after_probe(&Probe::Ready, Why::Stuck, 0), Next::Start);
    }
}

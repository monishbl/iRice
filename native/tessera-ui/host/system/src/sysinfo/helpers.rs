//! Shared bits of the sysinfo group: a thread-safe sender, a timer that
//! sends a tick, a process watcher that ends with the shell (watch.sh, as
//! Tools.watcher), Hyprland's event socket, island posts, version maths.

use serde_json::{json, Value};
use std::any::Any;
use std::io::{BufRead, BufReader};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tessera_core::host::Host;

/// A service's way to hear from its threads (Host::sender, shareable)
pub type Tx = Arc<dyn Fn(Box<dyn Any + Send>) + Send + Sync>;

pub fn tx(h: &Host, service: &'static str) -> Tx {
    let s = Mutex::new(h.sender(service));
    Arc::new(move |m| {
        if let Ok(f) = s.lock() {
            f(m)
        }
    })
}

/// "this timer fired", for a service's `message`
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tick(pub &'static str);

/// A one-shot / repeating timer that says `Tick(name)` to its service
/// (a QML `Timer { id: name }`). `restart` re-arms; `stop` cancels.
#[derive(Default)]
pub struct Ticker {
    t: slint::Timer,
    name: &'static str,
}

impl Ticker {
    pub fn new(name: &'static str) -> Self {
        Self { t: slint::Timer::default(), name }
    }
    pub fn once(&self, tx: &Tx, ms: u64) {
        let (tx, name) = (tx.clone(), self.name);
        self.t.start(slint::TimerMode::SingleShot, Duration::from_millis(ms), move || tx(Box::new(Tick(name))));
    }
    pub fn every(&self, tx: &Tx, ms: u64) {
        let (tx, name) = (tx.clone(), self.name);
        self.t.start(slint::TimerMode::Repeated, Duration::from_millis(ms), move || tx(Box::new(Tick(name))));
    }
    pub fn stop(&self) {
        self.t.stop();
    }
    pub fn running(&self) -> bool {
        self.t.running()
    }
}

pub fn now_ms() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as f64).unwrap_or(0.0)
}
pub fn now_s() -> f64 {
    now_ms() / 1000.0
}

/// A long-lived watcher script, as the QML runs them: through
/// shell/assets/scripts/watch.sh (it ends when our stdin closes: the shell
/// quit). Each stdout line goes to `on_line`; `on_exit` when it ends.
pub fn watch(cmd: Vec<String>, on_line: impl Fn(String) + Send + 'static, on_exit: impl FnOnce() + Send + 'static) {
    std::thread::spawn(move || {
        use std::process::{Command, Stdio};
        let mut c = Command::new(crate::util::script("watch.sh"));
        c.args(&cmd).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
        if let Ok(mut child) = c.spawn() {
            let _stdin = child.stdin.take(); // (kept open until the watcher ends)
            if let Some(out) = child.stdout.take() {
                for line in BufReader::new(out).lines().map_while(Result::ok) {
                    on_line(line);
                }
            }
            let _ = child.wait();
        }
        on_exit();
    });
}

/// Hyprland's event socket (Hyprland.onRawEvent): `on_event(name, data)` for
/// the events named in `only` (all if empty). Reconnects while the shell runs.
pub fn hypr_events(only: &'static [&'static str], on_event: impl Fn(String, String) + Send + 'static) {
    std::thread::spawn(move || loop {
        let sig = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").unwrap_or_default();
        if !sig.is_empty() {
            let path = crate::util::runtime_dir().join("hypr").join(&sig).join(".socket2.sock");
            if let Ok(s) = std::os::unix::net::UnixStream::connect(path) {
                for line in BufReader::new(s).lines().map_while(Result::ok) {
                    if let Some((name, data)) = line.split_once(">>") {
                        if only.is_empty() || only.contains(&name) {
                            on_event(name.to_string(), data.to_string());
                        }
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_secs(3));
    });
}

/// The island's post (Island.post(e, ms)): the host-shell's island service
/// answers "island.post" with {event, ms}. An `action` button carries
/// `onAction: {call, args}` for the service to run when it is pressed.
pub fn island_post(h: &mut Host, event: Value, ms: u64) {
    h.ask("island.post", json!({"event": event, "ms": ms}));
}

/// Island.enabled (config island.enabled, default true)
pub fn island_enabled(h: &Host) -> bool {
    h.cfg()["island"]["enabled"].as_bool().unwrap_or(true)
}

/// Island.wants(kind): the kinds that pop it (config island.events; the
/// default list when unset, newer kinds on until switched)
pub fn island_wants(h: &Host, kind: &str) -> bool {
    const DEFAULT: [&str; 11] = ["music", "charging", "battery", "timers", "recording", "focus", "bluetooth", "resources", "usb", "displays", "audio"];
    const NEW_KINDS: [&str; 1] = ["sysupdate"];
    let c = &h.cfg()["island"];
    let events: Vec<&str> = match c["events"].as_array() {
        Some(a) => a.iter().filter_map(Value::as_str).collect(),
        None => DEFAULT.to_vec(),
    };
    let known: Vec<&str> = c["known"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    events.contains(&kind) || (NEW_KINDS.contains(&kind) && !known.contains(&kind))
}

pub fn notify_send(args: &[&str]) {
    let mut v = vec!["notify-send"];
    v.extend_from_slice(args);
    tessera_core::services::bridge::run(&v);
}

/// QML `sandboxed` / TESSERA_SANDBOX (or the old IRICE_ name)
pub fn env_is(name: &str) -> bool {
    std::env::var(format!("TESSERA_{name}")).or_else(|_| std::env::var(format!("IRICE_{name}"))).map(|v| v == "1").unwrap_or(false)
}

// ── Versions (SoftwareUpdate.parts / cmp) ───────────────────────────────

/// "1:0.56.2-4.1" -> [0, 56, 2]; "0.56.x" matches any 0.56 (x = -1)
pub fn parts(v: &str) -> Vec<i64> {
    let v = match v.split_once(':') {
        Some((e, rest)) if !e.is_empty() && e.chars().all(|c| c.is_ascii_digit()) => rest,
        _ => v,
    };
    let v = match v.rfind('-') {
        Some(i) => &v[..i],
        None => v,
    };
    v.split(['.', '+']).map(|x| if x == "x" { -1 } else { leading_int(x) }).collect()
}

/// JavaScript's parseInt(x) || 0
fn leading_int(x: &str) -> i64 {
    let t = x.trim_start();
    let (neg, t) = match t.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    let d: String = t.chars().take_while(char::is_ascii_digit).collect();
    let n: i64 = d.parse().unwrap_or(0);
    if neg {
        -n
    } else {
        n
    }
}

/// b may hold wildcards (-1)
pub fn cmp(a: &[i64], b: &[i64]) -> i32 {
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (a.get(i).copied().unwrap_or(0), b.get(i).copied().unwrap_or(0));
        if y == -1 {
            return 0;
        }
        if x != y {
            return if x < y { -1 } else { 1 };
        }
    }
    0
}

/// Replace $HOME with ~ (RustBridgeAbout.home)
pub fn home_tilde(p: &str) -> String {
    let h = crate::util::home().to_string_lossy().into_owned();
    if h.is_empty() || h == "/" {
        return p.to_string();
    }
    p.replacen(&h, "~", 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn versions() {
        assert_eq!(parts("1:0.56.2-4.1"), vec![0, 56, 2]);
        assert_eq!(parts("0.56.x"), vec![0, 56, -1]);
        assert_eq!(cmp(&parts("0.56.9-1"), &parts("0.56.x")), 0);
        assert_eq!(cmp(&parts("0.57.0-1"), &parts("0.56.x")), 1);
        assert_eq!(cmp(&parts("0.55.0-1"), &parts("0.56.0")), -1);
    }
}

// ── Local time (Qt.formatTime / formatDateTime) ─────────────────────────

fn local(secs: i64) -> libc::tm {
    let t: libc::time_t = secs as libc::time_t;
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe { libc::localtime_r(&t, &mut tm) };
    tm
}

/// TimeFormat.hm: "09:41" (24-hour) or "9:41 AM"
pub fn fmt_hm(secs: i64, clock24: bool) -> String {
    let tm = local(secs);
    if clock24 {
        format!("{:02}:{:02}", tm.tm_hour, tm.tm_min)
    } else {
        format!("{}:{:02} {}", (tm.tm_hour + 11) % 12 + 1, tm.tm_min, if tm.tm_hour >= 12 { "PM" } else { "AM" })
    }
}

const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
const DAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];

/// "d MMMM, h:mm ap" (the update to go back from, long)
pub fn fmt_long(secs: i64) -> String {
    let tm = local(secs);
    format!("{} {}, {}", tm.tm_mday, MONTHS[(tm.tm_mon as usize) % 12], fmt_hm(secs, false).to_lowercase())
}

/// "dddd h:mm ap" (the same, by day)
pub fn fmt_day(secs: i64) -> String {
    let tm = local(secs);
    format!("{} {}", DAYS[(tm.tm_wday as usize) % 7], fmt_hm(secs, false).to_lowercase())
}

/// JavaScript truthiness of a JSON value
pub fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        Value::String(s) => !s.is_empty(),
        _ => true,
    }
}

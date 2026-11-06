//! Helpers the accounts group shares (processes with stderr and stdin, a
//! stream of lines, the root helper's command line, local dates). Kept here:
//! the crate-wide util.rs is another agent's.

use serde_json::{json, Value};
use std::any::Any;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};

/// A service's way to message itself from a thread (Host::sender), clonable
#[derive(Clone)]
pub struct Tx(Arc<Mutex<Box<dyn Fn(Box<dyn Any + Send>) + Send>>>);

impl Tx {
    pub fn new(f: impl Fn(Box<dyn Any + Send>) + Send + 'static) -> Self {
        Tx(Arc::new(Mutex::new(Box::new(f))))
    }
    pub fn send<T: Any + Send>(&self, t: T) {
        if let Ok(f) = self.0.lock() {
            f(Box::new(t));
        }
    }
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

/// Tools.has(cmd): is it on PATH
pub fn has_tool(cmd: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(cmd).is_file()))
}

/// The helper script the QML runs (shell/assets/scripts/<name>)
pub fn script(name: &str) -> String {
    crate::util::script(name)
}

/// RootHelper.pick: the helper that's there (Tessera's, else iRice's) as $h
pub const ROOT_PICK: &str = "h=/usr/lib/tessera/tessera-root; policy=/usr/share/polkit-1/actions/org.tessera.root.policy; if [ ! -x \"$h\" ] && [ -x /usr/lib/irice/irice-root ]; then h=/usr/lib/irice/irice-root; policy=/usr/share/polkit-1/actions/org.irice.root.policy; fi; ";

/// RootHelper.command(args): ["sh", "-c", pick + 'exec pkexec "$h" "$@"', "sh", args…]
/// (this only builds the command line; the QML runs it on the user's click)
pub fn root_command(args: &[&str]) -> Vec<String> {
    let mut v: Vec<String> = vec!["sh".into(), "-c".into(), format!("{ROOT_PICK}exec pkexec \"$h\" \"$@\""), "sh".into()];
    v.extend(args.iter().map(|s| s.to_string()));
    v
}

/// RootHelper.knows(subcommand): a shell test, for `sh -c`
pub fn root_knows(sub: &str) -> String {
    format!("{ROOT_PICK}[ -x \"$h\" ] && grep -q \"\\.root\\.{sub}\\\"\" \"$policy\" 2>/dev/null")
}

/// RootHelper.message: without the helper's "tessera-root: " (or "irice-root: ")
pub fn root_message(text: &str) -> String {
    let t = text.trim();
    for p in ["tessera-root: ", "irice-root: "] {
        if let Some(r) = t.strip_prefix(p) {
            return r.trim().to_string();
        }
    }
    t.to_string()
}

pub const NO_PKEXEC: &str = "Changing this needs polkit (pkexec), which isn't installed. Install polkit, then try again.";

/// Run to the end (call on a thread): exit code, stdout, stderr. `stdin` is
/// written once, then closed.
pub fn run_full(cmd: &[String], stdin: Option<&str>) -> (i32, String, String) {
    let Some(first) = cmd.first() else { return (-1, String::new(), String::new()) };
    let mut c = Command::new(first);
    c.args(&cmd[1..]).stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() }).stdout(Stdio::piped()).stderr(Stdio::piped());
    let Ok(mut child) = c.spawn() else { return (-1, String::new(), String::new()) };
    if let (Some(text), Some(mut si)) = (stdin, child.stdin.take()) {
        let _ = si.write_all(text.as_bytes());
    }
    let mut err = child.stderr.take();
    let et = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(e) = err.as_mut() {
            let _ = e.read_to_string(&mut s);
        }
        s
    });
    let mut out = String::new();
    if let Some(mut o) = child.stdout.take() {
        let _ = o.read_to_string(&mut out);
    }
    let code = child.wait().ok().and_then(|s| s.code()).unwrap_or(-1);
    (code, out, et.join().unwrap_or_default())
}

pub fn strs(cmd: &[&str]) -> Vec<String> {
    cmd.iter().map(|s| s.to_string()).collect()
}

/// A running child whose lines stream back: `stop()` sends SIGTERM to it
/// (our own child only)
#[derive(Clone, Default)]
pub struct Child(Arc<AtomicI32>);

impl Child {
    pub fn running(&self) -> bool {
        self.0.load(Ordering::SeqCst) > 0
    }
    pub fn stop(&self) {
        let pid = self.0.load(Ordering::SeqCst);
        if pid > 0 {
            unsafe { libc::kill(pid, libc::SIGTERM) };
        }
    }
    /// Start `cmd`; `line` for each stdout line, `exit` once with the code.
    /// Both run on the child's thread. False if it could not start.
    pub fn spawn(&self, cmd: &[String], stdin: Option<String>, line: impl Fn(String) + Send + 'static, exit: impl FnOnce(i32) + Send + 'static) -> bool {
        let Some(first) = cmd.first() else { return false };
        let mut c = Command::new(first);
        c.args(&cmd[1..]).stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() }).stdout(Stdio::piped()).stderr(Stdio::null());
        let Ok(mut child) = c.spawn() else { return false };
        self.0.store(child.id() as i32, Ordering::SeqCst);
        if let (Some(text), Some(mut si)) = (stdin, child.stdin.take()) {
            let _ = si.write_all(text.as_bytes());
        }
        let pid = self.0.clone();
        std::thread::spawn(move || {
            if let Some(o) = child.stdout.take() {
                for l in BufReader::new(o).lines().map_while(Result::ok) {
                    line(l);
                }
            }
            let code = child.wait().ok().and_then(|s| s.code()).unwrap_or(-1);
            pid.store(0, Ordering::SeqCst);
            exit(code);
        });
        true
    }
}

/// Qt.formatDate(d, "yyyy-MM-dd") for a time in ms (local time)
pub fn day_of(ms: i64) -> String {
    strftime(ms, "%Y-%m-%d")
}

/// Qt.formatDate(d, "ddd").slice(0, 1): the weekday's first letter (local)
pub fn weekday_letter(ms: i64) -> String {
    strftime(ms, "%a").chars().take(1).collect()
}

fn strftime(ms: i64, fmt: &str) -> String {
    unsafe {
        let t: libc::time_t = (ms / 1000) as libc::time_t;
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        let mut buf = [0u8; 64];
        let f = std::ffi::CString::new(fmt).unwrap_or_default();
        let n = libc::strftime(buf.as_mut_ptr() as *mut libc::c_char, buf.len(), f.as_ptr(), &tm);
        String::from_utf8_lossy(&buf[..n]).into_owned()
    }
}

/// An island alert (Island.post(e, ms)); host-shell's Island answers
/// "island.post" with {event, ms}. `action`: a button whose click asks the
/// host `{ask, args}` (the QML's onAction closure).
pub fn island_post(h: &mut tessera_core::host::Host, event: Value, ms: i64) {
    h.ask("island.post", json!({"event": event, "ms": ms}));
}

/// The bus' idle flag (a services::idle reading, see CORE-NEEDS): false
/// until something publishes it
pub fn idle() -> bool {
    crate::bus::field("idle", "isIdle").as_bool().unwrap_or(false)
}

/// Idle for 5 minutes (Google.qml's and CloudAccounts.qml's monitors; `idle()` is
/// ScreenTime's 2)
pub fn idle_long() -> bool {
    crate::bus::field("idle", "isIdleLong").as_bool().unwrap_or(false)
}

/// System.locked (the sysinfo group publishes bus "system")
pub fn locked() -> bool {
    crate::bus::field("system", "locked").as_bool().unwrap_or(false)
}

/// Updates.running (the sysinfo group publishes bus "updates")
pub fn updates_running() -> bool {
    crate::bus::field("updates", "running").as_bool().unwrap_or(false)
}

/// Publish a service's state: the bus and nothing else
pub fn put(name: &'static str, v: Value) {
    crate::bus::put(name, v);
}

pub fn first_line(s: &str) -> &str {
    s.lines().next().unwrap_or("")
}

/// new Date(y, m-1, d, h, mi) in the local zone, as ms
pub fn local_ms(y: i32, mo: i32, d: i32, h: i32, mi: i32) -> i64 {
    unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        tm.tm_year = y - 1900;
        tm.tm_mon = mo - 1;
        tm.tm_mday = d;
        tm.tm_hour = h;
        tm.tm_min = mi;
        tm.tm_isdst = -1;
        libc::mktime(&mut tm) as i64 * 1000
    }
}

/// "YYYY-MM-DD" at 23:59 local (a task's due time), 0 for no date
pub fn due_of(date: &str) -> i64 {
    let p: Vec<i32> = date.split('-').filter_map(|x| x.parse().ok()).collect();
    if p.len() == 3 { local_ms(p[0], p[1], p[2], 23, 59) } else { 0 }
}

/// new Date(ms).toISOString()
pub fn iso_ms(ms: i64) -> String {
    unsafe {
        let t: libc::time_t = ms.div_euclid(1000) as libc::time_t;
        let mut tm: libc::tm = std::mem::zeroed();
        libc::gmtime_r(&t, &mut tm);
        format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z", tm.tm_year + 1900, tm.tm_mon + 1, tm.tm_mday, tm.tm_hour, tm.tm_min, tm.tm_sec, ms.rem_euclid(1000))
    }
}

/// new Date(rfc3339).getTime() ("2025-01-02T03:04:05(.fff)(Z|+hh:mm)")
pub fn parse_iso(s: &str) -> Option<i64> {
    let (date, rest) = s.split_once('T')?;
    let d: Vec<i64> = date.split('-').filter_map(|x| x.parse().ok()).collect();
    if d.len() != 3 {
        return None;
    }
    let (clock, off) = match rest.find(['Z', '+']).or_else(|| rest.rfind('-')) {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "Z"),
    };
    let (hms, frac) = clock.split_once('.').unwrap_or((clock, "0"));
    let t: Vec<i64> = hms.split(':').filter_map(|x| x.parse().ok()).collect();
    if t.len() < 2 {
        return None;
    }
    // days from civil
    let (y, m, dd) = (d[0] - if d[1] <= 2 { 1 } else { 0 }, d[1], d[2]);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + dd - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    let ms3: i64 = format!("{:0<3}", frac).chars().take(3).collect::<String>().parse().unwrap_or(0);
    let mut ms = ((days * 24 + t[0]) * 60 + t[1]) * 60_000 + t.get(2).copied().unwrap_or(0) * 1000 + ms3;
    if off.len() >= 6 && (off.starts_with('+') || off.starts_with('-')) {
        let oh: i64 = off[1..3].parse().ok()?;
        let om: i64 = off[4..6].parse().ok()?;
        let o = (oh * 60 + om) * 60_000;
        ms -= if off.starts_with('+') { o } else { -o };
    }
    Some(ms)
}

/// A JSON string field
pub fn js(v: &Value, k: &str) -> String {
    v[k].as_str().unwrap_or("").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn root_messages_lose_their_prefix() {
        assert_eq!(root_message("tessera-root: nope\n"), "nope");
        assert_eq!(root_message("irice-root: nope"), "nope");
        assert_eq!(root_message(" plain "), "plain");
    }
    #[test]
    fn root_command_shape() {
        let c = root_command(&["face", "add"]);
        assert_eq!(c[0], "sh");
        assert!(c[2].ends_with("exec pkexec \"$h\" \"$@\""));
        assert_eq!(&c[4..], ["face", "add"]);
    }
    #[test]
    fn iso_round_trip() {
        assert_eq!(iso_ms(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(parse_iso("1970-01-01T00:00:00.000Z"), Some(0));
        assert_eq!(parse_iso("2025-03-04T05:06:07+02:00"), Some(parse_iso("2025-03-04T03:06:07Z").unwrap()));
        assert_eq!(parse_iso(&iso_ms(1_700_000_123_456)), Some(1_700_000_123_456));
        assert_eq!(parse_iso("2025-03-04T05:06:07-05:00"), Some(parse_iso("2025-03-04T10:06:07Z").unwrap()));
    }
    #[test]
    fn dates() {
        assert_eq!(day_of(0).len(), 10);
        assert_eq!(weekday_letter(0).len(), 1);
    }
}

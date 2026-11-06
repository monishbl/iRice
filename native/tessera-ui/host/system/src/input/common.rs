//! Helpers the input group shares: Hyprland's raw events (the QML's
//! `Hyprland.onRawEvent`), timers that come back as service messages, and
//! `hyprctl` calls on a thread. Nothing here blocks the loop.

use std::any::Any;
use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixStream;
use std::time::Duration;

/// One line of Hyprland's event socket (socket2): `name>>data`
#[derive(Clone, Debug, PartialEq)]
pub struct Raw {
    pub name: String,
    pub data: String,
}

pub fn parse_raw(line: &str) -> Option<Raw> {
    let (n, d) = line.trim_end().split_once(">>")?;
    Some(Raw { name: n.to_string(), data: d.to_string() })
}

/// Watch Hyprland's event socket on a thread; every event goes to `send` as a
/// `Raw` (reconnects when Hyprland restarts). `names`: only those events
/// (empty: all).
pub fn spawn_hypr_events(names: &'static [&'static str], send: impl Fn(Box<dyn Any + Send>) + Send + 'static) {
    std::thread::spawn(move || loop {
        let Some(path) = tessera_core::services::hypr::dir().map(|d| d.join(".socket2.sock")) else {
            std::thread::sleep(Duration::from_secs(2));
            continue;
        };
        let Ok(stream) = UnixStream::connect(path) else {
            std::thread::sleep(Duration::from_secs(1));
            continue;
        };
        let mut r = BufReader::new(stream);
        let mut line = String::new();
        loop {
            line.clear();
            match r.read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            if let Some(e) = parse_raw(&line) {
                if names.is_empty() || names.contains(&e.name.as_str()) {
                    send(Box::new(e));
                }
            }
        }
        std::thread::sleep(Duration::from_millis(500));
    });
}

/// After `ms`, deliver `msg` through `send` (a slint timer: on the loop)
pub fn after<M: Any + Send + 'static>(send: impl Fn(Box<dyn Any + Send>) + 'static, ms: u64, msg: M) {
    slint::Timer::single_shot(Duration::from_millis(ms), move || send(Box::new(msg)));
}

/// `hyprctl <args…>` on a thread, output ignored (Quickshell.execDetached)
pub fn hyprctl(args: &[&str]) {
    let mut v = vec!["hyprctl"];
    v.extend_from_slice(args);
    crate::util::run_async(&v);
}

/// `hyprctl eval <lua>` (Hyprland's Lua config)
pub fn hyprctl_eval(lua: &str) {
    hyprctl(&["eval", lua]);
}

/// A Lua string literal (Displays.luaStr)
pub fn lua_str(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// `hyprctl -j <what>` as JSON (blocking: on a thread)
pub fn hyprctl_json(args: &[&str]) -> serde_json::Value {
    let mut v = vec!["hyprctl", "-j"];
    v.extend_from_slice(args);
    crate::util::capture(&v, Duration::from_secs(5)).and_then(|(_, o)| serde_json::from_str(&o).ok()).unwrap_or(serde_json::Value::Null)
}

/// Milliseconds since the epoch (Date.now())
pub fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// A JS-style `Math.round` for numbers (halves go up)
pub fn round(x: f64) -> i64 {
    (x + 0.5).floor() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn raw_events() {
        assert_eq!(parse_raw("monitoraddedv2>>1,DP-1,Dell\n"), Some(Raw { name: "monitoraddedv2".into(), data: "1,DP-1,Dell".into() }));
        assert_eq!(parse_raw("nothing"), None);
        assert_eq!(lua_str("a\"b\\"), "\"a\\\"b\\\\\"");
        assert_eq!(round(2.5), 3);
        assert_eq!(round(-0.4), 0);
    }
}

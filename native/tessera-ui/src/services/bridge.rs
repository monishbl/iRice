//! The QML shell, while it still owns things the Rust surfaces show
//! (docs/rust-ui-plan.md: side by side). Two directions:
//!
//! In: shell/services/RustUi.qml writes JSON lines to this process's stdin
//! (it started it with a pipe), one per change; each becomes a reading:
//!   {"src":"shell","kind":"state", …}      ShellState: what's open, Focus,
//!        Caffeine, unread notifications, the sound, privacy, weather, …
//!   {"src":"shell","kind":"island","op":"post"|"update"|"remove"|"dismiss", …}
//!        IslandCall: the QML Island service's alerts, as they're posted
//!   {"src":"shell","kind":"live","activities":[…]}
//!        LiveActivities: its ongoing activities (timers, recording…)
//!
//! Out: `call(&["panels", "toggleAt", …])` runs shell/assets/scripts/
//! shell-ipc.sh call … on a thread (opening a QML panel, an island action).

use serde_json::Value;
use std::any::Any;

/// The QML shell's state, as last said (fields as RustUi.qml names them)
#[derive(Clone, Debug, Default)]
pub struct ShellState(pub Value);

impl ShellState {
    pub fn get(&self, path: &str) -> &Value {
        path.split('.').fold(&self.0, |v, k| &v[k])
    }
    pub fn str(&self, path: &str) -> &str {
        self.get(path).as_str().unwrap_or("")
    }
    pub fn bool(&self, path: &str) -> bool {
        self.get(path).as_bool().unwrap_or(false)
    }
    pub fn num(&self, path: &str) -> f64 {
        self.get(path).as_f64().unwrap_or(0.0)
    }
    /// Is this panel open on this screen? (Panels.isOpen)
    pub fn open(&self, panel: &str, screen: &str) -> bool {
        self.str("open") == panel && (self.str("screen") == screen || self.str("screen").is_empty())
    }
}

/// One of the QML Island's calls (its uid there, for the actions)
#[derive(Clone, Debug, PartialEq)]
pub struct IslandCall {
    pub op: String,
    pub uid: i64,
    pub ms: i64,
    /// the event (post) or the change (update)
    pub event: Value,
}

/// The QML shell's ongoing activities (Island.activities, with what
/// LiveInfo says for each), oldest snapshot replaced by the newest
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LiveActivities(pub Vec<Value>);

/// A line from the shell, as the reading it carries (None: not for us)
pub fn parse_line(line: &str) -> Option<Box<dyn Any + Send>> {
    let v: Value = serde_json::from_str(line.trim()).ok()?;
    if v["src"] != "shell" {
        return None;
    }
    match v["kind"].as_str()? {
        "state" => Some(Box::new(ShellState(v))),
        "island" => Some(Box::new(IslandCall {
            op: v["op"].as_str()?.to_string(),
            uid: v["uid"].as_i64().unwrap_or(0),
            ms: v["ms"].as_i64().unwrap_or(3000),
            event: if v["event"].is_null() { v["patch"].clone() } else { v["event"].clone() },
        })),
        "live" => Some(Box::new(LiveActivities(v["activities"].as_array().cloned().unwrap_or_default()))),
        _ => None,
    }
}

/// Splits what came in on stdin into whole lines (keeps a partial one)
#[derive(Default)]
pub struct Lines {
    buf: Vec<u8>,
}

impl Lines {
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<String> {
        self.buf.extend_from_slice(bytes);
        let mut out = Vec::new();
        while let Some(i) = self.buf.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = self.buf.drain(..=i).collect();
            out.push(String::from_utf8_lossy(&line[..line.len() - 1]).into_owned());
        }
        // (a runaway line without an end is dropped, not kept forever)
        if self.buf.len() > 1 << 20 {
            self.buf.clear();
        }
        out
    }
}

/// shell-ipc.sh, from the repo the shell runs from (TESSERA_ROOT), else this build's
pub fn ipc_script() -> std::path::PathBuf {
    let root = crate::env::var("ROOT").filter(|r| !r.is_empty()).map(std::path::PathBuf::from).unwrap_or_else(|| std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../..")));
    root.join("shell/assets/scripts/shell-ipc.sh")
}

/// `shell-ipc.sh call <args…>` on a thread (the QML shell does it)
pub fn call(args: &[&str]) {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    std::thread::spawn(move || {
        let out = std::process::Command::new(ipc_script()).arg("call").args(&args).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).status();
        if crate::env::is_set("DEBUG") {
            eprintln!("debug: shell-ipc call {args:?} -> {out:?}");
        }
    });
}

/// A command on a thread (hyprpicker, wpctl, brightnessctl…), its output ignored
pub fn run(cmd: &[&str]) {
    let cmd: Vec<String> = cmd.iter().map(|s| s.to_string()).collect();
    std::thread::spawn(move || {
        let _ = std::process::Command::new(&cmd[0]).args(&cmd[1..]).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_come_whole() {
        let mut l = Lines::default();
        assert!(l.feed(b"{\"a\":").is_empty());
        assert_eq!(l.feed(b"1}\n{\"b\":2}\n{\"c\""), vec!["{\"a\":1}", "{\"b\":2}"]);
        assert_eq!(l.feed(b":3}\n"), vec!["{\"c\":3}"]);
    }

    #[test]
    fn the_shell_says_its_state_and_island_calls() {
        let s = parse_line(r#"{"src":"shell","kind":"state","open":"wifiMenu","screen":"DP-1","focus":{"on":true,"icon":"moon"}}"#).unwrap();
        let s = s.downcast::<ShellState>().unwrap();
        assert!(s.open("wifiMenu", "DP-1") && !s.open("wifiMenu", "HDMI-A-1") && !s.open("soundMenu", "DP-1"));
        assert!(s.bool("focus.on"));
        assert_eq!(s.str("focus.icon"), "moon");
        assert_eq!(s.num("nothing.here"), 0.0);
        let c = parse_line(r#"{"src":"shell","kind":"island","op":"post","uid":7,"ms":5000,"event":{"kind":"charging","title":"Charging"}}"#).unwrap();
        let c = c.downcast::<IslandCall>().unwrap();
        assert_eq!((c.op.as_str(), c.uid, c.ms), ("post", 7, 5000));
        assert_eq!(c.event["title"], "Charging");
        let u = parse_line(r#"{"src":"shell","kind":"island","op":"update","uid":7,"patch":{"subtitle":"80%"}}"#).unwrap().downcast::<IslandCall>().unwrap();
        assert_eq!(u.event["subtitle"], "80%");
        let l = parse_line(r#"{"src":"shell","kind":"live","activities":[{"kind":"timer"}]}"#).unwrap().downcast::<LiveActivities>().unwrap();
        assert_eq!(l.0.len(), 1);
        // not for us, or not JSON
        assert!(parse_line(r#"{"src":"ui","kind":"ready"}"#).is_none());
        assert!(parse_line("hello").is_none());
    }
}

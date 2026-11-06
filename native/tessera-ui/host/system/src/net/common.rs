//! Helpers of the net group: the island's alerts, watched processes, tools
//! on PATH, paths the QML kept under Paths.state.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use tessera_core::host::Host;

/// Theme.accents (the dark set: the island is dark)
pub mod accent {
    pub const BLUE: &str = "#0A84FF";
    pub const GREEN: &str = "#30D158";
    pub const ORANGE: &str = "#FF9F0A";
    pub const YELLOW: &str = "#FFD60A";
    /// Qt.rgba(1, 1, 1, 0.55) in Qt's #AARRGGBB
    pub const GREY: &str = "#8cffffff";
    /// Theme.color.danger
    pub const DANGER: &str = "#FF453A";
}

/// Island.post(event, ms): the island service is host-shell's. The event is
/// as the QML builds it (kind, icon, title, subtitle, color, ripple…); an
/// action button is `action` (its label) with `actionCall` + `actionArgs`:
/// the call the island makes (shell::ask style, "power.saverOn") when it is
/// pressed, in place of the QML's `onAction` closure (`action2*` the same).
pub fn island_post(h: &mut Host, event: Value, ms: i64) {
    h.ask("island.post", json!({"event": event, "ms": ms}));
}

/// Island.enabled
pub fn island_enabled(cfg: &Value) -> bool {
    cfg["island"]["enabled"].as_bool().unwrap_or(true)
}

/// Island.wants(kind): chosen, or a kind added after the setting existed and
/// not touched yet (Island.qml newKinds)
pub fn island_wants(cfg: &Value, kind: &str) -> bool {
    const NEW_KINDS: [&str; 8] = ["usb", "displays", "audio", "temperature", "network", "transfers", "calls", "sysupdate"];
    let has = |k: &str, v: &str| cfg["island"][k].as_array().is_some_and(|a| a.iter().any(|x| x == v));
    has("events", kind) || (NEW_KINDS.contains(&kind) && !has("known", kind))
}

/// Tools.has(name): an executable on PATH
pub fn has_tool(name: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(name).is_file()))
}

/// Paths.state: ~/.local/state/tessera (irice while only its settings exist)
pub fn state_dir() -> std::path::PathBuf {
    let name = tessera_core::theme::config_path().parent().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "tessera".into());
    crate::util::state_dir().join(name)
}

/// Date.now()
pub fn now_ms() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as f64).unwrap_or(0.0)
}

/// The call's arguments as a list (a surface sends `[what, …]`; a service
/// call may send one object, which is then the single argument)
pub fn args_list(args: &Value) -> Vec<Value> {
    match args {
        Value::Array(a) => a.clone(),
        Value::Null => vec![],
        v => vec![v.clone()],
    }
}

pub fn str_arg(a: &[Value], i: usize) -> String {
    match a.get(i) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

/// A process watched line by line on a thread of its own (Tools.watcher /
/// KeptProcess style): the child gets SIGTERM if the shell dies, and `stop`
/// ends it. `on_line` runs on that thread; `on_end(code)` when it ended.
#[derive(Clone, Default)]
pub struct Watch {
    child: Arc<Mutex<Option<Child>>>,
    stopped: Arc<std::sync::atomic::AtomicBool>,
}

impl Watch {
    pub fn start(cmd: &[&str], stdin: Option<String>, on_line: impl Fn(String) + Send + 'static, on_end: impl FnOnce(i32, bool) + Send + 'static) -> Watch {
        let w = Watch::default();
        let me = w.clone();
        let cmd: Vec<String> = cmd.iter().map(|s| s.to_string()).collect();
        std::thread::spawn(move || {
            let mut c = Command::new(&cmd[0]);
            c.args(&cmd[1..]).stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() }).stdout(Stdio::piped()).stderr(Stdio::null());
            // (the child ends with this thread, whatever happens to the shell)
            unsafe {
                c.pre_exec(|| {
                    libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM as libc::c_ulong);
                    Ok(())
                });
            }
            let Ok(mut child) = c.spawn() else { return on_end(-1, false) };
            if let (Some(text), Some(mut si)) = (stdin, child.stdin.take()) {
                use std::io::Write;
                let _ = si.write_all(text.as_bytes());
            }
            let out = child.stdout.take();
            *me.child.lock().unwrap() = Some(child);
            if let Some(out) = out {
                for line in BufReader::new(out).lines().map_while(Result::ok) {
                    on_line(line);
                }
            }
            let child = me.child.lock().unwrap().take();
            let code = child.map(|mut c| c.wait().ok().and_then(|s| s.code()).unwrap_or(-1)).unwrap_or(-1);
            on_end(code, me.stopped.load(std::sync::atomic::Ordering::SeqCst));
        });
        w
    }

    /// End the child (ours only)
    pub fn stop(&self) {
        self.stopped.store(true, std::sync::atomic::Ordering::SeqCst);
        if let Some(c) = self.child.lock().unwrap().as_mut() {
            let _ = c.kill();
        }
    }
}

/// A command run to the end on a thread, its whole stdout handed on
pub fn capture_then(cmd: Vec<String>, stdin: Option<String>, then: impl FnOnce(i32, String) + Send + 'static) {
    std::thread::spawn(move || {
        let mut c = Command::new(&cmd[0]);
        c.args(&cmd[1..]).stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() }).stdout(Stdio::piped()).stderr(Stdio::null());
        let Ok(mut child) = c.spawn() else { return then(-1, String::new()) };
        if let (Some(t), Some(mut si)) = (stdin, child.stdin.take()) {
            use std::io::Write;
            let _ = si.write_all(t.as_bytes());
        }
        let out = child.wait_with_output();
        match out {
            Ok(o) => then(o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned()),
            Err(_) => then(-1, String::new()),
        }
    });
}

/// "sh -c <script> sh <args…>" as a command line
pub fn sh_cmd(script: &str, args: &[&str]) -> Vec<String> {
    let mut v = vec!["sh".to_string(), "-c".into(), script.into(), "sh".into()];
    v.extend(args.iter().map(|s| s.to_string()));
    v
}

pub fn script_cmd(py: &str, args: &[&str]) -> Vec<String> {
    let mut v = vec!["python3".to_string(), "-I".into(), crate::util::script(py)];
    v.extend(args.iter().map(|s| s.to_string()));
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn island_wants_new_kinds_until_touched() {
        let cfg = json!({"island": {"events": ["music"], "known": []}});
        assert!(island_wants(&cfg, "network") && island_wants(&cfg, "music") && !island_wants(&cfg, "charging"));
        let touched = json!({"island": {"events": [], "known": ["network"]}});
        assert!(!island_wants(&touched, "network"));
    }

    #[test]
    fn watch_reads_lines_and_ends() {
        let (tx, rx) = std::sync::mpsc::channel();
        let tx2 = tx.clone();
        Watch::start(&["printf", "a\\nb\\n"], None, move |l| {
            let _ = tx.send(l);
        }, move |c, _| {
            let _ = tx2.send(format!("end{c}"));
        });
        let got: Vec<String> = (0..3).filter_map(|_| rx.recv_timeout(std::time::Duration::from_secs(3)).ok()).collect();
        assert_eq!(got, vec!["a", "b", "end0"]);
    }
}

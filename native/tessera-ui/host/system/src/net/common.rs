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

/// Paths.state: ~/.local/state/tessera
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
            // (its lines are read: one language, "." decimals)
            tessera_core::util::plain_locale(&mut c);
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
            {
                // (stop() before the child was kept found nothing to end: it
                // is ended here, under the same lock stop() takes)
                let mut kept = me.child.lock().unwrap();
                if me.stopped.load(std::sync::atomic::Ordering::SeqCst) {
                    let _ = child.kill();
                }
                *kept = Some(child);
            }
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
    capture_then_within(cmd, stdin, None, then)
}

/// `capture_then` with a deadline: past it the command and everything it
/// started (its process group) are killed and `then(-1, "")` runs, so a
/// caller that waits for the answer (a busy flag) is never held for ever by
/// a hung tool (ddcutil on a monitor that stopped answering)
pub fn capture_then_within(cmd: Vec<String>, stdin: Option<String>, timeout: Option<std::time::Duration>, then: impl FnOnce(i32, String) + Send + 'static) {
    let Some(limit) = timeout else { return capture_then_open(cmd, stdin, then) };
    std::thread::spawn(move || {
        let mut c = Command::new(&cmd[0]);
        tessera_core::util::plain_locale(&mut c);
        c.args(&cmd[1..]).stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() }).stdout(Stdio::piped()).stderr(Stdio::null());
        c.process_group(0);
        let Ok(mut child) = c.spawn() else { return then(-1, String::new()) };
        if let (Some(t), Some(mut si)) = (stdin, child.stdin.take()) {
            // (on a thread: a child that never reads its input can't hold up the deadline)
            std::thread::spawn(move || {
                use std::io::Write;
                let _ = si.write_all(t.as_bytes());
            });
        }
        let Some(out) = child.stdout.take() else {
            crate::util::kill_tree(&mut child);
            return then(-1, String::new());
        };
        let rx = crate::util::read_all(out);
        let start = std::time::Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(st)) => {
                    let text = crate::util::collect(&rx, crate::util::output_grace(limit, start.elapsed()));
                    return then(st.code().unwrap_or(-1), text);
                }
                Ok(None) if start.elapsed() > limit => {
                    crate::util::kill_tree(&mut child);
                    return then(-1, String::new());
                }
                Ok(None) => std::thread::sleep(std::time::Duration::from_millis(20)),
                Err(_) => {
                    crate::util::kill_tree(&mut child);
                    return then(-1, String::new());
                }
            }
        }
    });
}

/// capture_then without a deadline (for what may rightly take long: a
/// password prompt, a join that waits on the network)
fn capture_then_open(cmd: Vec<String>, stdin: Option<String>, then: impl FnOnce(i32, String) + Send + 'static) {
    std::thread::spawn(move || {
        let mut c = Command::new(&cmd[0]);
        // (its output is read: one language, "." decimals; tessera_core::util::plain_locale)
        tessera_core::util::plain_locale(&mut c);
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
    fn capture_then_within_gives_up_on_a_hung_command() {
        let (tx, rx) = std::sync::mpsc::channel();
        let t = std::time::Instant::now();
        capture_then_within(sh_cmd("sleep 30 & sleep 30", &[]), None, Some(std::time::Duration::from_millis(200)), move |c, o| {
            let _ = tx.send((c, o));
        });
        let (c, o) = rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
        assert_eq!((c, o.as_str()), (-1, ""));
        assert!(t.elapsed() < std::time::Duration::from_secs(5));
    }

    #[test]
    fn capture_then_within_hands_on_what_finishes() {
        let (tx, rx) = std::sync::mpsc::channel();
        capture_then_within(vec!["cat".into()], Some("in\n".into()), Some(std::time::Duration::from_secs(5)), move |c, o| {
            let _ = tx.send((c, o));
        });
        assert_eq!(rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap(), (0, "in\n".to_string()));
        let (tx, rx) = std::sync::mpsc::channel();
        capture_then(sh_cmd("echo $1; exit 3", &["x"]), None, move |c, o| {
            let _ = tx.send((c, o));
        });
        assert_eq!(rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap(), (3, "x\n".to_string()));
    }

    #[test]
    fn a_watch_stopped_while_it_starts_does_not_run_on() {
        let (tx, rx) = std::sync::mpsc::channel();
        let w = Watch::start(&["sleep", "30"], Some("x".into()), |_| {}, move |_, stopped| {
            let _ = tx.send(stopped);
        });
        // (at once: before or while the child is kept, the stop must end it)
        w.stop();
        assert_eq!(rx.recv_timeout(std::time::Duration::from_secs(5)), Ok(true));
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

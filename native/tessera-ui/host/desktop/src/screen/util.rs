//! Small helpers the screen services share: the tools check (Tools.qml),
//! the settings and state files they write (Config.options.x = …, UiState),
//! island posts, background commands.

use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use tessera_core::host::Host;

/// Is this command-line tool installed? (Tools.has: a PATH lookup, cached by the OS)
pub fn has(tool: &str) -> bool {
    tessera_core::util::on_path(tool)
}

pub fn home() -> String {
    std::env::var("HOME").unwrap_or_default()
}

/// $XDG_RUNTIME_DIR, else /tmp
pub fn runtime() -> String {
    std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into())
}

/// shell/assets/scripts/<name> (from the repo the shell runs from)
pub fn script(name: &str) -> String {
    tessera_core::util::asset(&format!("shell/assets/scripts/{name}")).map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|| format!("shell/assets/scripts/{name}"))
}

/// Single-quote for sh (ScreenTools' q())
pub fn q(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// A command, detached, output ignored (Quickshell.execDetached)
pub fn detached(cmd: &[&str]) {
    let Some((prog, args)) = cmd.split_first() else { return };
    let _ = Command::new(prog).args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().map(|mut c| std::thread::spawn(move || c.wait()));
}

/// Same, owned strings
pub fn detached_owned(cmd: Vec<String>) {
    let r: Vec<&str> = cmd.iter().map(String::as_str).collect();
    detached(&r);
}

/// Text onto the clipboard through wl-copy's input, not its command line
pub fn copy_text(text: &str) {
    tessera_core::util::pipe_to(&["wl-copy"], text.as_bytes().to_vec());
}

/// notify-send, detached (the notification server shows it)
pub fn notify(app: &str, title: &str, body: &str) {
    detached(&["notify-send", "-a", app, title, body]);
}

/// A setting (config.json, dotted path), from a config as last read
pub fn cfg_get<'a>(cfg: &'a Value, path: &str) -> &'a Value {
    path.split('.').fold(cfg, |v, k| &v[k])
}

fn set_path(root: &mut Value, path: &str, v: Value) {
    let keys: Vec<&str> = path.split('.').collect();
    let mut at = root;
    for k in &keys[..keys.len() - 1] {
        if !at.get(*k).is_some_and(Value::is_object) {
            at[*k] = json!({});
        }
        at = &mut at[*k];
    }
    at[keys[keys.len() - 1]] = v;
}

/// `Config.options.<path> = v`: config.json read, changed and written whole
/// (the settings watcher then tells every part)
pub fn cfg_set(path: &str, v: Value) {
    let file = tessera_core::theme::config_path();
    let mut cur: Value = std::fs::read_to_string(&file).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_else(|| json!({}));
    set_path(&mut cur, path, v);
    if let Ok(t) = serde_json::to_string_pretty(&cur) {
        let _ = tessera_core::util::write_atomic(&file, t.as_bytes());
    }
}

/// state.json (UiState's file): one top-level key read and written
pub fn state_file() -> PathBuf {
    tessera_core::util::state_dir().join("state.json")
}

pub fn state_get(key: &str) -> Value {
    let v: Value = std::fs::read_to_string(state_file()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    v[key].clone()
}

pub fn state_set(key: &str, val: Value) {
    let file = state_file();
    let mut cur: Value = std::fs::read_to_string(&file).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_else(|| json!({}));
    if !cur.is_object() {
        cur = json!({});
    }
    cur[key] = val;
    if let Ok(t) = serde_json::to_string(&cur) {
        let _ = tessera_core::util::write_atomic(&file, t.as_bytes());
    }
}

/// Island.post({…}, ms): host-shell's island service takes it (it keeps the buttons'
/// `onAction` / `more[].call` for the island's act, the history, the quiet modes)
pub fn island_post(h: &mut Host, event: Value, ms: i64) {
    h.ask("island.post", json!([event, ms]));
}

/// Both screen-tool surfaces hear the same lines (RustUi.tell twice)
pub fn tell_both(h: &mut Host, obj: Value) {
    h.tell("screentools", obj.clone());
    h.tell("quicklook", obj);
}

/// The Files app, as Files.qml picks it (Settings' choice, else the first installed)
pub fn file_manager(cfg: &Value) -> String {
    let want = cfg["desktop"]["fileManager"].as_str().unwrap_or("");
    if !want.is_empty() && has(want) {
        return want.into();
    }
    ["nautilus", "dolphin", "thunar", "nemo", "caja", "pcmanfm-qt", "pcmanfm"].iter().find(|m| has(m)).map(|s| s.to_string()).unwrap_or_default()
}

/// A time as "yyyy-MM-dd 'at' hh.mm.ss" (+ ".zzz" when `ms`)
pub fn stamp(ms: bool) -> String {
    let now = tessera_core::util::now_ms();
    let base = tessera_core::util::strftime("%Y-%m-%d at %H.%M.%S", now / 1000);
    if ms {
        format!("{base}.{:03}", now % 1000)
    } else {
        base
    }
}

/// Eight random-ish base-36 characters (Math.random().toString(36).slice(2, 10))
pub fn nonce() -> String {
    let mut x = tessera_core::util::now_ms() as u64 ^ ((std::process::id() as u64) << 32) ^ 0x9E37_79B9_7F4A_7C15;
    let mut s = String::new();
    for _ in 0..8 {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        s.push(char::from_digit((x % 36) as u32, 36).unwrap());
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting_survives_quotes() {
        assert_eq!(q("a'b"), "'a'\\''b'");
    }

    #[test]
    fn config_paths_nest() {
        let mut v = json!({});
        set_path(&mut v, "recorder.fps", json!(30));
        assert_eq!(cfg_get(&v, "recorder.fps"), 30);
    }
}

/// A process kept running while wanted (KeptProcess.qml): its stdout lines
/// go to `on_line` (on its reader thread), its end to `on_end`; stdin is held
/// open so a watcher wrapper (watch.sh) ends when this is dropped or stopped.
#[derive(Default)]
pub struct Kept {
    pub gen: u64,
    child: Option<std::process::Child>,
    stdin: Option<std::process::ChildStdin>,
}

impl Kept {
    pub fn running(&self) -> bool {
        self.child.is_some()
    }

    /// The number the next start gets (for its `on_end` to say which run ended)
    pub fn next_gen(&self) -> u64 {
        self.gen + 1
    }

    /// The run `gen` ended by itself: collect it (a stale end is ignored)
    pub fn reap_if(&mut self, gen: u64) -> bool {
        if gen == self.gen {
            self.reap();
            true
        } else {
            false
        }
    }

    pub fn start(&mut self, cmd: &[String], on_line: impl Fn(String) + Send + 'static, on_end: impl Fn() + Send + 'static) -> bool {
        use std::io::BufRead;
        if self.child.is_some() || cmd.is_empty() {
            return false;
        }
        let spawned = Command::new(&cmd[0]).args(&cmd[1..]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn();
        let Ok(mut child) = spawned else { return false };
        self.stdin = child.stdin.take();
        if let Some(out) = child.stdout.take() {
            std::thread::spawn(move || {
                for line in std::io::BufReader::new(out).lines().map_while(Result::ok) {
                    on_line(line);
                }
                on_end();
            });
        }
        self.gen += 1;
        self.child = Some(child);
        true
    }

    /// SIGTERM it and let go of its stdin (reaped on a thread)
    pub fn stop(&mut self) {
        self.stdin = None;
        if let Some(mut c) = self.child.take() {
            unsafe {
                libc::kill(c.id() as i32, libc::SIGTERM);
            }
            std::thread::spawn(move || {
                let _ = c.wait();
            });
        }
    }

    /// It ended by itself: collect it
    pub fn reap(&mut self) {
        self.stdin = None;
        if let Some(mut c) = self.child.take() {
            std::thread::spawn(move || {
                let _ = c.wait();
            });
        }
    }

    pub fn signal(&self, sig: i32) {
        if let Some(c) = &self.child {
            unsafe {
                libc::kill(c.id() as i32, sig);
            }
        }
    }
}

impl Drop for Kept {
    fn drop(&mut self) {
        self.stop();
    }
}

/// After `ms`, the service hears `msg` (a Timer.qml single shot)
pub fn later(send: impl Fn(Box<dyn std::any::Any + Send>) + Send + 'static, ms: u64, msg: Box<dyn std::any::Any + Send>) {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(ms));
        send(msg);
    });
}

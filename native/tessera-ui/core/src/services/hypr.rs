//! Hyprland: its event socket (socket2) for workspace and focus changes, its
//! request socket for state, dispatches and the glass plugin.
//! Each request is connect, write, read back to back on a thread of its own:
//! Hyprland blocks while it waits for a request on a connection it accepted
//! (see shell/assets/scripts/glass-ipc.py), so the UI thread never holds one.

use crate::Msg;
use serde::Deserialize;
use smithay_client_toolkit::reexports::calloop::channel::Sender;
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

/// Hyprland's socket folder (this instance's)
pub fn dir() -> Option<PathBuf> {
    let sig = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").ok()?;
    let rt = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    Some(PathBuf::from(rt).join("hypr").join(sig))
}

pub fn request(cmd: &str) -> std::io::Result<String> {
    let path = dir().ok_or(std::io::ErrorKind::NotFound)?.join(".socket.sock");
    let mut s = UnixStream::connect(path)?;
    s.set_read_timeout(Some(Duration::from_secs(3)))?;
    let mut data = cmd.as_bytes().to_vec();
    // Hyprland reads 1023-byte chunks and waits for more when one is exactly full
    if data.len() % 1023 == 0 {
        data.push(b' ');
    }
    s.write_all(&data)?;
    let mut out = String::new();
    s.read_to_string(&mut out)?;
    Ok(out)
}

#[derive(Deserialize, Default, Clone, Debug, PartialEq)]
pub struct Workspace {
    pub id: i32,
    #[serde(default)]
    pub monitor: String,
    #[serde(default)]
    pub windows: u32,
    /// a window is fullscreen on it
    #[serde(default, rename = "hasfullscreen")]
    pub fullscreen: bool,
}

#[derive(Deserialize, Default, Clone, Debug, PartialEq)]
pub struct WsRef {
    #[serde(default)]
    pub id: i32,
}

#[derive(Deserialize)]
struct Monitor {
    name: String,
    #[serde(rename = "activeWorkspace")]
    active: WsRef,
    #[serde(default)]
    focused: bool,
}

/// A window (j/clients, j/activewindow)
#[derive(Deserialize, Default, Clone, Debug, PartialEq)]
pub struct Client {
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub class: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub workspace: WsRef,
    #[serde(default)]
    pub floating: bool,
    #[serde(default)]
    pub pinned: bool,
    /// 0 none, 1 maximized, 2 fullscreen, 3 both
    #[serde(default)]
    pub fullscreen: u8,
    #[serde(default)]
    pub hidden: bool,
    /// its tab group's windows (more than one: it's in tabs)
    #[serde(default)]
    pub grouped: Vec<String>,
}

#[derive(Default, Clone, Debug)]
pub struct Snapshot {
    pub workspaces: Vec<Workspace>,
    pub active: BTreeMap<String, i32>, // monitor -> its current workspace
    pub class: String,
    /// the focused window (class "" none)
    pub window: Client,
    /// every window
    pub clients: Vec<Client>,
    /// the focused monitor's name
    pub focused: String,
}

impl Snapshot {
    /// The focused monitor's current workspace has a fullscreen window
    pub fn fullscreen(&self) -> bool {
        let Some(id) = self.active.get(&self.focused) else { return false };
        self.workspaces.iter().any(|w| w.id == *id && w.fullscreen)
    }
    /// A window fills the monitor's current space (Desktop.fullscreen: any
    /// fullscreen state, maximized included, not hidden in a group)
    pub fn fullscreen_on(&self, monitor: &str) -> bool {
        let Some(&ws) = self.active.get(monitor) else { return false };
        self.clients.iter().any(|c| c.workspace.id == ws && c.fullscreen > 0 && !c.hidden)
    }
    /// The classes of a space's windows, in Hyprland's order
    pub fn classes_on(&self, ws: i32) -> Vec<String> {
        self.clients.iter().filter(|c| c.workspace.id == ws && !c.class.is_empty()).map(|c| c.class.clone()).collect()
    }
}

fn snapshot() -> Option<Snapshot> {
    let workspaces: Vec<Workspace> = serde_json::from_str(&request("j/workspaces").ok()?).ok()?;
    let monitors: Vec<Monitor> = serde_json::from_str(&request("j/monitors").ok()?).ok()?;
    let client: Client = serde_json::from_str(&request("j/activewindow").ok()?).unwrap_or_default();
    let clients: Vec<Client> = request("j/clients").ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    Some(Snapshot {
        workspaces,
        focused: monitors.iter().find(|m| m.focused).map(|m| m.name.clone()).unwrap_or_default(),
        active: monitors.into_iter().map(|m| (m.name, m.active.id)).collect(),
        class: client.class.clone(),
        window: client,
        clients,
    })
}

// ── keeping the link live (HyprlandLink.qml) ───────────────────────────────

/// The data channel and the event socket of the running `spawn_events`, so
/// `refresh` and `reconnect` can reach them
static LINK: std::sync::Mutex<(Option<Sender<Msg>>, Option<UnixStream>)> = std::sync::Mutex::new((None, None));
/// When the last event line came (unix ms; 0: none yet)
static LAST_EVENT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// How long since Hyprland's event socket last said anything (u64::MAX: never)
pub fn quiet_ms() -> u64 {
    match LAST_EVENT.load(std::sync::atomic::Ordering::Relaxed) {
        0 => u64::MAX,
        t => now_ms().saturating_sub(t),
    }
}

/// Read the whole state again (Hyprland.refreshMonitors / refreshWorkspaces /
/// refreshToplevels): a fresh `Snapshot` arrives as data, on a thread
pub fn refresh() {
    let tx = LINK.lock().ok().and_then(|l| l.0.clone());
    let Some(tx) = tx else { return };
    std::thread::spawn(move || {
        if let Some(s) = snapshot() {
            let _ = tx.send(Msg::Data(Box::new(s)));
        }
    });
}

/// Drop the event socket and connect again (it falls behind and Hyprland
/// closes it: "PeerClosedError"); the reader's loop then connects anew and
/// reads a fresh snapshot. The shell needn't be replaced for this.
pub fn reconnect() {
    if let Ok(l) = LINK.lock() {
        if let Some(s) = &l.1 {
            let _ = s.shutdown(std::net::Shutdown::Both);
        }
    }
}

const RELEVANT: &[&str] = &[
    "workspacev2>>", "focusedmonv2>>", "activewindow>>", "createworkspacev2>>", "destroyworkspacev2>>",
    "moveworkspacev2>>", "openwindow>>", "closewindow>>", "movewindowv2>>", "monitoraddedv2>>", "monitorremovedv2>>",
    "fullscreen>>", "changefloatingmode>>", "pin>>", "windowtitlev2>>", "togglegroup>>", "moveintogroup>>", "moveoutofgroup>>",
];

/// Watches socket2; sends a fresh snapshot after each burst of relevant events
pub fn spawn_events(tx: Sender<Msg>) {
    if let Ok(mut l) = LINK.lock() {
        l.0 = Some(tx.clone());
    }
    std::thread::spawn(move || loop {
        if let Some(s) = snapshot() {
            if tx.send(Msg::Data(Box::new(s))).is_err() {
                return;
            }
        }
        let Some(path) = dir().map(|d| d.join(".socket2.sock")) else { return };
        let Ok(stream) = UnixStream::connect(path) else {
            std::thread::sleep(Duration::from_secs(1));
            continue;
        };
        if let Ok(mut l) = LINK.lock() {
            l.1 = stream.try_clone().ok();
        }
        let mut r = BufReader::new(stream);
        let mut line = String::new();
        let mut dirty = false;
        LAST_EVENT.store(now_ms(), std::sync::atomic::Ordering::Relaxed);
        loop {
            line.clear();
            match r.read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            LAST_EVENT.store(now_ms(), std::sync::atomic::Ordering::Relaxed);
            dirty |= RELEVANT.iter().any(|p| line.starts_with(p));
            // coalesce: answer once the burst is read
            if dirty && r.buffer().is_empty() {
                dirty = false;
                if let Some(s) = snapshot() {
                    if tx.send(Msg::Data(Box::new(s))).is_err() {
                        return;
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_millis(500));
    });
}

/// A dispatch (Hyprland's Lua: `hl.dsp.window.float({ … })`), on a thread
pub fn dispatch(cmd: impl Into<String>) {
    let cmd = cmd.into();
    std::thread::spawn(move || {
        let _ = request(&format!("dispatch {cmd}"));
    });
}

pub fn focus_workspace(id: i32) {
    std::thread::spawn(move || {
        let _ = request(&format!("dispatch hl.dsp.focus({{ workspace = {id} }})"));
    });
}

// ── the glass plugin ────────────────────────────────────────────────────────

/// The plugin's command: `tessera-glass`, or `irice-glass` from a plugin built
/// before the rename (which knows only that name); set once it answers
static GLASS_CMD: std::sync::OnceLock<&'static str> = std::sync::OnceLock::new();
fn glass_cmd() -> &'static str {
    GLASS_CMD.get().copied().unwrap_or("tessera-glass")
}

/// The plugin's version from its last ping (`tessera-glass 5`); 0: not loaded.
/// 2+: shapes carry gloss (a 19th number); 3 and 4: see mouse.rs
static GLASS_VERSION: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);
pub fn plugin_version() -> i64 {
    GLASS_VERSION.load(std::sync::atomic::Ordering::Relaxed)
}

/// What a ping answered: the version after the name (1 when it says none)
fn version_of(answer: &str) -> i64 {
    answer.split_whitespace().nth(1).and_then(|v| v.parse().ok()).unwrap_or(1)
}

/// Which name the plugin answers to, if any (its `ping` replies with its name)
pub fn find_plugin(ask: impl Fn(&str) -> Option<String>) -> Option<&'static str> {
    ["tessera-glass", "irice-glass"].into_iter().find(|n| ask(&format!("{n} ping")).is_some_and(|a| a.starts_with(n)))
}

/// Sends glass shapes to the plugin: the newest per surface wins, several go
/// out as one Hyprland batch (as GlassPlugin.qml does)
#[derive(Clone)]
pub struct Glass {
    tx: mpsc::Sender<(String, String)>,
}

/// The queue key that asks the sender thread to ping the plugin again
const PING: &str = "\0ping";

impl Glass {
    pub fn spawn(ui: Sender<Msg>) -> Self {
        let (tx, rx) = mpsc::channel::<(String, String)>();
        std::thread::spawn(move || {
            // (TESSERA_GLASS_FORCE=1: send shapes even without the plugin, to see them in TESSERA_DEBUG's log)
            let debug = crate::env::is_set("DEBUG");
            let force = crate::env::is_set("GLASS_FORCE");
            // (a ping: finds the plugin and its version; false when it is gone)
            let ping = |ui: &Sender<Msg>, first: bool, was: &mut bool| {
                let found = find_plugin(|c| request(c).ok());
                if let Some(n) = found {
                    let _ = GLASS_CMD.set(n);
                    let v = request(&format!("{} ping", glass_cmd())).map(|a| version_of(&a)).unwrap_or(1);
                    GLASS_VERSION.store(v, std::sync::atomic::Ordering::Relaxed);
                } else {
                    GLASS_VERSION.store(0, std::sync::atomic::Ordering::Relaxed);
                }
                let ok = found.is_some() || force;
                if first || ok != *was {
                    let _ = ui.send(Msg::GlassPlugin(ok));
                }
                *was = ok;
            };
            let mut was = false;
            ping(&ui, true, &mut was);
            while let Ok(first) = rx.recv() {
                let mut pending: BTreeMap<String, String> = BTreeMap::new();
                pending.insert(first.0, first.1);
                while let Ok((k, c)) = rx.try_recv() {
                    pending.insert(k, c);
                }
                if pending.remove(PING).is_some() {
                    ping(&ui, false, &mut was);
                }
                if pending.is_empty() {
                    continue;
                }
                // (the plugin's name, as the ping found it: "\0glass" stands for it)
                let cmds: Vec<String> = pending.into_values().map(|c| c.replace("\0glass", glass_cmd())).collect();
                let cmd = if cmds.len() == 1 { cmds[0].clone() } else { format!("[[BATCH]]{}", cmds.join(";")) };
                let answer = request(&cmd);
                if debug {
                    eprintln!("debug: glass {cmd} -> {answer:?}");
                }
            }
        });
        Self { tx }
    }
    /// Ask the plugin again (after a config reload, where plugins load; now and
    /// then while it is missing): `Msg::GlassPlugin` follows when it changed, and
    /// `plugin_version()` is fresh
    pub fn ping(&self) {
        let _ = self.tx.send((PING.into(), String::new()));
    }
    /// Extra frost where glass covers windows (Settings → Appearance, 0..1):
    /// `window-frost <amount>`; the newest wins. Plugins load blank, so say it
    /// again whenever a ping finds one.
    pub fn window_frost(&self, amount: f32) {
        let _ = self.tx.send(("window-frost".into(), format!("\0glass window-frost {amount:.2}")));
    }
    /// `shapes`: each shape's 19 numbers (see hypr/plugin/tessera-glass/main.cpp)
    pub fn set(&self, monitor: &str, ns: &str, shapes: &[[f32; 19]]) {
        let mut cmd = format!("{} set {monitor} {ns} {}", glass_cmd(), shapes.len());
        for s in shapes {
            for v in s {
                cmd.push_str(&format!(" {}", (v * 1000.0).round() / 1000.0));
            }
        }
        let _ = self.tx.send((format!("{monitor}\n{ns}"), cmd));
    }
    /// Blocking: used on the way out, so the shapes don't outlive the bar
    pub fn clear_now(monitor: &str, ns: &str) {
        let _ = request(&format!("{} set {monitor} {ns} 0", glass_cmd()));
    }
}

#[cfg(test)]
mod tests {
    use super::find_plugin;

    #[test]
    fn finds_the_plugin_by_either_name() {
        // Tessera's plugin answers to both names
        assert_eq!(find_plugin(|c| c.starts_with("tessera-glass").then(|| "tessera-glass 5".into())), Some("tessera-glass"));
        // one built before the rename knows only irice-glass
        assert_eq!(find_plugin(|c| c.starts_with("irice-glass").then(|| "irice-glass 5".into())), Some("irice-glass"));
        // Hyprland without the plugin answers an unknown command with an error
        assert_eq!(find_plugin(|_| Some("unknown request".into())), None);
        assert_eq!(find_plugin(|_| None), None);
    }

    #[test]
    fn fullscreen_on_the_focused_monitor() {
        use super::{Snapshot, Workspace};
        let ws = |id, monitor: &str, fullscreen| Workspace { id, monitor: monitor.into(), fullscreen, ..Default::default() };
        let mut h = Snapshot { focused: "DP-1".into(), workspaces: vec![ws(1, "DP-1", false), ws(2, "HDMI-A-1", true)], ..Default::default() };
        h.active.insert("DP-1".into(), 1);
        h.active.insert("HDMI-A-1".into(), 2);
        assert!(!h.fullscreen());
        h.focused = "HDMI-A-1".into();
        assert!(h.fullscreen());
        // (as Hyprland's JSON says it)
        let w: Workspace = serde_json::from_str(r#"{"id":3,"monitor":"DP-1","windows":1,"hasfullscreen":true}"#).unwrap();
        assert!(w.fullscreen);
    }
}

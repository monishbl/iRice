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

fn dir() -> Option<PathBuf> {
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
    /// A window fills the monitor's current space (Desktop.fullscreen: any
    /// fullscreen state, maximized included, not hidden in a group)
    pub fn fullscreen(&self, monitor: &str) -> bool {
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

const RELEVANT: &[&str] = &[
    "workspacev2>>", "focusedmonv2>>", "activewindow>>", "createworkspacev2>>", "destroyworkspacev2>>",
    "moveworkspacev2>>", "openwindow>>", "closewindow>>", "movewindowv2>>", "monitoraddedv2>>", "monitorremovedv2>>",
    "fullscreen>>", "changefloatingmode>>", "pin>>", "windowtitlev2>>", "togglegroup>>", "moveintogroup>>", "moveoutofgroup>>",
];

/// Watches socket2; sends a fresh snapshot after each burst of relevant events
pub fn spawn_events(tx: Sender<Msg>) {
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
        let mut r = BufReader::new(stream);
        let mut line = String::new();
        let mut dirty = false;
        loop {
            line.clear();
            match r.read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
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

impl Glass {
    pub fn spawn(ui: Sender<Msg>) -> Self {
        let (tx, rx) = mpsc::channel::<(String, String)>();
        std::thread::spawn(move || {
            // (TESSERA_GLASS_FORCE=1: send shapes even without the plugin, to see them in TESSERA_DEBUG's log)
            let found = find_plugin(|c| request(c).ok());
            if let Some(n) = found {
                let _ = GLASS_CMD.set(n);
            }
            let ok = found.is_some() || crate::env::is_set("GLASS_FORCE");
            let debug = crate::env::is_set("DEBUG");
            let _ = ui.send(Msg::GlassPlugin(ok));
            while let Ok(first) = rx.recv() {
                let mut pending: BTreeMap<String, String> = BTreeMap::new();
                pending.insert(first.0, first.1);
                while let Ok((k, c)) = rx.try_recv() {
                    pending.insert(k, c);
                }
                let cmds: Vec<String> = pending.into_values().collect();
                let cmd = if cmds.len() == 1 { cmds[0].clone() } else { format!("[[BATCH]]{}", cmds.join(";")) };
                let answer = request(&cmd);
                if debug {
                    eprintln!("debug: glass {cmd} -> {answer:?}");
                }
            }
        });
        Self { tx }
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
}

//! Hyprland's windows (j/clients) and monitors (j/monitors), for the dock
//! and later the switcher and overview: what's open, where, which one is in
//! front, which want attention. Like services::hypr, a thread follows the
//! event socket and sends a fresh `Windows` after each burst of relevant
//! events.

use crate::services::hypr::request;
use crate::Msg;
use serde::Deserialize;
use smithay_client_toolkit::reexports::calloop::channel::Sender;
use std::collections::BTreeSet;
use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixStream;
use std::time::Duration;

#[derive(Deserialize, Default, Clone, Debug, PartialEq)]
pub struct WsRef {
    #[serde(default)]
    pub id: i32,
    #[serde(default)]
    pub name: String,
}

#[derive(Deserialize, Default, Clone, Debug, PartialEq)]
pub struct Client {
    pub address: String,
    #[serde(default)]
    pub class: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub pid: i64,
    #[serde(default)]
    pub workspace: WsRef,
    #[serde(default)]
    pub at: (i32, i32),
    #[serde(default)]
    pub size: (i32, i32),
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub mapped: bool,
    #[serde(default)]
    pub fullscreen: i32,
    #[serde(default)]
    pub monitor: i32,
    // (the Overview, Mission Control and the switcher: stacking, recency, tabs)
    #[serde(default)]
    pub floating: bool,
    /// 0: focused last (Hyprland's focusHistoryID; 999 when it doesn't say)
    #[serde(default = "far", rename = "focusHistoryID")]
    pub focus_history_id: i32,
    #[serde(default)]
    pub grouped: Vec<String>,
}

fn far() -> i32 {
    999
}

impl Client {
    /// Put out of sight: minimized (the dock's) or stepped aside for Show Desktop
    pub fn aside(&self) -> bool {
        matches!(self.workspace.name.as_str(), "special:minimized" | "special:tessera-desktop")
    }
    pub fn minimized(&self) -> bool {
        self.workspace.name == "special:minimized"
    }
}

#[derive(Deserialize, Default, Clone, Debug, PartialEq)]
pub struct Monitor {
    pub id: i32,
    pub name: String,
    #[serde(default)]
    pub x: i32,
    #[serde(default)]
    pub y: i32,
    #[serde(default)]
    pub width: i32,
    #[serde(default)]
    pub height: i32,
    #[serde(default = "one")]
    pub scale: f32,
    #[serde(default)]
    pub transform: i32,
    #[serde(default, rename = "activeWorkspace")]
    pub active: WsRef,
    #[serde(default)]
    pub focused: bool,
}

fn one() -> f32 {
    1.0
}

impl Monitor {
    /// Its size in layout (logical) pixels
    pub fn logical(&self) -> (f32, f32) {
        let (w, h) = (self.width as f32 / self.scale.max(0.1), self.height as f32 / self.scale.max(0.1));
        if self.transform % 2 == 1 {
            (h, w)
        } else {
            (w, h)
        }
    }
}

#[derive(Default, Clone, Debug)]
pub struct Windows {
    /// In Hyprland's order (oldest first, as Quickshell's toplevels)
    pub clients: Vec<Client>,
    pub monitors: Vec<Monitor>,
    /// The window in front ("" none)
    pub active: String,
    /// Windows asking for attention (urgent), until they're focused
    pub urgent: BTreeSet<String>,
}

impl Windows {
    pub fn monitor(&self, name: &str) -> Option<&Monitor> {
        self.monitors.iter().find(|m| m.name == name)
    }
    pub fn focused_monitor(&self) -> Option<&Monitor> {
        self.monitors.iter().find(|m| m.focused).or(self.monitors.first())
    }
    pub fn active_client(&self) -> Option<&Client> {
        self.clients.iter().find(|c| c.address == self.active)
    }

    /// Whether any window on this monitor's visible space overlaps a rectangle
    /// (monitor-local logical x, y, w, h): Desktop.qml's covered()
    pub fn covered(&self, monitor: &str, r: (f32, f32, f32, f32)) -> bool {
        let Some(m) = self.monitor(monitor) else { return false };
        self.clients.iter().filter(|c| c.workspace.id == m.active.id && c.mapped && !c.hidden).any(|c| {
            let (x, y) = ((c.at.0 - m.x) as f32, (c.at.1 - m.y) as f32);
            x < r.0 + r.2 && x + c.size.0 as f32 > r.0 && y < r.1 + r.3 && y + c.size.1 as f32 > r.1
        })
    }
}

#[derive(Deserialize, Default)]
struct Active {
    #[serde(default)]
    address: String,
}

fn snapshot(urgent: &BTreeSet<String>) -> Option<Windows> {
    let clients: Vec<Client> = serde_json::from_str(&request("j/clients").ok()?).ok()?;
    let monitors: Vec<Monitor> = serde_json::from_str(&request("j/monitors").ok()?).ok()?;
    let active: Active = serde_json::from_str(&request("j/activewindow").ok()?).unwrap_or_default();
    let urgent = urgent.iter().filter(|a| clients.iter().any(|c| &c.address == *a)).cloned().collect();
    Some(Windows { clients, monitors, active: active.address, urgent })
}

const RELEVANT: &[&str] = &[
    "openwindow>>", "closewindow>>", "movewindow>>", "movewindowv2>>", "activewindowv2>>", "windowtitlev2>>",
    "changefloatingmode>>", "fullscreen>>", "workspacev2>>", "focusedmonv2>>", "monitoraddedv2>>", "monitorremovedv2>>",
    "urgent>>", "minimized>>", "resizewindow>>",
];

/// "0x55d1…" from an event's first field (Hyprland gives it without the 0x)
fn address(field: &str) -> String {
    let a = field.split(',').next().unwrap_or("").trim();
    if a.starts_with("0x") { a.to_string() } else { format!("0x{a}") }
}

/// What an event does to the urgent set: urgent>>ADDR adds it, focusing a
/// window (activewindowv2>>ADDR) clears it
pub fn note_urgent(urgent: &mut BTreeSet<String>, line: &str) {
    if let Some(a) = line.strip_prefix("urgent>>") {
        urgent.insert(address(a));
    } else if let Some(a) = line.strip_prefix("activewindowv2>>") {
        urgent.remove(&address(a));
    }
}

pub fn spawn(tx: Sender<Msg>) {
    std::thread::spawn(move || {
        let mut urgent = BTreeSet::new();
        loop {
            if let Some(s) = snapshot(&urgent) {
                if tx.send(Msg::Data(Box::new(s))).is_err() {
                    return;
                }
            }
            let Some(path) = crate::services::hypr::dir().map(|d| d.join(".socket2.sock")) else { return };
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
                note_urgent(&mut urgent, line.trim_end());
                dirty |= RELEVANT.iter().any(|p| line.starts_with(p));
                if dirty && r.buffer().is_empty() {
                    dirty = false;
                    if let Some(s) = snapshot(&urgent) {
                        if tx.send(Msg::Data(Box::new(s))).is_err() {
                            return;
                        }
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(500));
        }
    });
}

/// "address:0x…" for a dispatch
fn sel(address: &str) -> String {
    format!("address:{}", if address.starts_with("0x") { address.to_string() } else { format!("0x{address}") })
}

fn dispatch(cmds: Vec<String>) {
    std::thread::spawn(move || {
        for c in cmds {
            let _ = request(&format!("dispatch {c}"));
        }
    });
}

/// Bring a window to the front
pub fn focus(address: &str) {
    dispatch(vec![format!("hl.dsp.focus({{ window = \"{}\" }})", sel(address))]);
}

/// A minimized or set-aside window back onto a space, and in front (Apps.restore)
pub fn restore(address: &str, workspace: i32) {
    let s = sel(address);
    dispatch(vec![
        format!("hl.dsp.window.move({{ workspace = {workspace}, follow = true, window = \"{s}\" }})"),
        format!("hl.dsp.focus({{ window = \"{s}\" }})"),
    ]);
}

pub fn close(address: &str) {
    dispatch(vec![format!("hl.dsp.window.close({{ window = \"{}\" }})", sel(address))]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_hyprlands_clients() {
        let j = r#"[{"address":"0x5f1","mapped":true,"hidden":false,"at":[100,1000],"size":[800,600],"workspace":{"id":1,"name":"1"},"floating":false,"monitor":0,"class":"foot","title":"~","pid":42,"fullscreen":0},
                    {"address":"0x5f2","mapped":true,"hidden":false,"at":[0,0],"size":[10,10],"workspace":{"id":-99,"name":"special:minimized"},"monitor":0,"class":"firefox","title":"News","pid":43,"fullscreen":0}]"#;
        let c: Vec<Client> = serde_json::from_str(j).unwrap();
        assert_eq!(c[0].class, "foot");
        assert!(!c[0].aside() && c[1].minimized() && c[1].aside());
        let m: Vec<Monitor> = serde_json::from_str(r#"[{"id":0,"name":"DP-1","x":0,"y":0,"width":3840,"height":2160,"scale":2.0,"transform":0,"activeWorkspace":{"id":1,"name":"1"},"focused":true}]"#).unwrap();
        assert_eq!(m[0].logical(), (1920.0, 1080.0));
        let w = Windows { clients: c, monitors: m, ..Default::default() };
        // the dock's resting place at the bottom: foot reaches into it, the minimized one doesn't count
        assert!(w.covered("DP-1", (700.0, 1000.0, 500.0, 80.0)));
        assert!(!w.covered("DP-1", (1000.0, 1000.0, 500.0, 80.0)));
        assert!(!w.covered("HDMI-A-1", (0.0, 0.0, 9999.0, 9999.0)));
    }

    #[test]
    fn urgent_until_focused() {
        let mut u = BTreeSet::new();
        note_urgent(&mut u, "urgent>>5f1");
        assert!(u.contains("0x5f1"));
        note_urgent(&mut u, "activewindowv2>>5f2");
        assert!(u.contains("0x5f1"));
        note_urgent(&mut u, "activewindowv2>>5f1");
        assert!(u.is_empty());
    }
}

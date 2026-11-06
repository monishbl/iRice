//! Apps.qml: what the QML shell does with desktop entries joined to open
//! windows. Entries and icons are core's (services::apps, icons); this is the
//! rest: Keep in Dock (dock.pinned), the recent apps (state.json recentApps),
//! quit (close the windows, then assets/scripts/quit-app.sh ends what's left),
//! minimize and restore (the hidden special workspace the dock lists), showing
//! a window that's put out of sight, activate-or-launch, merge into tabs, and
//! window actions on the focused window.

use super::desk::Desk;
use super::hx;
use serde_json::{json, Value};
use tessera_core::host::Host;
use tessera_core::services::apps::{self as entries, Apps as Entries};
use tessera_core::services::clients::{Client, Windows};

const DEFAULT_PINNED: [&str; 7] = ["org.kde.dolphin", "thunar", "firefox", "code", "foot", "org.gnome.Calculator", "org.xfce.mousepad"];

#[derive(Default)]
pub struct AppsSvc;

fn pinned(h: &Host) -> Vec<String> {
    match h.cfg()["dock"]["pinned"].as_array() {
        Some(a) => a.iter().filter_map(|v| v.as_str().map(String::from)).collect(),
        None => DEFAULT_PINNED.iter().map(|s| s.to_string()).collect(),
    }
}

/// The entry id a window belongs to (Apps.entryFor), else its class
fn id_of(h: &Host, class: &str) -> String {
    h.st.shell.get::<Entries>().and_then(|a| a.entry_for(class)).map(|e| e.id.clone()).unwrap_or_else(|| class.to_string())
}

impl AppsSvc {
    pub fn pin(&mut self, h: &Host, id: &str) {
        let mut p = pinned(h);
        if !id.is_empty() && !p.iter().any(|x| x == id) {
            p.push(id.to_string());
            hx::set_cfg("dock", "pinned", json!(p));
        }
    }
    pub fn unpin(&mut self, h: &Host, id: &str) {
        let p: Vec<String> = pinned(h).into_iter().filter(|x| x != id).collect();
        hx::set_cfg("dock", "pinned", json!(p));
    }

    /// Recently used apps, most recent first: the focused window's app goes to the front
    pub fn note_active(&mut self, h: &Host) {
        let Some(w) = h.st.shell.get::<Windows>() else { return };
        let Some(c) = w.active_client() else { return };
        let Some(e) = h.st.shell.get::<Entries>().and_then(|a| a.entry_for(&c.class)) else { return };
        let id = e.id.clone();
        if id == "org.quickshell" {
            return;
        }
        let cur = hx::state_get("recentApps");
        let cur: Vec<String> = cur.as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default();
        if cur.first() == Some(&id) {
            return;
        }
        let mut next = vec![id.clone()];
        next.extend(cur.into_iter().filter(|x| *x != id));
        next.truncate(12);
        hx::state_set("recentApps", json!(next));
    }

    fn windows_of<'a>(h: &Host, w: &'a Windows, entry_id: &str) -> Vec<&'a Client> {
        w.clients.iter().filter(|c| id_of(h, &c.class) == entry_id || c.class == entry_id).collect()
    }

    /// Quit, not just close: apps that hide to the tray when their windows
    /// close would keep running
    pub fn quit(&mut self, h: &Host, app_id: &str) {
        let Some(w) = h.st.shell.get::<Windows>() else { return };
        let entry = id_of(h, app_id);
        let wins = Self::windows_of(h, w, &entry);
        let mut pids: Vec<i64> = wins.iter().map(|c| c.pid).filter(|p| *p > 1).collect();
        pids.sort();
        pids.dedup();
        for c in &wins {
            hx::window_action(&c.address, "close", &[]);
        }
        if !pids.is_empty() {
            if let Some(s) = hx::script("quit-app.sh") {
                let mut argv = vec!["setsid".to_string(), "-f".into(), s.to_string_lossy().into_owned()];
                argv.extend(pids.iter().map(|p| p.to_string()));
                hx::spawn(argv);
            }
        }
    }

    /// Minimize: Hyprland has none, so windows park on a hidden special workspace
    pub fn minimize(&mut self, h: &Host, address: Option<&str>) {
        let Some(a) = address.map(String::from).or_else(|| h.st.shell.get::<Windows>().map(|w| w.active.clone())) else { return };
        if a.is_empty() {
            return;
        }
        hx::window_action(&a, "move", &[("workspace", hx::q("special:minimized")), ("follow", "false".into())]);
    }
    pub fn restore(&mut self, h: &Host, address: &str) {
        let ws = h.st.shell.get::<Windows>().and_then(|w| w.focused_monitor().map(|m| m.active.id)).filter(|i| *i != 0).unwrap_or(1);
        hx::window_action(address, "move", &[("workspace", ws.to_string()), ("follow", "true".into())]);
        hx::focus_window(address);
    }

    /// Bring a window forward, whatever hid it (minimized, or Show Desktop's)
    pub fn show(&mut self, h: &Host, desk: &mut Desk, address: &str) {
        let k = hx::key(address);
        let Some(c) = h.st.shell.get::<Windows>().and_then(|w| w.clients.iter().find(|c| hx::key(&c.address) == k).cloned()) else { return };
        let n = c.workspace.name.as_str();
        if n == "special:tessera-desktop" && desk.showing() {
            desk.restore_desktop(); // the whole desktop comes back
            hx::focus_window(&c.address);
        } else if n == "special:minimized" || n == "special:tessera-desktop" {
            self.restore(h, &c.address);
        } else {
            hx::focus_window(&c.address);
        }
    }

    /// Focus the app's most recent window, or launch it
    pub fn activate_or_launch(&mut self, h: &Host, desk: &mut Desk, id: &str) {
        let Some(w) = h.st.shell.get::<Windows>() else { return };
        let wins = Self::windows_of(h, w, id);
        if wins.is_empty() {
            if let Some(e) = h.st.shell.get::<Entries>().and_then(|a| a.by_id(id)) {
                entries::launch(e);
            }
            return;
        }
        // Show Desktop is on and this app's windows stepped aside: end it
        if desk.showing() {
            if let Some(a) = wins.iter().find(|c| c.workspace.name == "special:tessera-desktop") {
                let a = a.address.clone();
                return self.show(h, desk, &a);
            }
        }
        let seen: Vec<&&Client> = wins.iter().filter(|c| !c.aside()).collect();
        let target = seen.last().map(|c| c.address.clone()).unwrap_or_else(|| wins[wins.len() - 1].address.clone());
        let a = target;
        if seen.is_empty() {
            self.show(h, desk, &a)
        } else {
            hx::focus_window(&a);
        }
    }

    /// Window tabs: the focused window becomes a tab group, and the same
    /// app's other windows on this space join it (each steps toward the
    /// group until it's in; Hyprland merges by direction)
    pub fn merge_into_tabs(&mut self, h: &Host) {
        let Some(w) = h.st.shell.get::<Windows>() else { return };
        let Some(t) = w.active_client() else { return };
        if t.grouped.is_empty() {
            hx::dispatch("hl.dsp.group.toggle()");
        }
        let (cx, cy) = (t.at.0 as f64 + t.size.0 as f64 / 2.0, t.at.1 as f64 + t.size.1 as f64 / 2.0);
        for o in w.clients.iter().filter(|o| o.address != t.address && o.workspace.id == t.workspace.id && o.class == t.class && o.grouped.is_empty()) {
            let dx = cx - (o.at.0 as f64 + o.size.0 as f64 / 2.0);
            let dy = cy - (o.at.1 as f64 + o.size.1 as f64 / 2.0);
            let dir = if dx.abs() > dy.abs() {
                if dx > 0.0 { "r" } else { "l" }
            } else if dy > 0.0 {
                "d"
            } else {
                "u"
            };
            hx::focus_window(&o.address);
            hx::dispatch(format!("hl.dsp.window.move({{ into_group = \"{dir}\" }})"));
        }
        hx::focus_window(&t.address);
    }

    /// Runs a window dispatcher on the focused window: windowAction("float", { action = "toggle" })
    pub fn window_action(&mut self, h: &Host, name: &str, args: &Value) {
        let Some(a) = h.st.shell.get::<Windows>().map(|w| w.active.clone()).filter(|a| !a.is_empty()) else { return };
        let fields: Vec<(String, String)> = args
            .as_object()
            .map(|o| o.iter().map(|(k, v)| (k.clone(), if let Some(s) = v.as_str() { hx::q(s) } else { v.to_string() })).collect())
            .unwrap_or_default();
        let refs: Vec<(&str, String)> = fields.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
        hx::window_action(&a, name, &refs);
    }
}

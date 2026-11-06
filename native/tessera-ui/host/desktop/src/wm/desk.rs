//! Desktop.qml and Windows.qml. Windows.qml groups Hyprland's windows by
//! space for the views: that is `Windows` in core (services::clients, with
//! `covered`, fullscreen and minimized helpers), so nothing is kept here.
//! Desktop.qml's Show Desktop is: the focused space's windows step aside to a
//! hidden special workspace, and come back on the next call, or when the
//! space changes or a window opens.

use super::hx;
use tessera_core::host::Host;
use tessera_core::services::clients::Windows;

#[derive(Default)]
pub struct Desk {
    /// [(address, space)] of the windows stepped aside
    stashed: Vec<(String, i32)>,
}

impl Desk {
    pub fn showing(&self) -> bool {
        !self.stashed.is_empty()
    }

    pub fn show_desktop(&mut self, h: &Host) {
        if self.showing() {
            return self.restore_desktop();
        }
        let Some(w) = h.st.shell.get::<Windows>() else { return };
        let ws = w.focused_monitor().map(|m| m.active.id).unwrap_or(0);
        if ws <= 0 {
            return;
        }
        self.stashed = w.clients.iter().filter(|c| c.workspace.id == ws).map(|c| (hx::addr(&c.address), ws)).collect();
        for (a, _) in &self.stashed {
            hx::dispatch(format!("hl.dsp.window.move({{ workspace = \"special:tessera-desktop\", follow = false, window = \"address:{a}\" }})"));
        }
    }

    pub fn restore_desktop(&mut self) {
        for (a, ws) in std::mem::take(&mut self.stashed) {
            hx::dispatch(format!("hl.dsp.window.move({{ workspace = {ws}, follow = false, window = \"address:{a}\" }})"));
        }
    }

    /// Leaving the space while the desktop shows brings the windows back
    pub fn event(&mut self, name: &str) {
        if self.showing() && (name == "workspace" || name == "openwindow") {
            self.restore_desktop();
        }
    }
}

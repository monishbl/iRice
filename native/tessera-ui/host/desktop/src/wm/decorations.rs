//! Decorations.qml: window decorations: none, window controls, or hyprbars
//! title bars (optional plugin). The controls are drawn by the glass plugin
//! when it's loaded (they move with the window on the same frame); otherwise
//! the decorations surface draws them. Title bars switch live via hyprctl.

use super::hx;
use super::Io;
use serde_json::Value;
use tessera_core::host::Host;

#[derive(Default)]
pub struct Decorations {
    mode: String,
    plugin_loaded: bool,
    controls_sent: String,
}

fn mode_of(cfg: &Value) -> String {
    cfg["appearance"]["decorations"].as_str().unwrap_or("none").to_string()
}

impl Decorations {
    pub fn begin(&mut self, h: &Host, io: &mut Io) {
        self.mode = mode_of(h.cfg());
        self.refresh(io);
        io.arm("deco.controls", 1500);
    }

    pub fn config_changed(&mut self, h: &Host, io: &mut Io) {
        if h.cfg().is_null() {
            return;
        }
        let mode = mode_of(h.cfg());
        let key = format!("{mode}|{}|{}", h.st.shell.look.dark, h.st.shell.look.material);
        if mode != self.mode {
            self.mode = mode;
            self.apply();
            if self.mode == "titlebars" && !self.plugin_loaded {
                io.arm("deco.refresh", 30000);
            }
        }
        if key != self.controls_sent {
            self.controls_sent = key;
            self.apply_controls(h);
        }
    }

    fn apply(&self) {
        if self.plugin_loaded {
            hx::eval(format!("hl.config({{ plugin = {{ hyprbars = {{ enabled = {} }} }} }})", self.mode == "titlebars"));
        }
    }

    /// The traffic lights (or none), as the mode says; after the Material look
    /// (it sends its own style, apptheme.rs; the plugin outlives the shell)
    pub fn apply_controls(&self, h: &Host) {
        let (controls, dark, material) = (self.mode == "controls", h.st.shell.look.dark, h.st.shell.look.material);
        std::thread::spawn(move || {
            if !hx::glass_available() {
                return;
            }
            hx::glass_command(&format!("controls {}", if controls { format!("on {}", if dark { "dark" } else { "light" }) } else { "off".into() }));
            if !material {
                hx::glass_command("controls-style glass");
            }
        });
    }

    pub fn refresh(&mut self, io: &Io) {
        io.thread_then(
            || std::process::Command::new("sh").args(["-c", "hyprctl plugin list 2>/dev/null | grep -qi hyprbars"]).status().map(|s| s.success()).unwrap_or(false),
            |wm, _h, io, loaded| {
                let d = &mut wm.deco;
                let changed = d.plugin_loaded != loaded;
                d.plugin_loaded = loaded;
                if changed {
                    d.apply();
                }
                // (checked every 30 s while title bars are wanted and the plugin isn't loaded)
                if d.mode == "titlebars" && !d.plugin_loaded {
                    io.arm("deco.refresh", 30000);
                }
            },
        );
    }

    pub fn tick_controls(&mut self, h: &Host) {
        self.controls_sent = format!("{}|{}|{}", self.mode, h.st.shell.look.dark, h.st.shell.look.material);
        self.apply_controls(h);
    }
}

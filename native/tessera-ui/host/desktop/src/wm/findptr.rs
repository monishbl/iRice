//! FindPointer.qml: shake to find the pointer (Settings → Mouse & Trackpad).
//! Hyprland notices the shake (hypr/tessera/findpointer.lua) and calls
//! tessera:findPointer; then, as findPointer.style says: a ring shrinks onto
//! the pointer (ring), the pointer itself is big for a moment (grow, hyprctl
//! setcursor, held under a lock so it always shrinks back), or both.
//! Here: the enabled flag to Hyprland's Lua, the grow, and the ring's
//! position feed (assets/scripts/pointer-follow.py) for the ring surface:
//! `{"to":"findpointer","show":true,"x":…,"y":…}` lines while it shows, then
//! `{"to":"findpointer","show":false}` (see CORE-NEEDS-wm.md: no surface draws
//! the ring yet).

use super::hx;
use super::Io;
use serde_json::{json, Value};
use tessera_core::host::Host;

#[derive(Default)]
pub struct FindPointer {
    enabled: Option<bool>,
    active: bool,
}

fn style(cfg: &Value) -> String {
    cfg["findPointer"]["style"].as_str().filter(|s| !s.is_empty()).unwrap_or("ring").to_string()
}

impl FindPointer {
    fn is_enabled(cfg: &Value) -> bool {
        !cfg.is_null() && cfg["findPointer"]["enabled"].as_bool().unwrap_or(true) && cfg["findPointer"]["style"].as_str().unwrap_or("ring") != "off"
    }

    /// Hyprland's side follows the setting live
    pub fn config_changed(&mut self, h: &Host) {
        if h.cfg().is_null() {
            return;
        }
        let on = Self::is_enabled(h.cfg());
        if self.enabled != Some(on) {
            self.enabled = Some(on);
            hx::eval(format!("TESSERA_FIND_POINTER = {on}"));
        }
    }

    pub fn show(&mut self, h: &mut Host, io: &mut Io) {
        let cfg = h.cfg().clone();
        if !Self::is_enabled(&cfg) || self.active {
            return;
        }
        self.active = true;
        let style = style(&cfg);
        let ring = style != "grow";
        h.tell("findpointer", json!({"show": true, "style": style, "x": 0, "y": 0}));
        if ring {
            self.follow(h);
        }
        if style != "ring" {
            let theme = if h.st.shell.look.material { super::apptheme::cursor_theme(&cfg, true) } else if cfg["appearance"]["cursor"] == "bibata" { "Bibata-Modern-Classic" } else { "macOS" };
            // Run on its own (setsid), so it always shrinks back to the size it
            // had (XCURSOR_SIZE, else 24); one at a time (a lock), and only a
            // short hold: every size change reloads the theme
            hx::spawn(vec![
                "setsid".into(),
                "-f".into(),
                "sh".into(),
                "-c".into(),
                "h=$1; [ \"$1\" = macOS ] && [ -d /usr/share/icons/macOS-hypr ] && h=macOS-hypr; s=${XCURSOR_SIZE:-24}; l=${XDG_RUNTIME_DIR:-/tmp}/tessera-findpointer.lock; exec 9>\"$l\"; flock -n 9 || exit 0; trap 'hyprctl setcursor \"$h\" \"$s\" >/dev/null' EXIT INT TERM HUP; hyprctl setcursor \"$h\" $((s * 5 / 2)) >/dev/null; sleep 0.75".into(),
                "sh".into(),
                theme.into(),
            ]);
        }
        io.arm("findptr.done", 1400);
    }

    /// Where the pointer is, while the ring shows (every 8 ms for 1.4 s)
    fn follow(&self, h: &Host) {
        let Some(script) = hx::script("pointer-follow.py") else { return };
        let send = h.sender("Wm");
        std::thread::spawn(move || {
            use std::io::{BufRead, BufReader};
            let Ok(mut child) = std::process::Command::new("python3").arg("-I").arg(script).args(["1.4", "0.008"]).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null()).stdin(std::process::Stdio::null()).spawn() else { return };
            if let Some(out) = child.stdout.take() {
                for line in BufReader::new(out).lines().map_while(Result::ok) {
                    let mut p = line.split(',').map(|x| x.trim().parse::<f64>());
                    if let (Some(Ok(x)), Some(Ok(y))) = (p.next(), p.next()) {
                        send(Box::new(super::Msg::Run(Box::new(move |wm, h, _io| {
                            if wm.findp.active {
                                h.tell("findpointer", json!({"show": true, "x": x, "y": y}));
                            }
                        }))));
                    }
                }
            }
            let _ = child.wait();
        });
    }

    pub fn tick_done(&mut self, h: &mut Host) {
        self.active = false;
        h.tell("findpointer", json!({"show": false}));
    }
}

//! SpaceSwitch.qml's side of the work: deciding when the display you moved to
//! shows its cue, and whether Hyprland may move the pointer to it. The cue
//! itself is drawn by surfaces/screenfx (spaceswitch): this says
//! `{"to":"spaceswitch","cue":"DP-1","mode":"glow"}` when it should.
//!
//! A cue shows when Hyprland's focus moves to another display
//! (`focusedmonv2`), with two displays or more, Settings → Displays → "Show
//! which display you moved to" (workspaces.switchCue: glow | dim | off, from
//! the older workspaces.dimOthers until it's set) not off, not while a display
//! is being plugged in or unplugged (focus moving then is the hotplug's: the
//! shell state `displayfx.hotplugAt`, within 4 s), and not for moves made from
//! the Overview or Mission Control (moving a window or a space there never
//! takes you along: Spaces' quiet window, or either panel open).
//!
//! Also whether Hyprland may move the pointer to the display a shortcut takes
//! you to (workspaces.warpPointer, default on): `cursor.no_warps` is set when
//! it changes, and again whenever the config is reloaded (the file's own
//! value comes back with it).
//!
//! Registered from mod.rs: `event()` from Wm::event, `config_changed()` from
//! Wm::config_changed.

use super::hx;
use super::spaces::Spaces;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tessera_core::host::Host;

/// The focused display, as the last event said
static FOCUSED: Mutex<String> = Mutex::new(String::new());
static READ: AtomicBool = AtomicBool::new(false);
/// What no_warps was last set from (Hyprland starts at the setting's default, on)
static WARP: AtomicBool = AtomicBool::new(true);

/// glow | dim | off
pub fn mode_of(cfg: &Value) -> &'static str {
    let w = &cfg["workspaces"];
    match w["switchCue"].as_str() {
        Some("glow") => "glow",
        Some("dim") => "dim",
        Some("off") => "off",
        _ if w["dimOthers"] == false => "off",
        _ => "glow",
    }
}

fn warp_of(cfg: &Value) -> bool {
    cfg["workspaces"]["warpPointer"].as_bool().unwrap_or(true)
}

/// The focus moved to a display within 4 s of one coming or going
pub fn settling(hotplug_at: &Value, now: i64) -> bool {
    let t = hotplug_at.as_f64().unwrap_or(0.0);
    t > 0.0 && (now as f64 - t) < 4000.0
}

fn apply_warp(warp: bool) {
    hx::eval(format!("hl.config({{ cursor = {{ no_warps = {} }} }})", !warp));
}

/// The focused display from Hyprland itself at start (the first event after
/// it only sets the starting point otherwise)
fn read_focused() {
    if READ.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(|| {
        let Ok(out) = std::process::Command::new("hyprctl").args(["-j", "monitors"]).stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null()).output() else { return };
        let Ok(list) = serde_json::from_slice::<Value>(&out.stdout) else { return };
        let name = list.as_array().and_then(|a| a.iter().find(|m| m["focused"] == true)).and_then(|m| m["name"].as_str()).map(str::to_string);
        if let (Some(n), Ok(mut f)) = (name, FOCUSED.lock()) {
            if f.is_empty() {
                *f = n;
            }
        }
    });
}

/// One of Hyprland's events (Wm::event)
pub fn event(spaces: &Spaces, h: &mut Host, name: &str, data: &str) {
    match name {
        "configreloaded" => apply_warp(warp_of(h.cfg())),
        "focusedmonv2" => {
            read_focused();
            let now = data.split(',').next().unwrap_or("").to_string();
            let was = match FOCUSED.lock() {
                Ok(mut f) => std::mem::replace(&mut *f, now.clone()),
                Err(_) => return,
            };
            if now == was || was.is_empty() || now.is_empty() || h.st.outputs_now().len() < 2 {
                return;
            }
            let mode = mode_of(h.cfg());
            if mode == "off" {
                return;
            }
            // a display just came or went: focus moving now is the hotplug's
            if settling(h.state("displayfx.hotplugAt"), tessera_core::util::now_ms()) {
                return;
            }
            // a move from the Overview or Mission Control (or just made there): you didn't go anywhere
            let open = h.state("open").as_str().unwrap_or("");
            if spaces.quiet() || open == "overview" || open == "missionControl" {
                return;
            }
            h.tell("spaceswitch", json!({"cue": now, "mode": mode}));
        }
        _ => {}
    }
}

/// The settings changed (Wm::config_changed)
pub fn config_changed(h: &Host) {
    if h.cfg().is_null() {
        return;
    }
    read_focused();
    let warp = warp_of(h.cfg());
    if WARP.swap(warp, Ordering::SeqCst) != warp {
        apply_warp(warp);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mode_from_the_setting_or_the_older_one() {
        assert_eq!(mode_of(&json!({})), "glow");
        assert_eq!(mode_of(&json!({"workspaces": {"switchCue": "dim"}})), "dim");
        assert_eq!(mode_of(&json!({"workspaces": {"switchCue": "off"}})), "off");
        assert_eq!(mode_of(&json!({"workspaces": {"dimOthers": false}})), "off");
        assert_eq!(mode_of(&json!({"workspaces": {"switchCue": "glow", "dimOthers": false}})), "glow");
        assert_eq!(mode_of(&json!({"workspaces": {"switchCue": "other", "dimOthers": true}})), "glow");
    }

    #[test]
    fn four_seconds_after_a_hotplug_is_the_hotplugs() {
        assert!(settling(&json!(10_000.0), 12_000));
        assert!(!settling(&json!(10_000.0), 14_500));
        assert!(!settling(&json!(0), 1_000));
        assert!(!settling(&Value::Null, 1_000));
    }
}

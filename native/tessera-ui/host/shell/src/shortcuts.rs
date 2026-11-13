//! The compositor's keybinds, by name: the GlobalShortcut lists of
//! shell/TesseraShell.qml (`bind = hl.dsp.global("tessera:<name>")`) and
//! modules/osd/Osd.qml (the media keys). Standalone there is no Quickshell to
//! receive Hyprland's global shortcuts, so a bind runs
//!   tessera-ui ipc call shortcut <name>
//! (`shell-ipc.sh call shortcut <name>` does the same); `ask("shortcut", [name])`
//! is the same from a surface. A key with a release bind (dictation, Super)
//! runs `<name>Release` on release (`dictateRelease`, `superTapRelease`).
//!
//! Super alone: pressing it arms Overview (or Spotlight, Settings → Keyboard
//! Shortcuts); letting go within 700 ms acts, unless Super went into a shortcut
//! (`superTapCancel`, from every other Super bind and Super + mouse).

use crate::common::*;
use crate::panels::focused;
use serde_json::{json, Value};
use tessera_core::host::{Host, Service};

/// Panels a shortcut name toggles on the focused display
const PANELS: &[&str] = &["spotlight", "controlCenter", "notificationCenter", "missionControl", "sidebar", "wallpaper", "session", "cheatsheet", "overlay", "keyboard", "clipboard", "launchpad", "quickNote", "screenTools", "overview", "emoji"];

/// The layouts Tiling names (`snap-<layout>`)
const SNAPS: &[&str] = &["left", "right", "top", "bottom", "fill", "center"];

/// Every other name `run` answers (beside PANELS and snap-*): all of them are
/// registered as Hyprland global shortcuts `tessera:<name>`, as the QML's
/// GlobalShortcut items are (TesseraShell.qml, Osd.qml)
const OTHERS: &[&str] = &[
    "reset", "lock", "reportBug", "appExpose", "showDesktop", "settings", "screenshotScreen", "screenshotRegion", "ocr", "screenTranslate", "record",
    "askScreen", "circleSearch", "lens", "darkMode", "caffeine", "replyLatest", "shareSelection", "shelf", "pointerDown", "pointerUp", "dictate",
    "dictateRelease", "tabs", "pip", "snap-restore", "switcher", "switcherBack", "switcherCommit", "captions", "findPointer", "superTap",
    "superTapCancel", "superTapRelease", "closeFront", "dismiss", "volumeUp", "volumeDown", "volumeMute", "micMute", "brightnessUp", "brightnessDown",
];

/// A name keybinds.lua binds to a key coming up (`release = true`)
pub fn on_release(name: &str) -> bool {
    name.ends_with("Release") || name == "pointerUp"
}

/// The names registered with Hyprland
pub fn names() -> Vec<String> {
    PANELS.iter().map(|p| p.to_string()).chain(SNAPS.iter().map(|l| format!("snap-{l}"))).chain(OTHERS.iter().map(|n| n.to_string())).collect()
}

#[derive(Default)]
pub struct Shortcuts {
    super_down: i64,
    /// the volume and brightness the keys last set, and when: a key held
    /// down repeats faster than the sound server answers, so the next step
    /// goes on from here, not from a reading a step behind
    volume_set: Option<(f64, i64)>,
    brightness_set: Option<(f64, i64)>,
}

/// How long a level the keys set counts over the readings
const KEY_HOLD_MS: i64 = 600;

/// A step of 5% up or down, never outside 0..1 (Audio.setVolume(volume ± 0.05))
pub fn step(v: f64, d: f64) -> f64 {
    ((v + d) * 1000.0).round().clamp(0.0, 1000.0) / 1000.0
}

impl Shortcuts {
    /// Audio.setVolume(Audio.volume ± 0.05): unmutes, as the QML's
    fn set_volume(&mut self, h: &Host, d: f64) {
        use tessera_core::services::audio::{send, Cmd, Sound};
        if let Some(dev) = h.st.shell.get::<Sound>().and_then(|s| s.sink().cloned()) {
            let now = now_ms();
            let from = self.volume_set.filter(|(_, t)| now - t < KEY_HOLD_MS).map_or(dev.volume, |(v, _)| v);
            let v = step(from, d);
            self.volume_set = Some((v, now));
            if dev.muted {
                send(Cmd::SinkMute { index: dev.index, muted: false });
            }
            send(Cmd::SinkVolume { index: dev.index, channels: dev.channels.max(1), volume: v });
        }
    }

    fn toggle_mute(h: &Host, mic: bool) {
        use tessera_core::services::audio::{send, Cmd, Sound};
        if let Some(s) = h.st.shell.get::<Sound>() {
            if mic {
                if let Some(d) = s.source() {
                    send(Cmd::SourceMute { index: d.index, muted: !d.muted });
                }
            } else if let Some(d) = s.sink() {
                send(Cmd::SinkMute { index: d.index, muted: !d.muted });
            }
        }
    }

    fn run(&mut self, h: &mut Host, name: &str) {
        let screen = focused(h);
        if PANELS.contains(&name) {
            return h.ask("panels.toggle", json!([name, screen]));
        }
        if let Some(layout) = name.strip_prefix("snap-").filter(|l| SNAPS.contains(l)) {
            return h.ask("tiling.snap", json!([layout]));
        }
        match name {
            "reset" => h.ask("panels.reset", json!([])),
            // Super+Q / Alt+F4 while an overlay of ours holds the keyboard
            // (keybinds.lua asks only then): that overlay closes, not the window under it
            // ("dismiss": keybinds.lua's TESSERA_CLOSE, the close shortcuts while a
            // shell overlay holds the keyboard; the QML answers the same name)
            "closeFront" | "dismiss" => h.ask("panels.closeFront", json!([])),
            "lock" => h.ask("lock.lock", json!([])),
            // (screenshot first, then the report page: Settings → Report a Bug)
            "reportBug" => h.ask("bugreport.start", json!([])),
            // App Windows: the focused app's windows
            "appExpose" => {
                let class = h.st.shell.get::<tessera_core::services::hypr::Snapshot>().map(|s| s.window.class.clone()).unwrap_or_default();
                h.ask("panels.appExpose", json!([class, screen]));
            }
            "showDesktop" => h.ask("desktop.showDesktop", json!([])),
            "settings" => h.ask("panels.toggleSettings", json!([])),
            "screenshotScreen" => h.ask("screen.screenshotScreen", json!([])),
            "screenshotRegion" => h.ask("screen.select", json!(["screenshot"])),
            "ocr" => h.ask("screen.select", json!(["ocr"])),
            "screenTranslate" => h.ask("screen.select", json!(["translate"])),
            "record" => h.ask("screen.recordToggle", json!([])),
            "askScreen" => h.ask("screen.select", json!(["ask"])),
            "circleSearch" => h.ask("screen.select", json!(["circle"])),
            "lens" => launch_run(&[&script("lens-search.sh").to_string_lossy()], "lens"),
            "darkMode" => h.ask("system.toggleDarkMode", json!([])),
            "caffeine" => h.ask("system.toggleCaffeine", json!([])),
            "replyLatest" => h.ask("notify.replyLatest", json!([])),
            "shareSelection" => h.ask("share.selection", json!([])),
            // The drop shelf: its shortcut, and the left button going down and up
            "shelf" => h.ask("shelf.toggle", json!([screen])),
            "pointerDown" => h.ask("shelf.pointerDown", json!([])),
            "pointerUp" => h.ask("shelf.pointerUp", json!([])),
            // Dictation: the key going down, and coming back up
            "dictate" => h.ask("dictation.pressed", json!([])),
            "dictateRelease" => h.ask("dictation.released", json!([])),
            // Merge windows into tabs, or move a tab out
            "tabs" => {
                let grouped = h.st.shell.get::<tessera_core::services::hypr::Snapshot>().is_some_and(|s| s.window.grouped.len() > 1);
                if grouped {
                    tessera_core::services::hypr::dispatch("hl.dsp.window.move({ out_of_group = true })");
                } else {
                    h.ask("apps.mergeTabs", json!([]));
                }
            }
            "pip" => h.ask("tiling.pip", json!([])),
            "snap-restore" => h.ask("tiling.restore", json!([])),
            // Alt+Tab: step the window switcher, and let go of Alt to commit (Switcher.qml)
            "switcher" => h.ask("switcher.step", json!([1])),
            "switcherBack" => h.ask("switcher.step", json!([-1])),
            "switcherCommit" => h.ask("switcher.commit", json!([])),
            "captions" => h.ask("captions", json!(["toggle"])),
            "findPointer" => h.ask("desktop.findPointer", json!([])),
            "superTap" => self.super_down = now_ms(),
            "superTapCancel" => self.super_down = 0,
            "superTapRelease" => {
                let armed = self.super_down > 0 && now_ms() - self.super_down < 700;
                self.super_down = 0;
                if armed {
                    match cfg_str(h.cfg(), "keyboard.superTap", "overview") {
                        "overview" => h.ask("panels.toggle", json!(["overview", screen])),
                        "spotlight" => h.ask("panels.toggle", json!(["spotlight", screen])),
                        _ => {}
                    }
                }
            }
            // the media keys (Osd.qml)
            "volumeUp" | "volumeDown" => {
                self.set_volume(h, if name == "volumeUp" { 0.05 } else { -0.05 });
                h.ask("sound.play", json!(["volumeKeys"]));
            }
            "volumeMute" => Self::toggle_mute(h, false),
            "micMute" => Self::toggle_mute(h, true),
            // (no backlight: no OSD either)
            "brightnessUp" | "brightnessDown" => {
                if let Some(b) = h.st.shell.get::<tessera_core::services::brightness::Backlight>().filter(|b| b.present) {
                    let now = now_ms();
                    let from = self.brightness_set.filter(|(_, t)| now - t < KEY_HOLD_MS).map_or(b.value, |(v, _)| v);
                    // (Brightness.set: never fully dark)
                    let v = step(from, if name == "brightnessUp" { 0.05 } else { -0.05 }).max(0.01);
                    self.brightness_set = Some((v, now));
                    tessera_core::services::brightness::set(v);
                }
            }
            other => eprintln!("tessera-ui: host: no shortcut {other}"),
        }
    }
}

impl Service for Shortcuts {
    fn name(&self) -> &'static str {
        "Shortcuts"
    }
    fn calls(&self) -> &'static [&'static str] {
        &["shortcut"]
    }
    fn ipc_targets(&self) -> &'static [&'static str] {
        &["shortcut"]
    }
    fn start(&mut self, h: &mut Host) {
        h.st.shell.want("hypr");
        h.st.shell.want("audio");
        h.st.shell.want("brightness");
        // (binds say hl.dsp.global("tessera:<name>"): heard on a connection
        // of our own, no process per key press; hypr/tessera/keybinds.lua)
        tessera_core::global_shortcuts::register("tessera", names(), |name, pressed| {
            // (a bind's key going down says "pressed", and a `global` bind's
            // key coming up "released"; the names bound to a release
            // (dictateRelease, superTapRelease, pointerUp) act on what comes)
            if pressed || on_release(name) {
                tessera_core::host::route_ipc(&["shortcut".to_string(), name.to_string()], None);
            }
        });
    }
    fn call(&mut self, h: &mut Host, _call: &str, args: &Value) {
        if let Some(n) = args[0].as_str() {
            self.run(h, n);
        }
    }
    fn ipc(&mut self, h: &mut Host, _target: &str, func: &str, _args: &[String]) -> Option<String> {
        self.run(h, func);
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_steps_stay_inside_the_range() {
        assert_eq!(step(0.5, 0.05), 0.55);
        assert_eq!(step(0.98, 0.05), 1.0);
        assert_eq!(step(0.02, -0.05), 0.0);
        assert_eq!(step(0.3, -0.05), 0.25);
    }

    #[test]
    fn every_qml_global_shortcut_is_registered() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../shell");
        let mut qml = std::fs::read_to_string(root.join("TesseraShell.qml")).unwrap();
        qml += &std::fs::read_to_string(root.join("modules/osd/Osd.qml")).unwrap();
        let ours = names();
        let mut seen = 0;
        // (lines left inside a `Shortcut {` / `GlobalShortcut {` block: its name may be on a later line)
        let mut inside = 0;
        for l in qml.lines() {
            let l = l.trim();
            if l.contains("Shortcut {") && !l.starts_with("component") {
                inside = 4;
            }
            if inside == 0 {
                continue;
            }
            inside -= 1;
            // `Shortcut { panel: "x"` and `GlobalShortcut { … name: "x"` (one line or the next)
            for key in ["panel: \"", "name: \""] {
                if let Some(at) = l.find(key) {
                    inside = 0;
                    let n = &l[at + key.len()..];
                    let n = &n[..n.find('"').unwrap()];
                    if n.ends_with('-') || n.is_empty() {
                        continue; // ("snap-" + layout: the snaps, checked below)
                    }
                    seen += 1;
                    assert!(ours.iter().any(|o| o == n), "the QML's shortcut {n} isn't registered");
                }
            }
        }
        assert!(seen > 40, "only {seen} QML shortcuts found");
        assert!(ours.iter().any(|o| o == "snap-left"));
    }

    #[test]
    fn every_panel_shortcut_is_a_panel_name() {
        assert!(PANELS.contains(&"screenTools") && PANELS.contains(&"quickNote"));
        assert!(!PANELS.contains(&"settings")); // (Settings toggles its own window)
    }
}

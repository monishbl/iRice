//! Screen tools (rust.surfaces: screentools), both looks: the region
//! selector and Circle to Search (RegionSelector.qml), the ⇧⌘5 toolbar and
//! its Options (ScreenToolbar.qml), the recording countdown
//! (RecordCountdown.qml), the translation card (TranslationCard.qml), and the
//! screenshot's floating thumbnail and markup editor with Live Text
//! (editor/: ShotThumbnail, ShotEditorWindow, Annotation, EditorButton,
//! LiveTextLayer, LiveTextBar; ScreenshotEditor.qml's session).
//!
//! The shell keeps the capturing, recording and reading (services/
//! ScreenTools.qml, Recorder.qml, LiveText.qml, WifiShare.qml): its bridge
//! (modules/screentools/RustBridge.qml) tells this part their state as
//! `{"to": "screentools", "state": {…}}` lines, Live Text's answers as
//! `{"livetext": key, "result": …}` and Wi-Fi codes as `{"wifi": key,
//! "found": […]}`. The toolbar is the "screenTools" panel (RustUi maps the
//! names); screenshots reach the editor's session as commands (`shot`,
//! `edit`, `editorClose`, from ScreenshotEditor.qml). What only the shell
//! does is asked of it (`shell::ask`): `screen.captured` (a region was
//! chosen: grim, tesseract or the recorder run there), `screen.cancel`,
//! `screen.do` (the toolbar's tools), `recorder.set` / `recorder.folder`,
//! `livetext.read` / `livetext.act` / `livetext.install`, `panels.share`.
//!
//! Differences from the QML, where Slint or the platform can't:
//! - The thumbnail is dragged out into an app through core's drag and drop
//!   (core/src/dnd.rs; the file as a URI list). Wayland doesn't say whether
//!   the drop was taken, so it fades away after any drag that started.
//! - Material's shapes don't morph between seconds (the countdown's shape
//!   changes each second; "clover8" is drawn as the four-leaf clover) and
//!   Material Symbols draw without their FILL axis.
//! - Glass: tessera-ui's glass ignores the glass settings (docs/
//!   rust-ui-glass-compare.md, issue 1).
//! - The editor's labels flatten through resvg with the look's font file
//!   (Inter through fontconfig, Google Sans Flex from the repo), the QML
//!   through Qt's text; the baseline sits within a pixel or two.

pub mod ui {
    slint::include_modules!();
}
tessera_core::theme_global!(ui);

// tessera-core's modules (shell, wayland, theme, services, util…) under the
// names they had when everything was one crate: `crate::shell::Part`
#[allow(unused_imports)]
use tessera_core::*;

pub mod draw;
pub mod geometry;
pub mod livetext;
pub mod ltui;
pub mod session;
pub mod quicklook;
mod cards;
mod region;
mod shots;
mod toolbar;

use tessera_core::shell::{Command, Part};
use tessera_core::wayland::State;
use serde_json::Value;
use std::any::Any;

const NAME: &str = "screentools";

/// An icon by the shell's Phosphor name (Material: its Material Symbol)
pub fn ic(name: &str, material: bool) -> ui::StIc {
    let g = crate::icons::glyph(name);
    if material {
        let s = crate::icons::symbol(g);
        if !s.is_empty() {
            return ui::StIc { t: s.into(), s: true };
        }
    }
    ui::StIc { t: g.into(), s: false }
}

/// What the shell tells about its own state (RustBridge.qml's `state`)
#[derive(Clone, Debug, Default)]
pub struct Bridge {
    pub select: String,
    pub select_screen: String,
    pub countdown: i64,
    pub countdown_screen: String,
    pub translation: Option<Value>,
    pub recording: bool,
    pub available: bool,
    pub gsr: bool,
    pub folder: String,
    pub home: String,
    pub toolbar_record: bool,
    pub mics: Vec<(String, String)>,
    pub ocr: livetext::Ocr,
    /// the share sheet is open (over the editor: it gives up the keyboard)
    pub share_open: bool,
    pub focused: String,
}

impl Bridge {
    pub fn from_json(v: &Value) -> Self {
        let s = |k: &str| v[k].as_str().unwrap_or("").to_string();
        let b = |k: &str| v[k].as_bool().unwrap_or(false);
        Bridge {
            select: s("select"),
            select_screen: s("selectScreen"),
            countdown: v["countdown"].as_i64().unwrap_or(0),
            countdown_screen: s("countdownScreen"),
            translation: v.get("translation").filter(|t| t.is_object()).cloned(),
            recording: b("recording"),
            available: b("available"),
            gsr: b("gsr"),
            folder: s("folder"),
            home: s("home"),
            toolbar_record: b("toolbarRecord"),
            mics: v["mics"].as_array().map(|a| a.iter().map(|m| (m["name"].as_str().unwrap_or("").to_string(), m["description"].as_str().unwrap_or("").to_string())).collect()).unwrap_or_default(),
            ocr: livetext::Ocr::from_json(v),
            share_open: b("shareOpen"),
            focused: s("focused"),
        }
    }
}

/// Messages from the windows' callbacks and the worker threads
pub(crate) enum M {
    RegionFinish(geometry::Rect),
    RegionCancel,
    RegionWindows(String, Vec<geometry::Rect>),
    Tool(i32),
    Record,
    RecordArea(String),
    Mic(i32),
    SystemAudio,
    Cursor,
    Countdown(i32),
    Fps(i32),
    Codec(i32),
    Quality(i32),
    FolderReset,
    FolderPick,
    FolderOpen,
    ToolbarClosed,
    ToolbarDismiss,
    TranslationCopy,
    TranslationClose,
    Shots(shots::Sm),
}

/// The glass under one window's shapes, sent when they change
pub(crate) fn send_glass(st: &mut State, output: &str, ns: &str, shapes: &[[f32; 19]], sent: &mut String) {
    let key = format!("{shapes:?}");
    if key != *sent {
        st.shell.glass.set(output, ns, shapes);
        *sent = key;
    }
}

/// The output's size in logical px
pub(crate) fn output_size(st: &State, output: &str) -> (f32, f32) {
    st.output_named(output).and_then(|o| st.outputs.info(&o)).and_then(|i| i.logical_size).map_or((1920.0, 1080.0), |s| (s.0.max(1) as f32, s.1.max(1) as f32))
}

/// The focused output's name (or the first)
pub(crate) fn focused_output(st: &State, bridge: &Bridge) -> Option<String> {
    let all = st.outputs_now();
    all.iter().find(|(_, n)| *n == bridge.focused).or(all.first()).map(|(_, n)| n.clone())
}

#[derive(Default)]
pub struct ScreenTools {
    bridge: Bridge,
    region: region::Region,
    toolbar: toolbar::Toolbar,
    countdown: cards::Countdown,
    translation: cards::Translation,
    shots: shots::Shots,
}

impl ScreenTools {
    fn state(&mut self, st: &mut State, v: &Value) {
        let old = std::mem::replace(&mut self.bridge, Bridge::from_json(v));
        let b = self.bridge.clone();
        if b.select != old.select || b.select_screen != old.select_screen {
            self.region.select(st, &b.select, &b.select_screen);
        }
        if b.countdown != old.countdown {
            let screen = if b.countdown_screen.is_empty() { focused_output(st, &b).unwrap_or_default() } else { b.countdown_screen.clone() };
            self.countdown.set(st, b.countdown, &screen);
        }
        if b.translation != old.translation {
            self.translation.set(st, b.translation.as_ref());
        }
        self.toolbar.bridge(st, &b);
        self.shots.bridge(st, &b, &old);
    }
}

impl Part for ScreenTools {
    fn name(&self) -> &'static str {
        NAME
    }

    fn start(&mut self, st: &mut State) {
        self.shots.start(st);
    }

    fn command(&mut self, st: &mut State, cmd: &Command) {
        match cmd.cmd.as_str() {
            "open" => {
                let screen = cmd.screen.clone().or_else(|| focused_output(st, &self.bridge));
                self.toolbar.open(st, screen, &self.bridge, cmd.args["options"].as_bool().unwrap_or(false));
            }
            "close" => self.toolbar.close(false),
            "options" => self.toolbar.prepare(cmd.args["open"].as_bool().unwrap_or(false), cmd.args["record"].as_bool().unwrap_or(false)),
            "shot" | "edit" | "editorClose" | "thumbnail" => self.shots.command(st, cmd, &self.bridge),
            _ => {}
        }
    }

    fn config_changed(&mut self, st: &mut State, look: bool) {
        self.toolbar.config(st, look, &self.bridge);
        if look {
            self.region.look(st);
            self.countdown.look(st);
            self.translation.look(st);
            self.shots.look(st);
        }
    }

    fn plugin_changed(&mut self, st: &mut State) {
        self.config_changed(st, true);
    }

    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        let msg = match msg.downcast::<Value>() {
            Ok(v) => {
                if let Some(s) = v.get("state") {
                    self.state(st, s);
                } else if let Some(k) = v["livetext"].as_str() {
                    self.shots.live_done(k, &v["result"]);
                } else if let Some(k) = v["wifi"].as_str() {
                    self.shots.wifi(k, &v["found"]);
                }
                return;
            }
            Err(m) => m,
        };
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::RegionFinish(r) => self.region.finish(st, r),
            M::RegionCancel => crate::shell::ask("screen.cancel", serde_json::json!([])),
            M::RegionWindows(screen, w) => self.region.windows(&screen, w),
            M::Tool(i) => self.toolbar.tool(st, i),
            M::Record => self.toolbar.record(st),
            M::RecordArea(a) => self.toolbar.record_area(st, &a),
            M::Mic(i) => self.toolbar.mic(st, i, &self.bridge),
            M::SystemAudio => self.toolbar.flip(st, "systemAudio"),
            M::Cursor => self.toolbar.flip(st, "cursor"),
            M::Countdown(i) => self.toolbar.choose(st, "countdown", i),
            M::Fps(i) => self.toolbar.choose(st, "fps", i),
            M::Codec(i) => self.toolbar.choose(st, "codec", i),
            M::Quality(i) => self.toolbar.choose(st, "quality", i),
            M::FolderReset => self.toolbar.folder(st, "reset"),
            M::FolderPick => self.toolbar.folder(st, "pick"),
            M::FolderOpen => self.toolbar.folder(st, "open"),
            M::ToolbarClosed => self.toolbar.closed(st),
            M::ToolbarDismiss => self.toolbar.close(true),
            M::TranslationCopy => self.translation.copy(),
            M::TranslationClose => crate::shell::ask("screen.do", serde_json::json!(["translationClose"])),
            M::Shots(m) => self.shots.message(st, m, &self.bridge),
        }
    }

    // (the toolbar keeps its place when another window takes the keyboard,
    // as the QML's: no keyboard_left)

    fn output_removed(&mut self, st: &mut State, name: &str) {
        self.region.output_removed(st, name);
        self.toolbar.output_removed(st, name);
        self.countdown.output_removed(st, name);
        self.translation.output_removed(st, name);
        self.shots.output_removed(st, name);
    }

    fn after_frame(&mut self, st: &mut State) {
        self.region.after_frame(st);
        self.toolbar.after_frame(st);
        self.countdown.after_frame(st);
        self.translation.after_frame(st);
        self.shots.after_frame(st);
    }

    fn a11y(&mut self) -> Vec<&mut crate::a11y::A11y> {
        let mut v = Vec::new();
        v.extend(self.region.a11y());
        v.extend(self.toolbar.a11y());
        v.extend(self.translation.a11y());
        v.extend(self.shots.a11y());
        v
    }

    fn quit(&mut self, _st: &mut State) {
        self.toolbar.quit();
        self.countdown.quit();
        self.translation.quit();
        self.shots.quit();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_shells_state_as_it_says_it() {
        let b = Bridge::from_json(&json!({
            "select": "circle", "selectScreen": "DP-1", "countdown": 3, "recording": true, "gsr": true,
            "mics": [{"name": "alsa_input.usb", "description": "USB mic"}], "translation": null,
            "ocrAvailable": true, "telHandler": false, "shareOpen": true
        }));
        assert_eq!((b.select.as_str(), b.select_screen.as_str(), b.countdown, b.recording, b.gsr), ("circle", "DP-1", 3, true, true));
        assert_eq!(b.mics, vec![("alsa_input.usb".to_string(), "USB mic".to_string())]);
        assert!(b.translation.is_none() && b.ocr.available && b.share_open);
        assert!(Bridge::from_json(&json!({"translation": {"x": 1}})).translation.is_some());
    }
}

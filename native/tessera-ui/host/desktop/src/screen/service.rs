//! The screen tools' service: ScreenTools.qml, Recorder.qml, LiveText.qml and
//! the two bridges that tell the Rust surfaces about them
//! (modules/screentools/RustBridge.qml, editor/ScreenshotEditor.qml). One
//! struct, because the QML singletons lean on each other all the time; the
//! pieces live in screentools.rs, recorder.rs and livetext.rs.
//!
//! Calls it answers (a surface asks, shell::ask): `screen.*` (captured,
//! cancel, do, ask), `recorder.*` (set, folder), `livetext.*` (read, act,
//! install). IPC targets: `screen`, `recorder`, `shotEditor`.

use super::util;
use serde_json::{json, Value};
use std::collections::HashSet;
use tessera_core::host::{Host, Service};
use tessera_core::services::clients::Windows;

/// A screen, as the loop sees it: name, global place, logical size
#[derive(Clone, Debug, PartialEq)]
pub struct Screen {
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

pub fn screens(h: &Host) -> Vec<Screen> {
    h.st.shell.get::<Windows>().map(|w| w.monitors.iter().map(|m| {
        let (lw, lh) = m.logical();
        Screen { name: m.name.clone(), x: m.x as f64, y: m.y as f64, w: lw as f64, h: lh as f64 }
    }).collect()).unwrap_or_default()
}

/// ScreenTools.focusedScreen()
pub fn focused(h: &Host) -> Option<Screen> {
    let all = screens(h);
    let name = h.st.shell.get::<Windows>().and_then(|w| w.focused_monitor().map(|m| m.name.clone()));
    name.and_then(|n| all.iter().find(|s| s.name == n).cloned()).or_else(|| all.into_iter().next())
}

// ── Messages from its threads ───────────────────────────────────────────
/// A shell job ended (ScreenTools.run): its kind, exit status, output
pub(super) struct JobDone(pub String, pub bool, pub String);
/// The recorder ended: run number, exit code, its stderr
pub(super) struct RecExit(pub u64, pub i32, pub String);
pub(super) struct Mics(pub Vec<Value>);
pub(super) struct LangList(pub String, pub Vec<(String, String)>);
pub(super) struct ReadDone(pub String, pub i32, pub String);
pub(super) struct TelHandler(pub bool);
pub(super) struct Tick(pub u64);
pub(super) struct IndexNow;
pub(super) struct IndexEnded;
pub(super) struct Shoot(pub Vec<String>);
pub(super) struct AskShoot(pub Vec<String>);
pub(super) struct DropNow(pub String);
pub(super) struct StartRec(pub Value);
pub(super) struct NotifyAction(pub String, pub String);

#[derive(Default)]
pub struct ScreenSvc {
    // ScreenTools
    pub(super) selecting: String,
    pub(super) selecting_screen: String,
    pub(super) translation: Option<Value>,
    pub(super) ocr_langs: String,
    pub(super) ocr_missing: Vec<String>,
    pub(super) ocr_languages: Vec<Value>,
    pub(super) ocr_installing: Vec<String>,
    pub(super) ocr_install_error: String,
    // Recorder
    pub(super) recording: bool,
    pub(super) paused: bool,
    pub(super) file: String,
    pub(super) started: i64,
    pub(super) paused_at: i64,
    pub(super) toolbar_record: bool,
    pub(super) toolbar_options: bool,
    pub(super) toolbar_seen_open: bool,
    pub(super) mics: Vec<Value>,
    pub(super) mic_muted: bool,
    pub(super) countdown: i64,
    pub(super) pending: Option<Value>,
    pub(super) tick_gen: u64,
    pub(super) cpu_encoding: bool,
    pub(super) last_what: Option<Value>,
    pub(super) discarding: bool,
    pub(super) discarded: String,
    pub(super) rec_pid: Option<i32>,
    pub(super) rec_gen: u64,
    // LiveText
    pub(super) recent: Vec<(String, Value)>,
    pub(super) reading: HashSet<String>,
    pub(super) tel_handler: bool,
    pub(super) indexing_now: bool,
    pub(super) index_waiting: bool,
    pub(super) index_pending: bool,
    pub(super) index_seen_off: bool,
    // the bridge
    pub(super) last_state: Option<Value>,
}

impl ScreenSvc {
    pub fn new() -> Self {
        Self { ocr_langs: "eng".into(), ..Default::default() }
    }

    pub(super) fn ocr_lang_arg(&self) -> String {
        let a: String = self.ocr_langs.chars().filter(|c| c.is_ascii_lowercase() || *c == '_' || *c == '+').collect();
        if a.is_empty() { "eng".into() } else { a }
    }
    pub(super) fn has_tesseract(&self) -> bool {
        util::has("tesseract")
    }
    pub(super) fn gsr(&self) -> bool {
        util::has("gpu-screen-recorder")
    }
    pub(super) fn available(&self) -> bool {
        self.gsr() || util::has("wf-recorder")
    }
    pub(super) fn folder(&self, h: &Host) -> String {
        let f = h.cfg()["recorder"]["folder"].as_str().unwrap_or("").trim().to_string();
        if f.is_empty() {
            format!("{}/Videos/Screen Recordings", util::home())
        } else if f == "~" || f.starts_with("~/") {
            format!("{}{}", util::home(), &f[1..])
        } else {
            f
        }
    }
    pub(super) fn has_editor(&self, h: &Host) -> bool {
        h.cfg()["screenshot"]["editor"].as_str().unwrap_or("tessera") != "swappy"
    }

    /// RustBridge.qml's state()
    fn state(&self, h: &Host) -> Value {
        let t = self.translation.as_ref().map(|t| {
            json!({"x": t["x"], "y": t["y"], "w": t["w"], "h": t["h"], "screen": t["screen"], "source": t["source"], "text": t["text"], "busy": t["busy"].as_bool().unwrap_or(false), "error": t["error"]})
        });
        let focused_name = focused(h).map(|s| s.name).unwrap_or_default();
        let open = h.state("open").as_str().unwrap_or("");
        json!({
            "select": self.selecting,
            "selectScreen": if self.selecting.is_empty() { String::new() } else { self.selecting_screen.clone() },
            "countdown": self.countdown,
            "countdownScreen": if self.countdown > 0 { focused_name.clone() } else { String::new() },
            "translation": t,
            "recording": self.recording,
            "available": self.available(),
            "gsr": self.gsr(),
            "folder": self.folder(h),
            "home": util::home(),
            "toolbarRecord": self.toolbar_record,
            "mics": self.mics,
            "ocrAvailable": self.has_tesseract(),
            "ocrInstalling": !self.ocr_installing.is_empty(),
            "updatesRunning": !self.ocr_installing.is_empty(),
            "ocrInstallError": self.ocr_install_error,
            "telHandler": self.tel_handler,
            "shareOpen": open == "share",
            "focused": focused_name,
        })
    }

    /// Tell both surfaces when the state changed (RustBridge's snapshot);
    /// the host state `recorder` is for the island and the control center
    pub(super) fn publish(&mut self, h: &mut Host) {
        // (the toolbar closed: Recorder resets what it was opened for)
        let open = h.state("open").as_str().unwrap_or("").to_string();
        if open == "screenTools" {
            self.toolbar_seen_open = true;
        } else if self.toolbar_seen_open {
            self.toolbar_seen_open = false;
            self.toolbar_record = false;
        }
        let s = self.state(h);
        if self.last_state.as_ref() != Some(&s) {
            self.last_state = Some(s.clone());
            util::tell_both(h, json!({"state": s}));
        }
        // (the menu bar's red recording item: ScreenTools.recording / recordingStarted)
        h.set_state("recording", json!({"on": self.recording, "started": self.started}));
        h.set_state("recorder", json!({"recording": self.recording, "paused": self.paused, "file": self.file, "started": self.started, "pausedAt": self.paused_at, "countdown": self.countdown, "micMuted": self.mic_muted, "canPause": self.gsr()}));
    }

    /// ScreenshotEditor.qml's toRust
    pub(super) fn to_rust(&self, h: &mut Host, cmd: &str, file: &str, screen: Option<&str>) {
        let scr = screen.map(String::from).or_else(|| focused(h).map(|s| s.name)).unwrap_or_default();
        h.say(json!({"cmd": cmd, "surface": "screentools", "file": file, "screen": scr}));
    }

    fn screen_call(&mut self, h: &mut Host, name: &str, a: &Value) {
        match name {
            "screen.captured" => {
                let all = screens(h);
                let want = a[1].as_str().unwrap_or("");
                let scr = all.iter().find(|s| s.name == want).cloned().or_else(|| all.iter().find(|s| s.name == self.selecting_screen).cloned()).or_else(|| focused(h));
                let n = |i: usize| a[i].as_f64().unwrap_or(0.0);
                self.captured(h, a[0].as_str().unwrap_or(""), scr, (n(2), n(3), n(4), n(5)));
            }
            "screen.cancel" | "screen.cancelSelect" => self.cancel_select(h),
            // the shell's shortcuts, menus and panels (ScreenTools.select / screenshotScreen /
            // stopRecording / recordScreen; "record" = the shortcut's start-or-stop)
            "screen.select" => self.select(h, a[0].as_str().unwrap_or("screenshot")),
            "screen.screenshotScreen" => self.screenshot_screen(h),
            "screen.stopRecording" => self.stop(h),
            // a screenshot to mark up (Report a Bug's "Edit screenshot")
            "screen.editShot" => {
                let f = a[0].as_str().unwrap_or("").to_string();
                if f.starts_with('/') {
                    self.open_editor(h, &f);
                }
            }
            "screen.recordScreen" => self.record_screen(h),
            "screen.recordToggle" => {
                if self.recording {
                    self.stop(h);
                } else {
                    self.select(h, "record");
                }
            }
            // the buttons of the "Recording saved" / "discarded" alerts (more: [{call, args}])
            "recorder.open" => util::detached(&["xdg-open", a[0].as_str().unwrap_or("")]),
            "recorder.showInFolder" => {
                let f = a[0].as_str().unwrap_or("").to_string();
                let m = util::file_manager(h.cfg());
                util::detached(&[if m.is_empty() { "xdg-open" } else { m.as_str() }, f.rsplit_once('/').map(|(d, _)| d).unwrap_or("/")]);
            }
            "recorder.undo" => self.undo_discard(h),
            // Control Center's Record tile (Recorder.openToolbar: the toolbar, or stop)
            "recorder.openToolbar" => self.open_toolbar(h),
            // Settings -> Input (OCR languages): ScreenTools.installOcr
            "screentools.installOcr" => self.install_ocr(h, a[0].as_str().unwrap_or("")),
            "screen.do" => {
                let (k, v) = (a[0].as_str().unwrap_or(""), a[1].as_str().unwrap_or(""));
                if k == "shot" {
                    self.screenshot_screen(h);
                } else if k == "select" && ["screenshot", "ocr", "translate", "ask", "circle", "record", "record-window"].contains(&v) {
                    self.select(h, v);
                } else if k == "record" && ["screen", "window", "region"].contains(&v) {
                    if self.recording {
                        self.stop(h);
                    } else {
                        self.record_area(h, v);
                    }
                } else if k == "translationClose" {
                    self.translation = None;
                }
            }
            "screen.ask" => match a[0].as_str().unwrap_or("") {
                "screen" => self.ask_screen(h),
                m @ ("ask" | "circle") => self.select(h, m),
                _ => {}
            },
            "recorder.set" => {
                let (k, v) = (a[0].as_str().unwrap_or(""), &a[1]);
                let s = v.as_str().unwrap_or("");
                let ok = match k {
                    "area" => ["screen", "window", "region"].contains(&s),
                    "mic" => v.is_string(),
                    "systemAudio" | "cursor" => v.is_boolean(),
                    "countdown" => matches!(v.as_i64(), Some(0 | 3 | 5)),
                    "fps" => matches!(v.as_i64(), Some(30 | 60)),
                    "codec" => ["h264", "hevc", "av1"].contains(&s),
                    "quality" => ["medium", "high", "very_high", "ultra"].contains(&s),
                    "folder" => s.is_empty() && v.is_string(),
                    _ => false,
                };
                if ok {
                    util::cfg_set(&format!("recorder.{k}"), v.clone());
                }
            }
            "recorder.folder" => {
                h.ask("panels.closeOpen", json!([]));
                let folder = self.folder(h);
                match a[0].as_str().unwrap_or("") {
                    "pick" => {
                        std::thread::spawn(move || {
                            let out = std::process::Command::new("python3").args([&util::script("pick-file.py"), "--folder", "--title", "Save recordings to", "--start", &folder]).stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null()).output();
                            if let Ok(o) = out {
                                let p = String::from_utf8_lossy(&o.stdout).trim().to_string();
                                if o.status.success() && !p.is_empty() {
                                    util::cfg_set("recorder.folder", json!(p));
                                }
                            }
                        });
                    }
                    "open" => {
                        let m = util::file_manager(h.cfg());
                        util::detached(&["sh", "-c", "mkdir -p \"$1\" && exec ${2:-xdg-open} \"$1\"", "sh", &folder, &m]);
                    }
                    _ => {}
                }
            }
            "livetext.read" => {
                let (path, rev) = (a[0].as_str().unwrap_or("").to_string(), a[1].as_str().unwrap_or("").to_string());
                if path.starts_with('/') {
                    self.lt_read(h, &path, &rev);
                }
            }
            "livetext.act" => {
                if ["link", "email", "phone", "wifi"].contains(&a[0]["kind"].as_str().unwrap_or("")) {
                    self.lt_act(h, &a[0]);
                }
            }
            "livetext.install" => self.install_ocr(h, ""),
            // (from Spotlight's "Screen Recording Options", and a toolbar of its own)
            "recorder.options" => self.open_options(h),
            "recorder.toolbar" => self.open_toolbar(h),
            _ => {}
        }
        self.publish(h);
    }
}

impl Service for ScreenSvc {
    fn name(&self) -> &'static str {
        "ScreenTools"
    }
    fn calls(&self) -> &'static [&'static str] {
        &["screen", "recorder", "livetext", "screentools"]
    }
    fn ipc_targets(&self) -> &'static [&'static str] {
        &["screen", "recorder", "shotEditor"]
    }
    fn start(&mut self, h: &mut Host) {
        h.st.shell.want("clients");
        self.list_languages(h);
        self.lt_start(h);
        self.publish(h);
    }
    fn config_changed(&mut self, h: &mut Host) {
        self.lt_config(h);
        self.publish(h);
    }
    fn outputs_changed(&mut self, h: &mut Host) {
        self.last_state = None;
        self.publish(h);
    }
    fn call(&mut self, h: &mut Host, call: &str, args: &Value) {
        self.screen_call(h, call, args);
    }
    fn ipc(&mut self, h: &mut Host, target: &str, func: &str, args: &[String]) -> Option<String> {
        let out = match (target, func) {
            ("screen", "screenshot") => self.screenshot_screen(h),
            ("screen", "region") => {
                let m = args.first().filter(|m| !m.is_empty()).cloned().unwrap_or_else(|| "screenshot".into());
                self.select(h, &m)
            }
            ("screen", "ask") => self.ask_screen(h),
            ("screen", "circle") => self.select(h, "circle"),
            ("screen", "stop") => self.stop(h),
            // qs ipc call screen ocrLanguages  (e.g. "deu+eng; missing: fra")
            ("screen", "ocrLanguages") => {
                let m = if self.ocr_missing.is_empty() { "none".to_string() } else { self.ocr_missing.join(",") };
                return Some(format!("{}; missing: {}", self.ocr_langs, m));
            }
            // the software update job that installed OCR ended (the system host says so)
            ("screen", "installDone") => self.install_done(h, args.first().map(|a| a == "ok").unwrap_or(true), args.get(1).cloned().unwrap_or_default()),
            ("recorder", "toolbar") => self.open_toolbar(h),
            ("recorder", "options") => self.open_options(h),
            ("recorder", "stop") => self.stop(h),
            ("recorder", "pause") => self.toggle_pause(h),
            ("recorder", "discard") => self.discard(h),
            ("recorder", "undo") => self.undo_discard(h),
            ("recorder", "screen") => self.record_screen(h),
            ("recorder", "toggleMic") => self.toggle_mic(h),
            ("recorder", "open") => util::detached(&["xdg-open", args.first().map(String::as_str).unwrap_or("")]),
            ("recorder", "showInFolder") => {
                let f = args.first().cloned().unwrap_or_default();
                let m = util::file_manager(h.cfg());
                util::detached(&[if m.is_empty() { "xdg-open" } else { m.as_str() }, f.rsplit_once('/').map(|(d, _)| d).unwrap_or("/")]);
            }
            ("recorder", "refreshMics") => self.refresh_mics(h),
            ("shotEditor", "edit") => {
                let f = args.first().cloned().unwrap_or_default();
                self.open_editor(h, &f);
            }
            ("shotEditor", "thumbnail") => {
                let f = args.first().cloned().unwrap_or_default();
                self.editor_shot(h, &f);
            }
            ("shotEditor", "close") => h.say(json!({"cmd": "editorClose", "surface": "screentools"})),
            _ => {}
        };
        let _ = out;
        self.publish(h);
        None
    }
    fn message(&mut self, h: &mut Host, msg: Box<dyn std::any::Any + Send>) {
        self.on_message(h, msg);
        self.publish(h);
    }
    fn quit(&mut self, _h: &mut Host) {
        if let Some(p) = self.rec_pid {
            unsafe { libc::kill(p, libc::SIGINT) };
        }
    }
}

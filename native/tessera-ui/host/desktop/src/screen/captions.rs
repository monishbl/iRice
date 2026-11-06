//! Live captions (shell/services/Captions.qml + modules/captions/
//! CaptionsFeed.qml and the data half of CaptionsWindow.qml): subtitles for
//! the sound this computer plays, made here by assets/scripts/captions.py
//! (whisper.cpp's Tiny model; never the microphone, nothing saved).
//!
//! The helper runs while `captions.enabled` and it is set up (whisper binary
//! and the Tiny model, from `dictate.py status`). What the Rust captions
//! surface shows is said to it as `{"to":"captions","state":{…}}`; it asks
//! (`ask("captions", ["off"])`, `["place", x, y]`) to turn them off or keep
//! the bar's place. `qs ipc call captions toggle | on | off | demo`.

use super::util;
use serde_json::{json, Value};
use std::process::{Command, Stdio};
use tessera_core::host::{Host, Service};
use tessera_core::services::clients::Windows;

struct Status(Value);
struct Line(String);
struct Ended(u64);
struct Retry;

#[derive(Default)]
pub struct CaptionsSvc {
    info: Value,
    kept: util::Kept,
    lines: Vec<String>,
    partial: String,
    quiet: bool,
    error: String,
    was_enabled: bool,
    last: Option<Value>,
    language: String,
}

impl CaptionsSvc {
    pub fn new() -> Self {
        Self { quiet: true, ..Default::default() }
    }

    fn enabled(h: &Host) -> bool {
        h.cfg()["captions"]["enabled"].as_bool().unwrap_or(false)
    }
    fn placement(h: &Host) -> String {
        h.cfg()["captions"]["placement"].as_str().filter(|s| !s.is_empty()).unwrap_or("bar").into()
    }
    fn ready(&self) -> bool {
        let bin = self.info["binary"].as_str().is_some_and(|s| !s.is_empty()) || self.info["binary"].as_bool() == Some(true);
        bin && self.info["models"]["tiny"].as_bool().unwrap_or(false)
    }
    fn latest(&self) -> String {
        if !self.partial.is_empty() {
            self.partial.clone()
        } else {
            self.lines.last().cloned().unwrap_or_default()
        }
    }
    fn previous(&self) -> String {
        if !self.partial.is_empty() {
            self.lines.last().cloned().unwrap_or_default()
        } else if self.lines.len() >= 2 {
            self.lines[self.lines.len() - 2].clone()
        } else {
            String::new()
        }
    }

    fn refresh(&self, h: &Host) {
        let send = h.sender("Captions");
        std::thread::spawn(move || {
            let out = Command::new("python3").args(["-I", &util::script("dictate.py"), "status"]).stdin(Stdio::null()).stderr(Stdio::null()).output();
            if let Ok(o) = out {
                if let Ok(v) = serde_json::from_slice::<Value>(&o.stdout) {
                    send(Box::new(Status(v)));
                }
            }
        });
    }

    /// Run the helper while wanted (enabled and ready), stop it otherwise
    fn follow(&mut self, h: &mut Host) {
        let enabled = Self::enabled(h);
        let lang = h.cfg()["captions"]["language"].as_str().filter(|s| !s.is_empty()).unwrap_or("auto").to_string();
        if enabled != self.was_enabled {
            self.was_enabled = enabled;
            if enabled && !self.ready() {
                self.refresh(h); // (installed since it was last looked at?)
            }
            self.lines.clear();
            self.partial.clear();
            self.error.clear();
        }
        let want = enabled && self.ready();
        // (the language changed: started again with it)
        if self.kept.running() && (!want || lang != self.language) {
            self.kept.stop();
        }
        if want && !self.kept.running() {
            self.language = lang.clone();
            let cmd: Vec<String> = [util::script("watch.sh"), "python3".into(), "-I".into(), util::script("captions.py"), "run".into(), lang].into();
            let (s1, s2) = (h.sender("Captions"), h.sender("Captions"));
            let gen = self.kept.next_gen();
            if self.kept.start(&cmd, move |l| s1(Box::new(Line(l))), move || s2(Box::new(Ended(gen)))) {
                self.error.clear();
            }
        }
        self.send(h);
    }

    fn video_rect(&self, h: &Host, screen: &str) -> Value {
        let full = json!({"x": 0, "y": 0, "w": 1280, "h": 800});
        let Some(w) = h.st.shell.get::<Windows>() else { return full };
        let Some(mon) = w.monitor(screen) else { return full };
        let (lw, lh) = mon.logical();
        let full = json!({"x": 0, "y": 0, "w": lw, "h": lh});
        // (the window in front on that display, else the whole display)
        match w.active_client() {
            Some(c) if c.monitor == mon.id && !c.hidden && c.size.0 > 0 => json!({"x": c.at.0 - mon.x, "y": c.at.1 - mon.y, "w": c.size.0, "h": c.size.1}),
            _ => full,
        }
    }

    /// What the surface shows (CaptionsFeed.qml's `feed`), when it changed
    fn send(&mut self, h: &mut Host) {
        let ready = self.ready();
        let shown = Self::enabled(h) && Self::placement(h) != "island";
        let screen = h.st.shell.get::<Windows>().and_then(|w| w.focused_monitor().map(|m| m.name.clone())).unwrap_or_default();
        let latest = if !ready {
            "Live captions need setting up: Settings → Accessibility".to_string()
        } else if !self.error.is_empty() {
            self.error.clone()
        } else if !self.latest().is_empty() {
            self.latest()
        } else {
            "Captions appear here when something speaks".into()
        };
        let previous = if ready && self.error.is_empty() { self.previous() } else { String::new() };
        let hint = !ready || !self.error.is_empty() || self.latest().is_empty();
        let dim = ready && self.error.is_empty() && self.quiet && self.partial.is_empty();
        let lines_hint = if !self.error.is_empty() {
            self.error.clone()
        } else if !self.latest().is_empty() {
            String::new()
        } else if !ready {
            "Live captions need setting up: Settings → Accessibility".into()
        } else {
            "Captions appear here when something speaks".into()
        };
        let c = &h.cfg()["captions"];
        let state = json!({
            "on": shown, "placement": Self::placement(h), "screen": screen, "latest": latest, "previous": previous, "hint": hint, "dim": dim,
            "bar": {"x": c["barX"].as_f64().unwrap_or(-1.0), "y": c["barY"].as_f64().unwrap_or(-1.0)},
            "video": self.video_rect(h, &screen),
            "lines": {"hint": lines_hint, "previous": self.previous(), "latest": self.latest()},
        });
        if self.last.as_ref() != Some(&state) {
            self.last = Some(state.clone());
            h.tell("captions", json!({"state": state}));
        }
    }
}

impl Service for CaptionsSvc {
    fn name(&self) -> &'static str {
        "Captions"
    }
    fn calls(&self) -> &'static [&'static str] {
        &["captions"]
    }
    fn ipc_targets(&self) -> &'static [&'static str] {
        &["captions"]
    }
    fn start(&mut self, h: &mut Host) {
        h.st.shell.want("clients");
        self.refresh(h);
        self.follow(h);
    }
    fn config_changed(&mut self, h: &mut Host) {
        self.follow(h);
    }
    fn outputs_changed(&mut self, h: &mut Host) {
        self.last = None;
        self.send(h);
    }
    fn call(&mut self, h: &mut Host, _call: &str, args: &Value) {
        match args[0].as_str().unwrap_or("") {
            "off" => util::cfg_set("captions.enabled", json!(false)),
            // the shortcut (Super+Alt+C by default), as the IPC one
            "toggle" => util::cfg_set("captions.enabled", json!(!Self::enabled(h))),
            "place" => {
                if let (Some(x), Some(y)) = (args[1].as_f64(), args[2].as_f64()) {
                    util::cfg_set("captions.barX", json!(x.max(0.0).round() as i64));
                    util::cfg_set("captions.barY", json!(y.max(0.0).round() as i64));
                }
            }
            "state" => {
                // (a surface asks again: it just started)
                self.last = None;
                self.send(h);
            }
            _ => {}
        }
    }
    fn ipc(&mut self, h: &mut Host, _target: &str, func: &str, args: &[String]) -> Option<String> {
        match func {
            "toggle" => util::cfg_set("captions.enabled", json!(!Self::enabled(h))),
            "on" => util::cfg_set("captions.enabled", json!(true)),
            "off" => util::cfg_set("captions.enabled", json!(false)),
            // (to see how they look: captions demo "a sentence" "the next")
            "demo" => {
                self.lines = vec![args.first().cloned().unwrap_or_default()];
                self.partial = args.get(1).cloned().unwrap_or_default();
                self.quiet = false;
                self.send(h);
            }
            _ => {}
        }
        None
    }
    fn message(&mut self, h: &mut Host, msg: Box<dyn std::any::Any + Send>) {
        let msg = match msg.downcast::<Status>() {
            Ok(s) => {
                self.info = s.0;
                return self.follow(h);
            }
            Err(m) => m,
        };
        let msg = match msg.downcast::<Line>() {
            Ok(l) => {
                let Ok(m) = serde_json::from_str::<Value>(&l.0) else { return };
                if let Some(p) = m["partial"].as_str() {
                    self.partial = p.into();
                    self.quiet = false;
                } else if let Some(f) = m["final"].as_str() {
                    self.partial.clear();
                    if !f.is_empty() {
                        self.lines.push(f.into());
                        let n = self.lines.len();
                        if n > 2 {
                            self.lines.drain(..n - 2);
                        }
                    }
                    self.quiet = false;
                } else if m["quiet"].as_bool().unwrap_or(false) {
                    self.quiet = true;
                } else if let Some(e) = m["error"].as_str().filter(|e| !e.is_empty()) {
                    self.error = e.into();
                }
                return self.send(h);
            }
            Err(m) => m,
        };
        let msg = match msg.downcast::<Ended>() {
            Ok(e) => {
                if self.kept.reap_if(e.0) {
                    // (KeptProcess: started again if still wanted, after a pause)
                    util::later(h.sender("Captions"), 3000, Box::new(Retry));
                }
                return;
            }
            Err(m) => m,
        };
        if msg.downcast::<Retry>().is_ok() {
            self.follow(h);
        }
    }
    fn quit(&mut self, _h: &mut Host) {
        self.kept.stop();
    }
}

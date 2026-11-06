//! Screenshots, OCR, screen translation, recording and circle to search
//! (shell/services/ScreenTools.qml), plus the screenshot editor's session
//! hand-off (modules/screentools/editor/ScreenshotEditor.qml: the editing
//! itself lives in the Rust surface, which this only sends screenshots to).
//! Region picking happens in the surface; this does the capturing and the
//! follow-up, with the same tools (grim, tesseract, trans, swappy, wl-copy).

use super::service::*;
use super::{files, ocr, util};
use serde_json::{json, Value};
use std::process::{Command, Stdio};
use tessera_core::host::Host;

/// ScreenGeometry.clip: a rect inside the screen
pub fn clip(r: (f64, f64, f64, f64), w: f64, h: f64) -> (f64, f64, f64, f64) {
    if ![r.0, r.1, r.2, r.3, w, h].iter().all(|v| v.is_finite()) || w <= 0.0 || h <= 0.0 {
        return (0.0, 0.0, 0.0, 0.0);
    }
    let left = r.0.clamp(0.0, w);
    let top = r.1.clamp(0.0, h);
    let right = (r.0 + r.2.max(0.0)).clamp(0.0, w).max(left);
    let bottom = (r.1 + r.3.max(0.0)).clamp(0.0, h).max(top);
    (left, top, right - left, bottom - top)
}

/// ScreenGeometry.integerClip: whole pixels, growing outwards
pub fn integer_clip(r: (f64, f64, f64, f64), w: f64, h: f64) -> (f64, f64, f64, f64) {
    let c = clip(r, w, h);
    let (left, top) = (c.0.floor(), c.1.floor());
    let right = w.floor().min((c.0 + c.2).ceil());
    let bottom = h.floor().min((c.1 + c.3).ceil());
    (left, top, (right - left).max(0.0), (bottom - top).max(0.0))
}

/// ScreenTools.geometry: "x,y wxh" in global coordinates for grim / the recorder
pub fn geometry(s: &Screen, r: (f64, f64, f64, f64)) -> String {
    let r = integer_clip(r, s.w, s.h);
    format!("{},{} {}x{}", (s.x + r.0).round() as i64, (s.y + r.1).round() as i64, r.2.round() as i64, r.3.round() as i64)
}

impl ScreenSvc {
    pub(super) fn shots_dir() -> String {
        format!("{}/Pictures/Screenshots", util::home())
    }

    // ── Selecting ───────────────────────────────────────────────────────
    pub(super) fn select(&mut self, h: &mut Host, mode: &str) {
        if (mode == "record" || mode == "record-window") && self.recording {
            return self.stop(h);
        }
        h.ask("panels.closeOpen", json!([]));
        self.selecting_screen = focused(h).map(|s| s.name).unwrap_or_default();
        self.selecting = mode.into();
    }

    pub(super) fn cancel_select(&mut self, _h: &mut Host) {
        self.selecting.clear();
    }

    /// The overlay is hidden and a region was chosen
    pub(super) fn captured(&mut self, h: &mut Host, mode: &str, scr: Option<Screen>, rect: (f64, f64, f64, f64)) {
        self.selecting.clear();
        let Some(scr) = scr else { return };
        let rect = clip(rect, scr.w, scr.h);
        if rect.2 < 1.0 || rect.3 < 1.0 {
            return;
        }
        let geo = geometry(&scr, rect);
        match mode {
            "screenshot" => self.shoot(h, &["-g".into(), geo], true),
            "ocr" => self.run(h, "ocr", self.ocr_command(&geo)),
            "translate" => {
                self.translation = Some(json!({"x": rect.0, "y": rect.1, "w": rect.2, "h": rect.3, "screen": scr.name, "source": "", "text": "", "busy": true, "error": ""}));
                self.run(h, "translate", self.ocr_command(&geo));
            }
            "record" | "record-window" => self.start_recording(h, geo),
            "ask" | "circle" => self.ask_about(h, &["-g".into(), geo], mode),
            _ => {}
        }
    }

    fn ocr_command(&self, geo: &str) -> String {
        format!("grim -g '{}' - | tesseract stdin stdout -l '{}' 2>/dev/null", geo, self.ocr_lang_arg())
    }

    /// Ask AI about the screen: capture as JPEG, attach to the next chat
    /// message. The pictures stay owner-only in the runtime folder; ones over
    /// a day old go.
    fn ask_about(&mut self, h: &mut Host, args: &[String], kind: &str) {
        let dir = format!("{}/tessera-ai", util::runtime());
        let file = format!("{}/shot-{}.jpg", dir, tessera_core::util::now_ms());
        let a: Vec<String> = args.iter().map(|x| util::q(x)).collect();
        let script = format!(
            "umask 077; mkdir -p {d} && find {d} -name 'shot-*.jpg' -mmin +1440 -delete; grim -t jpeg -q 85 {a} {f} && echo {f} && base64 -w0 {f}",
            d = util::q(&dir), a = a.join(" "), f = util::q(&file)
        );
        self.run(h, kind, script);
    }

    pub(super) fn ask_screen(&mut self, h: &mut Host) {
        h.ask("panels.closeOpen", json!([]));
        let mon = focused(h).map(|s| s.name);
        // (a moment later: the panel has gone)
        util::later(h.sender("ScreenTools"), 100, Box::new(AskShoot(mon.map(|m| vec!["-o".into(), m]).unwrap_or_default())));
    }

    pub(super) fn screenshot_screen(&mut self, h: &mut Host) {
        h.ask("panels.closeOpen", json!([]));
        let mon = focused(h).map(|s| s.name);
        util::later(h.sender("ScreenTools"), 100, Box::new(Shoot(mon.map(|m| vec!["-o".into(), m]).unwrap_or_default())));
    }

    /// `area`: an area screenshot, which opens for markup (Swappy) when that's on
    fn shoot(&mut self, h: &mut Host, args: &[String], area: bool) {
        let dir = Self::shots_dir();
        let file = format!("{}/Screenshot {}-{}.png", dir, util::stamp(true), util::nonce());
        let a: Vec<String> = args.iter().map(|x| util::q(x)).collect();
        let kind = if area && !self.has_editor(h) && h.cfg()["screenshot"]["markup"].as_bool().unwrap_or(false) && util::has("swappy") { "shotMarkup" } else { "shot" };
        let script = format!("mkdir -p {d} && grim {a} {f} && wl-copy < {f} && echo {f}", d = util::q(&dir), a = a.join(" "), f = util::q(&file));
        self.run(h, kind, script);
    }

    // ── Jobs ────────────────────────────────────────────────────────────
    /// One shell job; its output handled by kind
    pub(super) fn run(&mut self, h: &mut Host, kind: &str, script: String) {
        let (send, kind) = (h.sender("ScreenTools"), kind.to_string());
        std::thread::spawn(move || {
            let out = Command::new("sh").args(["-c", &script]).stdin(Stdio::null()).stderr(Stdio::null()).output();
            let (ok, text) = match out {
                Ok(o) => (o.status.success(), String::from_utf8_lossy(&o.stdout).into_owned()),
                Err(_) => (false, String::new()),
            };
            send(Box::new(JobDone(kind, ok, text)));
        });
    }

    fn set_translation(&mut self, patch: Value) {
        let mut t = self.translation.clone().unwrap_or_else(|| json!({}));
        if let (Some(o), Some(p)) = (t.as_object_mut(), patch.as_object()) {
            for (k, v) in p {
                o.insert(k.clone(), v.clone());
            }
        }
        self.translation = Some(t);
    }

    fn finish(&mut self, h: &mut Host, kind: &str, out: &str) {
        match kind {
            "shot" | "shotMarkup" if !out.trim().is_empty() => {
                let file = out.trim().to_string();
                h.ask("sound", json!(["screenshot"]));
                // (its words, for Search: in the background, later)
                self.index_soon(h);
                if self.has_editor(h) {
                    // floating thumbnail; click it to mark up
                    return self.editor_shot(h, &file);
                }
                // Markup: Swappy edits the saved file (its Save writes it back)
                if kind == "shotMarkup" {
                    return util::detached(&["swappy", "-f", &file, "-o", &file]);
                }
                let m = util::file_manager(h.cfg());
                util::detached(&[
                    "sh", "-c",
                    "a=$(timeout 600 notify-send -a Screenshot -i \"$1\" -A open=Open -A folder='Show in Folder' 'Screenshot saved' 'Copied to the clipboard'); [ \"$a\" = open ] && xdg-open \"$1\"; [ \"$a\" = folder ] && ${2:-xdg-open} \"$(dirname \"$1\")\"",
                    "sh", &file, &m,
                ]);
            }
            "ocr" => {
                let text = out.trim();
                if text.is_empty() {
                    return util::notify("Text Capture", "No text found", &format!("Try selecting a larger area with clearer text.{}", self.missing_hint()));
                }
                util::detached(&["wl-copy", text]);
                let words = text.split_whitespace().count();
                let head: String = text.chars().take(200).collect();
                util::notify("Text Capture", &format!("Copied {} {}", words, if words == 1 { "word" } else { "words" }), &head);
            }
            "translate" => {
                let src = out.trim().split('\n').map(str::trim).collect::<Vec<_>>().join(" ");
                if src.is_empty() {
                    return self.set_translation(json!({"busy": false, "error": "No text found in that area."}));
                }
                self.set_translation(json!({"source": src}));
                let t = h.cfg()["sidebar"]["translateTarget"].as_str().unwrap_or("en").to_string();
                // ("--": text read off the screen that starts with "-" is never an option)
                self.run(h, "translated", format!("trans -brief -no-ansi -t {} -- {}", util::q(&t), util::q(&src)));
            }
            "ask" | "circle" => {
                let Some(nl) = out.find('\n') else { return };
                let path = &out[..nl];
                let scr = if kind == "circle" { self.selecting_screen.clone() } else { focused(h).map(|s| s.name).unwrap_or_default() };
                // the sidebar drawn by tessera-ui: the picture goes to its chat, which reads the file itself
                h.say(json!({"cmd": "tab", "surface": "sidebar", "tab": 0}));
                h.ask("panels.show", json!(["sidebar", scr]));
                h.say(json!({"cmd": "attach", "surface": "sidebar", "path": path, "lens": kind == "circle"}));
            }
            "translated" => self.set_translation(json!({"text": out.trim(), "busy": false})),
            _ => {}
        }
    }

    fn failed(&mut self, _h: &mut Host, kind: &str) {
        if kind == "translate" {
            self.set_translation(json!({"busy": false, "error": "Could not read text from the screen."}));
        }
        match kind {
            "shot" | "shotMarkup" | "ask" | "circle" => util::notify("Screenshot", "Couldn't take the screenshot", "Make sure grim is installed."),
            "ocr" | "translate" => util::notify("Text Capture", "Couldn't read the screen", &format!("Install tesseract and tesseract-data-eng, then try again.{}", self.missing_hint())),
            "translated" => self.set_translation(json!({"busy": false, "error": "Couldn't translate. Check your connection and that translate-shell is installed."})),
            _ => {}
        }
    }

    /// "For text in your language, install tesseract-data-deu."
    fn missing_hint(&self) -> String {
        let m: Vec<String> = self.ocr_missing.iter().filter(|c| !c.is_empty() && c.chars().all(|x| x.is_ascii_lowercase() || x == '_')).map(|c| format!("tesseract-data-{c}")).collect();
        if m.is_empty() { String::new() } else { format!(" For text in your language, install {}.", m.join(" and ")) }
    }

    // ── OCR languages and installing ────────────────────────────────────
    pub(super) fn list_languages(&mut self, h: &mut Host) {
        if !self.has_tesseract() {
            return;
        }
        let send = h.sender("ScreenTools");
        std::thread::spawn(move || {
            let text = Command::new("tesseract").arg("--list-langs").stdin(Stdio::null()).stderr(Stdio::null()).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
            // the keyboard layouts (KeyboardLayout.layouts / variants): hyprctl's main keyboard
            let mut layouts = Vec::new();
            if let Ok(d) = tessera_core::services::hypr::request("j/devices") {
                if let Ok(v) = serde_json::from_str::<Value>(&d) {
                    let kb = v["keyboards"].as_array().and_then(|a| a.iter().find(|k| k["main"] == json!(true)).or_else(|| a.first()).cloned()).unwrap_or_default();
                    let (l, var) = (kb["layout"].as_str().unwrap_or("").to_string(), kb["variant"].as_str().unwrap_or("").to_string());
                    let vars: Vec<&str> = var.split(',').collect();
                    for (i, lay) in l.split(',').filter(|s| !s.is_empty()).enumerate() {
                        layouts.push((lay.to_string(), vars.get(i).copied().unwrap_or("").to_string()));
                    }
                }
            }
            send(Box::new(LangList(text, layouts)));
        });
    }

    fn languages_listed(&mut self, text: String, layouts: Vec<(String, String)>) {
        let env: Vec<(&str, String)> = ["LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE"].iter().map(|k| (*k, std::env::var(k).unwrap_or_default())).collect();
        let (langs, missing) = ocr::pick(&env, &text, &layouts);
        let have = ocr::installed_set(&text);
        self.ocr_langs = langs.join("+");
        self.ocr_missing = missing;
        let w = ocr::wanted(&env, &layouts);
        let mut codes = vec!["eng".to_string()];
        codes.extend(w.into_iter().filter(|c| c != "eng"));
        self.ocr_languages = codes.iter().map(|c| json!({"code": c, "name": ocr::name_of(c), "installed": have.contains(c)})).collect();
    }

    /// Tesseract or a language through Software Update's job (never pacman here)
    pub(super) fn install_ocr(&mut self, h: &mut Host, code: &str) {
        if !self.ocr_installing.is_empty() {
            return;
        }
        self.ocr_installing = if !code.is_empty() && code.chars().all(|c| c.is_ascii_lowercase() || c == '_') { vec![format!("tesseract-data-{code}")] } else { vec!["tesseract".into(), "tesseract-data-eng".into()] };
        self.ocr_install_error.clear();
        h.ask("system.update", json!(["--install", self.ocr_installing.join(",")]));
    }

    pub(super) fn install_done(&mut self, h: &mut Host, ok: bool, why: String) {
        if !self.ocr_installing.is_empty() {
            self.ocr_install_error = if ok { String::new() } else if why.is_empty() { "The install didn't finish.".into() } else { why };
        }
        self.ocr_installing.clear();
        self.list_languages(h);
    }

    // ── The screenshot editor's session ─────────────────────────────────
    /// ScreenshotEditor.shot: a new screenshot floats in as a thumbnail, or
    /// (thumbnails off) the notification with Edit
    pub(super) fn editor_shot(&mut self, h: &mut Host, file: &str) {
        if !h.cfg()["screenshot"]["thumbnail"].as_bool().unwrap_or(true) {
            let send = h.sender("ScreenTools");
            let file = file.to_string();
            std::thread::spawn(move || {
                let out = Command::new("timeout").args(["600", "notify-send", "-a", "Screenshot", "-i", &file, "-A", "edit=Edit", "-A", "folder=Show in Folder", "Screenshot saved", "Copied to the clipboard"]).stdin(Stdio::null()).stderr(Stdio::null()).output();
                let a = out.map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
                send(Box::new(NotifyAction(file, a)));
            });
            return;
        }
        self.to_rust(h, "shot", file, None);
    }

    pub(super) fn open_editor(&mut self, h: &mut Host, file: &str) {
        if !file.is_empty() {
            self.to_rust(h, "edit", file, None);
        }
    }

    // ── Messages ────────────────────────────────────────────────────────
    pub(super) fn on_message(&mut self, h: &mut Host, msg: Box<dyn std::any::Any + Send>) {
        macro_rules! take {
            ($t:ty, $m:ident, $body:expr) => {
                match $m.downcast::<$t>() {
                    Ok(v) => return $body(*v),
                    Err(m) => m,
                }
            };
        }
        let msg = take!(JobDone, msg, |j: JobDone| {
            self.finish(h, &j.0, &j.2);
            if !j.1 {
                self.failed(h, &j.0);
            }
        });
        let msg = take!(Shoot, msg, |s: Shoot| self.shoot(h, &s.0, false));
        let msg = take!(AskShoot, msg, |s: AskShoot| self.ask_about(h, &s.0, "ask"));
        let msg = take!(LangList, msg, |l: LangList| self.languages_listed(l.0, l.1));
        let msg = take!(NotifyAction, msg, |n: NotifyAction| {
            if n.1 == "edit" {
                self.open_editor(h, &n.0);
            } else if n.1 == "folder" {
                files::reveal(h.cfg(), &n.0);
            }
        });
        let msg = take!(Mics, msg, |m: Mics| self.mics = m.0);
        let msg = take!(RecExit, msg, |r: RecExit| self.rec_exited(h, r.0, r.1, r.2));
        let msg = take!(Tick, msg, |t: Tick| self.tick(h, t.0));
        let msg = take!(StartRec, msg, |s: StartRec| self.launch(h, s.0, false, false));
        let msg = take!(DropNow, msg, |d: DropNow| self.drop_now(d.0));
        let msg = take!(ReadDone, msg, |r: ReadDone| self.lt_finished(h, r.0, r.1, r.2));
        let msg = take!(TelHandler, msg, |t: TelHandler| self.tel_handler = t.0);
        let msg = take!(IndexNow, msg, |_| self.index_now(h));
        let _ = take!(IndexEnded, msg, |_| self.indexing_now = false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regions_stay_on_the_screen() {
        assert_eq!(clip((-10.0, 5.0, 100.0, 50.0), 80.0, 40.0), (0.0, 5.0, 80.0 - 0.0, 35.0));
        assert_eq!(clip((0.0, 0.0, 10.0, 10.0), 0.0, 0.0), (0.0, 0.0, 0.0, 0.0));
        assert_eq!(integer_clip((1.5, 1.5, 2.0, 2.0), 100.0, 100.0), (1.0, 1.0, 3.0, 3.0));
    }

    #[test]
    fn grim_gets_global_coordinates() {
        let s = Screen { name: "DP-2".into(), x: 1920.0, y: 0.0, w: 1920.0, h: 1080.0 };
        assert_eq!(geometry(&s, (10.0, 20.0, 300.0, 200.0)), "1930,20 300x200");
    }
}

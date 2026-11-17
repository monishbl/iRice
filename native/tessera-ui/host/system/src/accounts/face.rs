//! Face.qml: face unlock for the lock screen (assets/scripts/face.py), with
//! Howdy. An infrared camera as before; a regular camera (17 Nov) with a
//! blink as well (face_live.py, the tessera-face-rgb PAM file: core::face).
//! Setting up, adding and removing faces go through the root helper (the
//! password), only on a user action; the blink check's own parts (its eye
//! model, downloaded when set up, and your face's features) are yours, in
//! ~/.local/share/tessera/face.
//!
//! Bus "face": info, camera (the one used or offered), cameras, hasCamera,
//! kind ("ir" | "rgb": the camera set up, else the one offered), device,
//! installed, setUp, faces, lockScreen, step/steps (setup progress),
//! progress (the eye model's download, 0-100, -1 when not downloading),
//! tryText/tryState ("" | "running" | "ok" | "no"), admin, installing,
//! installAsked, installError, busy, error.
//! Calls: face.refresh|install|setUpCamera|addFace|clearFaces|chooseCamera
//! [path]|tryIt|stopTry|setAdmin [bool]|updatesFinished [job].

use super::common::*;
use crate::util::config_set;
use serde_json::{json, Value};
use std::any::Any;
use tessera_core::host::{Host, Service};

enum M {
    Status(Value),
    Job(i32, String, String),
    Progress(i64),
    TryLine(String),
    TryDone(i32),
}

#[derive(Default)]
pub struct Face {
    tx: Option<Tx>,
    info: Value,
    install_asked: bool,
    install_error: String,
    busy: String,
    error: String,
    running: bool,
    /// jobs still to run after this one (adding a face with a regular camera
    /// is two: your features for the blink, then Howdy's)
    then: Vec<String>,
    progress: i64,
    try_state: String,
    try_text: String,
    try_checked: bool,
    try_child: Child,
    // config.json: lock.faceDevice, lock.faceCount, lock.face, lock.faceKind,
    // lock.faceCamera, lock.faceAdmin
    cfg_device: String,
    cfg_count: i64,
    cfg_face: bool,
    cfg_kind: String,
    cfg_camera: String,
    cfg_admin: bool,
}

/// The camera to offer: the one picked (lock.faceCamera) when it's there,
/// else the first (infrared first, face.py's order)
pub fn pick_camera<'a>(cameras: &'a Value, picked: &str) -> Option<&'a Value> {
    let list = cameras.as_array()?;
    list.iter().find(|c| !picked.is_empty() && c["path"] == picked).or_else(|| list.first())
}

/// What a camera of this kind needs, in order, and how far along it is:
/// (step done so far, steps). Infrared: install, set up, add a face.
/// Regular: install (Howdy and OpenCV), set up (with the eye model), add.
pub fn steps(installed: bool, set_up: bool, faces: i64) -> (i64, i64) {
    let done = if !installed {
        0
    } else if !set_up {
        1
    } else if faces == 0 {
        2
    } else {
        3
    };
    (done, 3)
}

/// The blink check can run: OpenCV for Python, its eye model, your
/// features, pam_exec (face.py's `live` and `pamExec`)
pub fn live_ready(info: &Value) -> bool {
    let l = &info["live"];
    l["cv2"].as_bool().unwrap_or(false) && l["eyeModel"].as_bool().unwrap_or(false) && l["enrolled"].as_i64().unwrap_or(0) > 0 && info["pamExec"].as_bool().unwrap_or(false)
}

/// "Try it"'s verdict from the conversation's end (pam_exec says "system
/// error" for any no from the blink check: one that spoke looked)
pub fn try_verdict(code: i32, checked: bool, said: &str) -> (&'static str, String) {
    match code {
        0 => ("ok", "It's you. Face unlock works.".into()),
        1 => ("no", if said.is_empty() { "Not recognised. Try again in good light, or add your face again.".into() } else { format!("{said}. Try again.") }),
        _ if checked => ("no", if said.is_empty() { "No blink seen. Try again.".into() } else { format!("{said}. Try again.") }),
        _ => ("no", "The camera couldn't be used. Set face unlock up again, or restart.".into()),
    }
}

impl Face {
    fn tx(&self) -> Tx {
        self.tx.clone().expect("started")
    }
    fn py(args: &[&str]) -> Vec<String> {
        let mut v = strs(&["python3", "-I"]);
        v.push(script("face.py"));
        v.extend(args.iter().map(|s| s.to_string()));
        v
    }
    fn live(args: &[&str]) -> Vec<String> {
        let mut v = strs(&["python3", "-I"]);
        v.push(script("face_live.py"));
        v.extend(args.iter().map(|s| s.to_string()));
        v
    }
    fn refresh(&self) {
        let tx = self.tx();
        std::thread::spawn(move || {
            let (_, out, _) = run_full(&Self::py(&["status"]), None);
            if let Ok(v) = serde_json::from_str::<Value>(&out) {
                tx.send(M::Status(v));
            }
        });
    }
    fn read_cfg(&mut self, cfg: &Value) {
        let l = &cfg["lock"];
        self.cfg_device = l["faceDevice"].as_str().unwrap_or("").to_string();
        self.cfg_count = l["faceCount"].as_i64().unwrap_or(0);
        self.cfg_face = l["face"].as_bool().unwrap_or(true);
        self.cfg_kind = l["faceKind"].as_str().unwrap_or("ir").to_string();
        self.cfg_camera = l["faceCamera"].as_str().unwrap_or("").to_string();
        self.cfg_admin = l["faceAdmin"].as_bool().unwrap_or(false);
    }
    /// The camera offered (or set up): {path, name, kind}
    fn camera(&self) -> Option<&Value> {
        let picked = if self.cfg_device.is_empty() { &self.cfg_camera } else { &self.cfg_device };
        pick_camera(&self.info["cameras"], picked).or_else(|| self.info.get("camera").filter(|c| !c.is_null()))
    }
    fn kind(&self) -> String {
        // (set up: what it was set up as; else the one offered)
        if !self.cfg_device.is_empty() {
            return if self.cfg_kind == "rgb" { "rgb".into() } else { "ir".into() };
        }
        self.camera().and_then(|c| c["kind"].as_str()).unwrap_or("ir").to_string()
    }
    fn howdy_installed(&self) -> bool {
        self.info["howdy"].as_bool().unwrap_or(false) && self.info["pam"].as_bool().unwrap_or(false)
    }
    fn installed(&self) -> bool {
        self.howdy_installed() && (self.kind() == "ir" || self.info["live"]["cv2"].as_bool().unwrap_or(false))
    }
    fn set_up(&self) -> bool {
        let i = &self.info;
        // The camera Howdy uses: from its config, or (howdy-next keeps that
        // root's to read) the one Tessera set it up with. Both when Howdy's
        // can be read: a camera picked since (lock.faceDevice cleared) or
        // set elsewhere isn't set up, so the lock never uses Howdy on one
        // camera with the other kind's PAM file (a regular camera without
        // the blink)
        let howdy = i["device"].as_str().unwrap_or("");
        let private = i["private"].as_bool().unwrap_or(false);
        let same = !self.cfg_device.is_empty() && (private || howdy == self.cfg_device);
        let eye = self.kind() == "ir" || i["live"]["eyeModel"].as_bool().unwrap_or(false);
        self.installed() && same && i["models"].as_bool().unwrap_or(false) && eye
    }
    fn publish(&self) {
        let i = &self.info;
        let kind = self.kind();
        let set_up = self.set_up();
        let lock = set_up && self.cfg_count > 0 && self.cfg_face && (kind == "ir" || live_ready(i));
        let (step, total) = steps(self.installed(), set_up, self.cfg_count);
        put(
            "face",
            json!({"info": i, "camera": self.camera(), "cameras": i["cameras"].clone(), "hasCamera": self.camera().is_some(),
                   "kind": kind, "device": self.cfg_device, "installed": self.installed(),
                   "setUp": set_up, "faces": self.cfg_count, "lockScreen": lock, "step": step, "steps": total,
                   "progress": self.progress, "tryState": self.try_state, "tryText": self.try_text, "admin": self.cfg_admin,
                   "installing": updates_running() && self.install_asked, "installError": self.install_error,
                   "busy": self.busy, "error": self.error}),
        );
    }
    /// A job: the root helper's (setup, add, clear, admin, adminOff) or the
    /// blink check's own, as you (models, enrol, liveClear)
    fn run(&mut self, job: &str) {
        if self.running {
            return;
        }
        self.error.clear();
        let cam = self.camera().cloned().unwrap_or(Value::Null);
        let path = cam["path"].as_str().unwrap_or("").to_string();
        let device = if self.cfg_device.is_empty() { path.clone() } else { self.cfg_device.clone() };
        let kind = self.kind();
        let cmd = match job {
            "models" => Self::live(&["models"]),
            "enrol" => Self::live(&["enrol", "--device", &device]),
            "liveClear" => Self::live(&["clear"]),
            _ => {
                if !has_tool("pkexec") {
                    self.error = NO_PKEXEC.into();
                    self.then.clear();
                    self.publish();
                    return;
                }
                let args: Vec<String> = match job {
                    "setup" => strs(&["face", "setup", &path]),
                    "admin" => strs(&["face", "admin", "on", &device, &kind]),
                    "adminOff" => strs(&["face", "admin", "off"]),
                    _ => strs(&["face", job]),
                };
                let refs: Vec<&str> = args.iter().map(String::as_str).collect();
                root_command(&refs)
            }
        };
        self.busy = job.into();
        self.running = true;
        let tx = self.tx();
        let job = job.to_string();
        if job == "models" {
            self.progress = 0;
            let (t1, t2) = (tx.clone(), tx);
            let mut err = String::new();
            let ok = Child::default().spawn(
                &cmd,
                None,
                move |l| {
                    if let Some(n) = l.strip_prefix("PROGRESS ").and_then(|n| n.trim().parse().ok()) {
                        t1.send(M::Progress(n));
                    }
                },
                move |code| t2.send(M::Job(code, "models".into(), String::new())),
            );
            if !ok {
                err = "Python isn't there to download the eye model.".into();
            }
            if !err.is_empty() {
                self.running = false;
                self.busy.clear();
                self.error = err;
            }
        } else {
            std::thread::spawn(move || {
                let (code, out, err) = run_full(&cmd, None);
                tx.send(M::Job(code, job, format!("{err}\n{out}")));
            });
        }
        self.publish();
    }
    /// The next of a chain of jobs, if any
    fn next(&mut self) {
        if !self.then.is_empty() {
            let j = self.then.remove(0);
            self.run(&j);
        }
    }
    /// "Try it": the lock's own conversation (its PAM file, Howdy, the
    /// blink), never in a sandbox
    fn try_it(&mut self) {
        if self.try_child.running() || !self.set_up() {
            return;
        }
        if tessera_core::env::sandboxed() {
            self.try_state = "no".into();
            self.try_text = "Not in the sandbox: try it on the lock screen.".into();
            self.publish();
            return;
        }
        let pam = crate::util::root().join("shell/assets/pam").to_string_lossy().into_owned();
        let service = tessera_core::face::service(&self.kind());
        // (/proc/self/exe's real file: Child runs a path as given, so it's resolved here)
        let exe = std::fs::read_link("/proc/self/exe").map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
        if exe.is_empty() || exe.ends_with(" (deleted)") {
            self.try_state = "no".into();
            self.try_text = "Tessera was updated: restart it, then try again.".into();
            self.publish();
            return;
        }
        let cmd = strs(&["env", &format!("TESSERA_FACE_DEVICE={}", self.cfg_device), &exe, "--pam-helper", service, &pam]);
        self.try_state = "running".into();
        self.try_checked = false;
        self.try_text = if self.kind() == "rgb" { "Look at the camera and blink".into() } else { "Look at the camera".into() };
        let (t1, t2) = (self.tx(), self.tx());
        let ok = self.try_child.spawn(
            &cmd,
            Some("\n".into()),
            move |l| t1.send(M::TryLine(l)),
            move |code| t2.send(M::TryDone(code)),
        );
        if !ok {
            self.try_state = "no".into();
            self.try_text = "The check couldn't start.".into();
        }
        self.publish();
    }
}

impl Service for Face {
    fn name(&self) -> &'static str {
        "Face"
    }
    fn calls(&self) -> &'static [&'static str] {
        &["face"]
    }
    fn start(&mut self, h: &mut Host) {
        self.tx = Some(Tx::new(h.sender("Face")));
        self.info = json!({});
        self.progress = -1;
        let cfg = h.cfg().clone();
        self.read_cfg(&cfg);
        self.publish();
        self.refresh();
    }
    fn config_changed(&mut self, h: &mut Host) {
        let cfg = h.cfg().clone();
        self.read_cfg(&cfg);
        self.publish();
    }
    fn call(&mut self, h: &mut Host, call: &str, args: &Value) {
        match call.split('.').nth(1).unwrap_or("") {
            "refresh" => self.refresh(),
            "install" => {
                if updates_running() || self.installed() {
                    return;
                }
                // Howdy (AUR); a regular camera's blink check needs OpenCV for Python too
                let mut a: Vec<&str> = Vec::new();
                if !self.howdy_installed() {
                    a.extend(["--aur-install", "howdy-next"]);
                }
                if self.kind() == "rgb" && !self.info["live"]["cv2"].as_bool().unwrap_or(false) {
                    a.extend(["--install", "python-opencv"]);
                }
                if a.is_empty() {
                    return;
                }
                self.install_asked = true;
                self.install_error.clear();
                h.ask("updates.update", json!([a]));
                self.publish();
            }
            "updatesFinished" => {
                if !self.install_asked {
                    return;
                }
                self.install_asked = false;
                let j = &args[0];
                self.install_error = if j["state"] == "done" {
                    String::new()
                } else {
                    j["error"].as_str().filter(|s| !s.is_empty()).or(j["text"].as_str().filter(|s| !s.is_empty())).unwrap_or("The install didn't finish.").to_string()
                };
                self.refresh();
                self.publish();
            }
            "chooseCamera" => {
                // (only before it's set up: changing it means setting up again)
                if let Some(p) = args[0].as_str() {
                    if self.info["cameras"].as_array().is_some_and(|l| l.iter().any(|c| c["path"] == p)) {
                        self.cfg_camera = p.to_string();
                        config_set("lock.faceCamera", json!(p));
                        if !self.cfg_device.is_empty() && self.cfg_device != p {
                            // a different camera: set up again (faces stay Howdy's; the blink's too)
                            self.cfg_device.clear();
                            config_set("lock.faceDevice", json!(""));
                        }
                        self.publish();
                    }
                }
            }
            "setUpCamera" => {
                if self.camera().is_some() {
                    self.then = if self.camera().and_then(|c| c["kind"].as_str()) == Some("rgb") { vec!["models".into()] } else { vec![] };
                    self.run("setup");
                }
            }
            "addFace" => {
                if self.kind() == "rgb" {
                    // your features for the blink first (as you), then Howdy's (root)
                    self.then = vec!["add".into()];
                    if self.cfg_admin {
                        self.then.push("admin".into());
                    }
                    self.run("enrol");
                } else {
                    self.run("add");
                }
            }
            "clearFaces" => {
                self.then = vec!["liveClear".into()];
                self.run("clear");
            }
            "tryIt" => self.try_it(),
            "stopTry" => {
                self.try_child.stop();
            }
            "setAdmin" => {
                let on = args[0].as_bool().unwrap_or(false);
                if on != self.cfg_admin && self.set_up() && self.cfg_count > 0 {
                    self.run(if on { "admin" } else { "adminOff" });
                }
            }
            _ => {}
        }
    }
    fn message(&mut self, _h: &mut Host, msg: Box<dyn Any + Send>) {
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Status(v) => self.info = v,
            M::Progress(n) => self.progress = n,
            M::TryLine(l) => {
                let (word, rest) = l.split_once(' ').unwrap_or((l.as_str(), ""));
                if matches!(word, "INFO" | "ERROR") {
                    if let Some(h) = tessera_core::face::hint(rest) {
                        self.try_text = h.to_string();
                        self.try_checked = true;
                    }
                }
                if word == "RESULT" {
                    // "<code> <text>": 0 yes, 4 (system error: pam_exec's no, or broken), else no
                    let code = rest.split(' ').next().and_then(|c| c.parse::<i32>().ok()).unwrap_or(-1);
                    let said = if self.try_checked && self.try_text != "Blink to unlock" { self.try_text.clone() } else { String::new() };
                    let pam = if code == 0 { 0 } else if matches!(code, 4 | 9) { 2 } else { 1 };
                    let (s, t) = try_verdict(pam, self.try_checked, &said);
                    self.try_state = s.into();
                    self.try_text = t;
                }
            }
            M::TryDone(code) => {
                // (over without a RESULT: stopped, or the helper couldn't ask)
                if self.try_state == "running" {
                    let (s, t) = try_verdict(2, false, "");
                    self.try_state = s.into();
                    self.try_text = if code == -1 || code == 143 { "Stopped.".into() } else { t };
                }
            }
            M::Job(code, job, said_all) => {
                self.busy.clear();
                self.running = false;
                if job == "models" {
                    self.progress = -1;
                }
                if code == 0 {
                    match job.as_str() {
                        "setup" => {
                            let cam = self.camera().cloned().unwrap_or(Value::Null);
                            let path = cam["path"].as_str().unwrap_or("").to_string();
                            let kind = if cam["kind"] == "rgb" { "rgb" } else { "ir" };
                            self.cfg_device = path.clone();
                            self.cfg_kind = kind.into();
                            config_set("lock.faceDevice", json!(path));
                            config_set("lock.faceKind", json!(kind));
                        }
                        "add" => {
                            self.cfg_count += 1;
                            config_set("lock.faceCount", json!(self.cfg_count));
                        }
                        "clear" => {
                            self.cfg_count = 0;
                            config_set("lock.faceCount", json!(0));
                            // (the root helper ended administrator prompts too)
                            self.cfg_admin = false;
                            config_set("lock.faceAdmin", json!(false));
                        }
                        "admin" => {
                            self.cfg_admin = true;
                            config_set("lock.faceAdmin", json!(true));
                        }
                        "adminOff" => {
                            self.cfg_admin = false;
                            config_set("lock.faceAdmin", json!(false));
                        }
                        _ => {}
                    }
                    self.next();
                } else {
                    self.then.clear();
                    if code != 126 {
                        // (126: the password prompt was cancelled)
                        let said = said_all.trim().lines().filter(|l| !l.trim().is_empty()).last().unwrap_or("").to_string();
                        let said = root_message(&said);
                        self.error = if code == 127 {
                            "Not allowed (an administrator's password is needed).".into()
                        } else if job == "add" && self.kind() == "ir" {
                            format!("No face was found. Look at the camera, then try again. If the camera's infrared light stays off, it needs linux-enable-ir-emitter (AUR) set up once in a terminal: sudo linux-enable-ir-emitter configure.{}", if said.is_empty() { String::new() } else { format!(" ({said})") })
                        } else if job == "add" || job == "enrol" {
                            if said.is_empty() { "No face was found. Look at the camera in good light, then try again.".into() } else { said }
                        } else if job == "models" {
                            if said.is_empty() { "The eye model couldn't be downloaded. Check the internet connection, then try again.".into() } else { said }
                        } else if said.is_empty() {
                            "That didn't work. Update Tessera, then try again.".into()
                        } else {
                            said
                        };
                    }
                }
                self.refresh();
            }
        }
        self.publish();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_picked_camera_or_infrared_first() {
        let cams = json!([{"path": "/dev/video2", "kind": "ir"}, {"path": "/dev/video0", "kind": "rgb"}]);
        assert_eq!(pick_camera(&cams, "").unwrap()["path"], "/dev/video2");
        assert_eq!(pick_camera(&cams, "/dev/video0").unwrap()["kind"], "rgb");
        // (a camera that's gone: the first again)
        assert_eq!(pick_camera(&cams, "/dev/video9").unwrap()["path"], "/dev/video2");
        assert!(pick_camera(&json!([]), "").is_none());
        assert!(pick_camera(&Value::Null, "").is_none());
    }

    #[test]
    fn setup_progress() {
        assert_eq!(steps(false, false, 0), (0, 3));
        assert_eq!(steps(true, false, 0), (1, 3));
        assert_eq!(steps(true, true, 0), (2, 3));
        assert_eq!(steps(true, true, 2), (3, 3));
    }

    #[test]
    fn the_blink_check_needs_all_its_parts() {
        let all = json!({"pamExec": true, "live": {"cv2": true, "eyeModel": true, "enrolled": 5}});
        assert!(live_ready(&all));
        for (k, v) in [("cv2", json!(false)), ("eyeModel", json!(false)), ("enrolled", json!(0))] {
            let mut i = all.clone();
            i["live"][k] = v;
            assert!(!live_ready(&i), "{k}");
        }
        let mut i = all.clone();
        i["pamExec"] = json!(false);
        assert!(!live_ready(&i));
        assert!(!live_ready(&json!({})));
    }

    #[test]
    fn try_it_says_what_happened() {
        assert_eq!(try_verdict(0, true, "").0, "ok");
        assert_eq!(try_verdict(1, false, "").0, "no");
        // (pam_exec's "system error" after the check spoke: a miss, said as such)
        assert_eq!(try_verdict(2, true, "No blink seen"), ("no", "No blink seen. Try again.".to_string()));
        assert!(try_verdict(2, false, "").1.contains("couldn't be used"));
    }

    fn face_with(info: Value, cfg: Value) -> Face {
        let mut f = Face { info, ..Default::default() };
        f.read_cfg(&json!({"lock": cfg}));
        f
    }

    #[test]
    fn settings_state_for_a_regular_camera() {
        let info = json!({"howdy": true, "pam": true, "pamExec": true, "models": true, "private": true,
                          "cameras": [{"path": "/dev/video0", "name": "HD Webcam", "kind": "rgb"}],
                          "live": {"cv2": false, "eyeModel": false, "enrolled": 0}});
        // not set up yet: the regular camera is offered, OpenCV is still needed
        let f = face_with(info.clone(), json!({}));
        assert_eq!(f.kind(), "rgb");
        assert!(!f.installed());
        // OpenCV in, set up, but no eye model yet: not set up
        let mut i = info.clone();
        i["live"]["cv2"] = json!(true);
        let f = face_with(i.clone(), json!({"faceDevice": "/dev/video0", "faceKind": "rgb", "faceCount": 1}));
        assert!(f.installed() && !f.set_up());
        // everything: the lock may use it
        i["live"]["eyeModel"] = json!(true);
        i["live"]["enrolled"] = json!(5);
        let f = face_with(i, json!({"faceDevice": "/dev/video0", "faceKind": "rgb", "faceCount": 1}));
        assert!(f.set_up());
        assert!(live_ready(&f.info));
    }

    #[test]
    fn infrared_needs_no_blink_parts() {
        let info = json!({"howdy": true, "pam": true, "models": true, "private": true,
                          "cameras": [{"path": "/dev/video2", "name": "IR", "kind": "ir"}, {"path": "/dev/video0", "name": "HD", "kind": "rgb"}],
                          "live": {}});
        let f = face_with(info, json!({"faceDevice": "/dev/video2", "faceCount": 1}));
        assert_eq!(f.kind(), "ir");
        assert!(f.installed() && f.set_up());
        // (an old setup, before regular cameras: no faceKind, infrared)
        assert_eq!(f.cfg_kind, "ir");
    }

    #[test]
    fn another_camera_picked_is_not_set_up() {
        // Howdy still watches the regular camera; infrared picked since:
        // set up again first (the lock would otherwise skip the blink)
        let info = json!({"howdy": true, "pam": true, "models": true, "private": false, "device": "/dev/video0",
                          "cameras": [{"path": "/dev/video2", "kind": "ir"}, {"path": "/dev/video0", "kind": "rgb"}], "live": {}});
        let f = face_with(info.clone(), json!({"faceDevice": "", "faceKind": "rgb", "faceCamera": "/dev/video2", "faceCount": 1}));
        assert_eq!(f.kind(), "ir");
        assert!(!f.set_up());
        // (Howdy set elsewhere than Tessera's camera: not set up either)
        let f = face_with(info, json!({"faceDevice": "/dev/video2", "faceKind": "ir", "faceCount": 1}));
        assert!(!f.set_up());
    }
}

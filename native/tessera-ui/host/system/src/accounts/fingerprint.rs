//! Fingerprint.qml: fingerprints through assets/scripts/fingerprint.py: the
//! reader and what it still needs, the fingers enrolled, and where a finger
//! is accepted. Everything that needs root goes through the root helper's
//! command, exactly as the QML builds it, and only on a user action.
//!
//! State is published on the bus as "fingerprint" (the QML property names):
//! info, reader, hasReader, missing, ready, enrolled, pam, lockScreen,
//! installing, installError, slowStart, fixing, fixError, enrollStage,
//! enrollFinger, enrollStages, enrollCount, enrollHint, enrollError,
//! enrolling, pamBusy, pamError, scanned ({seq, result}: the `scanned`
//! signal). Calls: fingerprint.refresh|install|fix|enroll [finger]|
//! cancelEnroll|remove [finger]|setPam [where, on]|updatesFinished [job].

use super::common::*;
use serde_json::{json, Value};
use std::any::Any;
use std::time::Duration;
use tessera_core::host::{Host, Service};

enum M {
    Status(Value),
    Fix(Value),
    EnrollLine(Value),
    EnrollExit,
    Pam(i32, String),
    Refresh,
}

pub struct Fingerprint {
    tx: Option<Tx>,
    info: Value,
    install_asked: bool,
    install_error: String,
    fix_error: String,
    fixing: bool,
    enroll_stage: String,
    enroll_finger: String,
    enroll_stages: i64,
    enroll_count: i64,
    enroll_hint: String,
    enroll_error: String,
    pam_busy: String,
    pam_error: String,
    pam_running: bool,
    scan_seq: i64,
    scan_result: String,
    enroll: Child,
    later: slint::Timer,
    lock_on: bool,
}

impl Default for Fingerprint {
    fn default() -> Self {
        Self {
            tx: None,
            info: json!({}),
            install_asked: false,
            install_error: String::new(),
            fix_error: String::new(),
            fixing: false,
            enroll_stage: String::new(),
            enroll_finger: String::new(),
            enroll_stages: 0,
            enroll_count: 0,
            enroll_hint: String::new(),
            enroll_error: String::new(),
            pam_busy: String::new(),
            pam_error: String::new(),
            pam_running: false,
            scan_seq: 0,
            scan_result: String::new(),
            enroll: Child::default(),
            later: slint::Timer::default(),
            lock_on: true,
        }
    }
}

/// fprintd's results, said plainly
pub fn hint_for(result: &str) -> Option<&'static str> {
    Some(match result {
        "enroll-stage-passed" => "Lift your finger, then touch the reader again",
        "enroll-retry-scan" => "Didn't read that one. Touch the reader again",
        "enroll-swipe-too-short" => "Too short. Try again",
        "enroll-finger-not-centered" => "Put your finger in the middle of the reader",
        "enroll-remove-and-retry" => "Lift your finger and try again",
        _ => return None,
    })
}

impl Fingerprint {
    fn tx(&self) -> Tx {
        self.tx.clone().expect("started")
    }
    fn script() -> String {
        script("fingerprint.py")
    }
    fn py(args: &[&str]) -> Vec<String> {
        let mut v = strs(&["python3", "-I"]);
        v.push(Self::script());
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
    fn publish(&self) {
        let i = &self.info;
        let reader = i.get("reader").cloned().filter(|r| !r.is_null());
        let has_reader = reader.is_some();
        let mut missing: Vec<Value> = i["packages"].as_array().cloned().unwrap_or_default();
        missing.extend(i["aur"].as_array().cloned().unwrap_or_default());
        let ready = has_reader && i["device"].as_bool().unwrap_or(false);
        let enrolled = i["enrolled"].as_array().cloned().unwrap_or_default();
        let lock_screen = ready && !enrolled.is_empty() && self.lock_on;
        put(
            "fingerprint",
            json!({
                "info": i, "reader": reader, "hasReader": has_reader, "missing": missing, "ready": ready,
                "enrolled": enrolled, "pam": if i["pam"].is_object() { i["pam"].clone() } else { json!({}) },
                "lockScreen": lock_screen,
                "installing": updates_running() && self.install_asked, "installError": self.install_error,
                "slowStart": i["slowStart"].as_bool().unwrap_or(false), "fixing": self.fixing, "fixError": self.fix_error,
                "enrollStage": self.enroll_stage, "enrollFinger": self.enroll_finger, "enrollStages": self.enroll_stages,
                "enrollCount": self.enroll_count, "enrollHint": self.enroll_hint, "enrollError": self.enroll_error,
                "enrolling": self.enroll.running(), "pamBusy": self.pam_busy, "pamError": self.pam_error,
                "scanned": {"seq": self.scan_seq, "result": self.scan_result},
            }),
        );
    }
}

impl Service for Fingerprint {
    fn name(&self) -> &'static str {
        "Fingerprint"
    }
    fn calls(&self) -> &'static [&'static str] {
        &["fingerprint"]
    }
    fn start(&mut self, h: &mut Host) {
        self.tx = Some(Tx::new(h.sender("Fingerprint")));
        self.lock_on = h.cfg()["lock"]["fingerprint"].as_bool().unwrap_or(true);
        self.publish();
        self.refresh();
    }
    fn config_changed(&mut self, h: &mut Host) {
        self.lock_on = h.cfg()["lock"]["fingerprint"].as_bool().unwrap_or(true);
        self.publish();
    }
    fn call(&mut self, h: &mut Host, call: &str, args: &Value) {
        let a0 = args.get(0).and_then(Value::as_str).unwrap_or("").to_string();
        match call.split('.').nth(1).unwrap_or("") {
            "refresh" => self.refresh(),
            "install" => {
                let packages = self.info["packages"].as_array().cloned().unwrap_or_default();
                let aur = self.info["aur"].as_array().cloned().unwrap_or_default();
                if updates_running() || (packages.is_empty() && aur.is_empty()) {
                    return;
                }
                self.install_asked = true;
                let join = |v: &[Value]| v.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(",");
                let mut a: Vec<Value> = vec![];
                if !packages.is_empty() {
                    a.push(json!("--install"));
                    a.push(json!(join(&packages)));
                }
                if !aur.is_empty() {
                    a.push(json!("--aur-install"));
                    a.push(json!(join(&aur)));
                }
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
            "fix" => {
                if self.fixing {
                    return;
                }
                self.fix_error.clear();
                self.fixing = true;
                let tx = self.tx();
                std::thread::spawn(move || {
                    let (_, out, _) = run_full(&Fingerprint::py(&["fix"]), None);
                    tx.send(M::Fix(serde_json::from_str(&out).unwrap_or(Value::Null)));
                });
                self.publish();
            }
            "enroll" => {
                if self.enroll.running() {
                    return;
                }
                self.enroll_finger = a0.clone();
                self.enroll_stage = "starting".into();
                self.enroll_stages = 0;
                self.enroll_count = 0;
                self.enroll_hint.clear();
                self.enroll_error.clear();
                let (t1, t2) = (self.tx(), self.tx());
                let ok = self.enroll.spawn(
                    &Self::py(&["enroll", &a0]),
                    None,
                    move |l| {
                        if let Ok(v) = serde_json::from_str::<Value>(&l) {
                            t1.send(M::EnrollLine(v));
                        }
                    },
                    move |_| t2.send(M::EnrollExit),
                );
                if !ok {
                    self.enroll_stage = "error".into();
                    self.enroll_error = "The reader stopped. Try again.".into();
                }
                self.publish();
            }
            "cancelEnroll" => {
                if self.enroll.running() {
                    self.enroll.stop();
                }
                self.enroll_stage.clear();
                self.publish();
            }
            "remove" => {
                // (Quickshell.execDetached, then a refresh after 800 ms)
                let cmd = Self::py(&["delete", &a0]);
                std::thread::spawn(move || {
                    let _ = run_full(&cmd, None);
                });
                let tx = self.tx();
                self.later.start(slint::TimerMode::SingleShot, Duration::from_millis(800), move || tx.send(M::Refresh));
            }
            "setPam" => {
                if self.pam_running {
                    return;
                }
                self.pam_error.clear();
                if !has_tool("pkexec") {
                    self.pam_error = NO_PKEXEC.into();
                    self.publish();
                    return;
                }
                let on = args.get(1).and_then(Value::as_bool).unwrap_or(false);
                self.pam_busy = a0.clone();
                self.pam_running = true;
                let cmd = root_command(&["fingerprint", if on { "on" } else { "off" }, &a0]);
                let tx = self.tx();
                std::thread::spawn(move || {
                    let (code, _, err) = run_full(&cmd, None);
                    tx.send(M::Pam(code, err));
                });
                self.publish();
            }
            _ => {}
        }
    }
    fn message(&mut self, _h: &mut Host, msg: Box<dyn Any + Send>) {
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Status(v) => self.info = v,
            M::Refresh => self.refresh(),
            M::Fix(r) => {
                self.fixing = false;
                self.fix_error = if r.is_null() {
                    "fprintd didn't start.".into()
                } else if r["ok"].as_bool().unwrap_or(false) {
                    String::new()
                } else {
                    r["error"].as_str().unwrap_or("").to_string()
                };
                self.refresh();
            }
            M::EnrollLine(v) => {
                if v["stage"] == "claimed" {
                    self.enroll_stage = "waiting".into();
                    self.enroll_stages = v["stages"].as_i64().unwrap_or(0);
                    self.enroll_hint = "Touch the fingerprint reader".into();
                } else if let Some(r) = v["result"].as_str() {
                    self.enroll_count = v["count"].as_i64().unwrap_or(self.enroll_count);
                    if let Some(hint) = hint_for(r) {
                        self.enroll_hint = hint.into();
                    }
                    self.scan_seq += 1;
                    self.scan_result = r.to_string();
                } else if let Some(ok) = v["ok"].as_bool() {
                    self.enroll_stage = if ok { "done" } else { "error" }.into();
                    self.enroll_error = if ok { String::new() } else { v["error"].as_str().filter(|s| !s.is_empty()).unwrap_or("That didn't work. Try again.").to_string() };
                    self.refresh();
                }
            }
            M::EnrollExit => {
                if self.enroll_stage == "starting" || self.enroll_stage == "waiting" {
                    self.enroll_stage = "error".into();
                    if self.enroll_error.is_empty() {
                        self.enroll_error = "The reader stopped. Try again.".into();
                    }
                }
            }
            M::Pam(code, err) => {
                if code != 0 && code != 126 {
                    self.pam_error = if code == 127 {
                        "Not allowed (an administrator's password is needed).".into()
                    } else {
                        let m = root_message(&err);
                        if m.is_empty() { "That didn't change. Update Tessera, then try again.".into() } else { m }
                    };
                }
                self.pam_busy.clear();
                self.pam_running = false;
                self.refresh();
            }
        }
        self.publish();
    }
}

/// Finger names, said plainly
pub fn finger_name(id: &str) -> &'static str {
    match id {
        "right-index-finger" => "Right index finger",
        "left-index-finger" => "Left index finger",
        "right-thumb" => "Right thumb",
        "left-thumb" => "Left thumb",
        "right-middle-finger" => "Right middle finger",
        "left-middle-finger" => "Left middle finger",
        "right-ring-finger" => "Right ring finger",
        "left-ring-finger" => "Left ring finger",
        "right-little-finger" => "Right little finger",
        "left-little-finger" => "Left little finger",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hints_and_names() {
        assert_eq!(hint_for("enroll-retry-scan"), Some("Didn't read that one. Touch the reader again"));
        assert_eq!(hint_for("x"), None);
        assert_eq!(finger_name("left-thumb"), "Left thumb");
    }
}

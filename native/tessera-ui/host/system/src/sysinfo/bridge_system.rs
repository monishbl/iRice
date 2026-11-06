//! shell/modules/settings/RustBridgeSystem.qml: the "system" page group of
//! Settings (surfaces/settings/src/pages/system). While Settings is open
//! (`system.live [true]`, with `system.page [id]` following the page) it
//! tells the pages what only the shell knows (`{"to":"settings","feed":
//! {"system": …}}`, the pages' cx.feed["system"]) and does what they ask
//! (`system.<what> [args]`: cx.ask("system", what, args)).
//!
//! It is part of the System service (system.rs: one owner of the prefix).
//! The things it reports belong to other services; it reads them from the
//! bus under the QML property names and forwards the acts as asks named by
//! the QML functions (see PORT-NOTES.md "contract"):
//!   bus "fingerprint" {hasReader, ready, reader:{name,unsupported}, missing,
//!        info:{aur[],gdm}, installing, installError, fixing, fixError,
//!        slowStart, enrolled[], fingerNames{}, fingerOrder[]?, enrolling,
//!        enrollStage, enrollCount, enrollStages, enrollError, pam{},
//!        pamBusy, pamError, hints{}}
//!   bus "face" {hasCamera, installed, setUp, faces, busy, error,
//!        installError, installing, camera:{name}}
//!   bus "quickshare" {trusted, trustBusy, trustError}
//!   bus "sharingports" {error, failedWith, busy}
//!   bus "power" {hasBattery, why, mode, modeName, guardError, ownHypridle,
//!        userHypridle, energyMinutes, energyApps:[{key,seconds}]}
//!   bus "backends" {ppd, powerShort, powerReason, hasPerformanceProfile}
//!   bus "shellpriority" {available, on, busy, error}
//!   bus "cooldown" {active, why}
//!   bus "captions" {hasBinary, hasModel, ready, error, downloading}
//!   bus "dictation" {installing, installError, downloadProgress,
//!        downloadError, lastDownload, downloading, languages:[{value,label}]}
//!   asks: fingerprint.{refresh,enroll,cancelEnroll,remove,fix,install,
//!        setPam}, face.{refresh,install,setUpCamera,addFace,clearFaces},
//!        quickshare.setTrusted, sharingports.retry, power.{setMode,
//!        clearEnergy,energyUsers}, shellpriority.set, apps.quit,
//!        cooldown.toggle, captions.{install,download}, dictation.{refresh,
//!        cancelDownload}.
//! Scan results arrive as `system.scanned [result]` (Fingerprint.scanned).

use super::helpers::{self, Ticker, Tx};
use crate::util;
use serde_json::{json, Value};
use std::any::Any;
use std::time::Duration;
use tessera_core::host::Host;

struct Check(Value);

#[derive(Default)]
pub struct SystemBridge {
    tx: Option<Tx>,
    live: bool,
    page_id: String,
    check: Value,
    takes: i64,
    misses: i64,
    retry: String,
    flash: bool,
    energy: bool,
    last_feed: Value,
    t_retry: Option<Ticker>,
    t_flash: Option<Ticker>,
    t_feed: Option<Ticker>,
}

fn obj(name: &str) -> Value {
    let v = crate::bus::get(name);
    if v.is_object() {
        v
    } else {
        json!({})
    }
}

fn get<'a>(v: &'a Value, k: &str, d: &'a Value) -> &'a Value {
    if v[k].is_null() {
        d
    } else {
        &v[k]
    }
}

impl SystemBridge {
    pub fn start(&mut self, _h: &mut Host, tx: Tx) {
        self.tx = Some(tx);
        self.t_retry = Some(Ticker::new("sysRetry"));
        self.t_flash = Some(Ticker::new("sysFlash"));
        self.t_feed = Some(Ticker::new("sysFeed"));
        self.check = json!({});
    }

    fn page(&self) -> &str {
        if self.live {
            &self.page_id
        } else {
            ""
        }
    }

    /// What each page needs fresh when it opens (the QML pages' own
    /// Component.onCompleted), and the energy report's user count
    fn page_changed(&mut self, h: &mut Host, old: &str) {
        let new = self.page().to_string();
        if new == old {
            return;
        }
        if new == "security" {
            h.ask("fingerprint.refresh", json!([]));
            h.ask("face.refresh", json!([]));
            self.check_system();
        } else if new == "accessibility" {
            h.ask("dictation.refresh", json!([]));
        }
        // The apps' energy report runs while Battery & Power shows (Power.energyUsers)
        let energy = new == "power";
        if energy != self.energy {
            self.energy = energy;
            h.ask("power.energyUsers", json!([if energy { 1 } else { -1 }]));
        }
    }

    /// Which firewall is on, and whether Tessera's root helper is there
    /// (sharing-check.py status: read only)
    fn check_system(&self) {
        let tx = self.tx.clone().unwrap();
        let script = util::script("sharing-check.py");
        std::thread::spawn(move || {
            if let Some((_, out)) = util::capture(&["python3", "-I", &script, "status"], Duration::from_secs(30)) {
                if let Ok(v) = serde_json::from_str::<Value>(&out) {
                    tx(Box::new(Check(v)));
                }
            }
        });
    }

    pub fn tick(&mut self, h: &mut Host, name: &str) {
        match name {
            "sysRetry" => self.retry.clear(),
            "sysFlash" => self.flash = false,
            "sysFeed" => {}
            _ => return,
        }
        self.push(h);
    }

    pub fn message(&mut self, h: &mut Host, msg: Box<dyn Any + Send>) {
        if let Ok(c) = msg.downcast::<Check>() {
            self.check = c.0;
            self.push(h);
        }
    }

    pub fn system_changed(&mut self, h: &mut Host) {
        self.push(h);
    }

    /// The feed, as RustBridgeSystem.qml builds it
    fn feed(&self) -> Value {
        let empty = json!({});
        let fp = obj("fingerprint");
        let finger_names = if fp["fingerNames"].is_object() { fp["fingerNames"].clone() } else { json!({}) };
        let order = if fp["fingerOrder"].is_array() { fp["fingerOrder"].clone() } else { json!(finger_names.as_object().map(|m| m.keys().cloned().collect::<Vec<_>>()).unwrap_or_default()) };
        let face = obj("face");
        let qs = obj("quickshare");
        let ports = obj("sharingports");
        let pw = obj("power");
        let be = obj("backends");
        let pr = obj("shellpriority");
        let cd = obj("cooldown");
        let cap = obj("captions");
        let dict = obj("dictation");
        let sys = obj("system");
        let apps: Vec<Value> = pw["energyApps"].as_array().map(|a| a.iter().map(|x| json!({"key": x["key"], "seconds": x["seconds"]})).collect()).unwrap_or_default();
        let langs: Vec<Value> = dict["languages"].as_array().map(|a| a.iter().map(|l| json!({"value": l["value"], "label": l["label"]})).collect()).unwrap_or_default();
        let b = |v: &Value| v.as_bool().unwrap_or(false);
        let s = |v: &Value| v.as_str().unwrap_or("").to_string();
        json!({
            "fingerprint": {
                "hasReader": b(&fp["hasReader"]), "ready": b(&fp["ready"]),
                "readerName": s(&fp["reader"]["name"]), "readerUnsupported": b(&fp["reader"]["unsupported"]),
                "missing": get(&fp, "missing", &json!([])),
                "aur": fp["info"]["aur"].as_array().map(|a| !a.is_empty()).unwrap_or(false),
                "gdm": !fp["info"]["gdm"].is_null() && fp["info"]["gdm"] != false,
                "installing": b(&fp["installing"]), "installError": s(&fp["installError"]),
                "fixing": b(&fp["fixing"]), "fixError": s(&fp["fixError"]), "slowStart": b(&fp["slowStart"]),
                "enrolled": get(&fp, "enrolled", &json!([])), "fingerOrder": order, "fingerNames": finger_names,
                "enrolling": get(&fp, "enrolling", &json!("")), "enrollStage": get(&fp, "enrollStage", &json!(0)),
                "enrollCount": get(&fp, "enrollCount", &json!(0)), "enrollStages": get(&fp, "enrollStages", &json!(0)),
                "enrollError": s(&fp["enrollError"]),
                "pam": get(&fp, "pam", &empty), "pamBusy": get(&fp, "pamBusy", &json!("")), "pamError": s(&fp["pamError"]),
            },
            "sheet": {"takes": self.takes, "misses": self.misses, "retry": self.retry, "flash": self.flash},
            "face": {
                "hasCamera": b(&face["hasCamera"]), "installed": b(&face["installed"]), "setUp": b(&face["setUp"]),
                "faces": get(&face, "faces", &json!([])), "busy": b(&face["busy"]), "error": s(&face["error"]),
                "installError": s(&face["installError"]), "installing": b(&face["installing"]), "camera": s(&face["camera"]["name"]),
            },
            "check": self.check,
            "quickShare": {"trusted": b(&qs["trusted"]), "trustBusy": b(&qs["trustBusy"]), "trustError": s(&qs["trustError"])},
            "ports": {"error": s(&ports["error"]), "failedWith": get(&ports, "failedWith", &json!("")), "busy": b(&ports["busy"])},
            "updatesRunning": b(&crate::bus::field("updates", "running")),
            "power": {
                "ppd": get(&be, "ppd", &json!(false)), "powerShort": s(&be["powerShort"]), "powerReason": s(&be["powerReason"]),
                "hasBattery": b(&pw["hasBattery"]), "why": get(&pw, "why", &json!("")), "mode": get(&pw, "mode", &json!("balanced")),
                "modeName": get(&pw, "modeName", &json!("")), "hasPerformance": b(&be["hasPerformanceProfile"]),
                "guardError": get(&pw, "guardError", &json!("")), "ownHypridle": b(&pw["ownHypridle"]), "userHypridle": b(&pw["userHypridle"]),
                "energyMinutes": get(&pw, "energyMinutes", &json!(0)), "energyApps": apps,
                "keepAwake": b(&sys["keepAwake"]), "keepAwakeStatus": s(&sys["keepAwakeStatus"]),
                "priority": {"available": b(&pr["available"]), "on": b(&pr["on"]), "busy": b(&pr["busy"]), "error": get(&pr, "error", &json!(""))},
            },
            "cooldown": {"active": b(&cd["active"]), "why": get(&cd, "why", &json!(""))},
            "captions": {
                "hasBinary": b(&cap["hasBinary"]), "hasModel": b(&cap["hasModel"]), "ready": b(&cap["ready"]), "error": s(&cap["error"]),
                "downloading": b(&cap["downloading"]), "installing": b(&dict["installing"]), "installError": s(&dict["installError"]),
                "downloadProgress": get(&dict, "downloadProgress", &json!(0)), "downloadError": s(&dict["downloadError"]),
                "lastDownload": get(&dict, "lastDownload", &json!("")), "dictDownloading": b(&dict["downloading"]), "languages": langs,
            },
        })
    }

    /// The feed to Settings, when it changed (onFeedChanged), while live
    fn push(&mut self, h: &mut Host) {
        if !self.live {
            return;
        }
        let f = self.feed();
        if f != self.last_feed {
            self.last_feed = f.clone();
            h.tell("settings", json!({"feed": {"system": f}}));
        }
    }

    /// `system.<what>` that System.qml doesn't have: this page group's.
    /// `out` collects asks for other services (and the one System runs).
    pub fn call(&mut self, h: &mut Host, what: &str, args: &Value, out: &mut Vec<(String, Value)>) {
        let a = args.as_array().cloned().unwrap_or_else(|| if args.is_null() { vec![] } else { vec![args.clone()] });
        let a0 = a.first().cloned().unwrap_or(Value::Null);
        let mut ask = |c: &str, v: Value| out.push((c.to_string(), v));
        match what {
            "live" => {
                let old = self.page().to_string();
                self.live = a0.as_bool().unwrap_or(false);
                if self.live {
                    self.t_feed.as_ref().unwrap().every(self.tx.as_ref().unwrap(), 400);
                    self.last_feed = Value::Null;
                } else {
                    self.t_feed.as_ref().unwrap().stop();
                }
                self.page_changed(h, &old);
                self.push(h);
                return;
            }
            "page" => {
                let old = self.page().to_string();
                self.page_id = a0.as_str().unwrap_or("").to_string();
                self.page_changed(h, &old);
                self.push(h);
                return;
            }
            "sharingBusy" => {
                // SharingPorts.onBusyChanged: not busy any more, on the security page
                if !a0.as_bool().unwrap_or(false) && self.page() == "security" {
                    self.check_system();
                }
                return;
            }
            // Fingerprint.scanned(result): each touch taken or missed, the reader's
            // reason for a miss for a moment, the next section flashing red
            "scanned" => {
                let r = a0.as_str().unwrap_or("");
                if r == "enroll-stage-passed" || r == "enroll-completed" {
                    self.retry.clear();
                    self.takes += 1;
                } else if r.starts_with("enroll-") && r != "enroll-failed" {
                    let hint = obj("fingerprint")["hints"][r].as_str().unwrap_or("Try that one again.").to_string();
                    self.retry = hint;
                    self.t_retry.as_ref().unwrap().once(self.tx.as_ref().unwrap(), 2200);
                    self.misses += 1;
                    self.flash = true;
                    self.t_flash.as_ref().unwrap().once(self.tx.as_ref().unwrap(), 510);
                }
                self.push(h);
                return;
            }
            // Fingerprint (fprintd and PAM through the shell's helper)
            "fpEnroll" => {
                let known = a0.as_str().is_some_and(|f| !obj("fingerprint")["fingerNames"][f].is_null());
                if known {
                    self.takes = 0;
                    self.misses = 0;
                    self.retry.clear();
                    ask("fingerprint.enroll", json!([a0]));
                }
            }
            "fpCancel" => {
                if obj("fingerprint")["enrolling"].as_str().is_some_and(|s| !s.is_empty()) || obj("fingerprint")["enrolling"].as_bool() == Some(true) {
                    ask("fingerprint.cancelEnroll", json!([]));
                }
            }
            "fpRemove" => {
                if obj("fingerprint")["enrolled"].as_array().is_some_and(|e| e.contains(&a0)) {
                    ask("fingerprint.remove", json!([a0]));
                }
            }
            "fpFix" => ask("fingerprint.fix", json!([])),
            "fpInstall" => ask("fingerprint.install", json!([])),
            "fpSetPam" => {
                if matches!(a0.as_str(), Some("polkit" | "sudo" | "login")) {
                    ask("fingerprint.setPam", json!([a0, a.get(1).and_then(Value::as_bool).unwrap_or(false)]));
                }
            }
            // Face (Howdy)
            "faceInstall" => ask("face.install", json!([])),
            "faceSetUp" => ask("face.setUpCamera", json!([])),
            "faceAdd" => ask("face.addFace", json!([])),
            "faceClear" => ask("face.clearFaces", json!([])),
            // Sharing's trust and the firewall's ports
            "quickShareTrust" => ask("quickshare.setTrusted", json!([a0.as_bool().unwrap_or(false)])),
            "portsRetry" => ask("sharingports.retry", json!([])),
            // Power
            "powerMode" => {
                if matches!(a0.as_str(), Some("power-saver" | "balanced" | "performance")) {
                    ask("power.setMode", json!([a0]));
                }
            }
            "caffeineOff" => ask("system.setKeepAwake", json!([false, 0])), // (System runs this one itself)
            "shellPriority" => ask("shellpriority.set", json!([a0.as_bool().unwrap_or(false)])),
            "clearEnergy" => ask("power.clearEnergy", json!([])),
            "quitApp" => {
                if a0.as_str().is_some_and(|s| !s.is_empty()) {
                    ask("apps.quit", json!([a0]));
                }
            }
            "coolDownToggle" => ask("cooldown.toggle", json!([])),
            // Live captions
            "captionsInstall" => ask("captions.install", json!([])),
            "captionsDownload" => ask("captions.download", json!([])),
            "captionsCancel" => ask("dictation.cancelDownload", json!([])),
            _ => eprintln!("settings (system): no act {what}"),
        }
        self.push(h);
    }
}

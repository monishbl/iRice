//! shell/modules/settings/RustBridgeAbout.qml: the "about" page group of
//! Settings. Also home of the three update models it shows (updates.rs,
//! tessera_update.rs, software_update.rs): one Service owns the prefixes
//! "about" (Settings' acts and live/page), and "updates", "tesseraupdate",
//! "softwareupdate" (their QML functions as calls, args = positional array).
//!
//! While Settings is open (`about.live [true]`, `about.page [id]`) it tells
//! the pages what only the shell knows (`{"to":"settings","feed":{"about":
//! …}}`) and does what they ask (`about.<what> [args]`, cx.ask("about",
//! what, args)). Software Update, the update's output, the import/export of
//! shortcuts and gestures (the file dialog is here; Keymap's own work is
//! asked of the keymap service with a `reply`) and forwarding to Health,
//! Conflicts, Report a Bug, BootSnapshots, Needs, CloudDrives.
//!
//! Bus keys read (QML property names): "bootsnapshots", "needs"
//! {installing}, "health" {checks, groups, checking, fixing, error,
//! checkedAt(ms), attention}, "conflicts" {found, muted}, "bugreport"
//! {title, happened, steps, expected, hasShot, shot, shotVersion,
//! includeShot, includeLog, includeInfo, gathering, log, sysinfo{}, state_,
//! error, waitSeconds, issueUrl}, "today" {accountName}, "clouddrives"
//! {hasRclone, installing, installError, busy, errors, names,
//! drives:{gdrive,onedrive}}, "keymap" {ready, hasZenity, hasKdialog, home}.
//! Asks sent: bootsnapshots.{refresh,openAssistant,setMenu}, health.{check,
//! copyReport,fix}, conflicts.{mute,unmute}, bugreport.{gather,set,editShot,
//! copy,openGithub,send}, needs.install, clouddrives.{install,open,act,
//! cancel}, focus.reload, welcome.request, launch.open, panels.openSettings,
//! keymap.{exportTo,previewFile,previewEnd4,applyPreview,restore}.
//! Keymap answers (calls to us): about.ieExported {key,path,ok,err},
//! about.iePreview {key,preview|error}, about.ieApplied {key,snapshot}.

use super::helpers::{self, Tick, Ticker, Tx};
use super::software_update::{self, SoftwareUpdate};
use super::tessera_update::{self, TesseraUpdate};
use super::updates::{self, Updates};
use crate::util;
use serde_json::{json, Map, Value};
use std::any::Any;
use std::time::Duration;
use tessera_core::host::{Host, Service};

/// The file dialog's answer (blocking zenity/kdialog, on a thread)
struct Picked {
    key: String,
    save: bool,
    secs: Vec<String>,
    code: i32,
    out: String,
}
/// A file read for an import
struct Read {
    key: String,
    path: String,
    secs: Vec<String>,
    res: Result<String, String>,
}
/// The undo notice of an import ran out (8 s)
struct IeExpire(String);

#[derive(Default)]
pub struct About {
    tx: Option<Tx>,
    live: bool,
    page_id: String,
    up: Updates,
    tu: TesseraUpdate,
    su: SoftwareUpdate,
    ie: Map<String, Value>,
    last_feed: Value,
    t_feed: Option<Ticker>,
    log_shown: bool,
}

fn obj(name: &str) -> Value {
    let v = crate::bus::get(name);
    if v.is_object() {
        v
    } else {
        json!({})
    }
}

fn pkgs(l: &[Value]) -> Vec<Value> {
    l.iter().map(|x| json!({"name": x["name"].as_str().unwrap_or(""), "from": x["from"].as_str().unwrap_or(""), "to": x["to"].as_str().unwrap_or("")})).collect()
}

fn s(v: &Value) -> Value {
    json!(v.as_str().unwrap_or(""))
}

/// The default of a section set's export file (RustBridgeAbout.ieDefault)
pub fn ie_default(secs: &[String]) -> String {
    format!("~/Documents/{}", if secs.len() == 1 && secs[0] == "gestures" { "tessera-gestures.json" } else { "tessera-shortcuts.json" })
}

/// "@@step …" lines are the update's own bookkeeping, not output
pub fn strip_steps(log: &str) -> String {
    log.split('\n').filter(|l| !l.starts_with("@@step ")).collect::<Vec<_>>().join("\n")
}

fn expand(p: &str) -> String {
    match p.strip_prefix("~/") {
        Some(r) => format!("{}/{}", util::home().to_string_lossy(), r),
        None => p.to_string(),
    }
}

impl About {
    fn tx(&self) -> &Tx {
        self.tx.as_ref().unwrap()
    }
    fn page(&self) -> &str {
        if self.live {
            &self.page_id
        } else {
            ""
        }
    }

    fn after(&mut self, h: &mut Host) {
        let tx = self.tx().clone();
        self.su.read_compat(&tx, &self.tu);
        self.su.follow_incoming(&tx, &self.up, &self.tu);
        self.su.fake = self.up.fake;
        self.su.settle(&self.tu, &self.up);
        self.up.sync_timers(&tx);
        self.up.publish();
        self.push(h);
    }

    /// The page on show: what opening it does in the QML pages
    fn page_changed(&mut self, h: &mut Host, old: &str) {
        let new = self.page().to_string();
        if new != old {
            let tx = self.tx().clone();
            if new == "update" {
                self.su.check_undo(&tx, &self.up);
                h.ask("bootsnapshots.refresh", json!([]));
            } else if new == "about/health" {
                h.ask("health.check", json!([]));
            } else if new == "about/bugreport" {
                let b = obj("bugreport");
                if b["log"].as_str().unwrap_or("").is_empty() && b["gathering"] != true {
                    h.ask("bugreport.gather", json!([]));
                }
            }
        }
        // The update's full output is read only while someone shows it
        let shown = self.page() == "update" && self.up.show_details && self.up.job.is_some();
        if shown != self.log_shown {
            self.log_shown = shown;
            let tx = self.tx().clone();
            self.up.set_log_users(&tx, if shown { 1 } else { -1 });
        }
    }

    fn feed(&self) -> Value {
        let (tu, up, su) = (&self.tu, &self.up, &self.su);
        let plan = su.plan(tu, up);
        let n = |k: &str| plan[k].as_array().map(|a| a.len()).unwrap_or(0);
        let hyprmax = su.hypr_max();
        let steps: Vec<Value> = {
            let now = su.step_now(tu, up);
            su.steps(tu, up).iter().map(|st| json!({"name": software_update::step_name(st), "now": *st == now})).collect()
        };
        let at = su.undo["at"].as_f64().unwrap_or(0.0);
        let sk: Vec<Value> = su.skipped_all(up).iter().map(|p| json!({"name": p["name"], "to": p["to"]})).collect();
        let report = tu.report.as_ref().map(|r| json!({"current": s(&r["current"]), "latest": s(&r["latest"]), "changelog": r["changelog"].as_array().cloned().map(Value::from).unwrap_or(json!([])), "notes": r["notes"].as_array().cloned().map(Value::from).unwrap_or(json!([]))}));
        let result = tu.result.as_ref().map(|r| json!({"backup": helpers::home_tilde(r["backup"].as_str().unwrap_or("")), "now": s(&r["now"]), "changelog": r["changelog"].as_array().cloned().map(Value::from).unwrap_or(json!([])), "notes": r["notes"].as_array().cloned().map(Value::from).unwrap_or(json!([]))}));
        let conflicts: Vec<Value> = tu.conflicts().iter().map(|c| json!({"id": c["id"], "kind": c["kind"], "title": s(&c["title"]), "detail": s(&c["detail"]), "mine": s(&c["mine"]), "theirs": s(&c["theirs"]), "choices": c["choices"].as_array().cloned().map(Value::from).unwrap_or(json!([])), "labels": if c["labels"].is_object() { c["labels"].clone() } else { json!({}) }, "change": s(&c["change"])})).collect();
        let job = up.job.as_ref().map(|j| json!({
            "state": s(&j["state"]), "kind": s(&j["kind"]), "text": s(&j["text"]), "phase": s(&j["phase"]), "package": s(&j["package"]), "error": s(&j["error"]),
            "restart": j["restart"] == true, "upgraded": j["upgraded"].as_array().map(|a| a.len()).unwrap_or(0),
            "aur": j["steps"].as_array().is_some_and(|a| a.iter().any(|x| x == "aur")),
        }));
        let health = obj("health");
        let checks: Vec<Value> = health["checks"].as_array().map(|a| a.iter().map(|c| json!({"id": c["id"], "name": s(&c["name"]), "group": s(&c["group"]), "status": s(&c["status"]), "detail": s(&c["detail"]), "fix": s(&c["fix"]), "canFix": c["canFix"] == true, "fixRoot": c["fixRoot"] == true, "fixPassword": c["fixPassword"] == true, "fixCommand": s(&c["fixCommand"])})).collect()).unwrap_or_default();
        let checked_at = health["checkedAt"].as_f64().unwrap_or(0.0);
        let conf = obj("conflicts");
        let muted = conf["muted"].as_array().cloned().unwrap_or_default();
        let found: Vec<Value> = conf["found"].as_array().map(|a| a.iter().map(|c| json!({"id": c["id"], "name": s(&c["name"]), "what": s(&c["what"]), "fix": s(&c["fix"]), "muted": muted.contains(&c["id"])})).collect()).unwrap_or_default();
        let bug = obj("bugreport");
        let info: String = bug["sysinfo"].as_object().map(|m| m.iter().map(|(k, v)| format!("{k}: {}", v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string()))).collect::<Vec<_>>().join("\n")).unwrap_or_default();
        let b = |v: &Value| v.as_bool().unwrap_or(false);
        let drives = obj("clouddrives");
        let km = obj("keymap");
        let bs = obj("bootsnapshots");
        let clock24 = false;
        let _ = clock24;
        json!({
            "su": {
                "running": SoftwareUpdate::running(up, tu), "checking": su.checking(tu, up), "checked": su.checked, "anything": su.anything(tu, up),
                "restartPending": su.restart_pending, "hyprNow": su.hypr_now(), "hyprTooNew": su.hypr_too_new(up), "hyprShort": su.hypr_short(up), "hyprMax": hyprmax,
                "hold": su.hold, "heldHypr": su.held_hypr(up), "held": su.held(up), "snapshot": su.snapshot, "includeAur": su.include_aur, "removeDropped": su.remove_dropped,
                "undecided": su.undecided(tu, up), "canUpdate": su.can_update(tu, up),
                "plan": {"system": plan["system"] == true, "aur": plan["aur"] == true, "sysCount": plan["sysCount"], "aurCount": plan["aurCount"], "install": n("install"), "remove": n("remove"),
                         "setup": plan["setup"] == true, "tessera": plan["tessera"] == true, "rebuild": n("rebuild")},
                "steps": steps, "undoCount": su.undo_count(),
                "undoMissing": su.undo["missing"].as_array().cloned().unwrap_or_default(),
                "undoPackages": su.undo["packages"].as_array().map(|a| a.iter().map(|p| p["name"].clone()).collect::<Vec<_>>()).unwrap_or_default(),
                "undoLong": if at > 0.0 { helpers::fmt_long(at as i64) } else { String::new() },
                "undoDay": if at > 0.0 { helpers::fmt_day(at as i64) } else { String::new() },
                "canGoBack": su.can_go_back(tu, up), "skippedAll": sk, "includeSkipped": su.include_skipped,
            },
            "tu": {
                "allowed": tu.allowed(), "available": tu.available(), "error": tu.error, "applying": tu.applying, "report": report, "result": result,
                "conflicts": conflicts, "choices": Value::Object(tu.choices.clone()),
                "missing": tu.packages["missing"].as_array().cloned().unwrap_or_default(), "dropped": tu.packages["dropped"].as_array().cloned().unwrap_or_default(),
                "reasons": if tu.packages["reasons"].is_object() { tu.packages["reasons"].clone() } else { json!({}) },
            },
            "up": {
                "job": job, "list": pkgs(&up.list), "aurList": pkgs(&up.aur_list), "fake": up.fake, "interrupted": up.interrupted,
                "cancellable": up.cancellable(), "running": up.running(), "progress": up.progress(), "showDetails": up.show_details,
                "log": if self.log_shown { strip_steps(&up.log) } else { String::new() },
            },
            "bs": {"booted": bs["booted"], "assistant": bs["assistant"], "canMenu": bs["canMenu"], "grubBtrfs": bs["grubBtrfs"], "menuOn": bs["menuOn"], "error": bs["error"], "busy": bs["busy"]},
            "needsInstalling": obj("needs")["installing"].clone(),
            "health": {
                "checks": checks, "groups": health["groups"].clone(), "checking": b(&health["checking"]), "fixing": health["fixing"].clone(), "error": s(&health["error"]),
                "checkedAt": if checked_at > 0.0 { helpers::fmt_hm((checked_at / 1000.0) as i64, false) } else { String::new() },
            },
            "conflicts": found,
            "bug": {
                "title": s(&bug["title"]), "happened": s(&bug["happened"]), "steps": s(&bug["steps"]), "expected": s(&bug["expected"]),
                "hasShot": b(&bug["hasShot"]), "shot": s(&bug["shot"]), "shotVersion": bug["shotVersion"].clone(), "includeShot": b(&bug["includeShot"]),
                "includeLog": b(&bug["includeLog"]), "includeInfo": b(&bug["includeInfo"]), "gathering": b(&bug["gathering"]), "log": s(&bug["log"]),
                "info": info, "state": s(&bug["state_"]), "error": s(&bug["error"]), "waitSeconds": bug["waitSeconds"].clone(), "issueUrl": s(&bug["issueUrl"]),
            },
            "welcomeName": s(&obj("today")["accountName"]),
            "healthChecks": health["checks"].as_array().map(|a| a.len()).unwrap_or(0),
            "healthAttention": health["attention"].as_array().map(|a| a.len()).unwrap_or(0),
            "drives": {
                "hasRclone": b(&drives["hasRclone"]), "installing": b(&drives["installing"]), "installError": s(&drives["installError"]), "busy": drives["busy"].clone(),
                "errors": drives["errors"].clone(), "names": drives["names"].clone(), "gdrive": drives["drives"]["gdrive"].clone(), "onedrive": drives["drives"]["onedrive"].clone(),
                "updatesRunning": up.running(),
            },
            "keymap": {"ready": b(&km["ready"]), "dialog": b(&km["hasZenity"]) || b(&km["hasKdialog"])},
            "ie": Value::Object(self.ie.clone()),
        })
    }

    fn push(&mut self, h: &mut Host) {
        if !self.live {
            return;
        }
        let f = self.feed();
        if f != self.last_feed {
            self.last_feed = f.clone();
            h.tell("settings", json!({"feed": {"about": f}}));
        }
    }

    // ── Import and export (ImportExport.qml's state, one per section set) ─
    fn ie_get(&self, key: &str) -> Map<String, Value> {
        let mut m = Map::new();
        for (k, v) in [("pathMode", json!("")), ("pathSections", json!([])), ("preview", Value::Null), ("error", json!("")), ("status", json!("")), ("busy", json!(false)), ("undoSnap", json!(""))] {
            m.insert(k.into(), v);
        }
        if let Some(Value::Object(o)) = self.ie.get(key) {
            for (k, v) in o {
                m.insert(k.clone(), v.clone());
            }
        }
        m
    }
    fn ie_set(&mut self, key: &str, patch: Value) {
        let mut m = self.ie_get(key);
        if let Value::Object(p) = patch {
            for (k, v) in p {
                m.insert(k, v);
            }
        }
        self.ie.insert(key.to_string(), Value::Object(m));
    }
    fn ie_undo_later(&self, key: &str) {
        let (tx, key) = (self.tx().clone(), key.to_string());
        slint::Timer::single_shot(Duration::from_millis(8000), move || tx(Box::new(IeExpire(key.clone()))));
    }

    fn ie_pick(&mut self, key: &str, save: bool, secs: Vec<String>) {
        let km = obj("keymap");
        let home = km["home"].as_str().map(str::to_string).unwrap_or_else(|| util::home().to_string_lossy().into_owned());
        let start = expand(&ie_default(&secs));
        let cmd: Option<Vec<String>> = if km["hasZenity"] == true {
            Some(if save {
                vec!["zenity".into(), "--file-selection".into(), "--save".into(), "--confirm-overwrite".into(), "--title=Export shortcuts".into(), format!("--filename={start}"), "--file-filter=JSON files | *.json".into()]
            } else {
                vec!["zenity".into(), "--file-selection".into(), "--title=Import shortcuts".into(), format!("--filename={home}/Documents/"), "--file-filter=JSON files | *.json".into()]
            })
        } else if km["hasKdialog"] == true {
            Some(if save {
                vec!["kdialog".into(), "--title".into(), "Export shortcuts".into(), "--getsavefilename".into(), start.clone(), "*.json".into()]
            } else {
                vec!["kdialog".into(), "--title".into(), "Import shortcuts".into(), "--getopenfilename".into(), format!("{home}/Documents"), "*.json".into()]
            })
        } else {
            None
        };
        let Some(cmd) = cmd else {
            self.ie_set(key, json!({"pathSections": secs, "pathMode": if save {"export"} else {"import"}, "pathText": ie_default(&secs)}));
            return;
        };
        self.ie_set(key, json!({"busy": true}));
        let (tx, key) = (self.tx().clone(), key.to_string());
        std::thread::spawn(move || {
            let c: Vec<&str> = cmd.iter().map(String::as_str).collect();
            let (code, out) = util::capture(&c, Duration::from_secs(3600)).unwrap_or((-1, String::new()));
            tx(Box::new(Picked { key, save, secs, code, out }));
        });
    }

    fn ie_export(&mut self, h: &mut Host, key: &str, path: &str, secs: &[String]) {
        self.ie_set(key, json!({"busy": true}));
        h.ask("keymap.exportTo", json!({"path": path, "sections": secs, "reply": {"call": "about.ieExported", "key": key, "path": path}}));
    }

    fn ie_import(&mut self, key: &str, path: &str, secs: Vec<String>) {
        self.ie_set(key, json!({"busy": true}));
        let (tx, key, path) = (self.tx().clone(), key.to_string(), path.to_string());
        std::thread::spawn(move || {
            let res = std::fs::read_to_string(expand(&path)).map_err(|e| {
                let m = e.to_string();
                m.split(" (os error").next().unwrap_or("").to_string()
            });
            tx(Box::new(Read { key, path, secs, res }));
        });
    }

    fn ie_act(&mut self, h: &mut Host, what: &str, a: &[Value]) {
        let key = a.first().and_then(Value::as_str).unwrap_or("").to_string();
        let secs: Vec<String> = a.get(1).and_then(Value::as_array).map(|x| x.iter().map(|v| v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string())).collect()).unwrap_or_default();
        let clear = json!({"error": "", "status": "", "preview": null, "pathMode": ""});
        match what {
            "export" => {
                self.ie_set(&key, clear);
                self.ie_pick(&key, true, secs);
            }
            "import" => {
                self.ie_set(&key, clear);
                self.ie_pick(&key, false, secs);
            }
            "end4" => {
                let mut c = clear;
                c["busy"] = json!(true);
                self.ie_set(&key, c);
                h.ask("keymap.previewEnd4", json!({"shortcuts": secs.iter().any(|s| s == "keybinds" || s == "shortcuts"), "gestures": secs.iter().any(|s| s == "gestures"), "reply": {"call": "about.iePreview", "key": key}}));
            }
            "path" => {
                // the typed path, when there's no file dialog
                let st = self.ie_get(&key);
                let p = a.get(2).and_then(Value::as_str).unwrap_or("").trim().to_string();
                if p.is_empty() {
                    return;
                }
                let ps: Vec<String> = st["pathSections"].as_array().map(|x| x.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()).unwrap_or_default();
                if st["pathMode"] == "export" {
                    self.ie_export(h, &key, &p, &ps);
                } else {
                    self.ie_import(&key, &p, ps);
                }
            }
            "pathCancel" => self.ie_set(&key, json!({"pathMode": ""})),
            "apply" => {
                // (Keymap takes the snapshot before it applies, and answers with it)
                let preview = self.ie_get(&key)["preview"].clone();
                h.ask("keymap.applyPreview", json!({"preview": preview, "reply": {"call": "about.ieApplied", "key": key}}));
                self.ie_set(&key, json!({"preview": null, "status": "Imported."}));
            }
            "close" => self.ie_set(&key, json!({"preview": null})),
            "dismiss" => self.ie_set(&key, json!({"error": ""})),
            "undo" => {
                let snap = self.ie_get(&key)["undoSnap"].clone();
                h.ask("keymap.restore", json!({"snapshot": snap}));
                self.ie_set(&key, json!({"undoSnap": "", "status": "Import undone."}));
                self.ie_undo_later(&key);
            }
            _ => {}
        }
    }

    /// `about.<what>`: Settings' acts
    fn act(&mut self, h: &mut Host, what: &str, args: &Value) {
        let a: Vec<Value> = args.as_array().cloned().unwrap_or_else(|| if args.is_null() { vec![] } else { vec![args.clone()] });
        let a0 = a.first().cloned().unwrap_or(Value::Null);
        let a0s = a0.as_str().unwrap_or("").to_string();
        let tx = self.tx().clone();
        if let Some(w) = what.strip_prefix("ie.") {
            self.ie_act(h, w, &a);
            return;
        }
        match what {
            // Software Update
            "su.check" => self.su.check(&tx, &mut self.tu, &mut self.up),
            "su.update" => self.su.update(h, &tx, &mut self.tu, &mut self.up),
            "su.cancel" => self.up.cancel(),
            "su.goBack" => self.su.go_back(&tx, &self.tu, &mut self.up),
            "su.hold" => self.su.hold = if a0.as_f64() == Some(0.0) { 1 } else { 0 },
            "su.includeAur" => self.su.include_aur = a0.as_bool().unwrap_or(false),
            "su.removeDropped" => self.su.remove_dropped = a0.as_bool().unwrap_or(false),
            "su.includeSkip" => self.su.include_skip(&a0s, a.get(1).and_then(Value::as_bool).unwrap_or(false)),
            "tu.choose" => self.tu.choose(&a0s, a.get(1).and_then(Value::as_str).unwrap_or("")),
            "tu.dismissResult" => self.tu.dismiss_result(),
            "up.dismiss" => self.up.dismiss(),
            "up.copyLog" => self.up.copy_log(),
            "up.showDetails" => {
                self.up.show_details = a0.as_bool().unwrap_or(false);
                let old = self.page().to_string();
                self.page_changed(h, &old);
            }
            "reboot" => tessera_core::services::bridge::run(&["systemctl", "reboot"]),
            "bs.openAssistant" => h.ask("bootsnapshots.openAssistant", json!([])),
            "bs.setMenu" => h.ask("bootsnapshots.setMenu", json!([a0.as_bool().unwrap_or(false)])),
            // (only the packages the pages name)
            "needs.install" => {
                if matches!(a0s.as_str(), "btrfs-assistant" | "grub-btrfs") {
                    h.ask("needs.install", json!([[a0s]]));
                }
            }
            // System health
            "health.check" => h.ask("health.check", json!([])),
            "health.copy" => h.ask("health.copyReport", json!([])),
            "health.fix" => h.ask("health.fix", json!([a0s])),
            "conflicts.mute" => h.ask("conflicts.mute", json!([[a0s]])),
            "conflicts.unmute" => h.ask("conflicts.unmute", json!([a0s])),
            // Report a Bug
            "bug.set" => {
                if matches!(a0s.as_str(), "title" | "happened" | "steps" | "expected") {
                    h.ask("bugreport.set", json!([a0s, a.get(1).and_then(Value::as_str).unwrap_or("")]));
                }
            }
            "bug.include" => {
                if matches!(a0s.as_str(), "includeShot" | "includeLog" | "includeInfo") {
                    h.ask("bugreport.set", json!([a0s, a.get(1).and_then(Value::as_bool).unwrap_or(false)]));
                }
            }
            "bug.editShot" => h.ask("bugreport.editShot", json!([])),
            "bug.copy" => h.ask("bugreport.copy", json!([])),
            "bug.openGithub" => h.ask("bugreport.openGithub", json!([])),
            "bug.send" => h.ask("bugreport.send", json!([])),
            "bug.openIssue" => {
                let u = obj("bugreport")["issueUrl"].as_str().unwrap_or("").to_string();
                if u.starts_with("https://") {
                    h.ask("launch.open", json!([u]));
                }
            }
            // About
            "welcome" => h.ask("welcome.request", json!([])),
            "focusReload" => h.ask("focus.reload", json!([])), // (a restore wrote the Focus modes)
            "openFile" => {
                // (a folder or file of yours, in Files)
                if a0s.starts_with('/') {
                    h.ask("launch.open", json!([format!("file://{a0s}")]));
                }
            }
            // Cloud drive folders
            "drives.install" => h.ask("clouddrives.install", json!([])),
            "drives.open" => h.ask("clouddrives.open", json!([a0s])),
            "drives.act" => {
                if matches!(a0s.as_str(), "mount" | "disconnect" | "connect") {
                    h.ask("clouddrives.act", json!([a0s, a.get(1).and_then(Value::as_str).unwrap_or("")]));
                }
            }
            "drives.cancel" => h.ask("clouddrives.cancel", json!([])),
            // (Keymap's answers)
            "ieExported" => {
                let (key, path) = (a0["key"].as_str().unwrap_or("").to_string(), a0["path"].as_str().unwrap_or("").to_string());
                let home = util::home().to_string_lossy().into_owned();
                if a0["ok"] == true {
                    self.ie_set(&key, json!({"busy": false, "pathMode": "", "status": format!("Exported to {}", path.replacen(&home, "~", 1))}));
                } else {
                    let err = a0["err"].as_str().unwrap_or("");
                    self.ie_set(&key, json!({"busy": false, "pathMode": "", "error": format!("Couldn't write {path}{}. Choose a folder you can write to, such as ~/Documents.", if err.is_empty() { String::new() } else { format!(" ({err})") })}));
                }
            }
            "iePreview" => {
                let key = a0["key"].as_str().unwrap_or("").to_string();
                let p = &a0["preview"];
                if let Some(e) = a0["error"].as_str().or_else(|| p["error"].as_str()).filter(|e| !e.is_empty()) {
                    self.ie_set(&key, json!({"busy": false, "error": e}));
                } else {
                    self.ie_set(&key, json!({"busy": false, "preview": p}));
                }
            }
            "ieApplied" => {
                let key = a0["key"].as_str().unwrap_or("").to_string();
                self.ie_set(&key, json!({"undoSnap": a0["snapshot"], "status": "Imported."}));
                self.ie_undo_later(&key);
            }
            _ => eprintln!("settings (about): no act {what}"),
        }
    }
}

impl Service for About {
    fn name(&self) -> &'static str {
        "About"
    }
    fn calls(&self) -> &'static [&'static str] {
        &["about", "updates", "tesseraupdate", "softwareupdate"]
    }

    fn start(&mut self, h: &mut Host) {
        let tx = helpers::tx(h, "About");
        self.tx = Some(tx.clone());
        self.t_feed = Some(Ticker::new("aboutFeed"));
        self.last_feed = Value::Null;
        self.su.fake = self.up.fake;
        self.up.start(&tx);
        self.tu.start(&tx);
        self.tu.publish();
    }

    fn call(&mut self, h: &mut Host, call: &str, args: &Value) {
        let tx = self.tx().clone();
        let (prefix, f) = call.split_once('.').unwrap_or((call, ""));
        let list = |v: &Value| -> Vec<String> { v.as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default() };
        match prefix {
            "about" => match f {
                "live" => {
                    let old = self.page().to_string();
                    self.live = args.as_array().and_then(|a| a.first()).and_then(Value::as_bool).or(args.as_bool()).unwrap_or(false);
                    let t = self.t_feed.as_ref().unwrap();
                    if self.live {
                        t.every(&tx, 400);
                        self.last_feed = Value::Null;
                    } else {
                        t.stop();
                    }
                    self.page_changed(h, &old);
                }
                "page" => {
                    let old = self.page().to_string();
                    self.page_id = args.as_array().and_then(|a| a.first()).and_then(Value::as_str).or(args.as_str()).unwrap_or("").to_string();
                    self.page_changed(h, &old);
                }
                _ => self.act(h, f, args),
            },
            "updates" => match f {
                "update" => self.up.update(&tx, &list(args)),
                "cancel" => self.up.cancel(),
                "dismiss" => self.up.dismiss(),
                "copyLog" => self.up.copy_log(),
                "refresh" => self.up.refresh(&tx),
                // The menu bar's and Spotlight's "Install updates": Software Update
                "install" => {
                    h.ask("panels.openSettings", json!(["update"]));
                    self.su.check(&tx, &mut self.tu, &mut self.up);
                }
                "showDetails" => {
                    self.up.show_details = args.as_array().and_then(|a| a.first()).and_then(Value::as_bool).unwrap_or(false);
                }
                _ => {}
            },
            "tesseraupdate" => match f {
                "check" => self.tu.check(&tx),
                "choose" => {
                    let a = args.as_array().cloned().unwrap_or_default();
                    self.tu.choose(a.first().and_then(Value::as_str).unwrap_or(""), a.get(1).and_then(Value::as_str).unwrap_or(""));
                }
                "launch" => self.tu.launch(h, &tx, &list(&args.as_array().and_then(|a| a.first()).cloned().unwrap_or(json!([])))),
                "dismissResult" => self.tu.dismiss_result(),
                _ => {}
            },
            "softwareupdate" => match f {
                "check" => self.su.check(&tx, &mut self.tu, &mut self.up),
                "update" => self.su.update(h, &tx, &mut self.tu, &mut self.up),
                "cancel" => self.up.cancel(),
                "goBack" => self.su.go_back(&tx, &self.tu, &mut self.up),
                "checkUndo" => self.su.check_undo(&tx, &self.up),
                _ => {}
            },
            _ => {}
        }
        self.after(h);
    }

    fn message(&mut self, h: &mut Host, msg: Box<dyn Any + Send>) {
        let tx = self.tx().clone();
        let msg = match msg.downcast::<Tick>() {
            Ok(t) => {
                match t.0 {
                    "upPoll" | "upLog" | "upLock" => self.up.tick(&tx, t.0),
                    "tuStart" | "tuAnnounce" => self.tu.tick(h, t.0),
                    _ => {}
                }
                return self.after(h);
            }
            Err(m) => m,
        };
        let msg = match msg.downcast::<updates::Ev>() {
            Ok(ev) => {
                let was_check = matches!(*ev, updates::Ev::Check(..));
                let _ = was_check;
                if let Some(job) = self.up.event(h, &tx, *ev) {
                    if self.up.log_users > 0 {
                        self.up.read_log(&tx);
                    }
                    self.up.refresh(&tx); // a fresh check
                    self.su.updates_finished(h, &tx, &mut self.tu, &self.up, &job);
                }
                return self.after(h);
            }
            Err(m) => m,
        };
        let msg = match msg.downcast::<tessera_update::Ev>() {
            Ok(ev) => {
                self.tu.event(&tx, *ev);
                return self.after(h);
            }
            Err(m) => m,
        };
        let msg = match msg.downcast::<software_update::Ev>() {
            Ok(ev) => {
                self.su.event(*ev);
                return self.after(h);
            }
            Err(m) => m,
        };
        let msg = match msg.downcast::<Picked>() {
            Ok(p) => {
                let p = *p;
                self.ie_set(&p.key, json!({"busy": false}));
                let first = p.out.trim().lines().next().unwrap_or("").to_string();
                if p.code == 0 && !first.is_empty() {
                    if p.save {
                        self.ie_export(h, &p.key, &first, &p.secs);
                    } else {
                        self.ie_import(&p.key, &first, p.secs);
                    }
                } else if p.code != 1 {
                    self.ie_set(&p.key, json!({"pathSections": p.secs, "pathMode": if p.save {"export"} else {"import"}, "pathText": ie_default(&p.secs)}));
                }
                return self.after(h);
            }
            Err(m) => m,
        };
        let msg = match msg.downcast::<Read>() {
            Ok(r) => {
                let r = *r;
                self.ie_set(&r.key, json!({"busy": false, "pathMode": ""}));
                match r.res {
                    Ok(text) => h.ask("keymap.previewFile", json!({"text": text, "sections": r.secs, "reply": {"call": "about.iePreview", "key": r.key}})),
                    Err(e) => self.ie_set(&r.key, json!({"error": format!("Couldn't read {}{}. Check the path and that the file exists.", r.path, if e.is_empty() { String::new() } else { format!(" ({}: {e})", expand(&r.path)) })})),
                }
                return self.after(h);
            }
            Err(m) => m,
        };
        if let Ok(e) = msg.downcast::<IeExpire>() {
            self.ie_set(&e.0, json!({"undoSnap": "", "status": ""}));
            self.after(h);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn export_defaults() {
        assert_eq!(ie_default(&["gestures".into()]), "~/Documents/tessera-gestures.json");
        assert_eq!(ie_default(&["keybinds".into()]), "~/Documents/tessera-shortcuts.json");
        assert_eq!(ie_default(&["keybinds".into(), "gestures".into()]), "~/Documents/tessera-shortcuts.json");
    }
    #[test]
    fn steps_are_not_output() {
        assert_eq!(strip_steps("@@step system\nhello\nworld"), "hello\nworld");
    }
}

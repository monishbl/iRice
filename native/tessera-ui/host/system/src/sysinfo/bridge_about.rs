//! shell/modules/settings/RustBridgeAbout.qml: the "about" page group of
//! Settings. Also home of the three update models it shows (updates.rs,
//! tessera_update.rs, software_update.rs): one Service owns the prefixes
//! "about" (Settings' acts and live/page), "updates" (their QML functions
//! as calls, args = positional array) and "tesseraupdate" (the island's Cancel, Restart).
//!
//! While Settings is open (`about.live [true]`, `about.page [id]`) it tells
//! the pages what only the shell knows (`{"to":"settings","feed":{"about":
//! …}}`) and does what they ask (`about.<what> [args]`, cx.ask("about",
//! what, args)). Software Update, the update's output, the import/export of
//! shortcuts and gestures (the file dialog is here; Keymap's own work is
//! asked of the keymap service with a `reply`) and forwarding to Health,
//! Conflicts, Report a Bug, BootSnapshots, Needs, CloudDrives.
//!
//! Read: the host states host-shell's probes publish, "needs"
//! {installing}, "health" {checks, checking, fixing, error, checkedAt(ms)},
//! "conflicts" {found, muted}, "bugreport" {title, happened, steps,
//! expected, hasShot, shot, shotVersion, includeShot, includeLog,
//! includeInfo, gathering, log, sysinfo{}, state_, error, waitSeconds,
//! issueUrl}; bus keys "bootsnapshots", "account" {name}, "clouddrives"
//! {hasRclone, installing, installError, busy, errors, names,
//! drives:{gdrive,onedrive}}, "keymap" {ready, hasZenity, hasKdialog, home}.
//! Asks sent: bootsnapshots.{refresh,openAssistant,setMenu}, health.{check,
//! copyReport,fix}, conflicts.{mute,unmute}, bugreport.{gather,set,editShot,
//! copy,openGithub,send}, needs.install, clouddrives.{install,open,act,
//! cancel}, focus.reload, welcome.request, launch.open, open, panels.openSettings,
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
/// Tessera's features not set up yet (features.py missing), and whether to
/// announce the one-time offer (a notification) when there are some
struct FeatList(Value, bool);
/// What setting the chosen features up takes (features.py plan)
struct FeatPlan(Value);
/// The offer's notification was answered ("setup" or "")
struct FeatAnswer(String);

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
    /// Software Update's sheet on show ("" none): a log ("tessera-log",
    /// "system-log", "busy-log": the update running elsewhere), "notes",
    /// "result", "packages" or "decide"
    sheet: String,
    /// the system update's log read while its sheet shows (Updates.log_users)
    log_on: bool,
    /// yay or paru is installed (Options' Include AUR updates)
    aur_helper: bool,
    /// "Set up the rest of Tessera" (features.py): the features this
    /// computer can use that aren't set up ({"items": […]}), a run of them
    /// started here, its results, and a check under way
    feat: Value,
    feat_running: bool,
    feat_result: Value,
    feat_checking: bool,
}

fn obj(name: &str) -> Value {
    let v = crate::bus::get(name);
    if v.is_object() {
        v
    } else {
        json!({})
    }
}

/// Another crate's host state (host-shell's probes: health, conflicts,
/// bugreport, needs), {} when not published yet
fn st(h: &Host, name: &str) -> Value {
    let v = h.state(name).clone();
    if v.is_object() {
        v
    } else {
        json!({})
    }
}

/// Health.groups (the order System health shows them in)
const HEALTH_GROUPS: &[&str] = &["Sound", "Network & sharing", "Power", "Desktop", "Tessera itself"];

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

/// The lines Software Update's details show: the last `n`, without the step
/// lines. (19 Nov: the whole output went to Settings on every new line and
/// was laid out again each time, so the page lagged more the longer an
/// update ran. The view shows about a dozen lines; Copy copies the full log
/// from here, up.copyLog.)
pub fn log_tail(log: &str, n: usize) -> String {
    // (only the end is looked at: the log can be megabytes)
    let from = log.len().saturating_sub(n * 400);
    let from = (from..=log.len()).find(|&i| log.is_char_boundary(i)).unwrap_or(log.len());
    let lines: Vec<&str> = log[from..].split('\n').skip(usize::from(from > 0)).filter(|l| !l.starts_with("@@step ")).collect();
    lines[lines.len().saturating_sub(n)..].join("\n")
}

/// Where the update running elsewhere runs, against this session ("this",
/// "other"; "" when it doesn't say)
pub fn busy_where(session: &str, here: &str) -> &'static str {
    if session.is_empty() {
        ""
    } else if session == here {
        "this"
    } else {
        "other"
    }
}

/// yay or paru on the PATH (the AUR's updates can be included)
fn has_aur_helper() -> bool {
    std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join("yay").is_file() || d.join("paru").is_file()))
}

/// The release publisher's status (tools/release-builder/publish-release),
/// on the machine where it's installed (~/tessera-verify/publish-release.sh);
/// null anywhere else
fn pipeline() -> Value {
    let home = util::home();
    if !home.join("tessera-verify/publish-release.sh").exists() {
        return Value::Null;
    }
    std::fs::read_to_string(home.join(".cache/tessera-publish.json")).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Null)
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
            // (Software Update and its sub-pages: what they show, read once on the way in)
            if new.starts_with("update") && !old.starts_with("update") {
                // (the one-time offer, while it's open: what's still to set up)
                if h.cfg()["features"]["offer"] == "pending" {
                    self.features_check(false);
                }
                self.su.check_undo(&tx, &self.up);
                self.tu.read_cache(&tx);
                self.tu.read_staged(&tx);
                self.aur_helper = has_aur_helper();
                h.ask("bootsnapshots.refresh", json!([]));
            }
            // (a sheet belongs to the page it opened on: leaving it, it closes)
            if old == "update" && new != "update" {
                self.sheet.clear();
            }
            if new == "about/health" {
                h.ask("health.check", json!([]));
            } else if new == "about/bugreport" {
                let b = st(h, "bugreport");
                if b["log"].as_str().unwrap_or("").is_empty() && b["gathering"] != true {
                    h.ask("bugreport.gather", json!([]));
                }
            }
        }
        self.sync_log();
    }

    /// The system update's output is read only while its sheet shows it
    fn sync_log(&mut self) {
        let shown = self.page() == "update" && self.sheet == "system-log" && self.up.job.is_some();
        if shown != self.log_on {
            self.log_on = shown;
            let tx = self.tx().clone();
            self.up.set_log_users(&tx, if shown { 1 } else { -1 });
        }
    }

    /// The sheet's log: its last 200 lines (the sheet shows about thirty;
    /// Copy log copies the whole file)
    fn sheet_log(&self) -> String {
        match self.sheet.as_str() {
            "system-log" => log_tail(&self.up.log, 200),
            "tessera-log" => tessera_core::build_progress::tail(&tessera_update::job_log(), 200),
            "busy-log" => self.busy_log().map(|p| tessera_core::build_progress::tail(&p, 200)).unwrap_or_default(),
            _ => String::new(),
        }
    }

    /// The log of the update running elsewhere (the repo lock's holder), if it says
    fn busy_log(&self) -> Option<std::path::PathBuf> {
        let p = self.tu.job.as_ref()?["holder"]["log"].as_str()?.to_string();
        (!p.is_empty()).then(|| std::path::PathBuf::from(p))
    }

    /// The file the sheet's Copy log copies
    fn sheet_log_path(&self) -> Option<std::path::PathBuf> {
        match self.sheet.as_str() {
            "system-log" => Some(std::path::PathBuf::from(format!("{}/log", self.up.dir))),
            "tessera-log" => Some(tessera_update::job_log()),
            "busy-log" => self.busy_log(),
            _ => None,
        }
    }

    /// Software Update's part of the feed (pages/about/update.rs): what its
    /// three pages and its sheets show, and nothing more
    fn upd_feed(&self, clock24: bool) -> Value {
        let (tu, up, su) = (&self.tu, &self.up, &self.su);
        let plan = su.plan(tu, up);
        let r = tu.report.clone().unwrap_or(Value::Null);
        let arr = |v: &Value| v.as_array().cloned().map(Value::from).unwrap_or(json!([]));
        let n = |v: &Value| v.as_array().map(|a| a.len()).unwrap_or(0);
        let info = tu.info.as_ref().map(|i| {
            let date = i["date"].as_i64().unwrap_or(0);
            json!({"channel": s(&i["channel"]), "channelName": s(&i["channelName"]), "branch": s(&i["branch"]), "current": s(&i["current"]), "date": if date > 0 { helpers::fmt_date(date) } else { String::new() }})
        });
        // (the background job; one running elsewhere said with when and where)
        let mut job = tu.job.clone().unwrap_or(Value::Null);
        if job["state"] == "busy" {
            let started = job["holder"]["started"].as_f64().unwrap_or(0.0);
            let here = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").unwrap_or_default();
            let session = job["holder"]["session"].as_str().unwrap_or("").to_string();
            job["holder"]["at"] = json!(if started > 0.0 { helpers::fmt_hm(started as i64, clock24) } else { String::new() });
            job["holder"]["where"] = json!(busy_where(&session, &here));
        }
        let sjob = up.job.as_ref().map(|j| json!({
            "state": s(&j["state"]), "kind": s(&j["kind"]), "text": s(&j["text"]), "phase": s(&j["phase"]), "package": s(&j["package"]), "error": s(&j["error"]),
            "restart": j["restart"] == true, "upgraded": j["upgraded"].as_array().map(|a| a.len()).unwrap_or(0),
        }));
        let packages = self.sheet == "packages";
        let notes = matches!(self.sheet.as_str(), "notes" | "decide");
        let at = su.undo["at"].as_f64().unwrap_or(0.0);
        let bs = obj("bootsnapshots");
        let result = tu.result.as_ref().map(|r| json!({"backup": helpers::home_tilde(r["backup"].as_str().unwrap_or("")), "now": s(&r["now"]), "changelog": arr(&r["changelog"]), "notes": arr(&r["notes"])}));
        let conflicts: Vec<Value> = if self.sheet == "decide" {
            tu.conflicts().iter().map(|c| json!({"id": c["id"], "kind": c["kind"], "title": s(&c["title"]), "detail": s(&c["detail"]), "mine": s(&c["mine"]), "theirs": s(&c["theirs"]), "choices": arr(&c["choices"]), "labels": if c["labels"].is_object() { c["labels"].clone() } else { json!({}) }, "change": s(&c["change"])})).collect()
        } else {
            vec![]
        };
        json!({
            "allowed": tu.allowed(), "fake": up.fake,
            "checking": su.checking(tu, up), "checked": su.checked,
            "checkedAt": if su.checked_at > 0.0 { helpers::fmt_hm(su.checked_at as i64, clock24) } else { String::new() },
            "anything": su.anything(tu, up), "running": SoftwareUpdate::running(up, tu),
            "restartComputer": !up.running() && (su.restart_pending || up.job.as_ref().is_some_and(|j| j["state"] == "done" && j["restart"] == true)),
            "t": {
                "current": s(&r["current"]), "latest": s(&r["latest"]), "available": tu.available(), "downgrade": r["downgrade"] == true,
                "channelName": s(&r["channelName"]), "ahead": r["ahead"].as_u64().unwrap_or(0), "preparing": r["preparing"].as_u64().unwrap_or(0),
                "hasNotes": n(&r["changelog"]) + n(&r["notes"]) > 0,
                "changelog": if notes { arr(&r["changelog"]) } else { json!([]) }, "notes": if notes { arr(&r["notes"]) } else { json!([]) },
                "leaving": if notes { arr(&r["leaving"]) } else { json!([]) },
                "error": tu.error, "offline": tu.error.starts_with("Couldn't reach the update server"),
                "job": job, "fraction": tu.job_fraction(), "ready": tu.ready(),
                "staged": tu.staged.as_ref().map(|x| s(&x["latest"])).unwrap_or(json!("")), "restarting": tu.restarting,
                "canUpdate": su.can_update_tessera(tu, up), "undecided": tu.undecided(), "result": result,
                "conflicts": conflicts, "choices": Value::Object(tu.choices.clone()),
            },
            "s": {
                "count": plan["sysCount"], "aurCount": plan["aurCount"], "aurAll": up.aur_list.len(), "includeAur": su.include_aur, "removeDropped": su.remove_dropped,
                "canCheck": up.available || !su.checked, "canUpdate": su.can_update_system(tu, up),
                "running": up.running(), "job": sjob, "progress": up.progress(), "cancellable": up.cancellable(), "interrupted": up.interrupted,
                "hyprTooNew": su.hypr_too_new(up), "hyprShort": su.hypr_short(up), "hyprNow": su.hypr_now(), "hyprMax": su.hypr_max(), "hold": su.hold,
                "missing": n(&tu.packages["missing"]), "dropped": n(&tu.packages["dropped"]),
                // (the lists only while the packages sheet shows them)
                "list": if packages { pkgs(&up.list) } else { vec![] }, "aurList": if packages { pkgs(&up.aur_list) } else { vec![] },
                "held": if packages { su.held(up) } else { vec![] }, "heldHypr": if packages { su.held_hypr(up) } else { vec![] },
                "missingList": if packages { arr(&tu.packages["missing"]) } else { json!([]) }, "droppedList": if packages { arr(&tu.packages["dropped"]) } else { json!([]) },
                "reasons": if packages && tu.packages["reasons"].is_object() { tu.packages["reasons"].clone() } else { json!({}) },
            },
            "back": {
                "count": su.undo_count(), "long": if at > 0.0 { helpers::fmt_long(at as i64) } else { String::new() },
                "day": if at > 0.0 { helpers::fmt_day(at as i64) } else { String::new() },
                "packages": su.undo["packages"].as_array().map(|a| a.iter().map(|p| p["name"].clone()).collect::<Vec<_>>()).unwrap_or_default(),
                "missing": su.undo["missing"].as_array().cloned().unwrap_or_default(), "canGoBack": su.can_go_back(tu, up),
                "skipped": su.skipped_all(up).iter().map(|p| json!({"name": p["name"], "to": p["to"]})).collect::<Vec<_>>(), "include": su.include_skipped,
            },
            "snap": {"tool": su.snapshot, "booted": bs["booted"], "assistant": bs["assistant"], "canMenu": bs["canMenu"], "grubBtrfs": bs["grubBtrfs"], "menuOn": bs["menuOn"], "error": bs["error"], "busy": bs["busy"]},
            "info": info, "aurHelper": self.aur_helper,
            "cache": tu.cache.map(|c| json!(c)).unwrap_or(Value::Null), "cacheBusy": tu.cache_busy, "cacheError": tu.cache_error,
            "sheet": self.sheet, "log": self.sheet_log(),
        })
    }

    // ── "Set up the rest of Tessera" (features.py) ──────────────────────
    /// What's still to set up, read again; `announce`: the offer's
    /// notification when there's anything (the first time only)
    fn features_check(&mut self, announce: bool) {
        if self.feat_checking {
            return;
        }
        self.feat_checking = true;
        let (t, script) = (self.tx().clone(), util::script("features.py"));
        std::thread::spawn(move || {
            let out = util::capture(&["python3", "-I", &script, "missing"], Duration::from_secs(60)).map(|(_, o)| o).unwrap_or_default();
            t(Box::new(FeatList(serde_json::from_str(out.trim()).unwrap_or(Value::Null), announce)));
        });
    }

    /// The offer, once: pending while it's open in Software Update, done
    /// when there's nothing to offer. One notification; Review opens it.
    fn features_announce(&mut self) {
        let items = self.feat["items"].as_array().cloned().unwrap_or_default();
        if items.is_empty() {
            util::config_set("features.offer", json!("done"));
            return;
        }
        util::config_set("features.offer", json!("pending"));
        let body = offer_text(&items);
        let t = self.tx().clone();
        std::thread::spawn(move || {
            let c = ["timeout", "86400", "notify-send", "-a", "Tessera", "-i", "system-software-update", "-h", "string:x-tessera-key:tessera-features",
                     "-A", "setup=Review", "Set up the rest of Tessera", &body];
            let out = util::capture(&c, Duration::from_secs(86400 + 60)).map(|(_, o)| o).unwrap_or_default();
            t(Box::new(FeatAnswer(out.trim().to_string())));
        });
    }

    /// A run of them ended (sysupdate.py's status, `features` per id): what
    /// was set up and what wasn't, with how to finish it. Never offered again.
    fn features_finished(&mut self, job: &Value) {
        if job["state"] == "cancelled" {
            return;
        }
        let items = self.feat["items"].as_array().cloned().unwrap_or_default();
        self.feat_result = features_result(&items, job);
        util::config_set("features.offer", json!("done"));
        self.features_check(false);
    }

    /// The feed's `upd.features`
    fn features_feed(&self, h: &Host) -> Value {
        let pending = h.cfg()["features"]["offer"] == "pending";
        let items: Vec<Value> = self.feat["items"].as_array().map(|a| a.iter().map(|i| json!({"id": s(&i["id"]), "title": s(&i["title"]), "detail": s(&i["detail"]), "group": s(&i["group"])})).collect()).unwrap_or_default();
        json!({
            "pending": pending || self.feat_running || !self.feat_result.is_null(),
            "items": if pending || self.feat_running { items } else { vec![] },
            "running": self.feat_running, "result": self.feat_result,
        })
    }

    fn feed(&self, h: &Host) -> Value {
        let up = &self.up;
        let health = st(h, "health");
        let checks: Vec<Value> = health["checks"].as_array().map(|a| a.iter().map(|c| json!({"id": c["id"], "name": s(&c["name"]), "group": s(&c["group"]), "status": s(&c["status"]), "detail": s(&c["detail"]), "fix": s(&c["fix"]), "canFix": c["canFix"] == true, "fixRoot": c["fixRoot"] == true, "fixPassword": c["fixPassword"] == true, "fixCommand": s(&c["fixCommand"])})).collect()).unwrap_or_default();
        let checked_at = health["checkedAt"].as_f64().unwrap_or(0.0);
        let conf = st(h, "conflicts");
        let muted = conf["muted"].as_array().cloned().unwrap_or_default();
        let found: Vec<Value> = conf["found"].as_array().map(|a| a.iter().map(|c| json!({"id": c["id"], "name": s(&c["name"]), "what": s(&c["what"]), "fix": s(&c["fix"]), "muted": muted.contains(&c["id"])})).collect()).unwrap_or_default();
        let bug = st(h, "bugreport");
        let info: String = bug["sysinfo"].as_object().map(|m| m.iter().map(|(k, v)| format!("{k}: {}", v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string()))).collect::<Vec<_>>().join("\n")).unwrap_or_default();
        let b = |v: &Value| v.as_bool().unwrap_or(false);
        let drives = obj("clouddrives");
        let km = obj("keymap");
        // (TimeFormat.hm: the 24-hour setting, as the QML's "Checked at")
        let clock24 = h.cfg()["appearance"]["clock24"].as_bool().unwrap_or(false);
        json!({
            // (Software Update's pages; About's Version row reads its channel too)
            "upd": if self.page().starts_with("update") || self.page() == "about" {
                let mut u = self.upd_feed(clock24);
                u["features"] = self.features_feed(h);
                u
            } else {
                Value::Null
            },
            "needsInstalling": st(h, "needs")["installing"].as_bool().unwrap_or(false),
            "health": {
                "checks": checks, "groups": HEALTH_GROUPS, "checking": b(&health["checking"]), "fixing": health["fixing"].clone(), "error": s(&health["error"]),
                "checkedAt": if checked_at > 0.0 { helpers::fmt_hm((checked_at / 1000.0) as i64, clock24) } else { String::new() },
            },
            "conflicts": found,
            "bug": {
                "title": s(&bug["title"]), "happened": s(&bug["happened"]), "steps": s(&bug["steps"]), "expected": s(&bug["expected"]),
                "hasShot": b(&bug["hasShot"]), "shot": s(&bug["shot"]), "shotVersion": bug["shotVersion"].clone(), "includeShot": b(&bug["includeShot"]),
                "includeLog": b(&bug["includeLog"]), "includeInfo": b(&bug["includeInfo"]), "gathering": b(&bug["gathering"]), "log": s(&bug["log"]),
                "info": info, "state": s(&bug["state_"]), "error": s(&bug["error"]), "waitSeconds": bug["waitSeconds"].clone(), "issueUrl": s(&bug["issueUrl"]),
            },
            "welcomeName": s(&obj("account")["name"]),
            "healthChecks": health["checks"].as_array().map(|a| a.len()).unwrap_or(0),
            "healthAttention": health["checks"].as_array().map(|a| a.iter().filter(|c| c["status"] == "fail" || c["status"] == "warn").count()).unwrap_or(0),
            "drives": {
                "hasRclone": b(&drives["hasRclone"]), "installing": b(&drives["installing"]), "installError": s(&drives["installError"]), "busy": drives["busy"].clone(),
                "errors": drives["errors"].clone(), "names": drives["names"].clone(), "gdrive": drives["drives"]["gdrive"].clone(), "onedrive": drives["drives"]["onedrive"].clone(),
                "updatesRunning": up.running(),
            },
            "keymap": {"ready": b(&km["ready"]), "dialog": b(&km["hasZenity"]) || b(&km["hasKdialog"])},
            "ie": Value::Object(self.ie.clone()),
            // (About's release pipeline line: only where the publisher runs)
            "pipeline": if self.page() == "about" { pipeline() } else { Value::Null },
        })
    }

    fn push(&mut self, h: &mut Host) {
        if !self.live {
            return;
        }
        let f = self.feed(h);
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
            let (code, out) = util::capture_shown(&c, Duration::from_secs(3600)).unwrap_or((-1, String::new()));
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

    /// Show (or hide) the log of the part that's running: the system's, else Tessera's
    fn show_details(&mut self, on: bool) {
        self.sheet = if !on {
            String::new()
        } else if self.up.running() || (self.up.job.is_some() && !self.tu.job_running()) {
            "system-log".into()
        } else {
            "tessera-log".into()
        };
        self.sync_log();
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
            "su.check" => {
                self.tu.sync_channel(h);
                self.su.check(&tx, &mut self.tu, &mut self.up)
            }
            // The channel picker (Settings wrote update.channel; it may not be
            // in config.json yet): follow it from now on, and a fresh check
            "su.channel" => {
                if (a0s == "stable" || a0s == "beta") && !SoftwareUpdate::running(&self.up, &self.tu) && !self.su.checking(&self.tu, &self.up) {
                    self.tu.channel = a0s.clone();
                    self.tu.read_info(&tx);
                    self.su.check(&tx, &mut self.tu, &mut self.up)
                }
            }
            // (Install updates: both parts, the system first with Tessera's
            // own packages, one password, then Tessera's in the background)
            "su.update" | "su.updateAll" => {
                self.sheet.clear();
                self.su.update(h, &tx, &mut self.tu, &mut self.up)
            }
            // (the two parts, each its own Update)
            "tu.update" => {
                self.sheet.clear();
                self.su.update_tessera(h, &tx, &mut self.tu, &mut self.up)
            }
            "sys.update" => self.su.update_system(&tx, &mut self.tu, &mut self.up),
            // Software Update's sheets: a log, the notes, the packages, the decisions
            "sheet" => {
                self.sheet = if matches!(a0s.as_str(), "tessera-log" | "system-log" | "busy-log" | "notes" | "result" | "packages" | "decide" | "features") { a0s.clone() } else { String::new() };
                self.sync_log();
            }
            "sheet.copy" => {
                if let Some(p) = self.sheet_log_path() {
                    tessera_core::services::bridge::run(&["sh", "-c", "wl-copy < \"$1\"", "sh", &p.to_string_lossy()]);
                }
            }
            // "Set up the rest of Tessera": the ones ticked, one password
            "features.setUp" => {
                let known: Vec<String> = self.feat["items"].as_array().map(|a| a.iter().filter_map(|i| i["id"].as_str().map(str::to_string)).collect()).unwrap_or_default();
                let ids: Vec<String> = a0.as_array().map(|a| a.iter().filter_map(|x| x.as_str()).filter(|x| known.iter().any(|k| k == x)).map(str::to_string).collect()).unwrap_or_default();
                if !ids.is_empty() && !self.up.running() && !self.feat_running {
                    self.feat_running = true;
                    self.feat_result = Value::Null;
                    let (t, script) = (tx.clone(), util::script("features.py"));
                    std::thread::spawn(move || {
                        let mut c = vec!["python3", "-I", script.as_str(), "plan"];
                        c.extend(ids.iter().map(String::as_str));
                        let out = util::capture(&c, Duration::from_secs(120)).map(|(_, o)| o).unwrap_or_default();
                        t(Box::new(FeatPlan(serde_json::from_str(out.trim()).unwrap_or(Value::Null))));
                    });
                }
            }
            // (not now, and never again: each feature's own page sets it up)
            "features.skip" | "features.done" => {
                util::config_set("features.offer", json!("done"));
                self.feat_result = Value::Null;
                self.sheet.clear();
            }
            "tu.cancel" => self.tu.cancel(&tx),
            "tu.restart" => self.tu.restart(h, &tx, false),
            "cache.delete" => self.tu.delete_cache(&tx),
            "su.cancel" => self.up.cancel(),
            "su.goBack" => self.su.go_back(&tx, &self.tu, &mut self.up),
            "su.hold" => self.su.hold = if a0.as_f64() == Some(0.0) { 1 } else { 0 },
            "su.includeAur" => self.su.include_aur = a0.as_bool().unwrap_or(false),
            "su.removeDropped" => self.su.remove_dropped = a0.as_bool().unwrap_or(false),
            "su.includeSkip" => self.su.include_skip(&a0s, a.get(1).and_then(Value::as_bool).unwrap_or(false)),
            "tu.choose" => self.tu.choose(&a0s, a.get(1).and_then(Value::as_str).unwrap_or("")),
            "tu.dismissResult" => {
                self.tu.dismiss_result();
                self.sheet.clear();
            }
            "up.dismiss" => self.up.dismiss(),
            "up.copyLog" => self.up.copy_log(),
            "up.showDetails" => self.show_details(a0.as_bool().unwrap_or(false)),
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
                let u = st(h, "bugreport")["issueUrl"].as_str().unwrap_or("").to_string();
                if u.starts_with("https://") {
                    h.ask("launch.open", json!([u]));
                }
            }
            // a link in the update notes (Launch.openLink: web links only)
            "openLink" => {
                if a0s.starts_with("https://") || a0s.starts_with("http://") {
                    h.ask("launch.openLink", json!([a0s]));
                }
            }
            // About
            "welcome" => h.ask("welcome.request", json!([])),
            "focusReload" => h.ask("focus.reload", json!([])), // (a restore wrote the Focus modes)
            "openFile" => {
                // (a folder or file of yours, in Files)
                if a0s.starts_with('/') {
                    h.ask("open", json!([a0s]));
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
        &["about", "updates", "tesseraupdate"]
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

    /// Each turn: the lock's state, for the restart into a ready update the
    /// moment the screen locks (TesseraUpdate.lock_seen)
    fn settled(&mut self, h: &mut Host) {
        let Some(tx) = self.tx.clone() else { return };
        let secure = h.state("lockSecure").as_bool().unwrap_or(false);
        if secure != self.tu.was_secure {
            self.tu.lock_seen(h, &tx, secure);
        }
    }

    fn call(&mut self, h: &mut Host, call: &str, args: &Value) {
        let tx = self.tx().clone();
        let (prefix, f) = call.split_once('.').unwrap_or((call, ""));
        let list = |v: &Value| -> Vec<String> { update_args(v) };
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
                    self.tu.sync_channel(h);
                    self.su.check(&tx, &mut self.tu, &mut self.up);
                }
                // (the island's Details: the log of whichever part runs, in its sheet)
                "showDetails" => self.show_details(args.as_array().and_then(|a| a.first()).and_then(Value::as_bool).unwrap_or(false)),
                _ => {}
            },
            // (the island's controls while the background update runs, and once it's ready)
            "tesseraupdate" => match f {
                "cancel" => self.tu.cancel(&tx),
                "restart" => self.tu.restart(h, &tx, false),
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
                    "tuStart" | "tuAnnounce" | "tuPeek" | "tuJob" => self.tu.tick(h, &tx, t.0),
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
                    if std::mem::take(&mut self.feat_running) {
                        self.features_finished(&job);
                    } else if job["state"] == "done" && h.cfg()["features"]["offer"].as_str().unwrap_or("").is_empty() {
                        // (the first update since this version: the offer, once)
                        self.features_check(true);
                    }
                }
                return self.after(h);
            }
            Err(m) => m,
        };
        let msg = match msg.downcast::<tessera_update::Ev>() {
            Ok(ev) => {
                self.tu.event(h, &tx, *ev);
                // (Tessera's part applied: the first start of this version offers the rest, once)
                if self.tu.result.as_ref().is_some_and(|r| helpers::truthy(&r["applied"])) && h.cfg()["features"]["offer"].as_str().unwrap_or("").is_empty() {
                    self.features_check(true);
                }
                // The update notice's Update: Software Update, with a fresh check
                if std::mem::take(&mut self.tu.open_asked) {
                    h.ask("panels.openSettings", json!(["update"]));
                    self.tu.sync_channel(h);
                    self.su.check(&tx, &mut self.tu, &mut self.up);
                }
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
        let msg = match msg.downcast::<FeatList>() {
            Ok(m) => {
                let FeatList(v, announce) = *m;
                self.feat_checking = false;
                if v.is_object() {
                    self.feat = v;
                }
                if announce && h.cfg()["features"]["offer"].as_str().unwrap_or("").is_empty() {
                    self.features_announce();
                }
                return self.after(h);
            }
            Err(m) => m,
        };
        let msg = match msg.downcast::<FeatPlan>() {
            Ok(m) => {
                match features_args(&m.0) {
                    Some(a) => self.up.update(&tx, &a),
                    None => {
                        self.feat_running = false;
                        self.feat_result = json!({"error": "The features couldn't be planned. Try again."});
                    }
                }
                return self.after(h);
            }
            Err(m) => m,
        };
        let msg = match msg.downcast::<FeatAnswer>() {
            Ok(a) => {
                if a.0 == "setup" {
                    self.sheet = "features".into();
                    h.ask("panels.openSettings", json!(["update"]));
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
    fn the_update_running_elsewhere_says_where() {
        assert_eq!(busy_where("abc_1", "abc_1"), "this");
        assert_eq!(busy_where("abc_1", "xyz_2"), "other");
        assert_eq!(busy_where("", "xyz_2"), "");
    }
    #[test]
    fn steps_are_not_output() {
        assert_eq!(strip_steps("@@step system\nhello\nworld"), "hello\nworld");
    }
}

#[cfg(test)]
mod log_tail_tests {
    use super::log_tail;
    #[test]
    fn keeps_the_last_lines_without_steps() {
        let log: String = (0..5000).map(|i| if i % 10 == 0 { format!("@@step {i}\n") } else { format!("line {i}\n") }).collect();
        let t = log_tail(log.trim_end(), 200);
        let lines: Vec<&str> = t.split('\n').collect();
        assert_eq!(lines.len(), 200);
        assert_eq!(*lines.last().unwrap(), "line 4999");
        assert!(lines.iter().all(|l| l.starts_with("line ")));
    }
    #[test]
    fn short_logs_whole_and_utf8_safe() {
        assert_eq!(log_tail("a\n@@step x\nb", 200), "a\nb");
        let long = "é".repeat(100_000);
        assert!(log_tail(&long, 2).chars().all(|c| c == 'é'));
        assert_eq!(log_tail("", 5), "");
    }
}

/// The offer's notification: how many, and the first few by name
pub fn offer_text(items: &[Value]) -> String {
    let names: Vec<String> = items.iter().take(3).filter_map(|i| i["title"].as_str()).map(str::to_string).collect();
    let n = items.len();
    let more = if n > names.len() { format!(" and {} more", n - names.len()) } else { String::new() };
    format!("{}{more} can be set up on this computer. Review them in Software Update; one password sets them all up.", names.join(", "))
}

/// sysupdate.py's arguments for features.py's plan (None: no plan)
pub fn features_args(plan: &Value) -> Option<Vec<String>> {
    let list = |k: &str| -> Vec<String> { plan[k].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default() };
    let ids = list("ids");
    if ids.is_empty() {
        return None;
    }
    let mut a: Vec<String> = Vec::new();
    for (flag, key) in [("--install", "install"), ("--aur-install", "aur")] {
        let l = list(key);
        if !l.is_empty() {
            a.push(flag.into());
            a.push(l.join(","));
        }
    }
    a.push("--features".into());
    a.push(ids.join(","));
    // (the root helper's options: its first is the ids again)
    a.extend(list("rootArgs").into_iter().skip(1));
    Some(a)
}

/// What a run of features did: {"ok": [{id, title}], "failed": [{id, title,
/// error, fix}], "error"}. A run that stopped before its features (the
/// packages didn't install) fails all of them with its error.
pub fn features_result(items: &[Value], job: &Value) -> Value {
    let title = |id: &str| items.iter().find(|i| i["id"] == id).and_then(|i| i["title"].as_str()).unwrap_or(id).to_string();
    let fix = |id: &str| items.iter().find(|i| i["id"] == id).and_then(|i| i["fix"].as_str()).unwrap_or("").to_string();
    let res = job["features"].as_object().cloned().unwrap_or_default();
    let (mut ok, mut failed) = (Vec::new(), Vec::new());
    for (id, r) in &res {
        if r["ok"] == true {
            ok.push(json!({"id": id, "title": title(id)}));
        } else {
            failed.push(json!({"id": id, "title": title(id), "error": r["error"].as_str().unwrap_or(""), "fix": fix(id)}));
        }
    }
    let error = if job["state"] == "done" { String::new() } else { job["error"].as_str().filter(|e| !e.is_empty()).or(job["text"].as_str()).unwrap_or("It stopped.").to_string() };
    json!({"ok": ok, "failed": failed, "error": error})
}

#[cfg(test)]
mod features_tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn the_offer_names_a_few() {
        let items = vec![json!({"title": "Fingerprint"}), json!({"title": "Quick Share"}), json!({"title": "Boot animation"}), json!({"title": "Cloud drives"})];
        assert_eq!(offer_text(&items[..1]), "Fingerprint can be set up on this computer. Review them in Software Update; one password sets them all up.");
        assert!(offer_text(&items).starts_with("Fingerprint, Quick Share, Boot animation and 1 more can be set up"));
    }
    #[test]
    fn a_plan_becomes_one_run() {
        let plan = json!({"ids": ["fingerprint", "cast"], "install": ["yt-dlp"], "aur": ["libfprint-tod", "fprintd"], "root": ["fingerprint"], "user": [],
                          "rootArgs": ["fingerprint", "--fprintd-wait"]});
        assert_eq!(features_args(&plan).unwrap(), ["--install", "yt-dlp", "--aur-install", "libfprint-tod,fprintd", "--features", "fingerprint,cast", "--fprintd-wait"]);
        assert!(features_args(&json!({"ids": []})).is_none());
        assert!(features_args(&Value::Null).is_none());
    }
    #[test]
    fn results_say_what_to_finish() {
        let items = vec![json!({"id": "face", "title": "Face unlock", "fix": "Settings → Privacy & Security → Face sets it up."})];
        let r = features_result(&items, &json!({"state": "done", "features": {"face": {"ok": false, "error": "no camera"}, "cast": {"ok": true, "error": ""}}}));
        assert_eq!(r["failed"][0]["title"], "Face unlock");
        assert_eq!(r["failed"][0]["fix"], "Settings → Privacy & Security → Face sets it up.");
        assert_eq!(r["ok"][0]["id"], "cast");
        assert_eq!(r["error"], "");
        let r = features_result(&items, &json!({"state": "error", "error": "The packages didn't install", "features": {}}));
        assert_eq!(r["error"], "The packages didn't install");
    }
}

/// updates.update's arguments, sent flat (["--install", "a,b"]) or as one
/// list ([["--install", "a,b"]]: fingerprint, face unlock and cloud drives
/// do). Only the flat form was read, so those three ran an empty job that
/// said "Updated" and installed nothing (19 Nov: a fingerprint driver).
pub fn update_args(v: &Value) -> Vec<String> {
    let strs = |a: &Vec<Value>| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect::<Vec<_>>();
    match v.as_array() {
        Some(a) if a.len() == 1 && a[0].is_array() => a[0].as_array().map(strs).unwrap_or_default(),
        Some(a) => strs(a),
        None => Vec::new(),
    }
}

#[cfg(test)]
mod update_args_tests {
    use super::update_args;
    use serde_json::json;
    #[test]
    fn flat_and_nested_both_read() {
        let want = vec!["--install".to_string(), "fprintd".into(), "--aur-install".into(), "libfprint-tod".into()];
        assert_eq!(update_args(&json!(["--install", "fprintd", "--aur-install", "libfprint-tod"])), want);
        assert_eq!(update_args(&json!([["--install", "fprintd", "--aur-install", "libfprint-tod"]])), want);
        assert!(update_args(&json!([])).is_empty());
        assert!(update_args(&json!(null)).is_empty());
    }
}

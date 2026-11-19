//! shell/services/SoftwareUpdate.qml: Software Update. Nothing is checked on
//! its own; Check for updates looks at Tessera (git fetch, the new version's
//! compat.json and packages), the system (checkupdates, AUR) and Tessera's
//! packages, and makes one plan in two parts, each with its own Update
//! (since background updates; the QML shell keeps its one button):
//!   Tessera          update_tessera: its packages first when it needs new
//!                    ones (sysupdate.py), then TesseraUpdate.stage, in the
//!                    background; the shell restarts when you choose, at
//!                    the next lock or at the next login
//!   System packages  update_system: sysupdate.py (snapshot, pacman -Syu
//!                    [holding Hyprland back if you chose to], the AUR), in
//!                    the background; a new Hyprland's glass plugin is
//!                    rebuilt after it (loaded at the next login); a kernel,
//!                    driver or core library ends with "Restart the computer")
//! One part at a time. Install updates does both: the system first, with
//! Tessera's own packages (one password), then Tessera's part.
//!
//! A plain model owned by the About service (bridge_about.rs).

use super::helpers::{self, Tx};
use super::tessera_update::TesseraUpdate;
use super::updates::Updates;
use crate::util;
use serde_json::{json, Value};
use std::time::Duration;
use tessera_core::host::Host;

pub enum Ev {
    Compat(Option<Value>),
    Hypr(String),
    Snap(String),
    Helper(i32),
    Undo(Option<Value>),
}

pub struct SoftwareUpdate {
    pub checked: bool,
    /// when the last check asked for finished (s; "Last checked at …")
    pub checked_at: f64,
    asked: bool,
    pub restart_pending: bool,
    /// the tested Hyprland range of the version that would be installed
    pub compat: Option<Value>,
    compat_pending: bool,
    compat_busy: bool,
    hypr_busy: bool,
    snap_busy: bool,
    helper_busy: bool,
    undo_busy: bool,
    pub hypr_installed: String,
    /// "snapper" | "timeshift" | ""
    pub snapshot: String,
    pub helper_current: bool,
    last_incoming: String,
    /// your choice: -1 not yet, 1 keep the current Hyprland, 0 update anyway
    pub hold: i64,
    pub remove_dropped: bool,
    pub include_aur: bool,
    /// { at, packages: [{ name, from, to }], files, missing, skip: { name: version } }
    pub undo: Value,
    pub include_skipped: Vec<String>,
    was_checking: bool,
    /// Updates.fake, as last seen (the fake run's helper is always current)
    pub fake: bool,
}

impl Default for SoftwareUpdate {
    fn default() -> Self {
        Self {
            checked: false,
            checked_at: 0.0,
            asked: false,
            restart_pending: false,
            compat: None,
            compat_pending: false,
            compat_busy: false,
            hypr_busy: false,
            snap_busy: false,
            helper_busy: false,
            undo_busy: false,
            hypr_installed: String::new(),
            snapshot: String::new(),
            helper_current: true,
            last_incoming: String::new(),
            hold: -1,
            remove_dropped: true,
            include_aur: false,
            undo: json!({}),
            include_skipped: vec![],
            was_checking: false,
            fake: false,
        }
    }
}

/// Hyprland and the hypr* / aquamarine libraries it's built with (and
/// anything named for it, like its portal): /^(hypr|aquamarine)|hyprland/
pub fn is_hypr(name: &str) -> bool {
    name.starts_with("hypr") || name.starts_with("aquamarine") || name.contains("hyprland")
}

pub fn step_name(s: &str) -> String {
    let n = match s {
        "snapshot" => "Snapshot",
        "setup" => "Helper",
        "system" => "System",
        "packages" => "Tessera packages",
        "remove" => "Unused packages",
        "aur" => "AUR",
        "tessera" => "Tessera",
        "restart" => "Restart",
        o => o,
    };
    n.to_string()
}

fn names(l: &[Value]) -> Vec<String> {
    l.iter().filter_map(|p| p["name"].as_str().map(str::to_string)).collect()
}

fn strs(v: &Value) -> Vec<String> {
    v.as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default()
}

impl SoftwareUpdate {
    pub fn checking(&self, tu: &TesseraUpdate, up: &Updates) -> bool {
        tu.checking || up.checking || tu.packages_checking || self.compat_pending || self.compat_busy || self.hypr_busy || self.snap_busy || self.helper_busy
    }
    pub fn running(up: &Updates, tu: &TesseraUpdate) -> bool {
        up.running() || tu.job_running() || tu.restarting
    }

    /// `onCheckingChanged`: a check that was asked for has finished
    pub fn settle(&mut self, tu: &TesseraUpdate, up: &Updates) {
        let now = self.checking(tu, up);
        if self.was_checking != now {
            self.was_checking = now;
            if !now && self.asked {
                self.asked = false;
                self.checked = true;
                self.checked_at = helpers::now_s();
            }
        }
    }

    pub fn incoming(tu: &TesseraUpdate) -> String {
        match &tu.report {
            Some(r) if r["available"] == true => r["upstream"].as_str().unwrap_or("").to_string(),
            _ => String::new(),
        }
    }

    pub fn check(&mut self, tx: &Tx, tu: &mut TesseraUpdate, up: &mut Updates) {
        if Self::running(up, tu) || self.checking(tu, up) {
            return;
        }
        self.fake = up.fake;
        self.clear_finished(tu, up);
        self.asked = true;
        self.hold = -1;
        // This check's compat.json is read once the Tessera check has said which
        // version would be installed; until then the plan can't be trusted
        // (an old compat could let an untested Hyprland through unasked)
        self.compat = None;
        self.compat_pending = true;
        tu.check(tx);
        up.refresh(tx);
        self.run_hypr(tx);
        self.run_snap(tx, up.fake);
        self.run_helper(tx, up, tu);
        self.was_checking = self.checking(tu, up);
    }

    /// A new check: the last run's result gives way to what's found now. A
    /// restart that run needed is still asked for.
    fn clear_finished(&mut self, tu: &mut TesseraUpdate, up: &mut Updates) {
        if Self::running(up, tu) {
            return;
        }
        if up.job.as_ref().is_some_and(|j| j["state"] == "done" && j["restart"] == true) {
            self.restart_pending = true;
        }
        up.dismiss();
        tu.dismiss_result();
    }

    /// Read compat.json once the Tessera check has said which version it is
    pub fn read_compat(&mut self, tx: &Tx, tu: &TesseraUpdate) {
        if !self.compat_pending || self.compat_busy || tu.checking || tu.pending_check() {
            return;
        }
        let reference = match &tu.report {
            Some(r) if r["available"] == true => r["upstream"].as_str().unwrap_or("HEAD").to_string(),
            _ => "HEAD".into(),
        };
        self.compat_busy = true;
        let (t, repo) = (tx.clone(), tu.repo.clone());
        std::thread::spawn(move || {
            let out = util::capture(&["git", "-C", &repo, "show", &format!("{reference}:compat.json")], Duration::from_secs(20)).map(|(_, o)| o).unwrap_or_default();
            t(Box::new(Ev::Compat(serde_json::from_str(&out).ok())));
        });
    }

    fn run_hypr(&mut self, tx: &Tx) {
        if self.hypr_busy {
            return;
        }
        self.hypr_busy = true;
        let t = tx.clone();
        std::thread::spawn(move || {
            let o = util::sh("pacman -Q hyprland 2>/dev/null | awk '{print $2}'", Duration::from_secs(10)).map(|(_, o)| o).unwrap_or_default();
            t(Box::new(Ev::Hypr(o.trim().to_string())));
        });
    }

    /// A snapshot tool that's set up: snapper with a root config, or timeshift
    /// on btrfs (an rsync snapshot would take far too long)
    fn run_snap(&mut self, tx: &Tx, fake: bool) {
        if self.snap_busy {
            return;
        }
        self.snap_busy = true;
        let t = tx.clone();
        std::thread::spawn(move || {
            let o = util::sh("if command -v snapper >/dev/null && [ -f /etc/snapper/configs/root ]; then echo snapper; elif command -v timeshift >/dev/null && [ \"$(findmnt -no FSTYPE /)\" = btrfs ]; then echo timeshift; fi", Duration::from_secs(10)).map(|(_, o)| o).unwrap_or_default();
            t(Box::new(Ev::Snap(if fake { "snapper".into() } else { o.trim().to_string() })));
        });
    }

    /// Tessera's root helper (/usr/lib/tessera) is missing or older than this
    /// folder's: the update's root part installs it
    fn run_helper(&mut self, tx: &Tx, up: &Updates, tu: &TesseraUpdate) {
        if self.helper_busy {
            return;
        }
        self.helper_busy = true;
        let incoming = Self::incoming(tu);
        self.last_incoming = incoming.clone();
        let (t, script) = (tx.clone(), up.script.clone());
        std::thread::spawn(move || {
            let mut c = vec!["python3".to_string(), "-I".into(), script, "helper-status".into()];
            if !incoming.is_empty() {
                c.push("--incoming".into());
                c.push(incoming);
            }
            let c: Vec<&str> = c.iter().map(String::as_str).collect();
            let code = util::capture(&c, Duration::from_secs(60)).map(|(c, _)| c).unwrap_or(-1);
            t(Box::new(Ev::Helper(code)));
        });
    }

    /// `onIncomingChanged`: look at the helper again for the new version
    pub fn follow_incoming(&mut self, tx: &Tx, up: &Updates, tu: &TesseraUpdate) {
        let now = Self::incoming(tu);
        if now != self.last_incoming {
            self.last_incoming = now;
            if !self.helper_busy {
                self.run_helper(tx, up, tu);
            }
        }
    }

    pub fn check_undo(&mut self, tx: &Tx, up: &Updates) {
        if self.undo_busy {
            return;
        }
        self.undo_busy = true;
        let (t, script) = (tx.clone(), up.script.clone());
        std::thread::spawn(move || {
            let out = util::capture(&["python3", "-I", &script, "undo-check"], Duration::from_secs(60)).map(|(_, o)| o).unwrap_or_default();
            t(Box::new(Ev::Undo(serde_json::from_str(&out).ok())));
        });
    }

    pub fn event(&mut self, ev: Ev) {
        match ev {
            Ev::Compat(c) => {
                self.compat = c;
                self.compat_busy = false;
                self.compat_pending = false;
            }
            Ev::Hypr(v) => {
                self.hypr_installed = v;
                self.hypr_busy = false;
            }
            Ev::Snap(s) => {
                self.snapshot = s;
                self.snap_busy = false;
            }
            Ev::Helper(code) => {
                self.helper_busy = false;
                self.helper_current = code == 0 || self.fake;
            }
            Ev::Undo(u) => {
                self.undo_busy = false;
                if let Some(u) = u {
                    self.undo = u;
                }
            }
        }
    }

    // ── Derived ─────────────────────────────────────────────────────────
    pub fn hypr_update(&self, up: &Updates) -> Option<Value> {
        up.list.iter().find(|p| p["name"] == "hyprland").cloned()
    }
    pub fn hypr_max(&self) -> String {
        self.compat.as_ref().and_then(|c| c["hyprland"]["max"].as_str()).unwrap_or("").to_string()
    }
    /// The repo's Hyprland is newer than this Tessera is tested with
    pub fn hypr_too_new(&self, up: &Updates) -> bool {
        match self.hypr_update(up) {
            Some(u) if !self.hypr_max().is_empty() => helpers::cmp(&helpers::parts(u["to"].as_str().unwrap_or("")), &helpers::parts(&self.hypr_max())) > 0,
            _ => false,
        }
    }
    pub fn hypr_short(&self, up: &Updates) -> String {
        self.hypr_update(up).map(|u| helpers::parts(u["to"].as_str().unwrap_or("")).iter().take(2).map(i64::to_string).collect::<Vec<_>>().join(".")).unwrap_or_default()
    }
    pub fn hypr_now(&self) -> String {
        if self.hypr_installed.is_empty() {
            String::new()
        } else {
            helpers::parts(&self.hypr_installed).iter().take(2).map(i64::to_string).collect::<Vec<_>>().join(".")
        }
    }
    fn all(up: &Updates) -> Vec<Value> {
        up.list.iter().chain(up.aur_list.iter()).cloned().collect()
    }
    pub fn held_hypr(&self, up: &Updates) -> Vec<String> {
        if self.hold == 1 {
            names(&Self::all(up)).into_iter().filter(|n| is_hypr(n)).collect()
        } else {
            vec![]
        }
    }
    /// Updates offered at a version you went back from
    pub fn skipped_all(&self, up: &Updates) -> Vec<Value> {
        Self::all(up).into_iter().filter(|p| self.undo["skip"][p["name"].as_str().unwrap_or("")] == p["to"] && !p["to"].is_null()).collect()
    }
    pub fn skipped(&self, up: &Updates) -> Vec<String> {
        names(&self.skipped_all(up)).into_iter().filter(|n| !self.include_skipped.contains(n)).collect()
    }
    /// Everything kept back this time: Hyprland's, and versions you went back from
    pub fn held(&self, up: &Updates) -> Vec<String> {
        let mut h = self.held_hypr(up);
        for n in self.skipped(up) {
            if !h.contains(&n) {
                h.push(n);
            }
        }
        h
    }
    pub fn include_skip(&mut self, name: &str, on: bool) {
        self.include_skipped.retain(|n| n != name);
        if on {
            self.include_skipped.push(name.to_string());
        }
    }

    /// The plan: { tessera, system, aur, install, remove, rebuild, setup, sysCount, aurCount }
    pub fn plan(&self, tu: &TesseraUpdate, up: &Updates) -> Value {
        let held = self.held(up);
        let sys = up.list.iter().filter(|p| !held.contains(&p["name"].as_str().unwrap_or("").to_string())).count();
        let aur = up.aur_list.iter().filter(|p| !held.contains(&p["name"].as_str().unwrap_or("").to_string())).count();
        let pk = &tu.packages;
        json!({
            "tessera": tu.available(),
            "system": sys > 0 || up.fake, // (the sandbox's simulated run can always be tried)
            "aur": self.include_aur && aur > 0,
            "install": if pk["missing"].is_array() { pk["missing"].clone() } else { json!([]) },
            "remove": if self.remove_dropped && pk["dropped"].is_array() { pk["dropped"].clone() } else { json!([]) },
            // Hyprland changes: rebuild the glass plugin against it
            "rebuild": if self.hypr_update(up).is_some() && self.hold != 1 { json!(["plugin"]) } else { json!([]) },
            "setup": !self.helper_current,
            "sysCount": sys,
            "aurCount": aur,
        })
    }
    pub fn anything(&self, tu: &TesseraUpdate, up: &Updates) -> bool {
        let p = self.plan(tu, up);
        let n = |k: &str| p[k].as_array().map(|a| a.len()).unwrap_or(0);
        p["tessera"] == true || p["system"] == true || p["aur"] == true || n("install") > 0 || n("remove") > 0 || (p["setup"] == true && !up.fake)
    }
    pub fn undo_count(&self) -> usize {
        self.undo["packages"].as_array().map(|a| a.len()).unwrap_or(0)
    }
    pub fn can_go_back(&self, tu: &TesseraUpdate, up: &Updates) -> bool {
        self.undo_count() > 0 && self.undo["missing"].as_array().map(|a| a.is_empty()).unwrap_or(true) && !Self::running(up, tu)
    }
    // ── Update ──────────────────────────────────────────────────────────
    // Two parts, each with its own Update, both in the background (no
    // full-screen screen): Tessera (TesseraUpdate.stage: the new version
    // made ready, then a restart of the shell when you choose, at the next
    // lock or at the next login) and System packages (sysupdate.py: pacman,
    // the AUR). They never run at once: the Tessera part builds against
    // the system's libraries.

    /// Tessera's own part can start: checked, available, every conflict
    /// decided, nothing running, nothing already staged
    pub fn can_update_tessera(&self, tu: &TesseraUpdate, up: &Updates) -> bool {
        self.checked && !self.checking(tu, up) && tu.available() && tu.undecided() == 0 && !Self::running(up, tu) && !tu.ready() && tu.allowed()
    }
    /// The system packages' part can start
    pub fn can_update_system(&self, tu: &TesseraUpdate, up: &Updates) -> bool {
        let p = self.plan(tu, up);
        let sys = p["system"] == true || p["aur"] == true;
        self.checked && !self.checking(tu, up) && sys && !(self.hypr_too_new(up) && self.hold == -1) && !Self::running(up, tu)
    }
    /// Update Tessera. Tessera's own packages it needs (and the root
    /// helper, when it's older) come first, through sysupdate.py with one
    /// password; then the job starts.
    pub fn update_tessera(&mut self, h: &Host, tx: &Tx, tu: &mut TesseraUpdate, up: &mut Updates) {
        if !self.can_update_tessera(tu, up) {
            return;
        }
        let mut args = self.tessera_packages(tu, up);
        if args.is_empty() {
            tu.stage(h, tx, &[]);
            return;
        }
        args.push("--then".into());
        args.push(json!({"tessera": true, "rebuild": [], "at": helpers::now_s()}).to_string());
        up.update(tx, &args);
    }

    /// Update the system packages (pacman, and the AUR if you chose).
    /// Hyprland changing: its glass plugin is rebuilt in the background
    /// afterwards, and Hyprland loads it at the next login. A kernel,
    /// driver, mesa, systemd or glibc update ends with "Restart the
    /// computer to finish" instead of interrupting.
    pub fn update_system(&mut self, tx: &Tx, tu: &mut TesseraUpdate, up: &mut Updates) {
        self.system_then(tx, tu, up, false);
    }

    /// Tessera's own packages (and its root helper, when older), as
    /// sysupdate.py's arguments: none when it needs nothing
    fn tessera_packages(&self, tu: &TesseraUpdate, up: &Updates) -> Vec<String> {
        let p = self.plan(tu, up);
        let list = |k: &str| strs(&p[k]);
        let (install, remove) = (list("install"), list("remove"));
        let mut args: Vec<String> = Vec::new();
        if !install.is_empty() {
            args.push("--install".into());
            args.push(install.join(","));
        }
        if !remove.is_empty() {
            args.push("--remove".into());
            args.push(remove.join(","));
        }
        let incoming = Self::incoming(tu);
        if p["setup"] == true && !incoming.is_empty() {
            args.push("--incoming".into());
            args.push(incoming);
        }
        args
    }

    /// The system packages, then (tessera) Tessera's part once they're done
    fn system_then(&mut self, tx: &Tx, tu: &mut TesseraUpdate, up: &mut Updates, tessera: bool) {
        if !self.can_update_system(tu, up) {
            return;
        }
        let p = self.plan(tu, up);
        let rebuild = strs(&p["rebuild"]);
        let held = self.held(up);
        let mut args: Vec<String> = Vec::new();
        if p["system"] == true {
            args.push("--system".into());
        }
        if !self.snapshot.is_empty() {
            args.push("--snapshot".into());
            args.push(self.snapshot.clone());
        }
        if !held.is_empty() {
            args.push("--ignore".into());
            args.push(held.join(","));
        }
        if p["aur"] == true {
            args.push("--aur".into());
        }
        if tessera {
            args.extend(self.tessera_packages(tu, up));
        }
        args.push("--then".into());
        args.push(json!({"tessera": tessera, "rebuild": rebuild, "at": helpers::now_s()}).to_string());
        up.update(tx, &args);
    }

    /// Install updates (the status's button): both parts. The system's
    /// first, with Tessera's own packages in the same run (one password);
    /// Tessera's part starts in the background once it's done
    /// (updates_finished). Only one of them to do: that one.
    pub fn update(&mut self, h: &Host, tx: &Tx, tu: &mut TesseraUpdate, up: &mut Updates) {
        let tessera = self.can_update_tessera(tu, up);
        if self.can_update_system(tu, up) {
            self.system_then(tx, tu, up, tessera);
        } else if tessera {
            self.update_tessera(h, tx, tu, up);
        }
    }

    pub fn go_back(&mut self, tx: &Tx, tu: &TesseraUpdate, up: &mut Updates) {
        if !self.can_go_back(tu, up) || up.running() {
            return;
        }
        let mut args = vec!["--undo".to_string()];
        if !self.snapshot.is_empty() {
            args.push("--snapshot".into());
            args.push(self.snapshot.clone());
        }
        up.update(tx, &args);
    }

    /// The root part finished: Tessera's job (it was asked for), or the
    /// glass plugin rebuilt for a new Hyprland, both in the background
    pub fn updates_finished(&mut self, h: &Host, tx: &Tx, tu: &mut TesseraUpdate, up: &Updates, j: &Value) {
        self.check_undo(tx, up); // (a new update to go back from, or one gone back from)
        let after = &j["then"];
        let rebuild = strs(&after["rebuild"]);
        if j["state"] != "done" || after.is_null() || !(helpers::truthy(&after["tessera"]) || !rebuild.is_empty()) {
            return;
        }
        if up.fake {
            helpers::notify_send(&["-a", "Tessera", "Sandbox", &format!("Tessera's part would start now in the background (rebuild {}).", if rebuild.is_empty() { "nothing".to_string() } else { rebuild.join(", ") })]);
            return;
        }
        tu.stage(h, tx, &rebuild);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hyprland_family() {
        for n in ["hyprland", "hyprlang", "aquamarine", "xdg-desktop-portal-hyprland"] {
            assert!(is_hypr(n), "{n}");
        }
        assert!(!is_hypr("firefox"));
    }
    #[test]
    fn step_names() {
        assert_eq!(step_name("packages"), "Tessera packages");
        assert_eq!(step_name("odd"), "odd");
    }
}

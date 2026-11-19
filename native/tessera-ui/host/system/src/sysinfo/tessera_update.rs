//! shell/services/TesseraUpdate.qml: Tessera's own updates (not the system's:
//! updates.rs), from the git repository the shell runs from, through
//! tools/tessera-update.py. Checked only when asked (Software Update ->
//! Check for updates). The report lists what's new and every conflict (your
//! code edits, settings whose defaults change, the material you use,
//! shortcut clashes); nothing is applied until each one is decided.
//!
//! Updating is the background job (`stage`: the new version made ready
//! with the shell running); restarting into it hands over to
//! assets/scripts/update-run.sh … finish, started detached from a copy: it
//! stops this shell, swaps in what was staged and starts the new shell, in
//! seconds.
//!
//! The channel (config.json's update.channel: stable follows the branch
//! main, beta the branch beta) is passed to the helper. Every few hours,
//! while update.autoCheck is on, a light check (`peek`) says whether a newer
//! Tessera is out on it, as a notice with Update / Later: Update opens
//! Software Update with a fresh check; nothing is installed from here.
//!
//! A plain model owned by the About service (bridge_about.rs).

use super::helpers::{self, Ticker, Tx};
use crate::util;
use serde_json::{json, Map, Value};
use std::time::Duration;
use tessera_core::host::Host;

pub enum Ev {
    /// update-source.py's answer (the repo it found)
    Source(String),
    /// tools/tessera-update.py check: its output
    Report(String),
    /// tessera-packages.sh status: its output
    Packages(String),
    /// the result file of the last apply (None: no file)
    Result(Option<Value>),
    /// tessera-update.py info: this copy's channel and version
    Info(String),
    /// tessera-update.py peek: is there a newer version to mention
    Peek(String),
    /// the notice's answer (notify-send's action: "update", "later" or "")
    Answer(String),
    /// the background job's status.json (None: none yet)
    Job(Option<Value>),
    /// tessera-update.py staged: what's ready to restart into
    Staged(Option<Value>),
    /// the ready notice's answer ("restart" or "")
    ReadyAnswer(String),
    /// native-install.py cache: the build cache's size (None: couldn't tell)
    Cache(Option<u64>),
    /// the cache was deleted (or not: why)
    CacheDeleted(Result<u64, String>),
    /// launch-shell.sh went back to the previous build: its note
    RolledBack(Value),
}

/// The job's state as Settings and the island show it: (title, detail).
/// Every state has its words: preparing, downloading N MB (a patch or the
/// whole update), building, ready to restart, failed (with how to fix it),
/// cancelled, up to date.
pub fn job_words(job: &Value) -> (String, String) {
    let s = |k: &str| job[k].as_str().unwrap_or("").to_string();
    let mb = job["bytes"].as_f64().unwrap_or(0.0) / 1e6;
    match job["state"].as_str().unwrap_or("") {
        "running" => {
            let title = match job["phase"].as_str().unwrap_or("") {
                "download" if job["how"] == "patch" => format!("Downloading a patch · {mb:.1} MB"),
                "download" => format!("Downloading · {mb:.1} MB"),
                "build" => "Building Tessera".to_string(),
                "parts" => s("text"),
                _ => "Preparing the update".to_string(),
            };
            (title, "You can keep working. The shell restarts only when you choose.".into())
        }
        "ready" => ("Ready to restart".into(), format!("Tessera {} is ready. Restart the shell now, or it restarts the next time you lock the screen or log in. Your windows stay open.", s("latest"))),
        "error" => ("The update didn't finish".into(), {
            let e = s("error");
            format!("{}{}", if e.is_empty() { "Something stopped it." } else { &e }, " Nothing was changed. Try again, or check your connection first.")
        }),
        "cancelled" => ("Update cancelled".into(), "Nothing was changed.".into()),
        "done" => ("Done".into(), "The glass plugin was rebuilt. It loads the next time you log in.".into()),
        "none" => ("Tessera is up to date".into(), s("text")),
        // (the repo lock held by an update running elsewhere: tessera-update.py's words)
        "busy" => ("Software Update is already running".into(), s("error")),
        _ => (String::new(), String::new()),
    }
}

/// Restart at the lock: the screen has just locked (the compositor confirmed
/// it: lockSecure went true), an update is ready for this copy, and no
/// restart is under way
pub fn restart_at_lock(was_secure: bool, secure: bool, ready: bool, restarting: bool) -> bool {
    secure && !was_secure && ready && !restarting
}

/// Was the job cut off (it says running, its process is gone)?
pub fn job_cut_off(job: &Value, alive: impl Fn(i64) -> bool) -> bool {
    job["state"] == "running" && !job["pid"].as_i64().is_some_and(&alive)
}

fn pid_alive(pid: i64) -> bool {
    std::fs::read(format!("/proc/{pid}/cmdline")).map(|c| String::from_utf8_lossy(&c).contains("tessera-update")).unwrap_or(false)
}

/// The background job's log (tessera-update.py's job_dir()/log): the
/// details sheet's while Tessera's part runs
pub fn job_log() -> std::path::PathBuf {
    std::path::PathBuf::from(format!("{}/tessera/update-job/log", state_home()))
}

fn state_home() -> String {
    std::env::var("XDG_STATE_HOME").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| format!("{}/.local/state", std::env::var("HOME").unwrap_or_default()))
}

/// The first light check after login, then every six hours
const PEEK_FIRST_MS: u64 = 3 * 60 * 1000;
const PEEK_EVERY_MS: u64 = 6 * 60 * 60 * 1000;

pub struct TesseraUpdate {
    pub repo: String,
    source_ready: bool,
    pending_check: bool,
    pub report: Option<Value>,
    pub checking: bool,
    pub error: String,
    pub result: Option<Value>,
    pub choices: Map<String, Value>,
    pub packages: Value, // { missing, unavailable, dropped, reasons }
    pub packages_checking: bool,
    /// update.channel as the helper gets it ("stable", "beta"; empty: the
    /// branch this copy follows)
    pub channel: String,
    /// tessera-update.py info: { channel, channelName, branch, current, date, … }
    pub info: Option<Value>,
    /// the notice's Update was pressed: About opens Software Update and checks
    pub open_asked: bool,
    peeking: bool,
    t_start: Ticker,
    t_announce: Ticker,
    t_peek: Ticker,
    /// the background job's status (tessera-update.py stage: status.json)
    pub job: Option<Value>,
    /// what's staged (tessera-update.py staged): {ready, valid, latest, …}
    pub staged: Option<Value>,
    /// a restart into the staged update is under way
    pub restarting: bool,
    /// the build cache's size in bytes (None: not known yet)
    pub cache: Option<u64>,
    pub cache_busy: bool,
    pub cache_error: String,
    /// the last lockSecure seen (the lock trigger: restart_at_lock)
    pub was_secure: bool,
    /// when the restart was asked for (a "busy" status after it ends it)
    restart_at: f64,
    t_job: Ticker,
}

impl Default for TesseraUpdate {
    fn default() -> Self {
        Self {
            repo: std::env::var("TESSERA_UPDATE_REPO").unwrap_or_default(),
            source_ready: false,
            pending_check: false,
            report: None,
            checking: false,
            error: String::new(),
            result: None,
            choices: Map::new(),
            packages: json!({"missing": [], "unavailable": [], "dropped": []}),
            packages_checking: false,
            channel: String::new(),
            info: None,
            open_asked: false,
            peeking: false,
            t_start: Ticker::new("tuStart"),
            t_announce: Ticker::new("tuAnnounce"),
            t_peek: Ticker::new("tuPeek"),
            job: None,
            staged: None,
            restarting: false,
            cache: None,
            cache_busy: false,
            cache_error: String::new(),
            was_secure: false,
            restart_at: 0.0,
            t_job: Ticker::new("tuJob"),
        }
    }
}

fn config_dir() -> String {
    tessera_core::theme::config_path().parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()
}

/// The helper's --channel for update.channel (nothing until one is chosen)
pub fn channel_args(channel: &str) -> Vec<String> {
    match channel {
        "stable" | "beta" => vec!["--channel".into(), channel.into()],
        _ => vec![],
    }
}

/// The notice's text for a peek that says to mention it (None: nothing to say)
pub fn notice(r: &Value) -> Option<String> {
    if r["notify"] != true {
        return None;
    }
    let n = r["behind"].as_u64().unwrap_or(0);
    Some(format!("Tessera {} has {n} new change{}. Nothing installs until you choose Update.", r["channelName"].as_str().unwrap_or(""), if n == 1 { "" } else { "s" }))
}

impl TesseraUpdate {
    pub fn helper(&self) -> String {
        format!("{}/tools/tessera-update.py", self.repo)
    }
    /// update.channel from the settings (before a check: the helper reads
    /// config.json too, but a change may not be written yet)
    pub fn sync_channel(&mut self, h: &Host) {
        self.channel = h.cfg()["update"]["channel"].as_str().unwrap_or("").to_string();
    }
    /// What this copy is now (no download): the channel picker's line
    pub fn read_info(&self, tx: &Tx) {
        if self.repo.is_empty() {
            return;
        }
        let (t, helper, cfg, ch) = (tx.clone(), self.helper(), config_dir(), channel_args(&self.channel));
        std::thread::spawn(move || {
            let mut c = vec!["python3", "-I", &helper, "info", "--config", &cfg];
            c.extend(ch.iter().map(String::as_str));
            let out = util::capture(&c, Duration::from_secs(30)).map(|(_, o)| o).unwrap_or_default();
            t(Box::new(Ev::Info(out)));
        });
    }
    /// The light check (tuPeek): fetches, and says whether to mention a new version
    fn peek(&mut self, h: &Host, tx: &Tx) {
        if self.peeking || self.checking || self.job_running() || self.repo.is_empty() || !self.allowed() || h.cfg()["update"]["autoCheck"] == false {
            return;
        }
        self.sync_channel(h);
        self.peeking = true;
        let (t, helper, cfg, ch) = (tx.clone(), self.helper(), config_dir(), channel_args(&self.channel));
        std::thread::spawn(move || {
            let mut c = vec!["python3", "-I", &helper, "peek", "--mark", "--config", &cfg];
            c.extend(ch.iter().map(String::as_str));
            let out = util::capture(&c, Duration::from_secs(300)).map(|(_, o)| o).unwrap_or_default();
            t(Box::new(Ev::Peek(out)));
        });
    }
    /// Never from the dev sandbox on its own copy: it would update the developer's working tree
    pub fn allowed(&self) -> bool {
        !helpers::env_is("SANDBOX") || std::env::var("TESSERA_UPDATE_REPO").map(|v| !v.is_empty()).unwrap_or(false)
    }
    pub fn available(&self) -> bool {
        self.report.as_ref().is_some_and(|r| r["available"] == true && (r["error"].is_null() || r["error"] == "" || r["error"] == false))
    }
    pub fn conflicts(&self) -> Vec<Value> {
        self.report.as_ref().and_then(|r| r["conflicts"].as_array().cloned()).unwrap_or_default()
    }
    /// Conflicts whose choice isn't one of theirs yet
    pub fn undecided(&self) -> usize {
        self.conflicts().iter().filter(|c| !c["choices"].as_array().is_some_and(|ch| self.choices.get(c["id"].as_str().unwrap_or("")).is_some_and(|x| ch.contains(x)))).count()
    }
    pub fn pending_check(&self) -> bool {
        self.pending_check
    }
    pub fn choose(&mut self, id: &str, choice: &str) {
        self.choices.insert(id.to_string(), json!(choice));
    }
    pub fn dismiss_result(&mut self) {
        self.result = None;
    }

    pub fn publish(&self) {
        let job = self.job.clone().unwrap_or(Value::Null);
        crate::bus::put("tesseraupdate", json!({"checking": self.checking, "available": self.available(), "error": self.error, "undecided": self.undecided(),
            // (the island's activity while the job runs)
            "running": self.job_running(), "job": job, "fraction": self.job_fraction(), "ready": self.ready()}));
    }

    // ── The background update (tessera-update.py stage / finish) ─────────
    pub fn job_running(&self) -> bool {
        self.job.as_ref().is_some_and(|j| j["state"] == "running")
    }
    /// An update ready to restart into, for this copy as it is now
    pub fn ready(&self) -> bool {
        self.staged.as_ref().is_some_and(|s| s["ready"] == true && s["valid"] == true)
    }
    /// How far the job is, 0…1 (a build's own progress while it builds)
    pub fn job_fraction(&self) -> f64 {
        let Some(j) = &self.job else { return -1.0 };
        if j["state"] != "running" {
            return -1.0;
        }
        let pct = j["percent"].as_f64().unwrap_or(0.0) / 100.0;
        if j["phase"] == "build" {
            if let Some(f) = tessera_core::build_progress::read().filter(|v| v.active).and_then(|v| v.fraction) {
                // (the build is most of the job: 15 % … 78 %)
                return 0.15 + 0.63 * f as f64;
            }
        }
        pct
    }
    fn job_dir() -> String {
        format!("{}/tessera/update-job", state_home())
    }
    fn read_job(&self, tx: &Tx) {
        let (t, p) = (tx.clone(), format!("{}/status.json", Self::job_dir()));
        std::thread::spawn(move || {
            let j = std::fs::read_to_string(p).ok().and_then(|s| serde_json::from_str::<Value>(&s).ok());
            t(Box::new(Ev::Job(j)));
        });
    }
    pub fn read_staged(&self, tx: &Tx) {
        if self.repo.is_empty() {
            return;
        }
        let (t, helper) = (tx.clone(), self.helper());
        std::thread::spawn(move || {
            let out = util::capture(&["python3", "-I", &helper, "staged"], Duration::from_secs(30)).map(|(_, o)| o).unwrap_or_default();
            t(Box::new(Ev::Staged(serde_json::from_str(out.trim()).ok())));
        });
    }
    /// Update Tessera: the job starts on its own (it outlives Settings and
    /// the shell); rebuild: parts to build anyway (the glass plugin after a
    /// Hyprland update)
    pub fn stage(&mut self, h: &Host, tx: &Tx, rebuild: &[String]) {
        if !self.allowed() || self.repo.is_empty() || self.job_running() || self.restarting {
            return;
        }
        self.sync_channel(h);
        self.error.clear();
        let mut c: Vec<String> = ["setsid", "-f", "python3", "-I", &self.helper(), "stage", "--config", &config_dir()].iter().map(|s| s.to_string()).collect();
        c.push("--choices-json".into());
        c.push(Value::Object(self.choices.clone()).to_string());
        if !rebuild.is_empty() {
            c.push("--rebuild".into());
            c.push(rebuild.join(","));
        }
        c.extend(channel_args(&self.channel));
        let c: Vec<&str> = c.iter().map(String::as_str).collect();
        tessera_core::services::bridge::run(&c);
        self.staged = None;
        self.job = Some(json!({"state": "running", "phase": "prepare", "percent": 0, "text": "Preparing", "started": helpers::now_s()}));
        self.t_job.every(tx, 500);
        self.publish();
    }
    /// Cancel: the job stops (nothing staged stays), or a ready update is forgotten
    pub fn cancel(&mut self, tx: &Tx) {
        let helper = self.helper();
        if self.job_running() {
            tessera_core::services::bridge::run(&["python3", "-I", &helper, "cancel"]);
        } else if self.ready() {
            tessera_core::services::bridge::run(&["python3", "-I", &helper, "discard"]);
            self.staged = None;
            self.job = Some(json!({"state": "cancelled"}));
        }
        self.read_job(tx);
        self.publish();
    }
    /// Restart shell now (or the lock just engaged: locked): update-run.sh
    /// finish stops the shell, swaps in what was staged and starts the new
    /// one, behind the update screen for those seconds (none when locked)
    pub fn restart(&mut self, h: &Host, tx: &Tx, locked: bool) {
        if !self.ready() || self.restarting || self.repo.is_empty() {
            return;
        }
        self.restarting = true;
        self.restart_at = helpers::now_s();
        let script = "t=$(mktemp -d \"${XDG_RUNTIME_DIR:-/tmp}/tessera-update.XXXXXX\") || exit 1; \
            cp -r \"$1/shell/assets/updater\" \"$t/ui\" && cp \"$1/shell/assets/scripts/update-run.sh\" \"$t/run.sh\" && printf '{}' > \"$t/choices.json\" && \
            printf '%s' \"$3\" > \"$t/look.json\" && cp \"$1/shell/assets/fonts/Phosphor-Fill.ttf\" \"$t/ui/\" 2>/dev/null; \
            exec setsid -f bash \"$t/run.sh\" \"$t\" \"$1\" \"$2\" finish $4 </dev/null >/dev/null 2>&1";
        let args = ["sh".to_string(), "-c".into(), script.into(), "sh".into(), self.repo.clone(), config_dir(), self.look(h).to_string(), if locked { "locked".into() } else { String::new() }];
        let c: Vec<&str> = args.iter().map(String::as_str).collect();
        tessera_core::services::bridge::run(&c);
        // (still here a minute on: it didn't go ahead; say so. Meanwhile the
        // job's status is read: an update running elsewhere says so there)
        self.t_start.once(tx, 60000);
        self.t_job.every(tx, 1000);
        self.publish();
    }
    /// The build cache's size (Software Update shows it, with Delete)
    pub fn read_cache(&mut self, tx: &Tx) {
        if self.repo.is_empty() || self.cache_busy {
            return;
        }
        let (t, ni) = (tx.clone(), format!("{}/tools/native-install.py", self.repo));
        std::thread::spawn(move || {
            let out = util::capture(&["python3", "-I", &ni, "cache", "--json"], Duration::from_secs(60)).map(|(_, o)| o).unwrap_or_default();
            t(Box::new(Ev::Cache(serde_json::from_str::<Value>(out.trim()).ok().and_then(|v| v["bytes"].as_u64()))));
        });
    }
    pub fn delete_cache(&mut self, tx: &Tx) {
        if self.repo.is_empty() || self.cache_busy || self.job_running() {
            return;
        }
        self.cache_busy = true;
        self.cache_error.clear();
        let (t, ni) = (tx.clone(), format!("{}/tools/native-install.py", self.repo));
        std::thread::spawn(move || {
            let out = util::capture(&["python3", "-I", &ni, "cache", "--delete", "--json"], Duration::from_secs(300)).map(|(_, o)| o).unwrap_or_default();
            let v = serde_json::from_str::<Value>(out.trim()).unwrap_or(Value::Null);
            t(Box::new(Ev::CacheDeleted(if v["ok"] == true { Ok(v["freed"].as_u64().unwrap_or(0)) } else { Err(v["why"].as_str().unwrap_or("it couldn't be deleted").to_string()) })));
        });
    }
    /// The lock's state, each turn (About's settled): the moment the screen
    /// locks, a ready update restarts the shell (no screen: the lock covers
    /// it, and the new shell takes the lock back)
    pub fn lock_seen(&mut self, h: &Host, tx: &Tx, secure: bool) {
        let go = restart_at_lock(self.was_secure, secure, self.ready(), self.restarting);
        self.was_secure = secure;
        if go {
            self.restart(h, tx, true);
        }
    }
    fn job_changed(&mut self, h: &mut Host, tx: &Tx, was: &str) {
        let now = self.job.as_ref().and_then(|j| j["state"].as_str()).unwrap_or("").to_string();
        // (the restart found an update running elsewhere: it didn't go ahead)
        if self.restarting && now == "busy" && self.job.as_ref().and_then(|j| j["finished"].as_f64()).unwrap_or(0.0) >= self.restart_at - 2.0 {
            self.restarting = false;
        }
        if self.job_running() || self.restarting {
            if !self.t_job.running() {
                self.t_job.every(tx, 500);
            }
        } else {
            self.t_job.stop();
        }
        if was != "running" || now == "running" {
            return;
        }
        let job = self.job.clone().unwrap_or(Value::Null);
        let (title, detail) = job_words(&job);
        match now.as_str() {
            "ready" => {
                self.read_staged(tx);
                // The notice: Restart shell now, or later (the lock, the next login)
                let t = tx.clone();
                let body = detail.clone();
                std::thread::spawn(move || {
                    let c = ["timeout", "86400", "notify-send", "-a", "Tessera", "-i", "system-software-update", "-h", "string:x-tessera-key:tessera-update", "-A", "restart=Restart shell now", "-A", "later=Later", "Tessera is ready to restart", &body];
                    let out = util::capture(&c, Duration::from_secs(86400 + 60)).map(|(_, o)| o).unwrap_or_default();
                    t(Box::new(Ev::ReadyAnswer(out.trim().to_string())));
                });
            }
            "error" => {
                if helpers::island_enabled(h) {
                    helpers::island_post(h, json!({"kind": "sysupdate", "key": "tessera-update", "icon": "warning-circle", "title": title, "subtitle": detail, "color": "#FF453A", "action": "Details", "onAction": {"call": "panels.openSettings", "args": ["update"]}}), 8000);
                } else {
                    helpers::notify_send(&["-a", "Tessera", "-h", "string:x-tessera-key:tessera-update", "-i", "dialog-error", &title, &detail]);
                }
            }
            "done" => helpers::notify_send(&["-a", "Tessera", "-h", "string:x-tessera-key:tessera-update", "-i", "system-software-update", "Glass plugin rebuilt", &detail]),
            _ => {}
        }
    }

    pub fn start(&mut self, tx: &Tx) {
        // update-source.py: where the installation came from
        let (t, shell) = (tx.clone(), util::root().join("shell").to_string_lossy().into_owned());
        std::thread::spawn(move || {
            let out = util::capture(&["python3", "-I", &format!("{shell}/assets/scripts/update-source.py"), &shell], Duration::from_secs(20)).map(|(_, o)| o).unwrap_or_default();
            t(Box::new(Ev::Source(out.trim().to_string())));
        });
        // After the restart: what the last install did, for Software Update
        let t = tx.clone();
        std::thread::spawn(move || {
            let run = std::env::var("XDG_RUNTIME_DIR").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "/tmp".into());
            let mut found = None;
            for name in ["tessera-update-result.json"] {
                let p = format!("{run}/{name}");
                if let Ok(s) = std::fs::read_to_string(&p) {
                    let v = serde_json::from_str::<Value>(&s).ok();
                    let _ = std::fs::remove_file(&p);
                    found = Some(v.unwrap_or(Value::Null));
                    break;
                }
            }
            t(Box::new(Ev::Result(found)));
        });
        // The background job as it was left (still running, ready, or cut
        // off by a reboot: resumed), and the note of a build gone back from
        self.read_job(tx);
        let t = tx.clone();
        std::thread::spawn(move || {
            let p = format!("{}/tessera/rolled-back.json", state_home());
            if let Some(v) = std::fs::read_to_string(&p).ok().and_then(|s| serde_json::from_str::<Value>(&s).ok()) {
                let _ = std::fs::remove_file(&p);
                t(Box::new(Ev::RolledBack(v)));
            }
        });
    }

    pub fn check(&mut self, tx: &Tx) {
        if !self.source_ready {
            self.pending_check = true;
            return;
        }
        if self.repo.is_empty() {
            self.error = "The installation source is missing. Reinstall Tessera from its checkout.".into();
            return;
        }
        if self.checking {
            return;
        }
        self.checking = true;
        self.error.clear();
        // (a new check: the last job's outcome gives way; one running stays)
        if !self.job_running() {
            self.job = None;
        }
        let (t, helper, cfg, ch) = (tx.clone(), self.helper(), config_dir(), channel_args(&self.channel));
        std::thread::spawn(move || {
            let mut c = vec!["python3", "-I", &helper, "check", "--config", &cfg];
            c.extend(ch.iter().map(String::as_str));
            let out = util::capture(&c, Duration::from_secs(300)).map(|(_, o)| o).unwrap_or_default();
            t(Box::new(Ev::Report(out)));
        });
    }

    /// tools/tessera-packages.sh: what's missing from tools/packages.sh (of the
    /// version about to be installed) and what Tessera dropped that's still installed
    pub fn check_packages(&mut self, tx: &Tx, reference: &str) {
        if self.repo.is_empty() || self.packages_checking {
            return;
        }
        self.packages_checking = true;
        let (t, script, r) = (tx.clone(), format!("{}/tools/tessera-packages.sh", self.repo), reference.to_string());
        std::thread::spawn(move || {
            let mut c = vec![script.as_str(), "status"];
            if !r.is_empty() {
                c.push(&r);
            }
            let out = util::capture(&c, Duration::from_secs(120)).map(|(_, o)| o).unwrap_or_default();
            t(Box::new(Ev::Packages(out)));
        });
    }

    /// The menu icon (as the menu bar draws it): its glyph and font, for the update screen
    fn look(&self, h: &Host) -> Value {
        let c = h.cfg()["appearance"]["menuIcon"].as_str().unwrap_or("tessera");
        let name = match c {
            "tessera" | "circle" => "circle-wavy",
            v if v == tessera_core::old_names::SETTING_VALUE => "circle-wavy",
            "linux" | "distro" => "linux-logo",
            "command" => "command",
            "terminal" => "terminal-window",
            "cpu" => "cpu",
            "atom" => "atom",
            "planet" => "planet",
            "rocket" => "rocket-launch",
            "star" => "star-four",
            "lightning" => "lightning",
            "flame" => "flame",
            "leaf" => "leaf",
            "snowflake" => "snowflake",
            "moon" => "moon-stars",
            "cat" => "cat",
            "ghost" => "ghost",
            "alien" => "alien",
            _ => "circle-wavy",
        };
        let logo = crate::bus::field("systeminfo", "osLogo");
        let image = match logo.as_str() {
            Some(l) if c == "distro" && !l.is_empty() => tessera_core::icon_theme::find(l, None).map(|p| p.to_string_lossy().into_owned()).unwrap_or_default(),
            _ => String::new(),
        };
        json!({"glyph": tessera_core::icons::glyph_or(name, "circle-wavy"), "image": image})
    }

    pub fn tick(&mut self, h: &mut Host, tx: &Tx, name: &str) {
        match name {
            "tuPeek" => {
                self.t_peek.once(tx, PEEK_EVERY_MS);
                self.peek(h, tx);
            }
            // (a restart that should have ended this shell a minute ago)
            "tuStart" => {
                if self.restarting {
                    self.restarting = false;
                    self.error = "The shell didn't restart. Try Restart shell now again; your update is still ready.".into();
                    self.read_staged(tx);
                }
            }
            "tuJob" => self.read_job(tx),
            // Updated: an alert once the island is up, its button opens the notes
            "tuAnnounce" => {
                let cl = self.result.as_ref().and_then(|r| r["changelog"].as_array()).cloned().unwrap_or_default();
                let n = cl.len();
                let title = "Tessera updated";
                let sub = if n > 0 {
                    format!("{}{}", cl[0]["title"].as_str().unwrap_or(""), if n > 1 { format!(" and {} more", n - 1) } else { String::new() })
                } else {
                    "Your settings and shortcuts are as you left them".into()
                };
                if helpers::island_enabled(h) {
                    helpers::island_post(h, json!({"kind": "sysupdate", "key": "tessera-update", "icon": "check-circle", "title": title, "subtitle": sub, "color": "#30D158", "action": "What's new", "onAction": {"call": "panels.openSettings", "args": ["update"]}}), 10000);
                } else {
                    helpers::notify_send(&["-a", "Tessera", "-h", "string:x-tessera-key:tessera-update", "-i", "system-software-update", title, &format!("{sub}. Settings → Software Update shows what's new.")]);
                }
            }
            _ => {}
        }
        self.publish();
    }

    /// An event of its threads. Returns true when a report arrived (SoftwareUpdate reads compat)
    pub fn event(&mut self, h: &mut Host, tx: &Tx, ev: Ev) -> bool {
        let mut report_changed = false;
        match ev {
            Ev::Job(j) => {
                let was = self.job.as_ref().and_then(|j| j["state"].as_str()).unwrap_or("").to_string();
                // (a job just asked for: the last job's file, read before the
                // new one wrote its own, isn't this one's news)
                let asked = self.job.as_ref().filter(|x| x["state"] == "running" && x["pid"].is_null()).and_then(|x| x["started"].as_f64()).unwrap_or(0.0);
                let j = j.filter(|j| asked <= 0.0 || j["started"].as_f64().unwrap_or(0.0) >= asked - 2.0);
                if let Some(j) = j {
                    // (cut off by a reboot, a logout, a crash: started again
                    // from what it was asked, in the background)
                    if job_cut_off(&j, pid_alive) && !self.repo.is_empty() && self.allowed() {
                        let helper = self.helper();
                        tessera_core::services::bridge::run(&["setsid", "-f", "python3", "-I", &helper, "resume", "--config", &config_dir()]);
                        self.job = Some(json!({"state": "running", "phase": "prepare", "percent": 0, "text": "Picking up where the update stopped", "started": helpers::now_s()}));
                    } else if !(was == "running" && j["state"] == "running" && j["pid"].is_null()) {
                        self.job = Some(j);
                    }
                }
                self.job_changed(h, tx, &was);
            }
            Ev::Staged(s) => {
                self.staged = s;
            }
            Ev::ReadyAnswer(a) => {
                if a == "restart" {
                    self.restart(h, tx, false);
                }
            }
            Ev::Cache(c) => self.cache = c,
            Ev::CacheDeleted(r) => {
                self.cache_busy = false;
                match r {
                    Ok(_) => self.cache = Some(0),
                    Err(e) => self.cache_error = format!("The build cache couldn't be deleted: {e}. Try again once nothing is building."),
                }
                self.read_cache(tx);
            }
            Ev::RolledBack(v) => {
                let back = v["to"].as_str().unwrap_or("").split(" (").next().unwrap_or("").to_string();
                let body = format!("Back on {}. That version isn't offered again; a newer one is when it's out.", if back.is_empty() { "the version you had" } else { &back });
                helpers::notify_send(&["-a", "Tessera", "-h", "string:x-tessera-key:tessera-update", "-i", "dialog-warning", "The new version didn't start", &body]);
            }
            Ev::Source(s) => {
                if self.repo.is_empty() {
                    self.repo = s;
                }
                self.source_ready = true;
                self.read_info(tx);
                self.read_staged(tx);
                self.read_job(tx);
                self.t_peek.once(tx, PEEK_FIRST_MS);
                if self.pending_check {
                    self.pending_check = false;
                    self.check(tx);
                }
            }
            Ev::Report(text) => {
                let Ok(r) = serde_json::from_str::<Value>(text.trim()) else {
                    self.checking = false;
                    self.error = "The update check didn't answer properly. Try again later.".into();
                    return true;
                };
                self.error = r["error"].as_str().unwrap_or("").to_string();
                // Keep decisions already made for conflicts that are still there
                let mut kept = Map::new();
                for c in r["conflicts"].as_array().cloned().unwrap_or_default() {
                    if let Some(id) = c["id"].as_str() {
                        if let Some(v) = self.choices.get(id) {
                            kept.insert(id.to_string(), v.clone());
                        }
                    }
                }
                self.choices = kept;
                // The packages of the version that would be installed
                let upstream = if r["available"] == true { r["upstream"].as_str().unwrap_or("").to_string() } else { String::new() };
                self.report = Some(r);
                self.read_info(tx); // (the channel's branch may be new here)
                self.check_packages(tx, &upstream);
                self.checking = false; // (last: the package check is already under way)
                report_changed = true;
            }
            Ev::Info(text) => {
                if let Ok(v) = serde_json::from_str::<Value>(text.trim()) {
                    self.info = Some(v);
                }
            }
            Ev::Peek(text) => {
                self.peeking = false;
                let r = serde_json::from_str::<Value>(text.trim()).unwrap_or(Value::Null);
                if let Some(body) = notice(&r).filter(|_| !self.job_running()) {
                    // (waits for the answer: the notice can sit in Notification Center)
                    let t = tx.clone();
                    std::thread::spawn(move || {
                        let c = ["timeout", "86400", "notify-send", "-a", "Tessera", "-i", "system-software-update", "-A", "update=Update", "-A", "later=Later", "Update available", &body];
                        let out = util::capture(&c, Duration::from_secs(86400 + 60)).map(|(_, o)| o).unwrap_or_default();
                        t(Box::new(Ev::Answer(out.trim().to_string())));
                    });
                }
            }
            Ev::Answer(a) => {
                if a == "update" {
                    self.open_asked = true;
                }
            }
            Ev::Packages(text) => {
                self.packages_checking = false;
                if let Ok(v) = serde_json::from_str::<Value>(text.trim()) {
                    self.packages = v;
                }
            }
            Ev::Result(r) => {
                if let Some(r) = r {
                    if let Some(e) = r["error"].as_str().filter(|e| !e.is_empty()) {
                        self.error = e.to_string();
                    } else if helpers::truthy(&r["applied"]) || helpers::truthy(&r["rebuilt"]) {
                        let applied = helpers::truthy(&r["applied"]);
                        self.result = Some(r);
                        if applied {
                            self.t_announce.once(tx, 4000);
                        }
                    }
                }
            }
        }
        self.publish();
        report_changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn undecided_counts_conflicts_without_a_valid_choice() {
        let mut t = TesseraUpdate::default();
        t.report = Some(json!({"available": true, "conflicts": [{"id": "a", "choices": ["keep", "take"]}, {"id": "b", "choices": ["x"]}]}));
        assert_eq!(t.undecided(), 2);
        t.choose("a", "keep");
        assert_eq!(t.undecided(), 1);
        t.choose("b", "nope");
        assert_eq!(t.undecided(), 1);
        assert!(t.available());
    }

    #[test]
    fn every_state_has_its_words() {
        let w = |j: Value| job_words(&j).0;
        assert_eq!(w(json!({"state": "running", "phase": "prepare"})), "Preparing the update");
        assert_eq!(w(json!({"state": "running", "phase": "download", "how": "patch", "bytes": 7996232})), "Downloading a patch · 8.0 MB");
        assert_eq!(w(json!({"state": "running", "phase": "download", "how": "full", "bytes": 29949608})), "Downloading · 29.9 MB");
        assert_eq!(w(json!({"state": "running", "phase": "build"})), "Building Tessera");
        assert_eq!(w(json!({"state": "running", "phase": "parts", "text": "Building the glass plugin"})), "Building the glass plugin");
        assert_eq!(w(json!({"state": "ready", "latest": "abc12345"})), "Ready to restart");
        assert!(job_words(&json!({"state": "ready", "latest": "abc12345"})).1.contains("next time you lock the screen or log in"));
        let (t, d) = job_words(&json!({"state": "error", "error": "Couldn't reach the update server."}));
        assert_eq!(t, "The update didn't finish");
        assert!(d.starts_with("Couldn't reach the update server.") && d.contains("Try again"));
        assert_eq!(w(json!({"state": "cancelled"})), "Update cancelled");
        assert_eq!(w(json!({"state": "none"})), "Tessera is up to date");
        // (an update running elsewhere: never "another update", always since when and where)
        let (t, d) = job_words(&json!({"state": "busy", "error": "Software Update is already running (started 21:58) in another session. Wait for it to finish, then try again."}));
        assert_eq!(t, "Software Update is already running");
        assert!(d.contains("started 21:58") && !d.contains("Another update"));
    }

    #[test]
    fn restart_only_as_the_lock_engages_with_an_update_ready() {
        assert!(restart_at_lock(false, true, true, false));
        assert!(!restart_at_lock(true, true, true, false), "already locked: not again each turn");
        assert!(!restart_at_lock(false, true, false, false), "nothing ready");
        assert!(!restart_at_lock(false, true, true, true), "a restart under way");
        assert!(!restart_at_lock(true, false, true, false), "unlocking isn't locking");
    }

    #[test]
    fn a_job_cut_off_is_one_that_says_running_with_no_process() {
        assert!(job_cut_off(&json!({"state": "running", "pid": 4242}), |_| false));
        assert!(!job_cut_off(&json!({"state": "running", "pid": 4242}), |_| true));
        assert!(job_cut_off(&json!({"state": "running"}), |_| true));
        assert!(!job_cut_off(&json!({"state": "ready", "pid": 4242}), |_| false));
    }

    #[test]
    fn the_channel_goes_to_the_helper_once_chosen() {
        assert_eq!(channel_args("beta"), vec!["--channel".to_string(), "beta".into()]);
        assert_eq!(channel_args("stable"), vec!["--channel".to_string(), "stable".into()]);
        assert!(channel_args("").is_empty() && channel_args("nightly").is_empty());
    }

    #[test]
    fn the_notice_only_when_the_helper_says_so() {
        assert_eq!(notice(&json!({"notify": true, "behind": 3, "channelName": "Beta"})).as_deref(), Some("Tessera Beta has 3 new changes. Nothing installs until you choose Update."));
        assert_eq!(notice(&json!({"notify": true, "behind": 1, "channelName": "Stable"})).as_deref(), Some("Tessera Stable has 1 new change. Nothing installs until you choose Update."));
        assert!(notice(&json!({"notify": false, "behind": 3})).is_none());
        assert!(notice(&Value::Null).is_none());
    }
}

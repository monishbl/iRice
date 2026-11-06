//! shell/services/TesseraUpdate.qml: Tessera's own updates (not the system's:
//! updates.rs), from the git repository the shell runs from, through
//! tools/tessera-update.py. Checked only when asked (Software Update ->
//! Check for updates). The report lists what's new and every conflict (your
//! code edits, settings whose defaults change, the material you use,
//! shortcut clashes); nothing is applied until each one is decided.
//!
//! Installing hands over to assets/scripts/update-run.sh, started detached
//! from a copy (with the update screen's): it stops this shell, updates
//! behind a full-screen "Tessera is updating" screen, rebuilds, and starts
//! the new shell once.
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
}

pub struct TesseraUpdate {
    pub repo: String,
    source_ready: bool,
    pending_check: bool,
    pub report: Option<Value>,
    pub checking: bool,
    pub applying: bool,
    pub error: String,
    pub result: Option<Value>,
    pub choices: Map<String, Value>,
    pub packages: Value, // { missing, unavailable, dropped, reasons }
    pub packages_checking: bool,
    t_start: Ticker,
    t_announce: Ticker,
}

impl Default for TesseraUpdate {
    fn default() -> Self {
        Self {
            repo: std::env::var("TESSERA_UPDATE_REPO").or_else(|_| std::env::var("IRICE_UPDATE_REPO")).unwrap_or_default(),
            source_ready: false,
            pending_check: false,
            report: None,
            checking: false,
            applying: false,
            error: String::new(),
            result: None,
            choices: Map::new(),
            packages: json!({"missing": [], "unavailable": [], "dropped": []}),
            packages_checking: false,
            t_start: Ticker::new("tuStart"),
            t_announce: Ticker::new("tuAnnounce"),
        }
    }
}

fn config_dir() -> String {
    tessera_core::theme::config_path().parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()
}

impl TesseraUpdate {
    pub fn helper(&self) -> String {
        format!("{}/tools/tessera-update.py", self.repo)
    }
    /// Never from the dev sandbox on its own copy: it would update the developer's working tree
    pub fn allowed(&self) -> bool {
        !helpers::env_is("SANDBOX") || std::env::var("TESSERA_UPDATE_REPO").or_else(|_| std::env::var("IRICE_UPDATE_REPO")).map(|v| !v.is_empty()).unwrap_or(false)
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
        crate::bus::put("tesseraupdate", json!({"checking": self.checking, "applying": self.applying, "available": self.available(), "error": self.error, "undecided": self.undecided()}));
    }

    pub fn start(&mut self, tx: &Tx) {
        // update-source.py: where the installation came from
        let (t, shell) = (tx.clone(), util::root().join("shell").to_string_lossy().into_owned());
        std::thread::spawn(move || {
            let out = util::capture(&["python3", "-I", &format!("{shell}/assets/scripts/update-source.py"), &shell], Duration::from_secs(20)).map(|(_, o)| o).unwrap_or_default();
            t(Box::new(Ev::Source(out.trim().to_string())));
        });
        // After the restart: what the last install did, for Software Update
        // (the update that renamed iRice to Tessera ran iRice's script, which
        // left its result under the old name)
        let t = tx.clone();
        std::thread::spawn(move || {
            let run = std::env::var("XDG_RUNTIME_DIR").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "/tmp".into());
            let mut found = None;
            for name in ["tessera-update-result.json", "irice-update-result.json"] {
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
        if self.checking || self.applying {
            return;
        }
        self.checking = true;
        self.error.clear();
        let (t, helper, cfg) = (tx.clone(), self.helper(), config_dir());
        std::thread::spawn(move || {
            let out = util::capture(&["python3", "-I", &helper, "check", "--config", &cfg], Duration::from_secs(300)).map(|(_, o)| o).unwrap_or_default();
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
            "tessera" | "irice" | "circle" => "circle-wavy",
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

    /// Started by SoftwareUpdate, after the system part. rebuild: parts to
    /// rebuild anyway ("plugin" after a Hyprland update); with no Tessera
    /// update, only those are rebuilt (same screen, same restart).
    pub fn launch(&mut self, h: &Host, tx: &Tx, rebuild: &[String]) {
        if !self.allowed() || !self.source_ready || self.repo.is_empty() {
            return;
        }
        self.applying = true;
        self.error.clear();
        self.result = None;
        let script = "t=$(mktemp -d \"${XDG_RUNTIME_DIR:-/tmp}/tessera-update.XXXXXX\") || exit 1; \
            cp -r \"$1/shell/assets/updater\" \"$t/ui\" && cp \"$1/shell/assets/scripts/update-run.sh\" \"$t/run.sh\" && printf '%s' \"$2\" > \"$t/choices.json\" && \
            printf '%s' \"$4\" > \"$t/look.json\" && printf '%s' \"$5\" > \"$t/rebuild\" && cp \"$1/shell/assets/fonts/Phosphor-Fill.ttf\" \"$t/ui/\" 2>/dev/null; \
            exec setsid -f bash \"$t/run.sh\" \"$t\" \"$1\" \"$3\" </dev/null >/dev/null 2>&1";
        let args = [
            "sh".to_string(),
            "-c".into(),
            script.into(),
            "sh".into(),
            self.repo.clone(),
            Value::Object(self.choices.clone()).to_string(),
            config_dir(),
            self.look(h).to_string(),
            rebuild.join(","),
        ];
        let c: Vec<&str> = args.iter().map(String::as_str).collect();
        tessera_core::services::bridge::run(&c);
        self.t_start.once(tx, 20000);
    }

    pub fn tick(&mut self, h: &mut Host, name: &str) {
        match name {
            // If the update screen never took over (the helper couldn't start)
            "tuStart" => {
                if self.applying {
                    self.applying = false;
                    self.error = "The update didn't start. Try again, or run tools/tessera-update.py in the Tessera folder.".into();
                }
            }
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
                    helpers::island_post(h, json!({"kind": "sysupdate", "icon": "check-circle", "title": title, "subtitle": sub, "color": "#30D158", "action": "What's new", "onAction": {"call": "panels.openSettings", "args": ["update"]}}), 10000);
                } else {
                    helpers::notify_send(&["-a", "Tessera", "-i", "system-software-update", title, &format!("{sub}. Settings → Software Update shows what's new.")]);
                }
            }
            _ => {}
        }
        self.publish();
    }

    /// An event of its threads. Returns true when a report arrived (SoftwareUpdate reads compat)
    pub fn event(&mut self, tx: &Tx, ev: Ev) -> bool {
        let mut report_changed = false;
        match ev {
            Ev::Source(s) => {
                if self.repo.is_empty() {
                    self.repo = s;
                }
                self.source_ready = true;
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
                self.check_packages(tx, &upstream);
                self.checking = false; // (last: the package check is already under way)
                report_changed = true;
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
}

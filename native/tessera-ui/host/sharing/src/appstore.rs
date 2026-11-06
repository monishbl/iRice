//! The App Store (shell/services/AppStore.qml, and what modules/appstore/
//! AppStoreFeed.qml tells the App Store surface; Spotlight's "Get" rows):
//! apps from the Arch repos, the AUR and Flathub, found by assets/scripts/
//! appstore.py (kept running while the store or Spotlight wants it, two
//! minutes after). Installing and removing: packages (repos, AUR) through
//! Software Update (updates.rs), with the password and progress in the
//! island; Flathub apps with flatpak, for this user only (no password), after
//! installing flatpak itself if needed. An AUR package is a community one:
//! the store says so first.
//!
//! The surface is told (`{"to":"appstore", ...}`):
//!   state   { open, catalogue, updates, jobs, app }
//!   answer  { token, apps, sections, error }   a place's apps
//!   changed { key, installed }                 installed or removed
//!   show    app                                Spotlight's "Get" while it's open
//! and asks (`shell::ask("appstore", [what, ...])`): load, close, install,
//! open, remove, catalogue, website, copy.
//! Calls for the other hosts: appstore.panel [open] / appstore.app [app] (the
//! shell's Panels.appStoreOpen / appStoreApp changed), appstore.lookUp [q] /
//! appstore.forget (Spotlight; the answer is the shell state appstore.quick),
//! appstore.users [+1 | -1] (a window showing the store).

use crate::localsend::js;
use crate::proc::{json_line, Keep};
use crate::{act, Job, Sharing, GREEN};
use serde_json::{json, Value};
use std::collections::HashMap;
use tessera_core::host::Host;

/// What a pending question to the backend does with its answer
#[derive(Clone, Debug)]
pub enum Pending {
    Load(Value),
    Status,
    Quick(String),
    Refresh,
}

pub struct Store {
    pub wanted: bool,
    pub users: i32,
    pub last_id: i64,
    pub waiting: HashMap<i64, Pending>,
    pub queued: Vec<String>,
    pub backend: Keep,
    /// What the backend can reach: { catalogue, flatpak, helper }
    pub status: Value,
    pub quick_query: String,
    pub quick: Vec<Value>,
    pub quick_loading: bool,
    pub quick_revision: i64,
    /// key -> { state: installing | removing, error }
    pub jobs: HashMap<String, Value>,
    /// the package job Software Update is running for us
    pub pending_key: String,
    pub pending_app: Option<Value>,
    /// { app, op } waiting for flatpak to be installed
    pub flatpak_queue: Option<(Value, String)>,
    pub flatpak_running: bool,
    pub panel_open: bool,
    pub panel_app: Option<Value>,
    pub counted: bool,
    pub last_feed: Value,
}

impl Default for Store {
    fn default() -> Self {
        Self {
            wanted: false,
            users: 0,
            last_id: 0,
            waiting: HashMap::new(),
            queued: vec![],
            backend: Keep::new("appstore", "store_rest"),
            status: json!({}),
            quick_query: String::new(),
            quick: vec![],
            quick_loading: false,
            quick_revision: 0,
            jobs: HashMap::new(),
            pending_key: String::new(),
            pending_app: None,
            flatpak_queue: None,
            flatpak_running: false,
            panel_open: false,
            panel_app: None,
            counted: false,
            last_feed: Value::Null,
        }
    }
}

/// A logo's file (iconFor gives a file URL, or a theme's), or Flathub's https
/// URL (tessera-ui fetches and keeps it, as Qt's Image would)
pub fn file(u: &str) -> String {
    if let Some(rest) = u.strip_prefix("file://") {
        tessera_core::services::mpris::percent_decode(rest)
    } else if u.starts_with('/') || u.starts_with("https://") {
        u.to_string()
    } else {
        String::new()
    }
}

pub fn source_name(app: &Value) -> &'static str {
    match app["source"].as_str().unwrap_or("") {
        "repo" => "Arch repos",
        "aur" => "AUR \u{b7} community",
        "flathub" => "Flathub",
        _ => "",
    }
}

/// An app's logo: the icon theme's (matching the other apps), else its own
/// (the catalogue's, or Flathub's), else "" (a tile with its initial)
pub fn icon_for(app: &Value) -> String {
    if app.is_null() {
        return String::new();
    }
    // (not by an AUR package's name: themes fall back on dashes, so every
    // "spotify-something" would get Spotify's)
    let names: Vec<String> = if app["source"] == "aur" { vec![js(&app["iconName"])] } else { vec![js(&app["iconName"]), js(&app["pkg"])] };
    for n in names.into_iter().filter(|n| !n.is_empty() && !n.contains('/')) {
        let last = n.rsplit('.').next().unwrap_or(&n).to_string();
        for v in [n.clone(), n.to_lowercase(), last.clone(), last.to_lowercase(), n.split_whitespace().collect::<Vec<_>>().join("-").to_lowercase()] {
            if let Some(p) = tessera_core::icon_theme::find(&v, None) {
                return format!("file://{}", p.to_string_lossy());
            }
        }
    }
    let own = js(&app["icon"]);
    if own.starts_with('/') {
        format!("file://{own}")
    } else {
        own
    }
}

pub fn enrich(a: &Value) -> Value {
    if a.is_null() {
        return Value::Null;
    }
    let mut o = a.clone();
    o["_icon"] = json!(file(&icon_for(a)));
    o["_source"] = json!(source_name(a));
    o
}

impl Sharing {
    pub fn store_available(&self) -> bool {
        tessera_core::util::on_path("pacman") || tessera_core::util::on_path("flatpak")
    }
    fn store_want(&mut self) {
        self.store.wanted = true;
        self.sched.set("store_idle", 120_000);
    }
    fn store_ask(&mut self, op: &str, args: Value, pending: Pending) -> i64 {
        self.store_want();
        self.store.last_id += 1;
        let id = self.store.last_id;
        self.store.waiting.insert(id, pending);
        let mut line = json!({"id": id, "op": op});
        if let Some(a) = args.as_object() {
            for (k, v) in a {
                line[k] = v.clone();
            }
        }
        let line = format!("{line}\n");
        if self.store.backend.kept.running {
            self.store.backend.kept.write(&line);
        } else {
            self.store.queued.push(line);
        }
        id
    }
    pub fn store_users(&mut self, delta: i32) {
        self.store.users = (self.store.users + delta).max(0);
        if self.store.users > 0 {
            self.store_want();
        } else {
            self.sched.set("store_idle", 120_000);
        }
    }
    pub fn store_idle(&mut self) {
        if self.store.users == 0 {
            self.store.wanted = false;
        }
    }
    pub fn store_refresh_status(&mut self) {
        self.store_ask("status", json!({}), Pending::Status);
    }

    /// The backend follows `wanted`; the window counts itself
    pub fn store_sync(&mut self, h: &mut Host) {
        // the shell's own word on the window (Panels.appStoreOpen), when it says
        if let Some(b) = h.state("appStoreOpen").as_bool() {
            self.store.panel_open = b;
        }
        let open = self.store.panel_open;
        if open && !self.store.counted {
            self.store.counted = true;
            self.store_users(1);
            self.store_refresh_status();
        } else if !open && self.store.counted {
            self.store.counted = false;
            self.store_users(-1);
        }
        let cmd = vec!["python3".to_string(), "-I".into(), self.script("appstore.py"), "--serve".into()];
        let wanted = self.store.wanted;
        self.store.backend.sync(&mut self.sched, h.sender("Sharing"), wanted, &cmd, 10_000, true);
        if self.store.backend.kept.running && !self.store.queued.is_empty() {
            for l in std::mem::take(&mut self.store.queued) {
                self.store.backend.kept.write(&l);
            }
        }
    }
    pub fn store_exit(&mut self, gen: u64) {
        let wanted = self.store.wanted;
        self.store.backend.ended(&mut self.sched, gen, wanted, 10_000);
    }
    pub fn store_line(&mut self, h: &mut Host, gen: u64, line: &str) {
        if !self.store.backend.is(gen) {
            return;
        }
        let Some(r) = json_line(line) else { return };
        let id = r["id"].as_i64().unwrap_or(-1);
        let partial = r["partial"].as_bool().unwrap_or(false);
        let cb = if partial { self.store.waiting.get(&id).cloned() } else { self.store.waiting.remove(&id) };
        let Some(cb) = cb else { return };
        match cb {
            Pending::Status => self.store.status = if r["status"].is_null() { json!({}) } else { r["status"].clone() },
            Pending::Refresh => {}
            Pending::Load(token) => {
                let apps: Vec<Value> = r["apps"].as_array().map(|a| a.iter().map(enrich).collect()).unwrap_or_default();
                let sections: Vec<Value> = r["sections"].as_array().map(|a| a.iter().map(|s| json!({"title": js(&s["title"]), "apps": s["apps"].as_array().map(|x| x.iter().map(enrich).collect::<Vec<_>>()).unwrap_or_default()})).collect()).unwrap_or_default();
                h.tell("appstore", json!({"answer": {"token": token, "error": js(&r["error"]), "apps": apps, "sections": sections}}));
            }
            Pending::Quick(q) => {
                if q != self.store.quick_query || r["stale"].as_bool().unwrap_or(false) {
                    return;
                }
                self.store.quick = r["apps"].as_array().map(|a| a.iter().filter(|x| !x["installed"].as_bool().unwrap_or(false)).take(3).cloned().collect()).unwrap_or_default();
                if !partial {
                    self.store.quick_loading = false;
                }
                self.store.quick_revision += 1;
            }
        }
    }

    // ── Spotlight: a few installable matches for what's typed ───────────
    // Asked only when nothing installed matches (Search decides), once typing
    // pauses; the repos' matches come at once (they're on this computer),
    // Flathub's after; searches typed past are dropped unanswered.
    pub fn store_look_up(&mut self, q: &str) {
        if q == self.store.quick_query {
            return;
        }
        self.store.quick_query = q.into();
        self.store.quick.clear();
        self.store.quick_loading = true;
        self.store.quick_revision += 1;
        self.sched.set("store_quick", 350);
    }
    /// Nothing to look up (an installed app matches): stop showing it
    pub fn store_forget(&mut self) {
        if self.store.quick_query.is_empty() && !self.store.quick_loading {
            return;
        }
        self.sched.cancel("store_quick");
        self.store.quick_query.clear();
        self.store.quick.clear();
        self.store.quick_loading = false;
    }
    pub fn store_quick_due(&mut self) {
        let q = self.store.quick_query.clone();
        self.store_ask("search", json!({"q": q, "limit": 4, "stream": true}), Pending::Quick(q));
    }

    // ── Installing, opening, removing ───────────────────────────────────
    pub fn store_busy_with(&self, key: &str) -> String {
        self.store.jobs.get(key).map(|j| js(&j["state"])).unwrap_or_default()
    }
    fn set_job(&mut self, key: &str, job: Option<Value>) {
        match job {
            Some(j) => {
                self.store.jobs.insert(key.to_string(), j);
            }
            None => {
                self.store.jobs.remove(key);
            }
        }
    }
    fn busy_error() -> Value {
        json!({"state": "", "error": "Software Update is busy. Try again when it's done."})
    }

    pub fn store_install(&mut self, h: &mut Host, app: &Value) {
        let key = js(&app["key"]);
        if key.is_empty() || !self.store_busy_with(&key).is_empty() {
            return;
        }
        if app["source"] == "flathub" {
            return self.flatpak_job(h, app, "install");
        }
        if self.upd.running() {
            self.set_job(&key, Some(Self::busy_error()));
            return;
        }
        self.set_job(&key, Some(json!({"state": "installing"})));
        self.store.pending_key = key;
        self.store.pending_app = Some(app.clone());
        let pkg = js(&app["pkg"]);
        if app["source"] == "aur" {
            self.update_run("store", &["--aur-install", &pkg]);
        } else {
            self.update_run("store", &["--install", &pkg]);
        }
    }
    pub fn store_remove(&mut self, h: &mut Host, app: &Value) {
        let key = js(&app["key"]);
        if key.is_empty() || !self.store_busy_with(&key).is_empty() {
            return;
        }
        if app["source"] == "flathub" {
            return self.flatpak_job(h, app, "remove");
        }
        if self.upd.running() {
            self.set_job(&key, Some(Self::busy_error()));
            return;
        }
        self.set_job(&key, Some(json!({"state": "removing"})));
        self.store.pending_key = key;
        self.store.pending_app = Some(app.clone());
        let pkg = js(&app["pkg"]);
        self.update_run("store", &["--remove", &pkg]);
    }
    /// The app catalogue (logos, categories for the repos' apps)
    pub fn store_install_catalogue(&mut self) {
        if !self.upd.running() {
            self.update_run("catalogue", &["--install", "archlinux-appstream-data"]);
        }
    }
    /// Updates.finished, for a package job of ours
    pub fn store_update_finished(&mut self, h: &mut Host, j: &Value) {
        let key = std::mem::take(&mut self.store.pending_key);
        if key.is_empty() {
            return;
        }
        let app = self.store.pending_app.take();
        let was = self.store_busy_with(&key);
        if j["state"] == "done" {
            self.set_job(&key, None);
            self.store_done(h, app, &was);
        } else {
            let err = js(&j["error"]);
            let text = js(&j["text"]);
            let msg = if !err.is_empty() { err } else if !text.is_empty() { text } else if was == "removing" { "It couldn't be removed.".into() } else { "It couldn't be installed.".into() };
            self.set_job(&key, Some(json!({"state": "", "error": msg})));
        }
    }

    fn store_done(&mut self, h: &mut Host, app: Option<Value>, was: &str) {
        self.store_ask("refresh", json!({}), Pending::Refresh);
        let key = app.as_ref().map(|a| js(&a["key"])).unwrap_or_default();
        h.tell("appstore", json!({"changed": {"key": key, "installed": was == "installing"}}));
        let Some(app) = app else { return };
        let name = js(&app["name"]);
        if was == "installing" {
            self.island(h, json!({"kind": "custom", "icon": "check-circle", "title": format!("{name} is installed"), "subtitle": source_name(&app), "color": GREEN, "action": "Open", "onAction": act("appstore.openApp", json!([app]))}), 8000);
        } else {
            self.island(h, json!({"kind": "custom", "icon": "trash", "title": format!("{name} is removed"), "subtitle": source_name(&app)}), 5000);
        }
    }

    // Flathub: per user, from Flathub (added once), flatpak itself first
    fn flatpak_job(&mut self, h: &mut Host, app: &Value, op: &str) {
        let key = js(&app["key"]);
        if !tessera_core::util::on_path("flatpak") {
            if self.upd.running() {
                self.set_job(&key, Some(Self::busy_error()));
                return;
            }
            self.set_job(&key, Some(json!({"state": "installing"})));
            self.store.flatpak_queue = Some((app.clone(), op.into()));
            self.update_run("flatpak", &["--install", "flatpak"]);
            return;
        }
        self.set_job(&key, Some(json!({"state": if op == "install" { "installing" } else { "removing" }})));
        let pkg = js(&app["pkg"]);
        let cmd = if op == "install" {
            crate::proc::s(&["sh", "-c", "flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo && flatpak install --user -y --noninteractive flathub \"$1\"", "sh", &pkg])
        } else {
            crate::proc::s(&["flatpak", "uninstall", "--user", "-y", "--noninteractive", &pkg])
        };
        self.store.flatpak_running = true;
        self.run(h, Job::Flatpak { app: app.clone(), op: op.into() }, &cmd, None);
    }
    /// Updates.finished, when flatpak itself was being installed for a job
    pub fn store_flatpak_installed(&mut self, h: &mut Host, j: &Value) {
        let Some((app, op)) = self.store.flatpak_queue.take() else { return };
        let key = js(&app["key"]);
        self.set_job(&key, None);
        if j["state"] == "done" {
            self.flatpak_job(h, &app, &op);
        } else {
            self.set_job(&key, Some(json!({"state": "", "error": format!("Flatpak couldn't be installed. {}", js(&j["error"]))})));
        }
    }
    pub fn flatpak_done(&mut self, h: &mut Host, app: Value, op: &str, stderr: &str, code: i32) {
        self.store.flatpak_running = false;
        let key = js(&app["key"]);
        let was = if op == "install" { "installing" } else { "removing" };
        if code == 0 {
            self.set_job(&key, None);
            self.store_done(h, Some(app), was);
        } else {
            let said = stderr.trim().lines().filter(|l| !l.trim().is_empty()).last().unwrap_or("").to_string();
            let said = said.strip_prefix("error:").or_else(|| said.strip_prefix("Error:")).unwrap_or(&said).trim().to_string();
            self.set_job(&key, Some(json!({"state": "", "error": format!("{}{said}", if was == "installing" { "It couldn't be installed. " } else { "It couldn't be removed. " })})));
        }
    }

    pub fn store_open_app(&mut self, app: &Value) {
        if app.is_null() {
            return;
        }
        let id = js(&app["desktop"]).trim_end_matches(".desktop").to_string();
        let find = |c: &str| if c.is_empty() { None } else { tessera_core::util::desktop_entry(c) };
        let entry = find(&id).or_else(|| find(&js(&app["pkg"]))).or_else(|| find(&js(&app["name"])));
        if let Some(p) = entry {
            let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            crate::proc::detach(&["gtk-launch", &stem]);
        } else if app["source"] == "flathub" {
            crate::proc::detach(&["flatpak", "run", &js(&app["pkg"])]);
        } else if !id.is_empty() {
            crate::proc::detach(&["gtk-launch", &id]);
        }
    }

    /// What the App Store surface is told as its state
    pub fn store_feed(&self) -> Value {
        json!({"open": self.store.panel_open, "catalogue": self.store.status["catalogue"] == false && tessera_core::util::on_path("pacman"), "updates": self.upd.running(),
            "jobs": self.store.jobs, "app": self.store.panel_app.as_ref().map(enrich).unwrap_or(Value::Null)})
    }

    /// What the surface asks (`shell::ask("appstore", [what, ...])`)
    pub fn store_call(&mut self, h: &mut Host, what: &str, a: &[Value]) {
        let arg = |i: usize| a.get(i).cloned().unwrap_or(Value::Null);
        match what {
            "load" => {
                // (the QML window's load(): the same questions)
                let token = arg(0);
                let view = js(&arg(1));
                let category = js(&arg(2));
                let query = js(&arg(3));
                let filter = if arg(4).is_null() { "all".to_string() } else { js(&arg(4)) };
                let p = Pending::Load(token);
                if view == "discover" {
                    self.store_ask("discover", json!({}), p);
                } else if view == "category" {
                    self.store_ask("category", json!({"category": category}), p);
                } else if view == "installed" {
                    self.store_ask("installed", json!({}), p);
                } else {
                    let q = query.trim_start();
                    let q = if q.len() >= 4 && q[..4].eq_ignore_ascii_case("aur:") { q[4..].trim_start() } else { &query };
                    let sources = if filter == "all" { json!(["repo", "flathub"]) } else { json!([filter]) };
                    self.store_ask("search", json!({"q": q, "limit": 40, "sources": sources}), p);
                }
            }
            "close" => {
                // (Panels.appStoreOpen = false)
                self.store.panel_open = false;
                h.ask("panels.appStoreOpen", json!([false]));
            }
            "install" => {
                let app = arg(0);
                if !js(&app["key"]).is_empty() {
                    self.store_install(h, &app);
                }
            }
            "open" | "openApp" => {
                let app = arg(0);
                if !js(&app["key"]).is_empty() || what == "openApp" {
                    self.store_open_app(&app);
                }
            }
            "remove" => {
                let app = arg(0);
                if !js(&app["key"]).is_empty() {
                    self.store_remove(h, &app);
                }
            }
            "catalogue" => self.store_install_catalogue(),
            "website" => self.open_link(h, &js(&arg(0))),
            "copy" => tessera_core::util::pipe_to(&["wl-copy"], js(&arg(0)).into_bytes()),
            // the other hosts
            "panel" => self.store.panel_open = arg(0).as_bool() == Some(true),
            "app" => {
                self.store.panel_app = if arg(0).is_null() { None } else { Some(arg(0)) };
                if let Some(app) = &self.store.panel_app {
                    h.tell("appstore", json!({"show": enrich(app)}));
                }
            }
            "lookUp" => self.store_look_up(&js(&arg(0))),
            "forget" => self.store_forget(),
            "users" => self.store_users(arg(0).as_i64().unwrap_or(0) as i32),
            _ => eprintln!("tessera-ui: host: no appstore call {what}"),
        }
    }

    pub fn store_state(&self) -> Value {
        json!({"quick": {"query": self.store.quick_query, "apps": self.store.quick.iter().map(enrich).collect::<Vec<_>>(), "loading": self.store.quick_loading, "revision": self.store.quick_revision}})
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_logo_is_a_file_or_flathubs_address() {
        assert_eq!(file("file:///usr/share/icons/a%20b.png"), "/usr/share/icons/a b.png");
        assert_eq!(file("https://dl.flathub.org/x.png"), "https://dl.flathub.org/x.png");
        assert_eq!(file("firefox"), "");
        assert_eq!(file("/opt/x.svg"), "/opt/x.svg");
    }

    #[test]
    fn sources_have_names() {
        assert_eq!(source_name(&json!({"source": "aur"})), "AUR \u{b7} community");
        assert_eq!(source_name(&json!({"source": "other"})), "");
        let e = enrich(&json!({"source": "repo", "icon": "https://x/y.png"}));
        assert_eq!(e["_source"], "Arch repos");
        assert_eq!(e["_icon"], "https://x/y.png");
    }
}

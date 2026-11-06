//! Which panel is open and where: shell/common/Panels.qml, with RustUi.qml's
//! forwarding of it (`follow`, the sidebar's tab, the session's action, the
//! settings window's open/page/close, Quick Look) and TesseraShell.qml's
//! `panels` and `shell` IPC targets. Also the `rustui act …` target the menu
//! bar drives (modules/menubar/RustBridge.qml's `act`).
//!
//! The model (`Model`) is Panels.qml's properties; after every change the
//! service (`sync`) says what the surfaces and the other services need: the
//! `open`/`close` command lines RustUi.follow wrote, and the shell state's
//! fields ("open", "screen", and "panels.*" for the other host crates).

use crate::common::*;
use serde_json::{json, Value};
use tessera_core::host::{Host, Service};

/// Panels whose name isn't the surface's (RustUi.panelNames; surface names are lowercase)
const PANEL_NAMES: &[(&str, &str)] = &[("controlcenter", "controlCenter"), ("screentools", "screenTools"), ("missioncontrol", "missionControl"), ("osk", "keyboard"), ("notes", "quickNote"), ("network", "wifiMenu"), ("widgetoptions", "widgetOptions")];

pub fn panel_of(surface: &str) -> String {
    PANEL_NAMES.iter().find(|(s, _)| *s == surface).map_or(surface, |(_, p)| *p).to_string()
}

pub fn surface_of(panel: &str) -> String {
    // (the Notification Center's panel is the notifications surface: its own
    // server draws the banners and the center)
    if panel == "notificationCenter" {
        return "notifications".into();
    }
    PANEL_NAMES.iter().find(|(_, p)| *p == panel).map_or(panel, |(s, _)| *s).to_string()
}

/// Panels.qml's properties and its functions
#[derive(Default, Clone, Debug)]
pub struct Model {
    pub open: String,
    /// the output's name ("" none)
    pub screen: String,
    pub anchor_x: f64,
    pub cc_page: String,
    pub session_action: String,
    pub share_paths: Vec<String>,
    pub share_note: String,
    pub quick_look_path: String,
    pub expose_app: String,
    pub cc_editing: bool,
    pub widget_editing: bool,
    pub widget_options_kind: String,
    pub hold_pull: i64,
    pub hold_menus: i64,
    pub settings_open: bool,
    pub settings_page: String,
    pub settings_target: String,
    pub app_store_open: bool,
    pub app_store_app: Value,
    pub osd_visible: bool,
    pub sidebar_tab: i64,
    pub nc_tab: i64,
    pub last_closed: String,
    pub last_closed_at: i64,
}

impl Model {
    pub fn new() -> Self {
        Self { settings_page: "appearance".into(), ..Default::default() }
    }

    pub fn is_open(&self, name: &str, screen: Option<&str>) -> bool {
        self.open == name && screen.map_or(true, |s| s.is_empty() || self.screen == s)
    }

    /// Panels.show: `screen` None is the focused one (`focused`), else the first
    pub fn show(&mut self, name: &str, screen: Option<&str>, x: Option<f64>, action: Option<&str>, focused: &str, first: &str) {
        if name != "missionControl" {
            self.expose_app.clear();
        }
        self.session_action = if name == "session" { action.unwrap_or("").to_string() } else { String::new() };
        self.open = name.to_string();
        self.screen = match screen.filter(|s| !s.is_empty()) {
            Some(s) => s.to_string(),
            None if !focused.is_empty() => focused.to_string(),
            None => first.to_string(),
        };
        if let Some(x) = x {
            self.anchor_x = x;
        }
    }

    /// Panels.toggle: a click on a menu bar item first dismisses the open
    /// popover, then reaches the item; that must not reopen the same popover
    pub fn toggle(&mut self, name: &str, screen: Option<&str>, x: Option<f64>, now: i64, focused: &str, first: &str) {
        if self.is_open(name, screen) || (self.last_closed == name && now - self.last_closed_at < 250) {
            self.close(now);
        } else {
            self.show(name, screen, x, None, focused, first);
        }
    }

    pub fn close(&mut self, now: i64) {
        self.expose_app.clear();
        if !self.open.is_empty() {
            self.last_closed = self.open.clone();
            self.last_closed_at = now;
        }
        self.open.clear();
        self.cc_page.clear();
    }

    pub fn open_settings(&mut self, page: Option<&str>, target: Option<&str>, now: i64) {
        self.close(now);
        if let Some(p) = page.filter(|p| !p.is_empty()) {
            self.set_settings_page(p);
        }
        if let Some(t) = target.filter(|t| !t.is_empty()) {
            self.settings_target = t.to_string();
        }
        self.settings_open = true;
    }

    /// Panels.settingsPage = …: an older page id goes to where that page is now
    pub fn set_settings_page(&mut self, page: &str) {
        match settings_alias(page) {
            Some((p, part)) => {
                self.settings_target = part.to_string();
                self.settings_page = p.to_string();
            }
            None => self.settings_page = page.to_string(),
        }
    }

    pub fn open_app_store(&mut self, app: Value, now: i64) {
        self.close(now);
        self.app_store_app = app;
        self.app_store_open = true;
    }

    pub fn quick_look(&mut self, path: &str) {
        self.quick_look_path = if self.quick_look_path == path { String::new() } else { path.to_string() };
    }

    pub fn share(&mut self, paths: Vec<String>, note: &str) {
        self.share_paths = paths;
        self.share_note = note.to_string();
    }

    /// Panels.appExpose
    pub fn app_expose(&mut self, app: &str, screen: Option<&str>, focused: &str, first: &str) {
        if app.is_empty() {
            return;
        }
        self.expose_app = app.to_string();
        self.show("missionControl", screen, None, None, focused, first);
    }
}

#[derive(Default)]
pub struct Panels {
    m: Model,
    /// the panel the surface was told to open, and its screen (RustUi.forwarded)
    forwarded: String,
    forwarded_screen: String,
    sent_tab: i64,
    sent_action: Option<(String, String)>,
    sent_about: bool,
    sent_settings: Option<(bool, String)>,
    /// the App Store window as last told to host-sharing (appstore.panel / .app)
    sent_app_store: Option<(bool, Value)>,
    /// host-system's settings bridges were told the window is open / which page shows
    sent_live: bool,
    sent_bridge_page: String,
    sent_quick_look: String,
    /// the other host crates' fields of each settings page group (`settings.feed`)
    feeds: std::collections::BTreeMap<String, Value>,
    sent_feeds: std::collections::BTreeMap<String, String>,
}

impl Panels {
    pub fn new() -> Self {
        Self { m: Model::new(), sent_tab: -1, ..Default::default() }
    }
}

fn str_arg<'a>(a: &'a Value, i: usize) -> &'a str {
    a.get(i).and_then(Value::as_str).unwrap_or("")
}

fn opt(s: &str) -> Option<&str> {
    (!s.is_empty()).then_some(s)
}

/// The focused output's name, else ""
pub fn focused(h: &Host) -> String {
    h.st.shell.get::<tessera_core::services::hypr::Snapshot>().map(|s| s.focused.clone()).unwrap_or_default()
}

pub fn first_output(h: &Host) -> String {
    h.st.outputs_now().into_iter().map(|(_, n)| n).next().unwrap_or_default()
}

/// Is a surface drawn here? (RustUi.drawing: standalone, every one the binary knows)
pub fn drawing(h: &Host, surface: &str) -> bool {
    h.st.shell.surfaces.on.iter().any(|s| *s == surface)
}

impl Panels {
    fn args_of(args: &Value) -> Value {
        if args.is_array() {
            args.clone()
        } else {
            json!([])
        }
    }

    fn show(&mut self, h: &Host, name: &str, screen: &str, x: Option<f64>, action: Option<&str>) {
        let (f, first) = (focused(h), first_output(h));
        self.m.show(name, opt(screen), x, action, &f, &first);
    }

    /// Panels.qml's functions, and what other services ask: panels.<function>
    fn act(&mut self, h: &mut Host, func: &str, a: &Value) {
        let now = now_ms();
        let (f, first) = (focused(h), first_output(h));
        match func {
            "show" | "open" => {
                let action = a.get(2).and_then(Value::as_str).filter(|_| str_arg(a, 0) == "session");
                self.m.show(str_arg(a, 0), opt(str_arg(a, 1)), a.get(2).and_then(Value::as_f64), action, &f, &first);
            }
            "showSession" => self.m.show("session", opt(str_arg(a, 1)), None, Some(str_arg(a, 0)), &f, &first),
            "toggle" => self.m.toggle(str_arg(a, 0), opt(str_arg(a, 1)), a.get(2).and_then(Value::as_f64), now, &f, &first),
            // toggleAt <panel> <screen> <x>: a panel under its menu bar item
            "toggleAt" => self.m.toggle(str_arg(a, 0), opt(str_arg(a, 1)), a.get(2).and_then(|x| x.as_f64().or_else(|| x.as_str().and_then(|s| s.parse().ok()))), now, &f, &first),
            "close" => self.m.close(now),
            // a panel drawn here is open: close it (the window-management surfaces)
            "closeOpen" => {
                if !self.m.open.is_empty() {
                    self.m.close(now);
                }
            }
            "closed" => {
                // it closed itself (Esc, a click outside, a pick): follow, if still the open one
                let surface = str_arg(a, 0).to_string();
                if self.forwarded == surface {
                    self.forwarded.clear();
                }
                if self.m.open == panel_of(&surface) || (surface == "notifications" && self.m.open == "notificationCenter") {
                    self.m.close(now);
                }
            }
            "openSettings" => self.m.open_settings(opt(str_arg(a, 0)), opt(str_arg(a, 1)), now),
            // the Settings shortcut: closes the window when it is open
            "toggleSettings" => {
                if self.m.settings_open {
                    self.m.settings_open = false;
                } else {
                    self.m.open_settings(None, None, now);
                }
            }
            "openAppStore" => self.m.open_app_store(a.get(0).cloned().unwrap_or(Value::Null), now),
            "closeAppStore" => self.m.app_store_open = false,
            "quickLook" | "quicklook" => {
                if func == "quicklook" {
                    self.m.quick_look_path = str_arg(a, 0).to_string();
                } else {
                    self.m.quick_look(str_arg(a, 0));
                }
            }
            "share" => {
                // [paths, screen?, note?]
                let paths: Vec<String> = a.get(0).and_then(Value::as_array).map(|l| l.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
                self.m.share(paths, str_arg(a, 2));
                self.m.show("share", opt(str_arg(a, 1)), None, None, &f, &first);
            }
            "pickShare" => h.ask("picker.pick", json!([{"multiple": true, "title": "Send files", "reply": "share"}])),
            "appExpose" => self.m.app_expose(str_arg(a, 0), opt(str_arg(a, 1)), &f, &first),
            // panels.set [key, value]: the plain properties other services keep
            "set" => {
                let key = str_arg(a, 0);
                self.set(key, a.get(1).cloned().unwrap_or(Value::Null));
                // (host-desktop's desktop feed keeps the edit itself)
                if key == "widgetEditing" {
                    h.ask("desktop", json!([if self.m.widget_editing { "edit" } else { "endEdit" }]));
                }
            }
            "page" => self.m.cc_page = str_arg(a, 0).to_string(),
            "tab" => self.m.sidebar_tab = a.get(0).and_then(|x| x.as_i64().or_else(|| x.as_str().and_then(|s| s.parse().ok()))).unwrap_or(0),
            "nctab" => self.m.nc_tab = a.get(0).and_then(|x| x.as_i64().or_else(|| x.as_str().and_then(|s| s.parse().ok()))).unwrap_or(0),
            // Edit Widgets on the first screen
            "widgets" => {
                self.m.close(now);
                self.m.screen = first.clone();
                self.m.widget_editing = matches!(a.get(0), Some(Value::Bool(true))) || str_arg(a, 0) == "true";
                // (host-desktop's desktop feed keeps the edit itself: DesktopFeed's
                // `editing` followed Panels.widgetEditing)
                h.ask("desktop", json!([if self.m.widget_editing { "edit" } else { "endEdit" }]));
            }
            // panicReset: close everything, in case something is stuck
            "reset" => {
                self.m.close(now);
                self.m.settings_open = false;
                h.ask("screen.cancelSelect", json!([]));
            }
            "shareFiles" => {
                // one path per line; a file:// URI is decoded
                let paths: Vec<String> = str_arg(a, 0).split('\n').filter(|p| !p.is_empty()).map(|p| p.strip_prefix("file://").map_or(p.to_string(), percent_decode)).collect();
                if !paths.is_empty() {
                    self.m.share(paths, "");
                    self.m.show("share", None, None, None, &f, &first);
                }
            }
            "shareSelection" => h.ask("share.selection", json!([])),
            other => eprintln!("tessera-ui: host: no panels.{other}"),
        }
    }

    fn set(&mut self, key: &str, v: Value) {
        let b = || v.as_bool().unwrap_or(false);
        let s = || v.as_str().unwrap_or("").to_string();
        match key {
            "ccPage" => self.m.cc_page = s(),
            "ccEditing" => self.m.cc_editing = b(),
            "widgetEditing" => self.m.widget_editing = b(),
            "widgetOptionsKind" => self.m.widget_options_kind = s(),
            "holdPull" => self.m.hold_pull = v.as_i64().unwrap_or(0),
            "holdMenus" => self.m.hold_menus = v.as_i64().unwrap_or(0),
            "sidebarTab" => self.m.sidebar_tab = v.as_i64().unwrap_or(0),
            "ncTab" => self.m.nc_tab = v.as_i64().unwrap_or(0),
            "settingsOpen" => self.m.settings_open = b(),
            "settingsPage" => self.m.set_settings_page(&s()),
            "settingsTarget" => self.m.settings_target = s(),
            "appStoreOpen" => self.m.app_store_open = b(),
            "osdVisible" => self.m.osd_visible = b(),
            "screen" => self.m.screen = s(),
            "exposeApp" => self.m.expose_app = s(),
            "sessionAction" => self.m.session_action = s(),
            "anchorX" => self.m.anchor_x = v.as_f64().unwrap_or(0.0),
            other => eprintln!("tessera-ui: host: no Panels.{other}"),
        }
    }

    /// The shell's state for the menu bar and the other host crates
    fn publish(&self, h: &mut Host) {
        let m = &self.m;
        h.set_state("open", json!(m.open));
        h.set_state("screen", json!(m.screen));
        h.set_state("panels", json!({
            "open": m.open, "screen": m.screen, "anchorX": m.anchor_x, "ccPage": m.cc_page, "sessionAction": m.session_action,
            "sharePaths": m.share_paths, "shareNote": m.share_note, "quickLookPath": m.quick_look_path, "exposeApp": m.expose_app,
            "ccEditing": m.cc_editing, "widgetEditing": m.widget_editing, "widgetOptionsKind": m.widget_options_kind,
            "holdPull": m.hold_pull, "holdMenus": m.hold_menus, "settingsOpen": m.settings_open, "settingsPage": m.settings_page,
            "appStoreOpen": m.app_store_open, "appStoreApp": m.app_store_app, "osdVisible": m.osd_visible,
            "sidebarTab": m.sidebar_tab, "ncTab": m.nc_tab
        }));
    }

    /// RustUi.follow: the panel a Rust surface draws opens there, and closes
    /// when another opens; a panel already open moves over
    fn follow(&mut self, h: &mut Host) {
        let want = if !self.m.open.is_empty() && drawing(h, &surface_of(&self.m.open)) { surface_of(&self.m.open) } else { String::new() };
        let scr = if want.is_empty() { String::new() } else { self.m.screen.clone() };
        if !self.forwarded.is_empty() && self.forwarded != want {
            h.command("close", &self.forwarded.clone(), None, json!({}));
        }
        if !want.is_empty() && (want != self.forwarded || scr != self.forwarded_screen) {
            h.command("open", &want, Some(&scr), json!({}));
        }
        self.forwarded = want;
        self.forwarded_screen = scr;
    }

    /// After a change: everything that follows Panels' properties
    fn sync(&mut self, h: &mut Host) {
        self.publish(h);
        self.follow(h);
        // The sidebar's tab (sent before it opens, and again once it's drawing)
        if self.sent_tab != self.m.sidebar_tab {
            self.sent_tab = self.m.sidebar_tab;
            if drawing(h, "sidebar") {
                h.command("tab", "sidebar", None, json!({"tab": self.m.sidebar_tab}));
            }
        }
        // The power menu: Panels.sessionAction goes with it
        let act = (self.m.open.clone(), self.m.session_action.clone());
        if drawing(h, "session") && self.sent_action.as_ref() != Some(&act) {
            if act.0 == "session" {
                h.command("action", "session", None, json!({"action": act.1}));
            }
            self.sent_action = Some(act);
        }
        // About (the power menu's surface draws it too)
        let about = self.m.open == "about";
        if drawing(h, "session") && (about || self.sent_about) && about != self.sent_about {
            let info = h.state("about.info").clone();
            h.command("about", "session", None, json!({"open": about, "screen": if about { self.m.screen.clone() } else { String::new() }, "info": if info.is_null() { json!({}) } else { info }}));
        }
        self.sent_about = about;
        // The App Store: Panels.appStoreOpen and .appStoreApp, to host-sharing's
        // AppStore (it keeps the backend and the window's state feed)
        let store = (self.m.app_store_open, self.m.app_store_app.clone());
        if self.sent_app_store.as_ref() != Some(&store) {
            if self.sent_app_store.as_ref().map(|s| s.1 != store.1).unwrap_or(true) {
                h.ask("appstore.app", json!([store.1]));
            }
            h.ask("appstore.panel", json!([store.0]));
            self.sent_app_store = Some(store);
        }
        // Settings: Panels.settingsOpen, .settingsPage and .settingsTarget as commands
        if drawing(h, "settings") {
            let focus = focused(h);
            let now = (self.m.settings_open, self.m.settings_page.clone());
            match (&self.sent_settings, self.m.settings_open) {
                (Some((false, _)) | None, true) => {
                    let screen = if focus.is_empty() { self.m.screen.clone() } else { focus };
                    h.command("open", "settings", None, json!({"screen": screen, "page": self.m.settings_page, "target": self.m.settings_target}));
                    self.m.settings_target.clear();
                }
                (Some((true, _)), false) => h.command("close", "settings", None, json!({})),
                (Some((true, page)), true) if *page != self.m.settings_page || !self.m.settings_target.is_empty() => {
                    h.command("page", "settings", None, json!({"page": self.m.settings_page, "target": self.m.settings_target}));
                    self.m.settings_target.clear();
                }
                _ => {}
            }
            self.sent_settings = Some(now);
        }
        // host-system's settings bridges (devices, input, accounts, system, about) work while the
        // window is open and follow its page (RustBridge*.qml's `live` and the Settings page)
        let open = self.m.settings_open;
        if open != self.sent_live {
            self.sent_live = open;
            for g in ["devices", "input", "accounts", "system", "about"] {
                h.ask(&format!("{g}.live"), json!([open]));
            }
            if !open {
                self.sent_bridge_page.clear();
            }
        }
        if open && self.sent_bridge_page != self.m.settings_page {
            self.sent_bridge_page = self.m.settings_page.clone();
            for g in ["devices", "system", "about"] {
                h.ask(&format!("{g}.page"), json!([self.m.settings_page]));
            }
        }
        // What only the shell knows for the pages, while the window is open
        if drawing(h, "settings") && self.m.settings_open {
            let mut groups: std::collections::BTreeSet<String> = ["look".to_string(), "notify".to_string()].into_iter().collect();
            groups.extend(self.feeds.keys().cloned());
            for g in groups {
                let mine = match g.as_str() {
                    "look" => crate::bridges::look_feed(h),
                    "notify" => json!({"glassPlugin": h.st.shell.plugin, "pickerBusy": h.state("picker.busy").clone()}),
                    _ => json!({}),
                };
                let feed = crate::bridges::merged(mine, self.feeds.get(&g));
                let text = feed.to_string();
                if self.sent_feeds.get(&g) != Some(&text) {
                    self.sent_feeds.insert(g.clone(), text);
                    h.tell("settings", json!({"feed": {g: feed}}));
                }
            }
        } else {
            self.sent_feeds.clear();
        }
        // Quick Look (QuickLook.qml's forward)
        if drawing(h, "quicklook") && self.sent_quick_look != self.m.quick_look_path {
            self.sent_quick_look = self.m.quick_look_path.clone();
            if self.m.quick_look_path.is_empty() {
                h.command("close", "quicklook", None, json!({}));
            } else {
                let screen = focused(h);
                let query = h.state("livetext.query").clone();
                if !query.is_null() && query != "" {
                    h.set_state("livetext.query", json!("")); // (taken once)
                }
                h.command("open", "quicklook", None, json!({"path": self.m.quick_look_path, "query": query, "screen": screen}));
            }
        }
        // Settings' own state (the model) is shared back through the published fields
        self.publish(h);
    }

    /// RustUi.handle: closed, osd, and the session menu's choice; the lock's go to Bridges
    fn ui_event(&mut self, h: &mut Host, e: &Value) {
        match e["kind"].as_str().unwrap_or("") {
            "closed" => self.act(h, "closed", &json!([e["surface"]])),
            "osd" => self.m.osd_visible = e["shown"] == true,
            "session" => {
                if let Some(p) = e["power"].as_str().filter(|p| ["lock", "sleep", "logout", "restart", "shutdown"].contains(p)) {
                    h.ask("system.power", json!([p]));
                }
                if e["close"] == "about" && self.m.open == "about" {
                    self.m.close(now_ms());
                }
            }
            "lock" => h.ask("lock.ui", json!([e])),
            // the surfaces are up: what they are told at "ready"
            "ready" => {
                self.forwarded.clear();
                self.sent_tab = -1;
                self.sent_settings = None;
                self.sent_app_store = None;
                self.sent_quick_look.clear();
                h.ask("lock.resend", json!([]));
            }
            "unknown" => eprintln!("tessera-ui: no {} in Rust yet", e["surface"].as_str().unwrap_or("")),
            _ => {}
        }
    }

    /// The menu bar's `qs ipc call rustui act <what> <arg>` (RustBridge.act)
    fn rustui_act(&mut self, h: &mut Host, what: &str, arg: &str) {
        let a: Vec<&str> = arg.split(' ').collect();
        let screen = |n: &str| if h.st.output_named(n).is_some() { n.to_string() } else { String::new() };
        match what {
            "toggleAt" => {
                let s = screen(a.get(1).copied().unwrap_or(""));
                self.act(h, "toggleAt", &json!([a[0], s, a.get(2).and_then(|x| x.parse::<f64>().ok()).unwrap_or(0.0)]));
            }
            "page" => {
                let s = screen(a.get(1).copied().unwrap_or(""));
                self.act(h, "show", &json!(["controlCenter", s]));
                self.m.cc_page = a[0].to_string();
            }
            "about" | "session" => {
                let s = screen(a[0]);
                self.act(h, "show", &json!([if what == "about" { "about" } else { "session" }, s]));
            }
            "settings" => self.m.open_settings(None, None, now_ms()),
            "shelf" => h.ask("shelf.toggle", json!([screen(a[0])])),
            // everything else is another service's (Updates, System, Audio, Apps, Spaces…)
            "installUpdates" => h.ask("system.installUpdates", json!([])),
            "repairShell" => h.ask("system.repairShell", json!([])),
            "power" => h.ask("system.power", json!([a[0]])),
            "darkMode" => h.ask("system.toggleDarkMode", json!([])),
            "mic" => h.ask("system.toggleMic", json!([true])),
            "record" => h.ask("screen.recordToggle", json!([])),
            "stopRecording" => h.ask("screen.stopRecording", json!([])),
            "keyboardLayout" => h.ask("keyboardlayout.next", json!([])),
            "powerProfile" => h.ask("system.cyclePowerProfile", json!([])),
            "launch" => h.ask("apps.launch", json!([a[0]])),
            "quit" => h.ask("apps.quit", json!([a[0]])),
            "mergeTabs" => h.ask("apps.mergeTabs", json!([])),
            "moveSpace" => h.ask("spaces.moveSpace", json!([a[0].parse::<i64>().unwrap_or(0), a.get(1).copied().unwrap_or("")])),
            "go" => h.ask("spaces.go", json!([a[0].parse::<i64>().unwrap_or(0)])),
            "islandAct" | "islandRemove" | "islandHold" | "islandPin" => h.ask(&format!("island.{}", &what[6..].to_lowercase()), json!(a)),
            "live" => h.ask("island.live", json!(a)),
            "hud" => h.ask("island.hud", json!([a[0]])),
            other => eprintln!("tessera-ui: host: RustBridge: nothing called {other}"),
        }
    }
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Some(v) = s.get(i + 1..i + 3).and_then(|x| u8::from_str_radix(x, 16).ok()) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

impl Service for Panels {
    fn name(&self) -> &'static str {
        "Panels"
    }
    fn calls(&self) -> &'static [&'static str] {
        // ("ui": what surfaces say on stdout, shell::tell; standalone it comes here)
        &["panels", "settings", "sidebar", "ui"]
    }
    fn ipc_targets(&self) -> &'static [&'static str] {
        &["panels", "shell", "rustui", "emoji"]
    }
    fn start(&mut self, h: &mut Host) {
        h.st.shell.want("hypr");
        self.sync(h);
    }
    fn call(&mut self, h: &mut Host, call: &str, args: &Value) {
        let a = Self::args_of(args);
        let func = call.split_once('.').map_or("", |(_, f)| f);
        match call.split('.').next().unwrap_or("") {
            "panels" => self.act(h, func, &a),
            // the sidebar's tab row: Panels keeps it for next time
            "sidebar" => self.m.sidebar_tab = a.get(0).and_then(Value::as_i64).unwrap_or(0),
            // what a surface says on stdout (`shell::tell`): RustUi.handle
            "ui" => self.ui_event(h, args),
            "settings" => {
                // `settings.feed` [group, patch] from the other host crates: as ["feed", group, patch]
                if func == "feed" {
                    let mut v = vec![json!("feed")];
                    v.extend(a.as_array().cloned().unwrap_or_default());
                    self.settings_call(h, &Value::Array(v));
                } else {
                    self.settings_call(h, &a)
                }
            }
            _ => {}
        }
        self.sync(h);
    }
    fn ipc(&mut self, h: &mut Host, target: &str, func: &str, args: &[String]) -> Option<String> {
        let out = match target {
            // shell: ready → "yes" once the desktop has drawn
            "shell" => (func == "ready").then(|| "yes".to_string()),
            // EmojiPicker.qml's target: toggle / open / close, on the focused screen
            "emoji" => {
                let s = focused(h);
                match func {
                    "toggle" => self.act(h, "toggle", &json!(["emoji", s])),
                    "open" => self.act(h, "show", &json!(["emoji", s])),
                    "close" if self.m.open == "emoji" => self.act(h, "close", &json!([])),
                    _ => {}
                }
                None
            }
            "rustui" => {
                if func == "act" {
                    self.rustui_act(h, args.first().map_or("", String::as_str), args.get(1).map_or("", String::as_str));
                }
                None
            }
            _ => {
                // qs ipc call panels <function> <args…>: strings, as IpcHandler takes them
                let a: Vec<Value> = args.iter().map(|s| json!(s)).collect();
                let func = match func {
                    "appStore" => "openAppStore",
                    "settings" => "openSettings",
                    f => f,
                };
                let a = if func == "widgets" { vec![json!(args.first().is_some_and(|s| s == "true"))] } else { a };
                self.act(h, func, &Value::Array(a));
                None
            }
        };
        self.sync(h);
        out
    }
    fn config_changed(&mut self, h: &mut Host) {
        self.sync(h);
    }
    fn outputs_changed(&mut self, h: &mut Host) {
        // (the screen a panel was on is gone: it closes, as Panels.isOpen no longer matches)
        if !self.m.screen.is_empty() && h.st.output_named(&self.m.screen).is_none() && !self.m.open.is_empty() {
            self.m.close(now_ms());
        }
        self.sync(h);
    }
}

impl Panels {
    /// What the Rust settings window asks (RustBridge.qml's onSettingsCall): close, page, pick, act
    fn settings_call(&mut self, h: &mut Host, a: &Value) {
        match str_arg(a, 0) {
            "close" => self.m.settings_open = false,
            "page" => {
                let p = str_arg(a, 1);
                if !p.is_empty() && self.m.settings_page != p {
                    self.m.set_settings_page(p);
                    // (the window is already there: not sent back)
                    self.sent_settings = Some((self.m.settings_open, self.m.settings_page.clone()));
                    self.m.settings_target.clear();
                }
            }
            // [row id, folder, title, start, filter]: a file chooser for a path row
            "pick" => h.ask("picker.pick", json!([{"reply": "settings", "id": str_arg(a, 1), "folder": a.get(2).and_then(Value::as_bool).unwrap_or(false), "title": str_arg(a, 3), "start": if str_arg(a, 4).is_empty() { "~" } else { str_arg(a, 4) }, "filter": str_arg(a, 5)}])),
            // [group, patch]: the other host crates tell the pages what only they know
            "feed" => {
                let group = str_arg(a, 1).to_string();
                let entry = self.feeds.entry(group).or_insert_with(|| json!({}));
                if let (Value::Object(e), Some(Value::Object(p))) = (entry, a.get(2)) {
                    for (k, v) in p {
                        e.insert(k.clone(), v.clone());
                    }
                }
            }
            // [group, what, …args], by page group
            "act" => {
                let group = str_arg(a, 1).to_string();
                let what = str_arg(a, 2).to_string();
                let rest: Vec<Value> = a.as_array().map(|l| l.iter().skip(3).cloned().collect()).unwrap_or_default();
                match group.as_str() {
                    "look" | "notify" => {
                        let mut ctx = crate::bridges::Ctx { panels: &mut self.m };
                        crate::bridges::settings_act(h, &mut ctx, &group, &what, &rest);
                        // (the window answers what it asked: its feed said again)
                        self.sent_feeds.clear();
                    }
                    // devices, input, accounts, system, about: host-system's bridges
                    // (the bridges' own names, as their notes say: devices.act / input.act [what, …args],
                    // accounts [what, …args], about.<what> and system.<what> […args])
                    "devices" | "input" => h.ask(&format!("{group}.act"), json!(std::iter::once(json!(what)).chain(rest).collect::<Vec<_>>())),
                    "accounts" => h.ask("accounts", json!(std::iter::once(json!(what)).chain(rest).collect::<Vec<_>>())),
                    "about" | "system" => h.ask(&format!("{group}.{what}"), Value::Array(rest)),
                    other => eprintln!("tessera-ui: host: settings: no page group {other}"),
                }
            }
            other => eprintln!("tessera-ui: host: settings: no call {other}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_and_surface_names() {
        assert_eq!(panel_of("controlcenter"), "controlCenter");
        assert_eq!(surface_of("controlCenter"), "controlcenter");
        assert_eq!(surface_of("wifiMenu"), "network");
        assert_eq!(surface_of("quickNote"), "notes");
        assert_eq!(panel_of("emoji"), "emoji");
        assert_eq!(surface_of("notificationCenter"), "notifications");
    }

    #[test]
    fn show_toggle_and_close() {
        let mut m = Model::new();
        m.show("spotlight", None, None, None, "DP-1", "HDMI-A-1");
        assert!(m.is_open("spotlight", Some("DP-1")) && !m.is_open("spotlight", Some("HDMI-A-1")));
        m.toggle("spotlight", Some("DP-1"), None, 1000, "DP-1", "");
        assert_eq!(m.open, "");
        // the click that dismissed it must not reopen it at once
        m.toggle("spotlight", Some("DP-1"), None, 1100, "DP-1", "");
        assert_eq!(m.open, "");
        m.toggle("spotlight", Some("DP-1"), None, 1400, "DP-1", "");
        assert_eq!(m.open, "spotlight");
        // no screen and no focused one: the first
        m.show("emoji", None, Some(12.0), None, "", "eDP-1");
        assert_eq!((m.screen.as_str(), m.anchor_x), ("eDP-1", 12.0));
    }

    #[test]
    fn closing_clears_the_control_center_page_and_expose() {
        let mut m = Model::new();
        m.app_expose("firefox", None, "DP-1", "");
        assert_eq!((m.open.as_str(), m.expose_app.as_str()), ("missionControl", "firefox"));
        m.show("controlCenter", None, None, None, "DP-1", "");
        assert_eq!(m.expose_app, "");
        m.cc_page = "wifi".into();
        m.close(5);
        assert_eq!((m.cc_page.as_str(), m.last_closed.as_str()), ("", "controlCenter"));
    }

    #[test]
    fn session_action_only_for_the_power_menu() {
        let mut m = Model::new();
        m.show("session", None, None, Some("shutdown"), "DP-1", "");
        assert_eq!(m.session_action, "shutdown");
        m.show("cheatsheet", None, None, Some("shutdown"), "DP-1", "");
        assert_eq!(m.session_action, "");
    }

    #[test]
    fn settings_open_with_an_older_page_id() {
        let mut m = Model::new();
        m.open_settings(Some("bluetooth"), None, 0);
        assert!(m.settings_open);
        assert_eq!((m.settings_page.as_str(), m.settings_target.as_str()), ("network", "Bluetooth"));
        m.open_settings(Some("sound"), Some("Output"), 0);
        assert_eq!((m.settings_page.as_str(), m.settings_target.as_str()), ("sound", "Output"));
    }

    #[test]
    fn quick_look_again_closes_it() {
        let mut m = Model::new();
        m.quick_look("/a.png");
        assert_eq!(m.quick_look_path, "/a.png");
        m.quick_look("/a.png");
        assert_eq!(m.quick_look_path, "");
    }

    #[test]
    fn file_uris_are_decoded() {
        assert_eq!(percent_decode("/tmp/100%25.png"), "/tmp/100%.png");
        assert_eq!(percent_decode("/tmp/a%20b"), "/tmp/a b");
        assert_eq!(percent_decode("/tmp/100%"), "/tmp/100%");
    }
}

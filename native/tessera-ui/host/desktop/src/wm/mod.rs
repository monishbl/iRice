//! The window-management half of the host (wave 4, host/desktop): the QML
//! services that keep windows, spaces and the desktop's shape, as ONE service
//! ("Wm") because they share Hyprland's events and each other's state:
//!
//!   Apps.qml            apps.rs        pin, recent, quit, minimize, show, tabs
//!   AppTheme.qml        apptheme.rs    Match apps, the Material look's windows, pointer
//!   Desktop.qml, Windows.qml  desk.rs  Show Desktop (the grouping is core's `Windows`)
//!   WindowRules.qml     rules.rs       per-app rules, default layout
//!   Spaces.qml, SpaceMap.js  spaces.rs, spacemap.rs
//!   StageManager.qml    stage.rs
//!   Tiling.qml          tiling.rs
//!   Decorations.qml     decorations.rs
//!   Focus.qml           focusmodes.rs  (the Focus modes)
//!   FocusDecision.js, FocusHistory.qml  focushist.rs
//!   FindPointer.qml     findptr.rs
//!   DesktopFeed.qml     feed.rs        the desktop's feed and calls
//!   RustWindows.qml     here           the `wm` call and what the views are told
//!   Switcher.qml        here           step / commit for the Rust switcher
//!
//! Hyprland's events (its socket2) arrive from one thread as `Msg::Ev`;
//! threads that fetch or run something answer with `Msg::Run`, a closure run
//! on the loop with the service and the host. Timers are `slint::Timer`s that
//! post `Msg::Tick(key)`.

#![allow(dead_code)]

mod apps;
mod apptheme;
mod decorations;
mod desk;
mod feed;
mod findptr;
mod focushist;
mod focusmodes;
pub mod hx;
mod rules;
mod spacemap;
mod spaces;
mod stage;
mod tiling;

use hx::Fresh;
use serde_json::{json, Value};
use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tessera_core::host::{Host, Service};

pub type Post = Arc<dyn Fn(Msg) + Send + Sync>;

pub enum Msg {
    /// a Hyprland event: name, data
    Ev(String, String),
    Tick(&'static str),
    /// something to do on the loop (a thread's answer)
    Run(Box<dyn FnOnce(&mut Wm, &mut Host, &mut Io) + Send>),
}

/// Timers and the way back to the service from threads and timers
#[derive(Default)]
pub struct Timers(HashMap<&'static str, slint::Timer>);

pub struct Io {
    post: Post,
    timers: Timers,
}

impl Io {
    fn new(post: Post) -> Self {
        Self { post, timers: Timers::default() }
    }
    /// A single-shot timer (started again if it was running: a debounce)
    pub fn arm(&mut self, key: &'static str, ms: u64) {
        let post = self.post.clone();
        self.timers.0.entry(key).or_default().start(slint::TimerMode::SingleShot, Duration::from_millis(ms), move || post(Msg::Tick(key)));
    }
    pub fn every(&mut self, key: &'static str, ms: u64) {
        let post = self.post.clone();
        self.timers.0.entry(key).or_default().start(slint::TimerMode::Repeated, Duration::from_millis(ms), move || post(Msg::Tick(key)));
    }
    pub fn stop(&mut self, key: &'static str) {
        if let Some(t) = self.timers.0.get(key) {
            t.stop();
        }
    }
    /// Hyprland's lists, asked on a thread, then `f` on the loop
    pub fn fetch_then(&self, f: impl FnOnce(&mut Wm, &mut Host, &mut Io, Fresh) + Send + 'static) {
        self.thread_then(hx::fetch, f);
    }
    /// `work` on a thread, then `then` on the loop with its result
    pub fn thread_then<T: Send + 'static>(&self, work: impl FnOnce() -> T + Send + 'static, then: impl FnOnce(&mut Wm, &mut Host, &mut Io, T) + Send + 'static) {
        let post = self.post.clone();
        std::thread::spawn(move || {
            let r = work();
            post(Msg::Run(Box::new(move |wm, h, io| then(wm, h, io, r))));
        });
    }
}

/// Hyprland's events worth following, from its socket2
fn follow_events(post: Post) {
    const WANTED: &[&str] = &[
        "openwindow", "closewindow", "movewindow", "movewindowv2", "activewindowv2", "workspace", "workspacev2", "focusedmonv2", "activespecial",
        "monitoraddedv2", "monitorremovedv2", "createworkspacev2", "destroyworkspacev2", "moveworkspacev2", "configreloaded", "changefloatingmode",
        "tessera-drag", "irice-drag",
    ];
    std::thread::spawn(move || loop {
        use std::io::{BufRead, BufReader};
        let Some(path) = tessera_core::services::hypr::dir().map(|d| d.join(".socket2.sock")) else { return };
        let Ok(stream) = std::os::unix::net::UnixStream::connect(path) else {
            std::thread::sleep(Duration::from_secs(1));
            continue;
        };
        for line in BufReader::new(stream).lines().map_while(Result::ok) {
            if let Some((name, data)) = line.split_once(">>") {
                if WANTED.contains(&name) {
                    post(Msg::Ev(name.to_string(), data.to_string()));
                }
            }
        }
        std::thread::sleep(Duration::from_millis(500));
    });
}

pub struct Wm {
    io: Io,
    spaces: spaces::Spaces,
    stage: stage::Stage,
    tiling: tiling::Tiling,
    hist: focushist::FocusHistory,
    apps: apps::AppsSvc,
    desk: desk::Desk,
    rules: rules::WindowRules,
    apptheme: apptheme::AppTheme,
    deco: decorations::Decorations,
    modes: focusmodes::FocusModes,
    findp: findptr::FindPointer,
    feed: feed::Feed,
    locked_sent: Option<bool>,
}

/// Every service of this module
pub fn services() -> Vec<Box<dyn Service>> {
    vec![Box::new(Wm::new())]
}

impl Wm {
    fn new() -> Self {
        // (the real post is made at start, when the host can hand out a sender)
        let none: Post = Arc::new(|_| {});
        Self {
            io: Io::new(none),
            spaces: Default::default(),
            stage: stage::Stage::new(),
            tiling: Default::default(),
            hist: Default::default(),
            apps: Default::default(),
            desk: Default::default(),
            rules: Default::default(),
            apptheme: Default::default(),
            deco: Default::default(),
            modes: Default::default(),
            findp: Default::default(),
            feed: Default::default(),
            locked_sent: None,
        }
    }

    fn event(&mut self, h: &mut Host, name: &str, data: &str) {
        self.hist.event(name, data);
        self.desk.event(name);
        self.tiling.event(&mut self.io, name, data);
        self.stage.event(h, &mut self.io, name, data);
        self.spaces.event(&mut self.io, name);
        self.rules.event(h, &mut self.io, name);
        self.apptheme.event(h, &mut self.io, name);
        if name == "activewindowv2" {
            self.io.arm("apps.recent", 300);
        }
    }

    fn tick(&mut self, h: &mut Host, key: &'static str) {
        let io = &mut self.io;
        match key {
            "spaces.reconcile" => self.spaces.tick_reconcile(io),
            "spaces.shown" => self.spaces.tick_shown(io),
            "stage.start" => self.stage.tick_start(h, io),
            "stage.refresh" => self.stage.tick_refresh(h),
            "apps.recent" => self.apps.note_active(h),
            "rules.sync" => self.rules.tick_sync(h, io),
            "apptheme.settle" => self.apptheme.tick_settle(h, io),
            "apptheme.windows" => self.apptheme.tick_windows(h),
            "apptheme.cursor" => self.apptheme.tick_cursor(),
            "deco.refresh" => self.deco.refresh(io),
            "deco.controls" => self.deco.tick_controls(h),
            "focus.tick" => self.modes.tick(h, io),
            "focus.save" => self.modes.save(),
            "findptr.done" => self.findp.tick_done(h),
            "feed.send" => self.feed.send(h, false),
            "wm.sendall" => {
                self.spaces.resend(h);
                self.stage.resend(h);
                self.locked_sent = None;
                self.watch(h);
                self.feed.send(h, true);
            }
            "wm.watch" => self.watch(h),
            _ => {}
        }
    }

    /// What the views are told that changes without a call: whether the screen is locked
    fn watch(&mut self, h: &mut Host) {
        let locked = h.state("locked").as_bool().unwrap_or(false);
        if self.locked_sent != Some(locked) {
            self.locked_sent = Some(locked);
            for n in ["switcher", "stagemanager"] {
                h.tell(n, json!({"locked": locked}));
            }
        }
    }

    // ── the `wm` call (RustWindows.qml) and the rest of this module's calls ──
    fn wm_call(&mut self, h: &mut Host, name: &str, a: &[Value]) {
        let s = |i: usize| a.get(i).and_then(|v| v.as_str()).unwrap_or("").to_string();
        let n = |i: usize| a.get(i).and_then(|v| v.as_f64()).unwrap_or(0.0) as i32;
        match name {
            "spaces.go" => self.spaces.go(h, n(0)),
            "spaces.newSpace" => self.spaces.new_space(h, &s(0)),
            "spaces.sendWindow" => self.spaces.send_window(h, &s(0), n(1)),
            "spaces.sendWindowToDisplay" => self.spaces.send_window_to_display(h, &s(0), &s(1)),
            "spaces.moveSpaceTo" | "spaces.moveSpace" => {
                let id = n(0);
                let exists = h.st.shell.get::<tessera_core::services::hypr::Snapshot>().is_some_and(|sn| sn.workspaces.iter().any(|w| w.id == id) || sn.active.values().any(|i| *i == id));
                if exists {
                    self.spaces.move_space_to(h, id, &s(1));
                }
            }
            "spaces.move" => {
                let order: Vec<i32> = a.first().and_then(|v| v.as_array()).map(|l| l.iter().map(|x| x.as_f64().unwrap_or(0.0) as i32).collect()).unwrap_or_default();
                self.spaces.reorder(h, &order, n(1), n(2));
            }
            // a display came (back), as its connect effect hands over (DisplayFx)
            "spaces.arrive" => self.spaces.arrive(&mut self.io, &s(0)),
            "spaces.reconcile" => self.spaces.reconcile(&mut self.io, 250),
            "apps.show" => self.apps.show(h, &mut self.desk, &s(0)),
            "apps.pin" => self.apps.pin(h, &s(0)),
            "apps.unpin" => self.apps.unpin(h, &s(0)),
            "apps.quit" => self.apps.quit(h, &s(0)),
            "apps.minimize" => self.apps.minimize(h, Some(s(0)).filter(|x| !x.is_empty()).as_deref()),
            "apps.restore" => self.apps.restore(h, &s(0)),
            "apps.activateOrLaunch" => self.apps.activate_or_launch(h, &mut self.desk, &s(0)),
            "apps.mergeIntoTabs" | "apps.mergeTabs" => self.apps.merge_into_tabs(h),
            // (by an entry id: focus its window, else start it; the shell's menus and Calls ask `apps.launch`)
            "apps.launch" => self.apps.activate_or_launch(h, &mut self.desk, &s(0)),
            "apps.windowAction" => self.apps.window_action(h, &s(0), a.get(1).unwrap_or(&Value::Null)),
            "desktop.showDesktop" => self.desk.show_desktop(h),
            "desktop.restoreDesktop" => self.desk.restore_desktop(),
            "desktop.add" => self.feed.add_widget(h, &s(0), &s(1)),
            "tiling.snap" => self.tiling.snap(&mut self.io, &s(0), Some(s(1)).filter(|x| !x.is_empty())),
            "tiling.arrange" => self.tiling.arrange(&mut self.io, &s(0)),
            "tiling.restore" => self.tiling.restore(&mut self.io),
            "tiling.pip" => self.tiling.pip(&mut self.io),
            "stage.show" => self.stage.show(h, n(0), n(1), None),
            "stage.merge" => self.stage.merge(h, n(0), n(1)),
            // Mission Control's App Exposé app (before Panels opens it)
            "overview.expose" => h.tell("missioncontrol", json!({"expose": s(0)})),
            "switcher.step" => h.command("step", "switcher", None, json!({"d": a.first().and_then(|v| v.as_i64()).unwrap_or(1)})),
            "switcher.commit" => h.command("commit", "switcher", None, json!({})),
            "switcher.cancel" => h.command("cancel", "switcher", None, json!({})),
            "focus.activate" => self.modes.activate(h, &mut self.io, &s(0), a.get(1).and_then(|v| v.as_i64()).unwrap_or(0), false),
            "focus.deactivate" => self.modes.deactivate(h, &mut self.io),
            "focus.toggle" => self.modes.toggle(h, &mut self.io, &s(0)),
            "focus.reload" => self.modes.reload(h, &mut self.io),
            "focus.setMode" => self.modes.set_mode(h, &mut self.io, &s(0), a.get(1).unwrap_or(&Value::Null)),
            "focus.addMode" => self.modes.add_mode(h, &mut self.io, &s(0)),
            // Undo of a removed mode (Settings -> Focus): the mode as it was
            "focus.restoreMode" => self.modes.restore_mode(h, &mut self.io, a.first().unwrap_or(&Value::Null)),
            "focus.removeMode" => self.modes.remove_mode(h, &mut self.io, &s(0)),
            "findpointer.find" | "findpointer.show" | "desktop.findPointer" => self.findp.show(h, &mut self.io),
            // the panels the views open are the host-shell's (RustWindows.qml forwards them to Panels)
            // Settings -> Windows: a terminal that builds the title-bar plugin (Decorations.installPlugin)
            "decorations.installPlugin" => h.ask("launch.terminal", json!([[crate::screen::util::script("install-hyprbars.sh")], false])),
            "panels.show" if s(0) == "switcher" => h.ask("panels.show", json!([s(0), s(1)])),
            "panels.closeOpen" | "panels.openAppStore" => h.ask(name, json!([])),
            _ => eprintln!("tessera-ui: no wm call {name}"),
        }
    }
}

impl Service for Wm {
    fn name(&self) -> &'static str {
        "Wm"
    }
    fn calls(&self) -> &'static [&'static str] {
        // ("focus" and "findpointer" are asked by no surface today but by the
        // shell's own shortcuts and Control Center: nobody else lists them)
        &["apps", "desktop", "wm", "spaces", "tiling", "stacks", "overview", "switcher", "focus", "findpointer", "decorations"]
    }
    fn ipc_targets(&self) -> &'static [&'static str] {
        &["tiling", "focus", "findpointer", "switcher"]
    }

    fn start(&mut self, h: &mut Host) {
        for s in ["hypr", "clients", "apps", "calendar", "todos", "weather", "mpris", "status"] {
            h.st.shell.want(s);
        }
        let send = h.sender("Wm");
        let send = Mutex::new(send);
        let post: Post = Arc::new(move |m| {
            if let Ok(s) = send.lock() {
                s(Box::new(m));
            }
        });
        self.io = Io::new(post.clone());
        follow_events(post);

        let mut io = self.io.take();
        self.stage.begin(h, &mut io);
        self.rules.begin(h, &mut io);
        self.deco.begin(h, &mut io);
        self.modes.begin(h, &mut io);
        self.feed.begin(&mut io);
        self.spaces.reconcile(&mut io, 250);
        self.apptheme.config_changed(h, &mut io);
        self.findp.config_changed(h);
        io.every("wm.watch", 1000);
        io.arm("wm.sendall", 800);
        self.io.restore(io);
    }

    fn call(&mut self, h: &mut Host, call: &str, args: &Value) {
        let list: Vec<Value> = match args {
            Value::Array(a) => a.clone(),
            Value::Null => vec![],
            other => vec![other.clone()],
        };
        match call {
            // the desktop surface's own: ["what", …]
            "desktop" => {
                let mut io = self.io.take();
                self.feed.call(h, &mut io, &list);
                self.io.restore(io);
            }
            "stacks" => {
                let mut a = vec![json!("stacks")];
                a.extend(list);
                let mut io = self.io.take();
                self.feed.call(h, &mut io, &a);
                self.io.restore(io);
            }
            // the window-management surfaces: ["spaces.go", …]
            "wm" => {
                let name = list.first().and_then(|v| v.as_str()).unwrap_or("").to_string();
                self.wm_call(h, &name, &list[list.len().min(1)..]);
            }
            other => self.wm_call(h, other, &list),
        }
    }

    fn ipc(&mut self, h: &mut Host, target: &str, func: &str, args: &[String]) -> Option<String> {
        match target {
            "tiling" => self.tiling.ipc(&mut self.io, func, args),
            "focus" => {
                let mut io = self.io.take();
                let r = self.modes.ipc(h, &mut io, func, args);
                self.io.restore(io);
                r
            }
            // qs ipc call findpointer find (to try it)
            "findpointer" => {
                if func == "find" {
                    self.findp.show(h, &mut self.io);
                }
                None
            }
            "switcher" => {
                match func {
                    "step" => self.wm_call(h, "switcher.step", &[json!(args.first().and_then(|d| d.parse::<i64>().ok()).unwrap_or(1))]),
                    "commit" => self.wm_call(h, "switcher.commit", &[]),
                    "cancel" => self.wm_call(h, "switcher.cancel", &[]),
                    _ => {}
                }
                None
            }
            _ => None,
        }
    }

    fn config_changed(&mut self, h: &mut Host) {
        let mut io = self.io.take();
        self.stage.config_changed(h, &mut io);
        self.rules.config_changed(h, &mut io);
        self.apptheme.config_changed(h, &mut io);
        self.deco.config_changed(h, &mut io);
        self.modes.config_changed(h);
        self.findp.config_changed(h);
        self.io.restore(io);
        self.spaces.resend(h); // (the always-shown count may have changed)
    }

    fn outputs_changed(&mut self, _h: &mut Host) {
        self.spaces.reconcile(&mut self.io, 250);
    }

    fn message(&mut self, h: &mut Host, msg: Box<dyn Any + Send>) {
        let Ok(m) = msg.downcast::<Msg>() else { return };
        match *m {
            Msg::Ev(name, data) => self.event(h, &name, &data),
            Msg::Tick(key) => self.tick(h, key),
            Msg::Run(f) => {
                let mut io = self.io.take();
                f(self, h, &mut io);
                self.io.restore(io);
            }
        }
    }

    fn quit(&mut self, h: &mut Host) {
        self.desk.restore_desktop();
        let _ = h;
    }
}

impl Io {
    /// The timers move out while something runs with `&mut Io` beside the service
    fn take(&mut self) -> Io {
        Io { post: self.post.clone(), timers: std::mem::take(&mut self.timers) }
    }
    fn restore(&mut self, other: Io) {
        // (timers armed meanwhile on `self` keep; the taken ones come back)
        for (k, t) in other.timers.0 {
            self.timers.0.entry(k).or_insert(t);
        }
    }
}

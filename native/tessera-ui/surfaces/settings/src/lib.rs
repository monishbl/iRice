//! Settings (rust.surfaces: settings): the window, its sidebar and search,
//! and every page, in both looks with Material's own, as
//! shell/modules/settings (Settings.qml's logic: history, search, paging,
//! finding a row; SettingsGlass.qml and SettingsMaterial.qml's looks; the
//! pages in pages/*.qml).
//!
//! The pieces: config.rs (config.json with Config.qml's keys and defaults,
//! written back atomically), map.rs (SettingsMap.js: sections, parts,
//! aliases), index.rs (what search finds, read from the QML page files as
//! Settings.qml does), model.rs (a page as blocks of rows and what each
//! control does), pages/ (one module per page group), ui/settings.slint (the
//! window) with ui/settings-kit.slint (the controls) and each page group's
//! own views (ui/settings-<group>.slint).
//!
//! How it opens: Panels.openSettings(page, target) in the QML shell, as
//! before (the shortcut, IPC, the menu bar's system menu, Quick Settings…).
//! shell/modules/settings/RustBridge.qml forwards Panels.settingsOpen,
//! .settingsPage and .settingsTarget while this draws it:
//!   {"cmd":"open","surface":"settings","screen":"DP-1","page":"dock","target":""}
//!   {"cmd":"page","surface":"settings","page":"…","target":"…"}
//!   {"cmd":"close","surface":"settings"}
//! and tells it what only the shell knows (`RustUi.tell("settings", {feed:
//! {<group>: …}})`) and a file chooser's answer (`{picked: {id, path}}`).
//! This asks back with `shell::ask("settings", [what, …])`: ["close"] (it
//! closed itself: Esc), ["page", id] (Panels.settingsPage follows), ["pick",
//! id, folder, title, start, filter] (a file chooser) and ["act", group,
//! what, …] (something only the shell does: a sign-in, a root helper's
//! patch, an install; the bridge's page group files answer).
//!
//! As the QML: a regular window (an xdg toplevel, `prepare_window`), which
//! the compositor places, stacks and resizes (a minimum of 640x480); Esc
//! or the compositor's close button closes it. Not as the QML: the mouse's
//! back and forward buttons aren't told apart (the platform reports other
//! buttons as one). Material's page cookie turns, but Material Symbols draw
//! outlined (Slint can't set the font's FILL axis).

pub mod ui {
    slint::include_modules!();
}
tessera_core::theme_global!(ui);

// tessera-core's modules (shell, wayland, theme, services, util…) under the
// names they had when everything was one crate: `crate::shell::Part`
#[allow(unused_imports)]
use tessera_core::*;

pub mod config;
pub mod index;
pub mod map;
pub mod model;
pub mod pages;

use tessera_core::shell::{self, Command as ShellCommand, Part};
use tessera_core::wayland::{State, Surface, WindowSpec};
use config::Cfg;
use model::{Arg, Ctx, Local, MenuItem, Models, Out, Page};
use serde_json::{json, Value};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

pub const NS: &str = "tessera:settings";
pub const NAME: &str = "settings";

/// Its own messages (Msg::Part("settings", …))
pub enum M {
    Act(String, Arg),
    Go(String),
    Query(String),
    QueryCommit,
    Back,
    Forward,
    Crumb,
    Menu(String, f32, f32, f32, f32),
    MenuPick(i32),
    MenuDismiss,
    Undo,
    ToastOver,
    Close,
    Drag(f32, f32),
    KeyNav(i32),
    /// a background job's result for page-local state
    Local(String, Value),
    Index(Vec<index::Entry>, bool),
    /// show the search target, once the page is drawn
    Reveal,
    Unflash,
    Picture(String, Option<slint::SharedPixelBuffer<slint::Rgb8Pixel>>),
}

struct Win {
    output: String,
    surface: Rc<Surface>,
    ui: ui::SettingsWindow,
    a11y: Option<crate::a11y::A11y>,
    /// where it is on its screen (its top-left corner, logical px)
    pos: (f32, f32),
    size: (u32, u32),
}

#[derive(Default)]
pub struct Settings {
    win: Option<Win>,
    open: bool,
    cfg: Cfg,
    page: String,
    back: Vec<String>,
    forward: Vec<String>,
    /// a row or part to show on the page (a search hit, an older link)
    target: String,
    flashing: String,
    query: String,
    index: Vec<index::Entry>,
    index_material: Option<bool>,
    results: Vec<index::Entry>,
    local: Local,
    feed: Value,
    actions: Vec<model::Action>,
    menus: HashMap<String, Vec<MenuItem>>,
    menu_id: String,
    toast_undo: Option<model::Action>,
    toast_timer: slint::Timer,
    reveal_timer: slint::Timer,
    flash_timer: slint::Timer,
    models: Models,
    built_page: String,
    reveal_tries: u32,
    pictures: HashMap<String, Option<slint::Image>>,
    decoding: std::collections::HashSet<String>,
    waiting: std::collections::VecDeque<(String, String, u32, u32)>,
    running: usize,
}

/// The window's size in each look (Settings.qml's implicit size)
pub fn window_size(material: bool) -> (u32, u32) {
    if material {
        (1040, 700)
    } else {
        (860, 620)
    }
}

/// Where a window of size `w`×`h` sits on a screen of `sw`×`sh`: centred,
/// or kept where it was moved to (inside the screen)
pub fn place(sw: f32, sh: f32, w: f32, h: f32, at: Option<(f32, f32)>) -> (f32, f32) {
    let (x, y) = at.unwrap_or(((sw - w) / 2.0, (sh - h) / 2.0));
    (x.clamp(0.0, (sw - w).max(0.0)).round(), y.clamp(0.0, (sh - h).max(0.0)).round())
}

/// The page an id names now (an older id goes where it lives), and the
/// part to show there
pub fn resolve(page: &str, target: &str) -> (String, String) {
    match map::alias(page) {
        Some((p, part)) => (p.to_string(), if target.is_empty() { part.to_string() } else { target.to_string() }),
        None => (if page.is_empty() { "appearance".to_string() } else { page.to_string() }, target.to_string()),
    }
}

/// Back and forward, like a browser: going to a page puts the one shown on
/// the back list (50 at most) and clears forward
pub fn visit(back: &mut Vec<String>, forward: &mut Vec<String>, shown: &str, to: &str) {
    if shown != to && !shown.is_empty() {
        back.push(shown.to_string());
        if back.len() > 50 {
            back.remove(0);
        }
        forward.clear();
    }
}

impl Settings {
    fn sender(st: &State) -> impl Fn(M) + Clone + 'static {
        let to = st.shell.sender(NAME);
        move |m: M| to(Box::new(m))
    }

    // ── The window ──────────────────────────────────────────────────────

    fn make_window(&mut self, st: &mut State, name: &str) {
        let size = window_size(st.shell.look.material);
        // (a regular window, an xdg toplevel: the compositor places, stacks
        // and resizes it, as the QML's FloatingWindow)
        let surface = st.prepare_window(WindowSpec {
            title: "Tessera Settings",
            app_id: "tessera-settings",
            namespace: NS,
            size,
            min_size: (640, 480),
        });
        let ui = ui::SettingsWindow::new().expect("SettingsWindow");
        st.shell.apply_theme(&ui.global::<ui::Theme>());
        ui.set_blocks(ModelRc::from(self.models.blocks.clone()));
        ui.set_cookie(crate::surfaces::shapes::outline("cookie9").into());
        let s = Self::sender(st);
        macro_rules! on {
            ($set:ident, |$($a:ident),*| $m:expr) => {{
                let s = s.clone();
                ui.$set(move |$($a),*| s($m));
            }};
            ($set:ident, $m:expr) => {{
                let s = s.clone();
                ui.$set(move || s($m));
            }};
        }
        on!(on_go, |p| M::Go(p.to_string()));
        on!(on_query, |q| M::Query(q.to_string()));
        on!(on_query_commit, M::QueryCommit);
        on!(on_back, M::Back);
        on!(on_forward, M::Forward);
        on!(on_crumb_clicked, M::Crumb);
        on!(on_menu_pick, |i| M::MenuPick(i));
        on!(on_menu_dismiss, M::MenuDismiss);
        on!(on_undo, M::Undo);
        on!(on_close_window, M::Close);
        on!(on_drag, |dx, dy| M::Drag(dx, dy));
        on!(on_key_nav, |d| M::KeyNav(d));
        {
            let bus = ui.global::<ui::SBus>();
            let s1 = s.clone();
            bus.on_act(move |id, i, f, t| s1(M::Act(id.to_string(), Arg { i, f, s: t.to_string(), id: id.to_string() })));
            let s2 = s.clone();
            bus.on_menu(move |id, x, y, w, h| s2(M::Menu(id.to_string(), x, y, w, h)));
        }
        ui.show().expect("show settings");
        let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
        self.win = Some(Win { output: name.to_string(), surface, ui, a11y, pos: (0.0, 0.0), size });
        self.models = Models::default();
        if let Some(w) = &self.win {
            w.ui.set_blocks(ModelRc::from(self.models.blocks.clone()));
        }
        self.built_page.clear();
    }

    fn drop_window(&mut self, st: &mut State) {
        if let Some(w) = self.win.take() {
            // (hidden first: Slint keeps a shown window, and with it the surface, alive)
            let _ = w.ui.hide();
            w.surface.hide();
            st.drop_surface(&w.surface);
        }
    }

    fn screen_size(st: &State, output: &str) -> (f32, f32) {
        st.output_named(output).and_then(|o| st.outputs.info(&o)).and_then(|i| i.logical_size).map_or((1920.0, 1080.0), |s| (s.0 as f32, s.1 as f32))
    }

    fn open(&mut self, st: &mut State, screen: Option<&str>, page: &str, target: &str) {
        let name = screen.filter(|s| !s.is_empty()).map(String::from).or_else(|| self.win.as_ref().map(|w| w.output.clone())).unwrap_or_default();
        let size = window_size(st.shell.look.material);
        if self.win.as_ref().is_some_and(|w| w.output != name || w.size != size) {
            self.drop_window(st);
        }
        let fresh = self.win.is_none();
        if fresh {
            self.make_window(st, &name);
        }
        if !self.open {
            // (opening again: the page's own state starts over, as the QML
            // window is made anew each time)
            self.local.retain(|k, _| !k.starts_with("more:"));
            self.query.clear();
            self.results.clear();
            if let Some(w) = &self.win {
                w.ui.invoke_clear_search();
            }
        }
        self.open = true;
        self.cfg.reload(st.shell.cfg.clone());
        if self.index.is_empty() || self.index_material != Some(st.shell.look.material) {
            self.read_index(st);
        }
        if !page.is_empty() || self.page.is_empty() {
            self.go(st, page, target, false);
        } else {
            self.target = target.to_string();
            self.rebuild(st);
        }
        self.sync_chrome(st);
    }

    fn close(&mut self, st: &mut State, say: bool) {
        if !self.open {
            return;
        }
        self.open = false;
        self.menu_id.clear();
        self.toast_timer.stop();
        self.toast_undo = None;
        if let Some(w) = &self.win {
            w.ui.set_menu_open(false);
            w.ui.set_toast_shown(false);
        }
        // (the window goes; the next open makes a fresh one, as the QML's)
        self.drop_window(st);
        if say {
            shell::ask("settings", json!(["close"]));
        }
    }

    fn read_index(&mut self, st: &State) {
        let material = st.shell.look.material;
        self.index_material = Some(material);
        let tx = st.tx.clone();
        std::thread::spawn(move || {
            let files = index::read_pages();
            let idx = index::build(&files, material);
            let _ = tx.send(crate::Msg::Part(NAME, Box::new(M::Index(idx, material))));
        });
    }

    // ── Paging ──────────────────────────────────────────────────────────

    /// Go to a page (an older id goes where it lives) and show `target` on it
    fn go(&mut self, st: &mut State, page: &str, target: &str, stepping: bool) {
        let (page, target) = resolve(page, target);
        if !stepping {
            visit(&mut self.back, &mut self.forward, &self.page.clone(), &page);
        }
        let changed = page != self.page;
        self.page = page.clone();
        self.target = target;
        if changed {
            shell::ask("settings", json!(["page", page]));
            self.local.retain(|k, _| !k.starts_with("more:"));
        }
        self.rebuild(st);
        self.sync_chrome(st);
    }

    fn step(&mut self, st: &mut State, forward: bool) {
        let to = if forward {
            if self.forward.is_empty() {
                return;
            }
            let to = self.forward.remove(0);
            self.back.push(self.page.clone());
            to
        } else {
            let Some(to) = self.back.pop() else { return };
            self.forward.insert(0, self.page.clone());
            to
        };
        self.go(st, &to, "", true);
    }

    /// The sidebar, the search results, the toolbar
    fn sync_chrome(&mut self, st: &State) {
        let Some(w) = &self.win else { return };
        let material = st.shell.look.material;
        let current = self.page.split('/').next().unwrap_or("").to_string();
        let mut last_group = "";
        let sections: Vec<ui::SSection> = map::SECTIONS
            .iter()
            .map(|s| {
                let group = if s.group != last_group { s.group } else { "" };
                last_group = s.group;
                ui::SSection { id: s.id.into(), name: s.name.into(), group: group.into(), icon: model::ic(s.icon), color: model::color(s.color), current: s.id == current }
            })
            .collect();
        w.ui.set_sections(ModelRc::new(VecModel::from(sections)));
        let results: Vec<ui::SResult> = self
            .results
            .iter()
            .map(|e| {
                let sec = map::section_of(&e.page);
                ui::SResult {
                    page: e.page.as_str().into(),
                    text: e.text.as_str().into(),
                    r#where: sec.map_or("", |s| s.name).into(),
                    icon: model::ic(sec.map_or("gear", |s| s.icon)),
                    color: sec.map_or(slint::Color::from_rgb_u8(0x8e, 0x8e, 0x93), |s| model::color(s.color)),
                    section: e.section,
                }
            })
            .collect();
        w.ui.set_results(ModelRc::new(VecModel::from(results)));
        w.ui.set_searching(!self.query.trim().is_empty());
        w.ui.set_section_id(current.as_str().into());
        w.ui.set_can_back(!self.back.is_empty());
        w.ui.set_can_forward(!self.forward.is_empty());
        let _ = material;
    }

    // ── The page ────────────────────────────────────────────────────────

    /// Build the page on show again (after a change, new data, another page)
    fn rebuild(&mut self, st: &mut State) {
        if !self.open || self.win.is_none() {
            return;
        }
        let material = st.shell.look.material;
        let mut out = Out::default();
        let mut page = Page::default();
        let page_id = self.page.clone();
        {
            let mut cx = Ctx {
                shell: &st.shell,
                cfg: &mut self.cfg,
                feed: &self.feed,
                local: &mut self.local,
                page: &page_id,
                material,
                dark: st.shell.look.dark,
                out: &mut out,
                tx: st.tx.clone(),
                screens: st.outputs_now().into_iter().map(|o| o.1).collect(),
                pictures: &self.pictures,
            };
            pages::build(&mut cx, &mut page);
        }
        // A row to show: open the More options it's in, then mark it
        let mut reveal = false;
        if !self.target.is_empty() {
            if let Some(key) = page.folded_key_of(&self.target) {
                self.local.insert(key, Value::Bool(true));
                return self.rebuild(st);
            }
            if self.target == self.flashing {
                page.mark(&self.target);
            } else {
                reveal = true;
            }
        }
        for s in std::mem::take(&mut out.want) {
            st.shell.want(s);
        }
        let fresh = self.built_page != page_id;
        self.built_page = page_id.clone();
        self.actions = std::mem::take(&mut out.actions);
        self.menus = model::menus(&page, &out.menus);
        self.models.apply(model::to_ui(&page, material), fresh);
        let sec = map::section_of(&page_id);
        let is_sub = page_id.contains('/');
        if let Some(w) = &self.win {
            let name = sec.map_or("", |s| s.name);
            w.ui.set_page_title(if is_sub { page.title.as_str() } else { name }.into());
            w.ui.set_crumb(name.into());
            w.ui.set_is_sub(is_sub);
            w.ui.set_spin(!self.cfg.b("appearance.reduceMotion"));
            w.ui.set_radius(self.cfg.f("appearance.windowRadius").clamp(0.0, 40.0) as f32);
            if fresh {
                w.ui.invoke_page_changed();
            }
        }
        self.after(st, out);
        if reveal {
            // (once it's laid out: mark the row, it scrolls itself there)
            self.reveal_tries = 0;
            let s = Self::sender(st);
            self.reveal_timer.start(slint::TimerMode::SingleShot, Duration::from_millis(80), move || s(M::Reveal));
        }
    }

    /// Pictures decode two at a time (a folder of wallpapers waits its turn)
    fn decode_next(&mut self, st: &State) {
        while self.running < 2 {
            let Some((key, path, w, h)) = self.waiting.pop_front() else { return };
            self.running += 1;
            let tx = st.tx.clone();
            std::thread::spawn(move || {
                let px = crate::picture::decode(&path, w.max(1), h.max(1));
                let _ = tx.send(crate::Msg::Part(NAME, Box::new(M::Picture(key, px))));
            });
        }
    }

    /// What a build or an action asked for besides its rows
    fn after(&mut self, st: &mut State, mut out: Out) {
        for job in std::mem::take(&mut out.pictures) {
            if self.decoding.insert(job.0.clone()) {
                self.waiting.push_back(job);
            }
        }
        self.decode_next(st);
        if let Some((text, undo)) = out.toast.take() {
            self.show_toast(st, &text, undo);
        }
        if let Some((id, folder, title, start, filter)) = out.pick.take() {
            shell::ask("settings", json!(["pick", id, folder, title, start, filter]));
        }
        if out.close {
            self.close(st, true);
        }
        if let Some((page, target)) = out.go.take() {
            self.go(st, &page, &target, false);
        }
    }

    fn act(&mut self, st: &mut State, id: &str, arg: Arg) {
        let Some(n) = id.strip_prefix('a').and_then(|n| n.parse::<usize>().ok()) else { return };
        let Some(f) = self.actions.get(n).cloned() else { return };
        self.run(st, f, arg);
    }

    fn run(&mut self, st: &mut State, f: model::Action, arg: Arg) {
        let material = st.shell.look.material;
        let mut out = Out::default();
        let page_id = self.page.clone();
        {
            let mut cx = Ctx {
                shell: &st.shell,
                cfg: &mut self.cfg,
                feed: &self.feed,
                local: &mut self.local,
                page: &page_id,
                material,
                dark: st.shell.look.dark,
                out: &mut out,
                tx: st.tx.clone(),
                screens: st.outputs_now().into_iter().map(|o| o.1).collect(),
                pictures: &self.pictures,
            };
            f(&mut cx, &arg);
        }
        let go = out.go.is_some();
        self.after(st, out);
        if !go {
            self.rebuild(st);
        }
    }

    fn show_toast(&mut self, st: &State, text: &str, undo: Option<model::Action>) {
        let Some(w) = &self.win else { return };
        w.ui.set_toast_text(text.into());
        w.ui.set_toast_undo(undo.is_some());
        w.ui.set_toast_shown(true);
        let ms = if undo.is_some() { 8000 } else { 5000 };
        self.toast_undo = undo;
        let s = Self::sender(st);
        self.toast_timer.start(slint::TimerMode::SingleShot, Duration::from_millis(ms), move || s(M::ToastOver));
    }

    fn search(&mut self, st: &State, q: &str) {
        self.query = q.to_string();
        self.results = index::search(&self.index, q);
        self.sync_chrome(st);
    }
}

impl Part for Settings {
    fn name(&self) -> &'static str {
        NAME
    }

    fn start(&mut self, st: &mut State) {
        self.cfg = Cfg::new(st.shell.cfg.clone());
    }

    fn command(&mut self, st: &mut State, c: &ShellCommand) {
        let a = &c.args;
        let s = |k: &str| a[k].as_str().unwrap_or("").to_string();
        match c.cmd.as_str() {
            "open" => self.open(st, c.screen.as_deref(), &s("page"), &s("target")),
            "close" => self.close(st, false),
            "page" => {
                let (page, target) = resolve(&s("page"), &s("target"));
                if page != self.page || !target.is_empty() {
                    self.go(st, &page, &target, false);
                }
            }
            other => eprintln!("tessera-ui: settings: no command {other:?}"),
        }
    }

    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        // From the shell (stdin): what only it knows, a chosen file
        let msg = match msg.downcast::<Value>() {
            Ok(v) => {
                if let Some(f) = v.get("feed").and_then(Value::as_object) {
                    if !self.feed.is_object() {
                        self.feed = json!({});
                    }
                    for (k, x) in f {
                        self.feed[k] = x.clone();
                    }
                    // Out of the way while you sign in to Google (the
                    // sign-in window and the browser would open behind it);
                    // back by itself when it's done, failed or cancelled
                    let aside = self.feed["accounts"]["signingIn"].as_bool().unwrap_or(false);
                    if let (true, Some(w)) = (self.open, &self.win) {
                        if aside && w.surface.mapped.get() {
                            w.surface.hide();
                        } else if !aside && !w.surface.mapped.get() {
                            w.surface.show(w.size.0, w.size.1);
                        }
                    }
                    self.rebuild(st);
                }
                if let Some(p) = v.get("picked") {
                    let (id, path) = (p["id"].as_str().unwrap_or(""), p["path"].as_str().unwrap_or(""));
                    if !path.is_empty() {
                        self.act(st, id, Arg { i: 1, f: 0.0, s: path.to_string(), id: id.to_string() });
                    }
                }
                return;
            }
            Err(m) => m,
        };
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Act(id, arg) => {
                if let Some(page) = id.strip_prefix("search:") {
                    // a search result: its page, and the row on it (not for a section)
                    let target = if arg.i == 1 { String::new() } else { arg.s.clone() };
                    let page = page.to_string();
                    self.go(st, &page, &target, false);
                    return;
                }
                if let Some(to) = id.strip_prefix("go:") {
                    let (page, target) = to.split_once('\t').unwrap_or((to, ""));
                    let (page, target) = (page.to_string(), target.to_string());
                    self.go(st, &page, &target, false);
                    return;
                }
                self.act(st, &id, arg);
            }
            M::Go(p) => self.go(st, &p, "", false),
            M::Query(q) => self.search(st, &q),
            M::QueryCommit => {
                if let Some(e) = self.results.first().cloned() {
                    let target = if e.section { String::new() } else { e.text.clone() };
                    self.go(st, &e.page, &target, false);
                }
            }
            M::Back => self.step(st, false),
            M::Forward => self.step(st, true),
            M::Crumb => {
                let sec = self.page.split('/').next().unwrap_or("").to_string();
                self.go(st, &sec, "", false);
            }
            M::Menu(id, x, y, w, h) => {
                let Some(items) = self.menus.get(&id) else { return };
                let Some(win) = &self.win else { return };
                let items: Vec<ui::SOpt> = items
                    .iter()
                    .map(|m| ui::SOpt { text: m.text.as_str().into(), on: m.checked, off: !m.enabled, style: if m.danger { 2 } else { 0 }, icon: model::ic(&m.icon), ..Default::default() })
                    .collect();
                win.ui.set_menu_items(ModelRc::new(VecModel::from(items)));
                win.ui.set_menu_x(x);
                win.ui.set_menu_y(y);
                win.ui.set_menu_w(w);
                win.ui.set_menu_h(h);
                win.ui.set_menu_id(id.as_str().into());
                win.ui.set_menu_open(true);
                self.menu_id = id;
            }
            M::MenuPick(i) => {
                let id = std::mem::take(&mut self.menu_id);
                if let Some(w) = &self.win {
                    w.ui.set_menu_open(false);
                }
                self.act(st, &id, Arg { i, f: 0.0, s: String::new(), id: id.clone() });
            }
            M::MenuDismiss => {
                self.menu_id.clear();
                if let Some(w) = &self.win {
                    w.ui.set_menu_open(false);
                }
            }
            M::Undo => {
                self.toast_timer.stop();
                if let Some(w) = &self.win {
                    w.ui.set_toast_shown(false);
                }
                if let Some(f) = self.toast_undo.take() {
                    self.run(st, f, Arg::default());
                }
            }
            M::ToastOver => {
                self.toast_undo = None;
                if let Some(w) = &self.win {
                    w.ui.set_toast_shown(false);
                }
            }
            M::Close => self.close(st, true),
            M::Drag(..) => {} // (the compositor moves the window)
            M::KeyNav(d) => {
                let cur = self.page.split('/').next().unwrap_or("");
                let i = map::SECTIONS.iter().position(|s| s.id == cur).unwrap_or(0) as i32;
                let j = (i + d).clamp(0, map::SECTIONS.len() as i32 - 1) as usize;
                if j as i32 != i {
                    let id = map::SECTIONS[j].id;
                    self.go(st, id, "", false);
                }
            }
            M::Local(k, v) => {
                self.local.remove(&format!("{k}.busy"));
                self.local.insert(k, v);
                self.rebuild(st);
            }
            M::Index(idx, material) => {
                if Some(material) == self.index_material {
                    self.index = idx;
                    if !self.query.is_empty() {
                        let q = self.query.clone();
                        self.search(st, &q);
                    }
                }
            }
            M::Reveal => {
                if self.target.is_empty() {
                    return;
                }
                self.flashing = self.target.clone();
                self.rebuild(st);
                self.target.clear();
                let s = Self::sender(st);
                self.flash_timer.start(slint::TimerMode::SingleShot, Duration::from_millis(1050), move || s(M::Unflash));
            }
            M::Unflash => {
                self.flashing.clear();
                self.rebuild(st);
            }
            M::Picture(key, px) => {
                self.decoding.remove(&key);
                self.running = self.running.saturating_sub(1);
                self.pictures.insert(key, px.map(slint::Image::from_rgb8));
                self.decode_next(st);
                self.rebuild(st);
            }
        }
    }

    fn output_removed(&mut self, st: &mut State, name: &str) {
        if self.win.as_ref().is_some_and(|w| w.output == name) {
            self.close(st, true);
            self.drop_window(st);
        }
    }

    fn config_changed(&mut self, st: &mut State, look: bool) {
        self.cfg.reload(st.shell.cfg.clone());
        if look {
            // (another look: the window's size changes; made anew)
            let was = self.open;
            let out = self.win.as_ref().map(|w| w.output.clone());
            if let Some(w) = &self.win {
                st.shell.apply_theme(&w.ui.global::<ui::Theme>());
            }
            if self.win.as_ref().is_some_and(|w| w.size != window_size(st.shell.look.material)) {
                self.open = false;
                self.drop_window(st);
                if was {
                    let page = self.page.clone();
                    self.open(st, out.as_deref(), &page, "");
                }
                return;
            }
            if self.index_material != Some(st.shell.look.material) && self.open {
                self.read_index(st);
            }
        }
        self.rebuild(st);
    }

    fn plugin_changed(&mut self, st: &mut State) {
        if let Some(w) = &self.win {
            st.shell.apply_theme(&w.ui.global::<ui::Theme>());
        }
    }

    fn data(&mut self, st: &mut State, _what: TypeId) {
        if self.open {
            self.rebuild(st);
        }
    }

    fn surface_closed(&mut self, st: &mut State, s: &Rc<Surface>) {
        if self.win.as_ref().is_some_and(|w| Rc::ptr_eq(&w.surface, s)) {
            self.close(st, true);
            self.drop_window(st);
        }
    }

    fn after_frame(&mut self, st: &mut State) {
        let kbd = st.keyboard_surface();
        let Some(w) = &mut self.win else { return };
        let f = kbd.as_ref().is_some_and(|k| Rc::ptr_eq(k, &w.surface));
        if let Some(a) = w.a11y.as_mut() {
            a.update(false, f);
        }
    }

    fn a11y(&mut self) -> Vec<&mut crate::a11y::A11y> {
        self.win.as_mut().and_then(|w| w.a11y.as_mut()).into_iter().collect()
    }

    fn stats(&self) {
        if let Some(w) = &self.win {
            eprintln!("tessera-ui: stats {}", w.surface.stats.borrow_mut().report("settings"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn older_ids_go_where_they_live() {
        assert_eq!(resolve("dock", ""), ("menubar".into(), "Dock".into()));
        assert_eq!(resolve("trackpad/gestures", ""), ("mouse/gestures".into(), "".into()));
        assert_eq!(resolve("dock", "Size"), ("menubar".into(), "Size".into()));
        assert_eq!(resolve("", ""), ("appearance".into(), "".into()));
        assert_eq!(resolve("about/health", ""), ("about/health".into(), "".into()));
    }

    #[test]
    fn history_like_a_browser() {
        let (mut b, mut f) = (Vec::new(), vec!["x".to_string()]);
        visit(&mut b, &mut f, "appearance", "dock");
        assert_eq!((b.clone(), f.len()), (vec!["appearance".to_string()], 0));
        visit(&mut b, &mut f, "dock", "dock");
        assert_eq!(b.len(), 1);
        for i in 0..60 {
            visit(&mut b, &mut f, &format!("p{i}"), "z");
        }
        assert_eq!(b.len(), 50);
    }

    #[test]
    fn the_window_sits_centred_and_stays_on_screen() {
        assert_eq!(window_size(false), (860, 620));
        assert_eq!(window_size(true), (1040, 700));
        assert_eq!(place(1920.0, 1080.0, 860.0, 620.0, None), (530.0, 230.0));
        assert_eq!(place(1920.0, 1080.0, 860.0, 620.0, Some((-50.0, 2000.0))), (0.0, 460.0));
    }
}

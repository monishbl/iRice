//! The App Store (rust.surfaces: appstore), "Software": a regular window
//! (an xdg toplevel, as the QML's FloatingWindow), compare
//! shell/modules/appstore/AppStoreWindow.qml. Search and the store's places
//! on the left (Discover, the categories, Installed), apps as cards on the
//! right, and an app's page over them: its logo, where it comes from,
//! Get / Open / Remove, the AUR's warning, its details. Material: the Play
//! Store's pill search over a navigation drawer, filter chips, cards on
//! surfaceContainer, the split Install button with its menu and "About this
//! app" as an Expressive segmented list (ui/appstore.slint).
//!
//! The window's own state is kept here, as the QML keeps it in the window:
//! the place (view, category), the search (300 ms after typing stops, at
//! least two letters; "aur: name" picks the AUR), the filter, the page, the
//! token that drops an answer for a place no longer shown. The store itself
//! stays the shell's (services/AppStore.qml, its appstore.py backend,
//! Software Update and flatpak): shell/modules/appstore/AppStoreFeed.qml
//! talks to it. Its lines on stdin (`{"to": "appstore", …}`):
//!   state   { open, catalogue, updates, jobs, app }  Panels.appStoreOpen, the
//!           catalogue notice (no catalogue, pacman there), Software Update
//!           running, AppStore.jobs (key -> { state, error }), Panels.appStoreApp
//!   answer  { token, apps, sections, error }  a place's apps, each with
//!           `_icon` (its logo's file: AppStore.iconFor) and `_source`
//!   changed { key, installed }                installed or removed
//!   show    app                               Spotlight's "Get": that app's page
//! and what it asks (`shell::ask("appstore", [what, …])`): load (token,
//! view, category, query, filter: the QML's AppStore.ask arguments), close,
//! install, open, remove (the app), catalogue, website (url), copy (name).
//!
//! Differences from the QML: the window is a fresh toplevel each time it
//! opens (as the QML's LazyLoader) but its app id is Tessera's own
//! ("tessera-appstore", not Quickshell's); Material's place icons and the
//! split button's glyphs draw Material Symbols in the font's default fill
//! (Slint sets no font axes); the AUR card's text and an app's description
//! don't follow the QML's line heights (Slint's StyledText); the Website
//! link isn't underlined on hover (Slint's Text has no underline); the page's
//! summary is one or two lines by an estimate of its length (Slint measures
//! wrapped text in a layout as one line); the page scrolls back to its top
//! for each app; Glass's press scale and Material's press swell are
//! approximations of ElasticControl; the split button's menu closes with
//! Escape only while the button has the keyboard (its items aren't reached
//! by the keyboard); Material's "About this app" rows cut a long detail (the
//! website) at its end, where the QML's wrap.

pub mod ui {
    slint::include_modules!();
}
tessera_core::theme_global!(ui);

// tessera-core's modules (shell, wayland, theme, services, util…) under the
// names they had when everything was one crate: `crate::shell::Part`
#[allow(unused_imports)]
use tessera_core::*;

pub mod logic;

use tessera_core::appicon::{self, Style};
use tessera_core::shell::Part;
use tessera_core::wayland::{State, Surface, WindowSpec};
use logic::{CardAct, FILTERS, PLACES};
use serde_json::{json, Value};
use slint::{ComponentHandle, Model, ModelRc, SharedString, StyledText, VecModel};
use std::any::{Any, TypeId};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::Duration;

const NAME: &str = "appstore";
const NS: &str = "tessera:appstore";

type Pixels = slint::SharedPixelBuffer<slint::Rgba8Pixel>;

enum M {
    Typed(String),
    Entered,
    Debounced,
    Pick(i32),
    Filter(i32),
    Open(i32, i32),
    Act(i32, i32),
    Back,
    Main,
    Remove,
    Website,
    MenuPick(i32),
    Catalogue,
    Image(String, Option<Pixels>),
}

/// What the shell's feed says
#[derive(Default)]
struct Feed {
    open: bool,
    catalogue_missing: bool,
    updates: bool,
    jobs: Value,
    app: Value,
}

struct Win {
    surface: Rc<Surface>,
    ui: ui::AppStoreWindow,
    a11y: Option<crate::a11y::A11y>,
}

#[derive(Default)]
pub struct AppStore {
    win: Option<Win>,
    feed: Feed,
    view: String,
    category: String,
    query: String,
    filter: String,
    detail: Option<Value>,
    token: i64,
    loading: bool,
    failed: String,
    apps: Vec<Value>,
    sections: Vec<(String, Vec<Value>)>,
    debounce: slint::Timer,
    images: HashMap<String, Option<slint::Image>>,
    pending: HashSet<String>,
}

fn glyphs() -> ui::AsGlyphs {
    let g = |n: &str| SharedString::from(crate::icons::glyph(n));
    ui::AsGlyphs {
        search: g("magnifying-glass"),
        x: g("x"),
        back: g("caret-left"),
        check: g("check"),
        info: g("info"),
        warning: g("warning"),
        package: g("package"),
        version: g("arrow-circle-up"),
        store: g("storefront"),
        globe: g("globe"),
        get: g("download-simple"),
        open: g("arrow-square-out"),
        trash: g("trash"),
        copy: g("copy"),
        caret_down: g("caret-down"),
        notch: g("circle-notch"),
    }
}

fn rgb(c: slint::Color) -> [u8; 3] {
    [c.red(), c.green(), c.blue()]
}

fn styled_plain(s: &str) -> StyledText {
    StyledText::from_plain_text(s)
}

fn styled_md(s: &str) -> StyledText {
    StyledText::from_markdown(s).unwrap_or_else(|_| StyledText::from_plain_text(s))
}

/// Rows into a model already in the window when it's a VecModel (changed
/// rows only: the cards under the pointer stay), else a new one
fn update<T: Clone + PartialEq + 'static>(current: &ModelRc<T>, rows: Vec<T>) -> Option<ModelRc<T>> {
    if let Some(vm) = current.as_any().downcast_ref::<VecModel<T>>() {
        if vm.row_count() == rows.len() {
            for (i, r) in rows.into_iter().enumerate() {
                if vm.row_data(i).as_ref() != Some(&r) {
                    vm.set_row_data(i, r);
                }
            }
        } else {
            vm.set_vec(rows);
        }
        return None;
    }
    Some(ModelRc::new(VecModel::from(rows)))
}

impl AppStore {
    fn material(st: &State) -> bool {
        st.shell.look.material
    }

    fn busy_with(&self, key: &str) -> String {
        self.feed.jobs[key]["state"].as_str().unwrap_or("").to_string()
    }

    fn error_for(&self, key: &str) -> String {
        self.feed.jobs[key]["error"].as_str().unwrap_or("").to_string()
    }

    // ── The window ───────────────────────────────────────────────────────

    fn open_window(&mut self, st: &mut State) {
        if self.win.is_some() {
            return;
        }
        // (logos the feed couldn't find: the icon theme by name)
        st.shell.want("icons");
        let m = Self::material(st);
        let surface = st.prepare_window(WindowSpec {
            title: "Software",
            app_id: "tessera-appstore",
            namespace: NS,
            size: if m { (1040, 720) } else { (980, 660) },
            min_size: if m { (820, 640) } else { (760, 500) },
        });
        let ui = ui::AppStoreWindow::new().expect("AppStoreWindow");
        st.shell.apply_theme(&ui.global::<ui::Theme>());
        ui.global::<ui::Symbols>().on_of(|g| crate::icons::symbol(&g).into());
        ui.set_g(glyphs());
        let to = st.shell.sender(NAME);
        macro_rules! on {
            ($set:ident, |$($a:ident),*| $m:expr) => {{
                let to = to.clone();
                ui.$set(move |$($a),*| to(Box::new($m)));
            }};
        }
        on!(on_typed, |t| M::Typed(t.to_string()));
        on!(on_entered, | | M::Entered);
        on!(on_pick, |i| M::Pick(i));
        on!(on_choose_filter, |i| M::Filter(i));
        on!(on_open_app, |s, i| M::Open(s, i));
        on!(on_act, |s, i| M::Act(s, i));
        on!(on_back, | | M::Back);
        on!(on_main, | | M::Main);
        on!(on_remove, | | M::Remove);
        on!(on_website, | | M::Website);
        on!(on_menu_pick, |i| M::MenuPick(i));
        on!(on_install_catalogue, | | M::Catalogue);
        ui.show().expect("show the App Store");
        let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
        self.win = Some(Win { surface, ui, a11y });

        // A fresh window: Discover (or the app Spotlight asked for)
        self.view = "discover".into();
        self.category.clear();
        self.query.clear();
        self.filter = "all".into();
        self.debounce.stop();
        self.detail = (!self.feed.app.is_null()).then(|| self.feed.app.clone());
        self.load(st);
        if self.detail.is_none() {
            if let Some(w) = &self.win {
                w.ui.invoke_focus_search();
            }
        }
    }

    fn close_window(&mut self, st: &mut State) {
        self.debounce.stop();
        if let Some(w) = self.win.take() {
            // (hidden first: Slint keeps a shown window, and with it the surface, alive)
            let _ = w.ui.hide();
            w.surface.hide();
            st.drop_surface(&w.surface);
        }
        self.images.clear();
        self.pending.clear();
        self.apps.clear();
        self.sections.clear();
        self.detail = None;
    }

    // ── Places and search (the QML's go / load / typed / entered) ────────

    fn go(&mut self, st: &mut State, view: &str, category: &str) {
        self.detail = None;
        self.view = view.into();
        self.category = category.into();
        self.load(st);
    }

    fn load(&mut self, st: &mut State) {
        self.token += 1;
        self.loading = true;
        self.failed.clear();
        self.apps.clear();
        self.sections.clear();
        crate::shell::ask(NAME, json!(["load", self.token, self.view, self.category, self.query, self.filter]));
        if let Some(w) = &self.win {
            w.ui.set_list_y(0.0);
        }
        self.push(st);
    }

    fn typed(&mut self, st: &mut State, text: String) {
        self.query = text;
        if logic::is_aur_query(&self.query) {
            self.filter = "aur".into();
        }
        let to = st.shell.sender(NAME);
        self.debounce.start(slint::TimerMode::SingleShot, Duration::from_millis(300), move || to(Box::new(M::Debounced)));
        self.push(st);
    }

    fn long_enough(&self) -> bool {
        self.query.trim().chars().count() >= 2
    }

    fn set_field(&mut self, text: &str) {
        if let Some(w) = &self.win {
            w.ui.set_search_text(text.into());
        }
    }

    /// The app at (section, index): a Discover section's, or the list's (-1)
    fn app_at(&self, s: i32, i: i32) -> Option<Value> {
        let i = usize::try_from(i).ok()?;
        if s < 0 {
            self.apps.get(i).cloned()
        } else {
            self.sections.get(s as usize).and_then(|(_, a)| a.get(i)).cloned()
        }
    }

    fn show_page(&mut self, st: &mut State, app: Value) {
        self.detail = Some(app);
        if let Some(w) = &self.win {
            w.ui.set_page_y(0.0);
            w.ui.set_menu_open(false);
        }
        self.push(st);
    }

    // ── Logos ────────────────────────────────────────────────────────────

    fn style(st: &State, t: &ui::Theme) -> Style {
        let themed = (rgb(t.get_m3_primary_container()), rgb(t.get_m3_on_primary_container()));
        Style::from_settings(&st.shell.cfg, st.shell.look.material, st.shell.look.dark, rgb(t.get_accent()), themed)
    }

    /// An app's logo at `size` (logical px), decoded off the UI thread
    /// (None until it's there, and for no logo)
    fn icon(&mut self, st: &State, app: &Value, size: f32) -> Option<slint::Image> {
        let path = logic::text(app, "_icon");
        let path = if path.is_empty() {
            // (the feed found no file, or the theme gave it as an image://
            // URL: AppStore.iconFor's steps here, the icon theme by its icon
            // name and (not for the AUR: themes fall back on dashes) its
            // package's, then its own)
            let names: Vec<String> = if app["source"].as_str() == Some("aur") { vec![logic::text(app, "iconName")] } else { vec![logic::text(app, "iconName"), logic::text(app, "pkg")] };
            let icons = st.shell.get::<crate::services::icons::Icons>();
            let found = names.iter().filter(|n| !n.is_empty() && !n.contains('/')).find_map(|n| icons.and_then(|i| i.exact(n)));
            match found {
                Some(p) => p.to_string_lossy().into_owned(),
                None => logic::text(app, "icon"),
            }
        } else {
            path
        };
        // (Flathub's logo by URL: fetched once into the cache, as Qt's Image does)
        let url = path.starts_with("https://").then(|| path.clone());
        let path = match &url {
            Some(u) => logic::cached_icon(u).to_string_lossy().into_owned(),
            None => path,
        };
        if !path.starts_with('/') {
            return None;
        }
        let w = self.win.as_ref()?;
        let px = (size * w.ui.window().scale_factor().max(1.0)).ceil() as u32;
        let key = format!("{path}@{px}");
        if let Some(i) = self.images.get(&key) {
            return i.clone();
        }
        if self.pending.insert(key.clone()) {
            let style = Self::style(st, &w.ui.global::<ui::Theme>());
            let to = st.shell.sender(NAME);
            std::thread::spawn(move || {
                if let Some(u) = url {
                    logic::fetch(&u, std::path::Path::new(&path));
                }
                let px_buf = appicon::render(std::path::Path::new(&path), px, &style, None);
                to(Box::new(M::Image(key, px_buf)));
            });
        }
        None
    }

    fn as_app(&mut self, st: &State, app: &Value, size: f32) -> ui::AsApp {
        let key = logic::text(app, "key");
        let name = logic::text(app, "name");
        let icon = self.icon(st, app, size);
        ui::AsApp {
            initial: logic::initial(&name).into(),
            name: name.into(),
            summary: logic::text(app, "summary").into(),
            source: logic::source_name(app).into(),
            installed: logic::truthy(&app["installed"]),
            busy: !self.busy_with(&key).is_empty(),
            has_icon: icon.is_some(),
            icon: icon.unwrap_or_default(),
        }
    }

    // ── Into the window ──────────────────────────────────────────────────

    fn push(&mut self, st: &State) {
        let Some(w) = &self.win else { return };
        let ui = w.ui.clone_strong();
        let m = Self::material(st);
        let (ww, _) = w.surface.logical_size();
        let view = self.view.clone();
        let filter = self.filter.clone();

        // The sidebar
        let places: Vec<ui::AsPlace> = PLACES
            .iter()
            .map(|p| ui::AsPlace { name: p.1.into(), icon: crate::icons::glyph(p.2).into(), selected: logic::is_on(&view, &self.category, p.0) })
            .collect();
        if let Some(n) = update(&ui.get_places(), places.clone()) {
            ui.set_places(n);
        }
        if let Some(n) = update(&ui.get_categories(), places[1..10].to_vec()) {
            ui.set_categories(n);
        }

        // The list
        ui.set_heading(logic::heading(&view, &self.category, &self.query).into());
        ui.set_subtitle(logic::subtitle(&view, &filter).into());
        ui.set_searching(view == "search");
        ui.set_filter(FILTERS.iter().position(|f| *f == filter).unwrap_or(0) as i32);
        ui.set_catalogue_missing(self.feed.catalogue_missing);
        ui.set_updates_running(self.feed.updates);
        ui.set_loading(self.loading);
        ui.set_loading_text(logic::loading_text(&view, &filter).into());
        ui.set_has_failed(!self.failed.is_empty());
        ui.set_failed_text(styled_plain(&logic::failed_text(&self.failed)));
        ui.set_empty(self.apps.is_empty() && self.sections.is_empty());
        ui.set_empty_title(logic::empty_title(&view, &self.query).into());
        ui.set_empty_help(styled_plain(logic::empty_help(&view, &filter)));
        ui.set_discover(view == "discover");
        let card = if m { 56.0 } else { 44.0 };
        let apps_now = self.apps.clone();
        let rows: Vec<ui::AsApp> = apps_now.iter().map(|a| self.as_app(st, a, card)).collect();
        if let Some(n) = update(&ui.get_apps(), rows) {
            ui.set_apps(n);
        }
        let sections_now = self.sections.clone();
        let secs: Vec<(String, Vec<ui::AsApp>)> = sections_now.iter().map(|(t, a)| (t.clone(), a.iter().map(|x| self.as_app(st, x, card)).collect())).collect();
        let current = ui.get_sections();
        let same = current.row_count() == secs.len() && current.as_any().downcast_ref::<VecModel<ui::AsSection>>().is_some();
        if same {
            for (i, (t, rows)) in secs.into_iter().enumerate() {
                let Some(mut s) = current.row_data(i) else { continue };
                let changed_title = s.title.as_str() != t;
                if let Some(n) = update(&s.apps, rows) {
                    s.apps = n;
                    s.title = t.into();
                    current.set_row_data(i, s);
                } else if changed_title {
                    s.title = t.into();
                    current.set_row_data(i, s);
                }
            }
        } else {
            let model: Vec<ui::AsSection> = secs.into_iter().map(|(t, rows)| ui::AsSection { title: t.into(), apps: ModelRc::new(VecModel::from(rows)) }).collect();
            ui.set_sections(ModelRc::new(VecModel::from(model)));
        }

        // An app's page
        ui.set_detail(self.detail.is_some());
        if let Some(app) = self.detail.clone() {
            let key = logic::text(&app, "key");
            let busy = self.busy_with(&key);
            let installed = logic::truthy(&app["installed"]);
            let d = self.as_app(st, &app, if m { 112.0 } else { 96.0 });
            // (the page: min(content - 64, 720), less the logo and the gap)
            let sidebar = if m { 272.0 } else { 230.0 };
            let text_w = ((ww as f32 - sidebar - 64.0).min(720.0) - if m { 136.0 } else { 116.0 }).max(1.0);
            ui.set_d_summary_two(logic::two_lines(&d.summary, text_w, if m { 16.0 } else { 15.0 }));
            ui.set_d_app(d);
            ui.set_d_chip(logic::chip_text(&app).into());
            ui.set_d_developer(logic::developer_line(&app).into());
            ui.set_d_busy(!busy.is_empty());
            ui.set_d_main_label(logic::main_label(&busy, installed, m).into());
            ui.set_d_main_icon(crate::icons::glyph(if installed { "arrow-square-out" } else { "download-simple" }).into());
            ui.set_d_remove_label(logic::remove_label(&busy).into());
            ui.set_d_error(self.error_for(&key).into());
            ui.set_d_note(!busy.is_empty() && app["source"].as_str() != Some("flathub"));
            ui.set_d_aur(app["source"].as_str() == Some("aur"));
            ui.set_d_aur_text(styled_md(&logic::aur_text(&app)));
            let desc = logic::description(&app);
            ui.set_d_has_description(!desc.is_empty());
            ui.set_d_description(styled_plain(&desc));
            ui.set_d_package_line(logic::package_line(&app).into());
            let home = logic::text(&app, "homepage");
            ui.set_d_has_homepage(!home.is_empty());
            ui.set_d_homepage(home.as_str().into());
            ui.set_d_pkg(logic::text(&app, "pkg").into());
            ui.set_d_version(logic::text(&app, "version").into());
            ui.set_d_from(logic::from_text(&app).into());
            let g = |n: &str| SharedString::from(crate::icons::glyph(n));
            let mut menu = Vec::new();
            if installed {
                menu.push(ui::AsMenuItem { text: "Remove".into(), icon: g("trash"), danger: true, enabled: busy.is_empty(), id: 0 });
            }
            if !home.is_empty() {
                menu.push(ui::AsMenuItem { text: "Open the website".into(), icon: g("globe"), danger: false, enabled: true, id: 1 });
            }
            menu.push(ui::AsMenuItem { text: "Copy the name".into(), icon: g("copy"), danger: false, enabled: true, id: 2 });
            ui.set_menu_items(ModelRc::new(VecModel::from(menu)));
        } else {
            ui.set_menu_open(false);
        }
    }

    // ── The feed ─────────────────────────────────────────────────────────

    fn said(&mut self, st: &mut State, v: Value) {
        if let Some(s) = v.get("state") {
            let was = self.feed.open;
            self.feed.open = s["open"].as_bool().unwrap_or(false);
            self.feed.catalogue_missing = s["catalogue"].as_bool().unwrap_or(false);
            self.feed.updates = s["updates"].as_bool().unwrap_or(false);
            self.feed.jobs = s["jobs"].clone();
            self.feed.app = s["app"].clone();
            if self.feed.open && (!was || self.win.is_none()) {
                self.open_window(st);
            } else if !self.feed.open && self.win.is_some() {
                self.close_window(st);
            } else {
                self.push(st);
            }
        }
        if let Some(a) = v.get("answer") {
            if a["token"].as_i64() != Some(self.token) {
                return; // (a place no longer shown)
            }
            self.loading = false;
            self.failed = a["error"].as_str().unwrap_or("").to_string();
            self.apps = a["apps"].as_array().cloned().unwrap_or_default();
            self.sections = a["sections"]
                .as_array()
                .map(|l| l.iter().map(|s| (logic::text(s, "title"), s["apps"].as_array().cloned().unwrap_or_default())).collect())
                .unwrap_or_default();
            self.push(st);
        }
        if let Some(c) = v.get("changed") {
            let key = c["key"].as_str().unwrap_or("").to_string();
            let installed = c["installed"].as_bool().unwrap_or(false);
            let mark = |a: &mut Value| {
                if a["key"].as_str() == Some(&key) {
                    a["installed"] = json!(installed);
                }
            };
            if let Some(d) = self.detail.as_mut() {
                mark(d);
            }
            self.apps.iter_mut().for_each(mark);
            self.sections.iter_mut().for_each(|(_, l)| l.iter_mut().for_each(mark));
            if self.view == "installed" {
                self.load(st);
            } else {
                self.push(st);
            }
        }
        if let Some(app) = v.get("show") {
            if !app.is_null() && self.win.is_some() {
                self.show_page(st, app.clone());
            }
        }
    }
}

impl Part for AppStore {
    fn name(&self) -> &'static str {
        NAME
    }

    fn config_changed(&mut self, st: &mut State, look: bool) {
        if look {
            if let Some(w) = &self.win {
                st.shell.apply_theme(&w.ui.global::<ui::Theme>());
            }
            // (logos are drawn in the look's style)
            self.images.clear();
            self.pending.clear();
        }
        self.push(st);
    }

    fn data(&mut self, st: &mut State, what: TypeId) {
        if what == TypeId::of::<crate::services::icons::Icons>() && self.win.is_some() {
            self.push(st);
        }
    }

    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        let msg = match msg.downcast::<Value>() {
            Ok(v) => return self.said(st, *v),
            Err(m) => m,
        };
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Typed(t) => self.typed(st, t),
            M::Entered => {
                self.debounce.stop();
                if self.long_enough() {
                    self.go(st, "search", "");
                }
            }
            M::Debounced => {
                if self.long_enough() {
                    self.go(st, "search", "");
                } else if self.view == "search" {
                    self.go(st, "discover", "");
                }
            }
            M::Pick(i) => {
                let (view, cat) = logic::view_for(i.max(0) as usize);
                self.set_field("");
                self.query.clear();
                self.debounce.stop();
                self.go(st, view, cat);
            }
            M::Filter(i) => {
                let f = FILTERS[(i.max(0) as usize).min(FILTERS.len() - 1)];
                self.filter = f.into();
                if f != "aur" && logic::is_aur_query(&self.query) {
                    // (the field loses its "aur:", as typing it would)
                    let q = logic::strip_aur(&self.query);
                    self.set_field(&q);
                    self.typed(st, q);
                }
                self.load(st);
            }
            M::Open(s, i) => {
                if let Some(app) = self.app_at(s, i) {
                    self.show_page(st, app);
                }
            }
            M::Act(s, i) => {
                let Some(app) = self.app_at(s, i) else { return };
                match logic::card_act(&app) {
                    CardAct::Open => crate::shell::ask(NAME, json!(["open", app])),
                    CardAct::Page => self.show_page(st, app),
                    CardAct::Install => crate::shell::ask(NAME, json!(["install", app])),
                }
            }
            M::Back => {
                self.detail = None;
                self.push(st);
            }
            M::Main => {
                if let Some(app) = &self.detail {
                    let what = if logic::truthy(&app["installed"]) { "open" } else { "install" };
                    crate::shell::ask(NAME, json!([what, app]));
                }
            }
            M::Remove => {
                if let Some(app) = &self.detail {
                    crate::shell::ask(NAME, json!(["remove", app]));
                }
            }
            M::Website => {
                if let Some(app) = &self.detail {
                    let url = logic::text(app, "homepage");
                    if !url.is_empty() {
                        crate::shell::ask(NAME, json!(["website", url]));
                    }
                }
            }
            M::MenuPick(id) => {
                let Some(app) = self.detail.clone() else { return };
                match id {
                    0 if self.busy_with(&logic::text(&app, "key")).is_empty() => crate::shell::ask(NAME, json!(["remove", app])),
                    1 => {
                        let url = logic::text(&app, "homepage");
                        if !url.is_empty() {
                            crate::shell::ask(NAME, json!(["website", url]));
                        }
                    }
                    2 => crate::shell::ask(NAME, json!(["copy", logic::text(&app, "name")])),
                    _ => {}
                }
            }
            M::Catalogue => crate::shell::ask(NAME, json!(["catalogue"])),
            M::Image(key, px) => {
                self.pending.remove(&key);
                self.images.insert(key, px.map(slint::Image::from_rgba8));
                self.push(st);
            }
        }
    }

    /// Closed by the compositor (Super+Q, its close button): Panels follows
    fn surface_closed(&mut self, st: &mut State, s: &Rc<Surface>) {
        if self.win.as_ref().is_some_and(|w| Rc::ptr_eq(&w.surface, s)) {
            self.close_window(st);
            self.feed.open = false;
            crate::shell::ask(NAME, json!(["close"]));
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
            eprintln!("tessera-ui: stats {}", w.surface.stats.borrow_mut().report("appstore"));
        }
    }

    fn quit(&mut self, st: &mut State) {
        self.close_window(st);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn models_change_in_place() {
        let m = ModelRc::new(VecModel::from(vec![1, 2, 3]));
        assert!(update(&m, vec![1, 5, 3]).is_none());
        assert_eq!(m.row_data(1), Some(5));
        assert!(update(&m, vec![7]).is_none());
        assert_eq!(m.row_count(), 1);
        // (not a VecModel: a new one)
        let empty: ModelRc<i32> = ModelRc::default();
        assert_eq!(update(&empty, vec![4]).map(|n| n.row_count()), Some(1));
    }

    #[test]
    fn jobs_say_whats_busy() {
        let mut a = AppStore::default();
        a.feed.jobs = json!({"flathub:org.example.Inkpad": {"state": "installing"}, "repo:sketchy": {"state": "", "error": "Software Update is busy. Try again when it's done."}});
        assert_eq!(a.busy_with("flathub:org.example.Inkpad"), "installing");
        assert_eq!(a.busy_with("repo:sketchy"), "");
        assert_eq!(a.error_for("repo:sketchy"), "Software Update is busy. Try again when it's done.");
        assert_eq!(a.busy_with("nothing"), "");
    }

    #[test]
    fn the_app_at_a_card() {
        let mut a = AppStore::default();
        a.apps = vec![json!({"key": "a"}), json!({"key": "b"})];
        a.sections = vec![("Popular".into(), vec![json!({"key": "c"})])];
        assert_eq!(a.app_at(-1, 1).unwrap()["key"], "b");
        assert_eq!(a.app_at(0, 0).unwrap()["key"], "c");
        assert!(a.app_at(0, 3).is_none());
        assert!(a.app_at(-1, -1).is_none());
    }
}

//! The app drawer (rust.surfaces: launchpad; Super+Ctrl+Space, the dock's
//! Apps button, the four-finger pinch), both looks: Glass's App Library
//! (shell/modules/launchpad/AppLibraryGlass.qml, a centred glass dialog over
//! a light scrim) and Material's app drawer (AppDrawer.qml, a large surface
//! rising from the dock on a spring, a search pill, a connected button group
//! and the More menu). Every app, in category folders or A–Z with a letter
//! index (model.rs, AppLibraryModel.qml): typing searches; the arrows move;
//! Enter opens; Tab switches views; Escape clears the search, steps out of a
//! folder, then closes. Right-click, press and hold, or the Menu key (or
//! Shift+F10): the app's menu (its own shortcuts, Open, Keep in Dock).
//!
//! It opens through Panels ("launchpad"), as the QML one did. Keep in Dock,
//! Get apps and the settings are the shell's (`wm` apps.pin / apps.unpin,
//! panels.openAppStore, panels.openSettings).
//!
//! Not like the QML yet:
//! - the scroll to a letter or the selection is an eased glide in both looks
//!   (Material's is a spring in QML);
//! - Material's press swell is a scale on the icon (ElasticControl);
//! - no elastic pull on the Glass dialog (ElasticGrab);
//! - no tooltips on the back and clear buttons.

pub mod ui {
    slint::include_modules!();
}
tessera_core::theme_global!(ui);

// tessera-core's modules (shell, wayland, theme, services, util…) under the
// names they had when everything was one crate: `crate::shell::Part`
#[allow(unused_imports)]
use tessera_core::*;

pub mod model;

use tessera_core::services::apps::{self, Apps};
use tessera_core::services::icons::Icons;
use tessera_core::shell::{Command, Part};
use tessera_core::wm::{self, AppIcons, IconsDone};
use tessera_core::theme;
use tessera_core::wayland::{Spec, State, Surface};
use model::{Activated, App, Lib};
use slint::platform::Key;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use smithay_client_toolkit::shell::WaylandSurface;
use std::any::Any;
use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

const NAME: &str = "launchpad";
pub const NS: &str = "tessera:launchpad";

/// Config.qml's dock.pinned
const PINNED: &[&str] = &["org.kde.dolphin", "thunar", "firefox", "code", "foot", "org.gnome.Calculator", "org.xfce.mousepad"];

enum M {
    Key(String, bool),
    Edited(String),
    Hovered(i32),
    Activate(i32),
    CellMenu(i32, f32, f32),
    CellDown(i32, f32, f32),
    CellUp,
    Hold,
    Back,
    View(i32),
    Jump(i32),
    FabToggle,
    FabPick(i32),
    MenuActivate(i32),
    MenuDismissed,
    Dismissed,
    SheetClicked,
    Closed,
    Cats(Vec<App>),
    Scroll,
    IntroEnd,
}

#[derive(Clone, Copy, Default)]
struct KeyCtx {
    has_text: bool,
    menu: bool,
}

fn is(text: &str, k: Key) -> bool {
    text == SharedString::from(k).as_str()
}

/// The keys the field leaves to the drawer
fn handles(c: KeyCtx, text: &str, shift: bool) -> bool {
    if c.menu {
        return is(text, Key::Escape) || is(text, Key::Return) || is(text, Key::UpArrow) || is(text, Key::DownArrow) || text == " ";
    }
    is(text, Key::Escape)
        || is(text, Key::Return)
        || is(text, Key::Tab)
        || is(text, Key::UpArrow)
        || is(text, Key::DownArrow)
        || is(text, Key::Menu)
        || (shift && is(text, Key::F10))
        || (!c.has_text && (is(text, Key::LeftArrow) || is(text, Key::RightArrow)))
}

enum Do {
    Action(String, String),
    Open(String),
    Pin(String),
    Unpin(String),
}

struct MenuItem {
    text: String,
    header: bool,
    act: Option<Do>,
}

struct Menu {
    items: Vec<MenuItem>,
    x: f32,
    y: f32,
    current: i32,
}

struct Win {
    output: String,
    surface: Rc<Surface>,
    ui: ui::LaunchpadPanel,
    a11y: Option<crate::a11y::A11y>,
    glass_sent: String,
}

/// The grid's glide: from, to, when it started
struct Glide {
    from: f32,
    to: f32,
    t0: Instant,
}

#[derive(Default)]
pub struct Launchpad {
    win: Option<Win>,
    open: bool,
    lib: Lib,
    text: String,
    key_ctx: Rc<Cell<KeyCtx>>,
    icons: AppIcons,
    menu: Option<Menu>,
    fab: bool,
    page: String,
    held: Option<(i32, f32, f32)>,
    hold_fired: bool,
    glide: Option<Glide>,
    timers: [slint::Timer; 3],
}

fn send(st: &State) -> impl Fn(Box<dyn Any + Send>) + Clone + 'static {
    st.shell.sender(NAME)
}

impl Launchpad {
    fn window(&mut self, st: &mut State, output: &str) -> bool {
        if self.win.as_ref().is_some_and(|w| w.output != output) {
            let w = self.win.take().unwrap();
            crate::services::hypr::Glass::clear_now(&w.output, NS);
            st.drop_surface(&w.surface);
        }
        if self.win.is_some() {
            return true;
        }
        let Some(wl_output) = st.output_named(output) else { return false };
        let surface = st.prepare_surface(Spec {
            layer: Layer::Overlay,
            namespace: NS,
            output: Some((&wl_output, output)),
            anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
            size: (0, 0),
            exclusive: -1,
            keyboard: KeyboardInteractivity::None,
            margin_left: 0,
        });
        let ui = ui::LaunchpadPanel::new().expect("LaunchpadPanel");
        st.shell.apply_theme(&ui.global::<ui::Theme>());
        let to = send(st);
        {
            let (to, ctx) = (to.clone(), self.key_ctx.clone());
            ui.on_key(move |t, shift| {
                let ok = handles(ctx.get(), &t, shift);
                if ok {
                    to(Box::new(M::Key(t.to_string(), shift)));
                }
                ok
            });
        }
        macro_rules! on {
            ($set:ident, |$($a:ident),*| $m:expr) => {{
                let to = to.clone();
                ui.$set(move |$($a),*| to(Box::new($m)));
            }};
        }
        on!(on_edited, |t| M::Edited(t.to_string()));
        on!(on_hovered, |i| M::Hovered(i));
        on!(on_activate, |i| M::Activate(i));
        on!(on_cell_menu, |i, x, y| M::CellMenu(i, x, y));
        on!(on_cell_down, |i, x, y| M::CellDown(i, x, y));
        on!(on_cell_up, | | M::CellUp);
        on!(on_back, | | M::Back);
        on!(on_view_chosen, |i| M::View(i));
        on!(on_jump, |k| M::Jump(k));
        on!(on_fab_toggle, | | M::FabToggle);
        on!(on_fab_pick, |k| M::FabPick(k));
        on!(on_menu_activate, |i| M::MenuActivate(i));
        on!(on_menu_dismissed, | | M::MenuDismissed);
        on!(on_dismissed, | | M::Dismissed);
        on!(on_sheet_clicked, | | M::SheetClicked);
        on!(on_closed, | | M::Closed);
        ui.show().expect("show launchpad");
        let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
        self.win = Some(Win { output: output.into(), surface, ui, a11y, glass_sent: String::new() });
        self.look(st);
        true
    }

    fn look(&mut self, st: &State) {
        let Some(w) = &self.win else { return };
        let m = st.shell.look.material;
        let ui = &w.ui;
        ui.set_g_search(wglyph("magnifying-glass", m, false));
        ui.set_g_back(wglyph("caret-left", m, false));
        ui.set_g_clear(wglyph("x", m, false));
        ui.set_g_x(wglyph("x", m, false));
        ui.set_g_dots(wglyph("dots-three", m, false));
        ui.set_cookie9(crate::surfaces::shapes::outline("cookie9").into());
        ui.set_glass_fill(crate::wm::glass_fill(st.shell.plugin, st.shell.look.dark));
        ui.set_scrim(ui.global::<ui::Theme>().get_m3_scrim());
        ui.set_app_names(st.shell.cfg["dock"]["appNames"].as_bool().unwrap_or(true));
        let t = ui.global::<ui::Theme>();
        self.icons.restyle(st, t.get_accent(), t.get_m3_primary_container(), t.get_m3_on_primary_container());
    }

    fn open(&mut self, st: &mut State, screen: Option<String>) {
        let output = screen.or_else(|| self.win.as_ref().map(|w| w.output.clone())).or_else(|| st.outputs_now().first().map(|o| o.1.clone()));
        let Some(output) = output else { return };
        if !self.window(st, &output) {
            return;
        }
        self.open = true;
        self.lib.reset();
        self.text.clear();
        self.menu = None;
        self.fab = false;
        self.glide = None;
        self.key_ctx.set(KeyCtx::default());
        let w = self.win.as_ref().unwrap();
        w.ui.invoke_open();
        w.ui.set_scroll_y(0.0);
        w.ui.set_shown(true);
        w.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        st.set_input_region(&w.surface, None);
        w.surface.show(0, 0);
        self.page.clear();
        self.push(st);
    }

    fn close(&mut self, st: &mut State, tell: bool) {
        if !self.open {
            return;
        }
        self.open = false;
        self.menu = None;
        self.fab = false;
        if let Some(w) = &self.win {
            w.ui.set_shown(false);
            w.ui.set_menu_shown(false);
            w.ui.set_fab_open(false);
            w.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::None);
            st.set_input_region(&w.surface, Some(&[]));
            w.surface.layer.commit();
        }
        if tell {
            crate::shell::closed(NAME);
        }
    }

    fn closed(&mut self, st: &mut State) {
        if self.open {
            return;
        }
        if let Some(w) = &mut self.win {
            w.surface.hide();
            st.shell.glass.set(&w.output, NS, &[]);
            w.glass_sent.clear();
        }
    }

    fn pinned(st: &State) -> Vec<String> {
        match st.shell.cfg["dock"]["pinned"].as_array() {
            Some(a) => a.iter().filter_map(|x| x.as_str().map(String::from)).collect(),
            None => PINNED.iter().map(|s| s.to_string()).collect(),
        }
    }

    /// Everything into the window
    fn push(&mut self, st: &mut State) {
        let Some(w) = &self.win else { return };
        let m = st.shell.look.material;
        let scale = w.ui.window().scale_factor().max(1.0);
        let apps_view = self.lib.showing_apps();
        let cells: Vec<ui::LpCell> = if apps_view {
            self.lib
                .visible()
                .into_iter()
                .map(|i| {
                    let a = &self.lib.apps[i];
                    let icon = self.icons.get(st, &a.id, 56.0, scale, true);
                    ui::LpCell { name: a.name.as_str().into(), has_icon: icon.is_some(), icon: icon.unwrap_or_default(), folder: false, count: 0, previews: ModelRc::default(), letter: model::letter_at(&self.lib.letters, i).into() }
                })
                .collect()
        } else {
            let folders = self.lib.folders.clone();
            folders
                .iter()
                .map(|f| {
                    let n = f.apps.len();
                    let shown: Vec<(usize, f32)> = if m {
                        f.apps.iter().take(4).map(|&i| (i, 60.0)).collect()
                    } else if n > 4 {
                        f.apps.iter().take(3).map(|&i| (i, 64.0)).chain(f.apps.iter().skip(3).take(4).map(|&i| (i, 27.0))).collect()
                    } else {
                        f.apps.iter().map(|&i| (i, 64.0)).collect()
                    };
                    let previews: Vec<slint::Image> = shown.into_iter().map(|(i, s)| self.icons.get(st, &self.lib.apps[i].id, s, scale, true).unwrap_or_default()).collect();
                    ui::LpCell { name: f.name.as_str().into(), icon: Default::default(), has_icon: false, folder: true, count: n as i32, previews: ModelRc::new(VecModel::from(previews)), letter: Default::default() }
                })
                .collect()
        };
        self.icons.flush(send(st));
        let letters: Vec<ui::LpLetter> = self.lib.letters.iter().enumerate().map(|(k, (l, i))| ui::LpLetter { letter: l.as_str().into(), index: *i as i32, next: self.lib.letters.get(k + 1).map(|x| x.1 as i32).unwrap_or(i32::MAX) }).collect();
        let w = self.win.as_ref().unwrap();
        let ui = &w.ui;
        let none = apps_view && !self.lib.query.is_empty() && cells.is_empty();
        ui.set_cells(ModelRc::new(VecModel::from(cells)));
        ui.set_apps(apps_view);
        ui.set_selected(self.lib.selected as i32);
        ui.set_picked(self.lib.picked.as_str().into());
        ui.set_letters(ModelRc::new(VecModel::from(letters)));
        ui.set_alphabetical(self.lib.alphabetical());
        let in_folder = self.lib.folder.is_some() && self.lib.query.is_empty();
        ui.set_in_folder(in_folder);
        let f = self.lib.folder.and_then(|f| self.lib.folders.get(f));
        ui.set_folder_name(f.map(|f| f.name.clone()).unwrap_or_default().into());
        ui.set_folder_count(f.map(|f| format!("{} {}", f.apps.len(), if f.apps.len() == 1 { "app" } else { "apps" })).unwrap_or_default().into());
        ui.set_view(self.lib.view);
        ui.set_app_count(self.lib.apps.len() as i32);
        ui.set_no_results(none);
        let toggle = if self.lib.view == 1 { ("Show folders", "squares-four") } else { ("Show A to Z", "list") };
        let fab = [("Get apps", "storefront"), toggle, ("Dock and drawer settings", "gear")];
        ui.set_fab_items(ModelRc::new(VecModel::from(fab.iter().map(|(t, g)| ui::LpFab { text: (*t).into(), glyph: wglyph(g, m, false) }).collect::<Vec<_>>())));
        ui.set_fab_open(self.fab);
        match &self.menu {
            Some(mn) => {
                let rows: Vec<ui::WMenuRow> = mn.items.iter().map(|i| ui::WMenuRow { text: i.text.as_str().into(), header: i.header, separator: false, enabled: true }).collect();
                let wdt = 200f32.max(mn.items.iter().map(|i| i.text.chars().count()).max().unwrap_or(0) as f32 * 7.5 + 40.0);
                ui.set_menu_rows(ModelRc::new(VecModel::from(rows)));
                ui.set_menu_w(wdt);
                ui.set_menu_h(mn.items.iter().map(|i| if i.header { 26.0 } else { 28.0 }).sum());
                ui.set_menu_x(mn.x + wdt / 2.0);
                ui.set_menu_y(mn.y);
                ui.set_menu_current(mn.current);
                ui.set_menu_shown(true);
            }
            None => ui.set_menu_shown(false),
        }
        // What the grid shows changed: its cells come in one after another (Material)
        let page = self.lib.page();
        if page != self.page {
            self.page = page;
            ui.set_scroll_y(0.0);
            self.glide = None;
            if m {
                ui.invoke_start_intro();
                ui.set_entering(true);
                let to = send(st);
                self.timers[1].start(slint::TimerMode::SingleShot, Duration::from_millis(500), move || to(Box::new(M::IntroEnd)));
            }
        }
        self.key_ctx.set(KeyCtx { has_text: !self.text.is_empty(), menu: self.menu.is_some() });
    }

    /// Glide the grid to `y` (the top of the view, in content px)
    fn scroll_to(&mut self, st: &State, y: f32) {
        let Some(w) = &self.win else { return };
        let max = (w.ui.get_content_h() - w.ui.get_view_h()).max(0.0);
        let to = y.clamp(0.0, max);
        let from = -w.ui.get_scroll_y();
        if (to - from).abs() < 0.5 {
            return;
        }
        self.glide = Some(Glide { from, to, t0: Instant::now() });
        let tx = send(st);
        self.timers[2].start(slint::TimerMode::Repeated, Duration::from_millis(16), move || tx(Box::new(M::Scroll)));
    }

    fn glide_step(&mut self) {
        let Some(g) = &self.glide else {
            self.timers[2].stop();
            return;
        };
        let t = (g.t0.elapsed().as_secs_f32() / 0.32).min(1.0);
        let e = 1.0 - (1.0 - t).powi(3);
        if let Some(w) = &self.win {
            w.ui.set_scroll_y(-(g.from + (g.to - g.from) * e));
        }
        if t >= 1.0 {
            self.glide = None;
            self.timers[2].stop();
        }
    }

    /// Keep cell i's row in view
    fn reveal(&mut self, st: &State, i: usize) {
        let Some(w) = &self.win else { return };
        let (cols, ch, vh) = (w.ui.get_cols().max(1) as usize, w.ui.get_cell_h(), w.ui.get_view_h());
        let top = (i / cols) as f32 * ch;
        let cur = -w.ui.get_scroll_y();
        if top < cur {
            self.scroll_to(st, top);
        } else if top + ch > cur + vh {
            self.scroll_to(st, top + ch - vh);
        }
    }

    fn lead(&mut self, st: &State, i: usize) {
        let Some(w) = &self.win else { return };
        let cols = w.ui.get_cols().max(1) as usize;
        let ch = w.ui.get_cell_h();
        self.scroll_to(st, (i / cols) as f32 * ch);
    }

    fn activate(&mut self, st: &mut State, i: usize) {
        match self.lib.activate(i) {
            Activated::Launch(a) => {
                let id = self.lib.apps[a].id.clone();
                self.close(st, true);
                if let Some(e) = st.shell.get::<Apps>().and_then(|x| x.by_id(&id)) {
                    apps::launch(e);
                }
            }
            Activated::Opened => self.push(st),
            Activated::Nothing => {}
        }
    }

    fn back(&mut self, st: &mut State) {
        if !self.text.is_empty() {
            self.text.clear();
            if let Some(w) = &self.win {
                w.ui.invoke_clear_field();
            }
            self.lib.set_query("");
            self.push(st);
        } else if self.lib.back(true) {
            self.close(st, true);
        } else {
            self.push(st);
            let s = self.lib.selected;
            self.reveal(st, s);
        }
    }

    /// AppLibraryModel.menuItems: its shortcuts, then Open and Keep in Dock
    fn menu_for(&mut self, st: &State, i: usize, x: f32, y: f32) {
        if !self.lib.showing_apps() {
            return;
        }
        let Some(&a) = self.lib.visible().get(i) else { return };
        let id = self.lib.apps[a].id.clone();
        let own = model::desktop_file(&id).and_then(|p| std::fs::read_to_string(p).ok()).map(|t| model::actions(&t)).unwrap_or_default();
        let mut items = Vec::new();
        if !own.is_empty() {
            items.push(MenuItem { text: "Shortcuts".into(), header: true, act: None });
            for (n, e) in own {
                items.push(MenuItem { text: n, header: false, act: Some(Do::Action(id.clone(), e)) });
            }
        }
        items.push(MenuItem { text: "Actions".into(), header: true, act: None });
        items.push(MenuItem { text: "Open".into(), header: false, act: Some(Do::Open(id.clone())) });
        let pinned = Self::pinned(st).contains(&id);
        items.push(MenuItem { text: if pinned { "Remove from Dock" } else { "Keep in Dock" }.into(), header: false, act: Some(if pinned { Do::Unpin(id) } else { Do::Pin(id) }) });
        self.menu = Some(Menu { items, x, y, current: -1 });
    }

    fn menu_activate(&mut self, st: &mut State, row: i32) {
        let act = self.menu.as_mut().and_then(|m| if row >= 0 && (row as usize) < m.items.len() { m.items[row as usize].act.take() } else { None });
        self.menu = None;
        match act {
            Some(Do::Action(id, exec)) => {
                self.close(st, true);
                let dir = st.shell.get::<Apps>().and_then(|a| a.by_id(&id)).map(|e| e.dir.clone()).filter(|d| !d.is_empty());
                apps::run(apps::exec_argv(&exec), &id, dir);
            }
            Some(Do::Open(id)) => {
                self.close(st, true);
                if let Some(e) = st.shell.get::<Apps>().and_then(|a| a.by_id(&id)) {
                    apps::launch(e);
                }
            }
            Some(Do::Pin(id)) => wm::ask("apps.pin", serde_json::json!([id])),
            Some(Do::Unpin(id)) => wm::ask("apps.unpin", serde_json::json!([id])),
            None => {}
        }
        self.push(st);
        if let Some(w) = &self.win {
            if self.open {
                w.ui.invoke_focus_field();
            }
        }
    }

    fn key(&mut self, st: &mut State, t: &str, shift: bool) {
        if let Some(mn) = &mut self.menu {
            if is(t, Key::Escape) {
                self.menu = None;
                self.push(st);
            } else if is(t, Key::UpArrow) || is(t, Key::DownArrow) {
                let d = if is(t, Key::UpArrow) { -1 } else { 1 };
                let mut i = mn.current + d;
                while i >= 0 && (i as usize) < mn.items.len() && mn.items[i as usize].header {
                    i += d;
                }
                if i >= 0 && (i as usize) < mn.items.len() {
                    mn.current = i;
                }
                self.push(st);
            } else {
                let c = mn.current;
                self.menu_activate(st, c);
            }
            return;
        }
        let cols = self.win.as_ref().map(|w| w.ui.get_cols().max(1)).unwrap_or(1);
        if is(t, Key::Escape) {
            if self.fab {
                self.fab = false;
                self.push(st);
            } else {
                self.back(st);
            }
        } else if is(t, Key::Return) {
            let s = self.lib.selected;
            self.activate(st, s);
        } else if is(t, Key::Tab) {
            self.lib.toggle_view();
            self.push(st);
        } else if is(t, Key::Menu) || (shift && is(t, Key::F10)) {
            // the selected app's menu, under its icon
            if let Some(w) = &self.win {
                let (cols, cw, ch) = (w.ui.get_cols().max(1) as usize, w.ui.get_cell_w(), w.ui.get_cell_h());
                let i = self.lib.selected;
                let x = w.ui.get_grid_x() + (i % cols) as f32 * cw + cw / 2.0;
                let y = w.ui.get_grid_y() + (i / cols) as f32 * ch + w.ui.get_scroll_y() + 92.0;
                self.menu_for(st, i, x, y);
                self.push(st);
            }
        } else {
            let d = if is(t, Key::LeftArrow) {
                -1
            } else if is(t, Key::RightArrow) {
                1
            } else if is(t, Key::UpArrow) {
                -cols
            } else {
                cols
            };
            self.lib.step(d);
            self.push(st);
            let s = self.lib.selected;
            self.reveal(st, s);
        }
    }

    fn cats_for(st: &State) {
        let Some(a) = st.shell.get::<Apps>() else { return };
        let entries: Vec<apps::Entry> = a.entries.iter().filter(|e| !e.no_display).cloned().collect();
        let to = send(st);
        std::thread::spawn(move || {
            let out = entries
                .iter()
                .map(|e| {
                    let cats = model::desktop_file(&e.id).and_then(|p| std::fs::read_to_string(p).ok()).map(|t| model::categories(&t)).unwrap_or_default();
                    model::app_of(e, cats)
                })
                .collect();
            to(Box::new(M::Cats(out)));
        });
    }

    fn glass(&mut self, st: &mut State) {
        let Some(w) = &mut self.win else { return };
        if !w.surface.mapped.get() || !st.shell.plugin || st.shell.look.material {
            return;
        }
        let ui = &w.ui;
        let p = ui.get_progress().clamp(0.0, 1.0);
        let k = ui.get_gscale();
        let (x, y, gw, gh) = (ui.get_gx(), ui.get_gy(), ui.get_gw(), ui.get_gh());
        let (cx, cy) = (x + gw / 2.0, y + gh / 2.0);
        let mut shapes = vec![theme::glass_shape(&st.shell.look, cx - gw * k / 2.0, cy - gh * k / 2.0, gw * k, gh * k, 28.0 * k, p, 1.0)];
        if let Some(mn) = &self.menu {
            let wdt = 200f32.max(mn.items.iter().map(|i| i.text.chars().count()).max().unwrap_or(0) as f32 * 7.5 + 40.0);
            let h: f32 = mn.items.iter().map(|i| if i.header { 26.0 } else { 28.0 }).sum::<f32>() + 10.0;
            let (sw, sh) = w.surface.logical_size();
            let (sw, sh) = (sw as f32, sh as f32);
            let mx = 0f32.max((sw - wdt).min(mn.x));
            let below = sh - mn.y;
            let up = h > below && mn.y > below;
            let my = 8f32.max((sh - 8.0 - h).min(if up { mn.y - h } else { mn.y }));
            shapes.push(theme::glass_shape(&st.shell.look, mx, my, wdt, h, 14.0, 1.0, 0.35));
        }
        let key = format!("{shapes:?}");
        if key != w.glass_sent {
            st.shell.glass.set(&w.output, NS, &shapes);
            w.glass_sent = key;
        }
    }
}

impl Part for Launchpad {
    fn name(&self) -> &'static str {
        NAME
    }

    fn start(&mut self, st: &mut State) {
        st.shell.want("apps");
        st.shell.want("icons");
    }

    fn command(&mut self, st: &mut State, cmd: &Command) {
        match cmd.cmd.as_str() {
            "open" => self.open(st, cmd.screen.clone()),
            "close" => self.close(st, false),
            _ => {}
        }
    }

    fn config_changed(&mut self, st: &mut State, look: bool) {
        if let Some(w) = &mut self.win {
            if look {
                st.shell.apply_theme(&w.ui.global::<ui::Theme>());
                w.glass_sent.clear();
            }
        }
        self.look(st);
        if self.open {
            self.push(st);
        }
    }

    fn plugin_changed(&mut self, st: &mut State) {
        if let Some(w) = &mut self.win {
            st.shell.apply_theme(&w.ui.global::<ui::Theme>());
            w.glass_sent.clear();
        }
        self.look(st);
    }

    fn data(&mut self, st: &mut State, what: std::any::TypeId) {
        if what == std::any::TypeId::of::<Apps>() {
            Self::cats_for(st);
        } else if what == std::any::TypeId::of::<Icons>() && self.open {
            self.push(st);
        }
    }

    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        let msg = match msg.downcast::<IconsDone>() {
            Ok(d) => {
                self.icons.done(*d);
                if self.open {
                    self.push(st);
                }
                return;
            }
            Err(m) => m,
        };
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Key(t, shift) => self.key(st, &t, shift),
            M::Edited(t) => {
                self.text = t;
                self.lib.set_query(&self.text);
                self.push(st);
            }
            M::Hovered(i) => {
                if self.lib.selected != i.max(0) as usize {
                    self.lib.select(i.max(0) as usize);
                    self.push(st);
                }
            }
            M::Activate(i) => {
                if self.hold_fired {
                    self.hold_fired = false;
                } else {
                    self.activate(st, i.max(0) as usize);
                }
            }
            M::CellMenu(i, x, y) => {
                self.menu_for(st, i.max(0) as usize, x, y);
                self.push(st);
            }
            M::CellDown(i, x, y) => {
                self.held = Some((i, x, y));
                self.hold_fired = false;
                let to = send(st);
                self.timers[0].start(slint::TimerMode::SingleShot, Duration::from_millis(800), move || to(Box::new(M::Hold)));
            }
            M::CellUp => {
                self.timers[0].stop();
                self.held = None;
            }
            M::Hold => {
                // press and hold: its menu
                if let Some((i, x, y)) = self.held.take() {
                    self.hold_fired = true;
                    self.menu_for(st, i.max(0) as usize, x, y);
                    self.push(st);
                }
            }
            M::Back => self.back(st),
            M::View(v) => {
                self.lib.set_view(v);
                self.push(st);
            }
            M::Jump(k) => {
                if let Some(i) = self.lib.pick(k.max(0) as usize) {
                    self.push(st);
                    self.lead(st, i);
                }
            }
            M::FabToggle => {
                self.fab = !self.fab;
                self.push(st);
            }
            M::FabPick(k) => {
                self.fab = false;
                match k {
                    0 => {
                        self.close(st, true);
                        wm::ask("panels.openAppStore", serde_json::json!([]));
                    }
                    1 => {
                        self.lib.toggle_view();
                        self.push(st);
                    }
                    _ => {
                        self.close(st, true);
                        crate::shell::ask("panels.openSettings", serde_json::json!(["menubar", "Dock"]));
                    }
                }
                self.push(st);
            }
            M::MenuActivate(r) => self.menu_activate(st, r),
            M::MenuDismissed => {
                self.menu = None;
                self.push(st);
            }
            M::Dismissed => {
                if self.fab {
                    self.fab = false;
                    self.push(st);
                } else {
                    self.close(st, true);
                }
            }
            M::SheetClicked => {
                if self.fab {
                    self.fab = false;
                    self.push(st);
                }
            }
            M::Closed => self.closed(st),
            M::Cats(list) => {
                self.lib.set_apps(list);
                if self.open {
                    self.push(st);
                }
            }
            M::Scroll => self.glide_step(),
            M::IntroEnd => {
                if let Some(w) = &self.win {
                    w.ui.set_entering(false);
                }
            }
        }
    }

    fn keyboard_left(&mut self, st: &mut State, s: &Rc<Surface>) {
        if self.open && self.menu.is_none() && self.win.as_ref().is_some_and(|w| Rc::ptr_eq(&w.surface, s)) {
            self.close(st, true);
        }
    }

    fn output_removed(&mut self, st: &mut State, name: &str) {
        if self.win.as_ref().is_some_and(|w| w.output == name) {
            let was = self.open;
            self.close(st, was);
            let w = self.win.take().unwrap();
            st.drop_surface(&w.surface);
        }
    }

    fn surface_closed(&mut self, st: &mut State, s: &Rc<Surface>) {
        if let Some(n) = self.win.as_ref().filter(|w| Rc::ptr_eq(&w.surface, s)).map(|w| w.output.clone()) {
            self.output_removed(st, &n);
        }
    }

    fn after_frame(&mut self, st: &mut State) {
        let kbd = st.keyboard_surface();
        if let Some(w) = &mut self.win {
            let f = kbd.as_ref().is_some_and(|k| Rc::ptr_eq(k, &w.surface));
            if let Some(a) = w.a11y.as_mut() {
                a.update(false, f);
            }
        }
        self.glass(st);
    }

    fn a11y(&mut self) -> Vec<&mut crate::a11y::A11y> {
        self.win.iter_mut().filter_map(|w| w.a11y.as_mut()).collect()
    }

    fn quit(&mut self, _st: &mut State) {
        if let Some(w) = &self.win {
            crate::services::hypr::Glass::clear_now(&w.output, NS);
        }
    }
}

/// A glyph as this crate's Slint takes it (tessera_core::wm::glyph)
fn wglyph(name: &str, material: bool, bold: bool) -> ui::WGlyph {
    let (text, family, scale) = tessera_core::wm::glyph(name, material, bold);
    ui::WGlyph { text, family, scale }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_field_keeps_what_it_types() {
        let k = |t: Key| SharedString::from(t).to_string();
        let none = KeyCtx::default();
        let typed = KeyCtx { has_text: true, menu: false };
        assert!(handles(none, &k(Key::Tab), false) && handles(none, &k(Key::LeftArrow), false));
        assert!(!handles(typed, &k(Key::LeftArrow), false) && handles(typed, &k(Key::DownArrow), false));
        assert!(handles(none, &k(Key::F10), true) && !handles(none, &k(Key::F10), false));
        assert!(!handles(none, "a", false));
        assert!(handles(KeyCtx { has_text: true, menu: true }, " ", false));
    }
}

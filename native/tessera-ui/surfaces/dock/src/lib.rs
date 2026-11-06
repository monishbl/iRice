//! The dock (rust.surfaces: dock): one window per output along the bottom
//! edge, its right-click menu, and the data behind it (Hyprland's windows,
//! the installed apps, the icon theme, the recent apps the shell remembers).
//! Compare shell/modules/dock/: Dock.qml, DockLogic.qml, DockSlot.qml, and
//! each look's own drawing (DockGlass + DockItem, MaterialDock +
//! MaterialDockItem), DockMenu.qml with glass/ContextMenu.qml.
//!
//! What it doesn't do yet, and why: minimized windows show their app's icon
//! on a tile, not their last frame (no screencopy here yet); no unread-count
//! badges (the notifications service isn't in Rust); clicking the app in
//! front doesn't open App Exposé (the QML shell has no IPC for it: it
//! focuses the app instead); the Downloads stack is its own part
//! (stack.rs: Glass and Material); the phone item and the touchscreen swipe aren't here; Empty Trash plays no
//! sound. App icons are styled on the CPU (src/appicon.rs: Slint has no
//! custom shaders), once per icon and size.

pub mod ui {
    slint::include_modules!();
}
tessera_core::theme_global!(ui);

// tessera-core's modules (shell, wayland, theme, services, util…) under the
// names they had when everything was one crate: `crate::shell::Part`
#[allow(unused_imports)]
use tessera_core::*;

pub mod layout;
pub mod model;
pub mod settings;
pub mod stack;

pub use stack::DownloadsStack;

use tessera_core::appicon::{self, Style};
use tessera_core::services::apps::{self, Apps};
use tessera_core::services::clients::{self, Windows};
use tessera_core::services::icons::Icons;
use tessera_core::shell::Part;
use tessera_core::theme;
use tessera_core::wayland::{Spec, State, Surface};
use layout::{Metrics, Reveal, Timers};
use model::{Action, Item, Kind};
use settings::Settings;
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use smithay_client_toolkit::reexports::client::protocol::wl_output::WlOutput;
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use std::any::{Any, TypeId};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

pub const NS: &str = "tessera:dock";
pub const MENU_NS: &str = "tessera:contextmenu";
const NAME: &str = "dock";

/// Its own messages (Msg::Part("dock", …)), from callbacks and timers
enum M {
    Pointer(String, f32, f32, bool),
    Press(String, f32, f32, i32),
    Release(String, f32, f32, i32),
    Activate(String, usize),
    Reveal(String),
    Hide(String),
    TipsWarm(String),
    Icons(Vec<(IconKey, Option<slint::SharedPixelBuffer<slint::Rgba8Pixel>>)>),
    Trash(usize),
    MenuChosen(i32),
    MenuDismissed,
    MenuClosed,
    /// the Downloads stack closed (stack.rs)
    StackClosed,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct IconKey {
    path: PathBuf,
    px: u32,
    style: Style,
    /// a symbolic icon's colour
    ink: Option<[u8; 3]>,
}

struct Win {
    output: String,
    surface: Rc<Surface>,
    ui: ui::Dock,
    slots: Rc<VecModel<ui::DockSlot>>,
    a11y: Option<crate::a11y::A11y>,
    glass_sent: String,
    region_sent: Option<Vec<(i32, i32, i32, i32)>>,
    /// window coordinates, while the pointer is on it
    pointer: Option<(f32, f32)>,
    hot: Option<usize>,
    pressed: Option<(usize, i32)>,
    hovered: bool,
    reveal: Reveal,
    reveal_timer: slint::Timer,
    hide_timer: slint::Timer,
    tip_timer: slint::Timer,
    tips_warm: bool,
}

struct MenuWin {
    output: String,
    surface: Rc<Surface>,
    ui: ui::DockMenu,
    a11y: Option<crate::a11y::A11y>,
    open: bool,
    item: Item,
    rows: Vec<model::Row>,
    glass_sent: String,
}

#[derive(Default)]
pub struct Dock {
    wins: Vec<Win>,
    menu: Option<MenuWin>,
    items: Vec<Item>,
    recent: Vec<String>,
    recent_read: bool,
    active_app: String,
    urgent: BTreeSet<String>,
    bounces: HashMap<String, i32>,
    icons: HashMap<IconKey, slint::Image>,
    pending: HashSet<IconKey>,
    /// per item key: the icon file and whether it's the theme's own app grid
    paths: HashMap<String, (Option<PathBuf>, bool)>,
    scale: f32,
    /// the text colour (symbolic icons)
    ink: [u8; 3],
    trash: usize,
    trash_timer: slint::Timer,
    /// the Downloads stack is up (the dock stays out meanwhile, as with its menu)
    stack_open: bool,
}

fn rgb(c: slint::Color) -> [u8; 3] {
    [c.red(), c.green(), c.blue()]
}

/// The Downloads folder (Transfers.qml's downloadsDir)
fn downloads_dir() -> PathBuf {
    std::env::var("XDG_DOWNLOAD_DIR").ok().filter(|s| !s.is_empty()).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap_or_default()).join("Downloads"))
}

fn trash_dir() -> PathBuf {
    let data = std::env::var("XDG_DATA_HOME").ok().filter(|s| !s.is_empty()).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".local/share"));
    data.join("Trash/files")
}

use tessera_core::util::on_path;

/// The file manager that opens folders and the Trash (Files.qml): Settings'
/// choice when it's installed, else the first installed
fn file_manager(cfg: &serde_json::Value) -> Option<String> {
    let want = cfg["desktop"]["fileManager"].as_str().unwrap_or("");
    if !want.is_empty() && on_path(want) {
        return Some(want.to_string());
    }
    ["nautilus", "dolphin", "thunar", "nemo", "caja", "pcmanfm-qt", "pcmanfm"].iter().find(|b| on_path(b)).map(|b| b.to_string())
}

fn open_folder(cfg: &serde_json::Value, path: &std::path::Path) {
    let p = path.to_string_lossy().into_owned();
    match file_manager(cfg) {
        Some(m) => apps::run(vec![m.clone(), p], &m, None),
        None => apps::run(vec!["xdg-open".into(), format!("file://{p}")], "open", None),
    }
}

fn open_trash(cfg: &serde_json::Value) {
    match file_manager(cfg) {
        Some(m) => {
            let uri = if m == "dolphin" { "trash:/" } else { "trash:///" };
            apps::run(vec![m.clone(), uri.into()], &m, None)
        }
        None => apps::run(vec!["xdg-open".into(), "trash:///".into()], "open", None),
    }
}

/// Ask the QML shell to do something it still does itself (open the App
/// Drawer…), on a thread
fn shell_ipc(args: Vec<String>) {
    std::thread::spawn(move || {
        let Some(script) = apps::shell_file("shell/assets/scripts/shell-ipc.sh") else { return };
        let _ = std::process::Command::new("bash").arg(script).arg("call").args(args).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
    });
}

fn count_trash() -> usize {
    std::fs::read_dir(trash_dir()).map(|d| d.count()).unwrap_or(0)
}

impl Dock {
    fn settings(st: &State) -> Settings {
        Settings::read(&st.shell.cfg)
    }

    fn metrics(st: &State) -> Metrics {
        Metrics::new(&Self::settings(st), st.shell.look.material)
    }

    /// How icons are drawn in this look
    fn style(st: &State, t: &ui::Theme) -> Style {
        let themed = (rgb(t.get_m3_primary_container()), rgb(t.get_m3_on_primary_container()));
        Style::from_settings(&st.shell.cfg, st.shell.look.material, st.shell.look.dark, rgb(t.get_accent()), themed)
    }

    fn spec<'a>(output: &'a WlOutput, name: &'a str, m: &Metrics, s: &Settings) -> Spec<'a> {
        Spec {
            layer: Layer::Top,
            namespace: NS,
            output: Some((output, name)),
            anchor: Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
            size: (0, m.window_height()),
            exclusive: m.exclusive(s.auto_hide),
            keyboard: KeyboardInteractivity::None,
            margin_left: 0,
        }
    }

    /// The items again, from the data (and their icons, rendered as needed)
    fn rebuild(&mut self, st: &mut State) {
        let s = Self::settings(st);
        let empty_apps = Apps::default();
        let apps = st.shell.get::<Apps>().unwrap_or(&empty_apps);
        let empty_windows = Windows::default();
        let w = st.shell.get::<Windows>().unwrap_or(&empty_windows);
        if !self.recent_read {
            self.recent = model::read_recent();
            self.recent_read = true;
        }
        let mut items = model::build(&s, apps, w, &self.recent);
        // the app that asked for attention hops (once per request)
        let urgent: BTreeSet<String> = w.urgent.clone();
        for it in items.iter_mut().filter(|i| i.kind.is_app()) {
            if it.windows.iter().any(|a| urgent.contains(a) && !self.urgent.contains(a)) {
                *self.bounces.entry(it.id.clone()).or_insert(0) += 1;
            }
        }
        self.urgent = urgent;
        self.items = items;
        self.find_icons(st);
        self.update(st);
    }

    /// Each item's icon file (looked up once per item and theme), and the
    /// renders still missing
    fn find_icons(&mut self, st: &mut State) {
        let Some(icons) = st.shell.get::<Icons>().cloned() else { return };
        let full = self.trash > 0;
        let mut keep = HashMap::new();
        for it in &self.items {
            let key = format!("{}{}", it.key(), if it.kind == Kind::Trash && full { "+full" } else { "" });
            let found = self.paths.get(&key).cloned().unwrap_or_else(|| {
                let (names, fallbacks) = it.icon_names(full);
                let names: Vec<&str> = names.iter().map(String::as_str).collect();
                if it.kind == Kind::Launcher {
                    // the theme's app grid, when it has one (else the dot grid);
                    // each name or its symbolic version in turn, as Qt's lookup does
                    let p = names.iter().find_map(|n| icons.exact(n).or_else(|| icons.exact(&format!("{n}-symbolic"))));
                    let themed = p.as_ref().is_some_and(|p| {
                        let s = p.to_string_lossy();
                        !s.contains("application-x-executable") && !s.contains("default-icon")
                    });
                    (p.filter(|_| themed), themed)
                } else {
                    (icons.path(&names, &fallbacks), false)
                }
            });
            keep.insert(key, found);
        }
        self.paths = keep;
        self.request_icons(st);
    }

    /// The size icons are drawn at (device pixels): Glass at their largest
    /// (magnified), Material its glyph
    fn icon_px(&self, st: &State) -> u32 {
        let m = Self::metrics(st);
        let logical = if m.material { m.glyph } else { m.base * m.max_scale };
        (logical * self.scale.max(1.0)).ceil() as u32
    }

    fn key_for(&self, st: &State, it: &Item, style: &Style) -> Option<IconKey> {
        let full = self.trash > 0;
        let k = format!("{}{}", it.key(), if it.kind == Kind::Trash && full { "+full" } else { "" });
        let path = self.paths.get(&k)?.0.clone()?;
        let px = if it.kind == Kind::Minimized { self.icon_px(st) / 2 } else { self.icon_px(st) };
        // (symbolic icons in the text colour, as the theme means them)
        let ink = path.to_string_lossy().contains("-symbolic").then_some(self.ink);
        Some(IconKey { path, px, style: style.clone(), ink })
    }

    fn request_icons(&mut self, st: &mut State) {
        let Some(w) = self.wins.first() else { return };
        let style = Self::style(st, &w.ui.global::<ui::Theme>());
        self.ink = rgb(w.ui.global::<ui::Theme>().get_label());
        let wanted: Vec<IconKey> = self.items.iter().filter_map(|it| self.key_for(st, it, &style)).collect();
        // (forget renders nothing shows any more)
        let used: HashSet<&IconKey> = wanted.iter().collect();
        self.icons.retain(|k, _| used.contains(k));
        let missing: Vec<IconKey> = wanted.iter().filter(|k| !self.icons.contains_key(*k) && !self.pending.contains(*k)).cloned().collect::<HashSet<_>>().into_iter().collect();
        if missing.is_empty() {
            return;
        }
        self.pending.extend(missing.iter().cloned());
        let to = st.shell.sender(NAME);
        std::thread::spawn(move || {
            let out = missing.into_iter().map(|k| {
                let img = appicon::render(&k.path, k.px, &k.style, k.ink);
                (k, img)
            });
            to(Box::new(M::Icons(out.collect())));
        });
    }

    fn slot_for(&self, st: &State, it: &Item, style: &Style) -> (slint::Image, bool, bool) {
        let themed = self.paths.get(&it.key()).is_some_and(|p| p.1);
        match self.key_for(st, it, style).and_then(|k| self.icons.get(&k).cloned()) {
            Some(img) => (img, true, themed),
            None => (slint::Image::default(), false, themed),
        }
    }

    /// Every window's slots, sizes, hover and auto-hide, from the items and the pointer
    fn update(&mut self, st: &mut State) {
        let s = Self::settings(st);
        let m = Self::metrics(st);
        let dividers: Vec<bool> = self.items.iter().map(|i| i.divider).collect();
        let popup = self.menu.as_ref().is_some_and(|mw| mw.open) || self.stack_open;
        let windows = st.shell.get::<Windows>().cloned().unwrap_or_default();
        let plugin = st.shell.plugin;
        let Some(first) = self.wins.first() else { return };
        let style = Self::style(st, &first.ui.global::<ui::Theme>());
        let mut base_slots = Vec::with_capacity(self.items.len());
        for it in &self.items {
            let (icon, has_icon, themed) = self.slot_for(st, it, &style);
            base_slots.push(ui::DockSlot {
                kind: it.kind.name().into(),
                name: it.name.as_str().into(),
                icon,
                has_icon,
                themed,
                running: it.running,
                focused: it.focused,
                divider: it.divider,
                bounce: self.bounces.get(&it.id).copied().unwrap_or(0),
                ..Default::default()
            });
        }
        let to = st.shell.sender(NAME);
        for w in &mut self.wins {
            let ui = &w.ui;
            let (ww, wh) = w.surface.logical_size();
            let (ww, wh) = (ww as f32, wh as f32);
            let settled = ui.get_slide() < 0.001;
            let rel = w.pointer.map(|(x, y)| (x - ww / 2.0, y - wh));
            let hit = layout::hit(&m, &dividers, rel, settled, popup);
            // Material's tooltips wait a moment the first time, then follow the pointer
            if hit.hovered != w.hovered {
                w.hovered = hit.hovered;
                if hit.hovered {
                    let (to, o) = (to.clone(), w.output.clone());
                    w.tip_timer.start(slint::TimerMode::SingleShot, Duration::from_millis(500), move || to(Box::new(M::TipsWarm(o.clone()))));
                } else {
                    w.tip_timer.stop();
                    w.tips_warm = false;
                }
            }
            w.hot = hit.hot;
            for (i, (slot, p)) in base_slots.iter().zip(&hit.placed).enumerate() {
                let mut slot = slot.clone();
                slot.x = p.x;
                slot.size = p.size;
                slot.lead = p.lead;
                slot.hot = hit.hot == Some(i);
                slot.pressed = w.pressed.is_some_and(|(j, _)| j == i);
                if i < w.slots.row_count() {
                    if w.slots.row_data(i).as_ref() != Some(&slot) {
                        w.slots.set_row_data(i, slot);
                    }
                } else {
                    w.slots.push(slot);
                }
            }
            while w.slots.row_count() > base_slots.len() {
                w.slots.remove(w.slots.row_count() - 1);
            }
            ui.set_pill_width(hit.pill_w);
            ui.set_base(m.base);
            ui.set_pad(m.pad);
            ui.set_gap(m.gap);
            ui.set_corner(m.corner);
            ui.set_bar_height(m.bar_h);
            ui.set_bottom_gap(m.bottom_gap);
            ui.set_max_scale(m.max_scale);
            ui.set_glass_plugin(plugin && !st.shell.look.material);
            ui.set_tips_warm(w.tips_warm);
            ui.set_popup_open(popup);
            // auto-hide: always, or while a window overlaps where it rests
            let overlapped = s.auto_hide == settings::AutoHide::Windows
                && windows.monitor(&w.output).is_some_and(|mon| {
                    let (sw, sh) = mon.logical();
                    windows.covered(&w.output, layout::cover_rect(&m, &dividers, sw, sh))
                });
            let auto = layout::auto_hiding(s.auto_hide, overlapped);
            let hidden = layout::hidden(auto, w.reveal.revealed, popup);
            ui.set_hidden(hidden);
            if ww > 0.0 && w.surface.mapped.get() {
                let rects = layout::input(&m, &dividers, ww, wh, auto, hidden);
                if w.region_sent.as_ref() != Some(&rects) {
                    st.set_input_region(&w.surface, Some(&rects));
                    w.region_sent = Some(rects);
                }
            }
        }
    }

    fn win(&mut self, output: &str) -> Option<&mut Win> {
        self.wins.iter_mut().find(|w| w.output == output)
    }

    fn timers(&mut self, st: &State, output: &str, t: Timers) {
        let to = st.shell.sender(NAME);
        let Some(w) = self.win(output) else { return };
        let o = output.to_string();
        if t.stop_reveal {
            w.reveal_timer.stop();
        }
        if t.stop_hide {
            w.hide_timer.stop();
        }
        if t.start_reveal {
            let (to, o) = (to.clone(), o.clone());
            w.reveal_timer.start(slint::TimerMode::SingleShot, Duration::from_millis(layout::REVEAL_DELAY_MS), move || to(Box::new(M::Reveal(o.clone()))));
        }
        if t.start_hide {
            w.hide_timer.start(slint::TimerMode::SingleShot, Duration::from_millis(layout::HIDE_DELAY_MS), move || to(Box::new(M::Hide(o.clone()))));
        }
    }

    fn pointer(&mut self, st: &mut State, output: &str, x: f32, y: f32, on: bool) {
        let m = Self::metrics(st);
        let dividers: Vec<bool> = self.items.iter().map(|i| i.divider).collect();
        let Some(w) = self.win(output) else { return };
        w.pointer = on.then_some((x, y));
        let (ww, wh) = w.surface.logical_size();
        let (edge, zone) = layout::edge_and_zone(&m, &dividers, ww as f32, wh as f32, w.pointer);
        let t = w.reveal.pointer(edge, zone);
        self.timers(st, output, t);
        self.update(st);
    }

    fn press(&mut self, st: &mut State, output: &str, x: f32, y: f32, button: i32) {
        self.pointer(st, output, x, y, true);
        let Some(w) = self.win(output) else { return };
        w.pressed = w.hot.map(|i| (i, button));
        self.update(st);
    }

    fn release(&mut self, st: &mut State, output: &str, x: f32, y: f32, button: i32) {
        if button <= 2 {
            self.pointer(st, output, x, y, true);
        }
        let Some(w) = self.win(output) else { return };
        let pressed = w.pressed.take();
        let hot = w.hot;
        self.update(st);
        if let Some((i, b)) = pressed {
            if b == button && hot == Some(i) {
                self.activate(st, output, i, button);
            }
        }
    }

    /// A click on an item (DockSlot.activate): right opens its menu
    fn activate(&mut self, st: &mut State, output: &str, i: usize, button: i32) {
        let Some(it) = self.items.get(i).cloned() else { return };
        if button == 1 {
            self.open_menu(st, output, i);
            return;
        }
        if button != 0 {
            return;
        }
        let cfg = st.shell.cfg.clone();
        let windows = st.shell.get::<Windows>().cloned().unwrap_or_default();
        match it.kind {
            Kind::Launcher => shell_ipc(vec!["panels".into(), "toggle".into(), "launchpad".into(), output.into()]),
            Kind::Minimized => {
                if let Some(a) = it.windows.first() {
                    clients::restore(a, windows.focused_monitor().map_or(1, |m| m.active.id));
                }
            }
            Kind::Downloads => self.open_stack(st, output, i),
            Kind::Trash => open_trash(&cfg),
            Kind::App | Kind::Recent => {
                if !it.running {
                    *self.bounces.entry(it.id.clone()).or_insert(0) += 1;
                    if let Some(e) = &it.entry {
                        apps::launch(e);
                    }
                    self.update(st);
                    return;
                }
                // its newest window you can see, else bring back its newest one
                let mine: Vec<&clients::Client> = windows.clients.iter().filter(|c| it.windows.contains(&c.address)).collect();
                if let Some(c) = mine.iter().rev().find(|c| !c.aside()) {
                    clients::focus(&c.address);
                } else if let Some(c) = mine.last() {
                    clients::restore(&c.address, windows.focused_monitor().map_or(1, |m| m.active.id));
                }
            }
        }
    }

    /// The Downloads stack, pointing at an item (DockSlot: stackX; stack.rs)
    fn open_stack(&mut self, st: &mut State, output: &str, i: usize) {
        let m = Self::metrics(st);
        let Some(w) = self.wins.iter().find(|w| w.output == output) else { return };
        let slot = w.slots.row_data(i).unwrap_or_default();
        let at = w.ui.get_pill_x() + slot.x + if m.material { m.base } else { slot.size } / 2.0;
        st.shell.sender("downloadsstack")(Box::new(serde_json::json!({"open": true, "screen": output, "x": at, "barH": m.bar_h, "gap": m.bottom_gap})));
        self.stack_open = true;
        self.update(st);
    }

    // ── The right-click menu ─────────────────────────────────────────────

    fn open_menu(&mut self, st: &mut State, output: &str, i: usize) {
        let Some(it) = self.items.get(i).cloned() else { return };
        let m = Self::metrics(st);
        let Some(w) = self.wins.iter().find(|w| w.output == output) else { return };
        // the screen x the menu points at: the icon's centre
        let slot = w.slots.row_data(i).unwrap_or_default();
        let at = w.ui.get_pill_x() + slot.x + if m.material { m.base } else { slot.size } / 2.0;
        if it.kind == Kind::Trash {
            self.trash = count_trash();
        }
        let rows = model::menu(&it, self.trash);
        if self.menu.as_ref().is_some_and(|mw| mw.output != output) {
            let mw = self.menu.take().unwrap();
            st.shell.glass.set(&mw.output, MENU_NS, &[]);
            st.drop_surface(&mw.surface);
        }
        if self.menu.is_none() {
            let Some(wl_output) = st.output_named(output) else { return };
            let surface = st.prepare_surface(Spec {
                layer: Layer::Overlay,
                namespace: MENU_NS,
                output: Some((&wl_output, output)),
                anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
                size: (0, 0),
                exclusive: -1,
                keyboard: KeyboardInteractivity::Exclusive,
                margin_left: 0,
            });
            let ui = ui::DockMenu::new().expect("DockMenu");
            st.shell.apply_theme(&ui.global::<ui::Theme>());
            let to = st.shell.sender(NAME);
            let (to2, to3) = (to.clone(), to.clone());
            ui.on_chosen(move |i| to(Box::new(M::MenuChosen(i))));
            ui.on_dismissed(move || to2(Box::new(M::MenuDismissed)));
            ui.on_closed(move || to3(Box::new(M::MenuClosed)));
            ui.show().expect("show dock menu");
            let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
            self.menu = Some(MenuWin { output: output.to_string(), surface, ui, a11y, open: false, item: it.clone(), rows: Vec::new(), glass_sent: String::new() });
        }
        let mw = self.menu.as_mut().unwrap();
        let ui_rows: Vec<ui::DockMenuRow> = rows
            .iter()
            .map(|r| ui::DockMenuRow { text: r.text.as_str().into(), confirm: r.confirm.as_str().into(), enabled: r.enabled, danger: r.danger, separator: r.action.is_none() })
            .collect();
        // (ContextMenu.qml: about 7.5 px a character, at least 200)
        let widest = rows.iter().map(|r| r.text.chars().count().max(r.confirm.chars().count())).max().unwrap_or(0);
        mw.ui.set_menu_width((widest as f32 * 7.5 + 40.0).max(200.0));
        mw.ui.set_rows(ModelRc::new(VecModel::from(ui_rows)));
        mw.ui.set_point_x(at);
        mw.ui.set_point_bottom(m.bar_h + m.bottom_gap + 8.0);
        mw.ui.set_glass_plugin(st.shell.plugin && !st.shell.look.material);
        mw.item = it;
        mw.rows = rows;
        mw.surface.show(0, 0);
        mw.open = true;
        mw.ui.set_shown(true);
        mw.ui.invoke_open();
        self.update(st);
    }

    fn close_menu(&mut self, st: &mut State) {
        if let Some(mw) = &mut self.menu {
            if mw.open {
                mw.open = false;
                mw.ui.set_shown(false); // the close animation, then MenuClosed hides it
            }
        }
        self.update(st);
    }

    fn menu_closed(&mut self, st: &mut State) {
        // (closed, it goes: a hidden layer stays in Hyprland's list; a menu
        // is made again in a moment when it's next opened)
        if self.menu.as_ref().is_some_and(|m| !m.open) {
            let m = self.menu.take().unwrap();
            st.shell.glass.set(&m.output, MENU_NS, &[]);
            st.drop_surface(&m.surface);
        }
    }

    fn menu_chosen(&mut self, st: &mut State, row: i32) {
        let Some(mw) = &self.menu else { return };
        let Some(action) = mw.rows.get(row as usize).and_then(|r| r.action) else { return };
        let (it, output) = (mw.item.clone(), mw.output.clone());
        self.close_menu(st);
        let cfg = st.shell.cfg.clone();
        let windows = st.shell.get::<Windows>().cloned().unwrap_or_default();
        let s = Self::settings(st);
        match action {
            Action::OpenDrawer => shell_ipc(vec!["panels".into(), "open".into(), "launchpad".into(), output]),
            Action::RemoveLauncher => settings::write("appsButton", serde_json::json!(false)),
            Action::Restore => {
                if let Some(a) = it.windows.first() {
                    clients::restore(a, windows.focused_monitor().map_or(1, |m| m.active.id));
                }
            }
            Action::CloseWindow => it.windows.iter().for_each(|a| clients::close(a)),
            Action::OpenDownloads => open_folder(&cfg, &downloads_dir()),
            Action::ShowDownloads => {
                if let Some(i) = self.items.iter().position(|x| x.id == it.id) {
                    self.open_stack(st, &output, i);
                }
            }
            Action::RemoveDownloads => settings::write("downloads", serde_json::json!(false)),
            Action::OpenTrash => open_trash(&cfg),
            Action::EmptyTrash => apps::run(vec!["gio".into(), "trash".into(), "--empty".into()], "gio", None),
            Action::Launch => {
                if let Some(e) = &it.entry {
                    apps::launch(e);
                }
            }
            Action::Pin => settings::write("pinned", serde_json::json!(settings::pinned_with(&s.pinned, &it.id, true))),
            Action::Unpin => settings::write("pinned", serde_json::json!(settings::pinned_with(&s.pinned, &it.id, false))),
            Action::Quit => {
                // close its windows, then quit-app.sh ends what's left (apps that hide to the tray)
                let mut pids: Vec<String> = windows.clients.iter().filter(|c| it.windows.contains(&c.address) && c.pid > 1).map(|c| c.pid.to_string()).collect();
                pids.dedup();
                it.windows.iter().for_each(|a| clients::close(a));
                if let Some(q) = apps::shell_file("shell/assets/scripts/quit-app.sh") {
                    if !pids.is_empty() {
                        let mut argv = vec![q.to_string_lossy().into_owned()];
                        argv.extend(pids);
                        std::thread::spawn(move || {
                            let _ = std::process::Command::new("setsid").arg("-f").args(argv).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
                        });
                    }
                }
            }
        }
    }

    fn apply_theme(&mut self, st: &State) {
        for w in &mut self.wins {
            st.shell.apply_theme(&w.ui.global::<ui::Theme>());
            w.glass_sent.clear();
        }
        if let Some(mw) = &mut self.menu {
            st.shell.apply_theme(&mw.ui.global::<ui::Theme>());
            mw.glass_sent.clear();
        }
    }

    /// Size and exclusive zone after a settings change
    fn reshape(&mut self, st: &mut State) {
        let s = Self::settings(st);
        let m = Self::metrics(st);
        for w in &mut self.wins {
            w.surface.layer.set_exclusive_zone(m.exclusive(s.auto_hide));
            w.surface.show(0, m.window_height());
            w.region_sent = None;
        }
    }

    fn watch_trash(&mut self, st: &State) {
        let to = st.shell.sender(NAME);
        // (a folder count every 2 s: cheap, and the icon follows within a moment)
        self.trash_timer.start(slint::TimerMode::Repeated, Duration::from_secs(2), move || {
            let to = to.clone();
            std::thread::spawn(move || to(Box::new(M::Trash(count_trash()))));
        });
    }
}

impl Part for Dock {
    fn name(&self) -> &'static str {
        NAME
    }

    fn start(&mut self, st: &mut State) {
        if crate::env::is_set("DEBUG") {
            eprintln!("debug: dock in {} from {:?}: {:?}", if st.shell.look.material { "Material" } else { "Glass" }, theme::config_path(), Self::settings(st));
        }
        st.shell.want("clients");
        st.shell.want("apps");
        st.shell.want("icons");
        self.scale = 1.0;
        self.trash = count_trash();
        self.watch_trash(st);
    }

    fn output_added(&mut self, st: &mut State, output: &WlOutput, name: &str) {
        let s = Self::settings(st);
        if !s.enabled || self.wins.iter().any(|w| w.output == name) {
            return;
        }
        let m = Self::metrics(st);
        let surface = st.prepare_surface(Self::spec(output, name, &m, &s));
        let ui = ui::Dock::new().expect("Dock");
        st.shell.apply_theme(&ui.global::<ui::Theme>());
        let slots = Rc::new(VecModel::<ui::DockSlot>::default());
        ui.set_slots(ModelRc::from(slots.clone()));
        let to = st.shell.sender(NAME);
        let (o1, o2, o3, o4) = (name.to_string(), name.to_string(), name.to_string(), name.to_string());
        let (t1, t2, t3, t4) = (to.clone(), to.clone(), to.clone(), to);
        ui.on_pointer(move |x, y, on| t1(Box::new(M::Pointer(o1.clone(), x, y, on))));
        ui.on_press(move |x, y, b| t2(Box::new(M::Press(o2.clone(), x, y, b))));
        ui.on_release(move |x, y, b| t3(Box::new(M::Release(o3.clone(), x, y, b))));
        ui.on_activate(move |i| t4(Box::new(M::Activate(o4.clone(), i.max(0) as usize))));
        ui.show().expect("show dock");
        let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
        self.wins.push(Win {
            output: name.to_string(),
            surface,
            ui,
            slots,
            a11y,
            glass_sent: String::new(),
            region_sent: None,
            pointer: None,
            hot: None,
            pressed: None,
            hovered: false,
            reveal: Reveal::default(),
            reveal_timer: slint::Timer::default(),
            hide_timer: slint::Timer::default(),
            tip_timer: slint::Timer::default(),
            tips_warm: false,
        });
        self.rebuild(st);
    }

    fn output_removed(&mut self, st: &mut State, name: &str) {
        if let Some(i) = self.wins.iter().position(|w| w.output == name) {
            let w = self.wins.remove(i);
            st.shell.glass.set(&w.output, NS, &[]);
            st.drop_surface(&w.surface);
        }
        if self.menu.as_ref().is_some_and(|m| m.output == name) {
            let m = self.menu.take().unwrap();
            st.drop_surface(&m.surface);
        }
    }

    fn config_changed(&mut self, st: &mut State, look: bool) {
        let s = Self::settings(st);
        // Settings → Dock → Show the dock off: no dock windows at all
        if !s.enabled {
            let names: Vec<String> = self.wins.iter().map(|w| w.output.clone()).collect();
            for n in names {
                self.output_removed(st, &n);
            }
            return;
        }
        if self.wins.is_empty() {
            for (o, n) in st.outputs_now() {
                self.output_added(st, &o, &n);
            }
        }
        if look {
            self.apply_theme(st);
        }
        self.reshape(st);
        self.rebuild(st);
    }

    fn plugin_changed(&mut self, st: &mut State) {
        self.apply_theme(st);
        self.update(st);
    }

    fn data(&mut self, st: &mut State, what: TypeId) {
        if what == TypeId::of::<Windows>() {
            // the app in front goes first among the recent ones
            let w = st.shell.get::<Windows>().cloned().unwrap_or_default();
            let apps = st.shell.get::<Apps>();
            if let (Some(c), Some(apps)) = (w.active_client(), apps) {
                if let Some(e) = apps.entry_for(&c.class) {
                    if e.id != self.active_app {
                        self.active_app = e.id.clone();
                        model::note_recent(&mut self.recent, &e.id);
                    }
                }
            }
            self.rebuild(st);
        } else if what == TypeId::of::<Apps>() || what == TypeId::of::<Icons>() {
            if let Some(i) = st.shell.get::<Icons>().filter(|_| what == TypeId::of::<Icons>()) {
                eprintln!("tessera-ui: dock icons from the {} theme ({} names)", i.theme, i.index.len());
            }
            self.paths.clear();
            self.rebuild(st);
        }
    }

    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Pointer(o, x, y, on) => self.pointer(st, &o, x, y, on),
            M::Press(o, x, y, b) => self.press(st, &o, x, y, b),
            M::Release(o, x, y, b) => self.release(st, &o, x, y, b),
            M::Activate(o, i) => self.activate(st, &o, i, 0),
            M::Reveal(o) => {
                let Some(w) = self.win(&o) else { return };
                let t = w.reveal.reveal();
                self.timers(st, &o, t);
                self.update(st);
            }
            M::Hide(o) => {
                if let Some(w) = self.win(&o) {
                    w.reveal.hide();
                }
                self.update(st);
            }
            M::TipsWarm(o) => {
                if let Some(w) = self.win(&o) {
                    w.tips_warm = w.hovered;
                }
                self.update(st);
            }
            M::Icons(list) => {
                for (k, buf) in list {
                    self.pending.remove(&k);
                    if let Some(buf) = buf {
                        self.icons.insert(k, slint::Image::from_rgba8(buf));
                    }
                }
                self.update(st);
            }
            M::StackClosed => {
                if self.stack_open {
                    self.stack_open = false;
                    self.update(st);
                }
            }
            M::Trash(n) => {
                let was = self.trash > 0;
                self.trash = n;
                if was != (n > 0) {
                    self.find_icons(st);
                    self.update(st);
                }
            }
            M::MenuChosen(i) => self.menu_chosen(st, i),
            M::MenuDismissed => self.close_menu(st),
            M::MenuClosed => self.menu_closed(st),
        }
    }

    fn keyboard_left(&mut self, st: &mut State, s: &Rc<Surface>) {
        if self.menu.as_ref().is_some_and(|m| m.open && Rc::ptr_eq(&m.surface, s)) {
            self.close_menu(st);
        }
    }

    fn surface_closed(&mut self, st: &mut State, s: &Rc<Surface>) {
        let name = s.output.clone();
        self.output_removed(st, &name);
    }

    /// Icons drawn for the output's scale; the glass shapes for the plugin
    /// (the slab as it slides; the menu as it springs open), sent when they
    /// change; then the accessibility trees
    fn after_frame(&mut self, st: &mut State) {
        let scale = self.wins.iter().map(|w| w.ui.window().scale_factor()).fold(1.0f32, f32::max);
        if (scale - self.scale).abs() > 0.01 {
            self.scale = scale;
            self.request_icons(st);
        }
        // the input shape once the window has its size
        if self.wins.iter().any(|w| w.region_sent.is_none() && w.surface.mapped.get()) {
            self.update(st);
        }
        let kbd = st.keyboard_surface();
        for w in &mut self.wins {
            if let Some(a) = w.a11y.as_mut() {
                a.update(false, false);
            }
        }
        if let Some(mw) = self.menu.as_mut() {
            let f = kbd.as_ref().is_some_and(|k| Rc::ptr_eq(k, &mw.surface));
            if let Some(a) = mw.a11y.as_mut() {
                a.update(false, f);
            }
        }
        if !st.shell.plugin || st.shell.look.material {
            return;
        }
        let look = st.shell.look.clone();
        let m = Self::metrics(st);
        for w in &mut self.wins {
            if !w.surface.mapped.get() {
                continue;
            }
            let ui = &w.ui;
            let shown = ui.get_slide() < 0.999 && !self.items.is_empty();
            let shapes: Vec<[f32; 19]> = if shown { vec![theme::glass_shape(&look, ui.get_pill_x(), ui.get_pill_y(), ui.get_pill_width(), m.bar_h, m.corner, 1.0, 1.0)] } else { Vec::new() };
            let key = format!("{shapes:?}");
            if key != w.glass_sent {
                st.shell.glass.set(&w.output, NS, &shapes);
                w.glass_sent = key;
            }
        }
        if let Some(mw) = &mut self.menu {
            if !mw.surface.mapped.get() {
                return;
            }
            let ui = &mw.ui;
            let p = ui.get_progress();
            let s = ui.get_panel_scale();
            let (px, py, pw, ph) = (ui.get_panel_x(), ui.get_panel_y(), ui.get_panel_width(), ui.get_panel_height());
            // scaled about its bottom centre, as dock.slint
            let (x, y) = (px + pw * (1.0 - s) / 2.0, py + ph * (1.0 - s));
            let opacity = p.min(1.0);
            let shape = theme::glass_shape(&look, x, y, pw * s, ph * s, ui.get_radius() * s, opacity, 1.0);
            let key = format!("{shape:?}");
            if key != mw.glass_sent {
                st.shell.glass.set(&mw.output, MENU_NS, if opacity > 0.002 { std::slice::from_ref(&shape) } else { &[] });
                mw.glass_sent = key;
            }
        }
    }

    fn a11y(&mut self) -> Vec<&mut crate::a11y::A11y> {
        let mut v: Vec<&mut crate::a11y::A11y> = self.wins.iter_mut().filter_map(|w| w.a11y.as_mut()).collect();
        if let Some(m) = self.menu.as_mut().and_then(|m| m.a11y.as_mut()) {
            v.push(m);
        }
        v
    }

    fn stats(&self) {
        for w in &self.wins {
            eprintln!("tessera-ui: stats {}", w.surface.stats.borrow_mut().report(&format!("dock:{}", w.output)));
        }
    }

    /// On the way out: no glass left behind
    fn quit(&mut self, _: &mut State) {
        for w in &self.wins {
            crate::services::hypr::Glass::clear_now(&w.output, NS);
        }
        if let Some(m) = &self.menu {
            crate::services::hypr::Glass::clear_now(&m.output, MENU_NS);
        }
    }
}

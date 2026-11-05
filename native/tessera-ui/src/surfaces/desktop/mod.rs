//! The desktop (rust.surfaces: desktop): the widgets on their grid, the
//! files on the desktop, and the right-click menus, on the first screen, on
//! the bottom layer above the wallpaper (the wallpaper itself is the
//! `background` surface's, or the QML shell's). Compare shell/modules/desktop/:
//! DesktopWidgets.qml, WidgetFrame.qml, WidgetDefs.qml, each *Widget.qml,
//! AnalogClock.qml, GaugeRing.qml, DesktopIcons.qml, and Material's
//! overrides inside them (Theme.material: tonal cards per kind, the cookie
//! clock, the weather circle, state layers).
//!
//! What the QML shell still owns reaches it through DesktopFeed.qml (feed.rs:
//! weather, events, to-dos, Screen Time, the player, Bluetooth batteries,
//! Spotlight's actions, the notes' text, Edit Widgets); what only the shell
//! does it asks for (`shell::ask("desktop", [what, …])`: ticking a to-do,
//! the media keys, a shortcut, saving a note, opening Notification Center,
//! Today, Widget Options, the wallpaper picker, Quick Preview, Share).
//! Clock, world clock, System (/proc), Photos, Battery History, Stocks and
//! the Desktop folder are read here.
//!
//! Not here yet, and why:
//! - Edit Widgets' gallery and Widget Options stay QML windows (they're
//!   panels of their own); the gallery's picks reach this surface as
//!   `{"to":"desktop","add":…}` lines.
//! - The large Calendar is Notification Center's calendar in QML (Month /
//!   Week / To-do, editing); here it's the medium layout at the large size.
//! - Glass widgets sit on the glass plugin's glass (shapes sent per widget);
//!   while a widget springs back after a drag, its glass is already at the
//!   new cell. Without the plugin they're a tint.
//! - Slint can't clip a picture to a Material shape: Now Playing's cover is
//!   round while playing (the QML's cookie), a small photo round (clover).
//! - Material draws Phosphor icons, not Material Symbols (as the OSD: Slint
//!   can't set the symbols font's FILL axis the QML fills them with).
//! - No lock/unlock zoom (LockFx), no GPU graph in System, no drag of files
//!   into or out of the desktop (no drag and drop between apps here).

pub mod feed;
pub mod files;
pub mod glyphs;
pub mod grid;
pub mod sysstat;
pub mod view;

use crate::appicon::{self, Style};
use crate::services::icons::Icons;
use crate::shell::{self, Part};
use crate::theme;
use crate::ui;
use crate::wayland::{Spec, State, Surface};
use feed::Feed;
use grid::{Grid, IconItem, W};
use serde_json::{json, Value};
use slint::{Color, ComponentHandle, Model, ModelRc, SharedString, VecModel};
use smithay_client_toolkit::reexports::client::protocol::wl_output::WlOutput;
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use smithay_client_toolkit::shell::WaylandSurface;
use std::any::{Any, TypeId};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::Duration;

pub const NS: &str = "tessera:desktop";
pub const MENU_NS: &str = "tessera:desktopmenu";
const NAME: &str = "desktop";

type Pixels = slint::SharedPixelBuffer<slint::Rgba8Pixel>;

enum M {
    Act(i32, String, String),
    Drag(i32, f32, f32, bool),
    Menu(i32, f32, f32),
    Bare,
    Icon(i32, String, f32, f32),
    StackFile(i32),
    StackFileMenu(i32, f32, f32),
    Tick,
    Sample,
    Slow,
    SaveNote,
    Image(String, Option<Pixels>),
    Zones(Vec<(String, view::ZoneTime)>),
    Quotes(Vec<view::Quote>, bool),
    Battery(Vec<(i64, f64, bool)>),
    Folder(Vec<grid::Entry>),
    MenuChosen(i32),
    MenuDismissed,
    MenuClosed,
}

#[derive(Clone, Debug)]
enum Act {
    Resize(usize, &'static str),
    StackKind(String, String),
    SmartRotate,
    Button(String, String),
    TodayOptions,
    WidgetOptions(String),
    EditWidgets,
    Remove(usize),
    Wallpaper,
    UseStacks(bool),
    ShowFiles,
    Settings,
    Open(String),
    QuickLook(String),
    Share(Vec<String>),
    Reveal(String),
    CopyPath(String),
    Trash(String),
    OpenStack(String),
}

struct Row {
    text: String,
    enabled: bool,
    danger: bool,
    act: Option<Act>,
}

fn row(text: impl Into<String>, act: Act) -> Row {
    Row { text: text.into(), enabled: true, danger: false, act: Some(act) }
}
fn sep() -> Row {
    Row { text: String::new(), enabled: false, danger: false, act: None }
}
fn check(on: bool, text: &str) -> String {
    format!("{}{text}", if on { "✓ " } else { "    " })
}

struct Win {
    output: String,
    surface: Rc<Surface>,
    ui: ui::Desktop,
    widgets: Rc<VecModel<ui::DeskWidget>>,
    icons: Rc<VecModel<ui::DeskIcon>>,
    a11y: Option<crate::a11y::A11y>,
    keys: Vec<String>,
    glass_sent: String,
    kbd: Option<bool>,
}

struct MenuWin {
    surface: Rc<Surface>,
    ui: ui::DeskMenu,
    a11y: Option<crate::a11y::A11y>,
    open: bool,
    rows: Vec<Row>,
    glass_sent: String,
}

#[derive(Default)]
struct Photo {
    list: Vec<String>,
    a: String,
    b: String,
    front_a: bool,
}

#[derive(Default)]
pub struct Desktop {
    win: Option<Win>,
    menu: Option<MenuWin>,
    feed: Feed,
    list: Vec<W>,
    unknown: Vec<String>,
    placed: Vec<W>,
    grid: Option<Grid>,
    // files on the desktop
    entries: Vec<grid::Entry>,
    items: Vec<IconItem>,
    spots: HashMap<String, (i32, i32)>,
    selected: String,
    open_stack: String,
    // per widget (by key)
    cal_day: HashMap<String, (i64, i64, i64)>,
    stack_at: HashMap<String, usize>,
    stack_dir: HashMap<String, i32>,
    manual_at: i64,
    photos: HashMap<String, Photo>,
    note_pending: Option<(String, String)>,
    note_timer: slint::Timer,
    // data read here
    images: HashMap<String, slint::Image>,
    pending: HashSet<String>,
    zones: HashMap<String, view::ZoneTime>,
    quotes: HashMap<String, view::Quote>,
    stocks_failed: bool,
    stocks_for: Vec<String>,
    battery: Vec<(i64, f64, bool)>,
    sampler: sysstat::Sampler,
    stats: Vec<sysstat::Stat>,
    tick: slint::Timer,
    sample_timer: slint::Timer,
    slow_timer: slint::Timer,
    minute: i64,
    ghost: Option<(i32, i32, String)>,
    live: Option<(usize, f32, f32)>,
    menu_for: Option<usize>,
    menu_timer: slint::Timer,
    roles: HashMap<String, Color>,
    started_rotation: String,
}

fn hexc(s: &str) -> Option<Color> {
    let h = s.trim().trim_start_matches('#');
    let v = u32::from_str_radix(h.get(0..6)?, 16).ok()?;
    Some(Color::from_rgb_u8((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

/// `over` laid on `base` at `alpha` (Qt.tint)
fn tint(base: Color, over: Color, alpha: f32) -> Color {
    let m = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * alpha).round() as u8;
    Color::from_argb_u8(base.alpha(), m(base.red(), over.red()), m(base.green(), over.green()), m(base.blue(), over.blue()))
}

fn rgb(c: Color) -> [u8; 3] {
    [c.red(), c.green(), c.blue()]
}

fn strs(v: &Value) -> Vec<String> {
    v.as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default()
}

fn ask(what: &str, args: Vec<Value>) {
    let mut a = vec![json!(what)];
    a.extend(args);
    shell::ask("desktop", Value::Array(a));
}

/// Write `desktop.<key>` (or another section's) into config.json, as one
/// write in place (the shell and this process both watch the file)
fn write_cfg(section: &str, key: &str, value: Value) {
    let path = theme::config_path();
    let (section, key) = (section.to_string(), key.to_string());
    std::thread::spawn(move || {
        let mut cfg = std::fs::read_to_string(&path).ok().and_then(|s| serde_json::from_str::<Value>(&s).ok()).unwrap_or(Value::Null);
        if !cfg.is_object() {
            cfg = json!({});
        }
        if !cfg[&section].is_object() {
            cfg[&section] = json!({});
        }
        cfg[&section][&key] = value;
        if let Ok(text) = serde_json::to_string_pretty(&cfg) {
            let _ = std::fs::write(&path, text + "\n");
        }
    });
}

/// Config.qml's defaults for the desktop section
fn default_widgets() -> Vec<String> {
    ["clock:m:0:0", "weather:m:0:1", "calendar:s:0:2", "batteries:s:1:2"].iter().map(|s| s.to_string()).collect()
}
fn default_cities() -> Vec<String> {
    ["Cupertino|America/Los_Angeles", "London|Europe/London", "Tokyo|Asia/Tokyo", "Sydney|Australia/Sydney"].iter().map(|s| s.to_string()).collect()
}
fn default_stocks() -> Vec<String> {
    ["AAPL", "MSFT", "^GSPC", "BTC-USD", "NVDA", "GOOGL"].iter().map(|s| s.to_string()).collect()
}

struct Cfg {
    widgets: Vec<String>,
    icons: bool,
    use_stacks: bool,
    positions: Vec<String>,
    smart_rotate: bool,
    photos: String,
    cities: Vec<String>,
    stocks: Vec<String>,
    graph: String,
    h24: bool,
}

impl Cfg {
    fn read(cfg: &Value) -> Self {
        let d = &cfg["desktop"];
        let list = |k: &str, def: fn() -> Vec<String>| if d[k].is_array() { strs(&d[k]) } else { def() };
        let home = std::env::var("HOME").unwrap_or_default();
        Cfg {
            widgets: list("widgets", default_widgets),
            icons: d["icons"].as_bool().unwrap_or(true),
            use_stacks: d["useStacks"].as_bool().unwrap_or(false),
            positions: strs(&d["iconPositions"]),
            smart_rotate: d["smartRotate"].as_bool().unwrap_or(true),
            photos: d["photosFolder"].as_str().filter(|s| !s.is_empty()).map(String::from).unwrap_or(format!("{home}/Pictures")),
            cities: list("worldClocks", default_cities),
            stocks: list("stocks", default_stocks),
            graph: cfg["resources"]["graphStyle"].as_str().unwrap_or("area").to_string(),
            h24: cfg["appearance"]["clock24"].as_bool().unwrap_or(false),
        }
    }
}

impl Desktop {
    fn cfg(st: &State) -> Cfg {
        Cfg::read(&st.shell.cfg)
    }

    /// Material roles the Theme global doesn't carry, and the accents
    fn colors(&mut self, st: &State) {
        let look = &st.shell.look;
        self.roles.clear();
        if look.material {
            for (n, v) in theme::material_roles(&look.seed, look.dark, &look.style) {
                if let Some(c) = hexc(&v) {
                    self.roles.insert(n, c);
                }
            }
        }
        if let Some(w) = &self.win {
            let d = w.ui.global::<ui::Desk>();
            let a = |n: &str| hexc(&theme::accent_hex(n, look.dark)).unwrap_or_default();
            d.set_green(a("green"));
            d.set_red(a("red"));
            d.set_yellow(a("yellow"));
            let accent = w.ui.global::<ui::Theme>().get_accent();
            // (Theme.accentInk: the accent where it reads on the window; a
            // lighter blue in light mode, as AccentRamp picks it)
            d.set_accent_ink(if look.material { accent } else if look.dark { accent } else { tint(accent, Color::from_rgb_u8(0, 0, 0), 0.12) });
            d.set_m3_on_tertiary(self.role("onTertiary"));
            d.set_m3_secondary(self.role("secondary"));
            d.set_tile(if look.material { 28.0 } else { 36.0 });
        }
    }

    fn role(&self, n: &str) -> Color {
        self.roles.get(n).copied().unwrap_or(Color::from_rgb_u8(0x0a, 0x84, 0xff))
    }

    fn accent(&self, st: &State, n: &str) -> Color {
        hexc(&theme::accent_hex(n, st.shell.look.dark)).unwrap_or_default()
    }

    fn theme_accent(&self) -> Color {
        self.win.as_ref().map(|w| w.ui.global::<ui::Theme>().get_accent()).unwrap_or_default()
    }

    /// The first screen (Quickshell.screens[0]: widgets live on one)
    fn primary(st: &State) -> Option<(WlOutput, String)> {
        st.outputs_now().into_iter().next()
    }

    fn make(&mut self, st: &mut State, output: &WlOutput, name: &str) {
        let surface = st.prepare_surface(Spec {
            layer: Layer::Bottom,
            namespace: NS,
            output: Some((output, name)),
            anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
            size: (0, 0),
            exclusive: -1,
            keyboard: KeyboardInteractivity::None,
            margin_left: 0,
        });
        let ui = ui::Desktop::new().expect("Desktop");
        st.shell.apply_theme(&ui.global::<ui::Theme>());
        let widgets = Rc::new(VecModel::<ui::DeskWidget>::default());
        let icons = Rc::new(VecModel::<ui::DeskIcon>::default());
        ui.set_widgets(ModelRc::from(widgets.clone()));
        ui.set_icons(ModelRc::from(icons.clone()));
        ui.set_cookie12(view_shape("cookie12").into());
        ui.set_cookie9(crate::surfaces::shapes::outline("cookie9").into());
        ui.set_spot(grid::SPOT);
        ui.set_cell(grid::CELL);
        ui.set_gap(grid::GAP);
        ui.set_ox(grid::ORIGIN.0);
        ui.set_oy(grid::ORIGIN.1);
        let to = st.shell.sender(NAME);
        let t = to.clone();
        ui.on_act(move |i, a, b| t(Box::new(M::Act(i, a.into(), b.into()))));
        let t = to.clone();
        ui.on_drag(move |i, x, y, on| t(Box::new(M::Drag(i, x, y, on))));
        let t = to.clone();
        ui.on_menu(move |i, x, y| t(Box::new(M::Menu(i, x, y))));
        let t = to.clone();
        ui.on_bare(move || t(Box::new(M::Bare)));
        let t = to.clone();
        ui.on_file(move |i, a, x, y| t(Box::new(M::Icon(i, a.into(), x, y))));
        let t = to.clone();
        ui.on_stack_file(move |i, _| t(Box::new(M::StackFile(i))));
        let t = to;
        ui.on_stack_file_menu(move |i, x, y| t(Box::new(M::StackFileMenu(i, x, y))));
        ui.show().expect("show desktop");
        let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
        self.win = Some(Win { output: name.to_string(), surface, ui, widgets, icons, a11y, keys: Vec::new(), glass_sent: String::new(), kbd: None });
        self.colors(st);
    }

    fn unmake(&mut self, st: &mut State) {
        if let Some(w) = self.win.take() {
            st.shell.glass.set(&w.output, NS, &[]);
            st.drop_surface(&w.surface);
        }
        if let Some(m) = self.menu.take() {
            st.drop_surface(&m.surface);
        }
    }

    /// The layout from config.json, placed on this screen
    fn read_layout(&mut self, st: &State) {
        let c = Self::cfg(st);
        let (list, unknown) = grid::read(&c.widgets);
        self.list = list;
        self.unknown = unknown;
        let g = self.grid_now();
        self.placed = g.placed(&self.list);
        self.grid = Some(g);
    }

    fn grid_now(&self) -> Grid {
        let (w, h) = self.win.as_ref().map(|w| w.surface.logical_size()).unwrap_or((1920, 1080));
        let (w, h) = if w == 0 || h == 0 { (1920, 1080) } else { (w, h) };
        Grid::new(w as f32, h as f32)
    }

    fn save(&mut self, list: Vec<W>) {
        write_cfg("desktop", "widgets", json!(grid::entries(&list, &self.unknown)));
        // (shown at once; the config change confirms it)
        self.list = list;
        if let Some(g) = self.grid {
            self.placed = g.placed(&self.list);
        }
    }

    /// Each widget's key, unique: its kind, id and which one of those it is
    fn keys(&self) -> Vec<String> {
        let mut seen: HashMap<String, usize> = HashMap::new();
        self.list
            .iter()
            .map(|w| {
                let k = format!("{}:{}", w.kind, w.id);
                let n = seen.entry(k.clone()).or_insert(0);
                *n += 1;
                format!("{k}#{n}")
            })
            .collect()
    }

    // ── Building what's drawn ────────────────────────────────────────────

    fn update(&mut self, st: &mut State) {
        if self.win.is_none() {
            return;
        }
        let keys = self.keys();
        let rows: Vec<ui::DeskWidget> = (0..self.placed.len()).map(|i| self.widget(st, i, &keys[i])).collect();
        let icons = self.icon_rows(st);
        let c = Self::cfg(st);
        let editing = self.feed.editing && self.win.as_ref().is_some_and(|w| self.feed.edit_screen.is_empty() || self.feed.edit_screen == w.output);
        let want_keys = self.list.iter().any(|w| w.kind == "notes" || ((w.kind == "reminders" || w.kind == "calendar") && w.size == "l"));
        let glass = st.shell.plugin && !st.shell.look.material;
        let stack = self.items.iter().find(|i| i.key == self.open_stack && i.is_stack()).cloned();
        let stack_at = stack.as_ref().map(|it| self.stack_panel_at(it));
        let win = self.win.as_mut().unwrap();
        // one slot per widget, changed in place: adding or removing one
        // inserts or removes just that one (the rest don't appear again)
        let prev = std::mem::take(&mut win.keys);
        let mut head = 0;
        while head < prev.len() && head < keys.len() && prev[head] == keys[head] {
            head += 1;
        }
        let mut tail = 0;
        while tail < prev.len() - head && tail < keys.len() - head && prev[prev.len() - 1 - tail] == keys[keys.len() - 1 - tail] {
            tail += 1;
        }
        for _ in 0..prev.len() - head - tail {
            win.widgets.remove(head);
        }
        for (i, r) in rows.iter().enumerate() {
            if i >= head && i < keys.len() - tail {
                win.widgets.insert(i, r.clone());
            } else if win.widgets.row_data(i).as_ref() != Some(r) {
                win.widgets.set_row_data(i, r.clone());
            }
        }
        win.keys = keys;
        sync(&win.icons, icons);
        win.ui.set_editing(editing);
        win.ui.set_glass_plugin(glass);
        if let Some(g) = self.grid {
            win.ui.set_cols(g.cols);
            win.ui.set_rows(g.rows);
        }
        match &self.ghost {
            Some((c, r, size)) => {
                let (x, y) = Grid::home(*c, *r);
                let (pw, ph) = grid::pixel_size(size);
                win.ui.set_ghost_x(x);
                win.ui.set_ghost_y(y);
                win.ui.set_ghost_w(pw);
                win.ui.set_ghost_h(ph);
                win.ui.set_ghost_on(true);
            }
            None => win.ui.set_ghost_on(false),
        }
        // an open stack's files
        win.ui.set_stack_open(stack.is_some() && c.icons);
        if let (Some(it), Some((sx, sy))) = (stack, stack_at) {
            win.ui.set_stack_x(sx);
            win.ui.set_stack_y(sy);
            win.ui.set_stack_name(it.name.as_str().into());
        }
        if win.kbd != Some(want_keys) && win.surface.mapped.get() {
            win.surface.layer.set_keyboard_interactivity(if want_keys { KeyboardInteractivity::OnDemand } else { KeyboardInteractivity::None });
            win.surface.layer.commit();
            win.kbd = Some(want_keys);
        }
        self.stack_files(st);
    }

    fn stack_panel_at(&self, it: &IconItem) -> (f32, f32) {
        let (c, r) = self.spots.get(&it.key).copied().unwrap_or((0, 0));
        let (_, sh) = self.win.as_ref().map(|w| w.surface.logical_size()).unwrap_or((1920, 1080));
        let rows = ((it.stack.len().min(16) + 3) / 4) as f32;
        let h = rows * 96.0 + 48.0;
        let x = (grid::ORIGIN.0 + c as f32 * grid::SPOT - (4.0 * 96.0 + 24.0) + 8.0).max(8.0);
        let y = (grid::ORIGIN.1 + r as f32 * grid::SPOT).min(sh as f32 - h - 100.0);
        (x, y)
    }

    fn stack_files(&mut self, st: &mut State) {
        let Some(it) = self.items.iter().find(|i| i.key == self.open_stack).cloned() else { return };
        let files: Vec<ui::DeskFile> = it.stack.iter().take(16).map(|e| ui::DeskFile { name: e.name.as_str().into(), icon: self.file_icon(st, e) }).collect();
        if let Some(w) = &self.win {
            w.ui.set_stack_files(ModelRc::new(VecModel::from(files)));
        }
    }

    /// An image loaded off the UI thread (once; default until it's there)
    fn image(&mut self, st: &State, key: String, job: impl FnOnce() -> Option<Pixels> + Send + 'static) -> Option<slint::Image> {
        if let Some(i) = self.images.get(&key) {
            return Some(i.clone());
        }
        if self.pending.insert(key.clone()) {
            let to = st.shell.sender(NAME);
            std::thread::spawn(move || {
                let px = job();
                to(Box::new(M::Image(key, px)));
            });
        }
        None
    }

    fn scale(&self) -> f32 {
        self.win.as_ref().map_or(1.0, |w| w.ui.window().scale_factor()).max(1.0)
    }

    /// A file's icon from the icon theme (or its folder icon), unstyled
    fn file_icon(&mut self, st: &State, e: &grid::Entry) -> slint::Image {
        let Some(icons) = st.shell.get::<Icons>() else { return slint::Image::default() };
        let Some(path) = icons.path(&[files::icon_name(e)], &["text-x-generic"]) else { return slint::Image::default() };
        let px = (48.0 * self.scale()).ceil() as u32;
        let style = Style { mode: 0, tiled: false, dark: st.shell.look.dark, contrast: 60, tint: [0; 3], tile: None, tile_from: "auto".into(), accent: [0; 3] };
        let key = format!("icon:{}@{px}", path.display());
        self.image(st, key, move || appicon::render(&path, px, &style, None)).unwrap_or_default()
    }

    fn icon_rows(&mut self, st: &State) -> Vec<ui::DeskIcon> {
        if !Self::cfg(st).icons {
            return Vec::new();
        }
        let items = self.items.clone();
        let px = (104.0 * self.scale() / 2.0).ceil() as u32 * 2;
        items
            .iter()
            .enumerate()
            .map(|(i, it)| {
                let (c, r) = self.spots.get(&it.key).copied().unwrap_or((0, 0));
                let mut row = ui::DeskIcon {
                    index: i as i32,
                    name: it.name.as_str().into(),
                    x: grid::ORIGIN.0 + c as f32 * grid::SPOT,
                    y: grid::ORIGIN.1 + r as f32 * grid::SPOT,
                    selected: self.selected == it.key,
                    stack: it.is_stack(),
                    ..Default::default()
                };
                if it.is_stack() {
                    let fan: Vec<slint::Image> = it.stack.iter().take(3).map(|e| self.file_icon(st, e)).collect::<Vec<_>>().into_iter().rev().collect();
                    row.fan = ModelRc::new(VecModel::from(fan));
                } else {
                    let e = grid::Entry { name: it.name.clone(), path: it.path.clone(), dir: it.dir, modified: 0 };
                    let thumb = if files::is_picture(&e) {
                        let p = it.path.clone();
                        self.image(st, format!("thumb:{p}@{px}"), move || files::thumbnail(&p, px))
                    } else {
                        None
                    };
                    row.has_pic = thumb.is_some();
                    row.pic = thumb.unwrap_or_else(|| self.file_icon(st, &e));
                }
                row
            })
            .collect()
    }

    /// The files on the desktop, placed around the widgets
    fn place_icons(&mut self, st: &State) {
        let c = Self::cfg(st);
        self.items = if c.icons { grid::items(&self.entries, c.use_stacks) } else { Vec::new() };
        let g = self.grid.unwrap_or(Grid { cols: 1, rows: 1 });
        let saved = grid::saved_positions(&c.positions);
        self.spots = grid::place_icons(&self.items, &saved, &grid::blocked(&self.placed), g.cols * 2, g.rows * 2);
    }

    /// One widget's row (a stack: the widget on top, with its dots)
    fn widget(&mut self, st: &State, i: usize, key: &str) -> ui::DeskWidget {
        let w = self.placed[i].clone();
        let material = st.shell.look.material;
        let (x, y) = Grid::home(w.col, w.row);
        let (pw, ph) = grid::pixel_size(&w.size);
        let mut kind = w.kind.clone();
        let mut row = ui::DeskWidget { index: i as i32, key: key.into(), size: w.size.as_str().into(), x, y, w: pw, h: ph, ..Default::default() };
        if w.kind == "stack" {
            let kinds = self.stack_kinds(&w);
            let n = kinds.len();
            let at = self.stack_at.get(key).copied().unwrap_or(0).min(n.saturating_sub(1));
            row.stack = true;
            row.stack_n = n as i32;
            row.stack_i = at as i32;
            row.stack_dir = self.stack_dir.get(key).copied().unwrap_or(1);
            kind = kinds.get(at).cloned().unwrap_or_default();
            if kind.is_empty() {
                row.t1 = "Right-click to add widgets".into();
            }
        }
        row.kind = kind.as_str().into();
        // the card (Material: its tonal container per kind; a stack's is the stack's)
        let card = grid::material_card(&w.kind, &w.size);
        let bleed = grid::kind(&kind).is_some_and(|k| k.bleed);
        if material {
            row.has_card = !card.bg.is_empty();
            // (Settings' transparency: containers let the desktop through,
            // Theme.controlAlpha)
            row.card = self.role(card.bg).with_alpha(1.0 - 0.4 * st.shell.look.transparency);
            row.pill = card.pill;
            row.pad = if bleed { 0.0 } else { card.pad };
            row.pad_x = if bleed { 0.0 } else { card.pad_x };
        } else {
            row.pad = if bleed { 0.0 } else { 16.0 };
            row.pad_x = row.pad;
        }
        let cw = pw - 2.0 * row.pad_x - if row.stack && row.stack_n > 1 { 10.0 } else { 0.0 };
        let ch = ph - 2.0 * row.pad;
        let id = if w.kind == "stack" { format!("{}-{kind}", w.id) } else { w.id.clone() };
        self.fill(st, &mut row, &kind, &w.size, key, &id, cw, ch);
        row
    }

    fn stack_kinds(&self, w: &W) -> Vec<String> {
        let chosen: Vec<String> = self.feed.stacks.get(&w.id).cloned().unwrap_or_else(|| feed::STACK_DEFAULTS.iter().map(|s| s.to_string()).collect());
        chosen.into_iter().filter(|k| k != "stack" && grid::kind(k).is_some_and(|d| d.sizes.contains(&w.size.as_str()))).collect()
    }

    #[allow(clippy::too_many_arguments)]
    fn fill(&mut self, st: &State, r: &mut ui::DeskWidget, kind: &str, size: &str, key: &str, id: &str, cw: f32, ch: f32) {
        let c = Self::cfg(st);
        let material = st.shell.look.material;
        let dark = st.shell.look.dark;
        let now = crate::util::now_ms();
        let (today, hh, mm) = view::local(now);
        let accent = self.theme_accent();
        let s: fn(&str) -> SharedString = |x| x.into();
        let f = self.feed.clone();
        let ev_tint = |e: &feed::Event| hexc(&e.color).unwrap_or(accent);
        match kind {
            "clock" => {
                r.clocks = clocks(vec![clock(hh, mm, String::new(), String::new(), now)]);
                r.shape = view_shape("cookie12").into();
                r.t1 = view::fmt("%A", now).to_uppercase().into();
                r.t2 = view::hm_short(now, c.h24).into();
                r.t3 = view::fmt("%-d %B %Y", now).into();
            }
            "worldClock" => {
                let n = if size == "s" { 1 } else { 4 };
                let list: Vec<ui::DClock> = c
                    .cities
                    .iter()
                    .take(n)
                    .map(|e| {
                        let (name, zone) = view::city(e);
                        let t = self.zones.get(&zone);
                        let (h, m) = t.map_or((0, 0), |t| (t.h, t.m));
                        clock(h, m, name, t.map(view::describe).unwrap_or_default(), now)
                    })
                    .collect();
                r.clocks = clocks(list);
            }
            "weather" => {
                let wx = &f.weather;
                r.on = wx.ready;
                r.t1 = s(&wx.place);
                r.t2 = format!("{}{}", wx.temp, wx.unit).into();
                r.t3 = s(&wx.condition);
                r.t4 = format!("H:{}°  L:{}°", wx.high, wx.low).into();
                r.t5 = if wx.error.is_empty() { "Loading weather…".into() } else { s(&wx.error) };
                r.t6 = format!("{}°", wx.temp).into();
                r.g1 = glyphs::glyph(if wx.icon.is_empty() { "cloud" } else { &wx.icon }).into();
                r.shape = crate::surfaces::shapes::outline("circle").into();
                let hours: Vec<ui::DHour> = wx.hourly.iter().map(|h| ui::DHour { time: s(&h.time), glyph: glyphs::glyph(&h.icon).into(), temp: format!("{}°", h.temp).into() }).collect();
                r.hours = ModelRc::new(VecModel::from(hours));
            }
            "today" => {
                let wx = &f.weather;
                r.t1 = if f.greeting.is_empty() { "Good day".into() } else { s(&f.greeting) };
                r.t2 = view::fmt(if size == "l" { "%A, %-d %B" } else { "%a, %-d %b" }, now).into();
                r.on = wx.ready;
                r.g1 = glyphs::glyph(&wx.icon).into();
                r.t3 = format!("{}{}", wx.temp, wx.unit).into();
                r.t4 = format!("High {}{u} · Low {}{u}{}", wx.high, wx.low, if wx.rain_chance > 0 { format!(" · {}% rain", wx.rain_chance) } else { String::new() }, u = wx.unit).into();
                let day0 = view::midnight_ms(today.0, today.1, today.2);
                let evs: Vec<ui::DLine> = f
                    .events_between(day0, view::midnight_ms(today.0, today.1, today.2 + 1))
                    .into_iter()
                    .filter(|e| e.all_day || e.end > now)
                    .take(if size == "l" { 3 } else { 2 })
                    .map(|e| ui::DLine { text: s(&e.title), sub: if e.all_day { "All day".into() } else { view::hm(e.start, c.h24).into() }, tint: ev_tint(e), ..Default::default() })
                    .collect();
                r.lines = ModelRc::new(VecModel::from(evs));
                r.t5 = "Nothing else today".into();
                let todos: Vec<ui::DLine> = f.open_todos().into_iter().take(3).map(|t| ui::DLine { text: s(&t.text), key: s(&t.key), ..Default::default() }).collect();
                r.lines2 = ModelRc::new(VecModel::from(todos));
                r.t6 = "No to-dos".into();
            }
            "calendar" => {
                let sel = self.cal_day.get(key).copied().filter(|d| d.0 == today.0 && d.1 == today.1).unwrap_or(today);
                let on_today = sel == today;
                let small = size == "s";
                let sel_ms = view::midnight_ms(sel.0, sel.1, sel.2) + 12 * 3_600_000;
                r.t1 = if on_today { view::fmt("%A", sel_ms) } else { view::fmt("%a %-d %b", sel_ms) }.to_uppercase().into();
                r.t2 = today.2.to_string().into();
                r.on = small || on_today;
                let evs: Vec<&feed::Event> = if small {
                    f.upcoming.iter().take(1).collect()
                } else {
                    let d0 = view::midnight_ms(sel.0, sel.1, sel.2);
                    let d1 = view::midnight_ms(sel.0, sel.1, sel.2 + 1);
                    f.events_between(d0, d1).into_iter().filter(|e| !on_today || e.all_day || e.end > now).take(if on_today { 2 } else { 3 }).collect()
                };
                let lines: Vec<ui::DLine> = evs.iter().map(|e| ui::DLine { text: s(&e.title), sub: view::when(e.start, e.all_day, today, small, c.h24).into(), tint: ev_tint(e), ..Default::default() }).collect();
                r.lines = ModelRc::new(VecModel::from(lines));
                r.t3 = if small || on_today { "No more events today" } else { "No events" }.into();
                let busy: HashSet<String> = f.busy.iter().cloned().collect();
                let (days, weeks) = view::month(today, sel, &busy);
                let days: Vec<ui::DDay> = days.iter().map(|d| ui::DDay { label: d.day.to_string().into(), col: d.col as i32, row: d.row as i32, today: d.today, selected: d.selected, busy: d.busy }).collect();
                r.days = ModelRc::new(VecModel::from(days));
                r.n1 = weeks as f32;
                r.t4 = view::fmt("%B", now).to_uppercase().into();
            }
            "reminders" => {
                let rows = match size {
                    "l" => 8,
                    "m" => 3,
                    _ => 2,
                };
                let open = f.open_todos();
                r.t1 = open.len().to_string().into();
                let lines: Vec<ui::DLine> = open.iter().take(rows).map(|t| ui::DLine { text: s(&t.text), sub: s(&t.due_label), flag: t.overdue, key: s(&t.key), ..Default::default() }).collect();
                r.lines = ModelRc::new(VecModel::from(lines));
                r.t3 = if f.todos.is_empty() { "No reminders" } else { "All done" }.into();
                r.t4 = if open.len() > rows { format!("{} more", open.len() - rows).into() } else { SharedString::new() };
            }
            "batteries" => {
                let green = self.accent(st, "green");
                let red = self.accent(st, "red");
                let mut gauges = Vec::new();
                for b in f.batteries.iter().take(4) {
                    let charging = b.icon == "lightning";
                    let tint = if material {
                        if !charging && b.value <= 0.2 { self.role("error") } else { self.role("primary") }
                    } else if !charging && b.value <= 0.2 {
                        red
                    } else {
                        green
                    };
                    let pic = if b.picture.is_empty() {
                        None
                    } else {
                        let p = b.picture.trim_start_matches("file://").to_string();
                        let px = (76.0 * self.scale()) as u32;
                        // (the shell's device drawings are SVG)
                        let plain = Style { mode: 0, tiled: false, dark, contrast: 60, tint: [0; 3], tile: None, tile_from: "auto".into(), accent: [0; 3] };
                        self.image(st, format!("pic:{p}@{px}"), move || if p.ends_with(".svg") { appicon::render(std::path::Path::new(&p), px, &plain, None) } else { files::thumbnail(&p, px) })
                    };
                    gauges.push(ui::DGauge { value: b.value as f32, glyph: glyphs::glyph(&b.icon).into(), has_picture: pic.is_some(), picture: pic.unwrap_or_default(), tint, text: format!("{}%", (b.value * 100.0).round()).into(), name: s(&b.name) });
                }
                r.gauges = ModelRc::new(VecModel::from(gauges));
                r.t1 = "No batteries".into();
            }
            "nowPlaying" => {
                let m = &f.media;
                r.t1 = if m.title.is_empty() { "Not Playing".into() } else { s(&m.title) };
                r.t2 = s(&m.artist);
                r.on = m.playing;
                r.flag = m.has;
                if !m.art.is_empty() {
                    let url = m.art.clone();
                    let px = (164.0 * 2.0 * self.scale()) as u32;
                    let img = self.image(st, format!("art:{url}"), move || art(&url, px));
                    r.has_img = img.is_some();
                    r.img = img.unwrap_or_default();
                }
            }
            "system" => {
                let have: Vec<&str> = self.stats.iter().map(|s| s.key).collect();
                let keys = sysstat::keys_for(size, &have);
                let cols = if size == "s" { 1 } else { 2 };
                let rows = keys.len().div_ceil(cols);
                let sw = (cw - (cols - 1) as f32 * 20.0) / cols as f32;
                let sh = (ch - (rows.max(1) - 1) as f32 * 14.0) / rows.max(1) as f32;
                let gh = (sh - 22.0).max(1.0);
                let stats: Vec<ui::DStat> = keys
                    .iter()
                    .filter_map(|k| self.stats.iter().find(|s| s.key == k))
                    .map(|x| {
                        let (area, line) = sysstat::graph(&x.history, x.max, sw, gh, &c.graph, 1.5);
                        let tone = if material {
                            self.role(match x.tone {
                                "green" | "teal" => "tertiary",
                                "purple" | "orange" => "secondary",
                                _ => "primary",
                            })
                        } else {
                            self.accent(st, x.tone)
                        };
                        // (bars are filled shapes: drawn as the area, full strength)
                        let (area, line, tone) = if c.graph == "bars" { (area, String::new(), tone) } else { (area, line, tone) };
                        ui::DStat { label: x.label.into(), text: s(&x.text), tint: tone, area: area.into(), line: line.into(), gw: sw, gh }
                    })
                    .collect();
                if c.graph == "bars" {
                    // (the area's alpha is for the soft fill; bars want the solid tone)
                    r.flag = true;
                }
                r.stats = ModelRc::new(VecModel::from(stats));
                r.n1 = cols as f32;
            }
            "photos" => {
                let p = self.photos.entry(key.to_string()).or_default();
                if p.list.is_empty() {
                    p.list = files::pictures(&c.photos);
                    if !p.list.is_empty() && p.a.is_empty() {
                        p.a = p.list[pick(key, p.list.len())].clone();
                        p.front_a = true;
                    }
                }
                let (a, b, front_a, n) = (p.a.clone(), p.b.clone(), p.front_a, p.list.len());
                let (w, h) = ((cw * self.scale()) as u32, (ch * self.scale()) as u32);
                let load = |me: &mut Self, path: &str| -> Option<slint::Image> {
                    if path.is_empty() {
                        return None;
                    }
                    let p = path.to_string();
                    me.image(st, format!("photo:{p}@{w}x{h}"), move || files::cover(&p, w.max(1), h.max(1)))
                };
                let ia = load(self, &a);
                let ib = load(self, &b);
                r.has_img = n > 0;
                r.front = front_a;
                r.img = ia.unwrap_or_default();
                r.img2 = ib.unwrap_or_default();
                r.on = n > 1;
                let home = std::env::var("HOME").unwrap_or_default();
                r.t1 = format!("Add pictures to {}", c.photos.replacen(&home, "~", 1)).into();
            }
            "notes" => {
                r.note = s(f.notes.get(id).map(String::as_str).unwrap_or(""));
            }
            "shortcuts" => {
                let slots = if size == "s" { 4 } else { 8 };
                let chosen: Vec<String> = f.shortcut_buttons.get(id).cloned().unwrap_or_else(|| feed::SHORTCUT_DEFAULTS.iter().map(|s| s.to_string()).collect());
                let tints = ["blue", "purple", "orange", "green", "red", "pink", "yellow", "graphite"];
                let roles = [("primaryContainer", "onPrimaryContainer"), ("secondaryContainer", "onSecondaryContainer"), ("tertiaryContainer", "onTertiaryContainer")];
                let buttons: Vec<ui::DButton> = chosen
                    .iter()
                    .filter_map(|t| f.actions.iter().find(|a| &a.title == t))
                    .take(slots)
                    .enumerate()
                    .map(|(i, a)| ui::DButton {
                        title: s(&a.title),
                        label: feed::short_name(&a.title).into(),
                        glyph: glyphs::glyph(&a.glyph).into(),
                        tint: self.accent(st, tints[i % 8]),
                        bg: self.role(roles[i % 3].0).with_alpha(1.0 - 0.4 * st.shell.look.transparency),
                        ink: self.role(roles[i % 3].1),
                    })
                    .collect();
                r.buttons = ModelRc::new(VecModel::from(buttons));
                r.t1 = "Right-click to choose buttons".into();
            }
            "screenTime" => {
                let t = &f.screen_time;
                r.t1 = if t.total > 0 { view::duration(t.total).into() } else { "—".into() };
                let max = t.week.iter().map(|d| d.seconds).max().unwrap_or(0).max(1);
                let bars: Vec<ui::DBar> = t.week.iter().enumerate().map(|(i, d)| ui::DBar { label: s(&d.label), frac: d.seconds as f32 / max as f32, today: i == 6 }).collect();
                r.bars = ModelRc::new(VecModel::from(bars));
                let most = t.apps.first().map_or(1, |a| a.seconds).max(1);
                let apps: Vec<feed::AppTime> = t.apps.iter().take(if size == "s" { 2 } else { 3 }).cloned().collect();
                let rows: Vec<ui::DApp> = apps
                    .iter()
                    .map(|a| {
                        let icon = self.app_icon(st, &a.icon);
                        ui::DApp { name: s(if a.name.is_empty() { &a.id } else { &a.name }), time: view::duration(a.seconds).into(), frac: a.seconds as f32 / most as f32, has_icon: icon.is_some(), icon: icon.unwrap_or_default() }
                    })
                    .collect();
                r.apps = ModelRc::new(VecModel::from(rows));
                r.t3 = "Nothing yet today".into();
            }
            "batteryHistory" => {
                let p = &f.power;
                let level = p.level;
                r.alert = level < 0.2 && !p.charging;
                r.g1 = glyphs::glyph(if p.charging { "battery-charging" } else { "battery-high" }).into();
                r.t1 = format!("{}%", (level * 100.0).round()).into();
                r.t2 = if size == "s" { "24h" } else { "24 hours ago" }.into();
                r.on = self.battery.len() >= 2;
                let gw = cw;
                let gh = (ch - 38.0 - 18.0).max(1.0);
                r.n1 = gw;
                r.n2 = gh;
                let on = if material { self.role("primary") } else { self.accent(st, "green") };
                let off = if material { self.role("outline") } else { hexc(if dark { "#A1A1AA" } else { "#71717A" }).unwrap_or_default() };
                let runs: Vec<ui::DRun> = view::battery_runs(&self.battery, now / 1000, gw, gh).into_iter().map(|(a, l, chg)| ui::DRun { area: a.into(), line: l.into(), tint: if chg { on } else { off } }).collect();
                r.runs = ModelRc::new(VecModel::from(runs));
            }
            "stocks" => {
                let rows = match size {
                    "l" => 6,
                    "m" => 3,
                    _ => 2,
                };
                r.flag = self.stocks_failed;
                r.t1 = "Couldn't load prices. Check your connection.".into();
                let green = self.accent(st, "green");
                let red = self.accent(st, "red");
                let up_tone = if material { tint(hexc(if dark { "#4ADE80" } else { "#15803D" }).unwrap_or_default(), self.role("primary"), 0.2) } else { green };
                let stocks: Vec<ui::DStock> = c
                    .stocks
                    .iter()
                    .take(rows)
                    .map(|sym| {
                        let q = self.quotes.get(sym);
                        let ch = q.map_or(0.0, view::change);
                        let up = ch >= 0.0;
                        let tone = if up { up_tone } else if material { self.role("error") } else { red };
                        let tone_text = if material {
                            if up { up_tone } else { self.role("onErrorContainer") }
                        } else {
                            hexc(match (up, dark) {
                                (true, true) => "#4ADE80",
                                (true, false) => "#15803D",
                                (false, true) => "#F87171",
                                (false, false) => "#B91C1C",
                            })
                            .unwrap_or_default()
                        };
                        let chip = if material && !up { self.role("errorContainer") } else { tone.with_alpha(if dark { 0.22 } else { 0.16 }) };
                        ui::DStock {
                            sym: sym.trim_start_matches('^').into(),
                            name: s(q.map_or("", |q| q.n.as_str())),
                            price: q.map_or("—".to_string(), |q| view::price(q.p)).into(),
                            change: q.map_or("…".to_string(), |_| view::change_text(ch)).into(),
                            has: q.is_some(),
                            tone,
                            tone_text,
                            chip,
                            spark: q.map(|q| view::spark(q, 100.0, 24.0)).unwrap_or_default().into(),
                        }
                    })
                    .collect();
                r.stocks = ModelRc::new(VecModel::from(stocks));
            }
            _ => {}
        }
    }

    /// An app's icon for Screen Time (Apps.iconFor: a file, or a theme name)
    fn app_icon(&mut self, st: &State, src: &str) -> Option<slint::Image> {
        let path = if let Some(p) = src.strip_prefix("file://") {
            Some(std::path::PathBuf::from(p))
        } else if src.starts_with('/') {
            Some(std::path::PathBuf::from(src))
        } else {
            let name = src.trim_start_matches("image://icon/");
            st.shell.get::<Icons>().and_then(|i| i.path(&[name], &["application-x-executable"]))
        }?;
        let px = (18.0 * self.scale()).ceil() as u32 * 2;
        let w = self.win.as_ref()?;
        let t = w.ui.global::<ui::Theme>();
        let style = Style::from_settings(&st.shell.cfg, st.shell.look.material, st.shell.look.dark, rgb(t.get_accent()), (rgb(t.get_m3_primary_container()), rgb(t.get_m3_on_primary_container())));
        self.image(st, format!("app:{}@{px}", path.display()), move || appicon::render(&path, px, &style, None))
    }

    // ── What's read here, now and then ───────────────────────────────────

    fn kinds_shown(&self) -> HashSet<String> {
        let mut k = HashSet::new();
        for w in &self.list {
            if w.kind == "stack" {
                k.extend(self.stack_kinds(w));
            } else {
                k.insert(w.kind.clone());
            }
        }
        k
    }

    fn refresh_zones(&self, st: &State) {
        let c = Self::cfg(st);
        let zones: Vec<String> = c.cities.iter().take(4).map(|e| view::city(e).1).collect();
        let to = st.shell.sender(NAME);
        std::thread::spawn(move || {
            let mut script = String::from("date '+here %H %M %z %j'; for z in \"$@\"; do echo \"$z $(TZ=$z date '+%H %M %z %j')\"; done");
            script.push('\n');
            let out = std::process::Command::new("sh").arg("-c").arg(script).arg("sh").args(&zones).output();
            if let Ok(o) = out {
                to(Box::new(M::Zones(view::zone_times(&String::from_utf8_lossy(&o.stdout)))));
            }
        });
    }

    fn refresh_stocks(&mut self, st: &State) {
        let c = Self::cfg(st);
        self.stocks_for = c.stocks.clone();
        let Some(script) = crate::util::asset("shell/assets/scripts/stocks.sh") else { return };
        let syms: Vec<String> = c.stocks.into_iter().take(6).collect();
        let to = st.shell.sender(NAME);
        std::thread::spawn(move || {
            let out = std::process::Command::new("bash").arg(script).args(&syms).stderr(std::process::Stdio::null()).output();
            let quotes: Vec<view::Quote> = out.map(|o| String::from_utf8_lossy(&o.stdout).lines().filter_map(view::quote).collect()).unwrap_or_default();
            let failed = quotes.is_empty();
            to(Box::new(M::Quotes(quotes, failed)));
        });
    }

    fn refresh_battery(&self, st: &State) {
        let Some(script) = crate::util::asset("shell/assets/scripts/battery-history.sh") else { return };
        let to = st.shell.sender(NAME);
        std::thread::spawn(move || {
            let out = std::process::Command::new("bash").arg(script).stderr(std::process::Stdio::null()).output();
            to(Box::new(M::Battery(out.map(|o| view::battery_samples(&String::from_utf8_lossy(&o.stdout))).unwrap_or_default())));
        });
    }

    fn refresh_folder(&self, st: &State) {
        let to = st.shell.sender(NAME);
        std::thread::spawn(move || to(Box::new(M::Folder(files::list(&files::desktop_dir())))));
    }

    /// Every 5 minutes / 15 minutes: the slow readings, Smart Rotate, photos
    fn slow(&mut self, st: &mut State, first: bool) {
        let k = self.kinds_shown();
        if k.contains("batteryHistory") {
            self.refresh_battery(st);
        }
        let quarter = first || (crate::util::now_ms() / 300_000) % 3 == 0;
        if k.contains("stocks") && quarter {
            self.refresh_stocks(st);
        }
        if quarter && !first {
            let keys: Vec<String> = self.photos.keys().cloned().collect();
            for key in keys {
                self.next_photo(&key);
            }
        }
        self.rotate(st);
    }

    fn next_photo(&mut self, key: &str) {
        let Some(p) = self.photos.get_mut(key) else { return };
        if p.list.len() < 2 {
            return;
        }
        let n = pick(&format!("{key}{}", crate::util::now_ms()), p.list.len());
        let next = p.list[n].clone();
        if p.front_a {
            p.b = next;
        } else {
            p.a = next;
        }
        p.front_a = !p.front_a;
    }

    // ── Smart Stacks ─────────────────────────────────────────────────────

    /// What matters now (StackWidget.relevant)
    fn relevant(&self, kinds: &[String]) -> Option<String> {
        let has = |k: &str| kinds.iter().any(|x| x == k);
        if self.feed.media.playing && has("nowPlaying") {
            return Some("nowPlaying".into());
        }
        let now = crate::util::now_ms();
        if let Some(e) = self.feed.upcoming.first() {
            if !e.all_day && e.start - now < 30 * 60_000 && has("calendar") {
                return Some("calendar".into());
            }
        }
        if self.feed.power.on_battery && self.feed.power.level < 0.2 && has("batteries") {
            return Some("batteries".into());
        }
        None
    }

    fn rotate(&mut self, st: &State) {
        if !Self::cfg(st).smart_rotate || crate::util::now_ms() - self.manual_at < 15 * 60_000 {
            return;
        }
        let keys = self.keys();
        let mut said = String::new();
        for (i, w) in self.list.clone().iter().enumerate() {
            if w.kind != "stack" {
                continue;
            }
            let kinds = self.stack_kinds(w);
            if let Some(r) = self.relevant(&kinds) {
                said += &r;
                if let Some(n) = kinds.iter().position(|k| *k == r) {
                    self.go(&keys[i], n, kinds.len());
                }
            }
        }
        self.started_rotation = said;
    }

    fn go(&mut self, key: &str, n: usize, len: usize) {
        if len == 0 {
            return;
        }
        let cur = self.stack_at.get(key).copied().unwrap_or(0);
        if n == cur {
            return;
        }
        self.stack_dir.insert(key.into(), if n > cur { 1 } else { -1 });
        self.stack_at.insert(key.into(), n % len);
    }

    // ── What a widget's control did ──────────────────────────────────────

    fn act(&mut self, st: &mut State, i: usize, what: &str, arg: &str) {
        let Some(w) = self.placed.get(i).cloned() else { return };
        let key = self.keys().get(i).cloned().unwrap_or_default();
        match what {
            "day" => {
                let now = crate::util::now_ms();
                let (today, _, _) = view::local(now);
                let d: i64 = arg.parse().unwrap_or(today.2);
                let cur = self.cal_day.get(&key).copied().unwrap_or(today);
                let pick = (today.0, today.1, d);
                // a second click on the chosen date goes back to today
                self.cal_day.insert(key, if cur == pick && pick != today { today } else { pick });
            }
            "calendar" => ask("calendar", vec![]),
            "today" => ask("today", vec![json!(false)]),
            "tick" => {
                // (ticked at once here; the shell's list follows)
                if let Some(t) = self.feed.todos.iter_mut().find(|t| t.key == arg) {
                    t.done = !t.done;
                }
                ask("todo", vec![json!(arg)]);
            }
            "add" => ask("todoAdd", vec![json!(arg)]),
            "media" => ask("media", vec![json!(arg)]),
            "shortcut" => ask("shortcut", vec![json!(arg)]),
            "photo" => self.next_photo(&key),
            "note" => {
                let id = w.id.clone();
                self.feed.notes.insert(id.clone(), arg.to_string());
                self.note_pending = Some((id, arg.to_string()));
                let to = st.shell.sender(NAME);
                self.note_timer.start(slint::TimerMode::SingleShot, Duration::from_millis(500), move || to(Box::new(M::SaveNote)));
                return; // (no redraw: the field has it)
            }
            "stack" | "scroll" => {
                let kinds = self.stack_kinds(&w);
                let cur = self.stack_at.get(&key).copied().unwrap_or(0);
                let n = if what == "stack" { arg.parse().unwrap_or(0) } else { (cur as i64 + arg.parse::<i64>().unwrap_or(1)).rem_euclid(kinds.len().max(1) as i64) as usize };
                self.go(&key, n, kinds.len());
                self.manual_at = crate::util::now_ms();
            }
            "remove" => self.save(grid::remove(&self.list, i)),
            _ => {}
        }
        self.after_layout(st);
    }

    fn after_layout(&mut self, st: &mut State) {
        self.place_icons(st);
        self.update(st);
    }

    fn drag(&mut self, st: &mut State, i: usize, x: f32, y: f32, on: bool) {
        let Some(g) = self.grid else { return };
        if on {
            self.live = Some((i, x, y));
            let (c, r) = Grid::cell_at(x, y);
            self.ghost = g.nearest_free(&self.placed, c, r, &self.placed[i].size, Some(i)).map(|(c, r)| (c, r, self.placed[i].size.clone()));
        } else {
            self.live = None;
            self.ghost = None;
            // (the placed layout is what you see: moving saves that)
            let base = self.placed.clone();
            match g.move_to(&base, i, x, y) {
                Some(l) => self.save(l),
                None => {}
            }
            self.after_layout(st);
            return;
        }
        self.update(st);
    }

    fn add(&mut self, st: &mut State, kind: &str, size: &str) {
        let Some(g) = self.grid else { return };
        let Some(def) = grid::kind(kind) else { return };
        let size = if def.sizes.contains(&size) { size } else { def.sizes[0] };
        let id = if def.own_id { grid::new_id(crate::util::now_ms() as u64 ^ self.list.len() as u64) } else { String::new() };
        match g.add(&self.placed, kind, size, &id) {
            Some(l) => self.save(l),
            None => ask("noRoom", vec![]),
        }
        self.after_layout(st);
    }

    // ── Files ────────────────────────────────────────────────────────────

    fn icon(&mut self, st: &mut State, i: usize, what: &str, x: f32, y: f32) {
        let Some(it) = self.items.get(i).cloned() else { return };
        match what {
            "click" => {
                if it.is_stack() {
                    self.open_stack = if self.open_stack == it.key { String::new() } else { it.key.clone() };
                } else {
                    self.selected = it.key.clone();
                }
            }
            "grab" => self.selected = it.key.clone(),
            "open" => {
                if !it.is_stack() {
                    ask("open", vec![json!(it.path)]);
                }
            }
            "menu" => {
                self.selected = it.key.clone();
                let rows = self.file_menu(st, &it);
                self.open_menu(st, rows, x, y);
            }
            "drop" => {
                let g = self.grid.unwrap_or(Grid { cols: 1, rows: 1 });
                let c = Self::cfg(st);
                if let Some(lines) = grid::move_icon(&it.key, x, y, &self.spots, &grid::blocked(&self.placed), g.cols * 2, g.rows * 2, &c.positions, &self.items) {
                    // (shown at once; the config change confirms it)
                    if let Some((cc, rr)) = lines.last().and_then(|l| {
                        let f: Vec<&str> = l.split('\t').collect();
                        Some((f.get(1)?.parse().ok()?, f.get(2)?.parse().ok()?))
                    }) {
                        self.spots.insert(it.key.clone(), (cc, rr));
                    }
                    write_cfg("desktop", "iconPositions", json!(lines));
                }
            }
            _ => {}
        }
        self.update(st);
    }

    fn file_menu(&self, st: &State, it: &IconItem) -> Vec<Row> {
        if it.is_stack() {
            let n = it.stack.len();
            return vec![
                row(format!("Open {}", it.name), Act::OpenStack(it.key.clone())),
                row(format!("Share {n} {}", if n == 1 { "file…" } else { "files…" }), Act::Share(it.stack.iter().map(|e| e.path.clone()).collect())),
                sep(),
                row("✓ Use Stacks", Act::UseStacks(false)),
            ];
        }
        let _ = st;
        let p = it.path.clone();
        vec![
            row("Open", Act::Open(p.clone())),
            row("Quick Preview", Act::QuickLook(p.clone())),
            row("Share…", Act::Share(vec![p.clone()])),
            row("Show in Files", Act::Reveal(p.clone())),
            row("Copy Path", Act::CopyPath(p.clone())),
            sep(),
            Row { danger: true, ..row("Move to Trash", Act::Trash(p)) },
        ]
    }

    // ── The right-click menus ────────────────────────────────────────────

    fn widget_menu(&self, st: &State, i: usize) -> Vec<Row> {
        let c = Self::cfg(st);
        let mut rows = Vec::new();
        let Some(w) = self.placed.get(i) else { return rows };
        let def = grid::kind(&w.kind);
        for s in def.map_or(&[][..], |d| d.sizes) {
            rows.push(Row { enabled: *s != w.size, ..row(check(*s == w.size, grid::size_name(s)), Act::Resize(i, s)) });
        }
        // its own items
        let inner = if w.kind == "stack" {
            let keys = self.keys();
            let kinds = self.stack_kinds(w);
            kinds.get(self.stack_at.get(&keys[i]).copied().unwrap_or(0)).cloned().unwrap_or_default()
        } else {
            w.kind.clone()
        };
        let id = if w.kind == "stack" { format!("{}-{inner}", w.id) } else { w.id.clone() };
        match inner.as_str() {
            "today" => {
                rows.push(sep());
                rows.push(row("Today Options…", Act::TodayOptions));
            }
            "shortcuts" => {
                let chosen: Vec<String> = self.feed.shortcut_buttons.get(&id).cloned().unwrap_or_else(|| feed::SHORTCUT_DEFAULTS.iter().map(|s| s.to_string()).collect());
                rows.push(sep());
                for a in &self.feed.actions {
                    rows.push(row(check(chosen.contains(&a.title), a.title.trim_end_matches('…')), Act::Button(id.clone(), a.title.clone())));
                }
            }
            _ => {}
        }
        if w.kind == "stack" {
            let on: Vec<String> = self.feed.stacks.get(&w.id).cloned().unwrap_or_else(|| feed::STACK_DEFAULTS.iter().map(|s| s.to_string()).collect());
            rows.push(sep());
            for k in grid::KINDS.iter().filter(|k| k.id != "stack" && k.id != "notes" && k.sizes.contains(&w.size.as_str())) {
                rows.push(row(check(on.iter().any(|x| x == k.id), k.label), Act::StackKind(w.id.clone(), k.id.to_string())));
            }
            rows.push(sep());
            rows.push(row(check(c.smart_rotate, "Smart Rotate"), Act::SmartRotate));
        }
        if ["photos", "worldClock", "stocks", "stack", "system"].contains(&w.kind.as_str()) {
            rows.push(sep());
            rows.push(row("Widget Options…", Act::WidgetOptions(w.kind.clone())));
        }
        rows.push(sep());
        rows.push(row("Edit Widgets…", Act::EditWidgets));
        rows.push(Row { danger: true, ..row("Remove Widget", Act::Remove(i)) });
        rows
    }

    fn desktop_menu(st: &State) -> Vec<Row> {
        let c = Self::cfg(st);
        vec![
            row("Edit Widgets…", Act::EditWidgets),
            row("Change Wallpaper…", Act::Wallpaper),
            sep(),
            row(check(c.use_stacks, "Use Stacks"), Act::UseStacks(!c.use_stacks)),
            row(check(c.icons, "Show Files on Desktop"), Act::ShowFiles),
            sep(),
            row("Desktop Settings…", Act::Settings),
        ]
    }

    fn open_menu(&mut self, st: &mut State, rows: Vec<Row>, x: f32, y: f32) {
        let Some(win) = &self.win else { return };
        let output = win.output.clone();
        if self.menu.is_none() {
            let Some(wl) = st.output_named(&output) else { return };
            let surface = st.prepare_surface(Spec {
                layer: Layer::Overlay,
                namespace: MENU_NS,
                output: Some((&wl, &output)),
                anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
                size: (0, 0),
                exclusive: -1,
                keyboard: KeyboardInteractivity::Exclusive,
                margin_left: 0,
            });
            let ui = ui::DeskMenu::new().expect("DeskMenu");
            st.shell.apply_theme(&ui.global::<ui::Theme>());
            let to = st.shell.sender(NAME);
            let (t2, t3) = (to.clone(), to.clone());
            ui.on_chosen(move |i| to(Box::new(M::MenuChosen(i))));
            ui.on_dismissed(move || t2(Box::new(M::MenuDismissed)));
            ui.on_closed(move || t3(Box::new(M::MenuClosed)));
            ui.show().expect("show desktop menu");
            let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
            self.menu = Some(MenuWin { surface, ui, a11y, open: false, rows: Vec::new(), glass_sent: String::new() });
        }
        let mw = self.menu.as_mut().unwrap();
        let ui_rows: Vec<ui::DeskMenuRow> = rows.iter().map(|r| ui::DeskMenuRow { text: r.text.as_str().into(), enabled: r.enabled, danger: r.danger, separator: r.act.is_none() }).collect();
        // (ContextMenu.qml: about 7.5 px a character, at least 200)
        let widest = rows.iter().map(|r| r.text.chars().count()).max().unwrap_or(0);
        mw.ui.set_menu_width((widest as f32 * 7.5 + 40.0).max(200.0));
        mw.ui.set_rows(ModelRc::new(VecModel::from(ui_rows)));
        mw.ui.set_point_x(x);
        mw.ui.set_point_y(y);
        mw.ui.set_glass_plugin(st.shell.plugin && !st.shell.look.material);
        mw.rows = rows;
        // (takes the whole screen's clicks and the keyboard while open)
        st.set_input_region(&mw.surface, None);
        mw.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        mw.surface.show(0, 0);
        mw.open = true;
        mw.ui.set_shown(true);
        mw.ui.invoke_open();
    }

    /// Close the menu: its exit animation (120 ms), then unmapped (a timer
    /// here: the overlay must never stay up over the desktop, taking clicks)
    fn close_menu(&mut self, st: &State) {
        if let Some(mw) = &mut self.menu {
            if mw.open {
                mw.open = false;
                mw.ui.set_shown(false);
                let to = st.shell.sender(NAME);
                self.menu_timer.start(slint::TimerMode::SingleShot, Duration::from_millis(180), move || to(Box::new(M::MenuClosed)));
            }
        }
    }

    fn menu_chosen(&mut self, st: &mut State, i: i32) {
        let Some(act) = self.menu.as_ref().and_then(|m| m.rows.get(i as usize)).and_then(|r| r.act.clone()) else { return };
        self.close_menu(st);
        let c = Self::cfg(st);
        match act {
            Act::Resize(i, s) => {
                if let Some(g) = self.grid {
                    if let Some(l) = g.resize(&self.placed, i, s) {
                        self.save(l);
                    }
                }
            }
            Act::StackKind(id, kind) => {
                let mut on: Vec<String> = self.feed.stacks.get(&id).cloned().unwrap_or_else(|| feed::STACK_DEFAULTS.iter().map(|s| s.to_string()).collect());
                if on.contains(&kind) {
                    on.retain(|k| *k != kind);
                } else {
                    on.push(kind);
                }
                self.feed.stacks.insert(id.clone(), on.clone());
                ask("stacks", vec![json!(id), json!(on)]);
            }
            Act::SmartRotate => write_cfg("desktop", "smartRotate", json!(!c.smart_rotate)),
            Act::Button(id, title) => {
                let mut l: Vec<String> = self.feed.shortcut_buttons.get(&id).cloned().unwrap_or_else(|| feed::SHORTCUT_DEFAULTS.iter().map(|s| s.to_string()).collect());
                if l.contains(&title) {
                    l.retain(|t| *t != title);
                } else {
                    l.push(title);
                }
                self.feed.shortcut_buttons.insert(id.clone(), l.clone());
                ask("buttons", vec![json!(id), json!(l)]);
            }
            Act::TodayOptions => ask("today", vec![json!(true)]),
            Act::WidgetOptions(k) => ask("options", vec![json!(k)]),
            Act::EditWidgets => ask("edit", vec![]),
            Act::Remove(i) => self.save(grid::remove(&self.list, i)),
            Act::Wallpaper => ask("show", vec![json!("wallpaper")]),
            Act::UseStacks(on) => write_cfg("desktop", "useStacks", json!(on)),
            Act::ShowFiles => write_cfg("desktop", "icons", json!(!c.icons)),
            Act::Settings => shell::ask("panels.openSettings", json!(["desktop"])),
            Act::Open(p) => ask("open", vec![json!(p)]),
            Act::QuickLook(p) => ask("quickLook", vec![json!(p)]),
            Act::Share(ps) => ask("share", vec![json!(ps)]),
            Act::Reveal(p) => ask("reveal", vec![json!(p)]),
            Act::CopyPath(p) => crate::util::pipe_to(&["wl-copy"], p.into_bytes()),
            Act::Trash(p) => {
                files::trash(&p);
                ask("sound", vec![json!("trash")]);
            }
            Act::OpenStack(k) => self.open_stack = k,
        }
        self.after_layout(st);
    }

    fn apply_theme(&mut self, st: &State) {
        if let Some(w) = &mut self.win {
            st.shell.apply_theme(&w.ui.global::<ui::Theme>());
            w.glass_sent.clear();
        }
        if let Some(m) = &mut self.menu {
            st.shell.apply_theme(&m.ui.global::<ui::Theme>());
            m.glass_sent.clear();
        }
        self.colors(st);
        // (icons drawn in the old look's style)
        self.images.retain(|k, _| !k.starts_with("app:"));
    }

    fn start_timers(&mut self, st: &State) {
        let to = st.shell.sender(NAME);
        let t = to.clone();
        self.tick.start(slint::TimerMode::Repeated, Duration::from_secs(1), move || t(Box::new(M::Tick)));
        let t = to.clone();
        self.sample_timer.start(slint::TimerMode::Repeated, Duration::from_secs(1), move || t(Box::new(M::Sample)));
        self.slow_timer.start(slint::TimerMode::Repeated, Duration::from_secs(300), move || to(Box::new(M::Slow)));
    }

    fn ensure(&mut self, st: &mut State) {
        let Some((o, n)) = Self::primary(st) else {
            self.unmake(st);
            return;
        };
        if self.win.as_ref().is_some_and(|w| w.output != n) {
            self.unmake(st);
        }
        if self.win.is_none() {
            self.make(st, &o, &n);
            self.read_layout(st);
            self.place_icons(st);
            self.update(st);
        }
    }
}

/// A random-ish pick (no rand crate): from a key and the time
fn pick(key: &str, n: usize) -> usize {
    if n == 0 {
        return 0;
    }
    let mut h: u64 = 1469598103934665603 ^ crate::util::now_ms() as u64;
    for b in key.bytes() {
        h = (h ^ b as u64).wrapping_mul(1099511628211);
    }
    (h % n as u64) as usize
}

/// A Material shape's outline: cookie12 (the clock's twelve scallops) here,
/// the rest from surfaces::shapes
fn view_shape(name: &str) -> String {
    if name != "cookie12" {
        return crate::surfaces::shapes::outline(name);
    }
    let mut d = String::new();
    for i in 0..=240 {
        let t = i as f64 / 240.0 * std::f64::consts::TAU;
        let rr = 47.0 * (1.0 + 0.045 * (12.0 * t).cos());
        let (x, y) = (50.0 + rr * (t - std::f64::consts::FRAC_PI_2).cos(), 50.0 + rr * (t - std::f64::consts::FRAC_PI_2).sin());
        d += &format!("{}{x:.2} {y:.2} ", if i == 0 { "M " } else { "L " });
    }
    d + "Z"
}

/// A clock face's hands (angles that only ever grow, so a hand never runs
/// backward over 12)
fn clock(h: i64, m: i64, name: String, sub: String, now: i64) -> ui::DClock {
    let days = (now / 86_400_000) as f32;
    let mins = (h * 60 + m) as f32;
    ui::DClock { angle_h: days * 720.0 + mins * 0.5, angle_m: days * 8640.0 + mins * 6.0, night: !(6..18).contains(&h), name: name.into(), sub: sub.into() }
}

fn clocks(v: Vec<ui::DClock>) -> ModelRc<ui::DClock> {
    ModelRc::new(VecModel::from(v))
}

/// Album art: a file, or fetched (http)
fn art(url: &str, px: u32) -> Option<Pixels> {
    if let Some(p) = url.strip_prefix("file://") {
        return files::thumbnail(&percent_decode(p), px);
    }
    if url.starts_with("http") {
        let out = std::process::Command::new("curl").args(["-sfL", "--max-time", "8", url]).stderr(std::process::Stdio::null()).output().ok()?;
        let img = image::load_from_memory(&out.stdout).ok()?.thumbnail(px, px).to_rgba8();
        let (w, h) = img.dimensions();
        return Some(slint::SharedPixelBuffer::clone_from_slice(img.as_raw(), w, h));
    }
    None
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
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

/// A model's rows made these, changing only what changed
fn sync<T: Clone + PartialEq + 'static>(m: &VecModel<T>, rows: Vec<T>) {
    for (i, r) in rows.iter().enumerate() {
        if i < m.row_count() {
            if m.row_data(i).as_ref() != Some(r) {
                m.set_row_data(i, r.clone());
            }
        } else {
            m.push(r.clone());
        }
    }
    while m.row_count() > rows.len() {
        m.remove(m.row_count() - 1);
    }
}

impl Part for Desktop {
    fn name(&self) -> &'static str {
        NAME
    }

    fn start(&mut self, st: &mut State) {
        st.shell.want("icons");
        self.start_timers(st);
        self.minute = -1;
    }

    fn output_added(&mut self, st: &mut State, _output: &WlOutput, _name: &str) {
        let had = self.win.is_some();
        self.ensure(st);
        if !had && self.win.is_some() {
            self.refresh_folder(st);
            self.refresh_zones(st);
            self.slow(st, true);
        }
    }

    fn output_removed(&mut self, st: &mut State, name: &str) {
        if self.win.as_ref().is_some_and(|w| w.output == name) {
            self.unmake(st);
        }
        self.ensure(st);
    }

    fn config_changed(&mut self, st: &mut State, look: bool) {
        if look {
            self.apply_theme(st);
        }
        let before = (self.stocks_for.clone(), Self::cfg(st).cities);
        self.read_layout(st);
        self.photos.retain(|_, p| !p.list.is_empty());
        let c = Self::cfg(st);
        if c.stocks != before.0 && self.kinds_shown().contains("stocks") {
            self.refresh_stocks(st);
        }
        self.refresh_zones(st);
        self.after_layout(st);
    }

    fn plugin_changed(&mut self, st: &mut State) {
        self.apply_theme(st);
        self.update(st);
    }

    fn data(&mut self, st: &mut State, what: TypeId) {
        if what == TypeId::of::<Icons>() {
            self.images.retain(|k, _| !k.starts_with("icon:") && !k.starts_with("app:"));
            self.update(st);
        }
    }

    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        // From the shell (stdin): the feed, and widgets the gallery added
        let msg = match msg.downcast::<Value>() {
            Ok(v) => {
                if let Some(f) = Feed::from(&v["feed"]) {
                    let editing_was = self.feed.editing;
                    self.feed = f;
                    if !self.feed.editing && editing_was {
                        self.ghost = None;
                    }
                    self.rotate(st);
                    self.update(st);
                }
                if let Some(a) = v["add"].as_array() {
                    let (k, s) = (a.first().and_then(|x| x.as_str()).unwrap_or(""), a.get(1).and_then(|x| x.as_str()).unwrap_or("s"));
                    self.add(st, k, s);
                }
                return;
            }
            Err(m) => m,
        };
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Act(i, a, b) => self.act(st, i.max(0) as usize, &a, &b),
            M::Drag(i, x, y, on) => self.drag(st, i.max(0) as usize, x, y, on),
            M::Menu(i, x, y) => {
                let rows = if i >= 0 { self.widget_menu(st, i as usize) } else { Self::desktop_menu(st) };
                self.menu_for = (i >= 0).then_some(i as usize);
                self.open_menu(st, rows, x, y);
            }
            M::Bare => {
                self.selected.clear();
                self.open_stack.clear();
                let editing = self.feed.editing;
                if editing {
                    self.feed.editing = false;
                    ask("endEdit", vec![]);
                }
                self.update(st);
            }
            M::Icon(i, a, x, y) => self.icon(st, i.max(0) as usize, &a, x, y),
            M::StackFile(i) => {
                if let Some(e) = self.items.iter().find(|it| it.key == self.open_stack).and_then(|it| it.stack.get(i as usize)).cloned() {
                    self.open_stack.clear();
                    ask("open", vec![json!(e.path)]);
                    self.update(st);
                }
            }
            M::StackFileMenu(i, x, y) => {
                if let Some(e) = self.items.iter().find(|it| it.key == self.open_stack).and_then(|it| it.stack.get(i as usize)).cloned() {
                    let it = IconItem { key: e.path.clone(), name: e.name.clone(), path: e.path.clone(), dir: e.dir, stack: Vec::new() };
                    let rows = self.file_menu(st, &it);
                    self.open_menu(st, rows, x, y);
                }
            }
            M::Tick => {
                let now = crate::util::now_ms();
                let minute = now / 60_000;
                if minute != self.minute {
                    let first = self.minute < 0;
                    self.minute = minute;
                    if !first {
                        self.refresh_zones(st);
                    }
                    self.update(st);
                }
                // the Desktop folder, every few seconds (cheap: one listing)
                if (now / 1000) % 3 == 0 && Self::cfg(st).icons && self.win.is_some() {
                    self.refresh_folder(st);
                }
            }
            M::Sample => {
                if self.kinds_shown().contains("system") {
                    self.sampler.sample(crate::util::now_ms() as f64 / 1000.0);
                    self.stats = self.sampler.stats();
                    self.update(st);
                }
            }
            M::Slow => self.slow(st, false),
            M::SaveNote => {
                if let Some((id, text)) = self.note_pending.take() {
                    ask("note", vec![json!(id), json!(text)]);
                }
            }
            M::Image(k, px) => {
                self.pending.remove(&k);
                if let Some(px) = px {
                    self.images.insert(k, slint::Image::from_rgba8(px));
                    self.update(st);
                }
            }
            M::Zones(z) => {
                self.zones = z.into_iter().collect();
                self.update(st);
            }
            M::Quotes(q, failed) => {
                for x in q {
                    self.quotes.insert(x.s.clone(), x);
                }
                self.stocks_failed = failed && self.quotes.is_empty();
                self.update(st);
            }
            M::Battery(b) => {
                self.battery = b;
                self.update(st);
            }
            M::Folder(e) => {
                if e != self.entries {
                    self.entries = e;
                    self.after_layout(st);
                }
            }
            M::MenuChosen(i) => self.menu_chosen(st, i),
            M::MenuDismissed => {
                self.close_menu(st);
                self.update(st);
            }
            M::MenuClosed => {
                // Click-through, not hidden: Hyprland answers a null buffer
                // with a configure, and the surface drawn again after that
                // would sit over the whole screen taking every click. So it
                // stays (clear, nothing drawn) with no input region and no
                // keyboard until the next right-click.
                if let Some(mw) = self.menu.as_mut().filter(|m| !m.open) {
                    st.set_input_region(&mw.surface, Some(&[]));
                    mw.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::None);
                    mw.surface.layer.commit();
                    if let Some(w) = &self.win {
                        st.shell.glass.set(&w.output, MENU_NS, &[]);
                    }
                    mw.glass_sent.clear();
                }
            }
        }
    }

    fn keyboard_left(&mut self, st: &mut State, s: &Rc<Surface>) {
        if self.menu.as_ref().is_some_and(|m| m.open && Rc::ptr_eq(&m.surface, s)) {
            self.close_menu(st);
            self.update(st);
        }
    }

    fn surface_closed(&mut self, st: &mut State, _s: &Rc<Surface>) {
        self.unmake(st);
        self.ensure(st);
    }

    /// The grid once the window has its size; glass under each widget (and
    /// the open stack, the menu) for the plugin; accessibility
    fn after_frame(&mut self, st: &mut State) {
        if let Some(w) = &self.win {
            let g = self.grid_now();
            if w.surface.mapped.get() && self.grid != Some(g) {
                self.read_layout(st);
                self.after_layout(st);
            }
        }
        let kbd = st.keyboard_surface();
        if let Some(w) = self.win.as_mut() {
            let f = kbd.as_ref().is_some_and(|k| Rc::ptr_eq(k, &w.surface));
            if let Some(a) = w.a11y.as_mut() {
                a.update(false, f);
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
        let Some(w) = self.win.as_mut() else { return };
        if !w.surface.mapped.get() {
            return;
        }
        let mut shapes: Vec<[f32; 19]> = Vec::new();
        for (i, p) in self.placed.iter().enumerate() {
            let (mut x, mut y) = Grid::home(p.col, p.row);
            if let Some((j, lx, ly)) = self.live {
                if j == i {
                    (x, y) = (lx, ly);
                }
            }
            let (pw, ph) = grid::pixel_size(&p.size);
            shapes.push(theme::glass_shape(&look, x, y, pw, ph, 36.0, 1.0, 1.0));
        }
        if w.ui.get_stack_open() {
            let rows = (w.ui.get_stack_files().row_count().min(16).div_ceil(4)) as f32;
            shapes.push(theme::glass_shape(&look, w.ui.get_stack_x(), w.ui.get_stack_y(), 4.0 * 96.0 + 24.0, rows * 96.0 + 48.0, 18.0, 1.0, 1.0));
        }
        let key = format!("{shapes:?}");
        if key != w.glass_sent {
            st.shell.glass.set(&w.output, NS, &shapes);
            w.glass_sent = key;
        }
        let output = w.output.clone();
        if let Some(mw) = &mut self.menu {
            if !mw.surface.mapped.get() {
                return;
            }
            let ui = &mw.ui;
            let p = ui.get_progress();
            let s = ui.get_panel_scale();
            let shape = theme::glass_shape(&look, ui.get_panel_x(), ui.get_panel_y(), ui.get_panel_width() * s, ui.get_panel_height() * s, ui.get_radius() * s, p.min(1.0), 1.0);
            let key = format!("{shape:?}");
            if key != mw.glass_sent {
                st.shell.glass.set(&output, MENU_NS, if p > 0.002 { std::slice::from_ref(&shape) } else { &[] });
                mw.glass_sent = key;
            }
        }
    }

    fn a11y(&mut self) -> Vec<&mut crate::a11y::A11y> {
        let mut v: Vec<&mut crate::a11y::A11y> = self.win.iter_mut().filter_map(|w| w.a11y.as_mut()).collect();
        if let Some(m) = self.menu.as_mut().and_then(|m| m.a11y.as_mut()) {
            v.push(m);
        }
        v
    }

    fn stats(&self) {
        if let Some(w) = &self.win {
            eprintln!("tessera-ui: stats {}", w.surface.stats.borrow_mut().report(&format!("desktop:{}", w.output)));
        }
    }

    fn quit(&mut self, _: &mut State) {
        if let Some(w) = &self.win {
            crate::services::hypr::Glass::clear_now(&w.output, NS);
            crate::services::hypr::Glass::clear_now(&w.output, MENU_NS);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_default_as_config_qml() {
        let c = Cfg::read(&json!({}));
        assert_eq!(c.widgets, default_widgets());
        assert!(c.icons && !c.use_stacks && c.smart_rotate && !c.h24);
        assert_eq!(c.graph, "area");
        // the same lists Config.qml has
        let qml = std::fs::read_to_string(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../shell/common/Config.qml")).unwrap();
        let line = |name: &str| qml.lines().find(|l| l.contains(&format!("property list<string> {name}:"))).unwrap().split_once(':').unwrap().1.split("//").next().unwrap().trim().to_string();
        assert_eq!(serde_json::from_str::<Vec<String>>(&line("widgets")).unwrap(), c.widgets);
        assert_eq!(serde_json::from_str::<Vec<String>>(&line("worldClocks")).unwrap(), c.cities);
        assert_eq!(serde_json::from_str::<Vec<String>>(&line("stocks")).unwrap(), c.stocks);
        let c = Cfg::read(&json!({"desktop": {"widgets": [], "useStacks": true, "photosFolder": "/p"}, "appearance": {"clock24": true}}));
        assert!(c.widgets.is_empty() && c.use_stacks && c.h24);
        assert_eq!(c.photos, "/p");
    }

    #[test]
    fn hands_only_move_forward() {
        let a = clock(11, 59, String::new(), String::new(), 0);
        let b = clock(0, 0, String::new(), String::new(), 86_400_000);
        assert!(b.angle_m > a.angle_m && b.angle_h > a.angle_h);
        assert!(clock(20, 0, String::new(), String::new(), 0).night && !clock(9, 0, String::new(), String::new(), 0).night);
        assert!(view_shape("cookie12").ends_with('Z'));
        assert_eq!(percent_decode("/a%20b.png"), "/a b.png");
        assert!(pick("x", 5) < 5);
    }

    #[test]
    fn colours() {
        assert_eq!(hexc("#FF0000"), Some(Color::from_rgb_u8(255, 0, 0)));
        assert_eq!(hexc("nope"), None);
        let t = tint(Color::from_rgb_u8(0, 0, 0), Color::from_rgb_u8(255, 255, 255), 0.5);
        assert_eq!(t.red(), 128);
    }
}

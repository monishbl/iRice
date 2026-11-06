//! The Overview (rust.surfaces: overview; tap Super), both looks, both
//! styles: shell/modules/overview/Overview.qml and WindowPreview.qml.
//!   grid   the spaces as miniatures of the screen, each window a picture at
//!          its place and size (floating above tiled, the most recently
//!          focused on top; fullscreen fills the miniature); a row per display
//!          with several (overview.displays: rows), or on every screen with
//!          its own spaces (each), the other displays then drop zones on the
//!          edges that face them
//!   macos  a strip of spaces per display on a bar at the top, a + for a new
//!          space, and only the search in the middle
//! Click a space to go there, a window to focus it; drag a window onto
//! another space (or a display's zone, or push it against that display's
//! edge) to move it; drag a space (by its empty area or its title) to put it
//! elsewhere in its row, or onto another display; right-click a window or a
//! space for its menu. Typing searches apps and actions as Search does
//! (spotlight::rank), Enter opens the first result or goes to the picked
//! space; the arrows pick a space.
//!
//! It opens as the QML one did, through Panels ("overview": Super's tap, the
//! hot corner, the four-finger swipe's `tessera:overview`, `qs ipc call`):
//! RustUi.qml forwards the open and close, and it says when it closed itself.
//! The spaces' blocks and names come from the QML Spaces service
//! (modules/overview/RustWindows.qml); its moves go back there (`wm`).
//!
//! Window pictures: services::capture (Hyprland's toplevel export, what the
//! QML's ScreencopyView uses), once per open (snapshot), or about 30 times a
//! second for every window (live) or the current spaces' (hybrid).
//!
//! Not like the QML yet:
//! - a window's miniature doesn't glide to a new place when the windows
//!   re-tile while it's open (it's placed again at once);
//! - search: apps, actions, emoji and sums from spotlight::rank; the rows
//!   only the QML shell knows (files, conversions, the calendar, contacts,
//!   the App Store…) aren't asked for here (Search in Rust has them);
//! - Material's press swell (ElasticControl) is the hover and press scale;
//! - no elastic pull on the context menu (ElasticGrab).

pub mod ui {
    slint::include_modules!();
}
tessera_core::theme_global!(ui);

// tessera-core's modules (shell, wayland, theme, services, util…) under the
// names they had when everything was one crate: `crate::shell::Part`
#[allow(unused_imports)]
use tessera_core::*;

pub mod layout;

use tessera_core::services::apps::Apps;
use tessera_core::services::capture::Shot;
use tessera_core::services::clients::{Client, Windows};
use tessera_core::services::hypr::{Glass, Snapshot};
use tessera_core::services::icons::Icons;
use tessera_core::shell::{Command, Part};
use tessera_core::rank::{self, Known, Row, Run};
use tessera_core::wm::{self, spaces, AppIcons, IconsDone, LiveTick, OverviewCfg, Thumbs, Wall, WallPicture};
use tessera_core::theme;
use tessera_core::wayland::{Spec, State, Surface};
use layout::{CellBox, Rect, ZoneBox};
use slint::platform::Key;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use smithay_client_toolkit::shell::WaylandSurface;
use spaces::{Geo, Zone};
use std::any::Any;
use std::cell::Cell;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;
use std::time::Duration;

const NAME: &str = "overview";
pub const NS: &str = "tessera:overview";
/// how far the pointer goes before a press is a drag
const DRAG: f32 = 10.0;

enum M {
    Key(String, bool, bool),
    Edited(String),
    Pointer(String, f32, f32, bool),
    Press(String, f32, f32, i32),
    Release(String, f32, f32, i32),
    RunResult(i32),
    MenuActivate(String, i32),
    MenuDismissed(String),
    Closed(String),
    Expand(String, String),
    Collapse(String, String),
    Ungone,
}

#[derive(Clone, Copy, Default)]
struct KeyCtx {
    has_text: bool,
    menu: bool,
}

/// What the keys the field gets do (the rest are typed)
fn handles(c: KeyCtx, text: &str) -> bool {
    let is = |k: Key| text == SharedString::from(k).as_str();
    if c.menu {
        return is(Key::Escape) || is(Key::Return) || is(Key::UpArrow) || is(Key::DownArrow) || text == " ";
    }
    is(Key::Escape) || is(Key::Return) || is(Key::UpArrow) || is(Key::DownArrow) || (!c.has_text && (is(Key::LeftArrow) || is(Key::RightArrow)))
}

/// A window inside a space, laid out
#[derive(Clone, Debug)]
struct Mini {
    address: String,
    class: String,
    title: String,
    /// relative to its space
    rect: Rect,
    full: bool,
}

#[derive(Clone, Debug, Default)]
enum Press {
    #[default]
    None,
    Window { cell: i32, address: String, from: (f32, f32), moved: bool, ofs: (f32, f32) },
    Space { id: i32, row: usize, from: (f32, f32), moved: bool },
    Tile { id: i32, row: usize, from: (f32, f32), window: Option<String>, moved: bool },
    Plus(usize),
    Zone(String),
    Background,
}

#[derive(Clone, Debug)]
enum Act {
    WindowToDisplay(String, String),
    WindowToSpace(String, i32),
    SpaceToDisplay(i32, String),
}

struct MenuItem {
    text: String,
    header: bool,
    separator: bool,
    enabled: bool,
    act: Option<Act>,
}

struct Menu {
    items: Vec<MenuItem>,
    x: f32,
    y: f32,
    up: bool,
    current: i32,
}

impl Menu {
    fn width(&self) -> f32 {
        200f32.max(self.items.iter().map(|i| i.text.chars().count()).max().unwrap_or(0) as f32 * 7.5 + 40.0)
    }
    fn rows_h(&self) -> f32 {
        self.items.iter().map(|i| if i.separator { 11.0 } else if i.header { 26.0 } else { 28.0 }).sum()
    }
    /// Where it sits (ContextMenu.qml's body), for its glass
    fn rect(&self, w: f32, h: f32) -> Rect {
        let (mw, natural) = (self.width(), self.rows_h() + 10.0);
        let (px, py) = (self.x + mw / 2.0, self.y);
        let below = h - py;
        let up = if self.up { natural <= py || py >= below } else { natural > below && py > below };
        Rect { x: 0f32.max((w - mw).min(px - mw / 2.0)), y: 8f32.max((h - 8.0 - natural).min(if up { py - natural } else { py })), w: mw, h: natural }
    }
    fn step(&mut self, d: i32) {
        let mut i = self.current + d;
        while i >= 0 && (i as usize) < self.items.len() {
            let it = &self.items[i as usize];
            if !it.separator && !it.header && it.enabled {
                self.current = i;
                return;
            }
            i += d;
        }
    }
}

struct Win {
    output: String,
    surface: Rc<Surface>,
    ui: ui::OverviewPanel,
    a11y: Option<crate::a11y::A11y>,
    glass_sent: String,
    w: f32,
    h: f32,
    rows: Vec<(String, Vec<i32>)>,
    rows_mode: bool,
    cells: Vec<CellBox>,
    minis: HashMap<i32, Vec<Mini>>,
    plus: Vec<Rect>,
    zone_defs: Vec<Zone>,
    zones: Vec<ZoneBox>,
    bar_h: f32,
    picked: i32,
    hover_cell: i32,
    pressed_cell: i32,
    hover_win: String,
    press: Press,
    drag_space: i32,
    drag_d: (f32, f32),
    drop_on: i32,
    space_zone: String,
    edge_zone: String,
    drag_win: String,
    drop_cell: i32,
    zone_hot: String,
    chip_hot: i32,
    expanded: Option<String>,
    pointer: (f32, f32),
    menu: Option<Menu>,
    timers: [slint::Timer; 2],
}

#[derive(Default)]
pub struct Overview {
    wins: Vec<Win>,
    open: bool,
    key_out: String,
    text: String,
    results: Vec<Row>,
    selected: usize,
    apps: Vec<rank::App>,
    actions: Vec<rank::Action>,
    home: String,
    key_ctx: Rc<Cell<KeyCtx>>,
    icons: AppIcons,
    thumbs: Thumbs,
    wall: Wall,
    info: spaces::Info,
    capturing: bool,
    gone_win: String,
    space_gone: i32,
    gone_timer: slint::Timer,
}

fn send(st: &State) -> impl Fn(Box<dyn Any + Send>) + Clone + 'static {
    st.shell.sender(NAME)
}

/// A display's place and logical size (Overview.geoOf)
fn geo_of(win: &Windows, name: &str, fallback: (f32, f32)) -> Geo {
    match win.monitor(name) {
        Some(m) => {
            let (w, h) = m.logical();
            Geo { x: m.x as f32, y: m.y as f32, w, h }
        }
        None => Geo { x: 0.0, y: 0.0, w: fallback.0, h: fallback.1 },
    }
}

impl Overview {
    fn cfg(st: &State) -> OverviewCfg {
        OverviewCfg::read(&st.shell.cfg)
    }

    fn data(st: &State) -> (Windows, Snapshot) {
        (st.shell.get::<Windows>().cloned().unwrap_or_default(), st.shell.get::<Snapshot>().cloned().unwrap_or_default())
    }

    /// Displays with spaces, in arrangement order
    fn displays(&self, w: &Windows) -> Vec<String> {
        let mut m: Vec<_> = w.monitors.iter().filter(|m| self.info.bases.contains_key(&m.name)).collect();
        m.sort_by_key(|m| (m.x, m.y));
        m.into_iter().map(|m| m.name.clone()).collect()
    }

    fn each(&self, st: &State, w: &Windows) -> bool {
        self.displays(w).len() > 1 && Self::cfg(st).displays == "each"
    }

    /// Where each space lives (Spaces.workspaces)
    fn host_of(h: &Snapshot) -> BTreeMap<i32, String> {
        let mut m = BTreeMap::new();
        for (n, id) in &h.active {
            if *id > 0 {
                m.entry(*id).or_insert_with(|| n.clone());
            }
        }
        for ws in &h.workspaces {
            if ws.id > 0 {
                m.entry(ws.id).or_insert_with(|| ws.monitor.clone());
            }
        }
        m
    }

    fn window(&mut self, st: &mut State, output: &str) -> Option<usize> {
        if let Some(i) = self.wins.iter().position(|w| w.output == output) {
            return Some(i);
        }
        let wl_output = st.output_named(output)?;
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
        let ui = ui::OverviewPanel::new().expect("OverviewPanel");
        st.shell.apply_theme(&ui.global::<ui::Theme>());
        let to = send(st);
        {
            let (to, ctx) = (to.clone(), self.key_ctx.clone());
            ui.on_key(move |text, shift, ctrl| {
                let ok = handles(ctx.get(), &text);
                if ok {
                    to(Box::new(M::Key(text.to_string(), shift, ctrl)));
                }
                ok
            });
        }
        let o = output.to_string();
        macro_rules! on {
            ($set:ident, |$($a:ident),*| $m:expr) => {{
                let to = to.clone();
                ui.$set(move |$($a),*| to(Box::new($m)));
            }};
        }
        on!(on_edited, |t| M::Edited(t.to_string()));
        {
            let o = o.clone();
            on!(on_pointer, |x, y, inside| M::Pointer(o.clone(), x, y, inside));
        }
        {
            let o = o.clone();
            on!(on_press, |x, y, b| M::Press(o.clone(), x, y, b));
        }
        {
            let o = o.clone();
            on!(on_release, |x, y, b| M::Release(o.clone(), x, y, b));
        }
        on!(on_run_result, |i| M::RunResult(i));
        {
            let o = o.clone();
            on!(on_menu_activate, |i| M::MenuActivate(o.clone(), i));
        }
        {
            let o = o.clone();
            on!(on_menu_dismissed, | | M::MenuDismissed(o.clone()));
        }
        {
            let o = o.clone();
            on!(on_closed, | | M::Closed(o.clone()));
        }
        ui.show().expect("show overview");
        let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
        self.wins.push(Win {
            output: output.into(),
            surface,
            ui,
            a11y,
            glass_sent: String::new(),
            w: 1920.0,
            h: 1080.0,
            rows: Vec::new(),
            rows_mode: false,
            cells: Vec::new(),
            minis: HashMap::new(),
            plus: Vec::new(),
            zone_defs: Vec::new(),
            zones: Vec::new(),
            bar_h: 0.0,
            picked: 1,
            hover_cell: 0,
            pressed_cell: 0,
            hover_win: String::new(),
            press: Press::None,
            drag_space: 0,
            drag_d: (0.0, 0.0),
            drop_on: 0,
            space_zone: String::new(),
            edge_zone: String::new(),
            drag_win: String::new(),
            drop_cell: 0,
            zone_hot: String::new(),
            chip_hot: 0,
            expanded: None,
            pointer: (-1.0, -1.0),
            menu: None,
            timers: Default::default(),
        });
        let i = self.wins.len() - 1;
        self.look(st, i);
        Some(i)
    }

    fn look(&mut self, st: &State, i: usize) {
        let m = st.shell.look.material;
        let w = &self.wins[i];
        w.ui.set_g_search(wglyph("magnifying-glass", m, true));
        w.ui.set_g_plus(wglyph("plus", m, true));
        w.ui.set_g_monitor(wglyph("monitor", m, false));
        w.ui.set_glass_fill(crate::wm::glass_fill(st.shell.plugin, st.shell.look.dark));
        w.ui.set_scrim(w.ui.global::<ui::Theme>().get_m3_scrim());
        w.ui.set_emoji_font(tessera_core::settings::emoji_font(&st.shell.cfg, &[]).into());
        let t = w.ui.global::<ui::Theme>();
        self.icons.restyle(st, t.get_accent(), t.get_m3_primary_container(), t.get_m3_on_primary_container());
    }

    fn open(&mut self, st: &mut State, screen: Option<String>) {
        let (wins, snap) = Self::data(st);
        let screen = screen.or_else(|| wins.focused_monitor().map(|m| m.name.clone())).or_else(|| st.outputs_now().first().map(|o| o.1.clone()));
        let Some(screen) = screen else { return };
        let outs: Vec<String> = if self.each(st, &wins) { self.displays(&wins) } else { vec![screen.clone()] };
        if self.capturing {
            self.thumbs.retake();
        }
        self.capturing = true;
        self.open = true;
        self.key_out = screen.clone();
        self.text.clear();
        self.results.clear();
        self.selected = 0;
        let cfg = Self::cfg(st);
        if cfg.wallpapers {
            self.wall.want(st, NAME, (400, 250));
        }
        // (other screens' windows from an earlier "each" close)
        for w in &self.wins {
            if !outs.contains(&w.output) {
                w.ui.set_shown(false);
            }
        }
        for o in &outs {
            let Some(i) = self.window(st, o) else { continue };
            let w = &mut self.wins[i];
            w.picked = snap.active.get(o).copied().unwrap_or(1);
            w.menu = None;
            w.press = Press::None;
            Self::reset_drag(w);
            w.ui.set_results(ModelRc::default());
            w.ui.invoke_open();
            w.ui.set_shown(true);
            let key = *o == screen;
            w.surface.layer.set_keyboard_interactivity(if key { KeyboardInteractivity::Exclusive } else { KeyboardInteractivity::None });
            st.set_input_region(&w.surface, None);
            w.surface.show(0, 0);
            if key {
                w.ui.invoke_focus_field();
            }
        }
        self.key_ctx.set(KeyCtx::default());
        self.rebuild(st);
    }

    /// Close (`tell`: it closed itself, so Panels follows)
    fn close(&mut self, st: &mut State, tell: bool) {
        if !self.open {
            return;
        }
        self.open = false;
        for w in &mut self.wins {
            w.menu = None;
            w.ui.set_menu_shown(false);
            w.ui.set_shown(false);
            w.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::None);
            st.set_input_region(&w.surface, Some(&[]));
            w.surface.layer.commit();
        }
        if tell {
            crate::shell::closed(NAME);
        }
    }

    fn closed(&mut self, st: &mut State, out: &str) {
        if self.open {
            return;
        }
        if let Some(w) = self.wins.iter_mut().find(|w| w.output == out) {
            w.surface.hide();
            st.shell.glass.set(&w.output, NS, &[]);
            w.glass_sent.clear();
        }
        // pictures are dropped once it's gone
        if self.wins.iter().all(|w| !w.surface.mapped.get()) {
            self.capturing = false;
            self.thumbs.clear();
        }
    }

    fn reset_drag(w: &mut Win) {
        w.drag_space = 0;
        w.drag_d = (0.0, 0.0);
        w.drop_on = 0;
        w.space_zone.clear();
        w.edge_zone.clear();
        w.drag_win.clear();
        w.drop_cell = 0;
        w.zone_hot.clear();
        w.chip_hot = 0;
        w.expanded = None;
        w.pressed_cell = 0;
        w.timers = Default::default();
    }

    /// Everything into every window: layout, pictures, icons
    fn rebuild(&mut self, st: &mut State) {
        if !self.open && !self.wins.iter().any(|w| w.surface.mapped.get()) {
            return;
        }
        let (wins, snap) = Self::data(st);
        let cfg = Self::cfg(st);
        let m = st.shell.look.material;
        let mac = cfg.style == "macos";
        let displays = self.displays(&wins);
        let multi = displays.len() > 1;
        let each = multi && cfg.displays == "each";
        let rows_mode = multi && !each;
        let host = Self::host_of(&snap);
        let all_geo: Vec<Geo> = wins.monitors.iter().map(|mo| geo_of(&wins, &mo.name, (1920.0, 1080.0))).collect();
        let mut live: HashMap<String, (u32, u32)> = HashMap::new();
        let visible: Vec<i32> = snap.active.values().copied().collect();
        for i in 0..self.wins.len() {
            let (ww, wh) = {
                let s = self.wins[i].surface.logical_size();
                if s.0 > 0 { (s.0 as f32, s.1 as f32) } else { wins.monitor(&self.wins[i].output).map(|m| m.logical()).unwrap_or((1920.0, 1080.0)) }
            };
            let scale = self.wins[i].ui.window().scale_factor().max(1.0);
            let out = self.wins[i].output.clone();
            let row_names: Vec<String> = if rows_mode { displays.clone() } else { vec![out.clone()] };
            let rows: Vec<(String, Vec<i32>, f32)> = row_names
                .iter()
                .map(|n| {
                    let g = geo_of(&wins, n, (ww, wh));
                    (n.clone(), self.info.ids_for(n, &snap), g.h / g.w.max(1.0))
                })
                .collect();
            let cols = layout::columns(rows_mode, rows.iter().map(|r| r.1.len()).max().unwrap_or(1));
            let here = geo_of(&wins, &out, (ww, wh));
            let others: Vec<(String, Geo)> = displays.iter().filter(|n| **n != out).map(|n| (n.clone(), geo_of(&wins, n, (ww, wh)))).collect();
            let zone_defs = if each { spaces::edge_zones(here, &others) } else { Vec::new() };
            let side = if zone_defs.iter().any(|z| z.side.vertical()) { 160.0 } else { 0.0 };
            let cell_w = layout::cell_w(ww, wh, cols, rows_mode, side, &rows.iter().map(|r| (r.1.len(), r.2)).collect::<Vec<_>>());
            let mut minis: HashMap<i32, Vec<Mini>> = HashMap::new();
            let mut cells_ui = Vec::new();
            let mut tiles_ui = Vec::new();
            let mut labels = Vec::new();
            let mut cells: Vec<CellBox> = Vec::new();
            let mut plus = Vec::new();
            let mut bar_h = 0.0;
            let place = |id: i32, row: &str, scale_to: f32| -> (Vec<Mini>, Geo) {
                let g = geo_of(&wins, host.get(&id).map(String::as_str).unwrap_or(row), (ww, wh));
                let k = scale_to / g.w.max(1.0);
                let mut list: Vec<&Client> = wins.clients.iter().filter(|c| c.workspace.id == id && c.size.0 > 0 && c.size.1 > 0 && !c.hidden).collect();
                list.sort_by_key(|c| layout::stack(c));
                let out = list
                    .into_iter()
                    .map(|c| {
                        let r = layout::place_in((c.at.0 as f32, c.at.1 as f32), (c.size.0 as f32, c.size.1 as f32), g, &all_geo);
                        let full = layout::is_full(c);
                        Mini { address: c.address.clone(), class: c.class.clone(), title: c.title.clone(), rect: Rect { x: r.x * k, y: r.y * k, w: r.w * k, h: r.h * k }, full }
                    })
                    .collect();
                (out, g)
            };
            if mac {
                let ml = layout::mac(ww, wh, &rows, rows_mode, m);
                bar_h = ml.bar_h;
                for r in &ml.rows {
                    if rows_mode {
                        labels.push(ui::OvLabel { x: r.label.0, y: r.label.1, text: self.info.label_for_name(&r.name).into() });
                    }
                    plus.push(r.plus);
                    for t in &r.tiles {
                        let (list, _) = place(t.id, &r.name, t.rect.w);
                        let ms: Vec<ui::OvMini> = list
                            .iter()
                            .map(|mi| {
                                let (w4, h4) = (mi.rect.w.max(4.0), mi.rect.h.max(4.0));
                                let icon = self.icons.get(st, &mi.class, 18f32.min(h4 * 0.6), scale, false);
                                ui::OvMini { x: mi.rect.x, y: mi.rect.y, w: w4, h: h4, has_icon: icon.is_some(), icon: icon.unwrap_or_default(), address: mi.address.as_str().into() }
                            })
                            .collect();
                        minis.insert(t.id, list);
                        let current = snap.active.get(host.get(&t.id).map(String::as_str).unwrap_or("")) == Some(&t.id);
                        tiles_ui.push(ui::OvTile {
                            id: t.id,
                            x: t.rect.x,
                            y: t.rect.y,
                            w: t.rect.w,
                            h: t.rect.h,
                            title: self.info.title(t.id, &r.name, "Desktop").into(),
                            title_y: t.title.y,
                            current,
                            minis: ModelRc::new(VecModel::from(ms)),
                        });
                        cells.push(t.clone());
                    }
                }
            } else {
                let grid = layout::grid(ww, wh, &rows, cols, cell_w, rows_mode, m);
                for r in &grid {
                    if rows_mode {
                        labels.push(ui::OvLabel { x: r.label.0, y: r.label.1, text: self.info.label_for_name(&r.name).into() });
                    }
                    for c in &r.cells {
                        let (mut list, _) = place(c.id, &r.name, cell_w);
                        let inset = if m { 0.0 } else { 1.0 };
                        let live_here = cfg.previews == "live" || (cfg.previews == "hybrid" && visible.contains(&c.id));
                        let ws: Vec<ui::OvWin> = list
                            .iter_mut()
                            .map(|mi| {
                                if mi.full {
                                    mi.rect = Rect { x: inset, y: inset, w: c.rect.w - 2.0 * inset, h: c.rect.h - 2.0 * inset };
                                } else {
                                    mi.rect.w = mi.rect.w.max(16.0);
                                    mi.rect.h = mi.rect.h.max(12.0);
                                }
                                let r = mi.rect;
                                let roomy = r.h >= 56.0 && r.w >= 72.0;
                                let mid = (12f32.max(32f32.min(r.h * if roomy { 0.4 } else { 0.7 }))).round();
                                let badge_s = (12f32.max(20f32.min(r.h * 0.22))).round();
                                let icon = self.icons.get(st, &mi.class, mid, scale, true);
                                let badge = self.icons.get(st, &mi.class, badge_s, scale, true);
                                let max = (((r.w * scale).ceil()) as u32, ((r.h * scale).ceil()) as u32);
                                if self.capturing {
                                    self.thumbs.ask(st, NAME, &mi.address, max);
                                    if live_here {
                                        live.insert(mi.address.clone(), max);
                                    }
                                }
                                let pic = self.thumbs.images.get(&mi.address).cloned();
                                let name = st.shell.get::<Apps>().and_then(|a| a.entry_for(&mi.class)).map(|e| e.name.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| mi.class.clone());
                                ui::OvWin {
                                    address: mi.address.as_str().into(),
                                    x: r.x,
                                    y: r.y,
                                    w: r.w,
                                    h: r.h,
                                    radius: if m { if mi.full { 28.0 } else { 12.0 } } else if mi.full { 13.0 } else { 4.0 },
                                    has_picture: pic.is_some(),
                                    picture: pic.unwrap_or_default(),
                                    has_icon: icon.is_some(),
                                    icon: icon.unwrap_or_default(),
                                    badge: badge.unwrap_or_default(),
                                    name: name.into(),
                                    failed: self.thumbs.failed.contains(&mi.address),
                                }
                            })
                            .collect();
                        let n = list.len();
                        minis.insert(c.id, list);
                        let current = snap.active.get(host.get(&c.id).map(String::as_str).unwrap_or("")) == Some(&c.id);
                        let title = self.info.title(c.id, &r.name, "Space") + &if n > 0 { format!(" · {n} {}", if n == 1 { "window" } else { "windows" }) } else { String::new() };
                        cells_ui.push(ui::OvCell {
                            id: c.id,
                            x: c.rect.x,
                            y: c.rect.y,
                            w: c.rect.w,
                            h: c.rect.h,
                            title: title.into(),
                            title_y: c.title.y,
                            current,
                            guest: self.info.guest(c.id),
                            wins: ModelRc::new(VecModel::from(ws)),
                        });
                        cells.push(c.clone());
                    }
                }
            }
            let w = &mut self.wins[i];
            w.w = ww;
            w.h = wh;
            w.rows = rows.iter().map(|r| (r.0.clone(), r.1.clone())).collect();
            w.rows_mode = rows_mode;
            w.cells = cells;
            w.minis = minis;
            w.plus = plus;
            w.bar_h = bar_h;
            w.zone_defs = zone_defs;
            let ui = &w.ui;
            ui.set_mac(mac);
            ui.set_rows_mode(rows_mode);
            ui.set_wallpapers(cfg.wallpapers);
            ui.set_wallpaper(self.wall.image.clone().unwrap_or_default());
            ui.set_cells(ModelRc::new(VecModel::from(cells_ui)));
            ui.set_tiles(ModelRc::new(VecModel::from(tiles_ui)));
            ui.set_labels(ModelRc::new(VecModel::from(labels)));
            ui.set_pluses(ModelRc::new(VecModel::from(w.plus.iter().map(|p| ui::OvPlus { x: p.x, y: p.y, size: p.w }).collect::<Vec<_>>())));
            ui.set_bar_h(bar_h);
            Self::zones_into(&self.info, &snap, w, mac, m);
            Self::push_state(w, &self.gone_win, self.space_gone, &snap);
        }
        if self.capturing {
            self.thumbs.set_live(st, NAME, live);
        }
        self.icons.flush(send(st));
        self.push_results(st);
    }

    /// The edge zones (each display its own): their pills and chips
    fn zones_into(info: &spaces::Info, snap: &Snapshot, w: &mut Win, mac: bool, material: bool) {
        let top = if mac { w.bar_h + 16.0 } else { 12.0 };
        let ids_of = |n: &str| -> Vec<i32> { info.ids_for(n, snap).into_iter().filter(|id| info.is_own(*id, n)).take(9).collect() };
        let open: Option<(String, Vec<i32>)> = w.expanded.as_ref().map(|n| (n.clone(), ids_of(n)));
        let label = |n: &str| info.label_for_name(n).chars().count() as f32 * 7.5;
        w.zones = layout::zones(w.w, w.h, top, &w.zone_defs, label, open.as_ref().map(|(n, ids)| (n.as_str(), ids.as_slice())));
        let zs: Vec<ui::OvZone> = w
            .zones
            .iter()
            .map(|z| {
                let expanded = !z.chips.is_empty();
                let vertical = z.side.vertical();
                let current = snap.active.get(&z.name).copied().unwrap_or(0);
                ui::OvZone {
                    name: z.name.as_str().into(),
                    x: z.rect.x,
                    y: z.rect.y,
                    w: z.rect.w,
                    h: z.rect.h,
                    head_h: z.head_h,
                    vertical,
                    label: info.label_for_name(&z.name).into(),
                    radius: if material { if expanded || vertical { 28.0 } else { z.rect.h / 2.0 } } else if expanded || vertical { 18.0 } else { 22.0 },
                    chips: ModelRc::new(VecModel::from(z.chips.iter().map(|(id, r)| ui::OvChip { id: *id, x: r.x, y: r.y, n: spaces::number_of(*id).to_string().into(), current: *id == current }).collect::<Vec<_>>())),
                }
            })
            .collect();
        w.ui.set_zones(ModelRc::new(VecModel::from(zs)));
    }

    /// The pointer's state into a window
    fn push_state(w: &Win, gone_win: &str, space_gone: i32, snap: &Snapshot) {
        let ui = &w.ui;
        ui.set_picked(w.picked);
        ui.set_hover_cell(w.hover_cell);
        ui.set_pressed_cell(w.pressed_cell);
        ui.set_hover_win(w.hover_win.as_str().into());
        let (px, py) = w.pointer;
        let hp = w.plus.iter().position(|p| p.has(px, py)).map(|i| i as i32).unwrap_or(-1);
        ui.set_plus_hover(if matches!(w.press, Press::None) { hp } else { -1 });
        ui.set_plus_pressed(match w.press {
            Press::Plus(i) => i as i32,
            _ => -1,
        });
        let zh = if matches!(w.press, Press::None) { w.zones.iter().find(|z| z.rect.has(px, py)).map(|z| z.name.clone()).unwrap_or_default() } else { String::new() };
        ui.set_zone_hover(zh.as_str().into());
        ui.set_zone_hot(if w.zone_hot.is_empty() { w.space_zone.as_str().into() } else { w.zone_hot.as_str().into() });
        ui.set_chip_hot(w.chip_hot);
        ui.set_drag_space(w.drag_space);
        ui.set_drag_dx(w.drag_d.0);
        ui.set_drag_dy(w.drag_d.1);
        ui.set_drop_on(w.drop_on);
        ui.set_drop_cell(w.drop_cell);
        let edge = if w.edge_zone.is_empty() { w.space_zone.clone() } else { w.edge_zone.clone() };
        ui.set_edge_hot(w.edge_zone.as_str().into());
        ui.set_edge_cell(if edge.is_empty() { 0 } else { snap.active.get(&edge).copied().unwrap_or(0) });
        ui.set_any_drag(w.drag_space > 0 || !w.drag_win.is_empty());
        ui.set_drag_win(w.drag_win.as_str().into());
        ui.set_gone_win(gone_win.into());
        ui.set_space_gone(space_gone);
        let cursor = if w.drag_space > 0 || !w.drag_win.is_empty() {
            "grabbing"
        } else if hp >= 0 || !zh.is_empty() || (ui.get_mac() && w.cells.iter().any(|c| c.rect.has(px, py))) {
            "pointer"
        } else if w.cells.iter().any(|c| Self::title_hit(c, px, py)) {
            "grab"
        } else {
            "default"
        };
        ui.set_cursor(cursor.into());
        match &w.menu {
            Some(mn) => {
                let rows: Vec<ui::WMenuRow> = mn.items.iter().map(|i| ui::WMenuRow { text: i.text.as_str().into(), header: i.header, separator: i.separator, enabled: i.enabled }).collect();
                ui.set_menu_rows(ModelRc::new(VecModel::from(rows)));
                ui.set_menu_x(mn.x + mn.width() / 2.0);
                ui.set_menu_y(mn.y);
                ui.set_menu_w(mn.width());
                ui.set_menu_h(mn.rows_h());
                ui.set_menu_up(mn.up);
                ui.set_menu_current(mn.current);
                ui.set_menu_shown(true);
            }
            None => ui.set_menu_shown(false),
        }
    }

    fn title_hit(c: &CellBox, px: f32, py: f32) -> bool {
        let t = c.title;
        let m = 8.0;
        px >= t.x - m && px < t.x + t.w + m && py >= t.y - m && py < t.y + t.h + m
    }

    // ── Search ──────────────────────────────────────────────────────────────

    fn edited(&mut self, st: &mut State, t: String) {
        self.text = t;
        let k = Known {
            apps: &self.apps,
            actions: &self.actions,
            emoji: if self.text.trim().starts_with(':') { crate::rank::emoji() } else { &[] },
            home: &self.home,
            ..Default::default()
        };
        self.results = rank::query(&self.text, &k).0.into_iter().filter(|r| !r.ai && !r.loading).collect();
        self.selected = 0;
        self.key_ctx.set(KeyCtx { has_text: !self.text.is_empty(), menu: false });
        for w in &self.wins {
            w.ui.set_results_y(0.0);
        }
        self.push_results(st);
    }

    fn push_results(&mut self, st: &State) {
        let m = st.shell.look.material;
        let Some(i) = self.wins.iter().position(|w| w.output == self.key_out) else { return };
        let scale = self.wins[i].ui.window().scale_factor();
        let rows: Vec<ui::OvResult> = self
            .results
            .iter()
            .map(|r| {
                let styled = r.section == "Applications" || (r.section == "Top Hit" && matches!(r.run, Run::App(_)));
                let icon = if r.icon.is_empty() { None } else { self.icons.get(st, &r.icon, 26.0, scale, styled) };
                ui::OvResult {
                    title: r.title.as_str().into(),
                    sub: r.subtitle.as_str().into(),
                    has_icon: icon.is_some(),
                    icon: icon.unwrap_or_default(),
                    emoji: r.emoji.as_str().into(),
                    glyph: wglyph(if r.glyph.is_empty() { "circle" } else { &r.glyph }, m, false),
                }
            })
            .collect();
        self.icons.flush(send(st));
        let ui = &self.wins[i].ui;
        ui.set_results(ModelRc::new(VecModel::from(rows)));
        ui.set_selected(self.selected as i32);
    }

    fn select(&mut self, i: usize) {
        self.selected = i;
        let Some(w) = self.wins.iter().find(|w| w.output == self.key_out) else { return };
        w.ui.set_selected(i as i32);
        // keep it in view (the list shows 420 px)
        let top = i as f32 * 40.0;
        let cur = -w.ui.get_results_y();
        if top < cur {
            w.ui.set_results_y(-top);
        } else if top + 40.0 > cur + 420.0 {
            w.ui.set_results_y(-(top + 40.0 - 420.0));
        }
    }

    fn run_result(&mut self, st: &mut State, i: usize) {
        let Some(r) = self.results.get(i).cloned() else { return };
        self.close(st, true);
        use serde_json::json;
        match &r.run {
            Run::Nothing | Run::Ask(_) | Run::Shell(_) => {}
            Run::Copy(t) => crate::util::pipe_to(&["wl-copy"], t.clone().into_bytes()),
            Run::App(id) => {
                if let Some(e) = st.shell.get::<Apps>().and_then(|a| a.by_id(id)) {
                    crate::services::apps::launch(e);
                }
            }
            Run::Url(u) => crate::shell::ask("launch.open", json!([u])),
            Run::Terminal(c) => crate::shell::ask("search.command", json!([c])),
            Run::File(p) => crate::shell::ask("files.open", json!([p])),
            Run::Action(t) => crate::shell::ask("search.action", json!([t])),
            Run::SetupAi => crate::shell::ask("panels.openSettings", json!(["ai"])),
        }
    }

    // ── Keys ────────────────────────────────────────────────────────────────

    fn key(&mut self, st: &mut State, text: &str) {
        let is = |k: Key| text == SharedString::from(k).as_str();
        let Some(i) = self.wins.iter().position(|w| w.output == self.key_out) else { return };
        if self.wins[i].menu.is_some() {
            if is(Key::Escape) {
                self.menu_dismissed(st, i);
            } else if is(Key::UpArrow) || is(Key::DownArrow) {
                let w = &mut self.wins[i];
                w.menu.as_mut().unwrap().step(if is(Key::UpArrow) { -1 } else { 1 });
                let (snap, gone, sg) = (Self::data(st).1, self.gone_win.clone(), self.space_gone);
                Self::push_state(&self.wins[i], &gone, sg, &snap);
            } else {
                let c = self.wins[i].menu.as_ref().map(|m| m.current).unwrap_or(-1);
                self.menu_activate(st, i, c);
            }
            return;
        }
        let mac = Self::cfg(st).style == "macos";
        let ids: Vec<i32> = self.wins[i].cells.iter().map(|c| c.id).collect();
        let cols = layout::columns(self.wins[i].rows_mode, self.wins[i].rows.iter().map(|r| r.1.len()).max().unwrap_or(1)) as i32;
        let has_text = !self.text.is_empty();
        if is(Key::Escape) {
            if has_text {
                self.wins[i].ui.invoke_open();
                self.edited(st, String::new());
            } else {
                self.close(st, true);
            }
        } else if is(Key::Return) {
            if !self.results.is_empty() {
                let s = self.selected;
                self.run_result(st, s);
            } else {
                let id = self.wins[i].picked;
                self.go(st, id);
            }
        } else if is(Key::LeftArrow) || is(Key::RightArrow) {
            let w = &mut self.wins[i];
            w.picked = layout::step(&ids, w.picked, if is(Key::LeftArrow) { -1 } else { 1 });
            w.ui.set_picked(w.picked);
        } else if is(Key::UpArrow) || is(Key::DownArrow) {
            let up = is(Key::UpArrow);
            if has_text {
                let n = if up { self.selected.saturating_sub(1) } else { (self.selected + 1).min(self.results.len().saturating_sub(1)) };
                self.select(n);
            } else if !mac {
                let w = &mut self.wins[i];
                w.picked = layout::step(&ids, w.picked, if up { -cols } else { cols });
                w.ui.set_picked(w.picked);
            }
        }
    }

    // ── Actions ─────────────────────────────────────────────────────────────

    fn go(&mut self, st: &mut State, id: i32) {
        self.close(st, true);
        wm::ask("spaces.go", serde_json::json!([id]));
    }

    /// Overview.focusWindow: a minimized one comes back; else its space and it
    fn focus_window(&mut self, st: &mut State, address: &str) {
        self.close(st, true);
        let (wins, _) = Self::data(st);
        match wins.clients.iter().find(|c| c.address == address) {
            Some(c) if c.aside() => wm::ask("apps.show", serde_json::json!([address])),
            _ => crate::services::clients::focus(address),
        }
    }

    fn move_window(&mut self, address: &str, id: i32) {
        wm::ask("spaces.sendWindow", serde_json::json!([address, id]));
    }

    fn move_window_to_display(&mut self, address: &str, name: &str) {
        wm::ask("spaces.sendWindowToDisplay", serde_json::json!([address, name]));
    }

    fn move_space(&mut self, st: &State, snap: &Snapshot, id: i32, name: &str) {
        if !snap.workspaces.iter().any(|w| w.id == id) {
            return;
        }
        self.space_gone = id;
        self.arm_ungone(st);
        wm::ask("spaces.moveSpaceTo", serde_json::json!([id, name]));
    }

    fn window_menu(&self, wins: &Windows, snap: &Snapshot, w: &Win, address: &str) -> Vec<MenuItem> {
        let host = Self::host_of(snap);
        let c = wins.clients.iter().find(|c| c.address == address);
        let here = c.and_then(|c| host.get(&c.workspace.id).cloned()).unwrap_or_else(|| w.output.clone());
        let item = |text: String, act: Act| MenuItem { text, header: false, separator: false, enabled: true, act: Some(act) };
        let mut out = Vec::new();
        for n in self.displays(wins) {
            if n == here {
                continue;
            }
            if !out.is_empty() {
                out.push(MenuItem { text: String::new(), header: false, separator: true, enabled: false, act: None });
            }
            let label = self.info.label_for_name(&n);
            out.push(item(format!("Move to {label}"), Act::WindowToDisplay(address.into(), n.clone())));
            let act = snap.active.get(&n).copied().unwrap_or(0);
            for id in self.info.ids_for(&n, snap).into_iter().filter(|i| self.info.is_own(*i, &n) && *i != act).take(8) {
                out.push(item(format!("Move to {} on {label}", self.info.title(id, &n, "Space")), Act::WindowToSpace(address.into(), id)));
            }
        }
        if out.is_empty() {
            out.push(MenuItem { text: "Only one display is connected".into(), header: false, separator: false, enabled: false, act: None });
        }
        out
    }

    fn space_menu(&self, wins: &Windows, snap: &Snapshot, w: &Win, id: i32) -> Vec<MenuItem> {
        let host = Self::host_of(snap);
        let owner = self.info.owner_name(id);
        let here = host.get(&id).cloned().unwrap_or(if owner.is_empty() { w.output.clone() } else { owner });
        let exists = snap.workspaces.iter().any(|x| x.id == id);
        let mut out: Vec<MenuItem> = self
            .displays(wins)
            .into_iter()
            .filter(|n| *n != here)
            .map(|n| MenuItem { text: format!("Move space to {}", self.info.label_for_name(&n)), header: false, separator: false, enabled: exists, act: Some(Act::SpaceToDisplay(id, n)) })
            .collect();
        if out.is_empty() {
            out.push(MenuItem { text: "Only one display is connected".into(), header: false, separator: false, enabled: false, act: None });
        }
        out
    }

    fn open_menu(&mut self, st: &State, i: usize, items: Vec<MenuItem>, x: f32, y: f32) {
        let h = self.wins[i].h;
        self.wins[i].menu = Some(Menu { items, x, y, up: y > h * 0.6, current: -1 });
        self.key_ctx.set(KeyCtx { has_text: !self.text.is_empty(), menu: true });
        let snap = Self::data(st).1;
        Self::push_state(&self.wins[i], &self.gone_win, self.space_gone, &snap);
    }

    fn menu_dismissed(&mut self, st: &mut State, i: usize) {
        self.wins[i].menu = None;
        self.key_ctx.set(KeyCtx { has_text: !self.text.is_empty(), menu: false });
        let snap = Self::data(st).1;
        Self::push_state(&self.wins[i], &self.gone_win, self.space_gone, &snap);
        if self.open && self.wins[i].output == self.key_out {
            self.wins[i].ui.invoke_focus_field();
        }
    }

    fn menu_activate(&mut self, st: &mut State, i: usize, row: i32) {
        let act = self.wins[i].menu.as_ref().and_then(|m| m.items.get(row.max(0) as usize)).filter(|it| it.enabled && row >= 0).and_then(|it| it.act.clone());
        self.menu_dismissed(st, i);
        let snap = Self::data(st).1;
        match act {
            Some(Act::WindowToDisplay(a, n)) => self.move_window_to_display(&a, &n),
            Some(Act::WindowToSpace(a, id)) => self.move_window(&a, id),
            Some(Act::SpaceToDisplay(id, n)) => self.move_space(st, &snap, id, &n),
            None => {}
        }
    }

    // ── The pointer ─────────────────────────────────────────────────────────

    /// The space (and the window in it) under a point
    fn hit(w: &Win, results: bool, mac: bool, px: f32, py: f32) -> Option<(i32, usize, Option<String>)> {
        if results && !mac {
            return None;
        }
        for c in &w.cells {
            if c.rect.has(px, py) {
                let (lx, ly) = (px - c.rect.x, py - c.rect.y);
                let win = w.minis.get(&c.id).and_then(|l| l.iter().rev().find(|m| {
                    let r = Rect { w: if mac { m.rect.w.max(4.0) } else { m.rect.w }, h: if mac { m.rect.h.max(4.0) } else { m.rect.h }, ..m.rect };
                    r.has(lx, ly)
                }));
                return Some((c.id, c.row, win.map(|m| m.address.clone())));
            }
        }
        None
    }

    fn zone_at(w: &Win, px: f32, py: f32) -> Option<&ZoneBox> {
        w.zones.iter().find(|z| z.rect.has(px, py))
    }

    fn chip_at(w: &Win, px: f32, py: f32) -> i32 {
        for z in &w.zones {
            for (id, r) in &z.chips {
                if (Rect { x: z.rect.x + r.x, y: z.rect.y + r.y, ..*r }).has(px, py) {
                    return *id;
                }
            }
        }
        0
    }

    fn edge_at(&self, wins: &Windows, w: &Win, px: f32, py: f32) -> String {
        let displays = self.displays(wins);
        if displays.len() < 2 {
            return String::new();
        }
        let here = geo_of(wins, &w.output, (w.w, w.h));
        let others: Vec<(String, Geo)> = displays.iter().filter(|n| **n != w.output).map(|n| (n.clone(), geo_of(wins, n, (w.w, w.h)))).collect();
        spaces::edge_target(here, &others, px, py, 3.0)
    }

    fn pointer(&mut self, st: &mut State, out: &str, px: f32, py: f32, _inside: bool) {
        let Some(i) = self.wins.iter().position(|w| w.output == out) else { return };
        if !self.open {
            return;
        }
        let (wins, snap) = Self::data(st);
        let mac = Self::cfg(st).style == "macos";
        let results = !self.results.is_empty();
        let edge = self.edge_at(&wins, &self.wins[i], px, py);
        let w = &mut self.wins[i];
        w.pointer = (px, py);
        let press = w.press.clone();
        match press {
            Press::Window { cell, address, from, moved, ofs } => {
                if !moved && (px - from.0).hypot(py - from.1) <= DRAG {
                    return;
                }
                if !moved {
                    w.drag_win = address.clone();
                    w.press = Press::Window { cell, address: address.clone(), from, moved: true, ofs };
                }
                // the miniature stays whole on screen; the pointer goes on
                let c = &w.ui.get_drag_copy();
                let (cw, chh) = (c.w, c.h);
                let mut copy = c.clone();
                copy.x = (px - ofs.0).clamp(0.0, (w.w - cw).max(0.0));
                copy.y = (py - ofs.1).clamp(0.0, (w.h - chh).max(0.0));
                w.ui.set_drag_copy(copy);
                w.ui.set_dragging_win(true);
                w.edge_zone = edge;
                w.drop_cell = Self::hit(w, results, mac, px, py).map(|h| h.0).unwrap_or(0);
                self.track_zone(st, i, px, py);
            }
            Press::Space { id, row, from, moved } => {
                let (cx, cy) = (px.clamp(0.0, w.w - 1.0), py.clamp(0.0, w.h - 1.0));
                let (dx, dy) = (cx - from.0, cy - from.1);
                if !moved && dx.hypot(dy) <= DRAG {
                    return;
                }
                w.press = Press::Space { id, row, from, moved: true };
                w.drag_space = id;
                w.drag_d = (dx, dy);
                w.space_zone = Self::zone_at(w, px, py).map(|z| z.name.clone()).unwrap_or(edge);
                w.drop_on = if w.space_zone.is_empty() { layout::nearest(&w.cells, row, id, px, py) } else { 0 };
            }
            Press::Tile { id, row, from, window, moved } => {
                let (dxp, dyp) = (px - from.0, py - from.1);
                if !moved && dxp.hypot(dyp) <= 8.0 {
                    return;
                }
                w.press = Press::Tile { id, row, from, window: window.clone(), moved: true };
                if let Some(a) = window {
                    // a window: its app's icon follows the pointer
                    w.drag_win = a.clone();
                    let class = wins.clients.iter().find(|c| c.address == a).map(|c| c.class.clone()).unwrap_or_default();
                    let scale = w.ui.window().scale_factor();
                    if let Some(img) = self.icons.get(st, &class, 30.0, scale, false) {
                        w.ui.set_proxy_icon(img);
                    }
                    w.ui.set_proxy_shown(true);
                    w.ui.set_proxy_x(px);
                    w.ui.set_proxy_y(py);
                    w.edge_zone = edge;
                    w.drop_cell = Self::hit(w, results, mac, px, py).map(|h| h.0).unwrap_or(0);
                    self.icons.flush(send(st));
                    self.track_zone(st, i, px, py);
                } else {
                    // the space, along the strip (and off it toward a zone)
                    let dx = px.clamp(0.0, w.w - 1.0) - from.0;
                    let dy = if w.zones.is_empty() { 0.0 } else { py.clamp(0.0, w.h - 1.0) - from.1 };
                    w.drag_space = id;
                    w.drag_d = (dx, dy);
                    w.space_zone = Self::zone_at(w, px, py).map(|z| z.name.clone()).unwrap_or(edge);
                    let cx = w.cells.iter().find(|c| c.id == id).map(|c| c.rect.x + c.rect.w / 2.0).unwrap_or(px);
                    w.drop_on = if w.space_zone.is_empty() { layout::nearest_x(&w.cells, row, cx + dx) } else { 0 };
                }
            }
            Press::None => {
                let hit = Self::hit(w, results, mac, px, py);
                w.hover_cell = hit.as_ref().map(|h| h.0).unwrap_or(0);
                w.hover_win = hit.as_ref().and_then(|h| h.2.clone()).filter(|_| !mac).unwrap_or_default();
                let title = w.cells.iter().find(|c| Self::title_hit(c, px, py)).map(|c| c.id);
                if let Some(id) = hit.as_ref().map(|h| h.0).or(title) {
                    if !mac {
                        w.picked = id;
                    }
                }
            }
            _ => {}
        }
        let (gone, sg) = (self.gone_win.clone(), self.space_gone);
        Self::push_state(&self.wins[i], &gone, sg, &snap);
    }

    /// A window dragged over a zone: held there a moment, its spaces open
    fn track_zone(&mut self, st: &State, i: usize, px: f32, py: f32) {
        let w = &mut self.wins[i];
        let over = Self::zone_at(w, px, py).map(|z| z.name.clone()).unwrap_or_default();
        let chip = Self::chip_at(w, px, py);
        w.zone_hot = if chip > 0 { String::new() } else { over.clone() };
        w.chip_hot = chip;
        let out = w.output.clone();
        if !over.is_empty() && w.expanded.as_deref() != Some(over.as_str()) {
            if !w.timers[0].running() {
                let to = send(st);
                let (o, z) = (out.clone(), over.clone());
                w.timers[0].start(slint::TimerMode::SingleShot, Duration::from_millis(600), move || to(Box::new(M::Expand(o.clone(), z.clone()))));
            }
        } else {
            w.timers[0].stop();
        }
        let still = w.expanded.as_ref().is_some_and(|e| *e == over || chip > 0);
        if w.expanded.is_some() && !still {
            if !w.timers[1].running() {
                let to = send(st);
                let z = w.expanded.clone().unwrap_or_default();
                w.timers[1].start(slint::TimerMode::SingleShot, Duration::from_millis(350), move || to(Box::new(M::Collapse(out.clone(), z.clone()))));
            }
        } else {
            w.timers[1].stop();
        }
    }

    fn press(&mut self, st: &mut State, out: &str, px: f32, py: f32, button: i32) {
        let Some(i) = self.wins.iter().position(|w| w.output == out) else { return };
        if !self.open {
            return;
        }
        let (wins, snap) = Self::data(st);
        let mac = Self::cfg(st).style == "macos";
        let results = !self.results.is_empty();
        let w = &self.wins[i];
        let hit = Self::hit(w, results, mac, px, py);
        let title = if results && !mac { None } else { w.cells.iter().find(|c| Self::title_hit(c, px, py)).map(|c| (c.id, c.row)) };
        let plus = if mac { w.plus.iter().position(|p| p.has(px, py)) } else { None };
        let zone = Self::zone_at(w, px, py).map(|z| z.name.clone());
        if button == 1 {
            // right: a space's menu at once (a window's on release)
            if let Some((id, _, None)) = &hit {
                let items = self.space_menu(&wins, &snap, w, *id);
                self.open_menu(st, i, items, px, py);
            } else if let (None, Some((id, _)), false) = (&hit, title, mac) {
                let items = self.space_menu(&wins, &snap, w, id);
                self.open_menu(st, i, items, px, py);
            } else if let Some((id, _, Some(_))) = &hit {
                if mac {
                    let items = self.space_menu(&wins, &snap, w, *id);
                    self.open_menu(st, i, items, px, py);
                }
            }
            return;
        }
        if button != 0 {
            return;
        }
        let w = &mut self.wins[i];
        w.press = if let Some(p) = plus {
            Press::Plus(p)
        } else if let Some(z) = zone {
            Press::Zone(z)
        } else if let Some((id, row, win)) = hit {
            w.pressed_cell = if win.is_some() && !mac { 0 } else { id };
            if mac {
                Press::Tile { id, row, from: (px, py), window: win, moved: false }
            } else if let Some(a) = win {
                let c = w.cells.iter().find(|c| c.id == id).unwrap();
                let mi = w.minis.get(&id).and_then(|l| l.iter().find(|m| m.address == a)).unwrap();
                let (ax, ay) = (c.rect.x + mi.rect.x, c.rect.y + mi.rect.y);
                // its copy, for the drag (the picture it shows)
                let pic = self.thumbs.images.get(&a).cloned();
                let scale = w.ui.window().scale_factor();
                let icon = self.icons.get(st, &mi.class, 32f32.min(mi.rect.h * 0.4).max(12.0), scale, true);
                let m = st.shell.look.material;
                w.ui.set_drag_copy(ui::OvWin {
                    address: a.as_str().into(),
                    x: ax,
                    y: ay,
                    w: mi.rect.w,
                    h: mi.rect.h,
                    radius: if m { if mi.full { 28.0 } else { 12.0 } } else if mi.full { 13.0 } else { 4.0 },
                    has_picture: pic.is_some(),
                    picture: pic.unwrap_or_default(),
                    has_icon: icon.is_some(),
                    icon: icon.clone().unwrap_or_default(),
                    badge: icon.unwrap_or_default(),
                    name: mi.title.as_str().into(),
                    failed: false,
                });
                Press::Window { cell: id, address: a, from: (px, py), moved: false, ofs: (px - ax, py - ay) }
            } else {
                Press::Space { id, row, from: (px, py), moved: false }
            }
        } else if let Some((id, row)) = title {
            if mac {
                Press::Tile { id, row, from: (px, py), window: None, moved: false }
            } else {
                Press::Space { id, row, from: (px, py), moved: false }
            }
        } else if mac && py < w.bar_h {
            Press::None // (a click on the bar doesn't close)
        } else if results && (Rect { x: w.ui.get_sx() + 20.0, y: w.ui.get_res_y() + 20.0, w: 560.0, h: w.ui.get_res_h() - 40.0 }).has(px, py) {
            Press::None
        } else {
            Press::Background
        };
        let (gone, sg) = (self.gone_win.clone(), self.space_gone);
        Self::push_state(&self.wins[i], &gone, sg, &snap);
    }

    fn release(&mut self, st: &mut State, out: &str, px: f32, py: f32, button: i32) {
        let Some(i) = self.wins.iter().position(|w| w.output == out) else { return };
        let (wins, snap) = Self::data(st);
        let mac = Self::cfg(st).style == "macos";
        let results = !self.results.is_empty();
        if button == 1 && self.open {
            // right-click on a window: its menu
            if let Some((_, _, Some(a))) = Self::hit(&self.wins[i], results, mac, px, py) {
                if !mac {
                    let items = self.window_menu(&wins, &snap, &self.wins[i], &a);
                    self.open_menu(st, i, items, px, py);
                }
            }
            return;
        }
        let press = std::mem::take(&mut self.wins[i].press);
        self.wins[i].pressed_cell = 0;
        let cancel = button == 3;
        match press {
            Press::Window { cell, address, moved, .. } => {
                let w = &self.wins[i];
                if !moved {
                    if !cancel {
                        self.focus_window(st, &address);
                    }
                } else if !cancel {
                    let (edge, chip, zone, target) = (w.edge_zone.clone(), w.chip_hot, w.zone_hot.clone(), w.drop_cell);
                    let ws_of = wins.clients.iter().find(|c| c.address == address).map(|c| c.workspace.id).unwrap_or(0);
                    let already = !edge.is_empty() && snap.active.get(&edge) == Some(&ws_of);
                    let moved_it = if chip > 0 {
                        self.move_window(&address, chip);
                        true
                    } else if !edge.is_empty() && !already {
                        self.move_window_to_display(&address, &edge);
                        true
                    } else if !zone.is_empty() {
                        self.move_window_to_display(&address, &zone);
                        true
                    } else if target > 0 && target != cell && target != ws_of {
                        self.move_window(&address, target);
                        true
                    } else {
                        false
                    };
                    if moved_it {
                        self.gone_win = address;
                        self.arm_ungone(st);
                    }
                }
                let w = &mut self.wins[i];
                w.ui.set_dragging_win(false);
                Self::reset_drag(w);
            }
            Press::Space { id, row, moved, .. } => {
                let w = &self.wins[i];
                if moved {
                    let (zone, drop) = (w.space_zone.clone(), w.drop_on);
                    let order = w.rows.get(row).map(|r| r.1.clone()).unwrap_or_default();
                    if !cancel {
                        if !zone.is_empty() {
                            self.move_space(st, &snap, id, &zone);
                        } else if drop > 0 && drop != id {
                            wm::ask("spaces.move", serde_json::json!([order, id, drop]));
                        }
                    }
                    Self::reset_drag(&mut self.wins[i]);
                } else if !cancel {
                    self.go(st, id);
                }
            }
            Press::Tile { id, row, window, moved, .. } => {
                let w = &self.wins[i];
                if let (Some(a), true) = (&window, moved) {
                    if !cancel {
                        let (edge, chip, zone, target) = (w.edge_zone.clone(), w.chip_hot, w.zone_hot.clone(), w.drop_cell);
                        let ws_of = wins.clients.iter().find(|c| c.address == *a).map(|c| c.workspace.id).unwrap_or(0);
                        if chip > 0 {
                            self.move_window(a, chip);
                        } else if !zone.is_empty() {
                            self.move_window_to_display(a, &zone);
                        } else if !edge.is_empty() && snap.active.get(&edge) != Some(&ws_of) {
                            self.move_window_to_display(a, &edge);
                        } else if target > 0 && target != ws_of {
                            self.move_window(a, target);
                        }
                    }
                    let w = &mut self.wins[i];
                    w.ui.set_proxy_shown(false);
                    Self::reset_drag(w);
                } else if let (Some(a), false) = (&window, moved) {
                    if !cancel {
                        let a = a.clone();
                        self.focus_window(st, &a);
                    }
                } else if moved {
                    let (zone, drop) = (w.space_zone.clone(), w.drop_on);
                    let order = w.rows.get(row).map(|r| r.1.clone()).unwrap_or_default();
                    if !cancel {
                        if !zone.is_empty() {
                            self.move_space(st, &snap, id, &zone);
                        } else if drop > 0 && drop != id {
                            wm::ask("spaces.move", serde_json::json!([order, id, drop]));
                        }
                    }
                    Self::reset_drag(&mut self.wins[i]);
                } else if !cancel {
                    self.go(st, id);
                }
            }
            Press::Plus(p) => {
                if !cancel && self.wins[i].plus.get(p).is_some_and(|r| r.has(px, py)) {
                    let name = self.wins[i].rows.get(p).map(|r| r.0.clone()).unwrap_or_default();
                    self.close(st, true);
                    wm::ask("spaces.newSpace", serde_json::json!([name]));
                }
            }
            Press::Zone(z) => {
                if !cancel && Self::zone_at(&self.wins[i], px, py).is_some_and(|zz| zz.name == z) {
                    let id = snap.active.get(&z).copied().filter(|x| *x > 0).unwrap_or_else(|| self.info.id_for(&z, 1));
                    self.go(st, id);
                }
            }
            Press::Background => {
                if !cancel {
                    self.close(st, true);
                }
            }
            Press::None => {}
        }
        if let Some(w) = self.wins.get_mut(i) {
            let (zone_defs_mac, m) = (Self::cfg(st).style == "macos", st.shell.look.material);
            Self::zones_into(&self.info, &snap, w, zone_defs_mac, m);
            Self::push_state(w, &self.gone_win, self.space_gone, &snap);
        }
    }

    /// A window or space dropped elsewhere stays hidden until it lands (or
    /// 1.5 s pass: the move didn't happen)
    fn arm_ungone(&mut self, st: &State) {
        let to = send(st);
        self.gone_timer.start(slint::TimerMode::SingleShot, Duration::from_millis(1500), move || to(Box::new(M::Ungone)));
    }

    fn apps_changed(&mut self, st: &State) {
        let Some(a) = st.shell.get::<Apps>() else { return };
        self.apps = a
            .entries
            .iter()
            .filter(|e| !e.no_display)
            .map(|e| rank::App { id: e.id.clone(), name: e.name.clone(), generic: e.generic_name.clone(), comment: e.comment.clone(), keywords: e.keywords.clone(), icon: e.icon.clone() })
            .collect();
    }

    /// Glass: the field, the results, the zones and the menu on the plugin's glass
    fn glass(&mut self, st: &mut State) {
        let look = st.shell.look.clone();
        for w in &mut self.wins {
            if !w.surface.mapped.get() || !st.shell.plugin || look.material {
                continue;
            }
            let ui = &w.ui;
            let o = ui.get_openness().clamp(0.0, 1.0);
            let k = ui.get_content_scale();
            let (cx, cy) = (w.w / 2.0, w.h / 2.0);
            let sc = |r: Rect| Rect { x: cx + (r.x - cx) * k, y: cy + (r.y - cy) * k, w: r.w * k, h: r.h * k };
            let mut shapes = Vec::new();
            let f = sc(Rect { x: ui.get_sx() + 20.0, y: ui.get_sy() + 20.0, w: 560.0, h: 52.0 });
            shapes.push(theme::glass_shape(&look, f.x, f.y, f.w, f.h, 26.0 * k, o, 1.0));
            if ui.get_res_h() > 0.0 && w.output == self.key_out {
                let r = sc(Rect { x: ui.get_sx() + 20.0, y: ui.get_res_y() + 20.0, w: 560.0, h: ui.get_res_h() - 40.0 });
                shapes.push(theme::glass_shape(&look, r.x, r.y, r.w, r.h, 22.0 * k, o, 1.0));
            }
            for z in &w.zones {
                let r = sc(z.rect);
                let rad = if !z.chips.is_empty() || z.side.vertical() { 18.0 } else { 22.0 };
                shapes.push(theme::glass_shape(&look, r.x, r.y, r.w, r.h, rad * k, o, 1.0));
            }
            if let Some(mn) = &w.menu {
                let r = mn.rect(w.w, w.h);
                shapes.push(theme::glass_shape(&look, r.x, r.y, r.w, r.h, 14.0, o, 0.35));
            }
            let key = format!("{shapes:?}");
            if key != w.glass_sent {
                st.shell.glass.set(&w.output, NS, &shapes);
                w.glass_sent = key;
            }
        }
    }
}

impl Part for Overview {
    fn name(&self) -> &'static str {
        NAME
    }

    fn start(&mut self, st: &mut State) {
        for s in ["clients", "hypr", "apps", "icons"] {
            st.shell.want(s);
        }
        self.actions = rank::actions();
        self.home = std::env::var("HOME").unwrap_or_default();
        self.info.count = wm::workspace_count(&st.shell.cfg);
    }

    fn command(&mut self, st: &mut State, cmd: &Command) {
        match cmd.cmd.as_str() {
            "open" => self.open(st, cmd.screen.clone()),
            "close" => self.close(st, false),
            _ => {}
        }
    }

    fn config_changed(&mut self, st: &mut State, look: bool) {
        self.info.count = wm::workspace_count(&st.shell.cfg);
        for i in 0..self.wins.len() {
            if look {
                st.shell.apply_theme(&self.wins[i].ui.global::<ui::Theme>());
                self.wins[i].glass_sent.clear();
            }
            self.look(st, i);
        }
        if self.open {
            if Self::cfg(st).wallpapers {
                self.wall.want(st, NAME, (400, 250));
            }
            self.rebuild(st);
        }
    }

    fn plugin_changed(&mut self, st: &mut State) {
        for i in 0..self.wins.len() {
            st.shell.apply_theme(&self.wins[i].ui.global::<ui::Theme>());
            self.wins[i].glass_sent.clear();
            self.look(st, i);
        }
    }

    fn data(&mut self, st: &mut State, what: std::any::TypeId) {
        if what == std::any::TypeId::of::<Apps>() {
            self.apps_changed(st);
        }
        if what == std::any::TypeId::of::<Windows>() {
            // a window dropped elsewhere shows again where it landed
            let (wins, _) = Self::data(st);
            if !self.gone_win.is_empty() && wins.clients.iter().all(|c| c.address != self.gone_win || c.workspace.id != 0) {
                self.gone_win.clear();
            }
        }
        if self.open && (what == std::any::TypeId::of::<Windows>() || what == std::any::TypeId::of::<Snapshot>() || what == std::any::TypeId::of::<Icons>() || what == std::any::TypeId::of::<Apps>()) {
            // (not mid-drag: the layout under the pointer stays put)
            if self.wins.iter().all(|w| matches!(w.press, Press::None)) {
                self.rebuild(st);
            }
        }
    }

    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        // From the shell (RustWindows.qml): the spaces' blocks and names
        let msg = match msg.downcast::<serde_json::Value>() {
            Ok(v) => {
                if v["spaces"].is_object() {
                    let count = wm::workspace_count(&st.shell.cfg);
                    self.info = spaces::Info::from_json(&v["spaces"], count);
                    if self.open {
                        self.rebuild(st);
                    }
                }
                return;
            }
            Err(m) => m,
        };
        let msg = match msg.downcast::<Shot>() {
            Ok(s) => {
                if self.thumbs.got(*s) && self.open {
                    self.rebuild(st);
                }
                return;
            }
            Err(m) => m,
        };
        let msg = match msg.downcast::<IconsDone>() {
            Ok(d) => {
                self.icons.done(*d);
                if self.open {
                    self.rebuild(st);
                }
                return;
            }
            Err(m) => m,
        };
        let msg = match msg.downcast::<LiveTick>() {
            Ok(_) => {
                self.thumbs.tick(st, NAME);
                return;
            }
            Err(m) => m,
        };
        let msg = match msg.downcast::<WallPicture>() {
            Ok(p) => {
                self.wall.got(*p);
                if self.open {
                    self.rebuild(st);
                }
                return;
            }
            Err(m) => m,
        };
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Key(t, _, _) => self.key(st, &t),
            M::Edited(t) => self.edited(st, t),
            M::Pointer(o, x, y, inside) => self.pointer(st, &o, x, y, inside),
            M::Press(o, x, y, b) => self.press(st, &o, x, y, b),
            M::Release(o, x, y, b) => self.release(st, &o, x, y, b),
            M::RunResult(i) => self.run_result(st, i.max(0) as usize),
            M::MenuActivate(o, r) => {
                if let Some(i) = self.wins.iter().position(|w| w.output == o) {
                    self.menu_activate(st, i, r);
                }
            }
            M::MenuDismissed(o) => {
                if let Some(i) = self.wins.iter().position(|w| w.output == o) {
                    self.menu_dismissed(st, i);
                }
            }
            M::Closed(o) => self.closed(st, &o),
            M::Expand(o, z) => {
                if let Some(i) = self.wins.iter().position(|w| w.output == o) {
                    let snap = Self::data(st).1;
                    let n = self.info.ids_for(&z, &snap).into_iter().filter(|id| self.info.is_own(*id, &z)).count();
                    let w = &mut self.wins[i];
                    if n > 1 && w.zone_hot == z {
                        w.expanded = Some(z);
                        let (mac, m) = (Self::cfg(st).style == "macos", st.shell.look.material);
                        Self::zones_into(&self.info, &snap, w, mac, m);
                    }
                }
            }
            M::Collapse(o, z) => {
                if let Some(i) = self.wins.iter().position(|w| w.output == o) {
                    let snap = Self::data(st).1;
                    let w = &mut self.wins[i];
                    if w.expanded.as_deref() == Some(z.as_str()) && w.zone_hot != z && w.chip_hot == 0 {
                        w.expanded = None;
                        let (mac, m) = (Self::cfg(st).style == "macos", st.shell.look.material);
                        Self::zones_into(&self.info, &snap, w, mac, m);
                    }
                }
            }
            M::Ungone => {
                self.gone_win.clear();
                self.space_gone = 0;
                let snap = Self::data(st).1;
                for w in &self.wins {
                    Self::push_state(w, "", 0, &snap);
                }
            }
        }
    }

    fn keyboard_left(&mut self, st: &mut State, s: &Rc<Surface>) {
        // (another window took the keyboard: as the QML panel, close)
        if self.open && self.wins.iter().any(|w| Rc::ptr_eq(&w.surface, s) && w.output == self.key_out && w.menu.is_none()) {
            self.close(st, true);
        }
    }

    fn output_removed(&mut self, st: &mut State, name: &str) {
        if let Some(i) = self.wins.iter().position(|w| w.output == name) {
            if self.key_out == name && self.open {
                self.close(st, true);
            }
            let w = self.wins.remove(i);
            Glass::clear_now(&w.output, NS);
            st.drop_surface(&w.surface);
        }
    }

    fn surface_closed(&mut self, st: &mut State, s: &Rc<Surface>) {
        if let Some(w) = self.wins.iter().find(|w| Rc::ptr_eq(&w.surface, s)) {
            let name = w.output.clone();
            self.output_removed(st, &name);
        }
    }

    fn after_frame(&mut self, st: &mut State) {
        let kbd = st.keyboard_surface();
        for w in &mut self.wins {
            let f = kbd.as_ref().is_some_and(|k| Rc::ptr_eq(k, &w.surface));
            if let Some(a) = w.a11y.as_mut() {
                a.update(false, f);
            }
        }
        self.glass(st);
    }

    fn a11y(&mut self) -> Vec<&mut crate::a11y::A11y> {
        self.wins.iter_mut().filter_map(|w| w.a11y.as_mut()).collect()
    }

    fn quit(&mut self, _st: &mut State) {
        for w in &self.wins {
            Glass::clear_now(&w.output, NS);
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
    fn the_field_keeps_typing_keys() {
        let k = |t: Key| SharedString::from(t).to_string();
        let empty = KeyCtx::default();
        let typed = KeyCtx { has_text: true, menu: false };
        assert!(handles(empty, &k(Key::Escape)) && handles(empty, &k(Key::LeftArrow)));
        // with text the arrows move the cursor
        assert!(!handles(typed, &k(Key::LeftArrow)) && handles(typed, &k(Key::DownArrow)));
        assert!(!handles(empty, "a"));
        // a menu takes Space too
        assert!(handles(KeyCtx { has_text: false, menu: true }, " "));
    }

    #[test]
    fn menus_step_over_what_cant_be_chosen() {
        let it = |t: &str, enabled: bool| MenuItem { text: t.into(), header: false, separator: false, enabled, act: None };
        let sep = MenuItem { text: String::new(), header: false, separator: true, enabled: false, act: None };
        let mut m = Menu { items: vec![it("Move to U2720Q", true), sep, it("x", false), it("Move to Space 2 on U2720Q", true)], x: 100.0, y: 100.0, up: false, current: -1 };
        m.step(1);
        assert_eq!(m.current, 0);
        m.step(1);
        assert_eq!(m.current, 3);
        m.step(1);
        assert_eq!(m.current, 3);
        assert_eq!(m.width(), "Move to Space 2 on U2720Q".len() as f32 * 7.5 + 40.0);
        assert_eq!(m.rows_h(), 28.0 * 3.0 + 11.0);
        // near the bottom it opens upward
        let r = Menu { y: 1000.0, up: true, ..m }.rect(1920.0, 1080.0);
        assert!(r.y + r.h <= 1000.0);
    }
}

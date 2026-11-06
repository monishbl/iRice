//! The emoji picker (rust.surfaces: emoji): emoji, kaomoji, text symbols and
//! GIFs, near the pointer in the app you're typing in. Enter or a click
//! types the pick into that app and copies it; Shift copies only. Compare
//! shell/modules/emoji/ (EmojiPicker.qml, PickerWindow.qml, EmojiStore.qml,
//! Klipy.qml).
//!
//! It opens as the QML one did, through Panels ("emoji": Super+Period,
//! `qs ipc call emoji toggle`, Spotlight): RustUi.qml forwards the open and
//! close as commands (src/shell.rs), and it says when it closed itself.
//!
//! Differences from the QML, where Slint can't do the same:
//! - the selected GIF shows its still, not the moving picture (Slint has no
//!   animated image);
//! - Material's empty-state shapes are outlines from src/surfaces/shapes.rs
//!   (near the QML's shader shapes, not the same curves);
//! - Material draws its own tonal surface (no glass), as the menu bar does.

pub mod ui {
    slint::include_modules!();
}
tessera_core::theme_global!(ui);

// tessera-core's modules (shell, wayland, theme, services, util…) under the
// names they had when everything was one crate: `crate::shell::Part`
#[allow(unused_imports)]
use tessera_core::*;

pub mod data;
pub mod klipy;
pub mod view;

use tessera_core::services::hypr::{self, Glass};
use tessera_core::shell::{Command, Part};
use tessera_core::theme;
use tessera_core::wayland::{Spec, State, Surface};
use data::{Data, Remembered};
use slint::platform::Key;
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use smithay_client_toolkit::shell::WaylandSurface;
use std::any::Any;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};
use view::{Kind, Mode, View};

const NAME: &str = "emoji";
pub const NS: &str = "tessera:emoji";
/// how long it stays closed before its window goes (made again on opening)
const RETIRE_SECS: u64 = 10;
const PANEL_W: f32 = 420.0;
const PANEL_H: f32 = 428.0;
const LIST_TOP: f32 = 36.0 + 8.0 + 32.0 + 4.0; // in the content, under the tabs

/// Its own messages (Msg::Part("emoji", …))
enum M {
    Loaded(Result<Arc<Data>, String>, Vec<String>),
    Placed(Placement),
    PlaceTimeout(u64),
    /// its first drawing took too long to reach the screen: open anyway
    WarmTimeout(u64),
    /// a while after start: draw the window once unseen, so the first open
    /// doesn't pay for the window, the emoji font and its glyphs
    Prewarm,
    Search,
    Key(String, bool, bool),
    Edited(String),
    Hovered(i32),
    Activate(i32, bool),
    ToneFor(i32),
    TonePick(i32, bool),
    ToneClose,
    SetTab(i32),
    Scrolled(f32),
    GifHovered(i32),
    GifActivate(i32, bool),
    GifScrolled(f32, f32),
    GifPage(u64, u32, Result<(Vec<klipy::Gif>, bool), klipy::Status>),
    GifPreview(u64, usize, slint::SharedPixelBuffer<slint::Rgba8Pixel>),
    StateAct(bool),
    /// closed long enough (its opening's number): the window goes
    Retire(u64),
    Dismissed,
    Closed,
}

/// Where the pointer and the focused window were as it opened (global
/// layout coordinates)
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Placement {
    pub pointer: Option<(f64, f64)>,
    /// x, y, w, h, address
    pub window: Option<(f64, f64, f64, f64, String)>,
}

/// The panel's top-left on a screen (origin ox, oy; size sw, sh), as
/// PickerWindow.place(): Wayland doesn't say where the text cursor is, so
/// below and right of the pointer when it's over the focused window (flipped
/// where there's no room), else centred in that window, else on the screen
pub fn place(screen: (f64, f64, f64, f64), p: &Placement, panel: (f64, f64)) -> (f64, f64) {
    let (ox, oy, sw, sh) = screen;
    let (pw, ph) = panel;
    let gap = 8.0;
    let (mut x, mut y) = ((sw - pw) / 2.0, (sh - ph) / 2.0);
    if let Some((wx, wy, ww, wh, _)) = &p.window {
        let (cx, cy) = (wx + ww / 2.0, wy + wh / 2.0);
        // (the window has to be mostly on this screen to count)
        if cx >= ox && cx < ox + sw && cy >= oy && cy < oy + sh {
            match p.pointer {
                Some((px, py)) if px >= *wx && px < wx + ww && py >= *wy && py < wy + wh => {
                    let (lx, ly) = (px - ox, py - oy);
                    x = lx + gap;
                    y = ly + 20.0;
                    if x + pw > sw - gap {
                        x = lx - pw - gap;
                    }
                    if y + ph > sh - gap {
                        y = ly - ph - gap;
                    }
                }
                _ => {
                    x = wx - ox + (ww - pw) / 2.0;
                    y = wy - oy + (wh - ph) / 2.0;
                }
            }
        }
    }
    (x.min(sw - pw - gap).max(gap).round(), y.min(sh - ph - gap).max(gap).round())
}

/// Keys as Slint writes them
fn k(key: Key) -> String {
    SharedString::from(key).to_string()
}

/// The keys the picker handles itself (the field gets the rest): Return,
/// arrows, Tab, Esc (the tone chooser, when open, takes the same ones)
pub fn handles(text: &str) -> bool {
    let nav = [Key::Escape, Key::Return, Key::LeftArrow, Key::RightArrow, Key::UpArrow, Key::DownArrow, Key::Tab, Key::Backtab];
    nav.iter().any(|&n| text == k(n))
}

/// The key caps for Enter and Shift+Enter, in Settings' style (KeyNames.qml)
pub fn key_names(cfg: &serde_json::Value) -> (Vec<String>, Vec<String>) {
    let a = &cfg["appearance"];
    let style = match a["keySymbols"].as_str().unwrap_or("auto") {
        "auto" => {
            if a["menuIcon"].as_str() == Some("command") {
                "mac"
            } else {
                "linux"
            }
        }
        s => s,
    };
    let (shift, ret) = if style == "mac" { ("⇧", "↩") } else { ("Shift", "Enter") };
    (vec![ret.into()], vec![shift.into(), ret.into()])
}

/// (shared: Search shows emoji in the same font)
pub use tessera_core::settings::emoji_font;

/// The footer's words for an item: its name (with the tone you chose), and
/// the line under it
pub fn footer_words(d: &Data, mem: &Remembered, c: view::Cell) -> (String, String, String) {
    let cap = |s: &str| {
        let mut ch = s.chars();
        ch.next().map(|f| f.to_uppercase().collect::<String>() + ch.as_str()).unwrap_or_default()
    };
    match c.kind {
        Kind::Emoji => {
            let e = &d.emoji[c.i];
            let t = mem.tone(e);
            let name = if t > 0 { format!("{}, {} skin tone", cap(&e.n), data::TONE_NAMES[t as usize].to_lowercase()) } else { cap(&e.n) };
            let sub = format!(":{}:{}", e.s, if e.toned() { "  ·  right-click for skin tones" } else { "" });
            (mem.display(e), name, sub)
        }
        Kind::Kaomoji => {
            let it = &d.kaomoji_flat[c.i];
            let first = it.n.split(' ').next().unwrap_or("");
            (it.c.clone(), cap(if first.is_empty() { "Kaomoji" } else { first }), it.n.clone())
        }
        Kind::Symbol => {
            let it = &d.symbols_flat[c.i];
            (it.c.clone(), cap(if it.n.is_empty() { "Symbol" } else { &it.n }), data::code_points(&it.c))
        }
    }
}

struct Win {
    output: String,
    surface: Rc<Surface>,
    ui: ui::EmojiPicker,
    a11y: Option<crate::a11y::A11y>,
    glass_sent: String,
    /// the screen's origin and size (logical)
    screen: (f64, f64, f64, f64),
    /// its content has been drawn once (fonts loaded, glyphs in the atlas)
    warmed: bool,
}

struct Gifs {
    status: klipy::Status,
    list: Vec<klipy::Gif>,
    query: Option<String>,
    page: u32,
    has_next: bool,
    loading_more: bool,
    serial: u64,
    /// the list's generation (a new query): stills for an older list are dropped
    gen: u64,
    sel: usize,
    model: Rc<VecModel<ui::GifCell>>,
}

impl Default for Gifs {
    fn default() -> Self {
        Self { status: klipy::Status::Idle, list: Vec::new(), query: None, page: 0, has_next: false, loading_more: false, serial: 0, gen: 0, sel: 0, model: Rc::new(VecModel::default()) }
    }
}

pub struct Emoji {
    win: Option<Win>,
    open: bool,
    data: Option<Arc<Data>>,
    failed: bool,
    loading: bool,
    fonts: Vec<String>,
    mem: Remembered,
    tab: usize,
    /// the search the list shows (the field's text, a moment later)
    query: String,
    field: String,
    sel: usize,
    keyboard_nav: bool,
    view: View,
    scroll: f32,
    sync_until: Instant,
    /// the tone chooser: the emoji it's for
    tone: Option<usize>,
    placement: Placement,
    opening: u64,
    search_timer: slint::Timer,
    place_timer: slint::Timer,
    gifs: Gifs,
    /// where it opens is known (or waited for long enough)
    placed: bool,
    /// the first open: the frame that draws the content unseen, awaited
    /// before the opening animation starts (else that slow frame eats it)
    warm: Option<u32>,
    warm_timer: slint::Timer,
    /// the first frame of it opening: its glass waits for that one
    glass_from: u32,
    retire_timer: slint::Timer,
    /// the unseen frame drawn before anyone opens it (no glass, no keys,
    /// clicks pass through); the window is kept, hidden, afterwards
    prewarm: bool,
    prewarm_timer: slint::Timer,
}

impl Default for Emoji {
    fn default() -> Self {
        Self {
            win: None,
            open: false,
            data: None,
            failed: false,
            loading: false,
            fonts: Vec::new(),
            mem: Remembered::default(),
            tab: 0,
            query: String::new(),
            field: String::new(),
            sel: 0,
            keyboard_nav: false,
            view: View::default(),
            scroll: 0.0,
            sync_until: Instant::now(),
            tone: None,
            placement: Placement::default(),
            opening: 0,
            search_timer: slint::Timer::default(),
            place_timer: slint::Timer::default(),
            gifs: Gifs::default(),
            placed: false,
            warm: None,
            warm_timer: slint::Timer::default(),
            glass_from: 0,
            retire_timer: slint::Timer::default(),
            prewarm: false,
            prewarm_timer: slint::Timer::default(),
        }
    }
}

fn installed_fonts() -> Vec<String> {
    let Ok(out) = std::process::Command::new("fc-list").args([":", "family"]).output() else { return Vec::new() };
    String::from_utf8_lossy(&out.stdout).lines().flat_map(|l| l.split(',').map(|f| f.trim().to_string()).collect::<Vec<_>>()).filter(|f| f.to_lowercase().contains("emoji")).collect()
}

impl Emoji {
    fn send(st: &State) -> impl Fn(Box<dyn Any + Send>) + Clone + 'static {
        st.shell.sender(NAME)
    }

    fn mode(&self) -> Mode {
        view::mode(self.tab)
    }

    fn searching(&self) -> bool {
        !self.query.trim().is_empty()
    }

    /// The window on this output (made the first time; kept while closed, so
    /// its drawn glyphs are kept too)
    fn window(&mut self, st: &mut State, output: &str) -> bool {
        if self.win.as_ref().is_some_and(|w| w.output != output) {
            let w = self.win.take().unwrap();
            Glass::clear_now(&w.output, NS);
            st.drop_surface(&w.surface);
        }
        let Some(wl_output) = st.output_named(output) else { return false };
        let info = st.outputs.info(&wl_output);
        let (ox, oy) = info.as_ref().and_then(|i| i.logical_position).unwrap_or((0, 0));
        let (sw, sh) = info.as_ref().and_then(|i| i.logical_size).unwrap_or((1920, 1080));
        if let Some(w) = &mut self.win {
            w.screen = (ox as f64, oy as f64, sw as f64, sh as f64);
            return true;
        }
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
        let ui = ui::EmojiPicker::new().expect("EmojiPicker");
        st.shell.apply_theme(&ui.global::<ui::Theme>());
        let tabs: Vec<ui::ETab> = view::TABS.iter().map(|(icon, text, label)| ui::ETab { icon: (*icon).into(), text: (*text).into(), label: (*label).into() }).collect();
        ui.set_tabs(ModelRc::new(VecModel::from(tabs)));
        ui.set_gifs(ModelRc::from(self.gifs.model.clone()));
        let to = Self::send(st);
        {
            let to = to.clone();
            ui.on_key(move |text, shift, alt, _ctrl| {
                let ok = handles(&text);
                if ok {
                    to(Box::new(M::Key(text.to_string(), shift, alt)));
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
        on!(on_activate, |i, c| M::Activate(i, c));
        on!(on_tone_for, |i| M::ToneFor(i));
        on!(on_tone_pick, |i, c| M::TonePick(i, c));
        on!(on_tone_close, | | M::ToneClose);
        on!(on_set_tab, |i| M::SetTab(i));
        on!(on_scrolled, |y| M::Scrolled(y));
        on!(on_gif_hovered, |i| M::GifHovered(i));
        on!(on_gif_activate, |i, c| M::GifActivate(i, c));
        on!(on_gif_scrolled, |y, h| M::GifScrolled(y, h));
        on!(on_state_act, |s| M::StateAct(s));
        on!(on_dismissed, | | M::Dismissed);
        on!(on_closed, | | M::Closed);
        ui.show().expect("show emoji");
        let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
        self.win = Some(Win { output: output.into(), surface, ui, a11y, glass_sent: String::new(), screen: (ox as f64, oy as f64, sw as f64, sh as f64), warmed: false });
        self.settings(st);
        true
    }

    /// Settings it follows: the emoji style, the key caps' style
    fn settings(&mut self, st: &State) {
        let Some(w) = &self.win else { return };
        w.ui.set_emoji_font(emoji_font(&st.shell.cfg, &self.fonts).into());
        let (ins, copy) = key_names(&st.shell.cfg);
        let m = |v: Vec<String>| ModelRc::new(VecModel::from(v.into_iter().map(SharedString::from).collect::<Vec<_>>()));
        w.ui.set_hint_insert(m(ins));
        w.ui.set_hint_copy(m(copy));
    }

    fn load(&mut self, st: &State) {
        if self.data.is_some() || self.loading {
            return;
        }
        self.loading = true;
        let to = Self::send(st);
        std::thread::spawn(move || {
            let d = Data::load().map(Arc::new);
            if let Err(e) = &d {
                eprintln!("tessera-ui: emoji: can't read emoji.json: {e}");
            }
            to(Box::new(M::Loaded(d, installed_fonts())));
        });
    }

    fn open(&mut self, st: &mut State, screen: Option<String>) {
        let output = screen.or_else(|| self.win.as_ref().map(|w| w.output.clone())).or_else(|| st.outputs_now().first().map(|o| o.1.clone()));
        let Some(output) = output else { return };
        if !self.window(st, &output) {
            return;
        }
        self.load(st);
        if self.prewarm {
            self.prewarm = false;
            self.warm = None;
            self.warm_timer.stop();
            if let Some(w) = &self.win {
                w.ui.set_warm(false);
            }
        }
        self.prewarm_timer.stop();
        if let Some(w) = &self.win {
            st.set_input_region(&w.surface, None);
        }
        self.mem = Remembered::load();
        self.open = true;
        self.opening += 1;
        self.placed = false;
        self.warm = None;
        // reset: the list opens at the top, frequently used
        self.tab = 0;
        self.query.clear();
        self.field.clear();
        self.sel = 0;
        self.scroll = 0.0;
        self.tone = None;
        self.keyboard_nav = false;
        self.placement = Placement::default();
        self.rebuild();
        let w = self.win.as_ref().unwrap();
        w.ui.set_tab(0);
        w.ui.set_gif_mode(false);
        w.ui.set_list_y(0.0);
        w.ui.invoke_open();
        w.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        w.surface.show(0, 0);
        self.push(st);
        // where to open: the pointer and the focused window, asked off this
        // thread; opened at the latest after 250 ms (centred)
        let to = Self::send(st);
        let n = self.opening;
        std::thread::spawn(move || {
            let pointer = hypr::request("j/cursorpos").ok().and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok()).and_then(|v| Some((v["x"].as_f64()?, v["y"].as_f64()?)));
            let window = hypr::request("j/activewindow").ok().and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok()).and_then(|v| {
                Some((v["at"][0].as_f64()?, v["at"][1].as_f64()?, v["size"][0].as_f64()?, v["size"][1].as_f64()?, v["address"].as_str()?.to_string()))
            });
            to(Box::new(M::Placed(Placement { pointer, window })));
        });
        let to = Self::send(st);
        self.place_timer.start(slint::TimerMode::SingleShot, Duration::from_millis(250), move || to(Box::new(M::PlaceTimeout(n))));
    }

    fn show_at(&mut self, st: &State) {
        self.placed = true;
        self.open_when_ready(st);
    }

    /// Open it once it's placed and its emoji are read. The first time, the
    /// content is drawn once unseen first and the animation starts when that
    /// frame is on screen: loading the emoji font and drawing its glyphs
    /// takes long enough that the spring would be over before its first frame
    fn open_when_ready(&mut self, st: &State) {
        if !self.open || !self.placed || (self.data.is_none() && !self.failed) || self.warm.is_some() {
            return;
        }
        let Some(w) = &mut self.win else { return };
        if w.ui.get_shown() {
            return;
        }
        let (x, y) = place(w.screen, &self.placement, (PANEL_W as f64, PANEL_H as f64));
        w.ui.set_panel_x(x as f32);
        w.ui.set_panel_y(y as f32);
        if !w.warmed {
            w.warmed = true;
            w.ui.set_warm(true);
            self.warm = Some(w.surface.rendered.get().wrapping_add(1));
            let (to, n) = (Self::send(st), self.opening);
            self.warm_timer.start(slint::TimerMode::SingleShot, Duration::from_millis(400), move || to(Box::new(M::WarmTimeout(n))));
            return;
        }
        self.glass_from = w.surface.rendered.get().wrapping_add(1);
        w.ui.set_shown(true);
    }

    /// The unseen frame is on screen (or it took too long): open
    fn warmed(&mut self) {
        if self.warm.take().is_none() {
            return;
        }
        self.warm_timer.stop();
        let Some(w) = &self.win else { return };
        w.ui.set_warm(false);
        if self.prewarm {
            self.prewarm = false;
            if !self.open {
                w.surface.hide();
            }
            return;
        }
        if self.open {
            self.glass_from = w.surface.rendered.get().wrapping_add(1);
            w.ui.set_shown(true);
        }
    }

    /// Close it (`tell`: it closed itself, so the shell's Panels follows)
    fn close(&mut self, tell: bool) {
        if !self.open {
            return;
        }
        self.open = false;
        self.tone = None;
        self.place_timer.stop();
        if self.warm.take().is_some() {
            self.warm_timer.stop();
            if let Some(w) = &self.win {
                w.ui.set_warm(false);
            }
        }
        if let Some(w) = &self.win {
            w.ui.set_shown(false);
            w.ui.set_tone_open(false);
            w.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::None);
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
        // (a hidden layer stays in Hyprland's list: closed for a while, the
        // window goes, and is made again at the next open)
        let (to, n) = (st.shell.sender(NAME), self.opening);
        self.retire_timer.start(slint::TimerMode::SingleShot, Duration::from_secs(RETIRE_SECS), move || to(Box::new(M::Retire(n))));
    }

    /// Still closed since then: the window goes
    fn retire(&mut self, st: &mut State, n: u64) {
        if self.open || n != self.opening {
            return;
        }
        if let Some(w) = self.win.take() {
            st.shell.glass.set(&w.output, NS, &[]);
            st.drop_surface(&w.surface);
        }
    }

    fn rebuild(&mut self) {
        let Some(d) = &self.data else {
            self.view = View::default();
            return;
        };
        self.view = View::build(d, &self.mem, self.tab, &self.query, crate::util::now_ms());
        self.sel = self.sel.min(self.view.flat.len().saturating_sub(1));
        let Some(w) = &self.win else { return };
        let rows: Vec<ui::ERow> = self
            .view
            .rows
            .iter()
            .map(|r| {
                let (kind, title, start, cols, cells) = match &r.kind {
                    view::RowKind::Header(t) => (0, t.clone(), 0, 0, Vec::new()),
                    view::RowKind::Hint(t) => (1, t.clone(), 0, 0, Vec::new()),
                    view::RowKind::Items { cols, start, cells } => (
                        2,
                        String::new(),
                        *start,
                        *cols,
                        cells
                            .iter()
                            .map(|c| match c.kind {
                                Kind::Emoji => {
                                    let e = &d.emoji[c.i];
                                    ui::ECell { text: self.mem.display(e).into(), name: e.n.as_str().into(), emoji: true, toned: e.toned() }
                                }
                                Kind::Kaomoji => {
                                    let it = &d.kaomoji_flat[c.i];
                                    ui::ECell { text: it.c.as_str().into(), name: it.n.as_str().into(), emoji: false, toned: false }
                                }
                                Kind::Symbol => {
                                    let it = &d.symbols_flat[c.i];
                                    ui::ECell { text: it.c.as_str().into(), name: it.n.as_str().into(), emoji: false, toned: false }
                                }
                            })
                            .collect(),
                    ),
                };
                ui::ERow { kind, y: r.y, h: r.h, title: title.into(), start: start as i32, cols: cols as i32, cells: ModelRc::new(VecModel::from(cells)) }
            })
            .collect();
        w.ui.set_rows(ModelRc::new(VecModel::from(rows)));
        w.ui.set_rows_height(self.view.height);
    }

    /// Selection, footer, state, tones and GIFs into the window
    fn push(&mut self, st: &State) {
        let Some(w) = &self.win else { return };
        let ui = &w.ui;
        let gif = self.mode() == Mode::Gif;
        ui.set_sel(self.sel as i32);
        ui.set_gif_mode(gif);
        ui.set_tab(self.tab as i32);
        ui.set_placeholder(if gif { "Search GIFs" } else { "Search emoji, kaomoji and symbols" }.into());
        // the state area
        let (kind, spin, icon, title, body, action, secondary): (&str, bool, &str, String, String, &str, &str) = if gif {
            let g = &self.gifs;
            match g.status {
                _ if klipy::key(&st.shell.cfg).is_empty() => {
                    ("noKey", false, "\u{e274}", "Add a KLIPY key to use GIFs".into(), "Get a free app key at partner.klipy.com, then paste it in Settings.".into(), "Open Settings", "Get a key")
                }
                klipy::Status::Idle => ("gifLoading", true, "", "Loading trending GIFs…".into(), String::new(), "", ""),
                klipy::Status::Loading if g.list.is_empty() => {
                    ("gifLoading", true, "", if g.query.as_deref().unwrap_or("").is_empty() { "Loading trending GIFs…".into() } else { "Searching GIFs…".into() }, String::new(), "", "")
                }
                klipy::Status::Offline => ("offline", false, "\u{e4f2}", "Can't reach KLIPY".into(), "Check your internet connection, then try again.".into(), "Try again", ""),
                klipy::Status::Error(code) => (
                    "error",
                    false,
                    "\u{e4e2}",
                    "GIFs didn't load".into(),
                    format!("KLIPY returned an error{}. Try again in a moment.", if code > 0 { format!(" ({code})") } else { String::new() }),
                    "Try again",
                    "",
                ),
                klipy::Status::BadKey => ("badKey", false, "\u{e2d6}", "KLIPY didn't accept the key".into(), "Check the app key in Settings. It's on partner.klipy.com.".into(), "Open Settings", ""),
                klipy::Status::Ready if g.list.is_empty() => ("noGifs", false, "\u{e30c}", format!("No GIFs for “{}”", g.query.as_deref().unwrap_or("")), "Try another word.".into(), "", ""),
                _ => ("", false, "", String::new(), String::new(), "", ""),
            }
        } else if self.failed {
            ("dataError", false, "\u{e4e2}", "Emoji couldn't load".into(), "The emoji list is missing or damaged. Run tools/gen-emoji-data.py to rebuild it.".into(), "Try again", "")
        } else if self.data.is_none() {
            ("loading", true, "", "Loading emoji…".into(), String::new(), "", "")
        } else if self.searching() && self.view.flat.is_empty() {
            ("noResults", false, "\u{e30c}", format!("No results for “{}”", self.query.trim()), "Try another word, like heart, cat or shrug.".into(), "", "")
        } else {
            ("", false, "", String::new(), String::new(), "", "")
        };
        ui.set_state_kind(kind.into());
        ui.set_state_spin(spin);
        ui.set_state_icon(icon.into());
        ui.set_state_title(title.into());
        ui.set_state_body(body.into());
        ui.set_state_action(action.into());
        ui.set_state_secondary(secondary.into());
        ui.set_state_problem(matches!(kind, "offline" | "error" | "badKey" | "dataError"));
        ui.set_state_outline(crate::surfaces::shapes::outline(match kind {
            "noResults" | "noGifs" => "clover4",
            "noKey" => "cookie9",
            _ => "softBurst",
        }).into());
        // the footer
        let mut shown = false;
        if kind.is_empty() {
            if gif {
                if let Some(g) = self.gifs.list.get(self.gifs.sel) {
                    ui.set_footer_kind(3);
                    ui.set_footer_name(if g.title.is_empty() { "GIF".into() } else { g.title.as_str().into() });
                    ui.set_footer_sub("Powered by KLIPY".into());
                    shown = true;
                }
            } else if let (Some(d), Some(&c)) = (&self.data, self.view.flat.get(self.sel)) {
                let (glyph, name, sub) = footer_words(d, &self.mem, c);
                ui.set_footer_kind(match c.kind {
                    Kind::Emoji => 0,
                    Kind::Kaomoji => 1,
                    Kind::Symbol => 2,
                });
                ui.set_footer_glyph(glyph.into());
                ui.set_footer_name(name.into());
                ui.set_footer_sub(sub.into());
                shown = true;
            }
        }
        ui.set_footer_shown(shown);
        ui.set_gif_sel(self.gifs.sel as i32);
        ui.set_gif_more(self.gifs.loading_more);
        // the tone chooser
        match (self.tone, &self.data) {
            (Some(e), Some(d)) => {
                let e = &d.emoji[e];
                let forms: Vec<SharedString> = (0..6).map(|t| e.with_tone(t).into()).collect();
                ui.set_tone_forms(ModelRc::new(VecModel::from(forms)));
                ui.set_tone_open(true);
            }
            _ => ui.set_tone_open(false),
        }
    }

    /// Show the selected item (scrolling just enough)
    fn reveal(&mut self) {
        let s = self.view.reveal(self.sel, self.scroll, view::GRID_H);
        if s != self.scroll {
            self.scroll = s;
            if let Some(w) = &self.win {
                w.ui.set_list_y(-s);
            }
        }
    }

    fn set_tab(&mut self, st: &State, t: usize) {
        self.tone = None;
        let was = self.mode();
        self.tab = t.min(view::TABS.len() - 1);
        self.sel = 0;
        if self.mode() == Mode::Gif {
            let q = self.query.clone();
            self.gif_fetch(st, &q);
            self.gifs.sel = 0;
            self.push(st);
            return;
        }
        if self.searching() && self.mode() == was {
            self.push(st);
            return;
        }
        if was != self.mode() {
            self.rebuild();
        }
        let mut target = 0.0;
        if self.mode() == Mode::Emoji && !self.searching() {
            if let Some((y, first)) = self.view.section_of_tab(self.tab) {
                target = y;
                self.sel = first;
            }
            self.sync_until = Instant::now() + Duration::from_millis(250);
        }
        self.scroll = target;
        if let Some(w) = &self.win {
            w.ui.set_list_y(-target);
        }
        self.push(st);
    }

    fn next_tab(&mut self, st: &State, d: i32) {
        let n = view::TABS.len() as i32;
        self.set_tab(st, ((self.tab as i32 + d + n) % n) as usize);
    }

    /// Copy it and (unless `copy_only`) type it into the window that had
    /// focus; the panel closes first so focus can go back there
    fn pick(&mut self, text: String, kind: &str, copy_only: bool, key: &str) {
        if text.is_empty() {
            return;
        }
        self.mem.record(key, kind, crate::util::now_ms());
        self.mem.save();
        let target = self.placement.window.as_ref().map(|w| w.4.clone()).unwrap_or_default();
        self.close(true);
        crate::util::pipe_to(&["wl-copy"], text.clone().into_bytes());
        if copy_only {
            return;
        }
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(140)); // (the layer gives up the keyboard first)
            if !target.is_empty() {
                let _ = hypr::request(&format!("dispatch hl.dsp.focus({{ window = \"address:0x{}\" }})", target.trim_start_matches("0x")));
            }
            std::thread::sleep(Duration::from_millis(60));
            crate::util::pipe_to(&["wtype", "-"], text.into_bytes());
        });
    }

    fn activate(&mut self, i: usize, copy_only: bool) {
        let (Some(d), Some(&c)) = (self.data.clone(), self.view.flat.get(i)) else { return };
        match c.kind {
            Kind::Emoji => {
                let e = &d.emoji[c.i];
                self.pick(self.mem.display(e), "emoji", copy_only, &e.c);
            }
            Kind::Kaomoji => self.pick(d.kaomoji_flat[c.i].c.clone(), "kaomoji", copy_only, &d.kaomoji_flat[c.i].c),
            Kind::Symbol => self.pick(d.symbols_flat[c.i].c.clone(), "symbol", copy_only, &d.symbols_flat[c.i].c),
        }
    }

    /// The tone chooser for an item, above its cell (below near the top)
    fn tone_for(&mut self, st: &State, i: usize) {
        let (Some(d), Some(&c)) = (&self.data, self.view.flat.get(i)) else { return };
        if c.kind != Kind::Emoji || !d.emoji[c.i].toned() {
            return;
        }
        self.sel = i;
        self.tone = Some(c.i);
        let (row, col) = self.view.pos[i];
        let r = &self.view.rows[row];
        let cols = match &r.kind {
            view::RowKind::Items { cols, .. } => *cols,
            _ => view::COLS,
        } as f32;
        let cw = 396.0 / cols;
        let cx = (col as f32 + 0.5) * cw;
        let cy = LIST_TOP + r.y - self.scroll;
        let (tw, th) = (6.0 * 40.0 + 8.0, 48.0);
        let x = (cx - tw / 2.0).clamp(0.0, 396.0 - tw);
        let y = if cy - th - 4.0 >= LIST_TOP { cy - th - 4.0 } else { cy + r.h + 4.0 };
        if let Some(w) = &self.win {
            w.ui.set_tone_x(x);
            w.ui.set_tone_y(y);
            w.ui.set_tone_choice(self.mem.tone(&d.emoji[c.i]) as i32);
        }
        self.push(st);
    }

    fn tone_pick(&mut self, t: u8, copy_only: bool) {
        let (Some(d), Some(e)) = (self.data.clone(), self.tone.take()) else { return };
        let e = &d.emoji[e];
        self.mem.set_tone(e, t);
        self.pick(e.with_tone(t), "emoji", copy_only, &e.c);
    }

    fn key(&mut self, st: &State, text: &str, shift: bool, alt: bool) {
        let is = |key: Key| text == k(key);
        let gif = self.mode() == Mode::Gif;
        if let Some(e) = self.tone {
            let choice = self.win.as_ref().map_or(0, |w| w.ui.get_tone_choice());
            if is(Key::Escape) || is(Key::UpArrow) || is(Key::DownArrow) || is(Key::Tab) || is(Key::Backtab) {
                self.tone = None;
            } else if is(Key::LeftArrow) || is(Key::RightArrow) {
                let c = (choice + if is(Key::LeftArrow) { -1 } else { 1 }).clamp(0, 5);
                if let Some(w) = &self.win {
                    w.ui.set_tone_choice(c);
                }
            } else if is(Key::Return) {
                self.tone = Some(e);
                self.tone_pick(choice.clamp(0, 5) as u8, shift);
                return;
            }
            self.push(st);
            return;
        }
        if is(Key::Escape) {
            self.close(true);
        } else if is(Key::Tab) {
            // (Shift+Tab: Backtab from a keyboard; a virtual one may send Tab with Shift)
            self.next_tab(st, if shift { -1 } else { 1 });
        } else if is(Key::Backtab) {
            self.next_tab(st, -1);
        } else if is(Key::LeftArrow) || is(Key::RightArrow) {
            let d = if is(Key::LeftArrow) { -1 } else { 1 };
            if gif {
                self.gif_move(st, d);
            } else {
                self.keyboard_nav = true;
                self.sel = self.view.move_h(self.sel, d);
                self.reveal();
                self.push(st);
            }
        } else if is(Key::UpArrow) || is(Key::DownArrow) {
            let d = if is(Key::UpArrow) { -1 } else { 1 };
            if !gif && d > 0 && alt {
                self.tone_for(st, self.sel);
            } else if gif {
                self.gif_move(st, 3 * d);
            } else {
                self.keyboard_nav = true;
                self.sel = self.view.move_v(self.sel, d);
                self.reveal();
                self.push(st);
            }
        } else if is(Key::Return) {
            let action = self.win.as_ref().is_some_and(|w| !w.ui.get_state_action().is_empty() && !w.ui.get_state_kind().is_empty());
            if action {
                self.state_act(st, false);
            } else if gif {
                self.gif_activate(self.gifs.sel, shift);
            } else {
                self.activate(self.sel, shift);
            }
        }
    }

    fn state_act(&mut self, st: &State, secondary: bool) {
        let kind = self.win.as_ref().map(|w| w.ui.get_state_kind().to_string()).unwrap_or_default();
        if secondary {
            crate::shell::ask("launch.open", serde_json::json!(["https://partner.klipy.com/api-keys"]));
            self.close(true);
            return;
        }
        match kind.as_str() {
            "noKey" | "badKey" => {
                self.close(true);
                crate::shell::ask("panels.openSettings", serde_json::json!(["ai", "GIFs"]));
            }
            "dataError" => {
                self.failed = false;
                self.load(st);
                self.push(st);
            }
            _ => {
                self.gifs.query = None;
                let q = self.query.clone();
                self.gif_fetch(st, &q);
            }
        }
    }

    // ── GIFs ──────────────────────────────────────────────────────────────

    /// Trending (q = "") or a search; keeps what's loaded for the same one
    fn gif_fetch(&mut self, st: &State, q: &str) {
        let key = klipy::key(&st.shell.cfg);
        let q = q.trim().to_string();
        if key.is_empty() {
            self.gifs.status = klipy::Status::Idle;
            self.push(st);
            return;
        }
        let g = &mut self.gifs;
        if g.query.as_deref() == Some(q.as_str()) && matches!(g.status, klipy::Status::Ready | klipy::Status::Loading) && !g.list.is_empty() {
            return;
        }
        g.query = Some(q);
        g.gen += 1;
        g.list.clear();
        g.model.set_vec(Vec::new());
        g.page = 0;
        g.has_next = false;
        g.sel = 0;
        if let Some(w) = &self.win {
            w.ui.set_gif_y(0.0);
        }
        self.gif_get(st, 1);
    }

    fn gif_more(&mut self, st: &State) {
        let g = &self.gifs;
        if !g.has_next || g.loading_more || g.status != klipy::Status::Ready {
            return;
        }
        let p = g.page + 1;
        self.gif_get(st, p);
    }

    fn gif_get(&mut self, st: &State, page: u32) {
        let key = klipy::key(&st.shell.cfg);
        let filter = st.shell.cfg["emoji"]["gifFilter"].as_str().unwrap_or("medium").to_string();
        let g = &mut self.gifs;
        g.serial += 1;
        if page == 1 {
            g.status = klipy::Status::Loading;
        } else {
            g.loading_more = true;
        }
        let url = klipy::url(&key, g.query.as_deref().unwrap_or(""), page, &self.mem.customer_id, &filter);
        let (to, serial) = (Self::send(st), g.serial);
        std::thread::spawn(move || to(Box::new(M::GifPage(serial, page, klipy::fetch(&url)))));
        self.push(st);
    }

    fn gif_page(&mut self, st: &State, serial: u64, page: u32, r: Result<(Vec<klipy::Gif>, bool), klipy::Status>) {
        if serial != self.gifs.serial {
            return; // (an older request's answer)
        }
        let g = &mut self.gifs;
        g.loading_more = false;
        match r {
            Ok((list, next)) => {
                let from = g.list.len();
                // the stills, four at a time
                let urls: Vec<(usize, String)> = list.iter().enumerate().map(|(i, x)| (from + i, x.preview.clone())).collect();
                for x in &list {
                    g.model.push(ui::GifCell { image: Default::default(), ready: false, title: x.title.as_str().into() });
                }
                g.list.extend(list);
                g.page = page;
                g.has_next = next;
                g.status = klipy::Status::Ready;
                let gen = g.gen;
                for part in urls.chunks(urls.len().div_ceil(4).max(1)) {
                    let (part, to) = (part.to_vec(), Self::send(st));
                    std::thread::spawn(move || {
                        for (i, u) in part {
                            if let Some(px) = klipy::preview(&u) {
                                to(Box::new(M::GifPreview(gen, i, px)));
                            }
                        }
                    });
                }
            }
            // a failed next page keeps what's shown
            Err(s) if page > 1 && s != klipy::Status::BadKey => g.has_next = false,
            Err(s) => g.status = s,
        }
        self.push(st);
    }

    fn gif_move(&mut self, st: &State, d: i32) {
        let n = self.gifs.list.len() as i32;
        if n == 0 {
            return;
        }
        let s = (self.gifs.sel as i32 + d).clamp(0, n - 1) as usize;
        self.gifs.sel = s;
        if s + 6 >= n as usize {
            self.gif_more(st);
        }
        // keep it in view (rows of 104 in 272)
        if let Some(w) = &self.win {
            let (top, h) = ((s / 3) as f32 * 104.0, 272.0);
            let y = -w.ui.get_gif_y();
            let ny = if top < y { top } else if top + 104.0 > y + h { top + 104.0 - h } else { y };
            w.ui.set_gif_y(-ny);
        }
        self.push(st);
    }

    fn gif_activate(&mut self, i: usize, copy_only: bool) {
        if let Some(g) = self.gifs.list.get(i) {
            let u = g.url.clone();
            self.pick(u.clone(), "gif", copy_only, &u);
        }
    }
}

impl Part for Emoji {
    fn name(&self) -> &'static str {
        NAME
    }

    /// The emoji are read now, off this thread, so the first open needn't wait
    fn start(&mut self, st: &mut State) {
        self.load(st);
    }

    fn command(&mut self, st: &mut State, cmd: &Command) {
        match cmd.cmd.as_str() {
            "open" => self.open(st, cmd.screen.clone()),
            "close" => self.close(false),
            "toggle" => {
                if self.open {
                    self.close(true)
                } else {
                    self.open(st, cmd.screen.clone())
                }
            }
            _ => {}
        }
    }

    fn config_changed(&mut self, st: &mut State, look: bool) {
        if look {
            if let Some(w) = &mut self.win {
                st.shell.apply_theme(&w.ui.global::<ui::Theme>());
                w.glass_sent.clear();
            }
        }
        self.settings(st);
    }

    fn plugin_changed(&mut self, st: &mut State) {
        if let Some(w) = &mut self.win {
            st.shell.apply_theme(&w.ui.global::<ui::Theme>());
            w.glass_sent.clear();
        }
    }

    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Loaded(d, fonts) => {
                self.loading = false;
                self.fonts = fonts;
                match d {
                    Ok(d) => {
                        self.data = Some(d);
                        self.failed = false;
                    }
                    Err(_) => self.failed = true,
                }
                self.settings(st);
                self.rebuild();
                self.push(st);
                self.open_when_ready(st);
                if self.win.is_none() && !self.open {
                    let to = st.shell.sender(NAME);
                    self.prewarm_timer.start(slint::TimerMode::SingleShot, Duration::from_secs(3), move || to(Box::new(M::Prewarm)));
                }
            }
            M::Prewarm => {
                // (only if nobody opened it meanwhile and a screen is there)
                if self.open || self.win.is_some() || self.data.is_none() {
                    return;
                }
                let Some(output) = st.outputs_now().first().map(|o| o.1.clone()) else { return };
                if !self.window(st, &output) {
                    return;
                }
                self.rebuild();
                self.push(st);
                let w = self.win.as_mut().unwrap();
                w.warmed = true;
                w.ui.set_warm(true);
                st.set_input_region(&w.surface, Some(&[]));
                w.surface.show(0, 0);
                let next = w.surface.rendered.get().wrapping_add(1);
                self.prewarm = true;
                self.warm = Some(next);
                let (to, n) = (Self::send(st), self.opening);
                self.warm_timer.start(slint::TimerMode::SingleShot, Duration::from_millis(1500), move || to(Box::new(M::WarmTimeout(n))));
            }
            M::Placed(p) => {
                self.placement = p;
                if self.open {
                    self.place_timer.stop();
                    self.show_at(st);
                }
            }
            M::PlaceTimeout(n) => {
                if self.open && n == self.opening && !self.win.as_ref().is_some_and(|w| w.ui.get_shown()) {
                    self.show_at(st);
                }
            }
            M::WarmTimeout(n) => {
                if n == self.opening {
                    self.warmed();
                }
            }
            M::Search => {
                self.query = self.field.clone();
                if self.mode() == Mode::Gif {
                    let q = self.query.clone();
                    self.gif_fetch(st, &q);
                } else {
                    self.rebuild();
                    self.scroll = 0.0;
                    if let Some(w) = &self.win {
                        w.ui.set_list_y(0.0);
                    }
                }
                self.push(st);
            }
            M::Key(t, shift, alt) => self.key(st, &t, shift, alt),
            M::Edited(t) => {
                self.field = t;
                self.sel = 0;
                self.tone = None;
                let to = Self::send(st);
                let wait = if self.mode() == Mode::Gif { 350 } else { 60 };
                self.search_timer.start(slint::TimerMode::SingleShot, Duration::from_millis(wait), move || to(Box::new(M::Search)));
                self.push(st);
            }
            M::Hovered(i) => {
                if self.keyboard_nav {
                    // (the list moved under a resting pointer: the key's choice stays)
                    self.keyboard_nav = false;
                    return;
                }
                if self.tone.is_none() && i >= 0 {
                    self.sel = i as usize;
                    self.push(st);
                }
            }
            M::Activate(i, c) => self.activate(i.max(0) as usize, c),
            M::ToneFor(i) => self.tone_for(st, i.max(0) as usize),
            M::TonePick(t, c) => self.tone_pick(t.clamp(0, 5) as u8, c),
            M::ToneClose => {
                if self.tone.take().is_some() {
                    self.push(st);
                }
            }
            M::SetTab(t) => self.set_tab(st, t.max(0) as usize),
            M::Scrolled(y) => {
                self.scroll = -y;
                if self.mode() == Mode::Emoji && !self.searching() && Instant::now() >= self.sync_until {
                    let t = self.view.tab_at(self.scroll);
                    if t >= 0 && t as usize != self.tab {
                        self.tab = t as usize;
                        if let Some(w) = &self.win {
                            w.ui.set_tab(t);
                        }
                    }
                }
            }
            M::GifHovered(i) => {
                self.gifs.sel = i.max(0) as usize;
                self.push(st);
            }
            M::GifActivate(i, c) => self.gif_activate(i.max(0) as usize, c),
            M::GifScrolled(y, h) => {
                if -y + 272.0 >= h - 104.0 {
                    self.gif_more(st);
                }
            }
            M::GifPage(s, p, r) => self.gif_page(st, s, p, r),
            M::GifPreview(gen, i, px) => {
                if gen == self.gifs.gen {
                    if let Some(mut c) = self.gifs.model.row_data(i) {
                        c.image = slint::Image::from_rgba8(px);
                        c.ready = true;
                        self.gifs.model.set_row_data(i, c);
                    }
                }
            }
            M::StateAct(s) => self.state_act(st, s),
            M::Retire(n) => self.retire(st, n),
            M::Dismissed => self.close(true),
            M::Closed => self.closed(st),
        }
    }

    fn keyboard_left(&mut self, _st: &mut State, s: &Rc<Surface>) {
        // (another layer took the keyboard: it closes, as when clicked outside)
        if self.open && self.win.as_ref().is_some_and(|w| Rc::ptr_eq(&w.surface, s)) {
            self.close(true);
        }
    }

    fn output_removed(&mut self, st: &mut State, name: &str) {
        if self.win.as_ref().is_some_and(|w| w.output == name) {
            let was = self.open;
            self.close(was);
            let w = self.win.take().unwrap();
            st.drop_surface(&w.surface);
        }
    }

    fn surface_closed(&mut self, st: &mut State, s: &Rc<Surface>) {
        if self.win.as_ref().is_some_and(|w| Rc::ptr_eq(&w.surface, s)) {
            let name = s.output.clone();
            self.output_removed(st, &name);
        }
    }

    /// The panel's glass for the plugin (Glass), as it springs open and
    /// closes; the accessibility tree
    fn after_frame(&mut self, st: &mut State) {
        let kbd = st.keyboard_surface();
        // (the unseen first frame is on screen: now it opens)
        let on_screen = |m: u32, w: &Win| w.surface.presented.get().wrapping_sub(m) <= u32::MAX / 2;
        if self.warm.zip(self.win.as_ref()).is_some_and(|(m, w)| on_screen(m, w)) {
            self.warmed();
        }
        let Some(w) = &mut self.win else { return };
        let f = kbd.as_ref().is_some_and(|k| Rc::ptr_eq(k, &w.surface));
        if let Some(a) = w.a11y.as_mut() {
            a.update(false, f);
        }
        if self.prewarm || !w.surface.mapped.get() || !st.shell.plugin || st.shell.look.material {
            return;
        }
        // (the glass comes with the first frame of it opening, never before)
        if self.open && (!w.ui.get_shown() || w.surface.presented.get().wrapping_sub(self.glass_from) > u32::MAX / 2) {
            return;
        }
        let ui = &w.ui;
        let s = ui.get_body_scale();
        let (pw, ph) = (ui.get_panel_width(), ui.get_panel_height());
        let x = ui.get_panel_x() + pw * (1.0 - s) / 2.0;
        let y = ui.get_body_y() + ph * (1.0 - s) / 2.0;
        let o = ui.get_body_opacity();
        let shape = theme::glass_shape(&st.shell.look, x, y, pw * s, ph * s, 22.0 * s, o, 1.0);
        let key = format!("{shape:?}");
        if key != w.glass_sent {
            st.shell.glass.set(&w.output, NS, if o > 0.002 { std::slice::from_ref(&shape) } else { &[] });
            w.glass_sent = key;
        }
    }

    fn a11y(&mut self) -> Vec<&mut crate::a11y::A11y> {
        self.win.as_mut().and_then(|w| w.a11y.as_mut()).into_iter().collect()
    }

    fn stats(&self) {
        if let Some(w) = &self.win {
            eprintln!("tessera-ui: stats {}", w.surface.stats.borrow_mut().report("emoji"));
        }
    }

    fn quit(&mut self, _: &mut State) {
        if let Some(w) = &self.win {
            Glass::clear_now(&w.output, NS);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const SCREEN: (f64, f64, f64, f64) = (0.0, 0.0, 1920.0, 1080.0);

    #[test]
    fn opens_by_the_pointer_in_the_focused_window() {
        let w = Some((100.0, 100.0, 1200.0, 800.0, "0xabc".to_string()));
        let p = Placement { pointer: Some((400.0, 300.0)), window: w.clone() };
        assert_eq!(place(SCREEN, &p, (420.0, 428.0)), (408.0, 320.0));
        // no room right or below: flipped to the other side
        let p = Placement { pointer: Some((1700.0, 850.0)), window: Some((0.0, 0.0, 1920.0, 1080.0, "a".into())) };
        assert_eq!(place(SCREEN, &p, (420.0, 428.0)), (1272.0, 414.0));
        // the pointer outside the window: centred in the window
        let p = Placement { pointer: Some((1500.0, 1000.0)), window: w };
        assert_eq!(place(SCREEN, &p, (420.0, 428.0)), (490.0, 286.0));
        // nothing known: the middle of the screen
        assert_eq!(place(SCREEN, &Placement::default(), (420.0, 428.0)), (750.0, 326.0));
        // a window on another screen doesn't count; on this one, its own coordinates
        let p = Placement { pointer: Some((2500.0, 300.0)), window: Some((2000.0, 0.0, 800.0, 600.0, "b".into())) };
        assert_eq!(place(SCREEN, &p, (420.0, 428.0)), (750.0, 326.0));
        assert_eq!(place((1920.0, 0.0, 1920.0, 1080.0), &p, (420.0, 428.0)), (588.0, 20.0 + 300.0));
    }

    #[test]
    fn keys_it_takes_from_the_field() {
        assert!(handles(&k(Key::Return)) && handles(&k(Key::Escape)) && handles(&k(Key::Backtab)));
        assert!(!handles("a") && !handles(&k(Key::Backspace)) && !handles(&k(Key::Home)));
    }

    #[test]
    fn key_caps_follow_settings() {
        assert_eq!(key_names(&json!({})), (vec!["Enter".to_string()], vec!["Shift".to_string(), "Enter".to_string()]));
        assert_eq!(key_names(&json!({"appearance": {"menuIcon": "command"}})).1, vec!["⇧", "↩"]);
        assert_eq!(key_names(&json!({"appearance": {"keySymbols": "windows", "menuIcon": "command"}})).0, vec!["Enter"]);
    }

    #[test]
    fn the_emoji_style_falls_back_to_noto() {
        let fonts = vec!["Noto Color Emoji".to_string(), "Apple Color Emoji".to_string()];
        assert_eq!(emoji_font(&json!({"emoji": {"style": "apple"}}), &fonts), "Apple Color Emoji");
        assert_eq!(emoji_font(&json!({"emoji": {"style": "windows"}}), &fonts), "Noto Color Emoji");
        assert_eq!(emoji_font(&json!({}), &fonts), "Noto Color Emoji");
    }

    #[test]
    fn the_footer_names_the_selection() {
        let d = data::tests::sample();
        let mut mem = Remembered::default();
        let wave = view::Cell { kind: Kind::Emoji, i: 3 };
        assert_eq!(footer_words(&d, &mem, wave), ("👋".into(), "Waving hand".into(), ":wave:  ·  right-click for skin tones".into()));
        mem.set_tone(&d.emoji[3], 2);
        assert_eq!(footer_words(&d, &mem, wave).1, "Waving hand, medium-light skin tone");
        assert_eq!(footer_words(&d, &mem, wave).0, "👋\u{1F3FC}");
        assert_eq!(footer_words(&d, &mem, view::Cell { kind: Kind::Kaomoji, i: 0 }).1, "Happy");
        assert_eq!(footer_words(&d, &mem, view::Cell { kind: Kind::Symbol, i: 0 }), ("→".into(), "Rightwards arrow".into(), "U+2192".into()));
    }
}

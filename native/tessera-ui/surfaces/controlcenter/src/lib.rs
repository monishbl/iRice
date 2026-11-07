//! Control Center (rust.surfaces: controlcenter), opened as the QML one is
//! through Panels ("controlCenter": its shortcut, the menu bar, IPC). Glass:
//! the module grid (ControlCenterGlass.qml, ModuleView.qml): 1×1 round
//! buttons, 2×1 pills, 2×2 cards, the capsule sliders, Now Playing, readings,
//! each its own glass chip unless the panel has a background; detail pages
//! in a second popover beside it; the footer (you, the battery, Settings,
//! Power, Edit controls) and Edit controls with the gallery beside the grid.
//! Material: Quick Settings (QuickSettings.qml, QSTile.qml): the same layout
//! in Android 16's tiles, the tall sliders, the media card, the press bounce
//! (the pressed tile widens, its row-mates give), the tiles rising in one
//! after another as it opens, and pages sliding in inside the panel.
//!
//! What it shows: the layout from config.json (`controlCenter.layout`, defs.rs);
//! the sliders, the Microphone module and the Sound and Microphone pages from
//! the sound server (services::audio) and the backlight (services::brightness);
//! the battery and Wi-Fi's state from services::status, Bluetooth's from
//! services::bluez, Now Playing from services::mpris (as the menu bar). Everything else is
//! still the shell's own state (Focus, Sharing, Night Light, Caffeine,
//! Bluetooth and the Wi-Fi networks, the player…): its RustBridge.qml sends
//! it on stdin while this is open and does what a click asks (bridge.rs).
//! The open page and Edit controls stay in step with Panels.ccPage and
//! Panels.ccEditing both ways, so the menu bar's Focus or Caffeine items still
//! open their pages here.
//!
//! Not like the QML yet: the controls a page dims don't blur (Slint has no
//! blur: they fade further instead); Material draws Material Symbols
//! outlined (Slint can't set the font's FILL axis) and your picture in a
//! circle, not a cookie; the media card shows one player (no carousel), a
//! flat position line (not the wavy one) and only local album art; an app's
//! volume row has no output chooser and a generic icon, and outputs show
//! icons, not their devices' pictures; CPU & Memory's page lists readings,
//! not graphs; Wi-Fi's enterprise sign-in form and the share-network QR card
//! open Settings → Network instead, and its rows don't hold still under the
//! pointer between rescans; Glass's panel doesn't give when pressed and
//! dragged (ElasticGrab) and its motion setting (snappy, expressive) isn't
//! followed; a click on the menu bar strip while it's open reaches the bar
//! (the strip is left out of its input), not "the item under it".

pub mod ui {
    slint::include_modules!();
}
tessera_core::theme_global!(ui);

// tessera-core's modules (shell, wayland, theme, services, util…) under the
// names they had when everything was one crate: `crate::shell::Part`
#[allow(unused_imports)]
use tessera_core::*;

pub mod bridge;
pub mod defs;
pub mod icons;

use tessera_core::services::audio::{self, Cmd, Sound};
use tessera_core::services::bluez::Bluetooth;
use tessera_core::services::brightness::{self, Backlight};
use tessera_core::services::mpris::{self, Players};
use tessera_core::services::hypr::Glass;
use tessera_core::services::status::{Battery, Wifi};
use tessera_core::shell::{self, Command, Part};
use tessera_core::theme;
use tessera_core::wayland::{Spec, State, Surface};
use bridge::{Block, BlockKind, MenuItem, Page, Said};
use defs::{Entry, Kind, Placed};
use serde_json::{json, Value};
use slint::platform::Key;
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use smithay_client_toolkit::shell::WaylandSurface;
use std::any::{Any, TypeId};
use std::rc::Rc;
use std::time::{Duration, Instant};

const NAME: &str = "controlcenter";
pub const NS: &str = "tessera:controlcenter";
/// Glass: 320 wide, four columns 10 apart; Material: 368, 8 apart, rows 72
const GLASS_W: f32 = 320.0;
const GLASS_GAP: f32 = 10.0;
const M3_W: f32 = 368.0;
const M3_GAP: f32 = 8.0;
const M3_ROW: f32 = 72.0;
const PAD: f32 = 16.0;

enum M {
    Act(i32),
    OpenPage(i32, f32),
    Secondary(i32, f32, f32),
    Remove(i32),
    Resize(i32),
    Drop(i32, f32, f32),
    Slide(i32, f32),
    Press(i32, bool),
    Media(String),
    Add(String),
    Reset,
    ToggleEditing,
    Settings,
    Power,
    Back,
    PageSwitch,
    PageRefresh,
    Block(i32, String, String),
    BlockMenu(i32, f32, f32),
    BlockSlide(i32, f32),
    MenuPick(i32),
    MenuClose,
    Dismissed,
    Key(String, bool),
    Closed,
    /// the album art for this address, read on a thread
    Art(String, Option<slint::SharedPixelBuffer<slint::Rgba8Pixel>>),
}

/// Whose choices the open menu shows
#[derive(Clone, Debug, PartialEq)]
enum MenuOf {
    Module(String),
    Block(String, String),
}

// ── Geometry ─────────────────────────────────────────────────────────────

/// A packed module's place in the grid (x, y, w, h)
pub fn place(p: &Placed, material: bool) -> (f32, f32, f32, f32) {
    let (gap, unit, row) = cell(material);
    let (c, r, w, h) = (p.col as f32, p.row as f32, p.w as f32, p.h as f32);
    (c * (unit + gap), r * (row + gap), w * unit + (w - 1.0) * gap, h * row + (h - 1.0) * gap)
}

/// (gap, column width, row height)
pub fn cell(material: bool) -> (f32, f32, f32) {
    if material {
        (M3_GAP, (M3_W - 3.0 * M3_GAP) / 4.0, M3_ROW)
    } else {
        let u = (GLASS_W - 3.0 * GLASS_GAP) / 4.0;
        (GLASS_GAP, u, u)
    }
}

pub fn grid_height(rows: u8, material: bool) -> f32 {
    let (gap, _, row) = cell(material);
    if rows == 0 {
        0.0
    } else {
        rows as f32 * row + (rows as f32 - 1.0) * gap
    }
}

/// Glass's chip radius for a module (ModuleView's shapeRadius)
pub fn chip_radius(kind: Kind, cols: u8, rows: u8, w: f32, h: f32) -> f32 {
    let round = cols == 1 && rows == 1 && matches!(kind, Kind::Toggle | Kind::Action);
    let pill = rows == 1 && cols == 2 && matches!(kind, Kind::Toggle | Kind::Action | Kind::Info);
    let unit = (GLASS_W - 3.0 * GLASS_GAP) / 4.0;
    if round || pill || kind == Kind::Slider {
        w.min(h) / 2.0
    } else {
        (unit / 2.0).min(w.min(h) / 2.0)
    }
}

/// Audio.icon's name: muted or silent, then by thirds
pub fn volume_icon(v: f64, muted: bool) -> &'static str {
    if muted || v == 0.0 {
        "speaker-x"
    } else if v < 0.34 {
        "speaker-none"
    } else if v < 0.67 {
        "speaker-low"
    } else {
        "speaker-high"
    }
}

/// Glass's on-white glyph (Theme.accentOnGlyph: the accent when it reads at
/// 3:1 on white, else near-black) and its accent text on the panel
/// (Theme.accentInk: 4.5:1, else near-black / near-white)
pub fn accent_on_white(accent: slint::Color) -> slint::Color {
    if theme::contrast_ratio(accent, slint::Color::from_rgb_u8(255, 255, 255)) >= 3.0 {
        accent
    } else {
        slint::Color::from_rgb_u8(0x1c, 0x1c, 0x1e)
    }
}

pub fn accent_ink(accent: slint::Color, dark: bool) -> slint::Color {
    let bg = if dark { slint::Color::from_rgb_u8(0x1e, 0x1e, 0x21) } else { slint::Color::from_rgb_u8(0xf5, 0xf5, 0xf7) };
    if theme::contrast_ratio(accent, bg) >= 4.5 {
        accent
    } else if dark {
        // (the ramp's nearest lighter shade that reads, approximately)
        let mut c = accent;
        for _ in 0..10 {
            c = c.mix(&slint::Color::from_rgb_u8(255, 255, 255), 0.85);
            if theme::contrast_ratio(c, bg) >= 4.5 {
                return c;
            }
        }
        slint::Color::from_rgb_u8(0xed, 0xed, 0xef)
    } else {
        slint::Color::from_rgb_u8(0x1c, 0x1c, 0x1e)
    }
}

/// The keys it takes (the rest go nowhere)
pub fn handles(text: &str) -> bool {
    let k = |key: Key| text == SharedString::from(key).as_str();
    k(Key::Escape) || k(Key::Tab) || k(Key::Backtab) || k(Key::Return) || k(Key::UpArrow) || k(Key::DownArrow) || k(Key::LeftArrow) || k(Key::RightArrow) || k(Key::Delete) || k(Key::Backspace) || k(Key::PageUp) || k(Key::PageDown) || k(Key::Home) || k(Key::End) || text == " " || k(Key::Menu)
}

// ── The part ─────────────────────────────────────────────────────────────

struct Win {
    output: String,
    surface: Rc<Surface>,
    ui: ui::ControlCenter,
    a11y: Option<crate::a11y::A11y>,
    glass_sent: String,
    region: (u32, u32),
}

pub struct ControlCenter {
    win: Option<Win>,
    open: bool,
    said: Said,
    /// the page open ("" none) and the module that opened it
    page: String,
    from: String,
    sub_y: f32,
    /// the open page's content from the shell (pages not drawn here)
    shell_page: Page,
    /// what's drawn now (the shell's, or Sound's / Microphone's from here)
    shown_page: Page,
    editing: bool,
    saved: Vec<String>,
    layout: Vec<Entry>,
    packed: Vec<Placed>,
    model: Rc<VecModel<ui::CcMod>>,
    menu: Option<(MenuOf, Vec<MenuItem>)>,
    kbd: i32,
    /// the page's row the keyboard is on (-1: none)
    pkbd: i32,
    /// the album art shown, by its address
    art_url: String,
    art: Option<slint::Image>,
    hushed: Option<Instant>,
}

impl Default for ControlCenter {
    fn default() -> Self {
        Self {
            win: None,
            open: false,
            said: Said::default(),
            page: String::new(),
            from: String::new(),
            sub_y: 0.0,
            shell_page: Page::default(),
            shown_page: Page::default(),
            editing: false,
            saved: Vec::new(),
            layout: Vec::new(),
            packed: Vec::new(),
            model: Rc::new(VecModel::default()),
            menu: None,
            kbd: -1,
            pkbd: -1,
            art_url: String::new(),
            art: None,
            hushed: None,
        }
    }
}

fn ic(name: &str, material: bool) -> ui::CcIc {
    let (t, s) = icons::glyph(name, material);
    ui::CcIc { t: t.into(), s }
}

fn glyphs(material: bool) -> ui::CcGlyphs {
    let i = |n: &str| ic(n, material);
    ui::CcGlyphs {
        back: i("caret-left"),
        refresh: i("arrow-clockwise"),
        plus: i("plus"),
        plus_circle: i("plus-circle"),
        gear: i("gear-six"),
        power: i("power"),
        pencil: i("pencil-simple"),
        prev: i("skip-back"),
        next: i("skip-forward"),
        play: i("play"),
        pause: i("pause"),
        notes: i("music-notes"),
        user: i("user"),
        more: i("sliders-horizontal"),
        check: i("check"),
        cast: i("screencast"),
        caret_right: i("caret-right"),
        caret_down: i("caret-down"),
        spinner: ic("circle-notch", false),
        battery: i("battery-high"),
        battery_charging: i("battery-charging"),
        battery_low: i("battery-low"),
        speaker_x: i("speaker-x"),
        bolt: ic("lightning", false),
    }
}

/// A picture file as an image (the device drawings, ~/.face, album art), or None
pub fn picture(path: &str) -> Option<slint::Image> {
    picture_px(path).map(slint::Image::from_rgba8)
}

fn picture_px(path: &str) -> Option<slint::SharedPixelBuffer<slint::Rgba8Pixel>> {
    if path.is_empty() || path.contains("://") {
        return None;
    }
    // (the device drawings, shell/assets/devices, are SVG)
    if path.ends_with(".svg") {
        use resvg::{tiny_skia, usvg};
        let tree = usvg::Tree::from_data(&std::fs::read(path).ok()?, &usvg::Options::default()).ok()?;
        let size = 96u32;
        let (w, h) = (tree.size().width(), tree.size().height());
        let k = size as f32 / w.max(h).max(1.0);
        let mut pix = tiny_skia::Pixmap::new(size, size)?;
        resvg::render(&tree, tiny_skia::Transform::from_scale(k, k).post_translate((size as f32 - w * k) / 2.0, (size as f32 - h * k) / 2.0), &mut pix.as_mut());
        let raw: Vec<u8> = pix.pixels().iter().flat_map(|p| {
            let c = p.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        }).collect();
        return Some(slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(&raw, size, size));
    }
    // (~/.face has no extension: the format from its bytes)
    let img = image::ImageReader::open(path).ok()?.with_guessed_format().ok()?.decode().ok()?.thumbnail(192, 192).to_rgba8();
    let (w, h) = img.dimensions();
    Some(slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(img.as_raw(), w, h))
}

impl ControlCenter {
    fn material(st: &State) -> bool {
        st.shell.look.material
    }

    fn usable(&self, id: &str, st: &State) -> bool {
        match id {
            // (the shell knows external displays too; until it says: the backlight)
            "display" => self.said.modules.get(id).map_or_else(|| st.shell.get::<Backlight>().is_some_and(|b| b.present), |m| m.usable),
            "micLevel" => st.shell.get::<Sound>().map_or_else(|| self.said.usable(id), |s| !s.sources.is_empty()),
            _ => self.said.usable(id),
        }
    }

    fn window(&mut self, st: &mut State, output: &str) -> bool {
        if self.win.as_ref().is_some_and(|w| w.output != output) {
            let w = self.win.take().unwrap();
            Glass::clear_now(&w.output, NS);
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
        let ui = ui::ControlCenter::new().expect("ControlCenter");
        st.shell.apply_theme(&ui.global::<ui::Theme>());
        let to = st.shell.sender(NAME);
        ui.on_key(move |text, shift| {
            let ok = handles(&text);
            if ok {
                to(Box::new(M::Key(text.to_string(), shift)));
            }
            ok
        });
        let to = st.shell.sender(NAME);
        macro_rules! on {
            ($set:ident, |$($a:ident),*| $m:expr) => {{
                let to = to.clone();
                ui.$set(move |$($a),*| to(Box::new($m)));
            }};
        }
        on!(on_act, |i| M::Act(i));
        on!(on_open_page, |i, y| M::OpenPage(i, y));
        on!(on_secondary, |i, x, y| M::Secondary(i, x, y));
        on!(on_remove, |i| M::Remove(i));
        on!(on_resize, |i| M::Resize(i));
        on!(on_dropped, |i, x, y| M::Drop(i, x, y));
        on!(on_slide, |i, v| M::Slide(i, v));
        on!(on_press, |i, d| M::Press(i, d));
        on!(on_media_do, |w| M::Media(w.to_string()));
        on!(on_add, |id| M::Add(id.to_string()));
        on!(on_reset_layout, | | M::Reset);
        on!(on_toggle_editing, | | M::ToggleEditing);
        on!(on_settings, | | M::Settings);
        on!(on_power, | | M::Power);
        on!(on_back, | | M::Back);
        on!(on_page_switched, | | M::PageSwitch);
        on!(on_page_refreshed, | | M::PageRefresh);
        on!(on_block, |i, w, t| M::Block(i, w.to_string(), t.to_string()));
        on!(on_block_menu, |i, x, y| M::BlockMenu(i, x, y));
        on!(on_block_slide, |i, v| M::BlockSlide(i, v));
        on!(on_menu_pick, |i| M::MenuPick(i));
        on!(on_menu_close, | | M::MenuClose);
        on!(on_dismissed, | | M::Dismissed);
        on!(on_closed, | | M::Closed);
        ui.set_modules(ModelRc::from(self.model.clone()));
        ui.show().expect("show controlcenter");
        let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
        self.win = Some(Win { output: output.into(), surface, ui, a11y, glass_sent: String::new(), region: (0, 0) });
        self.model.set_vec(Vec::new());
        self.apply_look(st);
        true
    }

    fn apply_look(&self, st: &State) {
        let Some(w) = &self.win else { return };
        let ui = &w.ui;
        st.shell.apply_theme(&ui.global::<ui::Theme>());
        let m = Self::material(st);
        ui.set_g(glyphs(m));
        let accent = ui.global::<ui::Theme>().get_accent();
        ui.set_accent_glyph(if m { ui.global::<ui::Theme>().get_m3_on_primary() } else { accent_on_white(accent) });
        ui.set_accent_ink(if m { ui.global::<ui::Theme>().get_m3_primary() } else { accent_ink(accent, st.shell.look.dark) });
        ui.set_bare(!m && defs::bare(&st.shell.cfg));
        ui.set_bounce(m && !st.shell.cfg["appearance"]["reduceMotion"].as_bool().unwrap_or(false));
    }

    fn open(&mut self, st: &mut State, screen: Option<String>) {
        let output = screen.or_else(|| self.win.as_ref().map(|w| w.output.clone())).or_else(|| st.outputs_now().first().map(|o| o.1.clone()));
        let Some(output) = output else { return };
        let reopen = self.open && self.win.as_ref().is_some_and(|w| w.output == output);
        if !self.window(st, &output) {
            return;
        }
        for s in ["audio", "brightness", "status", "mpris", "bluez"] {
            st.shell.want(s);
        }
        if reopen {
            return;
        }
        self.open = true;
        self.menu = None;
        self.set_kbd(-1);
        self.model.set_vec(Vec::new()); // (the tiles rise in afresh)
        self.rebuild(st);
        self.push_page(st);
        self.push_avatar();
        self.want_art(st);
        self.push_footer(st);
        let w = self.win.as_ref().unwrap();
        w.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        w.surface.show(0, 0);
        w.ui.invoke_focus_keys();
        w.ui.set_shown(true);
    }

    fn close(&mut self, tell: bool) {
        if !self.open {
            return;
        }
        self.open = false;
        self.menu = None;
        self.page.clear();
        self.from.clear();
        self.editing = false;
        if let Some(w) = &self.win {
            w.ui.set_shown(false);
            w.ui.set_menu_open(false);
            w.ui.set_paging(false);
            w.ui.set_editing(false);
            w.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::None);
            w.surface.layer.commit();
        }
        if tell {
            shell::closed(NAME);
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
            w.region = (0, 0);
        }
    }

    /// The layout from config.json, packed, into the model (rows kept by
    /// id, so a change glides instead of rebuilding)
    fn rebuild(&mut self, st: &State) {
        if self.saved.is_empty() {
            self.saved = defs::saved(&st.shell.cfg);
        }
        self.layout = {
            let usable = |id: &str| self.usable(id, st);
            defs::layout(&self.saved, &usable)
        };
        self.packed = defs::pack(&self.layout, 4);
        let material = Self::material(st);
        let rows: Vec<ui::CcMod> = self.packed.iter().enumerate().map(|(i, p)| self.module(st, p, i, material)).collect();
        // keep rows whose id stays; drop the gone; add the new at the end
        let keep: Vec<String> = rows.iter().map(|r| r.id.to_string()).collect();
        let mut i = 0;
        while i < self.model.row_count() {
            if keep.contains(&self.model.row_data(i).unwrap().id.to_string()) {
                i += 1;
            } else {
                self.model.remove(i);
            }
        }
        for r in rows {
            match (0..self.model.row_count()).find(|&k| self.model.row_data(k).unwrap().id == r.id) {
                Some(k) => {
                    if self.model.row_data(k).as_ref() != Some(&r) {
                        self.model.set_row_data(k, r);
                    }
                }
                None => self.model.push(r),
            }
        }
        let Some(w) = &self.win else { return };
        let ui = &w.ui;
        ui.set_grid_height(grid_height(defs::rows_used(&self.packed), material));
        ui.set_editing(self.editing);
        let usable = |id: &str| self.usable(id, st);
        let gal: Vec<ui::CcGal> = defs::gallery(&self.layout, &usable)
            .into_iter()
            .map(|id| {
                let d = defs::def(id).unwrap();
                let m = self.said.modules.get(id);
                let why = if usable(id) { String::new() } else { m.map(|m| m.why.clone()).filter(|w| !w.is_empty()).unwrap_or_else(|| "Not available on this computer".into()) };
                ui::CcGal {
                    id: id.into(),
                    label: m.map(|m| m.label.clone()).filter(|l| !l.is_empty()).unwrap_or_else(|| d.label.into()).into(),
                    icon: ic(m.map(|m| m.icon.as_str()).filter(|l| !l.is_empty()).unwrap_or(d.icon), material),
                    why: why.into(),
                    sizes: d.sizes.iter().map(|(w, h)| format!("{w}×{h}")).collect::<Vec<_>>().join(" · ").into(),
                }
            })
            .collect();
        ui.set_gallery(ModelRc::new(VecModel::from(gal)));
        self.push_footer(st);
    }

    /// One module's row: where it goes and what it says now
    fn module(&self, st: &State, p: &Placed, order: usize, material: bool) -> ui::CcMod {
        let d = defs::def(&p.id).unwrap();
        let said = self.said.modules.get(&p.id).cloned().unwrap_or_default();
        let (x, y, w, h) = place(p, material);
        let mut label = if said.label.is_empty() { d.label.to_string() } else { said.label.clone() };
        let mut icon = if said.icon.is_empty() { d.icon.to_string() } else { said.icon.clone() };
        let mut on = said.on;
        let mut status = said.status.clone();
        let (mut title, mut sub) = (said.title.clone(), said.sub.clone());
        let mut value = 0.0f64;
        let snd = st.shell.get::<Sound>();
        match p.id.as_str() {
            "wifi" => {
                if let Some(wf) = st.shell.get::<Wifi>().filter(|w| w.present) {
                    on = wf.enabled;
                    icon = if wf.enabled { "wifi-high" } else { "wifi-slash" }.into();
                    status = if !wf.enabled { "Off".into() } else if wf.ssid.is_empty() { "Not connected".into() } else { wf.ssid.clone() };
                }
            }
            "bluetooth" => {
                // (BlueZ, the menu bar's service; the shell's own words when
                // Bluetooth doesn't work: blocked, stopped)
                let why = ["Off", "On"].contains(&status.as_str()) || st.shell.get::<Bluetooth>().is_some_and(|b| b.devices.iter().any(|d| d.connected && d.name == status));
                if let Some(b) = st.shell.get::<Bluetooth>().filter(|b| b.present && (why || status.is_empty())) {
                    on = b.powered;
                    status = if !b.powered { "Off".into() } else { b.devices.iter().find(|d| d.connected).map_or_else(|| "On".into(), |d| d.name.clone()) };
                }
            }
            "mic" => {
                if let Some(s) = snd {
                    on = !s.mic_muted();
                    icon = if s.mic_muted() { "microphone-slash" } else { "microphone" }.into();
                    status = if s.mic_muted() { "Muted" } else { "On" }.into();
                }
            }
            "display" => value = st.shell.get::<Backlight>().map_or(0.0, |b| b.value),
            "sound" => {
                if let Some(s) = snd {
                    value = if s.muted() { 0.0 } else { s.volume() };
                    icon = volume_icon(s.volume(), s.muted()).into();
                }
            }
            "micLevel" => {
                if let Some(s) = snd {
                    value = if s.mic_muted() { 0.0 } else { s.mic_volume() };
                    icon = if s.mic_muted() { "microphone-slash" } else { "microphone" }.into();
                }
            }
            "battery" => {
                if let Some(b) = st.shell.get::<Battery>() {
                    title = format!("{}%", b.percent.round());
                    sub = if b.charging { "Charging" } else { "On battery" }.into();
                    icon = if b.charging { "battery-charging" } else if b.percent < 20.0 { "battery-low" } else { "battery-high" }.into();
                }
            }
            _ => {}
        }
        if d.kind == Kind::Info && title.is_empty() {
            title = "—".into();
        }
        if label.is_empty() {
            label = p.id.clone();
        }
        ui::CcMod {
            id: p.id.as_str().into(),
            kind: d.kind.code(),
            cols: p.w as i32,
            rows: p.h as i32,
            x,
            y,
            w,
            h,
            sq_dx: 0.0,
            sq_dw: 0.0,
            label: label.into(),
            status: status.into(),
            icon: ic(&icon, material),
            on,
            page: !d.page.is_empty(),
            menu: d.menu && !said.menu.is_empty(),
            value: value as f32,
            title: title.into(),
            sub: sub.into(),
            order: order as i32,
            focused: !self.page.is_empty() && self.from == p.id,
        }
    }

    /// Only what the sound server, backlight or battery changed
    fn refresh_levels(&mut self, st: &State) {
        if !self.open {
            return;
        }
        let material = Self::material(st);
        for k in 0..self.model.row_count() {
            let r = self.model.row_data(k).unwrap();
            let Some((i, p)) = self.packed.iter().enumerate().find(|(_, p)| p.id == r.id.as_str()) else { continue };
            let mut n = self.module(st, p, i, material);
            n.sq_dx = r.sq_dx;
            n.sq_dw = r.sq_dw;
            if n != r {
                self.model.set_row_data(k, n);
            }
        }
        self.push_footer(st);
        if bridge::local(&self.page) {
            self.push_page(st);
        }
    }

    fn push_footer(&self, st: &State) {
        let Some(w) = &self.win else { return };
        let ui = &w.ui;
        let b = st.shell.get::<Battery>().cloned().unwrap_or_default();
        ui.set_battery(b.present);
        ui.set_battery_level((b.percent / 100.0) as f32);
        ui.set_charging(b.charging);
        ui.set_user(self.said.user.as_str().into());
        let m = &self.said.media;
        let p = st.shell.get::<Players>().and_then(|p| p.chosen());
        let art = self.art.clone();
        ui.set_media(ui::CcMedia {
            has: p.is_some() || m.has,
            identity: p.map_or(m.identity.as_str(), |p| p.identity.as_str()).into(),
            title: p.map_or(m.title.as_str(), |p| p.track.title.as_str()).into(),
            artist: p.map_or(m.artist.as_str(), |p| p.track.artist.as_str()).into(),
            playing: p.map_or(m.playing, |p| p.playing),
            has_art: art.is_some(),
            art: art.unwrap_or_default(),
            prev: p.map_or(m.can_prev, |p| p.can_prev),
            next: p.map_or(m.can_next, |p| p.can_next),
            toggle: p.map_or(m.can_toggle, |p| p.can_play),
            seek: m.can_seek,
            progress: if m.length > 0.0 { (m.position / m.length).clamp(0.0, 1.0) as f32 } else { 0.0 },
            timed: m.length > 0.0,
        });
        ui.set_cast(self.said.cast);
        ui.set_casting(self.said.casting);
    }

    /// The album art for the shown player (fetched once into the cache on a
    /// thread when it's on the web: mpris::art_file)
    fn want_art(&mut self, st: &State) {
        let url = st.shell.get::<Players>().and_then(|p| p.chosen()).map(|p| p.track.art_url.clone()).filter(|u| !u.is_empty()).unwrap_or_else(|| self.said.media.art.clone());
        if url == self.art_url {
            return;
        }
        self.art_url = url.clone();
        self.art = None;
        if url.is_empty() {
            return;
        }
        let to = st.shell.sender(NAME);
        std::thread::spawn(move || {
            let px = if url.contains("://") { mpris::art_file(&url) } else { Some(std::path::PathBuf::from(&url)) }.and_then(|p| picture_px(&p.to_string_lossy()));
            to(Box::new(M::Art(url, px)));
        });
    }

    fn push_avatar(&self) {
        let Some(w) = &self.win else { return };
        let a = picture(&self.said.avatar);
        w.ui.set_has_avatar(a.is_some());
        w.ui.set_avatar(a.unwrap_or_default());
    }

    /// The open page into the window: Sound and Microphone from here, the
    /// rest as the shell last sent them
    fn push_page(&mut self, st: &State) {
        let page = if self.page == "sound" || self.page == "mic" {
            let snd = st.shell.get::<Sound>().cloned().unwrap_or_default();
            let mic_in = st.shell.cfg["controlCenter"]["micInSound"].as_bool().unwrap_or(true);
            if self.page == "sound" {
                bridge::sound_page(&snd, &self.said.profiles, mic_in)
            } else {
                bridge::mic_page(&snd)
            }
        } else if self.shell_page.name == self.page {
            self.shell_page.clone()
        } else {
            Page { name: self.page.clone(), title: page_title(&self.page).into(), ..Default::default() }
        };
        self.shown_page = page;
        let Some(w) = &self.win else { return };
        let ui = &w.ui;
        let material = Self::material(st);
        let p = &self.shown_page;
        let groups = bridge::groups(&p.blocks);
        let t = ui.global::<ui::Theme>();
        let fonts = (t.get_caption(), t.get_small());
        let blocks: Vec<ui::CcBlock> = p.blocks.iter().zip(groups).map(|(b, (first, last))| block(b, first, last, material, fonts)).collect();
        // (Wi-Fi's and Bluetooth's lists scroll past 360, as their QML lists)
        ui.set_page_cap(if self.page == "wifi" || self.page == "bluetooth" { 360.0 } else { 4000.0 });
        ui.set_paging(!self.page.is_empty());
        ui.set_page_title(p.title.as_str().into());
        ui.set_page_switch(p.switch);
        ui.set_page_switch_on(p.switch_on);
        ui.set_page_switch_label(p.switch_label.as_str().into());
        ui.set_page_refresh(p.refresh);
        ui.set_blocks(ModelRc::new(VecModel::from(blocks)));
        ui.set_sub_y(self.sub_y);
    }

    fn set_page(&mut self, st: &State, page: &str, from: &str, y: f32, tell: bool) {
        if self.page == page {
            return;
        }
        self.page = page.to_string();
        self.set_pkbd(-1);
        self.from = if page.is_empty() { String::new() } else if !from.is_empty() { from.to_string() } else { self.layout.iter().find(|e| defs::def(&e.id).is_some_and(|d| d.page == page)).map(|e| e.id.clone()).unwrap_or_default() };
        // (level with the module that opened it; opened from elsewhere, the
        // menu bar or IPC: at the panel's top, as the QML's)
        if !page.is_empty() {
            self.sub_y = y.max(0.0);
        }
        self.menu = None;
        if tell {
            shell::ask("cc.page", json!([page]));
        }
        self.rebuild(st);
        self.push_page(st);
    }

    /// A module's top on screen (the page's popover opens level with it)
    fn module_y_of(&self, st: &State, id: &str) -> f32 {
        let Some(p) = self.packed.iter().find(|p| p.id == id) else { return 0.0 };
        34.0 + PAD + place(p, Self::material(st)).1
    }

    fn set_editing(&mut self, st: &State, on: bool, tell: bool) {
        if self.editing == on {
            return;
        }
        self.editing = on;
        if on {
            self.set_page(st, "", "", 0.0, tell);
        }
        if tell {
            shell::ask("cc.editing", json!([on]));
        }
        self.rebuild(st);
    }

    fn save(&mut self, st: &State, entries: Vec<Entry>) {
        let usable = |id: &str| self.usable(id, st);
        self.saved = defs::save(&entries, &self.saved, &usable);
        shell::ask("cc.layout", json!([self.saved]));
        self.rebuild(st);
    }

    fn id_at(&self, i: i32) -> Option<String> {
        (i >= 0).then(|| self.model.row_data(i as usize).map(|r| r.id.to_string())).flatten()
    }

    fn hush(&mut self, st: &mut State, what: &str) {
        let v = json!({"to": "osd", "hush": what});
        st.part("osd", |p, st| p.message(st, Box::new(v)));
        // (the QML OSD's own quiet window; once a second is enough during a drag)
        if self.hushed.is_none_or(|t| t.elapsed() > Duration::from_millis(600)) {
            shell::ask(if what == "brightness" { "brightness.hush" } else { "audio.hush" }, json!([]));
            self.hushed = Some(Instant::now());
        }
    }

    fn slide(&mut self, st: &mut State, id: &str, v: f32) {
        let v = v.clamp(0.0, 1.0) as f64;
        let snd = st.shell.get::<Sound>().cloned().unwrap_or_default();
        match id {
            "display" => {
                self.hush(st, "brightness");
                brightness::set(v);
            }
            "sound" => {
                self.hush(st, "sound");
                if let Some(d) = snd.sink() {
                    if d.muted {
                        audio::send(Cmd::SinkMute { index: d.index, muted: false });
                    }
                    audio::send(Cmd::SinkVolume { index: d.index, channels: d.channels, volume: v });
                }
            }
            "micLevel" | "mic" => {
                self.hush(st, "sound");
                if let Some(d) = snd.source() {
                    if d.muted {
                        audio::send(Cmd::SourceMute { index: d.index, muted: false });
                    }
                    audio::send(Cmd::SourceVolume { index: d.index, channels: d.channels, volume: v });
                }
            }
            _ => {}
        }
    }

    /// A module's own action (a toggle switches, an action runs)
    fn act(&mut self, st: &mut State, id: &str) {
        if self.editing {
            let l = defs::cycle(&self.layout, id);
            self.save(st, l);
            return;
        }
        match id {
            // (the microphone is the sound server's: here)
            "mic" => {
                let snd = st.shell.get::<Sound>().cloned().unwrap_or_default();
                self.hush(st, "sound");
                if let Some(d) = snd.source() {
                    audio::send(Cmd::SourceMute { index: d.index, muted: !d.muted });
                }
            }
            _ => shell::ask("cc.act", json!([id])),
        }
    }

    /// Right-click or press and hold (or a large tile's name): its choices,
    /// else its page (`detail`: else its switch)
    fn secondary(&mut self, st: &mut State, id: &str, x: f32, y: f32, detail: bool) {
        if self.editing {
            return;
        }
        let d = defs::def(id);
        let items = self.said.modules.get(id).map(|m| m.menu.clone()).unwrap_or_default();
        if d.is_some_and(|d| d.menu) && !items.is_empty() {
            self.show_menu(MenuOf::Module(id.into()), items, x, y);
        } else if let Some(page) = d.map(|d| d.page).filter(|p| !p.is_empty()) {
            self.set_page(st, page, id, y, true);
        } else if detail {
            self.act(st, id);
        }
    }

    fn show_menu(&mut self, of: MenuOf, items: Vec<MenuItem>, x: f32, y: f32) {
        let Some(w) = &self.win else { return };
        let rows: Vec<ui::CcMenuItem> = items.iter().map(|m| ui::CcMenuItem { text: m.text.as_str().into(), checked: m.checked, sep: m.separator, danger: m.danger }).collect();
        w.ui.set_menu(ModelRc::new(VecModel::from(rows)));
        w.ui.set_menu_x(x);
        w.ui.set_menu_y(y);
        w.ui.set_menu_open(true);
        self.menu = Some((of, items));
    }

    fn hide_menu(&mut self) {
        self.menu = None;
        if let Some(w) = &self.win {
            w.ui.set_menu_open(false);
        }
    }

    /// A click in a page's block
    fn block(&mut self, st: &mut State, i: usize, what: &str, text: &str) {
        let Some(b) = self.shown_page.blocks.get(i).cloned() else { return };
        if bridge::local(&self.page) {
            let snd = st.shell.get::<Sound>().cloned().unwrap_or_default();
            if b.id.starts_with("prof:") {
                shell::ask("cc.block", json!(["sound", b.id, "click", ""]));
            } else if let Some(name) = b.id.strip_prefix("out:") {
                audio::send(Cmd::DefaultSink(name.into()));
            } else if let Some(name) = b.id.strip_prefix("in:") {
                audio::send(Cmd::DefaultSource(name.into()));
            } else if let Some(s) = b.id.strip_prefix("app:").and_then(|n| n.parse::<u32>().ok()).and_then(|n| snd.streams.iter().find(|s| s.index == n)) {
                if what == "mute" {
                    audio::send(Cmd::StreamMute { index: s.index, muted: !s.muted });
                }
            }
            return;
        }
        let page = self.page.clone();
        let what = if what == "seg" { format!("seg:{text}") } else { what.to_string() };
        // (a typed password goes to the shell's Wi-Fi, never anywhere else)
        let text = if what == "commit" { text.to_string() } else { String::new() };
        shell::ask("cc.block", json!([page, b.id, what, text]));
    }

    fn block_slide(&mut self, st: &mut State, i: usize, v: f32) {
        let Some(b) = self.shown_page.blocks.get(i).cloned() else { return };
        if b.id == "mic" {
            self.slide(st, "mic", v);
        } else if let Some(n) = b.id.strip_prefix("app:").and_then(|n| n.parse::<u32>().ok()) {
            let snd = st.shell.get::<Sound>().cloned().unwrap_or_default();
            if let Some(s) = snd.streams.iter().find(|s| s.index == n) {
                if s.muted {
                    audio::send(Cmd::StreamMute { index: s.index, muted: false });
                }
                audio::send(Cmd::StreamVolume { index: s.index, channels: s.channels, volume: v.clamp(0.0, 1.0) as f64 });
            }
        } else {
            shell::ask("cc.block", json!([self.page, b.id, "slide", v]));
        }
    }

    fn rects(&self) -> Vec<(f32, f32, f32, f32)> {
        (0..self.model.row_count()).map(|k| self.model.row_data(k).unwrap()).map(|r| (r.x, r.y, r.w, r.h)).collect()
    }

    fn set_pkbd(&mut self, k: i32) {
        self.pkbd = k;
        if let Some(w) = &self.win {
            w.ui.set_page_kbd(k);
        }
    }

    /// Keys on an open page: Tab or the arrows go from row to row, Space or
    /// Enter acts, the Menu key opens a row's menu (ListRow's keys)
    fn page_key(&mut self, st: &mut State, text: &str, shift: bool) {
        let is = |key: Key| text == SharedString::from(key).as_str();
        let rows: Vec<usize> = self.shown_page.blocks.iter().enumerate().filter(|(_, b)| b.kind == BlockKind::Row && !b.disabled).map(|(i, _)| i).collect();
        if rows.is_empty() {
            return;
        }
        let at = rows.iter().position(|&i| i as i32 == self.pkbd);
        let step = if is(Key::DownArrow) || (is(Key::Tab) && !shift) {
            1
        } else if is(Key::UpArrow) || is(Key::Backtab) || (is(Key::Tab) && shift) {
            -1
        } else {
            0
        };
        if step != 0 {
            let n = rows.len() as i32;
            let k = match at {
                None => if step > 0 { 0 } else { n - 1 },
                Some(a) => (a as i32 + step).clamp(0, n - 1),
            };
            self.set_pkbd(rows[k as usize] as i32);
            return;
        }
        let Some(a) = at else { return };
        let i = rows[a];
        if text == " " || is(Key::Return) {
            self.block(st, i, "click", "");
        } else if is(Key::Menu) {
            let b = self.shown_page.blocks[i].clone();
            if !b.menu.is_empty() {
                let (x, y) = self.win.as_ref().map_or((0.0, 0.0), |w| (w.ui.get_panel_x(), w.ui.get_panel_y() + 100.0));
                self.show_menu(MenuOf::Block(self.page.clone(), b.id.clone()), b.menu.clone(), x, y);
            }
        }
    }

    fn set_kbd(&mut self, k: i32) {
        self.kbd = k;
        if let Some(w) = &self.win {
            w.ui.set_kbd(k);
        }
    }

    fn key(&mut self, st: &mut State, text: &str, shift: bool) {
        let is = |key: Key| text == SharedString::from(key).as_str();
        if is(Key::Escape) {
            if self.menu.is_some() {
                self.hide_menu();
            } else if !self.page.is_empty() {
                self.set_page(st, "", "", 0.0, true);
            } else if self.editing {
                self.set_editing(st, false, true);
            } else {
                self.close(true);
            }
            return;
        }
        if self.menu.is_some() {
            return;
        }
        if !self.page.is_empty() {
            self.page_key(st, text, shift);
            return;
        }
        let n = self.model.row_count() as i32;
        if n == 0 {
            return;
        }
        let cur = self.kbd;
        let id = self.id_at(cur);
        let kind = id.as_deref().and_then(defs::def).map(|d| d.kind);
        if is(Key::Tab) || is(Key::Backtab) {
            let step = if shift || is(Key::Backtab) { -1 } else { 1 };
            self.set_kbd(if cur < 0 { 0 } else { (cur + step).rem_euclid(n) });
            return;
        }
        if cur < 0 {
            self.set_kbd(0);
            return;
        }
        let id = id.unwrap_or_default();
        // (a slider takes the arrows itself)
        if kind == Some(Kind::Slider) && !self.editing {
            let v = self.model.row_data(cur as usize).map_or(0.0, |r| r.value);
            let nv = if is(Key::LeftArrow) || is(Key::DownArrow) {
                Some(v - 0.05)
            } else if is(Key::RightArrow) || is(Key::UpArrow) {
                Some(v + 0.05)
            } else if is(Key::PageDown) {
                Some(v - 0.2)
            } else if is(Key::PageUp) {
                Some(v + 0.2)
            } else if is(Key::Home) {
                Some(0.0)
            } else if is(Key::End) {
                Some(1.0)
            } else {
                None
            };
            if let Some(v) = nv {
                self.slide(st, &id, v);
                return;
            }
        }
        let (dx, dy) = if is(Key::LeftArrow) {
            (-1, 0)
        } else if is(Key::RightArrow) {
            (1, 0)
        } else if is(Key::UpArrow) {
            (0, -1)
        } else if is(Key::DownArrow) {
            (0, 1)
        } else {
            (0, 0)
        };
        if dx != 0 || dy != 0 {
            if let Some(k) = defs::nearest_toward(&self.rects(), cur as usize, dx, dy) {
                self.set_kbd(k as i32);
            }
            return;
        }
        let page = defs::def(&id).map(|d| d.page).unwrap_or("");
        if text == " " {
            if matches!(kind, Some(Kind::Toggle | Kind::Action)) || self.editing {
                self.act(st, &id);
            } else if !page.is_empty() {
                let y = self.module_y_of(st, &id);
                self.set_page(st, page, &id, y, true);
            }
        } else if is(Key::Return) {
            if self.editing {
                self.act(st, &id);
            } else if !page.is_empty() {
                let y = self.module_y_of(st, &id);
                self.set_page(st, page, &id, y, true);
            } else {
                self.act(st, &id);
            }
        } else if is(Key::Menu) {
            self.secondary(st, &id, 0.0, 0.0, false);
        } else if self.editing && (is(Key::Delete) || is(Key::Backspace)) {
            let l = defs::remove(&self.layout, &id);
            self.save(st, l);
        }
    }

    fn from_shell(&mut self, st: &mut State, v: &Value) {
        if v.get("state").is_some() {
            let said = Said::from(&v["state"]);
            let avatar = said.avatar != self.said.avatar;
            self.said = said;
            if avatar {
                self.push_avatar();
            }
            if self.open {
                self.want_art(st);
            }
            if self.open {
                self.rebuild(st);
                if self.page == "sound" {
                    self.push_page(st);
                }
            }
        }
        if v.get("page").is_some() {
            self.shell_page = Page::from(&v["page"]);
            if self.open && self.shell_page.name == self.page && !bridge::local(&self.page) {
                self.push_page(st);
            }
        }
        if let Some(p) = v["ccPage"].as_str() {
            if self.open {
                self.set_page(st, p, "", 0.0, false);
            }
        }
        if let Some(e) = v["editing"].as_bool() {
            if self.open {
                self.set_editing(st, e, false);
            }
        }
    }
}

/// A page's name until the shell says
fn page_title(page: &str) -> &'static str {
    match page {
        "wifi" => "Wi-Fi",
        "bluetooth" => "Bluetooth",
        "sound" => "Sound",
        "mic" => "Microphone",
        "focus" => "Focus",
        "caffeine" => "Caffeine",
        "resources" => "CPU & Memory",
        "sharing" => "Sharing",
        "mirroring" => "Screen Mirroring",
        "cast" => "Cast",
        _ => "",
    }
}

fn block(b: &Block, first: bool, last: bool, material: bool, (caption, small): (f32, f32)) -> ui::CcBlock {
    let pic = picture(&b.picture);
    let width = if material { M3_W } else { GLASS_W } - 12.0;
    let gap = match b.kind {
        BlockKind::Note => bridge::text_height(&b.title, if b.tone == bridge::Tone::Tertiary { caption } else { small }, width),
        BlockKind::Notice => bridge::text_height(&b.status, small, width),
        _ => b.height,
    };
    let i = |n: &str| if n.is_empty() { ui::CcIc::default() } else { ic(n, material) };
    let mut options: Vec<SharedString> = b.options.iter().map(|o| o.as_str().into()).collect();
    if b.kind == BlockKind::Notice && !b.action.is_empty() {
        options = vec![b.action.as_str().into()];
    }
    ui::CcBlock {
        kind: b.kind.code(),
        id: b.id.as_str().into(),
        icon: i(&b.icon),
        has_pic: pic.is_some(),
        pic: pic.unwrap_or_default(),
        title: b.title.as_str().into(),
        status: b.status.as_str().into(),
        hl: b.highlighted,
        trail: i(&b.trailing),
        busy: b.busy,
        act: if b.kind == BlockKind::Notice { ui::CcIc::default() } else { i(&b.action) },
        dis: b.disabled,
        value: b.value as f32,
        tone: b.tone.code(),
        center: b.center,
        options: ModelRc::new(VecModel::from(options)),
        cur: b.current,
        flag: b.flag,
        gap,
        first,
        last,
    }
}

impl Part for ControlCenter {
    fn name(&self) -> &'static str {
        NAME
    }

    fn command(&mut self, st: &mut State, cmd: &Command) {
        match cmd.cmd.as_str() {
            "open" => self.open(st, cmd.screen.clone()),
            "close" => self.close(false),
            _ => {}
        }
    }

    fn config_changed(&mut self, st: &mut State, look: bool) {
        self.saved = defs::saved(&st.shell.cfg);
        if self.win.is_some() {
            if look {
                self.model.set_vec(Vec::new());
                if let Some(w) = &mut self.win {
                    w.glass_sent.clear();
                }
            }
            self.apply_look(st);
        }
        if self.open {
            self.rebuild(st);
            self.push_page(st);
        }
    }

    fn plugin_changed(&mut self, st: &mut State) {
        if let Some(w) = &mut self.win {
            w.glass_sent.clear();
        }
        self.apply_look(st);
    }

    fn data(&mut self, st: &mut State, what: TypeId) {
        if what == TypeId::of::<Players>() && self.open {
            self.want_art(st);
            self.push_footer(st);
            return;
        }
        if what == TypeId::of::<Sound>() || what == TypeId::of::<Backlight>() || what == TypeId::of::<Battery>() || what == TypeId::of::<Wifi>() || what == TypeId::of::<Bluetooth>() {
            // (a slider or the microphone may have just become usable)
            let before = self.layout.len();
            if self.open {
                let usable = |id: &str| self.usable(id, st);
                if defs::layout(&self.saved, &usable).len() != before {
                    self.rebuild(st);
                }
            }
            self.refresh_levels(st);
        }
    }

    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        let msg = match msg.downcast::<Value>() {
            Ok(v) => return self.from_shell(st, &v),
            Err(m) => m,
        };
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Act(i) => {
                if let Some(id) = self.id_at(i) {
                    self.act(st, &id);
                }
            }
            M::OpenPage(i, y) => {
                if let Some(id) = self.id_at(i) {
                    if let Some(p) = defs::def(&id).map(|d| d.page).filter(|p| !p.is_empty()) {
                        self.set_page(st, p, &id, y, true);
                    }
                }
            }
            M::Secondary(i, x, y) => {
                if let Some(id) = self.id_at(i) {
                    // (a large tile's name: x comes negated)
                    let detail = x < 0.0;
                    self.secondary(st, &id, if detail { -x - 1.0 } else { x }, y, detail);
                }
            }
            M::Remove(i) => {
                if let Some(id) = self.id_at(i) {
                    let l = defs::remove(&self.layout, &id);
                    self.save(st, l);
                }
            }
            M::Resize(i) => {
                if let Some(id) = self.id_at(i) {
                    let l = defs::cycle(&self.layout, &id);
                    self.save(st, l);
                }
            }
            M::Drop(i, cx, cy) => {
                if let Some(id) = self.id_at(i) {
                    let (gap, cw, ch) = cell(Self::material(st));
                    if let Some(best) = defs::nearest_slot(&self.packed, cx, cy, cw, ch, gap) {
                        let target = self.packed[best].id.clone();
                        if target != id {
                            let to = self.layout.iter().position(|e| e.id == target).unwrap_or(best);
                            let l = defs::move_to(&self.layout, &id, to);
                            self.save(st, l);
                        }
                    }
                    self.rebuild(st);
                }
            }
            M::Slide(i, v) => {
                if let Some(id) = self.id_at(i) {
                    self.slide(st, &id, v);
                }
            }
            M::Press(i, down) => {
                if let (Some(id), Some(w)) = (self.id_at(i), &self.win) {
                    if down {
                        let sq = defs::squeeze(&self.packed, &id, 14.0);
                        for k in 0..self.model.row_count() {
                            let mut r = self.model.row_data(k).unwrap();
                            let (dx, dw) = self.packed.iter().position(|p| p.id == r.id.as_str()).map_or((0.0, 0.0), |p| sq[p]);
                            if (r.sq_dx, r.sq_dw) != (dx, dw) {
                                r.sq_dx = dx;
                                r.sq_dw = dw;
                                self.model.set_row_data(k, r);
                            }
                        }
                    }
                    w.ui.set_press_now(down);
                }
            }
            M::Media(w) => {
                let bus = st.shell.get::<Players>().and_then(|p| p.chosen()).map(|p| p.bus.clone());
                let method = match w.as_str() {
                    "previous" => Some("Previous"),
                    "next" => Some("Next"),
                    "toggle" => Some("PlayPause"),
                    _ => None,
                };
                if w == "cast" {
                    self.set_page(st, "cast", "nowPlaying", 0.0, true);
                } else if let (Some(bus), Some(m)) = (bus, method) {
                    mpris::control(&bus, m);
                } else {
                    // (seeking: the shell's player knows the position)
                    shell::ask("cc.media", json!([w]));
                }
            }
            M::Art(url, px) => {
                if url == self.art_url {
                    self.art = px.map(slint::Image::from_rgba8);
                    self.push_footer(st);
                }
            }
            M::Add(id) => {
                let l = defs::add(&self.layout, &id);
                self.save(st, l);
            }
            M::Reset => {
                self.saved.clear();
                shell::ask("cc.layout", json!([[]]));
                self.saved = defs::DEFAULT_LAYOUT.iter().map(|s| s.to_string()).collect();
                self.rebuild(st);
            }
            M::ToggleEditing => {
                let on = !self.editing;
                self.set_editing(st, on, true);
            }
            M::Settings => {
                self.close(false);
                shell::ask("panels.openSettings", json!([]));
            }
            M::Power => {
                let out = self.win.as_ref().map(|w| w.output.clone()).unwrap_or_default();
                shell::ask("panels.show", json!(["session", out]));
            }
            M::Back => self.set_page(st, "", "", 0.0, true),
            M::PageSwitch => shell::ask("cc.block", json!([self.page, "", "switch", ""])),
            M::PageRefresh => shell::ask("cc.block", json!([self.page, "", "refresh", ""])),
            M::Block(i, w, t) => self.block(st, i.max(0) as usize, &w, &t),
            M::BlockMenu(i, x, y) => {
                if let Some(b) = self.shown_page.blocks.get(i.max(0) as usize).cloned() {
                    if !b.menu.is_empty() {
                        self.show_menu(MenuOf::Block(self.page.clone(), b.id.clone()), b.menu.clone(), x, y);
                    }
                }
            }
            M::BlockSlide(i, v) => self.block_slide(st, i.max(0) as usize, v),
            M::MenuPick(i) => {
                if let Some((of, items)) = self.menu.take() {
                    if items.get(i.max(0) as usize).is_some_and(|m| !m.separator) {
                        match of {
                            MenuOf::Module(id) => shell::ask("cc.pick", json!(["", id, i])),
                            MenuOf::Block(page, id) => shell::ask("cc.pick", json!([page, id, i])),
                        }
                    }
                }
                self.hide_menu();
            }
            M::MenuClose => self.hide_menu(),
            M::Dismissed => self.close(true),
            M::Key(t, shift) => self.key(st, &t, shift),
            M::Closed => self.closed(st),
        }
    }

    fn keyboard_left(&mut self, _st: &mut State, s: &Rc<Surface>) {
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

    /// The menu bar strip stays the bar's; Glass's shapes (the panel, or
    /// each control's chip; the page's popover); the accessibility tree
    fn after_frame(&mut self, st: &mut State) {
        let kbd = st.keyboard_surface();
        let material = Self::material(st);
        let bare = !material && defs::bare(&st.shell.cfg);
        let Some(w) = &mut self.win else { return };
        let f = kbd.as_ref().is_some_and(|k| Rc::ptr_eq(k, &w.surface));
        if let Some(a) = w.a11y.as_mut() {
            a.update(false, f);
        }
        if !w.surface.mapped.get() {
            return;
        }
        let size = w.surface.logical_size();
        if self.open && size != w.region && size.0 > 0 {
            st.set_input_region(&w.surface, Some(&[(0, 30, size.0 as i32, size.1 as i32 - 30)]));
            w.region = size;
        }
        if !st.shell.plugin || material {
            return;
        }
        let ui = &w.ui;
        let look = &st.shell.look;
        let o = ui.get_panel_opacity();
        let (px, py, pw, ph) = (ui.get_panel_x(), ui.get_panel_y(), ui.get_panel_w(), ui.get_panel_h());
        let mut shapes = Vec::new();
        if o > 0.002 {
            if !bare {
                shapes.push(theme::glass_shape(look, px, py, pw, ph, 36.0 + PAD, o, 1.0));
            } else {
                let editing = ui.get_editing();
                let gx = px + PAD + if editing { 260.0 + 16.0 + 2.0 * PAD } else { 0.0 };
                let gy = py + PAD;
                let paging = ui.get_paging();
                for k in 0..self.model.row_count() {
                    let r = self.model.row_data(k).unwrap();
                    let kind = defs::def(&r.id).map_or(Kind::Action, |d| d.kind);
                    let rad = chip_radius(kind, r.cols as u8, r.rows as u8, r.w, r.h);
                    let dim = if paging && !r.focused { 0.4 } else { 1.0 };
                    shapes.push(theme::glass_shape(look, gx + r.x, gy + r.y, r.w, r.h, rad, o * dim, 1.0));
                }
                let fy = gy + ui.get_footer_y();
                let fo = o * if paging { 0.45 } else { 1.0 };
                shapes.push(theme::glass_shape(look, gx, fy, 40.0, 40.0, 20.0, fo, 1.0));
                if ui.get_battery() {
                    shapes.push(theme::glass_shape(look, gx + 48.0, fy, ui.get_battery_w(), 40.0, 20.0, fo, 1.0));
                }
                let done = if editing { 74.0 } else { 40.0 };
                let mut x = gx + GLASS_W - done;
                shapes.push(theme::glass_shape(look, x, fy, done, 40.0, 20.0, fo, 1.0));
                for _ in 0..2 {
                    x -= 48.0;
                    shapes.push(theme::glass_shape(look, x, fy, 40.0, 40.0, 20.0, fo, 1.0));
                }
                if editing {
                    shapes.push(theme::glass_shape(look, px, py, 260.0 + 2.0 * PAD, ui.get_gallery_h() + 2.0 * PAD, 36.0 + PAD, o, 1.0));
                }
            }
            let s = ui.get_sub_shown();
            if s > 0.01 {
                let (sw, sh) = (ui.get_sub_w(), ui.get_sub_h());
                let k = 0.96 + 0.04 * s;
                let x = ui.get_sub_x() + (1.0 - s) * 12.0 + sw * (1.0 - k);
                let y = ui.get_sub_top() + 24.0 * (1.0 - k);
                shapes.push(theme::glass_shape(look, x, y, sw * k, sh * k, (36.0 + PAD) * k, o * s.min(1.0), 1.0));
            }
        }
        let key = format!("{shapes:?}");
        if key != w.glass_sent {
            st.shell.glass.set(&w.output, NS, &shapes);
            w.glass_sent = key;
        }
    }

    fn a11y(&mut self) -> Vec<&mut crate::a11y::A11y> {
        self.win.as_mut().and_then(|w| w.a11y.as_mut()).into_iter().collect()
    }

    fn stats(&self) {
        if let Some(w) = &self.win {
            eprintln!("tessera-ui: stats {}", w.surface.stats.borrow_mut().report(NAME));
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

    #[test]
    fn places_in_both_looks() {
        let p = Placed { id: "wifi".into(), w: 2, h: 1, col: 0, row: 0 };
        assert_eq!(place(&p, false), (0.0, 0.0, 155.0, 72.5));
        assert_eq!(place(&p, true), (0.0, 0.0, 180.0, 72.0));
        let p = Placed { id: "display".into(), w: 1, h: 2, col: 3, row: 1 };
        assert_eq!(place(&p, false), (247.5, 82.5, 72.5, 155.0));
        assert_eq!(place(&p, true), (282.0, 80.0, 86.0, 152.0));
        assert_eq!(grid_height(7, false), 7.0 * 72.5 + 60.0);
        assert_eq!(grid_height(0, true), 0.0);
    }

    #[test]
    fn chips_round_as_the_modules() {
        assert_eq!(chip_radius(Kind::Toggle, 1, 1, 72.5, 72.5), 36.25);
        assert_eq!(chip_radius(Kind::Toggle, 2, 1, 155.0, 72.5), 36.25);
        assert_eq!(chip_radius(Kind::Media, 2, 2, 155.0, 155.0), 36.25);
        assert_eq!(chip_radius(Kind::Slider, 4, 1, 320.0, 72.5), 36.25);
        assert_eq!(chip_radius(Kind::Info, 2, 2, 155.0, 155.0), 36.25);
    }

    #[test]
    fn accents_on_white_and_on_the_panel() {
        let blue = slint::Color::from_rgb_u8(0x0a, 0x84, 0xff);
        assert_eq!(accent_on_white(blue), blue);
        let yellow = slint::Color::from_rgb_u8(0xff, 0xd6, 0x0a);
        assert_eq!(accent_on_white(yellow), slint::Color::from_rgb_u8(0x1c, 0x1c, 0x1e));
        assert_eq!(accent_ink(yellow, false), slint::Color::from_rgb_u8(0x1c, 0x1c, 0x1e));
        let deep = slint::Color::from_rgb_u8(0x30, 0x30, 0xa0);
        assert!(theme::contrast_ratio(accent_ink(deep, true), slint::Color::from_rgb_u8(0x1e, 0x1e, 0x21)) >= 4.5);
    }

    #[test]
    fn keys_and_volume_icons() {
        let k = |key: Key| SharedString::from(key).to_string();
        assert!(handles(&k(Key::Escape)) && handles(" ") && handles(&k(Key::Tab)));
        assert!(!handles("a"));
        assert_eq!(volume_icon(0.5, true), "speaker-x");
        assert_eq!(volume_icon(0.2, false), "speaker-none");
        assert_eq!(volume_icon(0.9, false), "speaker-high");
    }
}

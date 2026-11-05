//! The menu bar (rust.surfaces: menubar): one Bar per output (with the
//! island, which opens below it), one menu window, and the data flowing into
//! them. Compare shell/modules/menubar/MenuBar.qml and its module.
//!
//! What it reads: Hyprland (spaces, the focused window, fullscreen), battery
//! and Wi-Fi, Bluetooth, media players, the tray, the wallpaper's top (text
//! colour), and what the QML shell still owns (services::bridge: the open
//! panel, Focus, Caffeine, notifications, the island's alerts…). Its own
//! menus (system, app, Window, tray, a space's) are drawn here; the status
//! items open the QML shell's popovers under them (`rustui act toggleAt`).
//! (Not yet: the Wayland idle inhibitor for Caffeine (logind's still holds),
//! touch swipes down from the bar, the drop shelf as a drop target, sliding
//! from a QML popover onto a Rust item, the island's Recent list, its alerts
//! card below an open island, Esc on an alert, the open music player's art
//! as the island itself, a scrolling song title; the HUD's outputs list
//! inline (its button opens the shell's Sound menu, or the Control Center
//! for displays).)
//! Where Slint differs: no font features, so the clock's digits aren't
//! tabular (a few px narrower); no font axes, so Material Symbols draw with
//! the font's default fill (outlined where Icon.qml fills them); Material's
//! see-through menus (materialTransparency) stay opaque, without the veil's
//! blur. Qt draws Google Sans Flex at one weight whatever the QML asks
//! (Material's app name and card titles look regular there); this bar draws
//! the weights MenuBarItem.qml asks for.

pub mod island;
pub mod items;
pub mod menus;
pub mod settings;
pub mod spaces;

use crate::icons::glyph;
use crate::services::bluez::Bluetooth;
use crate::services::bridge::{self, IslandCall, LiveActivities, ShellState};
use crate::services::hypr::{self, Glass, Snapshot};
use crate::services::mpris::{self, Players};
use crate::services::status::{Battery, Wifi};
use crate::services::tray::{self, Tray, TrayMenu};
use crate::services::wallpaper::{self, WallTop};
use crate::shell::Part;
use crate::theme;
use crate::ui;
use crate::util::{desktop_entry, desktop_name, now};
use crate::wayland::{Spec, State, Surface};
use island::{Activity, Island, Mode};
use menus::{Action, Row};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use smithay_client_toolkit::reexports::client::protocol::wl_output::WlOutput;
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use smithay_client_toolkit::shell::WaylandSurface;
use std::any::{Any, TypeId};
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

pub const BAR_NS: &str = "tessera:bar";
pub const MENU_NS: &str = "tessera:barmenu";
const NAME: &str = "menubar";
const BAR_H: u32 = 30;
/// with the island: room for it to open below the bar (MenuBar.qml's 420)
const ISLAND_H: u32 = 420;
/// without: room for the hover tips
const TIP_H: u32 = 64;

/// Its own messages (Msg::Part("menubar", …))
enum M {
    Tick,
    Sweep,
    MusicLinger,
    TrayCollapse,
    Menu(String, String, f32),
    Clicked(String, String, bool, f32),
    MenuActivated(i32),
    MenuTile(i32, i32),
    MenuStep(i32),
    MenuDismissed,
    MenuClosed,
    Go(i32),
    SpaceMenu(String, i32, f32),
    Arrow(String, i32),
    SpaceWheel(i32),
    Scrolled(bool, f32),
    IslandHover(String, bool),
    IslandPill(String),
    IslandCycle(f32, f32),
    IslandControl(i32),
    IslandAct(i32, String),
    IslandDismiss(i32),
    IslandLevel(f32),
    IslandPin,
    IslandHold(bool),
    Art(String, Option<std::path::PathBuf>),
}

struct BarEntry {
    output: String,
    surface: Rc<Surface>,
    ui: ui::Bar,
    a11y: Option<crate::a11y::A11y>,
    glass_sent: String,
    region_sent: String,
    layer: Layer,
    exclusive: i32,
    height: u32,
    hovered: bool,
    hover_open: bool,
    fs_open: bool,
    /// the window over its own spaces (Workspaces.start)
    start: i32,
    chip_ids: Vec<i32>,
    // what its models show last (so an unchanged refresh doesn't redraw)
    chips_key: String,
    live_key: String,
    alerts_key: String,
}

#[derive(Clone, Debug, PartialEq)]
enum MenuKind {
    System,
    App,
    Window,
    /// a tray item's (key, its menu's path)
    Tray(String, String),
    Space(i32),
}

struct MenuEntry {
    output: String,
    surface: Rc<Surface>,
    ui: ui::Menu,
    a11y: Option<crate::a11y::A11y>,
    open: bool,
    glass_sent: String,
    kind: MenuKind,
    rows: Vec<Row>,
    region_sent: String,
    size_sent: (u32, u32),
}

#[derive(Default)]
pub struct Menubar {
    bars: Vec<BarEntry>,
    menu: Option<MenuEntry>,
    names: HashMap<String, String>,
    clock: slint::Timer,
    sweep: slint::Timer,
    linger: slint::Timer,
    tray_timer: slint::Timer,
    island: Island,
    tray_open: bool,
    /// the tray item whose menu is open (its highlight)
    tray_menu: String,
    tray_images: HashMap<String, (tray::Icon, slint::Image)>,
    app_icons: HashMap<String, Option<slint::Image>>,
    pictures: HashMap<String, Option<slint::Image>>,
    /// the shown player's art: (its address, the picture once loaded)
    art: (String, Option<slint::Image>),
    /// music paused: it stays in the island until then (Island.musicLingers)
    linger_until: f64,
    was_playing: bool,
    /// the QML shell's activities (without music)
    live: Vec<Activity>,
    /// the wallpaper last sampled for the text colour
    wall: String,
}

fn ms_now() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64() * 1000.0).unwrap_or(0.0)
}

#[allow(dead_code)]
fn send(st: &State, m: M) {
    let _ = st.tx.send(crate::Msg::Part(NAME, Box::new(m)));
}

fn load(path: &std::path::Path) -> Option<slint::Image> {
    slint::Image::load_from_path(path).ok()
}

/// rgb luminance of a colour (is the bar's text dark?)
fn luma(c: slint::Color) -> f32 {
    (0.2126 * c.red() as f32 + 0.7152 * c.green() as f32 + 0.0722 * c.blue() as f32) / 255.0
}

impl Menubar {
    fn spec<'a>(output: &'a WlOutput, name: &'a str, height: u32, exclusive: i32, layer: Layer) -> Spec<'a> {
        Spec {
            layer,
            namespace: BAR_NS,
            output: Some((output, name)),
            anchor: Anchor::TOP | Anchor::LEFT | Anchor::RIGHT,
            size: (0, height),
            exclusive,
            keyboard: KeyboardInteractivity::None,
            margin_left: 0,
        }
    }

    fn cfg(st: &State) -> settings::Bar {
        settings::bar(&st.shell.cfg)
    }

    fn shell_state(st: &State) -> ShellState {
        st.shell.get::<ShellState>().cloned().unwrap_or_default()
    }

    fn app_name(&mut self, class: &str) -> String {
        if class.is_empty() {
            return "Desktop".to_string();
        }
        self.names
            .entry(class.to_string())
            .or_insert_with(|| {
                desktop_name(class).unwrap_or_else(|| {
                    let mut c = class.chars();
                    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
                })
            })
            .clone()
    }

    fn app_icon(&mut self, class: &str) -> Option<slint::Image> {
        self.app_icons.entry(class.to_string()).or_insert_with(|| crate::icon_theme::for_class(class).and_then(|p| load(&p))).clone()
    }

    fn picture(&mut self, src: &str) -> Option<slint::Image> {
        if src.is_empty() {
            return None;
        }
        if let Some(hit) = self.pictures.get(src) {
            return hit.clone();
        }
        let path = mpris::art_file(src).or_else(|| src.starts_with('/').then(|| std::path::PathBuf::from(src)));
        // (web art is fetched on a thread: it shows once it's here)
        let img = path.filter(|p| p.is_file()).and_then(|p| load(&p));
        if self.pictures.len() > 64 {
            self.pictures.clear();
        }
        self.pictures.insert(src.to_string(), img.clone());
        img
    }

    fn tray_image(&mut self, item: &tray::TrayItem) -> slint::Image {
        if let Some((icon, img)) = self.tray_images.get(&item.key) {
            if icon == &item.icon {
                return img.clone();
            }
        }
        let img = match &item.icon {
            tray::Icon::File(p) => load(p).unwrap_or_default(),
            tray::Icon::Pixels(w, h, rgba) => {
                let mut buf = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(*w, *h);
                buf.make_mut_bytes().copy_from_slice(&rgba[..(*w * *h * 4) as usize]);
                slint::Image::from_rgba8(buf)
            }
            tray::Icon::None => slint::Image::default(),
        };
        self.tray_images.insert(item.key.clone(), (item.icon.clone(), img.clone()));
        img
    }

    /// Music, as an activity: playing, or paused for less than a minute
    fn music(&mut self, st: &State) -> Option<Activity> {
        let players = st.shell.get::<Players>()?;
        let p = players.chosen()?;
        let now = ms_now();
        if p.playing || (!p.track.title.is_empty() && now < self.linger_until) {
            Some(Activity { kind: "music".into(), icon: "music-notes".into(), title: if p.track.title.is_empty() { p.identity.clone() } else { p.track.title.clone() }, ..Default::default() })
        } else {
            None
        }
    }

    fn update_activities(&mut self, st: &State) {
        let m = self.music(st);
        let acts = island::with_music(self.live.clone(), m);
        self.island.set_activities(acts);
    }

    /// The shown player changed: its art (fetched off the UI thread when it's on the web)
    fn follow_art(&mut self, st: &State) {
        let url = st.shell.get::<Players>().and_then(|p| p.chosen()).map(|p| p.track.art_url.clone()).unwrap_or_default();
        if url == self.art.0 {
            return;
        }
        self.art = (url.clone(), None);
        if url.is_empty() {
            return;
        }
        let tx = st.tx.clone();
        std::thread::spawn(move || {
            let path = mpris::art_file(&url);
            let _ = tx.send(crate::Msg::Part(NAME, Box::new(M::Art(url, path))));
        });
    }

    /// The wallpaper changed: is its top light? (the text colour)
    fn follow_wallpaper(&mut self, st: &State) {
        let path = st.shell.cfg["wallpaper"]["path"].as_str().unwrap_or("").to_string();
        if path != self.wall {
            self.wall = path.clone();
            if !path.is_empty() {
                wallpaper::sample(&path, st.tx.clone());
            }
        }
    }

    fn live_struct(&mut self, st: &State, cfg: &settings::Bar, material: bool, roles: &ui::Theme) -> ui::Live {
        let Some(a) = self.island.primary().cloned() else { return ui::Live::default() };
        let tint_of = |c: &str| -> slint::Color {
            let c = island::color(c).unwrap_or(roles.get_accent());
            if material {
                if island::is_red(c) {
                    roles.get_m3_error()
                } else {
                    roles.get_m3_primary()
                }
            } else {
                c
            }
        };
        if a.kind == "music" {
            let players = st.shell.get::<Players>().cloned().unwrap_or_default();
            let Some(p) = players.chosen() else { return ui::Live::default() };
            let ctl = |icon: &str, on: bool| ui::Ctl { icon: glyph(icon).into(), fill: true, on, danger: false };
            let mut controls = Vec::new();
            if cfg.shows("music", "prev") {
                controls.push(ctl("skip-back", p.can_prev));
            }
            if cfg.shows("music", "play") {
                controls.push(ctl(if p.playing { "pause" } else { "play" }, true));
            }
            if cfg.shows("music", "next") {
                controls.push(ctl("skip-forward", p.can_next));
            }
            let art = self.art.1.clone();
            return ui::Live {
                kind: "music".into(),
                icon: glyph("music-notes").into(),
                tint: tint_of(""),
                title: (if p.track.title.is_empty() { p.identity.as_str() } else { p.track.title.as_str() }).into(),
                artist: p.track.artist.as_str().into(),
                time: SharedString::new(),
                has_art: art.is_some(),
                art: art.unwrap_or_default(),
                playing: p.playing,
                show_lead: cfg.shows("music", "art"),
                show_title: cfg.shows("music", "title"),
                show_time: false,
                show_bars: cfg.shows("music", "bars"),
                controls: ModelRc::new(VecModel::from(controls)),
            };
        }
        ui::Live {
            kind: a.kind.as_str().into(),
            icon: crate::icons::glyph_or(&a.icon, "circle").into(),
            tint: tint_of(&a.color),
            title: a.title.as_str().into(),
            artist: SharedString::new(),
            time: a.time.as_str().into(),
            art: slint::Image::default(),
            has_art: false,
            playing: false,
            show_lead: a.show_lead,
            show_title: a.show_title,
            show_time: a.show_time,
            show_bars: false,
            controls: ModelRc::new(VecModel::from(a.controls.iter().map(|(i, f, on, d)| ui::Ctl { icon: glyph(i).into(), fill: *f, on: *on, danger: *d }).collect::<Vec<_>>())),
        }
    }

    fn alert_rows(&mut self, material: bool, roles: &ui::Theme) -> Vec<ui::AlertRow> {
        let alerts = self.island.alerts.clone();
        alerts
            .iter()
            .map(|al| {
                let e = &al.event;
                let s = |k: &str| e[k].as_str().unwrap_or("").to_string();
                let kind = s("kind");
                let raw = island::color(&s("color")).unwrap_or(roles.get_accent());
                let tint = if material { if island::is_red(raw) { roles.get_m3_error() } else { roles.get_m3_primary() } } else { raw };
                let value = e["value"].as_f64();
                let ringed = matches!(kind.as_str(), "charging" | "battery" | "timer" | "temperature") && value.is_some();
                let ring_color = |v: f64| if v <= 0.2 { roles.get_danger() } else if material { roles.get_m3_primary() } else { roles.get_success() };
                let mut rings: Vec<ui::Ring> = Vec::new();
                if let Some(parts) = e["batteries"].as_array().filter(|p| !p.is_empty()) {
                    for p in parts {
                        let v = p["value"].as_f64().unwrap_or(0.0);
                        rings.push(ui::Ring { arc: island::arc(17.0, 17.0, 15.0, v as f32).into(), color: ring_color(v), text: format!("{}", (v * 100.0).round()).into(), label: p["label"].as_str().unwrap_or("").into() });
                    }
                } else if let Some(v) = e["battery"].as_f64() {
                    rings.push(ui::Ring { arc: island::arc(17.0, 17.0, 15.0, v as f32).into(), color: ring_color(v), text: format!("{}", (v * 100.0).round()).into(), label: SharedString::new() });
                }
                let card = &e["card"];
                let picture = self.picture(&s("picture"));
                let art = self.picture(&s("art"));
                let app_icon = card["appIcon"].as_str().filter(|n| !n.is_empty()).and_then(|n| crate::icon_theme::find(n, None)).and_then(|p| load(&p));
                let sub_color = island::color(&s("subtitleColor"));
                let more: Vec<SharedString> = e["more"].as_array().map(|m| m.iter().map(|x| SharedString::from(x["text"].as_str().unwrap_or(""))).collect()).unwrap_or_default();
                ui::AlertRow {
                    uid: al.uid as i32,
                    kind: kind.as_str().into(),
                    icon: crate::icons::glyph_or(&s("icon"), "bell").into(),
                    tint,
                    title: s("title").into(),
                    subtitle: s("subtitle").into(),
                    sub_color: sub_color.unwrap_or_default(),
                    has_sub_color: sub_color.is_some(),
                    ringed,
                    ring_arc: island::arc(22.0, 22.0, 20.0, value.unwrap_or(0.0) as f32).into(),
                    rings: ModelRc::new(VecModel::from(rings)),
                    action: s("action").into(),
                    action2: s("action2").into(),
                    more: ModelRc::new(VecModel::from(more)),
                    has_picture: picture.is_some(),
                    picture: picture.unwrap_or_default(),
                    has_art: art.is_some(),
                    art: art.unwrap_or_default(),
                    card: card.is_object(),
                    headline: s("headline").into(),
                    card_name: card["name"].as_str().unwrap_or("").into(),
                    card_time: card["time"].as_str().unwrap_or("").into(),
                    card_text: card["text"].as_str().unwrap_or("").into(),
                    card_initials: card["initials"].as_str().unwrap_or("").into(),
                    has_app_icon: app_icon.is_some(),
                    app_icon: app_icon.unwrap_or_default(),
                    hud: kind == "hud",
                    hud_value: value.unwrap_or(0.0) as f32,
                    device: s("device").into(),
                    leaving: al.leaving,
                    delay: al.delay as i64,
                }
            })
            .collect()
    }

    /// Push the data into every bar
    fn refresh(&mut self, st: &mut State) {
        let snapshot = st.shell.get::<Snapshot>().cloned().unwrap_or_default();
        let bat = st.shell.get::<Battery>().cloned().unwrap_or_default();
        let wifi = st.shell.get::<Wifi>().cloned().unwrap_or_default();
        let bt = st.shell.get::<Bluetooth>().cloned().unwrap_or_default();
        let players = st.shell.get::<Players>().cloned().unwrap_or_default();
        let tray_now = st.shell.get::<Tray>().cloned().unwrap_or_default();
        let shell = Self::shell_state(st);
        let light = st.shell.get::<WallTop>().is_some_and(|w| w.light && w.path == self.wall);
        let cfg = Self::cfg(st);
        let material = st.shell.look.material;
        let class = snapshot.class.clone();
        let app_name = self.app_name(&class);
        let (short, long) = settings::clock_formats(&st.shell.cfg);
        let (clock, clock_long) = (now(&short), now(&long));
        let logo = settings::menu_icon(&st.shell.cfg);
        let now_ms = ms_now();
        // the bar's look over the wallpaper (MenuBar.qml: opaque, wallInk, pills)
        let opaque = cfg.background || cfg.auto_hide;
        let pills = material && !opaque;
        let pinned = shell.bool("island.pinned");
        let fs_mode = if shell.str("island.fsMode").is_empty() { "show".to_string() } else { shell.str("island.fsMode").to_string() };
        let tray_images: HashMap<String, slint::Image> = tray_now.items.clone().iter().map(|t| (t.key.clone(), self.tray_image(t))).collect();
        let bases: BTreeMap<String, i32> = shell.get("spaces.bases").as_object().map(|o| o.iter().filter_map(|(k, v)| v.as_i64().map(|b| (k.clone(), b as i32))).collect()).unwrap_or_default();
        let labels: BTreeMap<i32, String> = shell.get("spaces.labels").as_object().map(|o| o.iter().filter_map(|(k, v)| Some((k.parse().ok()?, v.as_str()?.to_string()))).collect()).unwrap_or_default();
        let open_menu = self.menu.as_ref().filter(|m| m.open).map(|m| (m.output.clone(), m.kind.clone()));
        let tray_menu = match &open_menu {
            Some((_, MenuKind::Tray(k, _))) => k.clone(),
            _ => String::new(),
        };
        let event = self.island.event().cloned();
        let has_card = self.island.has_card();
        let live_count = self.island.activities.len() as i32;
        let live_index = self.island.focus_index() as i32;
        let status = {
            let mut s = Vec::new();
            if bat.present {
                s.push(format!("Battery {}%{}", bat.percent.round(), if bat.charging { " · charging" } else { "" }));
            }
            if shell.num("resources.cpu") > 0.0 {
                s.push(format!("CPU {}%", (shell.num("resources.cpu") * 100.0).round()));
                s.push(format!("Memory {}%", (shell.num("resources.memory") * 100.0).round()));
            }
            s.join(" · ")
        };
        let playing = players.chosen().is_some_and(|p| p.playing);
        let mut bars = std::mem::take(&mut self.bars);
        for b in &mut bars {
            let theme = b.ui.global::<ui::Theme>();
            let label = theme.get_label();
            let wall_ink = if opaque { label } else if light { slint::Color::from_argb_u8(217, 0, 0, 0) } else { slint::Color::from_argb_u8(245, 255, 255, 255) };
            let ink = if pills { theme.get_m3_on_surface() } else { wall_ink };
            let ui = &b.ui;
            ui.set_ink(ink);
            ui.set_ink_dark(luma(ink) < 0.5);
            ui.set_wall_ink(wall_ink);
            ui.set_pills(pills);
            ui.set_opaque(opaque);
            ui.set_bg(if material {
                theme.get_m3_surface_container()
            } else if cfg.auto_hide {
                theme.get_bar_tint().with_alpha(0.92)
            } else {
                theme.get_bar_tint()
            });
            ui.set_corners(cfg.screen_corners);
            ui.set_corner_r(cfg.corner_radius as f32);
            // fullscreen and auto-hide
            let over_fs = snapshot.fullscreen_on(&b.output);
            let panel_here = !shell.str("open").is_empty() && (shell.str("screen") == b.output || shell.str("screen").is_empty());
            let menu_here = open_menu.as_ref().is_some_and(|(o, _)| *o == b.output);
            let view = island::view(&self.island, b.hover_open, pinned, over_fs, &fs_mode, b.fs_open);
            let revealed = !cfg.auto_hide || b.hovered || panel_here || menu_here || (cfg.island && event.is_some());
            let chrome = !over_fs || (cfg.auto_hide && (b.hovered || panel_here || menu_here));
            ui.set_revealed(revealed);
            ui.set_chrome(chrome);
            // left
            ui.set_logo(logo.into());
            ui.set_app_menu(cfg.app_menu);
            ui.set_app_name(app_name.as_str().into());
            ui.set_window_menu(cfg.window_menu);
            ui.set_window_title(if cfg.window_title { snapshot.window.title.as_str() } else { "" }.into());
            ui.set_open_menu(
                match &open_menu {
                    Some((o, MenuKind::System)) if *o == b.output => "system",
                    Some((o, MenuKind::App)) if *o == b.output => "app",
                    Some((o, MenuKind::Window)) if *o == b.output => "window",
                    _ => "",
                }
                .into(),
            );
            // centre: the spaces
            let groups = spaces::groups(&b.output, &bases, &snapshot, cfg.workspace_count, cfg.other_spaces, &labels);
            let strip = spaces::strip(&b.output, &groups, &snapshot, cfg.workspace_count, cfg.workspace_overflow == "dynamic", b.start);
            b.start = strip.start;
            let icons = cfg.workspaces == "icons";
            let mut x = 0.0f32;
            let mut chips = Vec::new();
            let mut left_x = 0.0;
            let mut right_x = 0.0;
            if strip.arrows.is_some() {
                left_x = 0.0;
                x = 14.0;
            }
            for (i, c) in strip.chips.iter().enumerate() {
                if i == strip.own_shown && strip.arrows.is_some() {
                    right_x = x;
                    x += 14.0;
                }
                let fall = match c.past {
                    0 => 1.0,
                    1 => 0.7,
                    2 => 0.45,
                    _ => 0.0,
                };
                let apps: Vec<slint::Image> = if icons { c.apps.iter().filter_map(|a| self.app_icon(a)).collect() } else { Vec::new() };
                let with_icons = icons && !apps.is_empty();
                let base_w = if with_icons {
                    apps.len() as f32 * 14.0 + 10.0
                } else if material {
                    if c.active {
                        30.0
                    } else if c.shown_there {
                        22.0
                    } else {
                        14.0
                    }
                } else if c.active || c.shown_there {
                    26.0
                } else {
                    18.0
                };
                let w = base_w * fall;
                let mark_w = if c.mark.is_empty() || fall < 1.0 { 0.0 } else { c.mark.chars().count() as f32 * 5.6 + 10.0 };
                chips.push(ui::Chip { id: c.id, x, w, active: c.active, elsewhere: c.elsewhere, shown_there: c.shown_there, occupied: c.occupied, icons: with_icons, apps: ModelRc::new(VecModel::from(apps)), mark: c.mark.as_str().into(), mark_w, fall });
                if fall > 0.0 {
                    x += mark_w + w + 2.0;
                }
            }
            if strip.arrows.is_some() && strip.own_shown == strip.chips.len() {
                right_x = x;
                x += 14.0;
            }
            let strip_w = (x - 2.0).max(0.0);
            let ids: Vec<i32> = chips.iter().map(|c| c.id).collect();
            // (the same chips: changed in place, so their widths and colours animate)
            let model = ui.get_chips();
            let chips_key = format!("{strip:?}{icons}{material}");
            if chips_key == b.chips_key {
                // nothing new (and a new model would redraw)
            } else if ids == b.chip_ids && model.row_count() == chips.len() {
                for (i, c) in chips.into_iter().enumerate() {
                    model.set_row_data(i, c);
                }
                b.chips_key = chips_key;
            } else {
                b.chips_key = chips_key;
                ui.set_chips(ModelRc::new(VecModel::from(chips)));
                b.chip_ids = ids;
            }
            ui.set_strip_w(strip_w);
            let (l, r) = strip.arrows.clone().unwrap_or_default();
            ui.set_arrows(strip.arrows.is_some());
            ui.set_left_x(left_x);
            ui.set_right_x(right_x);
            ui.set_left_more(l.more);
            ui.set_left_hot(l.hot);
            ui.set_right_more(r.more);
            ui.set_right_hot(r.hot);
            ui.set_spaces_shown(cfg.workspaces != "off" && !cfg.island && chrome);
            // the island
            ui.set_island_on(cfg.island);
            ui.set_island_mode(match view.mode {
                Mode::Idle => 0,
                Mode::Compact => 1,
                Mode::Expanded => 2,
                Mode::Pill => 3,
                Mode::Hidden => 4,
            });
            ui.set_event_inside(view.event_inside && view.mode == Mode::Expanded);
            ui.set_has_card(has_card);
            ui.set_hot(shell.bool("island.hot"));
            ui.set_pinned(pinned);
            ui.set_layout(cfg.activity_layout.as_str().into());
            ui.set_playing(playing);
            ui.set_live_count(live_count);
            ui.set_live_index(live_index);
            ui.set_status(status.as_str().into());
            if let Some(e) = &event {
                let card = &e.event["card"];
                let text = if card.is_object() {
                    format!("{}{}", card["name"].as_str().unwrap_or(""), card["text"].as_str().map(|t| format!(": {}", t.split_whitespace().collect::<Vec<_>>().join(" "))).unwrap_or_default())
                } else {
                    let sub = e.event["subtitle"].as_str().unwrap_or("");
                    format!("{}{}", e.event["title"].as_str().unwrap_or(""), if sub.is_empty() { String::new() } else { format!(" · {sub}") })
                };
                let more = self.island.alerts.len();
                ui.set_pill_text(if more > 1 { format!("{text}  +{}", more - 1) } else { text }.into());
                ui.set_pill_icon(crate::icons::glyph_or(e.event["icon"].as_str().unwrap_or(""), "bell").into());
                ui.set_pill_tint(island::color(e.event["color"].as_str().unwrap_or("")).unwrap_or(theme.get_accent()));
            }
            // (only when they changed: a new model redraws and lays out again)
            // (ModelRc compares by identity: keys of what they show instead)
            let live_key = format!("{material}{:?}{:?}{}{}", self.island.primary(), players.chosen(), self.art.0, self.art.1.is_some());
            if live_key != b.live_key {
                b.ui.set_live(self.live_struct(st, &cfg, material, &theme));
                b.live_key = live_key;
            }
            let alerts_key = format!("{material}{:?}", self.island.alerts);
            if alerts_key != b.alerts_key {
                let alerts = self.alert_rows(material, &theme);
                let old = b.ui.get_alerts();
                // the same alerts changed in place (so their rows don't play their entrance again)
                if old.row_count() == alerts.len() && alerts.iter().enumerate().all(|(i, a)| old.row_data(i).is_some_and(|o| o.uid == a.uid)) {
                    for (i, a) in alerts.into_iter().enumerate() {
                        old.set_row_data(i, a);
                    }
                } else {
                    b.ui.set_alerts(ModelRc::new(VecModel::from(alerts)));
                }
                b.alerts_key = alerts_key;
            }
            // right
            let entries = items::entries(&items::Inputs {
                bar: &cfg,
                material,
                screen: &b.output,
                shell: &shell,
                battery: &bat,
                wifi: &wifi,
                bt: &bt,
                players: &players,
                tray: &tray_now,
                tray_open: self.tray_open,
                tray_menu: &tray_menu,
                clock: &clock,
                clock_long: &clock_long,
                now_ms,
            });
            let rows: Vec<ui::Entry> = entries.iter().map(|e| entry_struct(e, &tray_images)).collect();
            let model = b.ui.get_entries();
            let same = model.row_count() == rows.len() && rows.iter().enumerate().all(|(i, r)| model.row_data(i).is_some_and(|m| m.id == r.id));
            if same {
                for (i, r) in rows.into_iter().enumerate() {
                    if model.row_data(i).as_ref() != Some(&r) {
                        model.set_row_data(i, r);
                    }
                }
            } else {
                b.ui.set_entries(ModelRc::new(VecModel::from(rows)));
            }
            // the surface: layer, reserved zone, height
            let lifted = cfg.island && over_fs && matches!(view.mode, Mode::Expanded | Mode::Pill);
            let layer = if cfg.auto_hide || lifted { Layer::Overlay } else { Layer::Top };
            let exclusive = if cfg.auto_hide { 0 } else { BAR_H as i32 };
            let height = if cfg.island { ISLAND_H } else { TIP_H };
            if layer != b.layer {
                b.surface.layer.set_layer(layer);
                b.layer = layer;
                b.surface.layer.commit();
            }
            if exclusive != b.exclusive {
                b.surface.layer.set_exclusive_zone(exclusive);
                b.exclusive = exclusive;
                b.surface.layer.commit();
            }
            if height != b.height {
                b.height = height;
                b.surface.show(0, height);
            }
        }
        self.bars = bars;
    }

    /// Re-arm the clock: on the minute, or every second when it shows
    /// seconds or something counts (a recording, the island's activities)
    fn tick(&mut self, st: &mut State) {
        self.refresh(st);
        let every_second = st.shell.cfg["menubar"]["showSeconds"].as_bool().unwrap_or(false) || Self::shell_state(st).bool("recording.on");
        let secs = now("%S").parse::<u64>().unwrap_or(0);
        let wait = if every_second { 1000 } else { (60 - secs.min(59)) * 1000 + 50 };
        let tx = st.tx.clone();
        self.clock.start(slint::TimerMode::SingleShot, std::time::Duration::from_millis(wait), move || {
            let _ = tx.send(crate::Msg::Part(NAME, Box::new(M::Tick)));
        });
    }

    /// The island's alerts leave on time (Island.qml's sweep, every 250 ms)
    fn arm_sweep(&mut self, st: &State) {
        if self.island.alerts.is_empty() {
            self.sweep.stop();
            return;
        }
        if !self.sweep.running() {
            let tx = st.tx.clone();
            self.sweep.start(slint::TimerMode::Repeated, std::time::Duration::from_millis(250), move || {
                let _ = tx.send(crate::Msg::Part(NAME, Box::new(M::Sweep)));
            });
        }
    }

    fn apply_theme(&mut self, st: &State) {
        for b in &mut self.bars {
            st.shell.apply_theme(&b.ui.global::<ui::Theme>());
            b.glass_sent.clear();
            b.chips_key.clear();
            b.live_key.clear();
            b.alerts_key.clear();
        }
        if let Some(m) = &mut self.menu {
            st.shell.apply_theme(&m.ui.global::<ui::Theme>());
            m.glass_sent.clear();
        }
    }

    /// `menubar.screens` changed: bars come and go to match
    fn screens_changed(&mut self, st: &mut State) {
        let list = st.shell.cfg["menubar"]["screens"].clone();
        let gone: Vec<String> = self.bars.iter().filter(|b| !crate::surfaces::on_screen(&list, &b.output)).map(|b| b.output.clone()).collect();
        for g in gone {
            self.output_removed(st, &g);
        }
        for (o, n) in st.outputs_now() {
            self.output_added(st, &o, &n);
        }
    }

    // ── The menus ────────────────────────────────────────────────────────

    fn rows_for(&mut self, st: &State, kind: &MenuKind, output: &str) -> Vec<Row> {
        let style = menus::key_style(&st.shell.cfg);
        let snapshot = st.shell.get::<Snapshot>().cloned().unwrap_or_default();
        match kind {
            MenuKind::System => {
                let user = std::env::var("USER").unwrap_or_default();
                menus::system(output, Self::shell_state(st).num("updates") as i32, &user, style)
            }
            MenuKind::App => {
                let class = snapshot.class.clone();
                let name = self.app_name(&class);
                let entry = desktop_entry(&class).and_then(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned())).unwrap_or_default();
                menus::app(&name, &entry, &class)
            }
            MenuKind::Window => menus::window(&snapshot.window, style),
            MenuKind::Space(id) => {
                let shell = Self::shell_state(st);
                let labels = shell.get("spaces.labels");
                let bases = shell.get("spaces.bases");
                let here = snapshot.workspaces.iter().find(|w| w.id == *id).map(|w| w.monitor.clone()).unwrap_or_else(|| output.to_string());
                let displays: Vec<(String, String)> = snapshot
                    .active
                    .keys()
                    .filter(|n| bases.get(n.as_str()).is_some())
                    .map(|n| (n.clone(), bases[n.as_str()].as_i64().and_then(|b| labels[b.to_string()].as_str()).unwrap_or(n).to_string()))
                    .collect();
                menus::space(*id, &here, &displays)
            }
            MenuKind::Tray(..) => Vec::new(), // (rows come with the item's answer)
        }
    }

    fn show_rows(m: &mut MenuEntry, rows: Vec<Row>, material: bool) {
        let groups = menus::groups(&rows);
        let ui_rows: Vec<ui::MenuRow> = rows
            .iter()
            .zip(groups)
            .map(|(r, (first, last))| ui::MenuRow {
                text: r.text.as_str().into(),
                shortcut: r.shortcut.as_str().into(),
                separator: r.separator,
                header: r.header,
                checkable: r.checked.is_some(),
                checked: r.checked == Some(true),
                enabled: r.enabled,
                tiles: ModelRc::new(VecModel::from(r.tiles.iter().map(|t| ui::MenuTile { x: t.frac[0], y: t.frac[1], w: t.frac[2], h: t.frac[3], label: t.label.as_str().into() }).collect::<Vec<_>>())),
                first,
                last,
            })
            .collect();
        m.ui.set_menu_w(if material { menus::material_width(&rows) } else { menus::width(&rows) });
        m.ui.set_any_checks(rows.iter().any(|r| r.checked == Some(true)));
        m.ui.set_rows(ModelRc::new(VecModel::from(ui_rows)));
        m.rows = rows;
        m.ui.set_current(-1);
    }

    fn resize_menu(m: &mut MenuEntry) {
        let w = ((m.ui.get_panel_width() + 32.0).ceil() as u32).max(64);
        let h = (34.0 + m.ui.get_panel_height() + 24.0).ceil() as u32;
        if (w, h) != m.size_sent {
            m.surface.show(w, h);
            m.size_sent = (w, h);
        }
    }

    fn open_menu(&mut self, st: &mut State, output: String, kind: MenuKind, x: f32) {
        // the same one again: it closes (a toggle)
        if self.menu.as_ref().is_some_and(|m| m.open && m.kind == kind && m.output == output) {
            self.close_menu(st);
            return;
        }
        if self.menu.as_ref().is_some_and(|m| m.output != output) {
            let m = self.menu.take().unwrap();
            st.drop_surface(&m.surface);
        }
        if self.menu.is_none() {
            let Some(wl_output) = st.output_named(&output) else { return };
            let surface = st.prepare_surface(Spec {
                layer: Layer::Overlay,
                namespace: MENU_NS,
                output: Some((&wl_output, &output)),
                anchor: Anchor::TOP | Anchor::LEFT,
                size: (292, 1),
                // (from the screen's top, over the bar's reserved strip)
                exclusive: -1,
                keyboard: KeyboardInteractivity::OnDemand,
                margin_left: 0,
            });
            let ui = ui::Menu::new().expect("Menu");
            st.shell.apply_theme(&ui.global::<ui::Theme>());
            ui.global::<ui::Symbols>().on_of(|g| crate::icons::symbol(&g).into());
            let to = st.shell.sender(NAME);
            let (to2, to3, to4, to5) = (to.clone(), to.clone(), to.clone(), to.clone());
            ui.on_activated(move |i| to(Box::new(M::MenuActivated(i))));
            ui.on_tile(move |i, j| to5(Box::new(M::MenuTile(i, j))));
            ui.on_step(move |d| to4(Box::new(M::MenuStep(d))));
            ui.on_dismissed(move || to2(Box::new(M::MenuDismissed)));
            ui.on_closed(move || to3(Box::new(M::MenuClosed)));
            ui.show().expect("show menu");
            let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
            self.menu = Some(MenuEntry { output: output.clone(), surface, ui, a11y, open: false, glass_sent: String::new(), kind: kind.clone(), rows: Vec::new(), region_sent: String::new(), size_sent: (0, 0) });
        }
        let rows = self.rows_for(st, &kind, &output);
        let material = st.shell.look.material;
        let m = self.menu.as_mut().unwrap();
        m.kind = kind.clone();
        Self::show_rows(m, rows, material);
        // (the surface starts 16 px left of the panel, room for its shadow)
        let screen_w = self.bars.iter().find(|b| b.output == output).map_or(1920.0, |b| b.surface.logical_size().0 as f32);
        let px = menus::panel_x(x, m.ui.get_panel_width(), screen_w, material);
        let left = (px - 16.0).max(0.0);
        m.surface.layer.set_margin(0, 0, 0, left as i32);
        m.ui.set_panel_left(px - left.floor());
        if let MenuKind::Tray(key, path) = &kind {
            self.tray_menu = key.clone();
            tray::menu(key, path, st.shell.sender(NAME));
        }
        let m = self.menu.as_mut().unwrap();
        // (shown afresh: hidden, it needs its size again to be mapped)
        m.size_sent = (0, 0);
        m.region_sent.clear();
        Self::resize_menu(m);
        m.open = true;
        m.ui.set_shown(true);
        m.ui.invoke_open();
        self.refresh(st);
    }

    fn close_menu(&mut self, st: &mut State) {
        if let Some(m) = &mut self.menu {
            if m.open {
                m.open = false;
                m.ui.set_shown(false); // the close animation, then MenuClosed hides it
            }
        }
        self.tray_menu.clear();
        self.refresh(st);
    }

    fn menu_closed(&mut self, st: &mut State) {
        if let Some(m) = &mut self.menu {
            if !m.open {
                m.surface.hide();
                st.shell.glass.set(&m.output, MENU_NS, &[]);
                m.glass_sent.clear();
            }
        }
    }

    fn run(&mut self, st: &mut State, a: Action) {
        match a {
            Action::None => {}
            Action::Act(what, arg) => bridge::call(&["rustui", "act", what, &arg]),
            Action::Ipc(v) => bridge::call(&v.iter().map(String::as_str).collect::<Vec<_>>()),
            Action::Dispatch(d) => hypr::dispatch(d),
            Action::Tray(id) => {
                if let Some(MenuKind::Tray(key, path)) = self.menu.as_ref().map(|m| m.kind.clone()) {
                    tray::menu_event(&key, &path, id);
                }
            }
            Action::TraySub(id) => {
                if let Some(MenuKind::Tray(key, path)) = self.menu.as_ref().map(|m| m.kind.clone()) {
                    tray::submenu(&key, &path, id, st.shell.sender(NAME));
                }
            }
        }
    }

    fn menu_activated(&mut self, st: &mut State, i: i32) {
        let Some(row) = self.menu.as_ref().and_then(|m| m.rows.get(i as usize).cloned()) else { return };
        if row.separator || row.header || !row.enabled {
            return;
        }
        if !row.keep_open {
            self.close_menu(st);
        }
        self.run(st, row.action);
    }

    // ── What the bar's items do ──────────────────────────────────────────

    fn toggle_at(output: &str, panel: &str, x: f32) {
        bridge::call(&["rustui", "act", "toggleAt", &format!("{panel} {output} {}", x.round())]);
    }

    fn clicked(&mut self, st: &mut State, output: String, id: String, right: bool, x: f32) {
        if self.menu.as_ref().is_some_and(|m| m.open) {
            self.close_menu(st);
        }
        let act = |what: &str, arg: &str| bridge::call(&["rustui", "act", what, arg]);
        if let Some(key) = id.strip_prefix("tray:") {
            let Some(item) = st.shell.get::<Tray>().and_then(|t| t.items.iter().find(|i| i.key == key).cloned()) else { return };
            if !right && !item.item_is_menu {
                tray::activate(key, 0, 0);
            } else if !item.menu.is_empty() {
                self.open_menu(st, output, MenuKind::Tray(item.key.clone(), item.menu.clone()), x);
            }
            return;
        }
        if let Some(b) = id.strip_prefix("util:") {
            match b {
                "screenshot" => Self::toggle_at(&output, "screenTools", x),
                "keyboard" => Self::toggle_at(&output, "keyboard", x),
                "colorPicker" => bridge::run(&["hyprpicker", "-a"]),
                "mic" => act("mic", ""),
                "darkMode" => act("darkMode", ""),
                "powerProfile" => act("powerProfile", ""),
                "record" => act("record", ""),
                _ => {}
            }
            return;
        }
        match id.as_str() {
            "recording" => act("stopRecording", ""),
            "shelf" => act("shelf", &output),
            "caffeine" => act("page", &format!("caffeine {output}")),
            "focus" => act("page", &format!("focus {output}")),
            "keyboard" => act("keyboardLayout", ""),
            "trayArrow" => {
                self.tray_open = !self.tray_open;
                // (tucked away again a while later, unless one of their menus is open)
                let tx = st.tx.clone();
                self.tray_timer.start(slint::TimerMode::SingleShot, std::time::Duration::from_secs(12), move || {
                    let _ = tx.send(crate::Msg::Part(NAME, Box::new(M::TrayCollapse)));
                });
                self.refresh(st);
            }
            "nowPlaying" => Self::toggle_at(&output, "nowPlaying", x),
            "resources" => Self::toggle_at(&output, "resourcesMenu", x),
            "weather" => Self::toggle_at(&output, "weatherMenu", x),
            "battery" => Self::toggle_at(&output, "batteryMenu", x),
            "bluetooth" => Self::toggle_at(&output, "bluetoothMenu", x),
            "wifi" => Self::toggle_at(&output, "wifiMenu", x),
            "sound" => Self::toggle_at(&output, "soundMenu", x),
            "privacy" => Self::toggle_at(&output, "privacyMenu", x),
            "ai" => Self::toggle_at(&output, "sidebar", x),
            "search" => Self::toggle_at(&output, "spotlight", x),
            "controlCenter" => Self::toggle_at(&output, "controlCenter", x),
            "clock" => Self::toggle_at(&output, "notificationCenter", x),
            _ => {}
        }
    }

    /// Spaces.go: a space that isn't there yet is made on its own display
    fn go(&self, st: &State, id: i32) {
        let snapshot = st.shell.get::<Snapshot>().cloned().unwrap_or_default();
        let exists = snapshot.workspaces.iter().any(|w| w.id == id);
        let shell = Self::shell_state(st);
        let owner = shell.get("spaces.bases").as_object().and_then(|o| o.iter().find(|(_, b)| b.as_i64() == Some(spaces::base_of(id) as i64)).map(|(n, _)| n.clone()));
        match owner {
            Some(o) if !exists => bridge::run(&["hyprctl", "eval", &format!("hl.dispatch(hl.dsp.focus({{ monitor = {o:?} }}))\nhl.dispatch(hl.dsp.focus({{ workspace = {id} }}))\n")]),
            _ => hypr::focus_workspace(id),
        }
    }

    /// The left half of the bar: brightness (with a backlight); the right: volume
    fn scrolled(&self, st: &State, left: bool, dy: f32) {
        if !Self::cfg(st).scroll_gestures {
            return;
        }
        let up = dy > 0.0;
        if left {
            let has_backlight = std::fs::read_dir("/sys/class/backlight").map(|mut d| d.next().is_some()).unwrap_or(false);
            if has_backlight {
                bridge::run(&["brightnessctl", "-q", "set", if up { "5%+" } else { "5%-" }]);
            }
        } else {
            // (the QML shell's sound service sees the change: its OSD or the island's level)
            bridge::run(&["wpctl", "set-volume", "-l", "1.0", "@DEFAULT_AUDIO_SINK@", if up { "5%+" } else { "5%-" }]);
        }
    }

    fn qml_uid(&self, uid: i32) -> i64 {
        self.island.alerts.iter().find(|a| a.uid == uid as u64).map(|a| a.qml_uid).unwrap_or(0)
    }

    /// The HUD showing: volume | mic | brightness
    fn hud_kind(&self) -> Option<String> {
        self.island.alerts.iter().find(|a| a.kind() == "hud").map(|a| a.event["hud"].as_str().unwrap_or("volume").to_string())
    }

    fn island_control(&mut self, st: &mut State, i: i32) {
        let Some(a) = self.island.primary().cloned() else { return };
        if a.kind == "music" {
            let Some(p) = st.shell.get::<Players>().and_then(|p| p.chosen().cloned()) else { return };
            let cfg = Self::cfg(st);
            let mut cmds = Vec::new();
            for (piece, cmd) in [("prev", "Previous"), ("play", "PlayPause"), ("next", "Next")] {
                if cfg.shows("music", piece) {
                    cmds.push(cmd);
                }
            }
            if let Some(cmd) = cmds.get(i as usize) {
                mpris::control(&p.bus, cmd);
            }
        } else {
            bridge::call(&["rustui", "act", "live", &format!("{} {i}", a.kind)]);
        }
    }

    fn handle(&mut self, st: &mut State, m: M) {
        match m {
            M::Tick => self.tick(st),
            M::Sweep => {
                if self.island.sweep(ms_now()) {
                    self.refresh(st);
                }
                self.arm_sweep(st);
            }
            M::MusicLinger => {
                self.update_activities(st);
                self.refresh(st);
            }
            M::TrayCollapse => {
                if self.tray_menu.is_empty() && self.tray_open {
                    self.tray_open = false;
                    self.refresh(st);
                }
            }
            M::Menu(o, kind, x) => {
                let k = match kind.as_str() {
                    "app" => MenuKind::App,
                    "window" => MenuKind::Window,
                    _ => MenuKind::System,
                };
                self.open_menu(st, o, k, x);
            }
            M::Clicked(o, id, right, x) => self.clicked(st, o, id, right, x),
            M::MenuActivated(i) => self.menu_activated(st, i),
            M::MenuTile(i, j) => {
                let a = self.menu.as_ref().and_then(|m| m.rows.get(i as usize)).and_then(|r| r.tiles.get(j as usize)).map(|t| t.action.clone());
                if let Some(a) = a {
                    self.close_menu(st);
                    self.run(st, a);
                }
            }
            M::MenuStep(d) => {
                if let Some(m) = &self.menu {
                    let next = menus::step(&m.rows, m.ui.get_current(), d);
                    m.ui.set_current(next);
                }
            }
            M::MenuDismissed => self.close_menu(st),
            M::MenuClosed => self.menu_closed(st),
            M::Go(id) => self.go(st, id),
            M::SpaceMenu(o, id, x) => self.open_menu(st, o, MenuKind::Space(id), x),
            M::Arrow(o, d) => {
                if let Some(b) = self.bars.iter_mut().find(|b| b.output == o) {
                    b.start += d;
                }
                self.refresh(st);
            }
            M::SpaceWheel(d) => hypr::dispatch(format!("hl.dsp.focus({{ workspace = \"m{}1\" }})", if d > 0 { "+" } else { "-" })),
            M::Scrolled(left, dy) => self.scrolled(st, left, dy),
            M::IslandHover(o, on) => {
                if let Some(b) = self.bars.iter_mut().find(|b| b.output == o) {
                    b.hover_open = on;
                }
                self.refresh(st);
            }
            M::IslandPill(o) => {
                if let Some(b) = self.bars.iter_mut().find(|b| b.output == o) {
                    b.fs_open = true;
                }
                self.refresh(st);
            }
            M::IslandCycle(dx, dy) => {
                // (Slint's wheel: logical px, up positive; Qt's angleDelta: 120 a notch)
                if self.island.scroll(dx as f64 * 8.0, dy as f64 * 8.0, ms_now()) != 0 {
                    self.refresh(st);
                }
            }
            M::IslandControl(i) => self.island_control(st, i),
            // the HUD's symbol: mute (not brightness)
            M::IslandAct(_, which) if which == "mute" => match self.hud_kind().as_deref() {
                Some("brightness") => {}
                Some("mic") => bridge::run(&["wpctl", "set-mute", "@DEFAULT_AUDIO_SOURCE@", "toggle"]),
                _ => bridge::run(&["wpctl", "set-mute", "@DEFAULT_AUDIO_SINK@", "toggle"]),
            },
            // the HUD's button: the outputs or inputs (the shell's Sound menu
            // under it), the displays (the Control Center's brightness)
            M::IslandAct(_, which) if which.starts_with("levels ") => {
                let x: f32 = which[7..].parse().unwrap_or(0.0);
                let output = self.bars.iter().find(|b| b.hovered).or(self.bars.first()).map(|b| b.output.clone()).unwrap_or_default();
                if self.hud_kind().as_deref() == Some("brightness") {
                    bridge::call(&["rustui", "act", "page", &format!(" {output}")]);
                } else {
                    bridge::call(&["rustui", "act", "toggleAt", &format!("soundMenu {output} {}", x.round())]);
                }
            }
            M::IslandAct(uid, which) => {
                let q = self.qml_uid(uid);
                bridge::call(&["rustui", "act", "islandAct", &format!("{q} {which}")]);
                self.island.dismiss(ms_now());
                self.refresh(st);
            }
            M::IslandDismiss(uid) => {
                let q = self.qml_uid(uid);
                self.island.remove(uid as u64);
                if q != 0 {
                    bridge::call(&["rustui", "act", "islandRemove", &q.to_string()]);
                }
                self.refresh(st);
            }
            M::IslandLevel(v) => {
                let v = v.clamp(0.0, 1.0);
                match self.hud_kind().as_deref() {
                    Some("brightness") => bridge::run(&["brightnessctl", "-q", "set", &format!("{}%", (v * 100.0).round().max(1.0))]),
                    Some("mic") => bridge::run(&["wpctl", "set-volume", "@DEFAULT_AUDIO_SOURCE@", &format!("{v:.2}")]),
                    _ => bridge::run(&["wpctl", "set-volume", "-l", "1.0", "@DEFAULT_AUDIO_SINK@", &format!("{v:.2}")]),
                }
            }
            M::IslandPin => bridge::call(&["rustui", "act", "islandPin", ""]),
            M::IslandHold(on) => {
                self.island.hold(on, ms_now());
                bridge::call(&["rustui", "act", "islandHold", if on { "1" } else { "0" }]);
                self.arm_sweep(st);
            }
            M::Art(url, path) => {
                if url == self.art.0 {
                    self.art.1 = path.as_deref().and_then(load);
                    self.refresh(st);
                }
            }
        }
    }

    /// The input region: the bar strip (or its 2 px edge while hidden), the island
    fn region(b: &BarEntry, cfg: &settings::Bar) -> Vec<(i32, i32, i32, i32)> {
        let (w, _) = b.surface.logical_size();
        let ui = &b.ui;
        let mut r = Vec::new();
        let chrome = ui.get_chrome();
        let revealed = ui.get_revealed();
        if revealed && chrome {
            r.push((0, 0, w as i32, BAR_H as i32));
        } else if cfg.auto_hide {
            r.push((0, 0, w as i32, 2));
        }
        if cfg.island && revealed && ui.get_island_mode() != 4 {
            r.push((ui.get_isl_x().floor() as i32 - 4, 0, ui.get_isl_w().ceil() as i32 + 8, (ui.get_isl_y() + ui.get_isl_h()).ceil() as i32 + 4));
        }
        r
    }
}

fn entry_struct(e: &items::Entry, tray_images: &HashMap<String, slint::Image>) -> ui::Entry {
    let mut out = ui::Entry {
        id: e.id.as_str().into(),
        icon: e.icon.into(),
        fill: e.fill,
        bold: e.bold,
        text: e.text.as_str().into(),
        tip: e.tip.as_str().into(),
        danger: e.tone == items::Tone::Danger,
        active: e.active,
        badge: e.badge,
        indicator: e.indicator.as_str().into(),
        unread: e.unread,
        ..Default::default()
    };
    match &e.glyph {
        items::Glyph::Icon => {}
        items::Glyph::Battery { level, charging, plugged, low_power, show_percent } => {
            out.glyph = 1;
            out.level = *level;
            out.charging = *charging;
            out.plugged = *plugged;
            out.low_power = *low_power;
            out.show_percent = *show_percent;
        }
        items::Glyph::Wifi { on, connected, strength } => {
            out.glyph = 2;
            out.wifi_on = *on;
            out.wifi_lit = if !connected {
                ""
            } else if *strength > 0.66 {
                glyph("wifi-high")
            } else if *strength > 0.33 {
                glyph("wifi-medium")
            } else {
                glyph("wifi-low")
            }
            .into();
        }
        items::Glyph::Privacy([camera, mic, location, share]) => {
            out.glyph = 3;
            out.camera = *camera;
            out.mic = *mic;
            out.location = *location;
            out.share = *share;
        }
        items::Glyph::Tray(key) => {
            out.glyph = 4;
            out.image = tray_images.get(key).cloned().unwrap_or_default();
        }
    }
    out
}

impl Part for Menubar {
    fn name(&self) -> &'static str {
        NAME
    }

    fn start(&mut self, st: &mut State) {
        for s in ["hypr", "status", "bluez", "mpris", "tray"] {
            st.shell.want(s);
        }
        self.follow_wallpaper(st);
        self.tick(st);
    }

    fn output_added(&mut self, st: &mut State, output: &WlOutput, name: &str) {
        if self.bars.iter().any(|b| b.output == name) || !crate::surfaces::on_screen(&st.shell.cfg["menubar"]["screens"], name) {
            return;
        }
        let cfg = Self::cfg(st);
        let height = if cfg.island { ISLAND_H } else { TIP_H };
        let exclusive = if cfg.auto_hide { 0 } else { BAR_H as i32 };
        let layer = if cfg.auto_hide { Layer::Overlay } else { Layer::Top };
        let surface = st.prepare_surface(Self::spec(output, name, height, exclusive, layer));
        let ui = ui::Bar::new().expect("Bar");
        st.shell.apply_theme(&ui.global::<ui::Theme>());
        ui.global::<ui::Symbols>().on_of(|g| crate::icons::symbol(&g).into());
        let out = name.to_string();
        let to = st.shell.sender(NAME);
        {
            let (to, o) = (to.clone(), out.clone());
            ui.on_menu(move |k, x| to(Box::new(M::Menu(o.clone(), k.to_string(), x))));
        }
        {
            let (to, o) = (to.clone(), out.clone());
            ui.on_clicked(move |id, b, x| to(Box::new(M::Clicked(o.clone(), id.to_string(), b == slint::platform::PointerEventButton::Right, x))));
        }
        {
            let to = to.clone();
            ui.on_go(move |id| to(Box::new(M::Go(id))));
        }
        {
            let (to, o) = (to.clone(), out.clone());
            ui.on_space_menu(move |id, x| to(Box::new(M::SpaceMenu(o.clone(), id, x))));
        }
        {
            let (to, o) = (to.clone(), out.clone());
            ui.on_arrow(move |d| to(Box::new(M::Arrow(o.clone(), d))));
        }
        {
            let to = to.clone();
            ui.on_space_wheel(move |d| to(Box::new(M::SpaceWheel(d))));
        }
        {
            let to = to.clone();
            ui.on_scrolled(move |left, dy| to(Box::new(M::Scrolled(left, dy))));
        }
        {
            let (to, o) = (to.clone(), out.clone());
            ui.on_island_hover(move |on| to(Box::new(M::IslandHover(o.clone(), on))));
        }
        {
            let (to, o) = (to.clone(), out.clone());
            ui.on_island_pill(move || to(Box::new(M::IslandPill(o.clone()))));
        }
        {
            let to = to.clone();
            ui.on_island_cycle(move |dx, dy| to(Box::new(M::IslandCycle(dx, dy))));
        }
        {
            let to = to.clone();
            ui.on_island_control(move |i| to(Box::new(M::IslandControl(i))));
        }
        {
            let to = to.clone();
            ui.on_island_act(move |u, w| to(Box::new(M::IslandAct(u, w.to_string()))));
        }
        {
            let to = to.clone();
            ui.on_island_dismiss(move |u| to(Box::new(M::IslandDismiss(u))));
        }
        {
            let to = to.clone();
            ui.on_island_level(move |v| to(Box::new(M::IslandLevel(v))));
        }
        {
            let to = to.clone();
            ui.on_island_pin(move || to(Box::new(M::IslandPin)));
        }
        {
            let to = to.clone();
            ui.on_island_hold(move |on| to(Box::new(M::IslandHold(on))));
        }
        ui.show().expect("show bar");
        let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
        self.bars.push(BarEntry {
            output: out,
            surface,
            ui,
            a11y,
            glass_sent: String::new(),
            region_sent: String::new(),
            layer,
            exclusive,
            height,
            hovered: false,
            hover_open: false,
            fs_open: false,
            start: 0,
            chip_ids: Vec::new(),
            chips_key: String::new(),
            live_key: String::new(),
            alerts_key: String::new(),
        });
        self.refresh(st);
    }

    fn output_removed(&mut self, st: &mut State, name: &str) {
        if let Some(i) = self.bars.iter().position(|b| b.output == name) {
            let b = self.bars.remove(i);
            st.drop_surface(&b.surface);
        }
        if self.menu.as_ref().is_some_and(|m| m.output == name) {
            let m = self.menu.take().unwrap();
            st.drop_surface(&m.surface);
        }
    }

    fn config_changed(&mut self, st: &mut State, look: bool) {
        self.screens_changed(st);
        self.follow_wallpaper(st);
        self.tick(st); // (the clock's format, the menu icon, the background)
        if look {
            self.apply_theme(st);
            self.refresh(st);
        }
    }

    fn plugin_changed(&mut self, st: &mut State) {
        self.apply_theme(st);
    }

    fn data(&mut self, st: &mut State, what: TypeId) {
        if what == TypeId::of::<IslandCall>() {
            let Some(c) = st.shell.get::<IslandCall>().cloned() else { return };
            let now = ms_now();
            match c.op.as_str() {
                "post" => {
                    self.island.post(c.event.clone(), c.ms as f64, c.uid, now);
                }
                "update" => self.island.update(c.uid, &c.event),
                "remove" => self.island.remove_qml(c.uid),
                "dismiss" => self.island.dismiss(c.event["since"].as_f64().unwrap_or(0.0)),
                _ => {}
            }
            // a new alert closes the compact pill over a fullscreen app
            for b in &mut self.bars {
                if c.op == "post" {
                    b.fs_open = false;
                }
            }
            self.arm_sweep(st);
            self.refresh(st);
            return;
        }
        if what == TypeId::of::<LiveActivities>() {
            self.live = st.shell.get::<LiveActivities>().map(|l| l.0.iter().map(Activity::from_json).filter(|a| a.kind != "music").collect()).unwrap_or_default();
            self.update_activities(st);
            self.refresh(st);
            return;
        }
        if what == TypeId::of::<Players>() {
            let playing = st.shell.get::<Players>().and_then(|p| p.chosen()).is_some_and(|p| p.playing);
            if self.was_playing && !playing {
                // paused: music stays a minute (Island.musicLingers)
                self.linger_until = ms_now() + 60_000.0;
                let tx = st.tx.clone();
                self.linger.start(slint::TimerMode::SingleShot, std::time::Duration::from_millis(60_100), move || {
                    let _ = tx.send(crate::Msg::Part(NAME, Box::new(M::MusicLinger)));
                });
            }
            self.was_playing = playing;
            self.follow_art(st);
            self.update_activities(st);
        }
        if what == TypeId::of::<ShellState>() {
            // (a recording's time counts each second)
            self.tick(st);
            return;
        }
        if [TypeId::of::<Snapshot>(), TypeId::of::<Battery>(), TypeId::of::<Wifi>(), TypeId::of::<Bluetooth>(), TypeId::of::<Players>(), TypeId::of::<Tray>(), TypeId::of::<WallTop>()].contains(&what) {
            self.refresh(st);
        }
    }

    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        let msg = match msg.downcast::<TrayMenu>() {
            Ok(t) => {
                // a tray item's menu (or one of its submenus) came back
                let material = st.shell.look.material;
                if let Some(m) = self.menu.as_mut().filter(|m| m.open && matches!(&m.kind, MenuKind::Tray(k, _) if *k == t.key)) {
                    Self::show_rows(m, menus::tray(&t.root), material);
                    Self::resize_menu(m);
                }
                return;
            }
            Err(m) => m,
        };
        let Ok(m) = msg.downcast::<M>() else { return };
        self.handle(st, *m);
    }

    fn keyboard_left(&mut self, st: &mut State, s: &Rc<Surface>) {
        // Clicking another window takes the keyboard: the menu closes, as on macOS
        if self.menu.as_ref().is_some_and(|m| m.open && Rc::ptr_eq(&m.surface, s)) {
            self.close_menu(st);
        }
    }

    fn surface_closed(&mut self, st: &mut State, s: &Rc<Surface>) {
        let name = s.output.clone();
        self.output_removed(st, &name);
    }

    /// The pointer (auto-hide), the input region, the glass shapes for the
    /// plugin (the strip, the island, the tip, the menu), then accessibility
    fn after_frame(&mut self, st: &mut State) {
        let cfg = Self::cfg(st);
        let mut changed = false;
        for b in &mut self.bars {
            let h = b.ui.get_hovered();
            if h != b.hovered {
                b.hovered = h;
                changed |= cfg.auto_hide;
            }
        }
        if changed {
            self.refresh(st);
        }
        for b in &mut self.bars {
            if !b.surface.mapped.get() {
                continue;
            }
            let r = Self::region(b, &cfg);
            let key = format!("{r:?}");
            if key != b.region_sent {
                st.set_input_region(&b.surface, Some(&r));
                b.region_sent = key;
            }
        }
        let kbd = st.keyboard_surface();
        for b in &mut self.bars {
            let f = kbd.as_ref().is_some_and(|k| Rc::ptr_eq(k, &b.surface));
            if let Some(a) = b.a11y.as_mut() {
                a.update(false, f);
            }
        }
        if let Some(m) = self.menu.as_mut() {
            let f = kbd.as_ref().is_some_and(|k| Rc::ptr_eq(k, &m.surface));
            if let Some(a) = m.a11y.as_mut() {
                a.update(false, f);
            }
            if m.open && m.surface.mapped.get() {
                // its rows are laid out once drawn: fit the window to them
                Self::resize_menu(m);
                // only the panel takes the pointer (the bar stays clickable above it)
                let ui = &m.ui;
                let r = [(ui.get_panel_x() as i32, ui.get_panel_y().max(34.0) as i32, ui.get_panel_width().ceil() as i32, ui.get_panel_height().ceil() as i32)];
                let key = format!("{r:?}");
                if key != m.region_sent {
                    st.set_input_region(&m.surface, Some(&r));
                    m.region_sent = key;
                }
            }
        }
        if !st.shell.plugin || st.shell.look.material {
            return;
        }
        let look = st.shell.look.clone();
        let strip = cfg.background || cfg.auto_hide;
        for b in &mut self.bars {
            if !b.surface.mapped.get() {
                continue;
            }
            let ui = &b.ui;
            let (w, _) = b.surface.logical_size();
            let mut shapes: Vec<[f32; 19]> = Vec::new();
            if strip && ui.get_chrome() && ui.get_revealed() {
                shapes.push(theme::glass_shape(&look, 0.0, 0.0, w as f32, BAR_H as f32, 0.0, 1.0, 1.0));
            }
            // the island: glass under its lower rim as it opens
            let fade = ui.get_isl_fade();
            if cfg.island && fade > 0.01 && ui.get_revealed() {
                shapes.push(theme::glass_shape(&look, ui.get_isl_x(), ui.get_isl_y(), ui.get_isl_w(), ui.get_isl_h(), ui.get_isl_r(), fade, 1.0));
            }
            // the hover tip
            let o = ui.get_tip_o();
            if o > 0.01 {
                shapes.push(theme::glass_shape(&look, ui.get_tip_x(), ui.get_tip_y(), ui.get_tip_w(), ui.get_tip_h(), 10.0, o, 0.35));
            }
            let key = format!("{shapes:?}");
            if key != b.glass_sent {
                st.shell.glass.set(&b.output, BAR_NS, &shapes);
                b.glass_sent = key;
            }
        }
        if let Some(m) = &mut self.menu {
            if !m.surface.mapped.get() {
                return;
            }
            let ui = &m.ui;
            let open = ui.get_openness();
            let s = ui.get_panel_scale();
            let (px, py, pw, ph) = (ui.get_panel_x(), ui.get_panel_y(), ui.get_panel_width(), ui.get_panel_height());
            // scaled about (18, 0) in the panel, as in menubar.slint
            let x = px + 18.0 * (1.0 - s);
            let opacity = (open * 1.6).min(1.0);
            let shape = theme::glass_shape(&look, x, py, pw * s, ph * s, 14.0 * s, opacity, 0.35);
            let key = format!("{shape:?}");
            if key != m.glass_sent {
                st.shell.glass.set(&m.output, MENU_NS, if opacity > 0.002 { std::slice::from_ref(&shape) } else { &[] });
                m.glass_sent = key;
            }
        }
    }

    fn a11y(&mut self) -> Vec<&mut crate::a11y::A11y> {
        let mut v: Vec<&mut crate::a11y::A11y> = self.bars.iter_mut().filter_map(|b| b.a11y.as_mut()).collect();
        if let Some(m) = self.menu.as_mut().and_then(|m| m.a11y.as_mut()) {
            v.push(m);
        }
        v
    }

    fn stats(&self) {
        for b in &self.bars {
            eprintln!("tessera-ui: stats {}", b.surface.stats.borrow_mut().report(&format!("bar:{}", b.output)));
        }
        if let Some(m) = &self.menu {
            eprintln!("tessera-ui: stats {}", m.surface.stats.borrow_mut().report("menu"));
        }
    }

    /// On the way out: no glass left behind
    fn quit(&mut self, _: &mut State) {
        for b in &self.bars {
            Glass::clear_now(&b.output, BAR_NS);
        }
        if let Some(m) = &self.menu {
            Glass::clear_now(&m.output, MENU_NS);
        }
    }
}

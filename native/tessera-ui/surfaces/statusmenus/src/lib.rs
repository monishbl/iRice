//! The menu bar's status menus (rust.surfaces: statusmenus): Bluetooth,
//! Sound, privacy ("In use"), battery, resources and weather, and Now
//! Playing, each a popover under its menu bar item. Compare
//! shell/modules/menubar/StatusMenus.qml (StatusMenu: a glass Popover with
//! lens 0.35, autoClose 5000, `targetX: Panels.anchorX - 20`, `width_`) and
//! shell/modules/menubar/NowPlaying.qml (320 wide, `Panels.anchorX - 120`).
//! The Wi-Fi menu is the network surface's.
//!
//! One surface, seven panels: Panels names the open one ("bluetoothMenu",
//! "soundMenu", "privacyMenu", "batteryMenu", "resourcesMenu",
//! "weatherMenu", "nowPlaying"); its `open` command carries `panel` and
//! `anchorX` (the host's panels.rs and RustUi.qml's follow), and a click
//! outside, Escape or 5 s without the pointer close it (`closed`).
//!
//! What it shows:
//! - Bluetooth: the Control Center's Bluetooth page (its PageView and
//!   blocks), from services::bluez and the shell's Backends state; a row
//!   connects, disconnects or pairs (bluetoothctl), right-click for its menu.
//! - Sound: the Sound slider, then the Control Center's Sound page without
//!   its title (outputs, the card-switching outputs, apps, the microphone
//!   when Settings says), from services::audio.
//! - Battery: services::status' battery; its health (BatteryHealth), the
//!   energy mode (Power, power-profiles-daemon, or who manages power).
//! - Privacy: Privacy's apps and shared screens; Stop sharing.
//! - Resources: Resources' CPU, memory and swap.
//! - Weather: services::weather.
//! - Now Playing: services::mpris; the player Media shows (`media.bus`), the
//!   others to switch to.
//! What only the shell knows reaches it as `{"to":"statusmenus", "health":…,
//! "power":…, "privacy":…, "resources":…, "profiles":…, "btPictures":…}`
//! (host/system's StatusMenusFeed; beside the QML shell,
//! modules/menubar/StatusMenusFeed.qml), and what it asks goes there:
//! `shell::ask("statusmenus", [what, …])`: stopSharing, mode <profile>,
//! profile <i> (Sound's card-switching outputs), btFix, pin <bus>.
//!
//! Differences from the QML:
//! - Material's album art rounds into a circle while it plays (Slint can't
//!   clip a picture to the cookie shape); its play button morphs between a
//!   circle and a rounded square without MaterialShape's spring.
//! - Battery health reads only the kernel's numbers (BatteryHealth), not
//!   UPower's health percentage; without a battery the menu says so.
//! - The keyboard: Escape closes (a device's menu first); no Tab chain.
//! - Wrapped text gets an estimated height (Slint measures wrapped text in a
//!   layout as one line); no tabular figures.
//! - Now Playing seeks whenever the track has a length (core's mpris reading
//!   has no CanSeek).

pub mod ui {
    slint::include_modules!();
}
tessera_core::theme_global!(ui);

// tessera-core's modules (shell, wayland, theme, services, util…) under the
// names they had when everything was one crate: `crate::shell::Part`
#[allow(unused_imports)]
use tessera_core::*;

pub mod model;

use model::BtAct;
use serde_json::{json, Value};
use slint::platform::Key;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use smithay_client_toolkit::shell::WaylandSurface;
use std::any::{Any, TypeId};
use std::collections::HashSet;
use std::rc::Rc;
use std::time::{Duration, Instant};
use tessera_controlcenter::bridge::{self, Block, BlockKind, Page, Profile, Tone};
use tessera_core::services::audio::{self, Cmd, Sound};
use tessera_core::services::bluez::Bluetooth;
use tessera_core::services::bridge::ShellState;
use tessera_core::services::hypr::Glass;
use tessera_core::services::mpris::{self, Player, Players};
use tessera_core::services::status::Battery;
use tessera_core::services::weather::Weather;
use tessera_core::shell::{self, Command, Part};
use tessera_core::theme;
use tessera_core::wayland::{Spec, State, Surface};

const NAME: &str = "statusmenus";
pub const NS: &str = "tessera:statusmenu";
/// Popover.qml's autoClose for the status menus
const AUTO_CLOSE_MS: u64 = 5000;

enum M {
    Escape,
    Hovered(bool),
    AutoClose,
    Dismissed,
    Closed,
    PageToggled,
    PageRefreshed,
    PageBlock(i32, String),
    PageMenu(i32, f32, f32),
    SoundBlock(i32, String),
    SoundSlide(i32, f32),
    Volume(f32),
    Mode(i32),
    StopSharing,
    Np(String),
    Seek(f32),
    Pick(i32),
    MenuPick(i32),
    MenuClose,
    /// bluetoothctl finished with a device
    BtDone(String),
    /// the playing track's position (seconds), read
    Position(String, f64),
    /// a picture for an art address
    Art(String, Option<std::path::PathBuf>),
    Tick,
}

struct Win {
    output: String,
    surface: Rc<Surface>,
    ui: ui::StatusMenus,
    a11y: Option<tessera_core::a11y::A11y>,
    glass_sent: String,
    region: (u32, u32),
}

#[derive(Default)]
pub struct StatusMenus {
    win: Option<Win>,
    open: bool,
    /// the open menu (Panels' name)
    panel: String,
    anchor_x: f32,
    hovered: bool,
    /// a device's menu: its address and entries
    menu: Option<(String, Vec<Option<(&'static str, BtAct, bool)>>)>,
    bt_busy: HashSet<String>,
    /// what the shell's feed said last (health, power, privacy, resources, profiles, btPictures)
    feed: Value,
    shown_bt: Page,
    shown_sound: Page,
    /// Now Playing: the player picked here (until the shell says `media.bus`)
    pinned: String,
    /// the position (seconds) when read, and when
    position: (String, f64, Option<Instant>),
    art: (String, Option<slint::Image>),
    timer: slint::Timer,
    tick: slint::Timer,
    sender: Option<Rc<dyn Fn(Box<dyn Any + Send>)>>,
}

fn ic(name: &str, material: bool) -> ui::CcIc {
    if name.is_empty() {
        return ui::CcIc::default();
    }
    let (t, s) = tessera_controlcenter::icons::glyph(name, material);
    ui::CcIc { t: t.into(), s }
}

/// The Control Center's glyphs, for its page and blocks
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

fn sm_glyphs() -> ui::SmGlyphs {
    let g = |n: &str| SharedString::from(tessera_core::icons::glyph(n));
    ui::SmGlyphs { notes: g("music-notes"), prev: g("skip-back"), next: g("skip-forward"), play: g("play"), pause: g("pause") }
}

/// A Control Center block in this window's Slint (as its lib.rs `block`)
fn block(b: &Block, first: bool, last: bool, material: bool, (caption, small): (f32, f32), width: f32) -> ui::CcBlock {
    let pic = tessera_controlcenter::picture(&b.picture);
    let gap = match b.kind {
        BlockKind::Note => bridge::text_height(&b.title, if b.tone == Tone::Tertiary { caption } else { small }, width - 12.0),
        BlockKind::Notice => bridge::text_height(&b.status, small, width - 12.0),
        _ => b.height,
    };
    let mut options: Vec<SharedString> = b.options.iter().map(|o| o.as_str().into()).collect();
    if b.kind == BlockKind::Notice && !b.action.is_empty() {
        options = vec![b.action.as_str().into()];
    }
    ui::CcBlock {
        kind: b.kind.code(),
        id: b.id.as_str().into(),
        icon: ic(&b.icon, material),
        has_pic: pic.is_some(),
        pic: pic.unwrap_or_default(),
        title: b.title.as_str().into(),
        status: b.status.as_str().into(),
        hl: b.highlighted,
        trail: ic(&b.trailing, material),
        busy: b.busy,
        act: if b.kind == BlockKind::Notice { ui::CcIc::default() } else { ic(&b.action, material) },
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

fn blocks(p: &Page, material: bool, fonts: (f32, f32), width: f32) -> ModelRc<ui::CcBlock> {
    let groups = bridge::groups(&p.blocks);
    ModelRc::new(VecModel::from(p.blocks.iter().zip(groups).map(|(b, (first, last))| block(b, first, last, material, fonts, width)).collect::<Vec<_>>()))
}

fn hex(c: u32) -> slint::Color {
    slint::Color::from_rgb_u8((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

fn model<T: Clone + 'static>(v: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(v))
}

impl StatusMenus {
    fn material(st: &State) -> bool {
        st.shell.look.material
    }

    fn window(&mut self, st: &mut State, output: &str) -> bool {
        if self.win.as_ref().is_some_and(|w| w.output != output) {
            let w = self.win.take().unwrap();
            Glass::clear_now(&w.output, NS);
            // (hidden first: Slint keeps a shown window, and with it the layer, alive)
            let _ = w.ui.hide();
            w.surface.hide();
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
        let ui = ui::StatusMenus::new().expect("StatusMenus");
        st.shell.apply_theme(&ui.global::<ui::Theme>());
        ui.global::<ui::Symbols>().on_of(|g| tessera_core::icons::symbol(&g).into());
        let to = st.shell.sender(NAME);
        {
            let to = to.clone();
            ui.on_key(move |text, _shift| {
                let esc = text == SharedString::from(Key::Escape);
                if esc {
                    to(Box::new(M::Escape));
                }
                esc
            });
        }
        macro_rules! on {
            ($set:ident, |$($a:ident),*| $m:expr) => {{
                let to = to.clone();
                ui.$set(move |$($a),*| to(Box::new($m)));
            }};
        }
        on!(on_hovered, |h| M::Hovered(h));
        on!(on_dismissed, | | M::Dismissed);
        on!(on_closed, | | M::Closed);
        on!(on_page_toggled, | | M::PageToggled);
        on!(on_page_refreshed, | | M::PageRefreshed);
        on!(on_page_block, |i, w, _t| M::PageBlock(i, w.to_string()));
        on!(on_page_menu, |i, x, y| M::PageMenu(i, x, y));
        on!(on_sound_block, |i, w, _t| M::SoundBlock(i, w.to_string()));
        on!(on_sound_slide, |i, v| M::SoundSlide(i, v));
        on!(on_volume, |v| M::Volume(v));
        on!(on_mode, |i| M::Mode(i));
        on!(on_stop_sharing, | | M::StopSharing);
        on!(on_np_do, |w| M::Np(w.to_string()));
        on!(on_np_seek, |v| M::Seek(v));
        on!(on_np_pick, |i| M::Pick(i));
        on!(on_menu_pick, |j| M::MenuPick(j));
        on!(on_menu_close, | | M::MenuClose);
        ui.set_sg(sm_glyphs());
        ui.show().expect("show status menu");
        let a11y = tessera_core::a11y::A11y::new(&surface, st.tx.clone());
        self.win = Some(Win { output: output.into(), surface, ui, a11y, glass_sent: String::new(), region: (0, 0) });
        self.apply_look(st);
        true
    }

    fn apply_look(&self, st: &State) {
        let Some(w) = &self.win else { return };
        let ui = &w.ui;
        st.shell.apply_theme(&ui.global::<ui::Theme>());
        let m = Self::material(st);
        ui.set_g(glyphs(m));
        let t = ui.global::<ui::Theme>();
        ui.set_accent_glyph(if m { t.get_m3_on_primary() } else { tessera_controlcenter::accent_on_white(t.get_accent()) });
    }

    fn open(&mut self, st: &mut State, screen: Option<String>, args: &Value) {
        // Which menu, and under which item (said with the command; else the shell's state)
        let state = st.shell.get::<ShellState>().cloned().unwrap_or_default();
        let panel = args["panel"].as_str().map(str::to_string).unwrap_or_else(|| state.str("open").to_string());
        let Some((panel, cw, off)) = model::panel(&panel) else { return };
        if let Some(x) = args["anchorX"].as_f64().or_else(|| state.get("panels.anchorX").as_f64()) {
            self.anchor_x = x as f32;
        }
        let output = screen.filter(|s| !s.is_empty()).or_else(|| self.win.as_ref().map(|w| w.output.clone())).or_else(|| st.outputs_now().first().map(|o| o.1.clone()));
        let Some(output) = output else { return };
        let again = self.open && self.panel == panel && self.win.as_ref().is_some_and(|w| w.output == output);
        if !self.window(st, &output) {
            return;
        }
        for s in ["audio", "status", "bluez", "mpris", "weather"] {
            st.shell.want(s);
        }
        if again {
            return;
        }
        let first = !self.open;
        self.open = true;
        self.panel = panel.to_string();
        self.menu = None;
        if first {
            self.hovered = false;
        }
        if panel == "bluetoothMenu" && st.shell.get::<Bluetooth>().is_some_and(|b| b.powered) {
            // (scanning while it shows, as BluetoothPage does)
            tessera_core::services::bridge::run(&["bluetoothctl", "--timeout", "8", "scan", "on"]);
        }
        if panel == "nowPlaying" {
            self.position.2 = None;
            self.read_position(st);
        }
        let w = self.win.as_mut().unwrap();
        w.region = (0, 0);
        w.ui.set_panel(panel.into());
        w.ui.set_cw(cw);
        w.ui.set_off(off);
        w.ui.set_anchor_x(self.anchor_x);
        w.ui.set_menu_open(false);
        self.push(st);
        let w = self.win.as_ref().unwrap();
        if first {
            w.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
            w.surface.show(0, 0);
            w.ui.invoke_focus_keys();
            w.ui.set_shown(true);
        }
        if let Some(tx) = self.sender.clone() {
            self.tick.start(slint::TimerMode::Repeated, Duration::from_secs(1), move || tx(Box::new(M::Tick)));
        }
        self.hold();
    }

    fn close(&mut self, st: &mut State, tell: bool) {
        if !self.open {
            return;
        }
        self.open = false;
        self.menu = None;
        self.timer.stop();
        self.tick.stop();
        if let Some(w) = &self.win {
            w.ui.set_shown(false);
            w.ui.set_menu_open(false);
            w.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::None);
            w.surface.layer.commit();
            // (input passes through while it fades, as the QML's mask)
            st.set_input_region(&w.surface, Some(&[]));
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

    /// The menu closes itself 5 s after the pointer left (or never came),
    /// unless a device's menu is open
    fn hold(&mut self) {
        if self.open && !self.hovered && self.menu.is_none() {
            if let (false, Some(tx)) = (self.timer.running(), self.sender.clone()) {
                self.timer.start(slint::TimerMode::SingleShot, Duration::from_millis(AUTO_CLOSE_MS), move || tx(Box::new(M::AutoClose)));
            }
        } else {
            self.timer.stop();
        }
    }

    /// The open menu's content, from the readings
    fn push(&mut self, st: &State) {
        if !self.open || self.win.is_none() {
            return;
        }
        match self.panel.as_str() {
            "bluetoothMenu" => self.push_bt(st),
            "soundMenu" => self.push_sound(st),
            "privacyMenu" => self.push_privacy(st),
            "batteryMenu" => self.push_battery(st),
            "resourcesMenu" => self.push_resources(),
            "weatherMenu" => self.push_weather(st),
            "nowPlaying" => self.push_np(st),
            _ => {}
        }
    }

    fn fonts(&self) -> (f32, f32) {
        self.win.as_ref().map(|w| {
            let t = w.ui.global::<ui::Theme>();
            (t.get_caption(), t.get_small())
        }).unwrap_or((11.0, 12.0))
    }

    fn small(&self) -> f32 {
        self.fonts().1
    }

    fn push_bt(&mut self, st: &State) {
        let bt = st.shell.get::<Bluetooth>().cloned().unwrap_or_default();
        let state = st.shell.get::<ShellState>().cloned().unwrap_or_default();
        let page = model::bt_page(&bt, state.get("backends"), &self.bt_busy, &self.feed["btPictures"]);
        let fonts = self.fonts();
        let Some(w) = &self.win else { return };
        let m = Self::material(st);
        w.ui.set_page_title(page.title.as_str().into());
        w.ui.set_page_switch(page.switch);
        w.ui.set_page_switch_on(page.switch_on);
        w.ui.set_page_switch_label(page.switch_label.as_str().into());
        w.ui.set_page_refresh(page.refresh);
        if page != self.shown_bt {
            w.ui.set_page_blocks(blocks(&page, m, fonts, 300.0));
        }
        self.shown_bt = page;
    }

    fn profiles(&self) -> Vec<Profile> {
        self.feed["profiles"]
            .as_array()
            .map(|a| a.iter().map(|p| Profile { name: p["name"].as_str().unwrap_or("").into(), kind: p["kind"].as_str().unwrap_or("").into(), busy: p["busy"].as_bool().unwrap_or(false) }).collect())
            .unwrap_or_default()
    }

    fn push_sound(&mut self, st: &State) {
        let snd = st.shell.get::<Sound>().cloned().unwrap_or_default();
        let mic_in = st.shell.cfg["controlCenter"]["micInSound"].as_bool().unwrap_or(true);
        let page = bridge::sound_page(&snd, &self.profiles(), mic_in);
        let fonts = self.fonts();
        let m = Self::material(st);
        let Some(w) = &self.win else { return };
        let (v, muted) = (snd.volume(), snd.muted());
        let vol = Block { kind: BlockKind::Slider, id: "volume".into(), icon: tessera_controlcenter::volume_icon(v, muted).into(), value: if muted { 0.0 } else { v }, ..Default::default() };
        w.ui.set_volume_block(block(&vol, true, true, m, fonts, 300.0));
        if page != self.shown_sound {
            w.ui.set_sound_blocks(blocks(&page, m, fonts, 300.0));
        }
        self.shown_sound = page;
    }

    fn push_privacy(&self, st: &State) {
        let Some(w) = &self.win else { return };
        let p = &self.feed["privacy"];
        let rows: Vec<ui::SmUse> = model::in_use(p).into_iter().map(|u| ui::SmUse { app: u.app.into(), what: u.what.into(), icon: tessera_core::icons::glyph(u.icon).into(), tint: hex(u.color) }).collect();
        let small = self.small();
        w.ui.set_in_use(model(rows));
        w.ui.set_nothing_h(bridge::text_height("Nothing is using your microphone, camera, location or screen.", small, 260.0));
        w.ui.set_sharing(p["screenShared"].as_bool().unwrap_or(false));
        w.ui.set_stopping(p["stopping"].as_bool().unwrap_or(false));
        let failed = model::stop_failed(p);
        w.ui.set_stop_failed_h(bridge::text_height(&failed, small, 260.0));
        w.ui.set_stop_failed(failed.into());
        let _ = st;
    }

    fn push_battery(&self, st: &State) {
        let Some(w) = &self.win else { return };
        let b = st.shell.get::<Battery>().cloned().unwrap_or_default();
        let state = st.shell.get::<ShellState>().cloned().unwrap_or_default();
        let health = &self.feed["health"];
        let power = &self.feed["power"];
        let small = self.small();
        let level = model::level(&b);
        w.ui.set_bat_present(b.present);
        w.ui.set_bat_pct(format!("{}%", (level * 100.0).round()).into());
        w.ui.set_bat_level(level as f32);
        w.ui.set_bat_low(level <= 0.2 && !model::charging(&b));
        let status = model::battery_status(&b);
        w.ui.set_bat_status_h(bridge::text_height(&status, small, 280.0));
        w.ui.set_bat_status(status.into());
        w.ui.set_bat_watts(model::watts_line(&b, health).into());
        let rows: Vec<ui::SmPair> = if b.present { model::health_rows(health) } else { vec![] }
            .into_iter()
            .map(|(l, v)| ui::SmPair { warn: v == "Service recommended", label: l.into(), value: v.into() })
            .collect();
        w.ui.set_health(model(rows));
        // power-profiles-daemon: the shell's Backends (state `backends`, or the feed's)
        let ppd = power["ppd"].as_bool().or_else(|| state.get("power.ppd").as_bool()).or_else(|| state.get("backends.ppd").as_bool()).unwrap_or(false);
        let short = power["short"].as_str().filter(|s| !s.is_empty()).unwrap_or(state.str("backends.powerShort")).to_string();
        let short = if short.is_empty() { "Needs power-profiles-daemon".to_string() } else { short };
        let perf = power["hasPerformance"].as_bool().unwrap_or(state.bool("power.performance"));
        let profile = power["profile"].as_str().filter(|s| !s.is_empty()).unwrap_or(state.str("power.profile")).to_string();
        w.ui.set_ppd(ppd);
        w.ui.set_power_short_h(bridge::text_height(&short, small, 280.0));
        w.ui.set_power_short(short.into());
        let (opts, cur) = model::energy_modes(perf, &profile);
        let seg = Block { kind: BlockKind::Seg, id: "mode".into(), options: opts.iter().map(|o| o.to_string()).collect(), current: cur, ..Default::default() };
        w.ui.set_modes(block(&seg, true, true, Self::material(st), self.fonts(), 280.0));
    }

    fn push_resources(&self) {
        let Some(w) = &self.win else { return };
        let rows: Vec<ui::SmBar> = model::resource_rows(&self.feed["resources"]).into_iter().map(|(l, v, d)| ui::SmBar { label: l.into(), detail: d.into(), value: v as f32 }).collect();
        w.ui.set_bars(model(rows));
    }

    fn push_weather(&self, st: &State) {
        let Some(w) = &self.win else { return };
        let wx = st.shell.get::<Weather>().cloned().unwrap_or_default();
        let h24 = st.shell.cfg["appearance"]["clock24"].as_bool().unwrap_or(false);
        w.ui.set_wx_note(model::weather_note(wx.ready, &wx.error).into());
        w.ui.set_wx_place(if wx.place.is_empty() { "Weather".into() } else { wx.place.as_str().into() });
        w.ui.set_wx_icon(tessera_core::icons::glyph(if wx.icon.is_empty() { "cloud" } else { wx.icon }).into());
        w.ui.set_wx_temp(format!("{}°", wx.temp).into());
        w.ui.set_wx_cond(wx.condition.as_str().into());
        w.ui.set_wx_feels(format!("Feels like {}° · H:{}° L:{}°", wx.feels_like, wx.high, wx.low).into());
        let hours: Vec<ui::SmHour> = wx.hourly.iter().take(5).map(|h| ui::SmHour { time: model::hour(h.hour, h24).into(), icon: tessera_core::icons::glyph(h.icon).into(), temp: format!("{}°", h.temp).into() }).collect();
        w.ui.set_hours(model(hours));
    }

    /// The player Now Playing shows: the one picked here, else the shell's
    /// (Media.player: `media.bus`), else the newest playing
    fn player(&self, st: &State) -> (Option<Player>, Vec<Player>) {
        let ps = st.shell.get::<Players>().cloned().unwrap_or_default();
        let shell_bus = st.shell.get::<ShellState>().map(|s| s.str("media.bus").to_string()).unwrap_or_default();
        let chosen = [self.pinned.as_str(), shell_bus.as_str()]
            .iter()
            .filter(|b| !b.is_empty())
            .find_map(|b| ps.list.iter().find(|p| p.bus == *b))
            .or_else(|| ps.chosen())
            .cloned();
        let others = ps.list.iter().filter(|p| chosen.as_ref().is_none_or(|c| c.bus != p.bus)).cloned().collect();
        (chosen, others)
    }

    /// The position now: as last read, moving on while it plays
    fn position_now(&self, p: &Player) -> f64 {
        let (bus, pos, at) = &self.position;
        if *bus != p.bus {
            return 0.0;
        }
        let since = if p.playing { at.map_or(0.0, |t| t.elapsed().as_secs_f64()) } else { 0.0 };
        pos + since
    }

    /// Ask the player where it is (a thread; the answer is M::Position)
    fn read_position(&self, st: &State) {
        let (Some(p), _) = self.player(st) else { return };
        Self::read_position_for(st, &p.bus);
    }

    fn read_position_for(st: &State, bus: &str) {
        let to = st.shell.sender(NAME);
        let bus = bus.to_string();
        std::thread::spawn(move || {
            let out = std::process::Command::new("busctl")
                .args(["--user", "get-property", &bus, "/org/mpris/MediaPlayer2", "org.mpris.MediaPlayer2.Player", "Position"])
                .stdin(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .output();
            if let Some(us) = out.ok().and_then(|o| String::from_utf8_lossy(&o.stdout).split_whitespace().nth(1).and_then(|n| n.parse::<f64>().ok())) {
                to(Box::new(M::Position(bus, us / 1e6)));
            }
        });
    }

    fn push_np(&mut self, st: &State) {
        let (p, others) = self.player(st);
        // The art: a local picture for its address (fetched once into the cache)
        let url = p.as_ref().map(|p| p.track.art_url.clone()).unwrap_or_default();
        if url != self.art.0 {
            self.art = (url.clone(), None);
            if !url.is_empty() {
                let to = st.shell.sender(NAME);
                std::thread::spawn(move || {
                    let file = mpris::art_file(&url);
                    to(Box::new(M::Art(url, file)));
                });
            }
        }
        let Some(w) = &self.win else { return };
        let np = match &p {
            Some(p) => {
                let len = p.track.length as f64 / 1e6;
                let pos = self.position_now(p).min(len.max(0.0));
                ui::SmPlayer {
                    title: if p.track.title.is_empty() { "Nothing playing".into() } else { p.track.title.as_str().into() },
                    byline: model::byline(&p.track.artist, &p.track.album).into(),
                    identity: p.identity.as_str().into(),
                    has_art: self.art.1.is_some(),
                    art: self.art.1.clone().unwrap_or_default(),
                    playing: p.playing,
                    prev: p.can_prev,
                    next: p.can_next,
                    toggle: p.can_play,
                    timed: len > 0.0,
                    progress: if len > 0.0 { (pos / len) as f32 } else { 0.0 },
                    pos: model::mmss(pos).into(),
                    rem: model::mmss(len - pos).into(),
                }
            }
            None => ui::SmPlayer { title: "Nothing playing".into(), ..Default::default() },
        };
        w.ui.set_np(np);
        let rows: Vec<SharedString> = others.iter().map(|o| format!("{}{}", o.identity, if o.track.title.is_empty() { String::new() } else { format!(" · {}", o.track.title) }).into()).collect();
        w.ui.set_others(model(rows));
    }

    fn np_do(&mut self, st: &State, what: &str) {
        let (Some(p), _) = self.player(st) else { return };
        let method = match what {
            "previous" => "Previous",
            "next" => "Next",
            _ => "PlayPause",
        };
        mpris::control(&p.bus, method);
        if what != "toggle" {
            self.position = (p.bus.clone(), 0.0, Some(Instant::now()));
        }
    }

    fn seek(&mut self, st: &State, frac: f32) {
        let (Some(p), _) = self.player(st) else { return };
        if p.track.length <= 0 {
            return;
        }
        let target = (frac.clamp(0.0, 1.0) as f64 * p.track.length as f64) as i64;
        let offset = target - (self.position_now(&p) * 1e6) as i64;
        tessera_core::services::bridge::run(&["busctl", "--user", "call", &p.bus, "/org/mpris/MediaPlayer2", "org.mpris.MediaPlayer2.Player", "Seek", "x", &offset.to_string()]);
        self.position = (p.bus.clone(), target as f64 / 1e6, Some(Instant::now()));
        self.push_np(st);
    }

    fn pick(&mut self, st: &State, i: i32) {
        let (_, others) = self.player(st);
        let Some(o) = others.get(i.max(0) as usize) else { return };
        self.pinned = o.bus.clone();
        // (Media.pinned: the menu bar and the widgets follow)
        shell::ask("statusmenus", json!(["pin", o.bus]));
        self.position = (o.bus.clone(), 0.0, None);
        self.push_np(st);
        Self::read_position_for(st, &o.bus);
    }

    // ── Bluetooth ──

    fn device(&self, st: &State, id: &str) -> Option<tessera_core::services::bluez::Device> {
        let addr = id.strip_prefix("d:")?;
        let bt = st.shell.get::<Bluetooth>()?;
        model::bt_devices(bt).into_iter().find(|d| d.address == addr)
    }

    fn bt_ctl(&mut self, st: &mut State, verb: &'static str, addr: String) {
        self.bt_busy.insert(addr.clone());
        let to = st.shell.sender(NAME);
        std::thread::spawn(move || {
            let _ = std::process::Command::new("bluetoothctl").args([verb, &addr]).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
            to(Box::new(M::BtDone(addr)));
        });
        self.push_bt(st);
    }

    fn page_block(&mut self, st: &mut State, i: i32, what: &str) {
        let Some(b) = self.shown_bt.blocks.get(i.max(0) as usize).cloned() else { return };
        if b.id == "fix" {
            shell::ask("statusmenus", json!(["btFix"]));
            return;
        }
        if what != "click" {
            return;
        }
        if let Some(d) = self.device(st, &b.id) {
            let verb = model::bt_click(&d);
            self.bt_ctl(st, verb, d.address);
        }
    }

    fn page_menu(&mut self, st: &State, i: i32, x: f32, y: f32) {
        let Some(b) = self.shown_bt.blocks.get(i.max(0) as usize).cloned() else { return };
        let Some(d) = self.device(st, &b.id) else { return };
        let entries = model::bt_menu(&d, self.bt_busy.contains(&d.address));
        let Some(w) = &self.win else { return };
        let items: Vec<ui::CcMenuItem> = model::menu_items(&entries).into_iter().map(|m| ui::CcMenuItem { text: m.text.into(), checked: m.checked, sep: m.separator, danger: m.danger }).collect();
        w.ui.set_menu_items(model(items));
        let (sw, sh) = w.surface.logical_size();
        let (mw, mh) = (w.ui.get_menu_w(), w.ui.get_menu_h());
        w.ui.set_menu_x(x.min(sw as f32 - mw - 8.0).max(8.0));
        w.ui.set_menu_y(y.min(sh as f32 - mh - 8.0).max(34.0));
        w.ui.set_menu_open(true);
        self.menu = Some((d.address, entries));
        self.hold();
    }

    fn close_menu(&mut self) {
        if self.menu.take().is_some() {
            if let Some(w) = &self.win {
                w.ui.set_menu_open(false);
            }
            self.hold();
        }
    }

    fn menu_pick(&mut self, st: &mut State, j: i32) {
        let Some((addr, entries)) = self.menu.clone() else { return };
        let Some(Some((_, act, _))) = entries.get(j.max(0) as usize).cloned() else { return };
        self.close_menu();
        match act {
            BtAct::Ctl(verb) => self.bt_ctl(st, verb, addr),
            BtAct::Copy => tessera_core::services::bridge::run(&["wl-copy", "--", &addr]),
            BtAct::Settings => shell::ask("panels.openSettings", json!(["network", "Bluetooth"])),
        }
    }

    // ── Sound ──

    fn hush(&mut self, st: &mut State) {
        let v = json!({"to": "osd", "hush": "sound"});
        st.part("osd", |p, st| p.message(st, Box::new(v)));
        shell::ask("audio.hush", json!([]));
    }

    fn volume(&mut self, st: &mut State, v: f32) {
        let snd = st.shell.get::<Sound>().cloned().unwrap_or_default();
        self.hush(st);
        if let Some(d) = snd.sink() {
            if d.muted {
                audio::send(Cmd::SinkMute { index: d.index, muted: false });
            }
            audio::send(Cmd::SinkVolume { index: d.index, channels: d.channels, volume: v.clamp(0.0, 1.0) as f64 });
        }
    }

    fn sound_block(&mut self, st: &mut State, i: i32, what: &str) {
        let Some(b) = self.shown_sound.blocks.get(i.max(0) as usize).cloned() else { return };
        let snd = st.shell.get::<Sound>().cloned().unwrap_or_default();
        if let Some(n) = b.id.strip_prefix("prof:") {
            shell::ask("statusmenus", json!(["profile", n.parse::<i64>().unwrap_or(-1)]));
        } else if let Some(name) = b.id.strip_prefix("out:") {
            audio::send(Cmd::DefaultSink(name.into()));
        } else if let Some(name) = b.id.strip_prefix("in:") {
            audio::send(Cmd::DefaultSource(name.into()));
        } else if let Some(s) = b.id.strip_prefix("app:").and_then(|n| n.parse::<u32>().ok()).and_then(|n| snd.streams.iter().find(|s| s.index == n)) {
            if what == "mute" {
                audio::send(Cmd::StreamMute { index: s.index, muted: !s.muted });
            }
        }
    }

    fn sound_slide(&mut self, st: &mut State, i: i32, v: f32) {
        let Some(b) = self.shown_sound.blocks.get(i.max(0) as usize).cloned() else { return };
        let snd = st.shell.get::<Sound>().cloned().unwrap_or_default();
        let v = v.clamp(0.0, 1.0) as f64;
        if b.id == "mic" {
            self.hush(st);
            if let Some(d) = snd.source() {
                if d.muted {
                    audio::send(Cmd::SourceMute { index: d.index, muted: false });
                }
                audio::send(Cmd::SourceVolume { index: d.index, channels: d.channels, volume: v });
            }
        } else if let Some(s) = b.id.strip_prefix("app:").and_then(|n| n.parse::<u32>().ok()).and_then(|n| snd.streams.iter().find(|s| s.index == n)) {
            if s.muted {
                audio::send(Cmd::StreamMute { index: s.index, muted: false });
            }
            audio::send(Cmd::StreamVolume { index: s.index, channels: s.channels, volume: v });
        }
    }
}

impl Part for StatusMenus {
    fn name(&self) -> &'static str {
        NAME
    }

    fn start(&mut self, st: &mut State) {
        let to: Rc<dyn Fn(Box<dyn Any + Send>)> = Rc::new(st.shell.sender(NAME));
        self.sender = Some(to);
    }

    fn command(&mut self, st: &mut State, cmd: &Command) {
        match cmd.cmd.as_str() {
            "open" => self.open(st, cmd.screen.clone(), &cmd.args),
            "close" => self.close(st, false),
            "toggle" => {
                if self.open {
                    self.close(st, true)
                } else {
                    self.open(st, cmd.screen.clone(), &cmd.args)
                }
            }
            _ => {}
        }
    }

    fn config_changed(&mut self, st: &mut State, look: bool) {
        if look {
            if let Some(w) = &mut self.win {
                w.glass_sent.clear();
            }
            self.shown_bt = Page::default();
            self.shown_sound = Page::default();
            self.apply_look(st);
        }
        self.push(st);
    }

    fn plugin_changed(&mut self, st: &mut State) {
        if let Some(w) = &mut self.win {
            w.glass_sent.clear();
        }
        self.apply_look(st);
    }

    fn data(&mut self, st: &mut State, what: TypeId) {
        if !self.open {
            return;
        }
        let wanted = match self.panel.as_str() {
            "bluetoothMenu" => what == TypeId::of::<Bluetooth>() || what == TypeId::of::<ShellState>(),
            "soundMenu" => what == TypeId::of::<Sound>(),
            "batteryMenu" => what == TypeId::of::<Battery>() || what == TypeId::of::<ShellState>(),
            "weatherMenu" => what == TypeId::of::<Weather>(),
            "nowPlaying" => what == TypeId::of::<Players>() || what == TypeId::of::<ShellState>(),
            _ => false,
        };
        if wanted {
            self.push(st);
        }
    }

    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        // The shell's feed (host/system's StatusMenusFeed, or StatusMenusFeed.qml)
        let msg = match msg.downcast::<Value>() {
            Ok(v) => {
                for k in ["health", "power", "privacy", "resources", "profiles", "btPictures"] {
                    if let Some(x) = v.get(k) {
                        self.feed[k] = x.clone();
                    }
                }
                if !self.feed.is_object() {
                    self.feed = json!({});
                }
                if self.panel == "soundMenu" {
                    self.shown_sound = Page::default();
                }
                self.push(st);
                return;
            }
            Err(m) => m,
        };
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Escape => {
                if self.menu.is_some() {
                    self.close_menu();
                } else {
                    self.close(st, true);
                }
            }
            M::Hovered(h) => {
                self.hovered = h;
                self.hold();
            }
            M::AutoClose => {
                if self.open && !self.hovered && self.menu.is_none() {
                    self.close(st, true);
                }
            }
            M::Dismissed => self.close(st, true),
            M::Closed => self.closed(st),
            M::PageToggled => {
                let on = st.shell.get::<Bluetooth>().is_some_and(|b| b.powered);
                tessera_core::services::bridge::run(&["bluetoothctl", "power", if on { "off" } else { "on" }]);
            }
            M::PageRefreshed => tessera_core::services::bridge::run(&["bluetoothctl", "--timeout", "8", "scan", "on"]),
            M::PageBlock(i, w) => self.page_block(st, i, &w),
            M::PageMenu(i, x, y) => self.page_menu(st, i, x, y),
            M::SoundBlock(i, w) => self.sound_block(st, i, &w),
            M::SoundSlide(i, v) => self.sound_slide(st, i, v),
            M::Volume(v) => self.volume(st, v),
            M::Mode(i) => {
                if let Some(m) = model::MODES.get(i.max(0) as usize) {
                    shell::ask("statusmenus", json!(["mode", m]));
                }
            }
            M::StopSharing => shell::ask("statusmenus", json!(["stopSharing"])),
            M::Np(w) => self.np_do(st, &w),
            M::Seek(v) => self.seek(st, v),
            M::Pick(i) => self.pick(st, i),
            M::MenuPick(j) => self.menu_pick(st, j),
            M::MenuClose => self.close_menu(),
            M::BtDone(addr) => {
                self.bt_busy.remove(&addr);
                if self.open && self.panel == "bluetoothMenu" {
                    self.push_bt(st);
                }
            }
            M::Position(bus, secs) => {
                self.position = (bus, secs, Some(Instant::now()));
                if self.open && self.panel == "nowPlaying" {
                    self.push_np(st);
                }
            }
            M::Art(url, file) => {
                if url == self.art.0 {
                    self.art.1 = file.and_then(|f| tessera_controlcenter::picture(&f.to_string_lossy()));
                    if self.open && self.panel == "nowPlaying" {
                        self.push_np(st);
                    }
                }
            }
            M::Tick => {
                if self.open && self.panel == "nowPlaying" {
                    // (the position, once a second while it plays: Popover's Timer)
                    if self.player(st).0.is_some_and(|p| p.playing) {
                        self.read_position(st);
                    }
                    self.push_np(st);
                }
            }
        }
    }

    fn keyboard_left(&mut self, st: &mut State, s: &Rc<Surface>) {
        if self.open && self.win.as_ref().is_some_and(|w| Rc::ptr_eq(&w.surface, s)) {
            self.close(st, true);
        }
    }

    fn output_removed(&mut self, st: &mut State, name: &str) {
        if self.win.as_ref().is_some_and(|w| w.output == name) {
            let was = self.open;
            self.close(st, was);
            let w = self.win.take().unwrap();
            Glass::clear_now(&w.output, NS);
            // (hidden first: Slint keeps a shown window, and with it the layer, alive)
            let _ = w.ui.hide();
            w.surface.hide();
            st.drop_surface(&w.surface);
        }
    }

    fn surface_closed(&mut self, st: &mut State, s: &Rc<Surface>) {
        if self.win.as_ref().is_some_and(|w| Rc::ptr_eq(&w.surface, s)) {
            let name = s.output.clone();
            self.output_removed(st, &name);
        }
    }

    /// The menu bar strip stays the bar's; the popover's glass (Glass), and
    /// a device menu's; the accessibility tree
    fn after_frame(&mut self, st: &mut State) {
        let kbd = st.keyboard_surface();
        let open = self.open;
        let menu = self.menu.is_some();
        let Some(w) = &mut self.win else { return };
        let f = kbd.as_ref().is_some_and(|k| Rc::ptr_eq(k, &w.surface));
        if let Some(a) = w.a11y.as_mut() {
            a.update(false, f);
        }
        if !w.surface.mapped.get() {
            return;
        }
        let size = w.surface.logical_size();
        if open && size != w.region && size.0 > 0 {
            st.set_input_region(&w.surface, Some(&[(0, 30, size.0 as i32, size.1 as i32 - 30)]));
            w.region = size;
        }
        if !st.shell.plugin || st.shell.look.material {
            return;
        }
        let ui = &w.ui;
        let o = ui.get_panel_opacity();
        let mut shapes = Vec::new();
        if o > 0.002 {
            shapes.push(theme::glass_shape(&st.shell.look, ui.get_panel_x(), ui.get_panel_y(), ui.get_panel_w(), ui.get_panel_h(), ui.get_panel_radius(), o, 0.35));
            if menu {
                shapes.push(theme::glass_shape(&st.shell.look, ui.get_menu_x(), ui.get_menu_y(), ui.get_menu_w(), ui.get_menu_h(), 14.0, o, 0.35));
            }
        }
        let key = format!("{shapes:?}");
        if key != w.glass_sent {
            st.shell.glass.set(&w.output, NS, &shapes);
            w.glass_sent = key;
        }
    }

    fn a11y(&mut self) -> Vec<&mut tessera_core::a11y::A11y> {
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

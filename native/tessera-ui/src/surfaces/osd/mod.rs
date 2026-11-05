//! The OSD (rust.surfaces: osd): volume, microphone and brightness shown
//! for a moment when they change, as shell/modules/osd/Osd.qml. Glass: a
//! capsule on the plugin's glass under the menu bar at the right (unless the
//! island shows it, the default, either menu bar's);
//! Material: Android's volume panel at the right edge
//! (MaterialVolumePanel.qml); over a fullscreen app, either look: a thin bar
//! along the top. Each takes the pointer: resting on it keeps it up; click,
//! drag or scroll the level; the symbol mutes; the button opens the outputs,
//! inputs or displays (IslandLevels.qml) under it (Glass) or beside it
//! (Material).
//!
//! It learns of changes from the sound server (services::audio) and the
//! backlight (services::brightness) itself. The QML shell keeps the media
//! keys (they only change the level) and tells this process, on stdin, what
//! its own sliders set (`{"to":"osd","hush":"sound"}`: you're looking at a
//! control already, no OSD) and `qs ipc call osd preview <kind>`
//! (`{"to":"osd","preview":"volume"}`); this one says on stdout while the
//! capsule shows (`{"src":"ui","kind":"osd","shown":true}`), so the
//! notification banners move down under it as they do for the QML one.
//!
//! Not like the QML yet: Material draws Phosphor icons, not Material Symbols
//! (Slint can't set the font's FILL axis); the apps in App volumes show a
//! generic symbol, not their own icons; external displays' brightness (DDC) isn't read;
//! the fullscreen bar's halo is a shadow under the track only (no
//! MultiEffect); Cool-down's slower motion isn't followed.

pub mod logic;
pub mod settings;

use crate::services::audio::{self, Cmd, Sound};
use crate::services::brightness::{self, Backlight};
use crate::services::hypr::{Glass, Snapshot};
use crate::shell::Part;
use crate::theme;
use crate::ui;
use crate::wayland::{Spec, State, Surface};
use logic::{Act, Cue, Kind, Place, Row};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use smithay_client_toolkit::reexports::client::protocol::wl_output::WlOutput;
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

pub const HUD_NS: &str = "tessera:osd";
pub const BAR_NS: &str = "tessera:fshud";
const NAME: &str = "osd";
const M: i32 = 24; // the capsule window's room for the glass's shadow

/// Its own messages (Msg::Part("osd", …))
enum Msg {
    Arm,
    Hide,
    BarHide,
    Switch,
    SetLevel(f32),
    ToggleMute,
    ToggleLevels,
    Closed,
    BarClosed,
    Picked(i32),
    RowLevel(i32, f32),
}

struct Win<T> {
    output: String,
    surface: Rc<Surface>,
    ui: T,
    a11y: Option<crate::a11y::A11y>,
    size: (u32, u32),
    region: Vec<(i32, i32, i32, i32)>,
    glass_sent: String,
}

pub struct Osd {
    capsule: Option<Win<ui::OsdCapsule>>,
    panel: Option<Win<ui::OsdPanel>>,
    bar: Option<Win<ui::OsdBar>>,
    rows_model: Rc<VecModel<ui::OsdRow>>,
    rows: Vec<Row>,
    // what shows
    kind: Kind,
    device: String,
    shown: bool,
    levels_open: bool,
    page: &'static str,
    pointer_on: bool,
    bar_kind: Kind,
    bar_shown: bool,
    told_shown: bool,
    // when it shows
    started: Instant,
    armed: bool,
    quiet_until: [u64; 2], // sound, brightness (ms since start)
    watch: logic::Watch,
    backlight_seen: Option<f64>,
    switch_kind: Option<Kind>,
    names: HashMap<String, String>,
    pictures: HashMap<String, slint::Image>,
    hide_after: u64,
    arm: slint::Timer,
    hide: slint::Timer,
    bar_hide: slint::Timer,
    switch: slint::Timer,
}

impl Default for Osd {
    fn default() -> Self {
        Self {
            capsule: None,
            panel: None,
            bar: None,
            rows_model: Rc::new(VecModel::default()),
            rows: Vec::new(),
            kind: Kind::Volume,
            device: String::new(),
            shown: false,
            levels_open: false,
            page: "list",
            pointer_on: false,
            bar_kind: Kind::Volume,
            bar_shown: false,
            told_shown: false,
            started: Instant::now(),
            armed: false,
            quiet_until: [0; 2],
            watch: logic::Watch::default(),
            backlight_seen: None,
            switch_kind: None,
            names: HashMap::new(),
            pictures: HashMap::new(),
            hide_after: 1600,
            arm: slint::Timer::default(),
            hide: slint::Timer::default(),
            bar_hide: slint::Timer::default(),
            switch: slint::Timer::default(),
        }
    }
}

fn slot(kind: Kind) -> usize {
    usize::from(kind == Kind::Brightness)
}

fn kind_named(s: &str) -> Option<Kind> {
    match s {
        "volume" | "sound" => Some(Kind::Volume),
        "brightness" => Some(Kind::Brightness),
        "mic" => Some(Kind::Mic),
        _ => None,
    }
}

/// shell-ipc.sh, from a thread (the QML shell opens Settings)
fn shell_ipc(args: Vec<String>) {
    std::thread::spawn(move || {
        let root = crate::env::var("ROOT").filter(|r| !r.is_empty()).unwrap_or_else(|| concat!(env!("CARGO_MANIFEST_DIR"), "/../..").into());
        let _ = std::process::Command::new(format!("{root}/shell/assets/scripts/shell-ipc.sh")).args(args).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
    });
}

impl Osd {
    fn now(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }

    fn after(&self, st: &State, timer: &slint::Timer, ms: u64, m: fn() -> Msg) {
        let to = st.shell.sender(NAME);
        timer.start(slint::TimerMode::SingleShot, Duration::from_millis(ms), move || to(Box::new(m())));
    }

    /// The display with focus (Hyprland's), else the first
    fn target_output(st: &State) -> Option<(WlOutput, String)> {
        let focused = st.shell.get::<Snapshot>().map(|s| s.focused.clone()).unwrap_or_default();
        let all = st.outputs_now();
        all.iter().find(|(_, n)| *n == focused).cloned().or_else(|| all.into_iter().next())
    }

    /// Osd.qml's show()
    fn show(&mut self, st: &mut State, kind: Kind, device: String) {
        let quiet = self.now() < self.quiet_until[slot(kind)];
        let fullscreen = st.shell.get::<Snapshot>().is_some_and(|s| s.fullscreen());
        let island = settings::island_hud(&st.shell.cfg, st.shell.look.material);
        match logic::place(self.armed, quiet, !device.is_empty(), fullscreen, island) {
            Place::Nowhere | Place::Island => {}
            Place::Bar => {
                self.bar_kind = kind;
                self.bar_shown = true;
                self.open_bar(st);
                self.after(st, &self.bar_hide, 1200, || Msg::BarHide);
            }
            Place::Hud => {
                if kind != self.kind {
                    self.page = "list";
                }
                self.kind = kind;
                self.device = device;
                self.shown = true;
                self.hide_after = logic::hide_after(&self.device);
                self.open_hud(st);
                if !self.pointer_on {
                    self.after(st, &self.hide, self.hide_after, || Msg::Hide);
                }
            }
        }
        self.refresh(st);
    }

    fn open_hud(&mut self, st: &mut State) {
        let Some((output, name)) = Self::target_output(st) else { return };
        if st.shell.look.material {
            if self.panel.as_ref().is_some_and(|w| w.output != name) {
                let w = self.panel.take().unwrap();
                st.drop_surface(&w.surface);
            }
            if self.panel.is_none() {
                let surface = st.prepare_surface(Spec {
                    layer: Layer::Overlay,
                    namespace: HUD_NS,
                    output: Some((&output, &name)),
                    anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::RIGHT,
                    size: (408, 0),
                    exclusive: -1,
                    keyboard: KeyboardInteractivity::None,
                    margin_left: 0,
                });
                let ui = ui::OsdPanel::new().expect("OsdPanel");
                self.wire_panel(st, &ui);
                ui.show().expect("show OSD");
                st.set_input_region(&surface, Some(&[])); // (until it rests)
                let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
                self.panel = Some(Win { output: name.clone(), surface, ui, a11y, size: (0, 0), region: Vec::new(), glass_sent: String::new() });
            }
            let w = self.panel.as_mut().unwrap();
            w.ui.set_shown(true);
            if !w.surface.mapped.get() {
                w.surface.show(408, 0);
            }
        } else {
            if self.capsule.as_ref().is_some_and(|w| w.output != name) {
                let w = self.capsule.take().unwrap();
                st.shell.glass.set(&w.output, HUD_NS, &[]);
                st.drop_surface(&w.surface);
            }
            if self.capsule.is_none() {
                let size = (280 + 2 * M as u32, 52 + 2 * M as u32);
                let surface = st.prepare_surface(Spec {
                    layer: Layer::Overlay,
                    namespace: HUD_NS,
                    output: Some((&output, &name)),
                    anchor: Anchor::TOP | Anchor::RIGHT,
                    size,
                    exclusive: -1,
                    keyboard: KeyboardInteractivity::None,
                    margin_left: 0,
                });
                // 6 px under the 30 px bar, 8 px from the edge (the window
                // reaches past both by its shadow's room)
                surface.layer.set_margin(30 + 6 - M, 8 - M, 0, 0);
                let ui = ui::OsdCapsule::new().expect("OsdCapsule");
                self.wire_capsule(st, &ui);
                ui.show().expect("show OSD");
                let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
                self.capsule = Some(Win { output: name.clone(), surface, ui, a11y, size: (0, 0), region: Vec::new(), glass_sent: String::new() });
            }
            let w = self.capsule.as_mut().unwrap();
            w.ui.set_shown(true);
            if !w.surface.mapped.get() {
                w.size = (0, 0); // (sized in after_frame)
            }
        }
    }

    fn wire_capsule(&self, st: &State, ui: &ui::OsdCapsule) {
        st.shell.apply_theme(&ui.global::<ui::Theme>());
        let to = st.shell.sender(NAME);
        let t = to.clone();
        ui.on_closed(move || t(Box::new(Msg::Closed)));
        let t = to.clone();
        ui.on_set_level(move |v| t(Box::new(Msg::SetLevel(v))));
        let t = to.clone();
        ui.on_toggle_mute(move || t(Box::new(Msg::ToggleMute)));
        let t = to.clone();
        ui.on_toggle_levels(move || t(Box::new(Msg::ToggleLevels)));
        let t = to.clone();
        ui.on_picked(move |i| t(Box::new(Msg::Picked(i))));
        ui.on_row_level(move |i, v| to(Box::new(Msg::RowLevel(i, v))));
        ui.set_rows(ModelRc::from(self.rows_model.clone()));
    }

    fn wire_panel(&self, st: &State, ui: &ui::OsdPanel) {
        st.shell.apply_theme(&ui.global::<ui::Theme>());
        let to = st.shell.sender(NAME);
        let t = to.clone();
        ui.on_closed(move || t(Box::new(Msg::Closed)));
        let t = to.clone();
        ui.on_set_level(move |v| t(Box::new(Msg::SetLevel(v))));
        let t = to.clone();
        ui.on_toggle_mute(move || t(Box::new(Msg::ToggleMute)));
        let t = to.clone();
        ui.on_toggle_levels(move || t(Box::new(Msg::ToggleLevels)));
        let t = to.clone();
        ui.on_picked(move |i| t(Box::new(Msg::Picked(i))));
        ui.on_row_level(move |i, v| to(Box::new(Msg::RowLevel(i, v))));
        ui.set_rows(ModelRc::from(self.rows_model.clone()));
    }

    fn open_bar(&mut self, st: &mut State) {
        let Some((output, name)) = Self::target_output(st) else { return };
        if self.bar.as_ref().is_some_and(|w| w.output != name) {
            let w = self.bar.take().unwrap();
            st.drop_surface(&w.surface);
        }
        // (30% of the display's width, as fsBar)
        let screen_w = st.outputs.info(&output).and_then(|i| i.logical_size).map_or(1920, |s| s.0.max(1));
        let w = (screen_w as f32 * 0.3).round() as u32 + 32;
        if self.bar.is_none() {
            let surface = st.prepare_surface(Spec {
                layer: Layer::Overlay,
                namespace: BAR_NS,
                output: Some((&output, &name)),
                anchor: Anchor::TOP,
                size: (w, 24),
                exclusive: -1,
                keyboard: KeyboardInteractivity::None,
                margin_left: 0,
            });
            let ui = ui::OsdBar::new().expect("OsdBar");
            st.shell.apply_theme(&ui.global::<ui::Theme>());
            let to = st.shell.sender(NAME);
            ui.on_closed(move || to(Box::new(Msg::BarClosed)));
            ui.show().expect("show OSD bar");
            st.set_input_region(&surface, Some(&[])); // (clicks go through)
            let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
            self.bar = Some(Win { output: name, surface, ui, a11y, size: (w, 24), region: Vec::new(), glass_sent: String::new() });
        }
        let b = self.bar.as_mut().unwrap();
        b.ui.set_shown(true);
        if !b.surface.mapped.get() || b.size.0 != w {
            b.size = (w, 24);
            b.surface.show(w, 24);
        }
    }

    /// Push what shows into whichever windows exist
    fn refresh(&mut self, st: &mut State) {
        let s = st.shell.get::<Sound>().cloned().unwrap_or_default();
        let b = st.shell.get::<Backlight>().cloned().unwrap_or_default();
        let kind = self.kind;
        let names = &mut self.names;
        let mut app_name = |x: &audio::Stream| {
            let key = [&x.icon, &x.binary, &x.name].into_iter().find(|k| !k.is_empty()).cloned().unwrap_or_default().to_lowercase();
            names.entry(key.clone()).or_insert_with(|| crate::util::desktop_name(&key).unwrap_or_else(|| x.name.clone())).clone()
        };
        self.rows = logic::rows(kind, self.page, &s, &b, &mut app_name);
        let pictures = &mut self.pictures;
        let mut picture = |kind: Option<&str>| {
            let Some(kind) = kind else { return slint::Image::default() };
            pictures
                .entry(kind.to_string())
                .or_insert_with(|| {
                    let root = crate::env::var("ROOT").filter(|r| !r.is_empty()).unwrap_or_else(|| concat!(env!("CARGO_MANIFEST_DIR"), "/../..").into());
                    slint::Image::load_from_path(std::path::Path::new(&format!("{root}/shell/assets/devices/{kind}.svg"))).unwrap_or_default()
                })
                .clone()
        };
        let model: Vec<ui::OsdRow> = self
            .rows
            .iter()
            .map(|r| ui::OsdRow { icon: r.icon.into(), picture: picture(r.picture), text: r.text.as_str().into(), current: r.current, chevron: r.chevron, value: r.value as f32, dim: r.dim })
            .collect();
        // (row by row: a row being dragged keeps its pointer)
        for (i, r) in model.iter().enumerate() {
            if i >= self.rows_model.row_count() {
                self.rows_model.push(r.clone());
            } else if self.rows_model.row_data(i).as_ref() != Some(r) {
                self.rows_model.set_row_data(i, r.clone());
            }
        }
        while self.rows_model.row_count() > model.len() {
            self.rows_model.remove(self.rows_model.row_count() - 1);
        }

        let value = logic::value(kind, &s, &b) as f32;
        let icon = logic::icon(kind, &s);
        let label = logic::label(kind, &self.device, &s);
        let pace = settings::pace(&st.shell.cfg, st.shell.look.material);
        if let Some(w) = &self.capsule {
            let ui = &w.ui;
            ui.set_on_glass(st.shell.plugin);
            ui.set_pace(pace);
            ui.set_glyph(icon.into());
            ui.set_label(label.as_str().into());
            ui.set_value(value);
            ui.set_can_mute(kind != Kind::Brightness);
            ui.set_more_label(logic::more_label(kind).into());
            ui.set_levels_open(self.levels_open);
        }
        if let Some(w) = &self.panel {
            let ui = &w.ui;
            ui.set_pace(pace);
            ui.set_glyph(icon.into());
            ui.set_label(label.as_str().into());
            ui.set_value(value);
            ui.set_can_mute(kind != Kind::Brightness);
            ui.set_mic(kind == Kind::Mic);
            ui.set_muted(if kind == Kind::Mic { s.mic_muted() } else { s.muted() });
            ui.set_toast(!self.device.is_empty() || kind == Kind::Mic);
            ui.set_more_label(logic::more_label(kind).into());
            ui.set_levels_open(self.levels_open);
        }
        if let Some(w) = &self.bar {
            let ui = &w.ui;
            ui.set_glyph(logic::icon(self.bar_kind, &s).into());
            ui.set_value(logic::value(self.bar_kind, &s, &b) as f32);
            ui.set_name(match self.bar_kind {
                Kind::Brightness => "Brightness",
                Kind::Mic => "Microphone",
                Kind::Volume => "Volume",
            }.into());
            ui.set_still(settings::reduce_motion(&st.shell.cfg));
        }
        // (the banners move down under the capsule)
        let capsule_shown = self.shown && !st.shell.look.material && self.capsule.is_some();
        if capsule_shown != self.told_shown {
            self.told_shown = capsule_shown;
            crate::surfaces::tell(serde_json::json!({"src": "ui", "kind": "osd", "shown": capsule_shown}));
        }
    }

    fn hide_now(&mut self, st: &mut State) {
        self.shown = false;
        self.levels_open = false;
        self.page = "list";
        if let Some(w) = &self.capsule {
            w.ui.set_shown(false);
        }
        if let Some(w) = &self.panel {
            w.ui.set_shown(false);
        }
        self.refresh(st);
    }

    fn hush(&mut self, kind: Kind) {
        self.quiet_until[slot(kind)] = self.now() + 1000;
    }

    /// Set the shown kind's level (its own slider: quiet)
    fn set_level(&mut self, st: &mut State, v: f64) {
        let v = v.clamp(0.0, 1.0);
        self.hush(self.kind);
        let s = st.shell.get::<Sound>().cloned().unwrap_or_default();
        match self.kind {
            Kind::Brightness => brightness::set(v),
            Kind::Volume => {
                if let Some(d) = s.sink() {
                    if d.muted {
                        audio::send(Cmd::SinkMute { index: d.index, muted: false });
                    }
                    audio::send(Cmd::SinkVolume { index: d.index, channels: d.channels, volume: v });
                }
            }
            Kind::Mic => {
                if let Some(d) = s.source() {
                    if d.muted {
                        audio::send(Cmd::SourceMute { index: d.index, muted: false });
                    }
                    audio::send(Cmd::SourceVolume { index: d.index, channels: d.channels, volume: v });
                }
            }
        }
        // (the level moves under the pointer at once; the server's answer follows)
        if let Some(w) = &self.capsule {
            w.ui.set_value(v as f32);
        }
        if let Some(w) = &self.panel {
            w.ui.set_value(v as f32);
        }
    }

    fn toggle_mute(&mut self, st: &mut State) {
        self.hush(self.kind);
        let s = st.shell.get::<Sound>().cloned().unwrap_or_default();
        match self.kind {
            Kind::Volume => {
                if let Some(d) = s.sink() {
                    audio::send(Cmd::SinkMute { index: d.index, muted: !d.muted });
                }
            }
            Kind::Mic => {
                if let Some(d) = s.source() {
                    audio::send(Cmd::SourceMute { index: d.index, muted: !d.muted });
                }
            }
            Kind::Brightness => {}
        }
    }

    fn picked(&mut self, st: &mut State, i: i32) {
        let Some(row) = self.rows.get(i as usize).cloned() else { return };
        let s = st.shell.get::<Sound>().cloned().unwrap_or_default();
        match row.act {
            Act::Sink(n) => audio::send(Cmd::DefaultSink(n)),
            Act::Source(n) => audio::send(Cmd::DefaultSource(n)),
            Act::Apps => self.page = "apps",
            Act::Back => self.page = "list",
            Act::Stream(index) => {
                if let Some(x) = s.streams.iter().find(|x| x.index == index) {
                    audio::send(Cmd::StreamMute { index, muted: !x.muted });
                }
            }
            Act::Settings(page) => shell_ipc(vec!["call".into(), "panels".into(), "settings".into(), page.into()]),
            Act::Backlight => {}
        }
        self.refresh(st);
    }

    fn row_level(&mut self, st: &mut State, i: i32, v: f64) {
        let Some(row) = self.rows.get(i as usize).cloned() else { return };
        let s = st.shell.get::<Sound>().cloned().unwrap_or_default();
        match row.act {
            Act::Backlight => {
                self.hush(Kind::Brightness);
                brightness::set(v);
            }
            Act::Stream(index) => {
                if let Some(x) = s.streams.iter().find(|x| x.index == index) {
                    if x.muted {
                        audio::send(Cmd::StreamMute { index, muted: false });
                    }
                    audio::send(Cmd::StreamVolume { index, channels: x.channels, volume: v });
                }
            }
            _ => {}
        }
    }

    fn drop_all(&mut self, st: &mut State) {
        if let Some(w) = self.capsule.take() {
            st.shell.glass.set(&w.output, HUD_NS, &[]);
            st.drop_surface(&w.surface);
        }
        if let Some(w) = self.panel.take() {
            st.drop_surface(&w.surface);
        }
        if let Some(w) = self.bar.take() {
            st.drop_surface(&w.surface);
        }
        self.shown = false;
        self.bar_shown = false;
        self.levels_open = false;
    }
}

impl Part for Osd {
    fn name(&self) -> &'static str {
        NAME
    }

    fn start(&mut self, st: &mut State) {
        self.started = Instant::now();
        st.shell.want("hypr");
        st.shell.want("audio");
        st.shell.want("brightness");
        // (the first readings aren't changes: armed after 2 s, as Osd.qml)
        self.after(st, &self.arm, 2000, || Msg::Arm);
    }

    fn output_removed(&mut self, st: &mut State, name: &str) {
        if self.capsule.as_ref().is_some_and(|w| w.output == name) {
            let w = self.capsule.take().unwrap();
            st.drop_surface(&w.surface);
        }
        if self.panel.as_ref().is_some_and(|w| w.output == name) {
            let w = self.panel.take().unwrap();
            st.drop_surface(&w.surface);
        }
        if self.bar.as_ref().is_some_and(|w| w.output == name) {
            let w = self.bar.take().unwrap();
            st.drop_surface(&w.surface);
        }
    }

    fn config_changed(&mut self, st: &mut State, look: bool) {
        if look {
            // (Glass and Material draw different windows: made again on the next change)
            self.drop_all(st);
        }
        self.refresh(st);
    }

    fn plugin_changed(&mut self, st: &mut State) {
        if let Some(w) = &mut self.capsule {
            st.shell.apply_theme(&w.ui.global::<ui::Theme>());
            w.glass_sent.clear();
        }
        self.refresh(st);
    }

    fn data(&mut self, st: &mut State, what: TypeId) {
        if what == TypeId::of::<Sound>() {
            let s = st.shell.get::<Sound>().cloned().unwrap_or_default();
            for cue in self.watch.sound(&s, self.now()) {
                match cue {
                    Cue::Show(k) => self.show(st, k, String::new()),
                    Cue::Switched(k) => {
                        // (both switching: the output's name)
                        if !(k == Kind::Mic && self.switch_kind == Some(Kind::Volume)) {
                            self.switch_kind = Some(k);
                            self.after(st, &self.switch, 700, || Msg::Switch);
                        }
                    }
                }
            }
            self.refresh(st);
        } else if what == TypeId::of::<Backlight>() {
            let b = st.shell.get::<Backlight>().cloned().unwrap_or_default();
            let changed = self.backlight_seen.is_some_and(|v| (v - b.value).abs() > 1e-6);
            self.backlight_seen = Some(b.value);
            if changed {
                self.show(st, Kind::Brightness, String::new());
            } else {
                self.refresh(st);
            }
        }
    }

    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        // From the shell (stdin): its own sliders, and previews
        let msg = match msg.downcast::<serde_json::Value>() {
            Ok(v) => {
                if let Some(k) = v["hush"].as_str().and_then(kind_named) {
                    self.hush(k);
                    if k == Kind::Volume {
                        self.hush(Kind::Mic);
                    }
                }
                if let Some(k) = v["preview"].as_str().and_then(kind_named) {
                    self.armed = true;
                    self.show(st, k, String::new());
                }
                return;
            }
            Err(m) => m,
        };
        let Ok(m) = msg.downcast::<Msg>() else { return };
        match *m {
            Msg::Arm => self.armed = true,
            Msg::Hide => self.hide_now(st),
            Msg::BarHide => {
                self.bar_shown = false;
                if let Some(w) = &self.bar {
                    w.ui.set_shown(false);
                }
            }
            Msg::Switch => {
                let Some(k) = self.switch_kind.take() else { return };
                let s = st.shell.get::<Sound>().cloned().unwrap_or_default();
                let d = if k == Kind::Mic { s.source() } else { s.sink() };
                let name = d.map(audio::name_of).unwrap_or_default();
                self.show(st, k, name);
            }
            Msg::SetLevel(v) => self.set_level(st, v as f64),
            Msg::ToggleMute => self.toggle_mute(st),
            Msg::ToggleLevels => {
                self.levels_open = !self.levels_open;
                self.page = "list";
                self.refresh(st);
            }
            Msg::Closed => {
                if self.shown {
                    return;
                }
                if let Some(w) = &mut self.capsule {
                    w.surface.hide();
                    st.shell.glass.set(&w.output, HUD_NS, &[]);
                    w.glass_sent.clear();
                }
                if let Some(w) = &mut self.panel {
                    w.surface.hide();
                }
            }
            Msg::BarClosed => {
                if !self.bar_shown {
                    if let Some(w) = &self.bar {
                        w.surface.hide();
                    }
                }
            }
            Msg::Picked(i) => self.picked(st, i),
            Msg::RowLevel(i, v) => self.row_level(st, i, v as f64),
        }
    }

    fn surface_closed(&mut self, st: &mut State, s: &Rc<Surface>) {
        let name = s.output.clone();
        self.output_removed(st, &name);
    }

    /// The pointer resting on it, its size and where it takes the pointer
    /// (as the levels open), and the capsule's glass shape as it fades and grows
    fn after_frame(&mut self, st: &mut State) {
        let wins: [Option<&mut crate::a11y::A11y>; 3] = [
            self.capsule.as_mut().and_then(|w| w.a11y.as_mut()),
            self.panel.as_mut().and_then(|w| w.a11y.as_mut()),
            self.bar.as_mut().and_then(|w| w.a11y.as_mut()),
        ];
        for a in wins.into_iter().flatten() {
            a.update(false, false);
        }

        let pointer = self.capsule.as_ref().is_some_and(|w| w.ui.get_pointer_on()) || self.panel.as_ref().is_some_and(|w| w.ui.get_pointer_on());
        if pointer != self.pointer_on {
            if crate::env::is_set("DEBUG") {
                eprintln!("debug: osd pointer {pointer}");
            }
            self.pointer_on = pointer;
            if pointer {
                self.hide.stop();
            } else if self.shown {
                self.after(st, &self.hide, self.hide_after, || Msg::Hide);
            }
        }

        if let Some(w) = &mut self.capsule {
            if self.shown {
                let h = 52 + 2 * M as u32 + w.ui.get_levels_height().ceil() as u32;
                let size = (280 + 2 * M as u32, h);
                if size != w.size || !w.surface.mapped.get() && w.size == (0, 0) {
                    w.size = size;
                    w.surface.show(size.0, size.1);
                }
                let region = vec![(M, M, 280, h as i32 - 2 * M)];
                if region != w.region {
                    st.set_input_region(&w.surface, Some(&region));
                    w.region = region;
                }
            }
            if st.shell.plugin && !st.shell.look.material && w.surface.mapped.get() {
                let (ww, wh) = w.surface.logical_size();
                let p = w.ui.get_progress().max(0.0);
                let s = 0.94 + 0.06 * p;
                // (scaled about the window's centre, as the QML Glass item)
                let (cx, cy) = (ww as f32 / 2.0, wh as f32 / 2.0);
                let (x, y) = (cx + (M as f32 - cx) * s, cy + (M as f32 - cy) * s);
                let shape = theme::glass_shape(&st.shell.look, x, y, 280.0 * s, (wh as f32 - 2.0 * M as f32) * s, 26.0 * s, p.min(1.0), 1.0);
                let key = if p > 0.002 { format!("{shape:?}") } else { String::new() };
                if key != w.glass_sent {
                    st.shell.glass.set(&w.output, HUD_NS, if p > 0.002 { std::slice::from_ref(&shape) } else { &[] });
                    w.glass_sent = key;
                }
            }
        }

        if let Some(w) = &mut self.panel {
            let ui = &w.ui;
            let r = |v: f32| v.round() as i32;
            let mut region = Vec::new();
            if self.shown && w.surface.mapped.get() {
                region.push((r(ui.get_panel_x0()), r(ui.get_panel_y0()), r(ui.get_panel_w()), r(ui.get_panel_h())));
                if self.levels_open {
                    region.push((r(ui.get_card_x()), r(ui.get_card_y()), r(ui.get_card_w()), r(ui.get_card_h())));
                }
            }
            if region != w.region {
                st.set_input_region(&w.surface, Some(&region));
                w.region = region;
            }
        }
    }

    fn a11y(&mut self) -> Vec<&mut crate::a11y::A11y> {
        let mut v = Vec::new();
        if let Some(a) = self.capsule.as_mut().and_then(|w| w.a11y.as_mut()) {
            v.push(a);
        }
        if let Some(a) = self.panel.as_mut().and_then(|w| w.a11y.as_mut()) {
            v.push(a);
        }
        if let Some(a) = self.bar.as_mut().and_then(|w| w.a11y.as_mut()) {
            v.push(a);
        }
        v
    }

    fn stats(&self) {
        if let Some(w) = &self.capsule {
            eprintln!("tessera-ui: stats {}", w.surface.stats.borrow_mut().report("osd"));
        }
        if let Some(w) = &self.panel {
            eprintln!("tessera-ui: stats {}", w.surface.stats.borrow_mut().report("osd"));
        }
    }

    /// On the way out: no glass left behind, and the banners back up
    fn quit(&mut self, _: &mut State) {
        if let Some(w) = &self.capsule {
            Glass::clear_now(&w.output, HUD_NS);
        }
        if self.told_shown {
            crate::surfaces::tell(serde_json::json!({"src": "ui", "kind": "osd", "shown": false}));
        }
    }
}

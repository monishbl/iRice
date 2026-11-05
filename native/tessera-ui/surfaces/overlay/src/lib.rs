//! The overlay (rust.surfaces: overlay), compare
//! shell/modules/overlay/Overlay.qml and OverlayWidget.qml (Super+G):
//! widgets over everything. While open the screen dims and a toolbar
//! toggles widgets; pinned widgets stay after closing (optionally
//! click-through: then only the others take clicks). A crosshair can be
//! drawn at the screen's centre (gaming overlays). Widgets: notes, resources,
//! the recorder, a volume mixer, and with gaming overlays a floating image,
//! the crosshair's settings and an FPS limit.
//!
//! A panel: Panels opens and closes it, RustUi.qml forwards those; Esc or a
//! click on the dim closes it. The widgets' places, pins and the crosshair
//! are the shell's state (UiState): Overlay.qml tells this part
//! `{"to":"overlay","state":{widgets, crosshair, floatingImage, recording,
//! notes}}` and every change goes back as an `overlay.*` call (checked here
//! first: model.rs), which Overlay.qml applies with its own functions, as
//! the QML widgets did (recording, pasting the image, MangoHud's limit).
//! The notes are a file of their own (notes.md in the shell's state
//! folder), read and written here as the QML did; the resources sample here
//! (tessera_core::sysstat); the mixer is the sound server's (services::audio).
//!
//! Where it differs: the mixer's Material slider is the medium Expressive
//! track without its icon's morph; the notes field is a plain multi-line
//! field (no QML ScrollView bar).

pub mod ui {
    slint::include_modules!();
}
tessera_core::theme_global!(ui);

// tessera-core's modules (shell, wayland, theme, services, util…) under the
// names they had when everything was one crate: `crate::shell::Part`
#[allow(unused_imports)]
use tessera_core::*;

pub mod model;

use tessera_core::services::audio::{self, Sound};
use tessera_core::shell::{self, Command, Part};
use tessera_core::sysstat::Sampler;
use tessera_core::surfaces::kit::hex;
tessera_core::gl_fn!(ui);
use tessera_core::theme;
use tessera_core::wayland::{Spec, State, Surface};
use model::{Crosshair, M};
use serde_json::{json, Value};
use slint::{ComponentHandle, ModelRc, VecModel};
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use std::any::{Any, TypeId};
use std::rc::Rc;

const NAME: &str = "overlay";
const NS: &str = "tessera:overlay";

enum Msg {
    Dismissed,
    Toggle(String),
    Flag(String, String),
    Moved(String, f32, f32, f32, f32),
    Saved(String),
    Notes(String),
    SaveNotes,
    Record(String),
    Paste,
    Fps(i32),
    Cross(String, f32),
    CrossColor(String),
    Mixer(i32, f32),
    Tick,
    Picture(u64, Option<slint::SharedPixelBuffer<slint::Rgb8Pixel>>),
}

struct Win {
    output: String,
    surface: Rc<Surface>,
    ui: ui::OverlayUi,
    a11y: Option<crate::a11y::A11y>,
    region: Option<Vec<(i32, i32, i32, i32)>>,
    glass_sent: String,
    settled: bool,
}

#[derive(Default)]
pub struct Overlay {
    win: Option<Win>,
    open: bool,
    output: Option<String>,
    /// UiState.data.overlay as the shell last said, with this part's own
    /// changes on top until it says again
    widgets: Value,
    cross: Option<Crosshair>,
    image: String,
    image_stamp: Option<std::time::SystemTime>,
    image_gen: u64,
    recording: bool,
    notes_path: String,
    notes_loaded: bool,
    notes_text: String,
    notes_timer: Option<slint::Timer>,
    fps_status: String,
    sampler: Sampler,
    sampling: Option<slint::Timer>,
    started: Option<std::time::Instant>,
}

fn gaming(st: &State) -> bool {
    st.shell.cfg["overlays"]["gaming"].as_bool().unwrap_or(false)
}

fn ov(w: &model::Widget, shown: bool) -> ui::OvWidget {
    ui::OvWidget { shown, pinned: w.pinned, clickthrough: w.clickthrough, open: w.open, x: w.x as f32, y: w.y as f32, w: w.w as f32, h: w.h as f32 }
}

impl Overlay {
    fn cross(&self) -> Crosshair {
        self.cross.clone().unwrap_or_else(|| model::crosshair(&Value::Null))
    }

    fn widget(&self, id: &str) -> model::Widget {
        model::widget(&self.widgets, id)
    }

    /// Change a widget here at once and ask the shell to keep it
    fn set_widget(&mut self, id: &str, p: Value, tell: bool) {
        let Some(p) = model::patch(id, &p) else { return };
        if !self.widgets.is_object() {
            self.widgets = json!({});
        }
        let mut cur = self.widgets.get(id).cloned().unwrap_or(json!({}));
        if !cur.is_object() {
            cur = json!({});
        }
        if let (Some(c), Some(o)) = (cur.as_object_mut(), p.as_object()) {
            for (k, v) in o {
                c.insert(k.clone(), v.clone());
            }
        }
        self.widgets[id] = cur;
        if tell {
            shell::ask("overlay.widget", json!([id, p]));
        }
    }

    fn now(&mut self) -> f64 {
        self.started.get_or_insert_with(std::time::Instant::now).elapsed().as_secs_f64() + 100.0
    }

    /// The window exists while open, a widget is pinned or the crosshair is on
    fn ensure(&mut self, st: &mut State) {
        let g = gaming(st);
        let want = model::active(self.open, &self.widgets, g, &self.cross());
        if !want {
            if let Some(w) = self.win.take() {
                st.shell.glass.set(&w.output, NS, &[]);
                w.surface.hide(); // (unmapped now: dropping the window alone leaves it on screen)
                st.drop_surface(&w.surface);
            }
            self.sampling = None;
            return;
        }
        // (the screen it last opened on, else the focused one: Hyprland.focusedMonitor)
        let focused = st.shell.get::<crate::services::hypr::Snapshot>().map(|h| h.focused.clone()).filter(|f| !f.is_empty());
        let output = self.output.clone().or(focused).filter(|o| st.output_named(o).is_some()).or_else(|| st.outputs_now().first().map(|o| o.1.clone()));
        let Some(output) = output else { return };
        if self.win.as_ref().is_some_and(|w| w.output != output) {
            let w = self.win.take().unwrap();
            st.shell.glass.set(&w.output, NS, &[]);
            w.surface.hide(); // (unmapped now: dropping the window alone leaves it on screen)
            st.drop_surface(&w.surface);
        }
        if self.win.is_none() {
            let Some(wl_output) = st.output_named(&output) else { return };
            let surface = st.prepare_surface(Spec {
                layer: Layer::Overlay,
                namespace: NS,
                output: Some((&wl_output, &output)),
                anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
                size: (0, 0),
                exclusive: -1,
                keyboard: if self.open { KeyboardInteractivity::Exclusive } else { KeyboardInteractivity::None },
                margin_left: 0,
            });
            let ui = ui::OverlayUi::new().expect("OverlayUi");
            st.shell.apply_theme(&ui.global::<ui::Theme>());
            self.wire(st, &ui);
            ui.set_icons(ModelRc::new(VecModel::from(model::WIDGETS.iter().map(|w| gl(w.2)).collect::<Vec<_>>())));
            ui.global::<ui::OvG>().set_bar_icons(ModelRc::new(VecModel::from(["cursor-click", "push-pin", "x", "arrows-out-simple"].iter().map(|n| gl(n)).collect::<Vec<_>>())));
            ui.set_swatches(ModelRc::new(VecModel::from(model::COLORS.iter().map(|c| hex(c)).collect::<Vec<_>>())));
            ui.show().expect("show overlay");
            let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
            self.win = Some(Win { output: output.clone(), surface, ui, a11y, region: Some(vec![(-1, -1, -1, -1)]), glass_sent: String::new(), settled: false });
        }
        self.load_notes();
        self.push(st);
    }

    fn wire(&self, st: &State, ui: &ui::OverlayUi) {
        let to = st.shell.sender(NAME);
        let t = to.clone();
        ui.on_dismissed(move || t(Box::new(Msg::Dismissed)));
        let t = to.clone();
        ui.on_toggle(move |id| t(Box::new(Msg::Toggle(id.to_string()))));
        let t = to.clone();
        ui.on_flag(move |id, f| t(Box::new(Msg::Flag(id.to_string(), f.to_string()))));
        let t = to.clone();
        ui.on_moved(move |id, x, y, w, h| t(Box::new(Msg::Moved(id.to_string(), x, y, w, h))));
        let t = to.clone();
        ui.on_saved(move |id| t(Box::new(Msg::Saved(id.to_string()))));
        let t = to.clone();
        ui.on_notes_edited(move |s| t(Box::new(Msg::Notes(s.to_string()))));
        let t = to.clone();
        ui.on_record(move |k| t(Box::new(Msg::Record(k.to_string()))));
        let t = to.clone();
        ui.on_paste(move || t(Box::new(Msg::Paste)));
        let t = to.clone();
        ui.on_fps(move |n| t(Box::new(Msg::Fps(n))));
        let t = to.clone();
        ui.on_cross_set(move |k, v| t(Box::new(Msg::Cross(k.to_string(), v))));
        let t = to.clone();
        ui.on_cross_color(move |c| t(Box::new(Msg::CrossColor(c.to_string()))));
        let t = to;
        ui.on_mixer(move |i, v| t(Box::new(Msg::Mixer(i, v))));
    }

    /// Everything the window shows, from the state here
    fn push(&mut self, st: &mut State) {
        let g = gaming(st);
        let open = self.open;
        let cross = self.cross();
        let res_shown = self.widget("resources").shown("resources", open, g);
        let Some(win) = self.win.as_ref() else { return };
        let ui = &win.ui;
        ui.set_open(open);
        ui.set_gaming(g);
        ui.set_on_glass(st.shell.plugin && !st.shell.look.material);
        ui.global::<ui::OvG>().set_open(open);
        ui.global::<ui::OvG>().set_on_glass(st.shell.plugin && !st.shell.look.material);
        let w = |id: &str| {
            let x = model::widget(&self.widgets, id);
            ov(&x, x.shown(id, open, g))
        };
        ui.set_notes(w("notes"));
        ui.set_resources(w("resources"));
        ui.set_recorder(w("recorder"));
        ui.set_mixer_w(w("mixer"));
        ui.set_image_w(w("image"));
        ui.set_crosshair_w(w("crosshair"));
        ui.set_fps_w(w("fps"));
        let tools: Vec<ui::OvTool> = model::ids(g).iter().map(|id| ui::OvTool { id: (*id).into(), icon: gl(model::WIDGETS.iter().find(|x| x.0 == *id).map_or("x", |x| x.2)), on: self.widget(id).open, name: model::WIDGETS.iter().find(|x| x.0 == *id).map_or("", |x| x.1).into() }).collect();
        ui.set_tools(ModelRc::new(VecModel::from(tools)));
        ui.set_cross(ui::OvCross { enabled: cross.enabled && g, color: hex(&cross.color), hex: cross.color.clone().into(), size: cross.size as f32, thickness: cross.thickness as f32, gap: cross.gap as f32, dot: cross.dot });
        ui.set_recording(self.recording);
        ui.set_has_image(!self.image.is_empty());
        ui.set_fps_status(self.fps_status.clone().into());
        if ui.get_notes_text().as_str() != self.notes_text && !ui.get_notes_focused() {
            ui.set_notes_text(self.notes_text.clone().into());
        }
        // (the mixer, from the sound server)
        let sliders = self.sliders(st);
        win.ui.set_sliders(ModelRc::new(VecModel::from(sliders)));
        // input and keyboard
        let rects = model::input_rects(open, &self.widgets, g);
        let win = self.win.as_mut().unwrap();
        if rects != win.region {
            win.region = rects.clone();
            st.set_input_region(&win.surface, rects.as_deref());
        }
        win.surface.layer.set_keyboard_interactivity(if open { KeyboardInteractivity::Exclusive } else { KeyboardInteractivity::None });
        win.surface.wl.commit();
        // Resources: sampled once a second while its widget shows
        if res_shown && self.sampling.is_none() {
            let now = self.now();
            self.sampler.sample(now);
            let tx = st.tx.clone();
            let t = slint::Timer::default();
            t.start(slint::TimerMode::Repeated, std::time::Duration::from_secs(1), move || {
                let _ = tx.send(tessera_core::Msg::Part(NAME, Box::new(Msg::Tick)));
            });
            self.sampling = Some(t);
        } else if !res_shown {
            self.sampling = None;
        }
        self.resources();
        self.picture(st);
    }

    fn sliders(&self, st: &State) -> Vec<ui::OvSlider> {
        let Some(s) = st.shell.get::<Sound>() else { return vec![ui::OvSlider { label: "Output".into(), icon: gl("speaker-high"), value: 0.0 }] };
        let mut out = vec![ui::OvSlider { label: "Output".into(), icon: gl(model::volume_icon(s.volume(), s.muted())), value: if s.muted() { 0.0 } else { s.volume() as f32 } }];
        for x in &s.streams {
            out.push(ui::OvSlider { label: x.name.clone().into(), icon: gl(if x.muted { "speaker-x" } else { "speaker-high" }), value: if x.muted { 0.0 } else { x.volume as f32 } });
        }
        out
    }

    fn resources(&mut self) {
        let Some(win) = &self.win else { return };
        let stats = self.sampler.stats();
        let find = |k: &str| stats.iter().find(|s| s.key == k);
        let width = (win.ui.get_resources().w - 24.0).max(1.0) as f64;
        let cpu = find("cpu");
        let temp = find("temperature").and_then(|t| t.history.last().copied()).filter(|t| *t > 0.0);
        let cpu_v = cpu.and_then(|c| c.history.last().copied()).unwrap_or(0.0);
        let cpu_detail = format!("{}%{}", (cpu_v * 100.0).round(), temp.map_or(String::new(), |t| format!(" · {}°C", t.round())));
        let info = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
        let mem_detail = crate::surfaces::kit::mem_detail(&info).replace(" of ", " / ");
        let (ca, cl) = model::line_graph(&cpu.map(|c| c.history.clone()).unwrap_or_default(), width, 52.0);
        let (ma, ml) = model::line_graph(&find("memory").map(|c| c.history.clone()).unwrap_or_default(), width, 52.0);
        win.ui.set_res(ui::OvRes { cpu_detail: cpu_detail.into(), mem_detail: mem_detail.into(), cpu_area: ca.into(), cpu_line: cl.into(), mem_area: ma.into(), mem_line: ml.into(), gw: width as f32 });
    }

    /// The floating image, decoded again when its file changes
    fn picture(&mut self, st: &State) {
        if self.image.is_empty() || self.win.is_none() {
            return;
        }
        let stamp = std::fs::metadata(&self.image).and_then(|m| m.modified()).ok();
        if stamp == self.image_stamp && stamp.is_some() {
            return;
        }
        self.image_stamp = stamp;
        self.image_gen += 1;
        let (gen, path, tx) = (self.image_gen, self.image.clone(), st.tx.clone());
        std::thread::spawn(move || {
            // (fitted, not cropped: decoded whole, at most 1600 px)
            let px = image::ImageReader::open(&path).ok().and_then(|r| r.with_guessed_format().ok()).and_then(|r| r.decode().ok()).map(|img| {
                let img = if img.width() > 1600 || img.height() > 1600 { img.resize(1600, 1600, image::imageops::FilterType::Triangle) } else { img };
                let rgb = img.into_rgb8();
                slint::SharedPixelBuffer::<slint::Rgb8Pixel>::clone_from_slice(rgb.as_raw(), rgb.width(), rgb.height())
            });
            let _ = tx.send(tessera_core::Msg::Part(NAME, Box::new(Msg::Picture(gen, px))));
        });
    }

    fn load_notes(&mut self) {
        if self.notes_loaded {
            return;
        }
        let path = if self.notes_path.is_empty() { crate::util::state_dir().join("notes.md").to_string_lossy().into_owned() } else { self.notes_path.clone() };
        self.notes_text = std::fs::read_to_string(&path).unwrap_or_default();
        self.notes_loaded = true;
    }

    fn save_notes(&self) {
        let path = if self.notes_path.is_empty() { crate::util::state_dir().join("notes.md") } else { std::path::PathBuf::from(&self.notes_path) };
        if let Some(d) = path.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = crate::util::write_atomic(&path, self.notes_text.as_bytes());
    }

    fn told(&mut self, st: &mut State, v: &Value) {
        let s = &v["state"];
        if !s.is_object() {
            return;
        }
        if s["widgets"].is_object() {
            self.widgets = s["widgets"].clone();
        }
        if s["crosshair"].is_object() {
            self.cross = Some(model::crosshair(&s["crosshair"]));
        }
        if let Some(i) = s["floatingImage"].as_str() {
            if i != self.image {
                self.image_stamp = None;
            }
            self.image = i.to_string();
        }
        if let Some(r) = s["recording"].as_bool() {
            self.recording = r;
        }
        if let Some(n) = s["notes"].as_str() {
            if n != self.notes_path {
                self.notes_path = n.to_string();
                self.notes_loaded = false;
            }
        }
        self.ensure(st);
    }

    fn close(&mut self, st: &mut State, tell_shell: bool) {
        if self.open {
            self.open = false;
            if tell_shell {
                shell::closed(NAME);
            }
        }
        self.ensure(st);
    }
}

impl Part for Overlay {
    fn name(&self) -> &'static str {
        NAME
    }

    fn start(&mut self, st: &mut State) {
        st.shell.want("audio");
        st.shell.want("hypr");
    }

    fn command(&mut self, st: &mut State, cmd: &Command) {
        match cmd.cmd.as_str() {
            "open" => {
                if cmd.screen.is_some() {
                    self.output = cmd.screen.clone();
                }
                self.open = true;
                self.ensure(st);
                if let Some(w) = &self.win {
                    w.ui.invoke_focus_keys();
                }
            }
            "close" => self.close(st, false),
            _ => {}
        }
    }

    fn data(&mut self, st: &mut State, what: TypeId) {
        if what == TypeId::of::<Sound>() {
            if let Some(w) = &self.win {
                let s = self.sliders(st);
                w.ui.set_sliders(ModelRc::new(VecModel::from(s)));
            }
        }
    }

    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        let msg = match msg.downcast::<Value>() {
            Ok(v) => return self.told(st, &v),
            Err(m) => m,
        };
        let Ok(m) = msg.downcast::<Msg>() else { return };
        match *m {
            Msg::Dismissed => self.close(st, true),
            Msg::Toggle(id) => {
                let on = self.widget(&id).open;
                self.set_widget(&id, json!({"open": !on}), true);
                self.ensure(st);
            }
            Msg::Flag(id, f) => {
                let w = self.widget(&id);
                let p = match f.as_str() {
                    "clickthrough" => json!({"clickthrough": !w.clickthrough}),
                    "pinned" => json!({"pinned": !w.pinned}),
                    "close" => json!({"open": false}),
                    _ => return,
                };
                self.set_widget(&id, p, true);
                self.ensure(st);
            }
            Msg::Moved(id, x, y, w, h) => {
                self.set_widget(&id, json!({"x": x, "y": y, "w": w, "h": h}), false);
                self.push(st);
            }
            Msg::Saved(id) => {
                let w = self.widget(&id);
                shell::ask("overlay.widget", json!([id, model::patch(&id, &json!({"x": w.x, "y": w.y, "w": w.w, "h": w.h}))]));
            }
            Msg::Notes(text) => {
                self.notes_text = text;
                let tx = st.tx.clone();
                let t = slint::Timer::default();
                t.start(slint::TimerMode::SingleShot, std::time::Duration::from_millis(600), move || {
                    let _ = tx.send(tessera_core::Msg::Part(NAME, Box::new(Msg::SaveNotes)));
                });
                self.notes_timer = Some(t);
            }
            Msg::SaveNotes => self.save_notes(),
            Msg::Record(k) => {
                if ["screen", "area", "stop"].contains(&k.as_str()) {
                    shell::ask("overlay.record", json!([k]));
                }
            }
            Msg::Paste => shell::ask("overlay.pasteImage", json!([])),
            Msg::Fps(n) => {
                if [0, 30, 60, 90, 120].contains(&n) {
                    shell::ask("overlay.fps", json!([n]));
                    self.fps_status = model::fps_status(n as u32);
                    self.push(st);
                }
            }
            Msg::Cross(k, v) => {
                let value = if k == "enabled" || k == "dot" { json!(v > 0.5) } else { json!(v) };
                if let Some(p) = model::crosshair_patch(&k, &value) {
                    let mut c = self.cross_json();
                    for (pk, pv) in p.as_object().into_iter().flatten() {
                        c[pk] = pv.clone();
                    }
                    self.cross = Some(model::crosshair(&c));
                    shell::ask("overlay.crosshair", json!([p]));
                    self.ensure(st);
                }
            }
            Msg::CrossColor(c) => {
                if let Some(p) = model::crosshair_patch("color", &json!(c)) {
                    let mut x = self.cross();
                    x.color = c;
                    self.cross = Some(x);
                    shell::ask("overlay.crosshair", json!([p]));
                    self.push(st);
                }
            }
            Msg::Mixer(i, v) => {
                let Some(s) = st.shell.get::<Sound>() else { return };
                let v = (v as f64).clamp(0.0, 1.0);
                if i == 0 {
                    if let Some(d) = s.sink() {
                        audio::send(audio::Cmd::SinkMute { index: d.index, muted: false });
                        audio::send(audio::Cmd::SinkVolume { index: d.index, channels: d.channels, volume: v });
                    }
                } else if let Some(x) = usize::try_from(i - 1).ok().and_then(|i| s.streams.get(i)) {
                    audio::send(audio::Cmd::StreamMute { index: x.index, muted: false });
                    audio::send(audio::Cmd::StreamVolume { index: x.index, channels: x.channels, volume: v });
                }
            }
            Msg::Tick => {
                let now = self.now();
                self.sampler.sample(now);
                self.resources();
            }
            Msg::Picture(gen, px) => {
                if gen == self.image_gen {
                    if let (Some(w), Some(px)) = (&self.win, px) {
                        w.ui.set_picture(slint::Image::from_rgb8(px));
                    }
                }
            }
        }
    }

    fn config_changed(&mut self, st: &mut State, look: bool) {
        if look {
            if let Some(w) = self.win.as_mut() {
                st.shell.apply_theme(&w.ui.global::<ui::Theme>());
                w.glass_sent.clear();
            }
        }
        self.ensure(st);
    }

    fn plugin_changed(&mut self, st: &mut State) {
        self.config_changed(st, true);
    }

    fn keyboard_left(&mut self, _: &mut State, _: &Rc<Surface>) {}

    fn output_removed(&mut self, st: &mut State, name: &str) {
        if self.win.as_ref().is_some_and(|w| w.output == name) {
            let w = self.win.take().unwrap();
            w.surface.hide(); // (unmapped now: dropping the window alone leaves it on screen)
            st.drop_surface(&w.surface);
            self.output = None;
            self.ensure(st);
        }
    }

    fn surface_closed(&mut self, st: &mut State, s: &Rc<Surface>) {
        if self.win.as_ref().is_some_and(|w| Rc::ptr_eq(&w.surface, s)) {
            let name = s.output.clone();
            self.output_removed(st, &name);
        }
    }

    /// The glass: the toolbar while open, and every widget on screen
    fn after_frame(&mut self, st: &mut State) {
        let kbd = st.keyboard_surface();
        let g = gaming(st);
        let open = self.open;
        let widgets = self.widgets.clone();
        let Some(w) = self.win.as_mut() else { return };
        crate::picture::second_frame(&w.surface, &mut w.settled);
        let focused = kbd.as_ref().is_some_and(|k| Rc::ptr_eq(k, &w.surface));
        if let Some(a) = w.a11y.as_mut() {
            a.update(false, focused);
        }
        if !st.shell.plugin || st.shell.look.material || !w.surface.mapped.get() {
            return;
        }
        let look = &st.shell.look;
        let mut shapes = Vec::new();
        if open {
            let (x, y, ww, hh) = (w.ui.get_bar_x(), w.ui.get_bar_y(), w.ui.get_bar_w(), w.ui.get_bar_h());
            shapes.push(theme::glass_shape(look, x, y, ww, hh, 26.0, 1.0, 1.0));
        }
        for id in model::ids(g) {
            let x = model::widget(&widgets, id);
            if x.shown(id, open, g) {
                let o = if !open && x.clickthrough { 0.55 } else { 1.0 };
                shapes.push(theme::glass_shape(look, x.x as f32, x.y as f32, x.w as f32, x.h as f32, 20.0, o, 1.0));
            }
        }
        let _ = M;
        let key = format!("{shapes:?}");
        if key != w.glass_sent {
            st.shell.glass.set(&w.output, NS, &shapes);
            w.glass_sent = key;
        }
    }

    fn a11y(&mut self) -> Vec<&mut crate::a11y::A11y> {
        self.win.as_mut().and_then(|w| w.a11y.as_mut()).into_iter().collect()
    }

    fn quit(&mut self, _: &mut State) {
        if let Some(w) = &self.win {
            crate::services::hypr::Glass::clear_now(&w.output, NS);
        }
        if self.notes_timer.is_some() {
            self.save_notes();
        }
    }
}

impl Overlay {
    fn cross_json(&self) -> Value {
        let c = self.cross();
        json!({"enabled": c.enabled, "color": c.color, "size": c.size, "thickness": c.thickness, "gap": c.gap, "dot": c.dot})
    }
}

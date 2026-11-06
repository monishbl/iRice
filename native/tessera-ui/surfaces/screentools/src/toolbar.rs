//! ⇧⌘5's toolbar (ScreenToolbar.qml), the "screenTools" panel: every tool
//! (capture screen or selection, copy text, translate, ask AI, record), or
//! from Control Center's Record only the recording ones (Entire screen,
//! Window, Selected region, then Record). Options open above the bar: the
//! microphone, system audio, the pointer, the countdown, frame rate, format,
//! quality and the folder. Esc closes the options, then the toolbar. A full
//! screen overlay that takes the pointer only over the bar and its options.

use super::geometry::{self, Act, RecCfg, Tool, CODECS, COUNTDOWNS, QUALITIES};
use super::{ic, Bridge, M};
use crate::theme;
use crate::ui;
use crate::wayland::{Spec, State, Surface};
use serde_json::json;
use slint::platform::Key;
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use smithay_client_toolkit::shell::WaylandSurface;
use std::rc::Rc;

const NS: &str = "tessera:screenTools";
const PANEL: &str = "screentools";

struct Win {
    output: String,
    surface: Rc<Surface>,
    ui: ui::ScreenToolbar,
    a11y: Option<crate::a11y::A11y>,
    glass_sent: String,
    region_sent: Vec<(i32, i32, i32, i32)>,
}

#[derive(Default)]
pub struct Toolbar {
    win: Option<Win>,
    open: bool,
    options: bool,
    record_only: bool,
    /// what the shell said just before it opens: Options showing, recording only
    pending: Option<(bool, bool)>,
    recording: bool,
    cfg: Option<RecCfg>,
    tools: Vec<Tool>,
    mics: Vec<(String, String)>,
}

impl Toolbar {
    fn cfg(&self, st: &State) -> RecCfg {
        self.cfg.clone().unwrap_or_else(|| RecCfg::from_cfg(&st.shell.cfg))
    }

    fn window(&mut self, st: &mut State, output: &str) -> bool {
        if self.win.as_ref().is_some_and(|w| w.output != output) {
            let w = self.win.take().unwrap();
            crate::services::hypr::Glass::clear_now(&w.output, NS);
            st.drop_surface(&w.surface);
        }
        if self.win.is_some() {
            return true;
        }
        let Some(wl) = st.output_named(output) else { return false };
        let surface = st.prepare_surface(Spec {
            layer: Layer::Overlay,
            namespace: NS,
            output: Some((&wl, output)),
            anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
            size: (0, 0),
            exclusive: -1,
            keyboard: KeyboardInteractivity::None,
            margin_left: 0,
        });
        let ui = ui::ScreenToolbar::new().expect("ScreenToolbar");
        st.shell.apply_theme(&ui.global::<ui::Theme>());
        let to = st.shell.sender(super::NAME);
        macro_rules! on {
            ($set:ident, |$($a:ident),*| $m:expr) => {{
                let to = to.clone();
                ui.$set(move |$($a),*| to(Box::new($m)));
            }};
        }
        on!(on_tool, |i| M::Tool(i));
        on!(on_record, | | M::Record);
        on!(on_record_area, |a| M::RecordArea(a.to_string()));
        on!(on_mic, |i| M::Mic(i));
        on!(on_set_system_audio, | | M::SystemAudio);
        on!(on_set_cursor, | | M::Cursor);
        on!(on_set_countdown, |i| M::Countdown(i));
        on!(on_set_fps, |i| M::Fps(i));
        on!(on_set_codec, |i| M::Codec(i));
        on!(on_set_quality, |i| M::Quality(i));
        on!(on_folder_reset, | | M::FolderReset);
        on!(on_folder_pick, | | M::FolderPick);
        on!(on_folder_open, | | M::FolderOpen);
        on!(on_closed, | | M::ToolbarClosed);
        {
            let (to, weak) = (to.clone(), ui.as_weak());
            ui.on_key(move |text| {
                let Some(ui) = weak.upgrade() else { return false };
                let is = |k: Key| text == SharedString::from(k);
                if is(Key::Escape) {
                    if ui.get_menu_open() {
                        ui.set_menu_open(false);
                    } else if ui.get_options_open() {
                        to(Box::new(M::Tool(-1)));
                    } else {
                        to(Box::new(M::ToolbarDismiss));
                    }
                    return true;
                }
                if is(Key::Return) && ui.get_record_only() {
                    to(Box::new(M::Record));
                    return true;
                }
                false
            });
        }
        let m = st.shell.look.material;
        ui.set_ic_record(ic("record", m));
        ui.set_ic_monitor(ic("monitor", m));
        ui.set_ic_window(ic("app-window", m));
        ui.set_ic_selection(ic("selection", m));
        ui.set_ic_reset(ic("arrow-counter-clockwise", m));
        ui.set_ic_pencil(ic("pencil-simple", m));
        ui.set_ic_folder(ic("folder-open", m));
        ui.set_ic_caret_up(ic("caret-up", m));
        ui.set_ic_caret_down(ic("caret-down", m));
        ui.show().expect("show toolbar");
        let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
        st.set_input_region(&surface, Some(&[]));
        self.win = Some(Win { output: output.into(), surface, ui, a11y, glass_sent: String::new(), region_sent: Vec::new() });
        true
    }

    pub fn open(&mut self, st: &mut State, screen: Option<String>, b: &Bridge, options: bool) {
        let Some(output) = screen.or_else(|| st.outputs_now().first().map(|o| o.1.clone())) else { return };
        if !self.window(st, &output) {
            return;
        }
        self.open = true;
        self.cfg = None;
        let (o, r) = self.pending.take().unwrap_or((false, b.toolbar_record));
        self.options = options || o;
        self.record_only = r || b.toolbar_record;
        self.recording = b.recording;
        self.mics = b.mics.clone();
        self.push(st, b);
        let w = self.win.as_ref().unwrap();
        w.ui.set_menu_open(false);
        w.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        w.surface.show(0, 0);
        w.ui.set_shown(true);
        w.ui.invoke_focus_keys();
    }

    /// Before it opens: Options showing (Spotlight's "Screen Recording
    /// Options"), recording only (Control Center's Record)
    pub fn prepare(&mut self, options: bool, record: bool) {
        if self.open {
            self.set_options(options || self.options);
        } else {
            self.pending = Some((options, record));
        }
    }

    pub fn set_options(&mut self, open: bool) {
        self.options = open;
        if let Some(w) = &self.win {
            w.ui.set_options_open(open);
        }
    }

    pub fn close(&mut self, tell: bool) {
        if !self.open {
            return;
        }
        self.open = false;
        if let Some(w) = &self.win {
            w.ui.set_shown(false);
            w.ui.set_menu_open(false);
            w.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::None);
            w.surface.layer.commit();
        }
        if tell {
            crate::shell::closed(PANEL);
        }
    }

    /// Its exit finished
    pub fn closed(&mut self, st: &mut State) {
        if self.open {
            return;
        }
        if let Some(w) = &mut self.win {
            w.surface.hide();
            st.shell.glass.set(&w.output, NS, &[]);
            w.glass_sent.clear();
            w.region_sent.clear();
        }
    }

    /// The shell's state changed (recording, the microphones…)
    pub fn bridge(&mut self, st: &mut State, b: &Bridge) {
        if self.open && (b.recording != self.recording || b.mics != self.mics || b.toolbar_record != self.record_only) {
            self.recording = b.recording;
            self.mics = b.mics.clone();
            self.record_only = b.toolbar_record || self.record_only;
            self.push(st, b);
        } else if self.open {
            self.push(st, b);
        }
    }

    pub fn config(&mut self, st: &mut State, look: bool, b: &Bridge) {
        self.cfg = None;
        if let Some(w) = &mut self.win {
            if look {
                st.shell.apply_theme(&w.ui.global::<ui::Theme>());
                w.glass_sent.clear();
            }
        }
        if self.open {
            self.push(st, b);
        }
    }

    fn push(&mut self, st: &State, b: &Bridge) {
        let Some(w) = &self.win else { return };
        let ui = &w.ui;
        let m = st.shell.look.material;
        let c = self.cfg(st);
        self.tools = geometry::tools(self.record_only, b.recording);
        let tools: Vec<ui::StTool> = self
            .tools
            .iter()
            .map(|t| ui::StTool {
                kind: match t.act {
                    Act::Sep => 1,
                    Act::Options => 2,
                    _ => 0,
                },
                icon: ic(t.icon, m),
                label: t.label.into(),
                picked: matches!(t.act, Act::Area(a) if a == c.area),
                red: t.icon == "record" && b.recording,
            })
            .collect();
        ui.set_tools(ModelRc::new(VecModel::from(tools)));
        ui.set_record_only(self.record_only);
        ui.set_options_open(self.options);
        ui.set_available(b.available);
        ui.set_gsr(b.gsr);
        ui.set_bottom(geometry::toolbar_bottom(&st.shell.cfg));
        let mut mics = vec![(String::new(), "None".to_string()), ("default".to_string(), "Default microphone".to_string())];
        mics.extend(self.mics.iter().cloned());
        let opts: Vec<ui::StOpt> = mics.iter().map(|(n, d)| ui::StOpt { text: d.as_str().into(), checked: *n == c.mic }).collect();
        ui.set_mics(ModelRc::new(VecModel::from(opts)));
        ui.set_system_audio(c.system_audio);
        ui.set_cursor(c.cursor);
        ui.set_countdown(COUNTDOWNS.iter().position(|x| *x == c.countdown).unwrap_or(0) as i32);
        ui.set_fps(if c.fps == 30 { 0 } else { 1 });
        ui.set_codec(CODECS.iter().position(|x| *x == c.codec).unwrap_or(0) as i32);
        ui.set_quality(QUALITIES.iter().position(|x| *x == c.quality).unwrap_or(0) as i32);
        ui.set_codec_note(geometry::codec_note(&c.codec).into());
        let folder = if b.folder.is_empty() { format!("{}/Videos/Screen Recordings", b.home) } else { b.folder.clone() };
        ui.set_folder(geometry::tilde(&folder, &b.home).into());
        ui.set_custom_folder(!c.folder.is_empty());
    }

    /// A setting changed here: at once on screen, then in config.json
    fn set(&mut self, st: &State, key: &str, v: serde_json::Value) {
        let mut c = self.cfg(st);
        c.set(key, &v);
        self.cfg = Some(c);
        crate::shell::ask("recorder.set", json!([key, v]));
    }

    fn refresh(&mut self, st: &State) {
        if let Some(w) = &self.win {
            let c = self.cfg(st);
            let ui = &w.ui;
            let tools: Vec<ui::StTool> = (0..ui.get_tools().row_count())
                .filter_map(|i| ui.get_tools().row_data(i))
                .zip(self.tools.iter())
                .map(|(mut t, tool)| {
                    t.picked = matches!(tool.act, Act::Area(a) if a == c.area);
                    t
                })
                .collect();
            ui.set_tools(ModelRc::new(VecModel::from(tools)));
            let mut names = vec![String::new(), "default".to_string()];
            names.extend(self.mics.iter().map(|m| m.0.clone()));
            let opts: Vec<ui::StOpt> = (0..ui.get_mics().row_count())
                .filter_map(|i| ui.get_mics().row_data(i))
                .zip(names.iter())
                .map(|(mut o, n)| {
                    o.checked = *n == c.mic;
                    o
                })
                .collect();
            ui.set_mics(ModelRc::new(VecModel::from(opts)));
            ui.set_system_audio(c.system_audio);
            ui.set_cursor(c.cursor);
            ui.set_countdown(COUNTDOWNS.iter().position(|x| *x == c.countdown).unwrap_or(0) as i32);
            ui.set_fps(if c.fps == 30 { 0 } else { 1 });
            ui.set_codec(CODECS.iter().position(|x| *x == c.codec).unwrap_or(0) as i32);
            ui.set_quality(QUALITIES.iter().position(|x| *x == c.quality).unwrap_or(0) as i32);
            ui.set_codec_note(geometry::codec_note(&c.codec).into());
            ui.set_custom_folder(!c.folder.is_empty());
        }
    }

    /// A tool clicked (-1: close the options)
    pub fn tool(&mut self, st: &mut State, i: i32) {
        if i < 0 {
            self.set_options(false);
            return;
        }
        let Some(t) = self.tools.get(i as usize).cloned() else { return };
        match t.act {
            Act::Sep => {}
            Act::Options => {
                let o = !self.options;
                self.set_options(o);
            }
            Act::Area(a) => {
                self.set(st, "area", json!(a));
                self.refresh(st);
            }
            Act::Shot => crate::shell::ask("screen.do", json!(["shot"])),
            Act::Select(m) => crate::shell::ask("screen.do", json!(["select", m])),
            Act::Record(a) => crate::shell::ask("screen.do", json!(["record", a])),
        }
    }

    pub fn record(&mut self, st: &mut State) {
        let area = self.cfg(st).area;
        crate::shell::ask("screen.do", json!(["record", area]));
    }

    pub fn record_area(&mut self, st: &mut State, area: &str) {
        if let Some(w) = &self.win {
            w.ui.set_menu_open(false);
        }
        self.set(st, "area", json!(area));
        crate::shell::ask("screen.do", json!(["record", area]));
    }

    pub fn mic(&mut self, st: &mut State, i: i32, _b: &Bridge) {
        let name = match i {
            0 => String::new(),
            1 => "default".into(),
            n => match self.mics.get(n as usize - 2) {
                Some(m) => m.0.clone(),
                None => return,
            },
        };
        self.set(st, "mic", json!(name));
        self.refresh(st);
    }

    pub fn flip(&mut self, st: &mut State, key: &str) {
        let c = self.cfg(st);
        let v = if key == "cursor" { !c.cursor } else { !c.system_audio };
        self.set(st, key, json!(v));
        self.refresh(st);
    }

    pub fn choose(&mut self, st: &mut State, key: &str, i: i32) {
        let i = i.max(0) as usize;
        let v = match key {
            "countdown" => json!(COUNTDOWNS.get(i).copied().unwrap_or(0)),
            "fps" => json!(if i == 0 { 30 } else { 60 }),
            "codec" => json!(CODECS.get(i).copied().unwrap_or("h264")),
            _ => json!(QUALITIES.get(i).copied().unwrap_or("high")),
        };
        self.set(st, key, v);
        self.refresh(st);
    }

    /// Open it, choose another (the toolbar closes for the file chooser), or
    /// back to ~/Videos/Screen Recordings
    pub fn folder(&mut self, st: &mut State, what: &str) {
        if what == "reset" {
            self.set(st, "folder", json!(""));
            self.refresh(st);
        } else {
            crate::shell::ask("recorder.folder", json!([what]));
        }
    }

    pub fn output_removed(&mut self, st: &mut State, name: &str) {
        if self.win.as_ref().is_some_and(|w| w.output == name) {
            let was = self.open;
            self.close(was);
            let w = self.win.take().unwrap();
            st.drop_surface(&w.surface);
        }
    }

    /// The glass under the bar and its options (Glass), and where it takes the pointer
    pub fn after_frame(&mut self, st: &mut State) {
        let kbd = st.keyboard_surface();
        let open = self.open;
        let Some(w) = &mut self.win else { return };
        let f = kbd.as_ref().is_some_and(|k| Rc::ptr_eq(k, &w.surface));
        if let Some(a) = w.a11y.as_mut() {
            a.update(false, f);
        }
        if !w.surface.mapped.get() {
            return;
        }
        let ui = &w.ui;
        let (bx, by, bw, bh) = (ui.get_bar_x(), ui.get_bar_y(), ui.get_bar_w(), ui.get_bar_h());
        let (ox, oy, ow, oh) = (ui.get_opt_x(), ui.get_opt_y(), ui.get_opt_w(), ui.get_opt_h());
        let rect = |x: f32, y: f32, w: f32, h: f32| (x.floor() as i32, y.floor() as i32, w.ceil() as i32, h.ceil() as i32);
        let mut region = Vec::new();
        if open {
            if ui.get_menu_open() {
                region.push((0, 0, 100_000, 100_000));
            } else {
                region.push(rect(bx, by, bw, bh));
                if oh > 0.0 {
                    region.push(rect(ox, oy, ow, oh));
                }
            }
        }
        if region != w.region_sent {
            st.set_input_region(&w.surface, Some(&region));
            w.region_sent = region;
        }
        if !st.shell.plugin || st.shell.look.material {
            return;
        }
        let p = ui.get_progress();
        let mut shapes = Vec::new();
        if p > 0.002 {
            shapes.push(theme::glass_shape(&st.shell.look, bx, by, bw, bh, 22.0, p, 1.0));
            if oh > 0.0 {
                shapes.push(theme::glass_shape(&st.shell.look, ox, oy, ow, oh, 18.0, p, 0.35));
            }
        }
        let output = w.output.clone();
        super::send_glass(st, &output, NS, &shapes, &mut w.glass_sent);
    }

    pub fn a11y(&mut self) -> Vec<&mut crate::a11y::A11y> {
        self.win.as_mut().and_then(|w| w.a11y.as_mut()).into_iter().collect()
    }

    pub fn quit(&mut self) {
        if let Some(w) = &self.win {
            crate::services::hypr::Glass::clear_now(&w.output, NS);
        }
    }
}

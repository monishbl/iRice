//! The screenshot's floating thumbnail and its markup editor (editor/:
//! ShotThumbnail.qml, ShotEditorWindow.qml, ScreenshotEditor.qml's session).
//! Everything the pointer and keys do happens in `Core`, shared with the
//! windows' callbacks, so a stroke follows the hand in the next frame; the
//! part makes, maps and hides the two windows to match it (`reconcile`).
//! Saving flattens the marks onto the screenshot's own pixels on a thread
//! (draw.rs), then the editor slides away and the result floats in.

use super::draw::{self, LabelFont};
use super::geometry::{integer_clip, Rect};
use super::livetext::{self, Live, Ocr};
use super::session::{base_name, copy_name, Ann, Opened, Session, Thumb, PALETTE, PALETTE_NAMES, TOOLS};
use super::{ic, ltui, Bridge, M};
use crate::services::hypr::Glass;
use crate::shell::Command;
use crate::theme;
use crate::ui;
use crate::wayland::{Spec, State, Surface};
use crate::Msg;
use image::RgbaImage;
use serde_json::{json, Value};
use slint::platform::Key;
use slint::{ComponentHandle, ModelRc, SharedPixelBuffer, SharedString, VecModel};
use smithay_client_toolkit::reexports::calloop::channel::Sender;
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use smithay_client_toolkit::shell::WaylandSurface;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

const NS_THUMB: &str = "tessera:shotThumbnail";
const NS_EDITOR: &str = "tessera:shotEditor";
const M_: f32 = 24.0; // shadow room round the cards
const THUMB_WIN: (u32, u32) = (340, 260);

pub(crate) enum Sm {
    ThumbDecoded(String, u32, Option<(u32, u32, Vec<u8>)>),
    BaseDecoded(String, u32, Option<RgbaImage>),
    ThumbGone,
    ThumbDismiss,
    ThumbUndo,
    ThumbCopyText,
    /// the press became a drag of the file: start the system drag
    ThumbDragOut,
    ThumbHover(bool),
    EdClosed,
    Flattened(&'static str, Option<String>),
    Copied(bool),
    SaidGone,
    LiveSaidGone,
    /// (the part follows: windows, sizes)
    Named,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum After {
    Close,
    Saved,
    Delete,
}

/// The editor's size for a picture (ShotEditorWindow.qml's geometry)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Geo {
    pub canvas_w: f32,
    pub canvas_h: f32,
    pub card_w: f32,
    pub card_h: f32,
    pub fit: f32,
    pub unit: f32,
}

pub fn geo(img: Option<(u32, u32)>, screen: (f32, f32), shown: Rect, material: bool) -> Geo {
    let (pad, bar) = (12.0, if material { 48.0 } else { 36.0 });
    let max_w = screen.0 * 0.82 - 2.0 * pad;
    let max_h = screen.1 * 0.82 - 2.0 * bar - 4.0 * pad;
    let (canvas_w, canvas_h, unit) = match img {
        Some((w, h)) if w > 0 && h > 0 => {
            let f = (max_w / w as f32).min(max_h / h as f32).min(1.0);
            ((w as f32 * f).round().max(720.0), (h as f32 * f).round().max(240.0), (w.min(h) as f32 / 360.0).max(1.0))
        }
        _ => (720.0, 440.0, 1.0),
    };
    let fit = if shown.w > 0.0 { (canvas_w / shown.w).min(canvas_h / shown.h).min(2.0) } else { 1.0 };
    Geo { canvas_w, canvas_h, card_w: canvas_w + 2.0 * pad, card_h: canvas_h + 2.0 * bar + 4.0 * pad, fit, unit }
}

/// The thumbnail's card: the picture fitted in 200 × 200, never thinner than 72
pub fn thumb_size(aspect: f32) -> (f32, f32) {
    let (w, h) = if aspect >= 1.0 { (200.0, (200.0 / aspect).max(72.0)) } else { ((200.0 * aspect).max(72.0), 200.0) };
    (w.round() + 12.0, h.round() + 12.0)
}

fn to_msg(tx: &Sender<Msg>, m: Sm) {
    let _ = tx.send(Msg::Part(super::NAME, Box::new(M::Shots(m))));
}

/// Put in the Trash where possible (gio), else removed, as the QML's
fn trash(file: String) {
    std::thread::spawn(move || {
        let _ = std::process::Command::new("sh")
            .args(["-c", "if command -v gio >/dev/null 2>&1; then gio trash -- \"$1\" || rm -f -- \"$1\"; else rm -f -- \"$1\"; fi", "sh", &file])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    });
}

fn runtime_dir() -> String {
    std::env::var("XDG_RUNTIME_DIR").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "/tmp".into())
}

/// The editor's and the thumbnail's state, shared with their callbacks
pub struct Core {
    pub s: Session,
    tx: Option<Sender<Msg>>,
    material: bool,
    screen_size: (f32, f32),
    ocr: Ocr,
    // ── the thumbnail ──
    th: Option<slint::Weak<ui::ShotThumbnail>>,
    pub t_visible: bool,
    t_status: i32,
    t_aspect: f32,
    t_loaded: (String, u32),
    t_mode: u8, // 0 idle, 1 swiping, 2 a drag that isn't one
    t_start: (f32, f32),
    t_offset: f32,
    t_leaving: bool,
    t_hovered: bool,
    t_said: String,
    t_reading: bool,
    t_key: String,
    life: slint::Timer,
    said_timer: slint::Timer,
    // ── the editor ──
    ed: Option<slint::Weak<ui::ShotEditor>>,
    pub e_visible: bool,
    e_leaving: bool,
    after: Option<After>,
    after_file: String,
    base: Option<Arc<RgbaImage>>,
    base_img: slint::Image,
    base_key: (String, u32),
    status: i32,
    img: (u32, u32),
    pub geo: Geo,
    draft: Option<Ann>,
    edit_text: Option<(f32, f32, String, f32)>,
    pending_crop: Rect,
    last_tool: &'static str,
    busy: String,
    banner: (i32, String, String),
    retry: String,
    toast: String,
    toast_timer: slint::Timer,
    grab: &'static str,
    start: (f32, f32),
    start_crop: Rect,
    pressed: bool,
    pub live: Live,
    glow_timer: slint::Timer,
    live_said: slint::Timer,
}

impl Default for Core {
    fn default() -> Self {
        Core {
            s: Session::default(),
            tx: None,
            material: false,
            screen_size: (1920.0, 1080.0),
            ocr: Ocr::default(),
            th: None,
            t_visible: false,
            t_status: 0,
            t_aspect: 16.0 / 10.0,
            t_loaded: (String::new(), 0),
            t_mode: 0,
            t_start: (0.0, 0.0),
            t_offset: 0.0,
            t_leaving: false,
            t_hovered: false,
            t_said: String::new(),
            t_reading: false,
            t_key: String::new(),
            life: slint::Timer::default(),
            said_timer: slint::Timer::default(),
            ed: None,
            e_visible: false,
            e_leaving: false,
            after: None,
            after_file: String::new(),
            base: None,
            base_img: slint::Image::default(),
            base_key: (String::new(), 0),
            status: 0,
            img: (0, 0),
            geo: geo(None, (1920.0, 1080.0), Rect::default(), false),
            draft: None,
            edit_text: None,
            pending_crop: Rect::default(),
            last_tool: "pen",
            busy: String::new(),
            banner: (0, String::new(), String::new()),
            retry: String::new(),
            toast: String::new(),
            toast_timer: slint::Timer::default(),
            grab: "",
            start: (0.0, 0.0),
            start_crop: Rect::default(),
            pressed: false,
            live: Live::new(),
            glow_timer: slint::Timer::default(),
            live_said: slint::Timer::default(),
        }
    }
}

type Shared = Rc<RefCell<Core>>;

impl Core {
    fn tui(&self) -> Option<ui::ShotThumbnail> {
        self.th.as_ref().and_then(|w| w.upgrade())
    }
    fn eui(&self) -> Option<ui::ShotEditor> {
        self.ed.as_ref().and_then(|w| w.upgrade())
    }
    fn ready(&self) -> bool {
        self.status == 1 && self.base.is_some()
    }
    fn full(&self) -> Rect {
        Rect::new(0.0, 0.0, self.img.0 as f32, self.img.1 as f32)
    }
    /// What the canvas shows: everything while cropping, else the crop
    fn shown(&self) -> Rect {
        if self.s.tool == "crop" || self.s.crop.w <= 0.0 {
            self.full()
        } else {
            self.s.crop
        }
    }
    fn ink(&self) -> String {
        PALETTE.get(self.s.color).unwrap_or(&PALETTE[0]).to_string()
    }
    fn pen_width(&self) -> f32 {
        self.geo.unit * [1.0, 2.0, 3.5][self.s.width.min(2)]
    }
    fn text_size(&self) -> f32 {
        self.geo.unit * [6.0, 9.0, 13.0][self.s.width.min(2)]
    }
    fn block_size(&self) -> f32 {
        (self.geo.unit * [3.0, 5.0, 8.0][self.s.width.min(2)]).round().max(2.0)
    }
    fn view(&self) -> (f32, f32) {
        let s = self.shown();
        (s.w * self.geo.fit, s.h * self.geo.fit)
    }
    fn recompute(&mut self) {
        let img = (self.img.0 > 0).then_some(self.img);
        let full = geo(img, self.screen_size, Rect::default(), self.material);
        self.geo = geo(img, self.screen_size, self.shown(), self.material);
        // (the window keeps the whole picture's size while cropping)
        self.geo.card_w = full.card_w;
        self.geo.card_h = full.card_h;
    }

    // ── The thumbnail ────────────────────────────────────────────────
    /// A newer screenshot (or a fresh save): slide in again
    fn thumb_enter(&mut self) {
        self.t_visible = true;
        self.t_leaving = false;
        self.t_offset = 0.0;
        self.t_mode = 0;
        let file = self.s.thumb_file.clone();
        let serial = self.s.serial;
        if self.s.thumb == Thumb::Shot && self.t_loaded != (file.clone(), serial) {
            self.t_loaded = (file.clone(), serial);
            self.t_status = 0;
            if let Some(tx) = self.tx.clone() {
                std::thread::spawn(move || {
                    let px = image::open(&file).ok().map(|i| {
                        let t = i.thumbnail(400, 400).to_rgba8();
                        let (w, h) = t.dimensions();
                        (w, h, t.into_raw())
                    });
                    to_msg(&tx, Sm::ThumbDecoded(file, serial, px));
                });
            }
        }
        self.push_thumb();
        if let Some(ui) = self.tui() {
            ui.set_swiping(false);
            ui.set_offset(0.0);
            ui.set_fade(false);
            ui.set_shown(true);
        }
        self.life_check();
    }

    fn thumb_leave(&mut self, fade: bool) {
        if self.t_leaving {
            return;
        }
        self.t_leaving = true;
        self.life.stop();
        if let Some(ui) = self.tui() {
            ui.set_fade(fade);
            ui.set_shown(false);
        }
    }

    /// 5 s for a screenshot, 8 s for the Undo toast; hovering holds it
    fn life_check(&mut self) {
        let run = self.t_visible && !self.t_leaving && !self.t_hovered && self.t_mode == 0 && self.t_said.is_empty();
        if !run {
            self.life.stop();
            return;
        }
        if self.life.running() {
            return;
        }
        let ms = if self.s.thumb == Thumb::Deleted { 8000 } else { 5000 };
        let tx = self.tx.clone();
        self.life.start(slint::TimerMode::SingleShot, Duration::from_millis(ms), move || {
            if let Some(tx) = &tx {
                to_msg(tx, Sm::ThumbDismiss);
            }
        });
    }

    fn push_thumb(&self) {
        let Some(ui) = self.tui() else { return };
        let deleted = self.s.thumb == Thumb::Deleted;
        let (w, h) = if deleted {
            if self.material { (262.0, 48.0) } else { (244.0, 44.0) }
        } else {
            thumb_size(self.t_aspect)
        };
        ui.set_deleted(deleted);
        ui.set_card_w(w);
        ui.set_card_h(h);
        ui.set_status(self.t_status);
        ui.set_said(self.t_said.as_str().into());
        ui.set_reading(self.t_reading);
        ui.set_hold(self.t_mode != 0);
    }

    fn thumb_press(&mut self, x: f32, y: f32) {
        self.t_start = (x, y);
        self.t_mode = 0;
        if let Some(ui) = self.tui() {
            ui.set_hold(false);
        }
    }
    /// True when this move turned the press into a drag of the file (once)
    fn thumb_drag(&mut self, x: f32, y: f32) -> bool {
        let (dx, dy) = (x - self.t_start.0, y - self.t_start.1);
        let mut out = false;
        if self.t_mode == 0 && dx.hypot(dy) > 6.0 {
            // (sideways: a swipe; otherwise, once the picture is loaded, a
            // drag of the file out into an app: the part starts it, ThumbDragOut)
            if dx > dy.abs() {
                self.t_mode = 1;
            } else if self.t_status == 1 {
                self.t_mode = 2;
                out = true;
            } else {
                return false;
            }
            self.life_check();
            if let Some(ui) = self.tui() {
                ui.set_hold(true);
                ui.set_swiping(self.t_mode == 1);
            }
        }
        if self.t_mode == 1 {
            self.t_offset = if dx > 0.0 { dx } else { dx / 4.0 }; // (gives a little the wrong way)
            if let Some(ui) = self.tui() {
                ui.set_offset(self.t_offset);
            }
        }
        out
    }
    fn thumb_release(&mut self) {
        let mode = self.t_mode;
        self.t_mode = 0;
        if let Some(ui) = self.tui() {
            ui.set_hold(false);
            ui.set_swiping(false);
        }
        match mode {
            1 => {
                if self.t_offset > 56.0 {
                    self.thumb_leave(false);
                } else {
                    self.t_offset = 0.0;
                    if let Some(ui) = self.tui() {
                        ui.set_offset(0.0);
                    }
                }
            }
            0 if self.s.thumb == Thumb::Shot => {
                let f = self.s.thumb_file.clone();
                self.open_editor(&f);
            }
            _ => {}
        }
        self.life_check();
    }

    /// Copy text: all the text in it (Live Text, read on this computer)
    fn thumb_copy_text(&mut self) {
        self.t_reading = true;
        self.t_said = "Reading text…".into();
        self.said_timer.stop();
        let rev = self.s.serial.to_string();
        self.t_key = livetext::key_of(&self.s.thumb_file, &rev);
        crate::shell::ask("livetext.read", json!([self.s.thumb_file, rev]));
        self.push_thumb();
        self.life_check();
    }

    fn thumb_said(&mut self, said: String) {
        self.t_said = said;
        self.push_thumb();
        let tx = self.tx.clone();
        self.said_timer.start(slint::TimerMode::SingleShot, Duration::from_millis(2500), move || {
            if let Some(tx) = &tx {
                to_msg(tx, Sm::SaidGone);
            }
        });
    }

    // ── The editor: opening and leaving ───────────────────────────────
    pub fn open_editor(&mut self, file: &str) {
        let (o, gone) = self.s.open_editor(file);
        if let Some(g) = gone {
            trash(g);
        }
        self.t_visible = false;
        self.life.stop();
        match o {
            Opened::Now => self.editor_now(),
            Opened::AskFirst => {
                self.banner = (2, "Save changes before opening the new screenshot?".into(), "Your markup on this one will be lost if you don't save it.".into());
                self.push_editor();
            }
            Opened::No => {}
        }
    }

    fn editor_now(&mut self) {
        if self.s.tool == "crop" {
            self.s.tool = self.last_tool; // (left over from a session that ended mid-crop)
        }
        self.e_visible = true;
        self.e_leaving = false;
        self.after = None;
        self.draft = None;
        self.edit_text = None;
        self.busy.clear();
        self.banner = (0, String::new(), String::new());
        self.toast.clear();
        let key = (self.s.path.clone(), self.s.serial);
        if self.base_key != key {
            self.base_key = key.clone();
            self.base = None;
            self.status = 0;
            self.img = (0, 0);
            self.base_img = slint::Image::default();
            if let Some(tx) = self.tx.clone() {
                std::thread::spawn(move || {
                    let img = image::open(&key.0).ok().map(|i| i.to_rgba8());
                    to_msg(&tx, Sm::BaseDecoded(key.0, key.1, img));
                });
            }
        }
        self.live.active = self.s.tool == "livetext";
        self.live_start();
        self.recompute();
        self.push_anns();
        self.push_editor();
        if let Some(ui) = self.eui() {
            ui.set_menu_open(false);
            ui.set_shown(true);
            ui.invoke_focus_keys();
        }
    }

    fn leave(&mut self, after: After) {
        if self.e_leaving {
            return;
        }
        self.e_leaving = true;
        self.after = Some(after);
        if let Some(ui) = self.eui() {
            ui.set_menu_open(false);
            ui.set_shown(false);
        }
    }

    /// The editor finished sliding away: what it left for
    fn editor_closed(&mut self, thumbnails: bool) {
        if !self.e_leaving {
            return;
        }
        self.e_leaving = false;
        self.e_visible = false;
        match self.after.take() {
            Some(After::Close) => {
                if self.s.close_editor() {
                    self.editor_now();
                }
            }
            Some(After::Saved) => {
                let f = std::mem::take(&mut self.after_file);
                if self.s.saved(&f, thumbnails) {
                    self.editor_now();
                } else if self.s.thumb == Thumb::Shot {
                    self.thumb_enter();
                }
            }
            Some(After::Delete) => {
                if let Some(g) = self.s.delete_shot() {
                    trash(g);
                }
                self.thumb_enter();
            }
            None => {}
        }
    }

    fn request_close(&mut self) {
        self.commit_text();
        if self.s.tool == "crop" {
            self.cancel_crop();
        }
        if self.s.dirty() && self.ready() {
            self.banner = (2, "Save changes to this screenshot?".into(), "Your markup will be lost if you don't save it.".into());
            self.push_editor();
            return;
        }
        self.leave(After::Close);
    }

    fn keep_editing(&mut self) {
        self.banner = (0, String::new(), String::new());
        self.s.pending_open.clear();
        self.push_editor();
    }

    fn fail(&mut self, action: &str, title: String, body: &str) {
        self.busy.clear();
        self.retry = action.to_string();
        self.banner = (1, title, body.to_string());
        self.push_editor();
    }

    fn show_toast(&mut self, t: &str) {
        self.toast = t.to_string();
        self.push_editor();
        let weak = self.ed.clone();
        self.toast_timer.start(slint::TimerMode::SingleShot, Duration::from_millis(2000), move || {
            if let Some(ui) = weak.as_ref().and_then(|w| w.upgrade()) {
                ui.set_toast("".into());
            }
        });
    }

    // ── Tools ────────────────────────────────────────────────────────
    fn set_tool(&mut self, t: &'static str) {
        if !self.ready() || t == self.s.tool {
            return;
        }
        self.commit_text();
        if self.s.tool == "crop" {
            self.apply_crop(false);
        }
        if t == "crop" {
            self.last_tool = self.s.tool;
            self.pending_crop = if self.s.crop.w > 0.0 { self.s.crop } else { self.full() };
        }
        self.s.tool = t;
        let live = t == "livetext";
        if live != self.live.active {
            self.live.active = live;
            self.live_start();
        }
        self.recompute();
        self.push_anns();
        self.push_editor();
    }

    fn apply_crop(&mut self, leave: bool) {
        let r = self.pending_crop;
        let (w, h) = (self.img.0 as f32, self.img.1 as f32);
        let whole = r.x <= 0.0 && r.y <= 0.0 && r.w >= w && r.h >= h;
        let next = if whole || r.w < 2.0 || r.h < 2.0 { Rect::default() } else { Rect::new(r.x.round(), r.y.round(), r.w.round(), r.h.round()) };
        if next != self.s.crop {
            self.s.set_crop(next);
        }
        if leave {
            self.s.tool = self.last_tool;
        }
        self.recompute();
        self.push_anns();
        self.push_editor();
    }

    fn cancel_crop(&mut self) {
        self.pending_crop = if self.s.crop.w > 0.0 { self.s.crop } else { self.full() };
        self.s.tool = self.last_tool;
        self.recompute();
        self.push_anns();
        self.push_editor();
    }

    fn commit_text(&mut self) {
        let Some((x, y, color, size)) = self.edit_text.take() else { return };
        let t = self.eui().map(|u| u.invoke_text_value().to_string()).unwrap_or_default();
        if !t.trim().is_empty() {
            self.s.add(Ann::Text { x, y, text: t, color, size });
            self.push_anns();
        }
        if let Some(ui) = self.eui() {
            ui.invoke_set_text("".into());
            ui.invoke_focus_keys();
        }
        self.push_editor();
    }

    fn cancel_text(&mut self) {
        self.edit_text = None;
        if let Some(ui) = self.eui() {
            ui.invoke_set_text("".into());
            ui.invoke_focus_keys();
        }
        self.push_editor();
    }

    /// Before anything leaves the editor: typed text and a pending crop count
    fn prepare(&mut self) {
        self.commit_text();
        if self.s.tool == "crop" {
            self.apply_crop(true);
        }
        self.draft = None;
        self.banner = (0, String::new(), String::new());
    }

    fn undo(&mut self) {
        self.commit_text();
        if self.s.undo() {
            self.after_history();
        }
    }
    fn redo(&mut self) {
        if self.s.redo() {
            self.after_history();
        }
    }
    fn after_history(&mut self) {
        if self.s.tool == "crop" {
            self.pending_crop = if self.s.crop.w > 0.0 { self.s.crop } else { self.full() };
        }
        self.recompute();
        self.push_anns();
        self.push_editor();
    }

    // ── Drawing, in the image's own pixels ───────────────────────────
    fn native(&self, x: f32, y: f32) -> (f32, f32) {
        let s = self.shown();
        let f = self.geo.fit.max(0.0001);
        ((x / f + s.x).clamp(0.0, self.img.0 as f32), (y / f + s.y).clamp(0.0, self.img.1 as f32))
    }

    fn crop_hit(&self, p: (f32, f32)) -> &'static str {
        let r = self.pending_crop;
        let t = 12.0 / self.geo.fit.max(0.0001);
        let near = |a: f32, b: f32| (a - b).abs() <= t;
        let (l, rt, tp, bt) = (near(p.0, r.x), near(p.0, r.x + r.w), near(p.1, r.y), near(p.1, r.y + r.h));
        match (l, rt, tp, bt) {
            (true, _, true, _) => "tl",
            (_, true, true, _) => "tr",
            (true, _, _, true) => "bl",
            (_, true, _, true) => "br",
            _ if p.0 > r.x && p.0 < r.x + r.w && p.1 > r.y && p.1 < r.y + r.h => "move",
            _ => "new",
        }
    }

    fn down(&mut self, x: f32, y: f32, _shift: bool) {
        let p = self.native(x, y);
        self.start = p;
        self.pressed = true;
        if self.banner.0 != 1 {
            self.banner = (0, String::new(), String::new());
        }
        let t = self.s.tool;
        if t == "crop" {
            self.grab = self.crop_hit(p);
            self.start_crop = self.pending_crop;
            if self.grab == "new" {
                self.pending_crop = Rect::new(p.0, p.1, 0.0, 0.0);
            }
            self.push_editor();
            return;
        }
        if t == "text" {
            self.commit_text();
            let size = self.text_size();
            self.edit_text = Some((p.0, p.1 - size * 0.6, self.ink(), size));
            self.push_editor();
            if let Some(ui) = self.eui() {
                ui.invoke_set_text("".into());
                ui.invoke_focus_text();
            }
            return;
        }
        self.commit_text();
        let c = self.ink();
        self.draft = match t {
            "pen" => Some(Ann::Stroke { points: vec![p], color: c, width: self.pen_width(), highlighter: false }),
            "highlighter" => Some(Ann::Stroke { points: vec![p], color: c, width: self.pen_width() * 4.0, highlighter: true }),
            _ => None, // (shapes appear once the pointer moves)
        };
        self.push_draft();
        self.push_editor();
    }

    fn moved(&mut self, x: f32, y: f32, shift: bool, pressed: bool) {
        let t = self.s.tool;
        if t == "crop" && !pressed {
            let h = self.crop_hit(self.native(x, y));
            let c = match h {
                "tl" | "br" => 1,
                "tr" | "bl" => 2,
                "move" => 3,
                _ => 0,
            };
            if let Some(ui) = self.eui() {
                ui.set_crop_cursor(c);
            }
            return;
        }
        if !pressed || !self.pressed {
            return;
        }
        let p = self.native(x, y);
        let (w, h) = (self.img.0 as f32, self.img.1 as f32);
        if t == "crop" {
            let r = self.start_crop;
            let (mut x0, mut y0, mut x1, mut y1) = (r.x, r.y, r.x + r.w, r.y + r.h);
            match self.grab {
                "new" => self.pending_crop = super::geometry::box_between(self.start, p),
                "move" => {
                    let dx = (p.0 - self.start.0).max(-x0).min(w - x1);
                    let dy = (p.1 - self.start.1).max(-y0).min(h - y1);
                    self.pending_crop = Rect::new(x0 + dx, y0 + dy, r.w, r.h);
                }
                g => {
                    if g.starts_with('t') {
                        y0 = p.1;
                    } else {
                        y1 = p.1;
                    }
                    if g.ends_with('l') {
                        x0 = p.0;
                    } else {
                        x1 = p.0;
                    }
                    self.pending_crop = Rect::new(x0.min(x1), y0.min(y1), (x1 - x0).abs(), (y1 - y0).abs());
                }
            }
            self.push_editor();
            return;
        }
        let c = self.ink();
        let fit = self.geo.fit.max(0.0001);
        match t {
            "pen" | "highlighter" => {
                let Some(Ann::Stroke { points, .. }) = &mut self.draft else { return };
                let last = *points.last().unwrap();
                if (p.0 - last.0).hypot(p.1 - last.1) < 1.5 / fit {
                    return;
                }
                points.push(p);
            }
            "arrow" => {
                let e = constrain(self.start, p, shift, true);
                self.draft = Some(Ann::Arrow { x1: self.start.0, y1: self.start.1, x2: e.0, y2: e.1, color: c, width: self.pen_width() });
            }
            "rect" | "ellipse" => {
                let r = super::geometry::box_between(self.start, constrain(self.start, p, shift, false));
                let width = self.pen_width();
                self.draft = Some(if t == "rect" { Ann::Rect { r, color: c, width } } else { Ann::Ellipse { r, color: c, width } });
            }
            "pixelate" => {
                let r = super::geometry::box_between(self.start, p);
                self.draft = Some(Ann::Pixelate { r, block: self.block_size() });
            }
            _ => return,
        }
        self.push_draft();
    }

    fn up(&mut self) {
        if !self.pressed {
            return;
        }
        self.pressed = false;
        let fit = self.geo.fit.max(0.0001);
        if self.s.tool == "crop" {
            if self.grab == "new" && (self.pending_crop.w < 4.0 / fit || self.pending_crop.h < 4.0 / fit) {
                self.pending_crop = self.start_crop; // (a click, not a drag)
            }
            self.grab = "";
            self.push_editor();
            return;
        }
        let Some(d) = self.draft.take() else {
            self.push_editor();
            return;
        };
        let tiny = 4.0 / fit;
        let keep = match &d {
            Ann::Stroke { .. } => true,
            Ann::Arrow { x1, y1, x2, y2, .. } => (x2 - x1).hypot(y2 - y1) >= 2.0 * tiny,
            Ann::Rect { r, .. } | Ann::Ellipse { r, .. } | Ann::Pixelate { r, .. } => r.w >= tiny && r.h >= tiny,
            Ann::Text { .. } => true,
        };
        if keep {
            let d = match d {
                // (a click leaves a dot)
                Ann::Stroke { mut points, color, width, highlighter } => {
                    if points.len() == 1 {
                        points.push((points[0].0 + 0.01, points[0].1));
                    }
                    Ann::Stroke { points, color, width, highlighter }
                }
                o => o,
            };
            self.s.add(d);
            self.push_anns();
        }
        self.push_draft();
        self.push_editor();
    }

    // ── Output ───────────────────────────────────────────────────────
    fn flatten_to(&self, action: &'static str, target: Option<String>) {
        let (Some(base), Some(tx)) = (self.base.clone(), self.tx.clone()) else { return };
        let (items, crop, material, path) = (self.s.items.clone(), self.s.crop, self.material, self.s.path.clone());
        std::thread::spawn(move || {
            let target = match target {
                Some(t) => t,
                None => copy_name(&path, |p| std::path::Path::new(p).exists()),
            };
            if let Some(dir) = std::path::Path::new(&target).parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let font: Option<LabelFont> = if items.iter().any(|a| matches!(a, Ann::Text { .. })) { draw::label_font(material) } else { None };
            let ok = draw::flatten(&base, &items, crop, font.as_ref()).is_some_and(|img| draw::write_png(&img, &target));
            to_msg(&tx, Sm::Flattened(action, ok.then_some(target)));
        });
    }

    fn save(&mut self) {
        if !self.busy.is_empty() || !self.ready() {
            return;
        }
        self.prepare();
        if !self.s.dirty() {
            self.after_file = self.s.path.clone();
            self.leave(After::Saved);
            return;
        }
        self.busy = "save".into();
        self.push_editor();
        self.flatten_to("save", Some(self.s.path.clone()));
    }

    fn save_copy(&mut self) {
        if !self.busy.is_empty() || !self.ready() {
            return;
        }
        self.prepare();
        self.busy = "saveCopy".into();
        self.push_editor();
        self.flatten_to("saveCopy", None);
    }

    fn copy(&mut self) {
        if !self.busy.is_empty() || !self.ready() {
            return;
        }
        self.prepare();
        self.busy = "copy".into();
        self.push_editor();
        if !self.s.dirty() {
            send_png(self.tx.clone(), self.s.path.clone());
        } else {
            self.flatten_to("copy", Some(format!("{}/tessera-markup.png", runtime_dir())));
        }
    }

    fn share(&mut self, screen: &str) {
        if !self.busy.is_empty() || !self.ready() {
            return;
        }
        self.prepare();
        if !self.s.dirty() {
            crate::shell::ask("panels.share", json!([[self.s.path], screen]));
            self.push_editor();
            return;
        }
        // (the edited image, under the screenshot's name, for the recipient)
        self.busy = "share".into();
        self.push_editor();
        self.flatten_to("share", Some(format!("{}/tessera-share/{}", runtime_dir(), base_name(&self.s.path))));
    }

    /// Immediate; the thumbnail turns into an Undo toast
    fn remove(&mut self) {
        if !self.busy.is_empty() {
            return;
        }
        self.commit_text();
        self.draft = None;
        self.leave(After::Delete);
    }

    fn flattened(&mut self, action: &str, file: Option<String>, screen: &str) {
        let name = base_name(&self.s.path).to_string();
        match (action, file) {
            ("save", Some(f)) | ("saveCopy", Some(f)) => {
                self.busy.clear();
                self.after_file = f;
                self.leave(After::Saved);
            }
            ("save", None) => self.fail("save", format!("Couldn't save “{name}”"), "Check that its folder still exists and the disk isn't full, then try again. Or copy it to the clipboard so the markup isn't lost."),
            ("saveCopy", None) => self.fail("saveCopy", "Couldn't save a copy".into(), "Check that the folder isn't read-only and the disk isn't full, then try again."),
            ("copy", Some(f)) => send_png(self.tx.clone(), f),
            ("copy", None) => self.fail("copy", "Couldn't copy the screenshot".into(), "There wasn't room to prepare the image. Free some memory, then try again."),
            ("share", Some(f)) => {
                self.busy.clear();
                self.push_editor();
                crate::shell::ask("panels.share", json!([[f], screen]));
            }
            _ => self.fail("share", "Couldn't prepare the image to share".into(), "Try again, or save it first and share the file."),
        }
    }

    fn copied(&mut self, ok: bool) {
        if ok {
            self.busy.clear();
            self.show_toast("Copied to the clipboard");
        } else {
            self.fail("copy", "Couldn't copy the screenshot".into(), "Make sure wl-clipboard is installed, then try again.");
        }
    }

    fn run_again(&mut self, screen: &str) {
        let r = std::mem::take(&mut self.retry);
        self.banner = (0, String::new(), String::new());
        match r.as_str() {
            "save" => self.save(),
            "saveCopy" => self.save_copy(),
            "copy" => self.copy(),
            "share" => self.share(screen),
            _ => self.push_editor(),
        }
    }

    // ── Keys ─────────────────────────────────────────────────────────
    fn key(&mut self, text: &str, ctrl: bool, shift: bool, screen: &str) -> bool {
        let is = |k: Key| text == SharedString::from(k).as_str();
        let typing = self.edit_text.is_some();
        if is(Key::Escape) {
            if typing {
                self.cancel_text();
            } else if self.banner.0 == 2 {
                self.keep_editing();
            } else if self.banner.0 == 1 {
                self.banner = (0, String::new(), String::new());
                self.push_editor();
            } else if self.draft.is_some() {
                self.draft = None;
                self.pressed = false;
                self.push_draft();
            } else if self.live.has_selection() {
                self.live.clear();
                self.push_live();
            } else if self.s.tool == "crop" {
                self.cancel_crop();
            } else {
                self.request_close();
            }
            return true;
        }
        if (is(Key::Return) || text == "\u{a}") && self.s.tool == "crop" && !typing {
            self.apply_crop(true);
            return true;
        }
        if typing {
            return false;
        }
        let k = text.to_lowercase();
        if ctrl {
            match k.as_str() {
                "z" if !shift && self.busy.is_empty() => self.undo(),
                "z" | "y" if self.busy.is_empty() => self.redo(),
                "c" => {
                    if self.live.has_selection() {
                        let t = self.live.selected_text();
                        self.live_copy(t, "Text copied");
                    } else {
                        self.copy();
                    }
                }
                "a" if self.s.tool == "livetext" => {
                    self.live.select_all();
                    self.push_live();
                }
                "s" if shift => self.save_copy(),
                "s" => self.save(),
                _ => return false,
            }
            return true;
        }
        if self.busy.is_empty() && k.chars().count() == 1 {
            if let Some(t) = TOOLS.iter().find(|t| t.3.eq_ignore_ascii_case(&k)) {
                self.set_tool(t.0);
                return true;
            }
        }
        let _ = screen;
        false
    }

    // ── Live Text in the editor ──────────────────────────────────────
    fn live_start(&mut self) {
        self.live.path = self.s.path.clone();
        self.live.rev = self.s.serial.to_string();
        if let Some(args) = self.live.start() {
            crate::shell::ask("livetext.read", args);
        }
        self.push_live();
    }

    fn live_copy(&mut self, text: String, what: &str) {
        if text.is_empty() {
            return;
        }
        crate::util::pipe_to(&["wl-copy", "--type", "text/plain"], text.into_bytes());
        self.live_say(what.to_string());
    }

    fn live_say(&mut self, what: String) {
        if what.is_empty() {
            return;
        }
        self.live.said = what;
        self.push_live();
        let tx = self.tx.clone();
        self.live_said.start(slint::TimerMode::SingleShot, Duration::from_millis(2000), move || {
            if let Some(tx) = &tx {
                to_msg(tx, Sm::LiveSaidGone);
            }
        });
    }

    fn live_chip(&mut self, i: usize) {
        let Some(a) = self.live.actions().get(i).cloned() else { return };
        crate::shell::ask("livetext.act", json!([livetext::act_json(&a)]));
        self.live_say(livetext::act_says(&a, &self.ocr).to_string());
    }

    fn glow(&mut self) {
        let weak = self.ed.clone();
        let t0 = Instant::now();
        self.glow_timer.start(slint::TimerMode::Repeated, Duration::from_millis(16), move || {
            let ms = t0.elapsed().as_secs_f32() * 1000.0;
            if let Some(ui) = weak.as_ref().and_then(|w| w.upgrade()) {
                ui.set_glow(ltui::glow_at(ms));
            }
        });
    }

    fn sx(&self) -> f32 {
        self.geo.fit
    }

    fn push_live(&self) {
        let Some(ui) = self.eui() else { return };
        let s = self.shown();
        let (vw, vh) = self.view();
        ui.set_boxes(ltui::boxes(&self.live, (s.x, s.y, s.w, s.h), vw, vh));
        ui.set_lt(ltui::bar(&self.live, &self.ocr, self.material, self.geo.canvas_w - 380.0));
    }

    // ── Into the window ──────────────────────────────────────────────
    fn ann_ui(&self, a: &Ann) -> ui::StAnn {
        let s = self.shown();
        let f = self.geo.fit;
        let map = |x: f32, y: f32| ((x - s.x) * f, (y - s.y) * f);
        let col = |c: &str| {
            let (r, g, b) = draw::rgb(c);
            slint::Color::from_rgb_u8(r, g, b)
        };
        let mut o = ui::StAnn::default();
        match a {
            Ann::Stroke { points, color, width, highlighter } => {
                o.kind = if *highlighter { 1 } else { 0 };
                o.cmd = draw::polyline(points, map).into();
                o.color = col(color);
                o.width = width * f;
            }
            Ann::Arrow { x1, y1, x2, y2, color, width } => {
                let (end, head) = draw::arrow_parts(*x1, *y1, *x2, *y2, *width);
                o.kind = 2;
                o.cmd = draw::polyline(&[(*x1, *y1), end], map).into();
                let h: Vec<(f32, f32)> = head.iter().map(|p| map(p.0, p.1)).collect();
                o.head = format!("M {:.1} {:.1} L {:.1} {:.1} L {:.1} {:.1} Z", h[0].0, h[0].1, h[1].0, h[1].1, h[2].0, h[2].1).into();
                o.color = col(color);
                o.width = width * f;
                o.head_width = (width * 0.5).max(1.0) * f;
            }
            Ann::Rect { r, color, width } => {
                let (x, y) = map(r.x, r.y);
                o.kind = 3;
                o.x = x;
                o.y = y;
                o.w = r.w * f;
                o.h = r.h * f;
                o.color = col(color);
                o.width = width * f;
                o.radius = draw::rect_radius(*r, *width) * f;
            }
            Ann::Ellipse { r, color, width } => {
                let (rx, ry) = draw::ellipse_radii(*r, *width);
                let (cx, cy) = map(r.x + r.w / 2.0, r.y + r.h / 2.0);
                o.kind = 4;
                o.cmd = draw::ellipse_path(cx, cy, rx * f, ry * f).into();
                o.color = col(color);
                o.width = width * f;
            }
            Ann::Text { x, y, text, color, size } => {
                let (dx, dy) = map(*x, *y);
                o.kind = 5;
                o.x = dx;
                o.y = dy;
                o.text = text.as_str().into();
                o.color = col(color);
                o.size = size * f;
            }
            Ann::Pixelate { r, block } => {
                let (x, y) = map(r.x, r.y);
                o.kind = 6;
                o.x = x;
                o.y = y;
                o.w = r.w * f;
                o.h = r.h * f;
                if let Some(base) = &self.base {
                    let (cw, ch, px) = draw::pixelate(base, *r, *block);
                    o.img = slint::Image::from_rgba8(SharedPixelBuffer::clone_from_slice(&px, cw, ch));
                }
            }
        }
        o
    }

    fn push_anns(&self) {
        let Some(ui) = self.eui() else { return };
        let v: Vec<ui::StAnn> = self.s.items.iter().map(|a| self.ann_ui(a)).collect();
        ui.set_anns(ModelRc::new(VecModel::from(v)));
    }

    fn push_draft(&self) {
        let Some(ui) = self.eui() else { return };
        match &self.draft {
            Some(d) => {
                ui.set_draft(self.ann_ui(d));
                ui.set_has_draft(true);
            }
            None => ui.set_has_draft(false),
        }
    }

    fn push_editor(&self) {
        let Some(ui) = self.eui() else { return };
        let g = self.geo;
        let s = self.shown();
        let (vw, vh) = self.view();
        let t = self.s.tool;
        ui.set_canvas_w(g.canvas_w);
        ui.set_canvas_h(g.canvas_h);
        ui.set_status(if self.ready() { 1 } else { self.status.min(2).max(0) });
        ui.set_has_path(!self.s.path.is_empty());
        let c = integer_clip(s, self.img.0 as f32, self.img.1 as f32);
        ui.set_clip_x(c.x as i32);
        ui.set_clip_y(c.y as i32);
        ui.set_clip_w(c.w.max(1.0) as i32);
        ui.set_clip_h(c.h.max(1.0) as i32);
        ui.set_view_w(vw);
        ui.set_view_h(vh);
        ui.set_tool(TOOLS.iter().position(|x| x.0 == t).map_or(-1, |i| i as i32));
        ui.set_crop_mode(t == "crop");
        ui.set_live_mode(t == "livetext");
        ui.set_text_tool(t == "text");
        ui.set_pixel_tool(t == "pixelate");
        ui.set_busy_any(!self.busy.is_empty());
        ui.set_busy(self.busy.as_str().into());
        ui.set_can_undo(!self.s.undo.is_empty());
        ui.set_can_redo(!self.s.redo.is_empty());
        ui.set_dirty(self.s.dirty());
        ui.set_color_index(self.s.color as i32);
        ui.set_width_index(self.s.width as i32);
        let ink = draw::rgb(&self.ink());
        ui.set_ink(slint::Color::from_rgb_u8(ink.0, ink.1, ink.2));
        let tips: [&str; 3] = match t {
            "text" => ["Small text", "Medium text", "Large text"],
            "pixelate" => ["Fine blocks", "Medium blocks", "Coarse blocks"],
            _ => ["Thin", "Medium", "Thick"],
        };
        ui.set_width_tips(ModelRc::new(VecModel::from(tips.iter().map(|x| SharedString::from(*x)).collect::<Vec<_>>())));
        let f = g.fit;
        let p = self.pending_crop;
        ui.set_crop_x((p.x - s.x) * f);
        ui.set_crop_y((p.y - s.y) * f);
        ui.set_crop_w(p.w * f);
        ui.set_crop_h(p.h * f);
        ui.set_crop_size(format!("{} × {}", p.w.round(), p.h.round()).into());
        ui.set_crop_whole(!(p.w < self.img.0 as f32 || p.h < self.img.1 as f32));
        match &self.edit_text {
            Some((x, y, color, size)) => {
                ui.set_typing(true);
                ui.set_text_x((x - s.x) * f);
                ui.set_text_y((y - s.y) * f);
                ui.set_text_size(size * f);
                let (r, gg, b) = draw::rgb(color);
                ui.set_text_color(slint::Color::from_rgb_u8(r, gg, b));
            }
            None => ui.set_typing(false),
        }
        ui.set_banner(ui::StBanner { kind: self.banner.0, title: self.banner.1.as_str().into(), body: self.banner.2.as_str().into() });
        ui.set_retry(self.retry.as_str().into());
        ui.set_toast(self.toast.as_str().into());
    }

    fn setup_editor(&self, ui: &ui::ShotEditor) {
        let m = self.material;
        let tools: Vec<ui::StToolBtn> = TOOLS.iter().map(|t| ui::StToolBtn { icon: ic(t.1, m), label: t.2.into(), key: t.3.into() }).collect();
        ui.set_tool_list(ModelRc::new(VecModel::from(tools)));
        let pal: Vec<slint::Color> = PALETTE.iter().map(|c| {
            let (r, g, b) = draw::rgb(c);
            slint::Color::from_rgb_u8(r, g, b)
        }).collect();
        ui.set_palette(ModelRc::new(VecModel::from(pal)));
        ui.set_palette_names(ModelRc::new(VecModel::from(PALETTE_NAMES.iter().map(|x| SharedString::from(*x)).collect::<Vec<_>>())));
        ui.set_ic_close(ic("x", m));
        ui.set_ic_undo(ic("arrow-counter-clockwise", m));
        ui.set_ic_redo(ic("arrow-clockwise", m));
        ui.set_ic_check(ic("check", m));
        ui.set_ic_trash(ic("trash", m));
        ui.set_ic_share(ic("export", m));
        ui.set_ic_copy(ic("copy", m));
        ui.set_ic_files(ic("files", m));
        ui.set_ic_image(ic("image", m));
        ui.set_ic_warning(ic("warning-circle", m));
        ui.set_ic_floppy(ic("floppy-disk", m));
        ui.set_ic_download(ic("download-simple", m));
        ui.set_ic_caret(ic("caret-down", m));
        ui.set_empty_shape(crate::surfaces::shapes::outline("cookie9").into());
    }
}

/// Constrained by Shift: squares, circles, 45° arrows
fn constrain(a: (f32, f32), b: (f32, f32), shift: bool, angle: bool) -> (f32, f32) {
    if !shift {
        return b;
    }
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    if angle {
        let q = std::f32::consts::FRAC_PI_4;
        let ang = (dy.atan2(dx) / q).round() * q;
        let l = dx.hypot(dy);
        return (a.0 + ang.cos() * l, a.1 + ang.sin() * l);
    }
    let d = dx.abs().max(dy.abs());
    let sx = if dx == 0.0 { 1.0 } else { dx.signum() };
    let sy = if dy == 0.0 { 1.0 } else { dy.signum() };
    (a.0 + sx * d, a.1 + sy * d)
}

/// The picture onto the clipboard (wl-copy, as PNG)
fn send_png(tx: Option<Sender<Msg>>, file: String) {
    std::thread::spawn(move || {
        let ok = std::fs::File::open(&file)
            .ok()
            .and_then(|f| std::process::Command::new("wl-copy").args(["-t", "image/png"]).stdin(f).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status().ok())
            .is_some_and(|s| s.success());
        if let Some(tx) = tx {
            to_msg(&tx, Sm::Copied(ok));
        }
    });
}

// ── The part's side: windows ─────────────────────────────────────────────

struct TWin {
    output: String,
    surface: Rc<Surface>,
    ui: ui::ShotThumbnail,
    a11y: Option<crate::a11y::A11y>,
    glass_sent: String,
    region_sent: Vec<(i32, i32, i32, i32)>,
    mapped: bool,
}

struct EWin {
    output: String,
    surface: Rc<Surface>,
    ui: ui::ShotEditor,
    a11y: Option<crate::a11y::A11y>,
    glass_sent: String,
    size: (u32, u32),
    mapped: bool,
    keyboard: bool,
}

/// A file's path as a file:// URI (the bytes outside the unreserved set escaped)
pub(crate) fn file_uri(path: &str) -> String {
    let mut u = String::from("file://");
    for b in path.bytes() {
        if b.is_ascii_alphanumeric() || b"/-_.~".contains(&b) {
            u.push(b as char);
        } else {
            u.push_str(&format!("%{b:02X}"));
        }
    }
    u
}

#[derive(Default)]
pub struct Shots {
    core: Shared,
    thumb: Option<TWin>,
    editor: Option<EWin>,
    share_open: bool,
}

impl Shots {
    pub fn start(&mut self, st: &mut State) {
        let mut c = self.core.borrow_mut();
        c.tx = Some(st.tx.clone());
        c.material = st.shell.look.material;
    }

    fn thumbnails(st: &State) -> bool {
        st.shell.cfg["screenshot"]["thumbnail"].as_bool().unwrap_or(true)
    }

    fn screen_of(&self, st: &State, b: &Bridge) -> String {
        let s = self.core.borrow().s.screen.clone();
        if !s.is_empty() && st.output_named(&s).is_some() {
            return s;
        }
        super::focused_output(st, b).unwrap_or_default()
    }

    pub fn command(&mut self, st: &mut State, cmd: &Command, b: &Bridge) {
        let file = cmd.args["file"].as_str().unwrap_or("").to_string();
        let screen = cmd.args["screen"].as_str().map(str::to_string).filter(|s| !s.is_empty()).or_else(|| super::focused_output(st, b)).unwrap_or_default();
        {
            let mut c = self.core.borrow_mut();
            c.screen_size = super::output_size(st, &screen);
        }
        match cmd.cmd.as_str() {
            "shot" | "thumbnail" => {
                let mut c = self.core.borrow_mut();
                if let Some(g) = c.s.shot(&file, &screen) {
                    trash(g);
                }
                c.thumb_enter();
            }
            "edit" => {
                let mut c = self.core.borrow_mut();
                c.s.screen = screen;
                c.open_editor(&file);
            }
            "editorClose" => {
                let mut c = self.core.borrow_mut();
                if let Some(g) = c.s.thumb_gone() {
                    trash(g);
                }
                c.s.close_editor();
                c.t_visible = false;
                c.e_visible = false;
                c.e_leaving = false;
            }
            _ => {}
        }
        self.reconcile(st, b);
    }

    pub fn bridge(&mut self, st: &mut State, b: &Bridge, old: &Bridge) {
        {
            let mut c = self.core.borrow_mut();
            c.ocr = b.ocr.clone();
            if b.ocr != old.ocr {
                c.push_live();
            }
        }
        if b.share_open != self.share_open {
            self.share_open = b.share_open;
            self.reconcile(st, b);
        }
    }

    pub fn live_done(&mut self, key: &str, r: &Value) {
        let mut c = self.core.borrow_mut();
        if c.t_reading && key == c.t_key {
            c.t_reading = false;
            let words: Vec<livetext::Word> = r["words"].as_array().map(|a| a.iter().filter_map(livetext::Word::from_json).collect()).unwrap_or_default();
            let said = match r["status"].as_str().unwrap_or("error") {
                "ready" => {
                    crate::util::pipe_to(&["wl-copy", "--type", "text/plain"], livetext::all_text(&words).into_bytes());
                    "Text copied"
                }
                "none" => "No text found",
                "missing" => "Install Tesseract to copy text",
                _ => "Couldn't read the text",
            };
            c.thumb_said(said.into());
        }
        if c.live.done(key, r) {
            if c.live.found {
                c.live.found = false;
                c.glow();
            }
            c.push_live();
        }
    }

    pub fn wifi(&mut self, key: &str, found: &Value) {
        let mut c = self.core.borrow_mut();
        if key == c.live.key() && c.live.active {
            c.live.wifi = found.as_array().cloned().unwrap_or_default();
            c.push_live();
        }
    }

    /// The thumbnail's file dragged out into an app (core's drag and drop):
    /// the file as a URI list, from the press still held. Whether the drop
    /// was taken isn't known on Wayland, so the thumbnail leaves as the QML's
    /// does after a drop (fading in place); a drag that can't start leaves it.
    fn drag_out(&mut self, st: &mut State) {
        let Some(t) = &self.thumb else { return };
        let file = self.core.borrow().s.thumb_file.clone();
        if file.is_empty() {
            return;
        }
        let offers = vec![("text/uri-list".to_string(), format!("{}\r\n", file_uri(&file)).into_bytes())];
        let surface = t.surface.clone();
        if st.start_drag(&surface, offers) {
            self.core.borrow_mut().thumb_leave(true);
        }
    }

    pub fn message(&mut self, st: &mut State, m: Sm, b: &Bridge) {
        if matches!(m, Sm::ThumbDragOut) {
            self.drag_out(st);
            return;
        }
        let screen = self.screen_of(st, b);
        {
            let mut c = self.core.borrow_mut();
            match m {
                Sm::ThumbDecoded(file, serial, px) => {
                    if c.t_loaded == (file, serial) {
                        match px {
                            Some((w, h, data)) => {
                                c.t_status = 1;
                                c.t_aspect = w as f32 / h.max(1) as f32;
                                if let Some(ui) = c.tui() {
                                    ui.set_preview(slint::Image::from_rgba8(SharedPixelBuffer::clone_from_slice(&data, w, h)));
                                }
                            }
                            None => c.t_status = 2,
                        }
                        c.push_thumb();
                    }
                }
                Sm::BaseDecoded(path, serial, img) => {
                    if c.base_key == (path, serial) {
                        match img {
                            Some(img) => {
                                let (w, h) = img.dimensions();
                                c.img = (w, h);
                                c.base_img = slint::Image::from_rgba8(SharedPixelBuffer::clone_from_slice(img.as_raw(), w, h));
                                c.base = Some(Arc::new(img));
                                c.status = 1;
                                if c.s.tool == "crop" {
                                    c.pending_crop = if c.s.crop.w > 0.0 { c.s.crop } else { c.full() };
                                }
                            }
                            None => c.status = 2,
                        }
                        c.recompute();
                        if let Some(ui) = c.eui() {
                            ui.set_base(c.base_img.clone());
                        }
                        c.push_anns();
                        c.push_editor();
                        c.push_live();
                    }
                }
                Sm::ThumbGone => {
                    if c.t_leaving {
                        c.t_leaving = false;
                        c.t_visible = false;
                        if let Some(g) = c.s.thumb_gone() {
                            trash(g);
                        }
                    }
                }
                Sm::ThumbDismiss => c.thumb_leave(false),
                Sm::ThumbUndo => {
                    c.s.undo_delete();
                    c.t_visible = false;
                    c.life.stop();
                    c.editor_now();
                }
                Sm::ThumbCopyText => c.thumb_copy_text(),
                Sm::ThumbHover(h) => {
                    c.t_hovered = h;
                    c.life_check();
                }
                Sm::EdClosed => {
                    let thumbs = Self::thumbnails(st);
                    c.editor_closed(thumbs);
                }
                Sm::Flattened(action, file) => c.flattened(action, file, &screen),
                Sm::Copied(ok) => {
                    if c.busy == "copy" {
                        c.copied(ok);
                    }
                }
                Sm::SaidGone => {
                    c.t_said.clear();
                    c.push_thumb();
                    c.life_check();
                }
                Sm::LiveSaidGone => {
                    c.live.said.clear();
                    c.push_live();
                }
                Sm::Named | Sm::ThumbDragOut => {}
            }
        }
        self.reconcile(st, b);
    }

    /// The windows match the session: made, mapped, sized, hidden
    fn reconcile(&mut self, st: &mut State, b: &Bridge) {
        let (tv, ev, card) = {
            let c = self.core.borrow();
            (c.t_visible, c.e_visible, (c.geo.card_w, c.geo.card_h))
        };
        let screen = self.screen_of(st, b);
        // ── the thumbnail
        if tv {
            if self.thumb.as_ref().is_some_and(|w| w.output != screen) {
                let w = self.thumb.take().unwrap();
                Glass::clear_now(&w.output, NS_THUMB);
                st.drop_surface(&w.surface);
            }
            if self.thumb.is_none() {
                self.make_thumb(st, &screen);
            }
            if let Some(w) = &mut self.thumb {
                if !w.mapped {
                    w.mapped = true;
                    w.surface.show(THUMB_WIN.0, THUMB_WIN.1);
                    self.core.borrow().push_thumb();
                }
            }
        } else if let Some(w) = &mut self.thumb {
            if w.mapped {
                w.mapped = false;
                w.surface.hide();
                st.shell.glass.set(&w.output, NS_THUMB, &[]);
                w.glass_sent.clear();
                w.region_sent.clear();
            }
        }
        // ── the editor
        if ev {
            if self.editor.as_ref().is_some_and(|w| w.output != screen) {
                let w = self.editor.take().unwrap();
                Glass::clear_now(&w.output, NS_EDITOR);
                st.drop_surface(&w.surface);
            }
            if self.editor.is_none() {
                self.make_editor(st, &screen);
            }
            let share = self.share_open;
            if let Some(w) = &mut self.editor {
                let size = ((card.0 + 2.0 * M_).round() as u32, (card.1 + 2.0 * M_).round() as u32);
                if !w.mapped || w.size != size {
                    w.mapped = true;
                    w.size = size;
                    w.surface.show(size.0, size.1);
                }
                let kb = !share;
                if kb != w.keyboard {
                    w.keyboard = kb;
                    w.surface.layer.set_keyboard_interactivity(if kb { KeyboardInteractivity::Exclusive } else { KeyboardInteractivity::None });
                    w.surface.layer.commit();
                }
            }
        } else if let Some(w) = &mut self.editor {
            if w.mapped {
                w.mapped = false;
                w.keyboard = false;
                w.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::None);
                w.surface.hide();
                st.shell.glass.set(&w.output, NS_EDITOR, &[]);
                w.glass_sent.clear();
            }
        }
    }

    fn make_thumb(&mut self, st: &mut State, output: &str) {
        let Some(wl) = st.output_named(output) else { return };
        let surface = st.prepare_surface(Spec {
            layer: Layer::Overlay,
            namespace: NS_THUMB,
            output: Some((&wl, output)),
            anchor: Anchor::BOTTOM | Anchor::RIGHT,
            size: THUMB_WIN,
            exclusive: -1,
            keyboard: KeyboardInteractivity::None,
            margin_left: 0,
        });
        let ui = ui::ShotThumbnail::new().expect("ShotThumbnail");
        st.shell.apply_theme(&ui.global::<ui::Theme>());
        let core = self.core.clone();
        let tx = st.tx.clone();
        {
            let core = core.clone();
            ui.on_press(move |x, y| {
                if let Ok(mut c) = core.try_borrow_mut() {
                    c.thumb_press(x, y);
                }
            });
        }
        {
            let core = core.clone();
            let tx = tx.clone();
            ui.on_drag_to(move |x, y| {
                let out = core.try_borrow_mut().map_or(false, |mut c| c.thumb_drag(x, y));
                if out {
                    to_msg(&tx, Sm::ThumbDragOut);
                }
            });
        }
        {
            let core = core.clone();
            let tx = tx.clone();
            ui.on_release(move || {
                if let Ok(mut c) = core.try_borrow_mut() {
                    c.thumb_release();
                }
                // (the part makes the editor's window next)
                to_msg(&tx, Sm::Named);
            });
        }
        macro_rules! on {
            ($set:ident, |$($a:ident),*| $m:expr) => {{
                let tx = tx.clone();
                ui.$set(move |$($a),*| to_msg(&tx, $m));
            }};
        }
        on!(on_hovered, |h| Sm::ThumbHover(h));
        on!(on_copy_text, | | Sm::ThumbCopyText);
        on!(on_dismiss, | | Sm::ThumbDismiss);
        on!(on_undo, | | Sm::ThumbUndo);
        on!(on_gone, | | Sm::ThumbGone);
        let m = st.shell.look.material;
        ui.set_ic_image(ic("image", m));
        ui.set_ic_warning(ic("warning-circle", m));
        ui.set_ic_text(ic("text-aa", m));
        ui.set_ic_close(ic("x", m));
        ui.set_ic_trash(ic("trash", m));
        ui.show().expect("show thumbnail");
        st.set_input_region(&surface, Some(&[]));
        let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
        {
            let mut c = self.core.borrow_mut();
            c.th = Some(ui.as_weak());
            c.material = m;
        }
        self.thumb = Some(TWin { output: output.into(), surface, ui, a11y, glass_sent: String::new(), region_sent: Vec::new(), mapped: false });
        let c = self.core.borrow();
        c.push_thumb();
        if let Some(ui) = c.tui() {
            ui.set_shown(true);
        }
    }

    fn make_editor(&mut self, st: &mut State, output: &str) {
        let Some(wl) = st.output_named(output) else { return };
        let card = self.core.borrow().geo;
        let size = ((card.card_w + 2.0 * M_).round() as u32, (card.card_h + 2.0 * M_).round() as u32);
        let surface = st.prepare_surface(Spec {
            layer: Layer::Overlay,
            namespace: NS_EDITOR,
            output: Some((&wl, output)),
            anchor: Anchor::empty(),
            size,
            exclusive: -1,
            keyboard: KeyboardInteractivity::None,
            margin_left: 0,
        });
        let ui = ui::ShotEditor::new().expect("ShotEditor");
        st.shell.apply_theme(&ui.global::<ui::Theme>());
        let core = self.core.clone();
        let tx = st.tx.clone();
        let screen = output.to_string();
        // (`$c`: the core, borrowed, as the call site names it)
        macro_rules! sync {
            ($set:ident, $c:ident, |$($a:ident),*| $body:expr) => {{
                let core = core.clone();
                let tx = tx.clone();
                ui.$set(move |$($a),*| {
                    if let Ok(mut $c) = core.try_borrow_mut() {
                        $body;
                    }
                    // (the part follows: windows, sizes)
                    to_msg(&tx, Sm::Named);
                });
            }};
        }
        {
            let core = core.clone();
            let tx = tx.clone();
            let screen = screen.clone();
            ui.on_key(move |text, ctrl, shift| {
                let r = core.try_borrow_mut().map(|mut c| c.key(&text, ctrl, shift, &screen)).unwrap_or(false);
                to_msg(&tx, Sm::Named);
                r
            });
        }
        sync!(on_set_tool, c, |i| c.set_tool(TOOLS.get(i.max(0) as usize).map_or("pen", |t| t.0)));
        sync!(on_close_clicked, c, | | c.request_close());
        sync!(on_undo, c, | | c.undo());
        sync!(on_redo, c, | | c.redo());
        {
            let core = core.clone();
            ui.on_canvas_down(move |x, y, s| {
                if let Ok(mut c) = core.try_borrow_mut() {
                    c.down(x, y, s);
                }
            });
        }
        {
            let core = core.clone();
            ui.on_canvas_move(move |x, y, s, p| {
                if let Ok(mut c) = core.try_borrow_mut() {
                    c.moved(x, y, s, p);
                }
            });
        }
        {
            let core = core.clone();
            ui.on_canvas_up(move || {
                if let Ok(mut c) = core.try_borrow_mut() {
                    c.up();
                }
            });
        }
        sync!(on_text_done, c, |_t| c.commit_text());
        sync!(on_text_cancel, c, | | c.cancel_text());
        sync!(on_swatch, c, |i| {
            c.s.color = (i.max(0) as usize).min(PALETTE.len() - 1);
            // (recolour the label being typed)
            let ink = c.ink();
            if let Some(t) = c.edit_text.as_mut() {
                t.2 = ink;
            }
            c.push_editor();
        });
        sync!(on_set_width, c, |i| {
            c.s.width = (i.max(0) as usize).min(2);
            let size = c.text_size();
            if let Some(t) = c.edit_text.as_mut() {
                t.3 = size;
            }
            c.push_editor();
        });
        sync!(on_crop_reset, c, | | {
            c.pending_crop = c.full();
            c.push_editor();
        });
        sync!(on_crop_apply, c, | | c.apply_crop(true));
        sync!(on_remove, c, | | c.remove());
        {
            let screen = screen.clone();
            sync!(on_share, c, | | c.share(&screen));
        }
        sync!(on_copy, c, | | c.copy());
        sync!(on_save, c, | | c.save());
        sync!(on_save_copy, c, | | c.save_copy());
        sync!(on_keep_editing, c, | | c.keep_editing());
        sync!(on_discard, c, | | {
            c.banner = (0, String::new(), String::new());
            c.leave(After::Close);
        });
        sync!(on_dismiss_banner, c, | | {
            c.banner = (0, String::new(), String::new());
            c.push_editor();
        });
        sync!(on_banner_copy, c, | | {
            c.banner = (0, String::new(), String::new());
            c.copy();
        });
        {
            let screen = screen.clone();
            sync!(on_try_again, c, | | c.run_again(&screen));
        }
        {
            let core = core.clone();
            ui.on_lt_press(move |x, y, s| {
                if let Ok(mut c) = core.try_borrow_mut() {
                    let (shown, sx) = (c.shown(), c.sx());
                    c.live.press(x / sx + shown.x, y / sx + shown.y, s, sx);
                    c.push_live();
                }
            });
        }
        {
            let core = core.clone();
            ui.on_lt_drag(move |x, y| {
                if let Ok(mut c) = core.try_borrow_mut() {
                    let (shown, sx) = (c.shown(), c.sx());
                    c.live.drag(x / sx + shown.x, y / sx + shown.y, sx, sx);
                    c.push_live();
                }
            });
        }
        {
            let core = core.clone();
            let weak = ui.as_weak();
            ui.on_lt_hover(move |x, y| {
                if let Ok(c) = core.try_borrow() {
                    let (shown, sx) = (c.shown(), c.sx());
                    let over = c.live.over_word(x / sx + shown.x, y / sx + shown.y, sx);
                    if let Some(ui) = weak.upgrade() {
                        ui.set_over_word(over);
                    }
                }
            });
        }
        {
            let core = core.clone();
            ui.on_lt_double(move |x, y| {
                if let Ok(mut c) = core.try_borrow_mut() {
                    let (shown, sx) = (c.shown(), c.sx());
                    c.live.double(x / sx + shown.x, y / sx + shown.y, sx);
                    c.push_live();
                }
            });
        }
        sync!(on_lt_chip, c, |i| c.live_chip(i.max(0) as usize));
        sync!(on_lt_copy_sel, c, | | {
            let t = c.live.selected_text();
            c.live_copy(t, "Text copied");
        });
        sync!(on_lt_copy_all, c, | | {
            let t = c.live.all_text();
            c.live_copy(t, "All text copied");
        });
        sync!(on_lt_retry, c, | | c.live_start());
        {
            ui.on_lt_install(move || crate::shell::ask("livetext.install", json!([])));
        }
        {
            let tx = tx.clone();
            ui.on_closed(move || to_msg(&tx, Sm::EdClosed));
        }
        ui.set_no_icon(ui::StIc::default());
        ui.show().expect("show editor");
        let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
        {
            let mut c = self.core.borrow_mut();
            c.ed = Some(ui.as_weak());
            c.material = st.shell.look.material;
            c.setup_editor(&ui);
            ui.set_base(c.base_img.clone());
            c.push_anns();
            c.push_editor();
            c.push_live();
            ui.set_shown(true);
            ui.invoke_focus_keys();
        }
        self.editor = Some(EWin { output: output.into(), surface, ui, a11y, glass_sent: String::new(), size, mapped: false, keyboard: false });
    }

    pub fn look(&mut self, st: &mut State) {
        let m = st.shell.look.material;
        // (sizes, icons and shapes follow the look: made again)
        if let Some(w) = self.thumb.take() {
            Glass::clear_now(&w.output, NS_THUMB);
            st.drop_surface(&w.surface);
        }
        if let Some(w) = self.editor.take() {
            Glass::clear_now(&w.output, NS_EDITOR);
            st.drop_surface(&w.surface);
        }
        {
            let mut c = self.core.borrow_mut();
            c.material = m;
            c.th = None;
            c.ed = None;
            c.t_loaded = (String::new(), 0); // (the new window loads its picture again)
            c.recompute();
        }
        let b = Bridge::default();
        self.reconcile(st, &b);
    }

    pub fn output_removed(&mut self, st: &mut State, name: &str) {
        if self.thumb.as_ref().is_some_and(|w| w.output == name) {
            let w = self.thumb.take().unwrap();
            st.drop_surface(&w.surface);
        }
        if self.editor.as_ref().is_some_and(|w| w.output == name) {
            let w = self.editor.take().unwrap();
            st.drop_surface(&w.surface);
        }
    }

    pub fn after_frame(&mut self, st: &mut State) {
        let kbd = st.keyboard_surface();
        let plugin = st.shell.plugin && !st.shell.look.material;
        if let Some(w) = &mut self.thumb {
            if let Some(a) = w.a11y.as_mut() {
                a.update(false, false);
            }
            if w.mapped && w.surface.mapped.get() {
                let ui = &w.ui;
                let (x, y) = (ui.get_card_x(), ui.get_card_y());
                let (cw, ch) = (ui.get_card_w(), ui.get_card_h());
                let p = ui.get_progress();
                let region = if ui.get_shown() { vec![((x - 8.0).floor() as i32, (y - 8.0).floor() as i32, (cw + 16.0).ceil() as i32, (ch + 16.0).ceil() as i32)] } else { Vec::new() };
                if region != w.region_sent {
                    st.set_input_region(&w.surface, Some(&region));
                    w.region_sent = region;
                }
                if plugin {
                    let deleted = ui.get_deleted();
                    let o = if ui.get_fade() { p } else { 1.0 };
                    let shapes = if p > 0.002 { vec![theme::glass_shape(&st.shell.look, x, y, cw, ch, if deleted { 22.0 } else { 14.0 }, o, 1.0)] } else { Vec::new() };
                    let output = w.output.clone();
                    super::send_glass(st, &output, NS_THUMB, &shapes, &mut w.glass_sent);
                }
            }
        }
        if let Some(w) = &mut self.editor {
            let f = kbd.as_ref().is_some_and(|k| Rc::ptr_eq(k, &w.surface));
            if let Some(a) = w.a11y.as_mut() {
                a.update(false, f);
            }
            if w.mapped && w.surface.mapped.get() && plugin {
                let p = w.ui.get_progress();
                let (ww, wh) = (w.size.0 as f32 - 2.0 * M_, w.size.1 as f32 - 2.0 * M_);
                let s = 0.97 + 0.03 * p;
                let (x, y) = (M_ + ww * (1.0 - s) / 2.0, M_ + wh * (1.0 - s) / 2.0);
                let shapes = if p > 0.002 { vec![theme::glass_shape(&st.shell.look, x, y, ww * s, wh * s, 20.0 * s, p, 1.0)] } else { Vec::new() };
                let output = w.output.clone();
                super::send_glass(st, &output, NS_EDITOR, &shapes, &mut w.glass_sent);
            }
        }
    }

    pub fn a11y(&mut self) -> Vec<&mut crate::a11y::A11y> {
        let mut v: Vec<&mut crate::a11y::A11y> = Vec::new();
        if let Some(a) = self.thumb.as_mut().and_then(|w| w.a11y.as_mut()) {
            v.push(a);
        }
        if let Some(a) = self.editor.as_mut().and_then(|w| w.a11y.as_mut()) {
            v.push(a);
        }
        v
    }

    pub fn quit(&mut self) {
        if let Some(w) = &self.thumb {
            Glass::clear_now(&w.output, NS_THUMB);
        }
        if let Some(w) = &self.editor {
            Glass::clear_now(&w.output, NS_EDITOR);
        }
        // (a deletion waiting on its Undo is final now)
        if let Some(g) = self.core.borrow_mut().s.finish_delete() {
            let _ = std::process::Command::new("gio").args(["trash", "--", &g]).status();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_editor_fits_the_screenshot_in_the_screen() {
        // a 1080p screenshot on a 1080p screen: 82% of it, at most 1:1
        let g = geo(Some((1920, 1080)), (1920.0, 1080.0), Rect::new(0.0, 0.0, 1920.0, 1080.0), false);
        assert_eq!((g.canvas_w, g.canvas_h), ((1920.0f32 * ((1080.0 * 0.82 - 72.0 - 48.0) / 1080.0)).round(), (1080.0f32 * 0.82 - 120.0).round()));
        assert_eq!((g.card_w, g.card_h), (g.canvas_w + 24.0, g.canvas_h + 72.0 + 48.0));
        assert_eq!(g.unit, 3.0);
        // a small one isn't blown up; the canvas stays at least 720 × 240
        let g = geo(Some((300, 100)), (1920.0, 1080.0), Rect::new(0.0, 0.0, 300.0, 100.0), true);
        assert_eq!((g.canvas_w, g.canvas_h, g.fit), (720.0, 240.0, 2.0));
        assert_eq!(g.card_h, 240.0 + 96.0 + 48.0);
        // nothing loaded yet
        assert_eq!(geo(None, (1920.0, 1080.0), Rect::default(), false).canvas_h, 440.0);
    }

    #[test]
    fn thumbnails_keep_their_shape() {
        assert_eq!(thumb_size(16.0 / 9.0), (212.0, 125.0));
        assert_eq!(thumb_size(0.25), (84.0, 212.0));
        assert_eq!(thumb_size(10.0), (212.0, 84.0));
    }

    #[test]
    fn shift_makes_squares_and_45_degree_arrows() {
        assert_eq!(constrain((0.0, 0.0), (10.0, 4.0), false, false), (10.0, 4.0));
        assert_eq!(constrain((0.0, 0.0), (10.0, -4.0), true, false), (10.0, -10.0));
        let (x, y) = constrain((0.0, 0.0), (10.0, 9.0), true, true);
        assert!((x - y).abs() < 0.001);
    }

    #[test]
    fn drawing_makes_marks_in_the_images_pixels() {
        let mut c = Core::default();
        c.img = (1000, 500);
        c.status = 1;
        c.base = Some(Arc::new(RgbaImage::new(1000, 500)));
        c.recompute();
        let f = c.geo.fit;
        c.s.tool = "rect";
        c.down(10.0 * f, 10.0 * f, false);
        c.moved(110.0 * f, 60.0 * f, false, true);
        c.up();
        assert_eq!(c.s.items.len(), 1);
        match &c.s.items[0] {
            Ann::Rect { r, .. } => assert!((r.w - 100.0).abs() < 0.01 && (r.h - 50.0).abs() < 0.01),
            _ => panic!("not a rectangle"),
        }
        // a click with a shape tool leaves nothing; with the pen, a dot
        c.down(5.0, 5.0, false);
        c.up();
        assert_eq!(c.s.items.len(), 1);
        c.s.tool = "pen";
        c.down(5.0, 5.0, false);
        c.up();
        assert_eq!(c.s.items.len(), 2);
        // crop: a box, applied on Enter, undone with the history
        c.set_tool("crop");
        assert_eq!(c.s.tool, "crop");
        c.down(0.0, 0.0, false);
        c.moved(500.0 * f, 250.0 * f, false, true);
        c.up();
        assert!(c.key(SharedString::from(Key::Return).as_str(), false, false, ""));
        assert_eq!((c.s.tool, c.s.crop.w), ("pen", 500.0));
        assert!(c.key("z", true, false, ""));
        assert_eq!(c.s.crop.w, 0.0);
        // tool keys
        assert!(c.key("a", false, false, ""));
        assert_eq!(c.s.tool, "arrow");
    }
}

//! Circle to Search (Rust only: RUST-ONLY.md, "Screen tools"). The shell's
//! `select` = "circle" (the shortcut, Settings, a trackpad hold) opens it on
//! that screen:
//!
//! 1. The window maps empty and takes the keyboard while grim pictures the
//!    screen (the picture everything circled is cut from, so nothing of ours
//!    is in it); then a glow breathes along the four edges and the screen
//!    dims a little, rippling out from the pointer (a gesture) or rising from
//!    the bottom (a key). circle.slint draws it all; Reduce Motion: a fade,
//!    the glow still.
//! 2. Circle or scribble round anything (a straight drag is a box); the stroke
//!    follows the hand in the window's own callbacks, as the region
//!    selector's. Let go: the area is cut from the picture (a JPEG in the
//!    runtime folder's tessera-ai, owner-only, as Ask AI's), and an input bar
//!    comes in beside it: its thumbnail, "Ask about this…", Send, and the
//!    chips What is this?, Search the web, Copy text, Translate.
//! 3. Asking streams the answer into a card under the bar (the person's model
//!    from Settings → AI, through core's ai: models, keys, stream), the glow
//!    quickening while it thinks; "Continue in sidebar" hands the picture,
//!    question and answer to the sidebar's chat (`ai.continueCircle`).
//!    Search the web: the shell's lens-upload.py (the browser uploads it);
//!    Copy text: tesseract on the area, if installed; Translate: the model,
//!    else tesseract and translate-shell.
//! 4. Esc, a right click or the card's close: it plays back out, hides, and
//!    the glass goes with it. A press outside the bar starts a new circle.
//!
//! Errors say how to fix them: no model ("Set up AI in Settings → AI", with a
//! button there), no credential, a model that can't see, offline (Try
//! again), Lens failing, no Tesseract (Install).

use super::geometry::{stroke_box, stroke_kind, Rect, Stroke};
use super::{ic, Bridge, M};
use crate::services::ai::models::{self, Model, Opts, Settings};
use crate::services::ai::{keys::Keys, prompt, stream, Chat, Event, CIRCLE_QUESTION};
use crate::services::hypr::Glass;
use crate::theme;
use crate::ui;
use crate::wayland::{Spec, State, Surface};
use serde_json::{json, Value};
use slint::platform::Key;
use slint::{ComponentHandle, ModelRc, SharedPixelBuffer, SharedString, VecModel};
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use std::cell::RefCell;
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Off until its input bar is verified: in the sandbox (13 Nov) the glow,
/// the circling, the bar and Esc worked, but a click on a chip and typed
/// text drew no change, so the answer path is unchecked. Off, "circle"
/// opens the region selector's circle mode, as before.
pub const ENABLED: bool = false;

const NS: &str = "tessera:circle";
/// circle.slint's exit (320 ms), and a frame to spare
const EXIT_MS: u64 = 360;
/// The picture's longest side for the model (larger costs more, reads no better)
const MAX_SIDE: u32 = 1568;

/// What the chips are, in order
const CHIPS: [(&str, &str); 4] = [("What is this?", "question"), ("Search the web", "magnifying-glass"), ("Copy text", "text-t"), ("Translate", "translate")];

/// The circled area, cut out: the JPEG's path, it in base64, the thumbnail (w, h, RGBA)
pub struct Crop {
    path: String,
    b64: String,
    thumb: (u32, u32, Vec<u8>),
}

/// What an error's button does
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Fix {
    #[default]
    None,
    /// Settings → AI
    Settings,
    /// the same again
    Retry,
    /// tesseract, through Software Update (as Live Text's Install)
    Install,
}

impl Fix {
    fn label(self) -> &'static str {
        match self {
            Fix::None => "",
            Fix::Settings => "Open AI Settings",
            Fix::Retry => "Try again",
            Fix::Install => "Install",
        }
    }
}

/// Messages from the window's callbacks and the threads
pub enum Cm {
    /// the screen's picture (gen, it, the pointer on this screen)
    Captured(u64, Option<Arc<image::RgbaImage>>, Option<(f32, f32)>),
    /// a circle was drawn round this area
    Chosen(Rect),
    /// a press outside the bar and card: a new circle begins
    Restart,
    Cropped(u64, Result<Crop, String>),
    Dismiss,
    Chip(i32),
    Ask(String),
    StatusAct,
    AnswerAct,
    CopyAnswer,
    Continue,
    Ai(Event),
    /// the request never started: why, and the way to fix it
    AiFailed(u64, String, Fix),
    Lens(u64, Value),
    /// tesseract's text, or why there's none
    Text(u64, Result<String, String>),
    /// the translation (tesseract, then translate-shell)
    Translated(u64, Result<String, String>),
    Hide(u64),
}

fn to_part(st: &State) -> impl Fn(Cm) + Clone + 'static {
    let to = st.shell.sender(super::NAME);
    move |m| to(Box::new(M::Circle(m)))
}

/// The stroke as it's drawn (in the window's callbacks: the frame after a
/// move shows it)
#[derive(Default)]
struct Drag {
    dragging: bool,
    stroke: Vec<(f32, f32)>,
    path: String,
    w: f32,
    h: f32,
}

struct Win {
    output: String,
    surface: Rc<Surface>,
    ui: ui::CircleSearch,
    a11y: Option<crate::a11y::A11y>,
    glass_sent: String,
}

/// What was asked last (Try again asks it again)
#[derive(Clone, Default)]
struct Asked {
    shown: String,
    prompt: String,
    translate: bool,
}

#[derive(Default)]
pub struct Circle {
    win: Option<Win>,
    open: bool,
    screen: String,
    /// this opening's, then this circle's number: replies of an older one are dropped
    gen: u64,
    shot: Option<Arc<image::RgbaImage>>,
    drag: Rc<RefCell<Drag>>,
    crop: Option<Crop>,
    cropping: bool,
    /// a chip or question that came before the cut was ready
    pending: Option<Cm>,
    chips: [(bool, bool); 4],
    status_fix: Fix,
    /// Lens (true) or Copy text failed: which Try again means
    status_lens: bool,
    asked: Asked,
    answer: String,
    /// the answer as the model wrote it (Markdown: for the sidebar)
    raw: String,
    answer_fix: Fix,
    ai_gen: u64,
    ai_next: u64,
    /// tesseract's languages (the shell's pick)
    langs: String,
    cancel: Arc<AtomicU64>,
    model: Model,
    hide: slint::Timer,
    later: slint::Timer,
}

/// The chosen model (`ai.model`), else the first (the sidebar's and Spotlight's)
fn selected(cfg: &Value) -> Model {
    let list = models::read(&cfg["ai"]["models"]);
    let want = cfg["ai"]["model"].as_str().unwrap_or("");
    list.iter().find(|m| m.key == want).or(list.first()).cloned().unwrap_or_else(Model::none)
}

/// base64 (standard, padded)
fn base64(data: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(A[(n >> 18) as usize & 63] as char);
        out.push(A[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { A[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { A[n as usize & 63] as char } else { '=' });
    }
    out
}

/// An answer's Markdown as plain text for the card: no **, no #, bullets as •
pub fn plain(md: &str) -> String {
    let mut out = Vec::new();
    for line in md.lines() {
        let t = line.trim_start();
        let indent = &line[..line.len() - t.len()];
        let t = if let Some(r) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) {
            format!("{indent}• {r}")
        } else if t.starts_with('#') {
            t.trim_start_matches('#').trim_start().to_string()
        } else if t.starts_with("```") {
            continue;
        } else {
            line.to_string()
        };
        out.push(t.replace("**", "").replace("__", "").replace('`', ""));
    }
    out.join("\n").trim_end().to_string()
}

/// A language's English name for the prompt (sidebar.translateTarget's codes)
pub fn language(code: &str) -> String {
    let n = match code {
        "en" => "English",
        "de" => "German",
        "fr" => "French",
        "es" => "Spanish",
        "it" => "Italian",
        "pt" => "Portuguese",
        "nl" => "Dutch",
        "pl" => "Polish",
        "ru" => "Russian",
        "uk" => "Ukrainian",
        "tr" => "Turkish",
        "ar" => "Arabic",
        "hi" => "Hindi",
        "ja" => "Japanese",
        "ko" => "Korean",
        "zh" | "zh-CN" => "Chinese",
        "sv" => "Swedish",
        "da" => "Danish",
        "no" | "nb" => "Norwegian",
        "fi" => "Finnish",
        "el" => "Greek",
        "cs" => "Czech",
        _ => return if code.is_empty() { "English".into() } else { code.to_string() },
    };
    n.to_string()
}

/// "Copied 12 words"
pub fn copied(text: &str) -> String {
    let n = text.split_whitespace().count();
    format!("Copied {n} {}", if n == 1 { "word" } else { "words" })
}

/// The area of the screen's picture under `r` (logical px; the picture in
/// the output's pixels), as a JPEG for the model and a small thumbnail
fn cut(img: &image::RgbaImage, r: Rect, w: f32) -> Result<Crop, String> {
    let s = img.width() as f32 / w.max(1.0);
    let x = ((r.x * s).floor().max(0.0) as u32).min(img.width().saturating_sub(1));
    let y = ((r.y * s).floor().max(0.0) as u32).min(img.height().saturating_sub(1));
    let cw = ((r.w * s).ceil() as u32).clamp(1, img.width() - x);
    let ch = ((r.h * s).ceil() as u32).clamp(1, img.height() - y);
    let part = image::imageops::crop_imm(img, x, y, cw, ch).to_image();
    let long = cw.max(ch);
    let big = if long > MAX_SIDE {
        let k = MAX_SIDE as f32 / long as f32;
        image::imageops::resize(&part, ((cw as f32 * k).round() as u32).max(1), ((ch as f32 * k).round() as u32).max(1), image::imageops::FilterType::Triangle)
    } else {
        part.clone()
    };
    let rgb = image::DynamicImage::ImageRgba8(big).to_rgb8();
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 85).encode_image(&rgb).map_err(|e| e.to_string())?;
    // (owner-only, beside Ask AI's: the host clears ones over a day old)
    let dir = std::path::PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR").unwrap_or_else(|| "/tmp".into())).join("tessera-ai");
    {
        use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
        let _ = std::fs::DirBuilder::new().recursive(true).mode(0o700).create(&dir);
        let path = dir.join(format!("shot-circle-{}.jpg", crate::util::now_ms()));
        let mut f = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&path).map_err(|e| e.to_string())?;
        std::io::Write::write_all(&mut f, &jpeg).map_err(|e| e.to_string())?;
        // the thumbnail: its short side 96 px (the bar shows 44 px, covered)
        let k = (96.0 / cw.min(ch) as f32).min(1.0);
        let t = image::imageops::thumbnail(&part, ((cw as f32 * k).round() as u32).max(1), ((ch as f32 * k).round() as u32).max(1));
        Ok(Crop { path: path.to_string_lossy().into_owned(), b64: base64(&jpeg), thumb: (t.width(), t.height(), t.into_raw()) })
    }
}

/// The screen's picture (grim, the whole output) and the pointer on it
fn capture(output: &str, pointer: bool) -> (Option<image::RgbaImage>, Option<(f32, f32)>) {
    let at = if pointer {
        let j = |c: &str| crate::services::hypr::request(c).ok().and_then(|s| serde_json::from_str::<Value>(&s).ok()).unwrap_or_default();
        let (c, mons) = (j("j/cursorpos"), j("j/monitors"));
        let m = mons.as_array().and_then(|a| a.iter().find(|m| m["name"].as_str() == Some(output)).cloned()).unwrap_or_default();
        c["x"].as_f64().map(|x| ((x - m["x"].as_f64().unwrap_or(0.0)) as f32, (c["y"].as_f64().unwrap_or(0.0) - m["y"].as_f64().unwrap_or(0.0)) as f32))
    } else {
        None
    };
    let out = Command::new("grim").args(["-l", "0", "-o", output, "-"]).stdin(Stdio::null()).stderr(Stdio::null()).output();
    let img = out.ok().filter(|o| o.status.success()).and_then(|o| image::load_from_memory(&o.stdout).ok()).map(|i| i.to_rgba8());
    (img, at)
}

impl Circle {
    fn window(&mut self, st: &mut State, output: &str) -> bool {
        if self.win.as_ref().is_some_and(|w| w.output != output) {
            let w = self.win.take().unwrap();
            Glass::clear_now(&w.output, NS);
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
        let ui = ui::CircleSearch::new().expect("CircleSearch");
        st.shell.apply_theme(&ui.global::<ui::Theme>());
        let to = to_part(st);
        {
            let (d, weak, to) = (self.drag.clone(), ui.as_weak(), to.clone());
            ui.on_down(move |x, y, right| {
                let Some(ui) = weak.upgrade() else { return };
                if right {
                    to(Cm::Dismiss);
                    return;
                }
                if ui.get_chosen() {
                    to(Cm::Restart);
                }
                let mut d = d.borrow_mut();
                let p = (x.clamp(0.0, d.w), y.clamp(0.0, d.h));
                d.stroke = vec![p];
                d.path = format!("M {:.1} {:.1} L {:.1} {:.1}", p.0, p.1, p.0 + 0.01, p.1);
                d.dragging = true;
                ui.set_chosen(false);
                ui.set_card(false);
                ui.set_stroke(d.path.as_str().into());
                ui.set_dragging(true);
            });
        }
        {
            let (d, weak) = (self.drag.clone(), ui.as_weak());
            ui.on_moved_to(move |x, y| {
                let mut d = d.borrow_mut();
                if !d.dragging {
                    return;
                }
                let p = (x.clamp(0.0, d.w), y.clamp(0.0, d.h));
                // (a point every 2 px is smooth enough)
                let last = *d.stroke.last().unwrap_or(&p);
                if (p.0 - last.0).hypot(p.1 - last.1) >= 2.0 {
                    d.stroke.push(p);
                    d.path += &format!(" L {:.1} {:.1}", p.0, p.1);
                    if let Some(ui) = weak.upgrade() {
                        ui.set_stroke(d.path.as_str().into());
                    }
                }
            });
        }
        {
            let (d, weak, to) = (self.drag.clone(), ui.as_weak(), to.clone());
            ui.on_up(move |_, _| {
                let mut d = d.borrow_mut();
                if !d.dragging {
                    return;
                }
                d.dragging = false;
                let Some(ui) = weak.upgrade() else { return };
                ui.set_dragging(false);
                // (a click alone circles nothing)
                if stroke_kind(&d.stroke) == Stroke::None {
                    return;
                }
                let r = stroke_box(&d.stroke, 12.0, d.w, d.h);
                if r.w < 8.0 || r.h < 8.0 {
                    return;
                }
                ui.set_sel_x(r.x);
                ui.set_sel_y(r.y);
                ui.set_sel_w(r.w);
                ui.set_sel_h(r.h);
                ui.set_chosen(true);
                to(Cm::Chosen(r));
            });
        }
        {
            let to = to.clone();
            ui.on_key(move |t| {
                if t == SharedString::from(Key::Escape) {
                    to(Cm::Dismiss);
                    return true;
                }
                false
            });
        }
        let cb = |f: fn() -> Cm| {
            let to = to.clone();
            move || to(f())
        };
        ui.on_dismiss(cb(|| Cm::Dismiss));
        ui.on_status_act(cb(|| Cm::StatusAct));
        ui.on_answer_act(cb(|| Cm::AnswerAct));
        ui.on_copy_answer(cb(|| Cm::CopyAnswer));
        ui.on_continue_sidebar(cb(|| Cm::Continue));
        {
            let to = to.clone();
            ui.on_chip(move |i| to(Cm::Chip(i)));
        }
        {
            let to = to.clone();
            ui.on_ask(move |q| to(Cm::Ask(q.to_string())));
        }
        Self::icons(&ui, st.shell.look.material);
        ui.show().expect("show circle to search");
        let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
        self.win = Some(Win { output: output.into(), surface, ui, a11y, glass_sent: String::new() });
        true
    }

    fn icons(ui: &ui::CircleSearch, m: bool) {
        ui.set_ic_send(ic("arrow-up", m));
        ui.set_ic_copy(ic("copy", m));
        ui.set_ic_close(ic("x", m));
        ui.set_ic_sidebar(ic("sidebar-simple", m));
        ui.set_ic_sparkle(ic("sparkle", m));
        ui.set_ic_gear(ic("gear", m));
    }

    fn sync_chips(&self) {
        let Some(w) = &self.win else { return };
        let m = theme_material(&w.ui);
        let v: Vec<ui::CsChip> = CHIPS.iter().zip(self.chips).map(|((label, icon), (busy, done))| ui::CsChip { label: (*label).into(), icon: ic(icon, m), busy, done }).collect();
        w.ui.set_chips(ModelRc::new(VecModel::from(v)));
    }

    /// Made ahead (a moment after start) and hidden at once, so the first
    /// opening draws without building the window
    pub fn make_ahead(&mut self, st: &mut State, output: &str) {
        if self.win.is_none() && !self.open && self.window(st, output) {
            if let Some(w) = &self.win {
                w.surface.hide();
            }
        }
    }

    /// The shell's select became "circle"
    pub fn open(&mut self, st: &mut State, b: &Bridge) {
        let output = if b.select_screen.is_empty() { super::focused_output(st, b).unwrap_or_default() } else { b.select_screen.clone() };
        if self.open && output == self.screen {
            return;
        }
        if !self.window(st, &output) {
            return;
        }
        self.hide.stop();
        self.later.stop();
        self.stop_ai();
        self.open = true;
        self.gen += 1;
        self.screen = output.clone();
        self.shot = None;
        self.crop = None;
        self.cropping = false;
        self.pending = None;
        self.chips = Default::default();
        self.asked = Asked::default();
        self.answer.clear();
        self.status_fix = Fix::None;
        self.answer_fix = Fix::None;
        let (w, h) = super::output_size(st, &output);
        *self.drag.borrow_mut() = Drag { w, h, ..Default::default() };
        let pointer = b.select_from == "pointer";
        let reduce = st.shell.look.reduce_motion;
        st.shell.want("ai");
        self.sync_chips();
        let win = self.win.as_ref().unwrap();
        let ui = &win.ui;
        ui.set_open(false);
        ui.set_pointer_origin(pointer);
        ui.set_ox(w / 2.0);
        ui.set_oy(h);
        ui.set_reduce_motion(reduce);
        ui.set_busy(false);
        ui.set_hint(super::geometry::hint("circle").into());
        ui.set_dragging(false);
        ui.set_stroke("".into());
        ui.set_chosen(false);
        ui.set_thumb(slint::Image::default());
        ui.set_question("".into());
        ui.set_card(false);
        self.set_status("", false);
        win.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        st.set_input_region(&win.surface, None);
        win.surface.show(0, 0);
        ui.invoke_focus_keys();
        // the picture first (the window maps empty: nothing of ours in it), then the glow
        let (to, gen) = (to_part(st), self.gen);
        std::thread::spawn(move || {
            let (img, at) = capture(&output, pointer);
            to(Cm::Captured(gen, img.map(Arc::new), at));
        });
    }

    /// Put away: played back out, then hidden (the shell's select changed)
    pub fn close(&mut self, st: &mut State) {
        if !self.open {
            return;
        }
        self.open = false;
        self.stop_ai();
        self.gen += 1;
        self.later.stop();
        let Some(w) = &self.win else { return };
        w.ui.set_open(false);
        w.ui.set_busy(false);
        w.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::None);
        let (to, gen) = (to_part(st), self.gen);
        self.hide.start(slint::TimerMode::SingleShot, Duration::from_millis(EXIT_MS), move || to(Cm::Hide(gen)));
    }

    /// Esc, a right click, close: put away, and the shell stops selecting
    fn dismiss(&mut self, st: &mut State) {
        if self.open {
            self.close(st);
            crate::shell::ask("screen.cancel", json!([]));
        }
    }

    fn set_status(&self, text: &str, error: bool) {
        if let Some(w) = &self.win {
            w.ui.set_status(text.into());
            w.ui.set_status_error(error);
            w.ui.set_status_action(if error { self.status_fix.label() } else { "" }.into());
        }
    }

    fn stop_ai(&mut self) {
        self.ai_gen = 0;
        self.cancel.store(0, Ordering::SeqCst);
    }

    fn sync_answer(&self, state: i32) {
        let Some(w) = &self.win else { return };
        w.ui.set_card(true);
        w.ui.set_card_question(self.asked.shown.as_str().into());
        w.ui.set_answer(self.answer.as_str().into());
        w.ui.set_answer_state(state);
        w.ui.set_answer_action(if state == 3 { self.answer_fix.label() } else { "" }.into());
        w.ui.set_model_name(self.model.name.as_str().into());
        w.ui.set_busy(state <= 1);
    }

    fn fail(&mut self, text: &str, fix: Fix) {
        self.ai_gen = 0;
        self.answer = text.to_string();
        self.answer_fix = fix;
        self.sync_answer(3);
    }

    /// Ask the person's model about the circled picture
    fn ask(&mut self, st: &mut State, a: Asked) {
        self.stop_ai();
        self.asked = a.clone();
        self.answer.clear();
        self.answer_fix = Fix::None;
        let cfg = st.shell.cfg.clone();
        self.model = selected(&cfg);
        let m = self.model.clone();
        if let Some(w) = &self.win {
            w.ui.set_question("".into());
        }
        if !m.configured() {
            if a.translate && self.translate_without_ai(st) {
                return;
            }
            self.model = Model::none();
            return self.fail("Set up AI in Settings → AI to ask about what you circle.", Fix::Settings);
        }
        if !m.vision {
            return self.fail(&format!("{} can't read pictures. Choose a model that can, such as Claude or Gemini, in Settings → AI.", m.name), Fix::Settings);
        }
        let Some(crop) = &self.crop else { return };
        self.sync_answer(0);
        self.cancel = Arc::new(AtomicU64::new(0));
        self.ai_next += 1;
        self.ai_gen = self.ai_next;
        let gen = self.ai_gen;
        self.cancel.store(gen, Ordering::SeqCst);
        let cancel = self.cancel.clone();
        let cached = st.shell.get::<Keys>().cloned();
        let (to, b64, prompt_text) = (to_part(st), crop.b64.clone(), a.prompt.clone());
        // (on a thread: a credential not read yet is looked up in the keyring)
        std::thread::spawn(move || {
            let cred = models::credential_provider(&m);
            let mut ks = cached.unwrap_or_default();
            if m.auth != "none" && !ks.has(&cred, &m.key) {
                let mut slots: Vec<String> = crate::services::ai::keys::SLOTS.iter().map(|s| s.to_string()).collect();
                slots.push(m.key.clone());
                ks.merge(Keys::load(&slots));
            }
            if m.auth != "none" && !ks.has(&cred, &m.key) {
                let why = format!("Add your {} {} in Settings → AI, then try again.", models::provider_name(&m.provider), models::credential_label(&m).to_lowercase());
                return to(Cm::AiFailed(gen, why, Fix::Settings));
            }
            let opts = Opts { system: prompt::system(&cfg["ai"], true), max_tokens: Some(1024), thinking: Some(false), effort: Some("low".into()) };
            let turns = [json!({"role": "user", "text": prompt_text, "images": [{"data": b64, "mediaType": "image/jpeg"}]})];
            match models::build_request(&m, &turns, &opts, &Settings::from(&cfg["ai"]), &ks.get(&cred, &m.key)) {
                Ok(req) => {
                    let t = to.clone();
                    stream(req, models::protocol(&m).to_string(), gen, cancel, move |e| t(Cm::Ai(e)));
                }
                Err(e) => to(Cm::AiFailed(gen, e, Fix::Settings)),
            }
        });
    }

    /// Translate with no model set up: tesseract, then translate-shell (as the
    /// screen's Translate); false when either is missing
    fn translate_without_ai(&mut self, st: &mut State) -> bool {
        let has = |t: &str| Command::new("sh").args(["-c", "command -v \"$1\" >/dev/null", "sh", t]).status().is_ok_and(|s| s.success());
        let Some(crop) = &self.crop else { return false };
        if !has("tesseract") || !has("trans") {
            return false;
        }
        self.model = Model { name: "Translate".into(), ..Model::none() };
        self.sync_answer(0);
        let target = st.shell.cfg["sidebar"]["translateTarget"].as_str().unwrap_or("en").to_string();
        let (to, gen, path, langs) = (to_part(st), self.gen, crop.path.clone(), self.langs.clone());
        std::thread::spawn(move || {
            let src = Command::new("tesseract").args([&path, "stdout", "-l", &langs]).stdin(Stdio::null()).stderr(Stdio::null()).output();
            let src = src.ok().map(|o| String::from_utf8_lossy(&o.stdout).split_whitespace().collect::<Vec<_>>().join(" ")).unwrap_or_default();
            if src.is_empty() {
                return to(Cm::Translated(gen, Err("No text found in the circled area. Circle a larger area with clearer text.".into())));
            }
            let out = Command::new("trans").args(["-brief", "-no-ansi", "-t", &target, "--", &src]).stdin(Stdio::null()).stderr(Stdio::null()).output();
            let r = match out {
                Ok(o) if o.status.success() && !o.stdout.is_empty() => Ok(String::from_utf8_lossy(&o.stdout).trim().to_string()),
                _ => Err("Couldn't translate. Check your internet connection, then try again.".to_string()),
            };
            to(Cm::Translated(gen, r));
        });
        true
    }

    fn chip(&mut self, st: &mut State, i: i32, b: &Bridge) {
        if self.crop.is_none() {
            if self.cropping {
                self.pending = Some(Cm::Chip(i));
            }
            return;
        }
        match i {
            0 => self.ask(st, Asked { shown: "What is this?".into(), prompt: CIRCLE_QUESTION.into(), translate: false }),
            1 => self.lens(st),
            2 => self.copy_text(st, b),
            3 => {
                let lang = language(st.shell.cfg["sidebar"]["translateTarget"].as_str().unwrap_or("en"));
                let prompt = format!("Translate the text in this picture into {lang}. Reply with the translation only.");
                self.ask(st, Asked { shown: format!("Translate into {lang}"), prompt, translate: true });
            }
            _ => {}
        }
    }

    /// Search the web: the browser uploads the picture to Lens (lens-upload.py)
    fn lens(&mut self, st: &mut State) {
        let Some(crop) = &self.crop else { return };
        if self.chips[1].0 {
            return;
        }
        self.chips[1] = (true, false);
        self.sync_chips();
        self.status_fix = Fix::None;
        self.set_status("", false);
        let (to, gen, path) = (to_part(st), self.gen, crop.path.clone());
        let script = crate::util::asset("shell/assets/scripts/lens-upload.py").map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
        std::thread::spawn(move || {
            let out = Command::new("python3").args(["-I", &script, &path]).stdin(Stdio::null()).stderr(Stdio::null()).output();
            let r = match out {
                Ok(o) => serde_json::from_slice::<Value>(&o.stdout).unwrap_or(json!({"ok": false})),
                Err(_) => json!({"ok": false, "error": "Couldn't start the search. Make sure python3 is installed, then try again."}),
            };
            to(Cm::Lens(gen, r));
        });
    }

    fn lens_done(&mut self, st: &mut State, r: Value) {
        let url = r["url"].as_str().unwrap_or("").trim().to_string();
        let ok_url = url.starts_with("http://127.0.0.1:") || url.starts_with("https://");
        if r["ok"] == true && ok_url {
            self.chips[1] = (false, true);
            self.sync_chips();
            crate::shell::ask("launch.openLink", json!([url]));
            // (the results open in the browser: out of its way)
            self.dismiss(st);
        } else if r["fallback"] == true {
            // (Lens changed how it takes pictures: paste it there instead)
            if let Some(c) = &self.crop {
                if let Ok(bytes) = std::fs::read(&c.path) {
                    crate::util::pipe_to(&["wl-copy", "--type", "image/jpeg"], bytes);
                }
            }
            crate::shell::ask("launch.openLink", json!(["https://lens.google.com/"]));
            self.chips[1] = (false, true);
            self.sync_chips();
            self.status_fix = Fix::None;
            self.set_status("Lens opened in your browser. Paste the picture there with Ctrl+V: it's on the clipboard.", false);
        } else {
            self.chips[1] = (false, false);
            self.sync_chips();
            self.status_fix = Fix::Retry;
            self.status_lens = true;
            let why = r["error"].as_str().filter(|e| !e.is_empty()).unwrap_or("Lens didn't answer. Check your internet connection, then try again.");
            self.set_status(why, true);
        }
    }

    /// Copy text: tesseract reads the circled area, if it's installed
    fn copy_text(&mut self, st: &mut State, b: &Bridge) {
        let Some(crop) = &self.crop else { return };
        if self.chips[2].0 {
            return;
        }
        if !b.ocr.available {
            self.status_fix = if b.ocr.installing { Fix::None } else { Fix::Install };
            self.status_lens = false;
            let why = if b.ocr.installing { "Tesseract is being installed. Try again in a moment." } else { "Copying text needs Tesseract, which isn't installed." };
            return self.set_status(why, true);
        }
        self.chips[2] = (true, false);
        self.sync_chips();
        self.status_fix = Fix::None;
        self.set_status("", false);
        let (to, gen, path, langs) = (to_part(st), self.gen, crop.path.clone(), self.langs.clone());
        std::thread::spawn(move || {
            let out = Command::new("tesseract").args([&path, "stdout", "-l", &langs]).stdin(Stdio::null()).stderr(Stdio::null()).output();
            let r = match out {
                Ok(o) if o.status.success() => Ok(String::from_utf8_lossy(&o.stdout).trim().to_string()),
                _ => Err("Couldn't read the text. Make sure Tesseract and its English data (tesseract-data-eng) are installed.".to_string()),
            };
            to(Cm::Text(gen, r));
        });
    }

    pub fn message(&mut self, st: &mut State, m: Cm, b: &Bridge) {
        self.langs = if b.ocr_langs.is_empty() { "eng".into() } else { b.ocr_langs.clone() };
        match m {
            Cm::Captured(gen, img, at) => {
                if gen != self.gen || !self.open {
                    return;
                }
                let Some(w) = &self.win else { return };
                if let Some((x, y)) = at {
                    w.ui.set_ox(x);
                    w.ui.set_oy(y);
                }
                if img.is_none() {
                    w.ui.set_hint("Couldn't take a picture of the screen. Make sure grim is installed, then try again.".into());
                }
                self.shot = img;
                w.ui.set_open(true);
            }
            Cm::Restart => {
                self.gen += 1;
                self.stop_ai();
                self.crop = None;
                self.cropping = false;
                self.pending = None;
                self.chips = Default::default();
                self.sync_chips();
                self.status_fix = Fix::None;
                self.set_status("", false);
                if let Some(w) = &self.win {
                    w.ui.set_busy(false);
                    w.ui.set_card(false);
                }
            }
            Cm::Chosen(r) => {
                if !self.open {
                    return;
                }
                self.gen += 1;
                self.stop_ai();
                self.crop = None;
                self.chips = Default::default();
                self.sync_chips();
                self.status_fix = Fix::None;
                self.set_status("", false);
                if let Some(w) = &self.win {
                    w.ui.set_thumb(slint::Image::default());
                    w.ui.set_card(false);
                    w.ui.set_busy(false);
                    w.ui.invoke_focus_field();
                }
                let Some(img) = self.shot.clone() else {
                    self.status_fix = Fix::None;
                    return self.set_status("Couldn't take a picture of the screen. Make sure grim is installed, then try again.", true);
                };
                self.cropping = true;
                let (to, gen, lw) = (to_part(st), self.gen, self.drag.borrow().w);
                std::thread::spawn(move || to(Cm::Cropped(gen, cut(&img, r, lw))));
            }
            Cm::Cropped(gen, r) => {
                if gen != self.gen {
                    return;
                }
                self.cropping = false;
                match r {
                    Ok(c) => {
                        if let Some(w) = &self.win {
                            let (tw, th, px) = &c.thumb;
                            w.ui.set_thumb(slint::Image::from_rgba8(SharedPixelBuffer::clone_from_slice(px, *tw, *th)));
                        }
                        self.crop = Some(c);
                        if let Some(p) = self.pending.take() {
                            self.message(st, p, b);
                        }
                    }
                    Err(e) => {
                        eprintln!("tessera-ui: circle to search: couldn't keep the picture: {e}");
                        self.status_fix = Fix::None;
                        self.set_status("Couldn't keep the circled picture. Free some space in the runtime folder, then circle again.", true);
                    }
                }
            }
            Cm::Dismiss => self.dismiss(st),
            Cm::Chip(i) => self.chip(st, i, b),
            Cm::Ask(q) => {
                let q = q.trim().to_string();
                if q.is_empty() {
                    return;
                }
                if self.crop.is_none() {
                    if self.cropping {
                        self.pending = Some(Cm::Ask(q));
                    }
                    return;
                }
                self.ask(st, Asked { shown: q.clone(), prompt: q, translate: false });
            }
            Cm::StatusAct => match self.status_fix {
                Fix::Retry if self.status_lens => self.lens(st),
                Fix::Retry => self.copy_text(st, b),
                Fix::Install => {
                    crate::shell::ask("livetext.install", json!([]));
                    self.status_fix = Fix::None;
                    self.set_status("Installing Tesseract with Software Update…", false);
                }
                Fix::Settings => self.open_settings(st),
                Fix::None => {}
            },
            Cm::AnswerAct => match self.answer_fix {
                Fix::Settings => self.open_settings(st),
                Fix::Retry => {
                    let a = self.asked.clone();
                    self.ask(st, a);
                }
                _ => {}
            },
            Cm::CopyAnswer => {
                if !self.answer.is_empty() {
                    crate::util::pipe_to(&["wl-copy"], self.answer.clone().into_bytes());
                }
            }
            Cm::Continue => {
                if self.answer.is_empty() {
                    return;
                }
                let path = self.crop.as_ref().map(|c| c.path.clone()).unwrap_or_default();
                crate::shell::ask("ai.continueCircle", json!([self.asked.shown, self.raw.clone(), path, self.screen]));
                self.dismiss(st);
            }
            Cm::Ai(Event::Update { gen, text, .. }) => {
                if gen != self.ai_gen || gen == 0 {
                    return;
                }
                self.raw = text.clone();
                self.answer = plain(&text);
                self.sync_answer(1);
            }
            Cm::Ai(Event::Done { gen, status, parser, body }) => {
                if gen != self.ai_gen || gen == 0 {
                    return;
                }
                self.ai_gen = 0;
                self.raw = parser.text.clone();
                if status == 200 && parser.error.is_empty() && parser.stop_reason != "refusal" && !parser.text.trim().is_empty() {
                    self.answer = plain(&parser.text);
                    self.answer_fix = Fix::None;
                    return self.sync_answer(2);
                }
                let mut c = Chat::default();
                c.model = self.model.clone();
                let p = models::provider_name(&self.model.provider);
                let (why, fix) = match status {
                    // (offline, or the endpoint is down)
                    0 => (format!("Couldn't reach {p}. You may be offline: check your internet connection{}, then try again.", if self.model.provider == "ollama" { " or that Ollama is running" } else { "" }), Fix::Retry),
                    401 | 403 | 404 => (c.error_text(status, &body), Fix::Settings),
                    200 if parser.stop_reason == "refusal" => ("This can't be answered here. Try asking another way.".into(), Fix::None),
                    200 if !parser.error.is_empty() => (parser.error.clone(), Fix::Retry),
                    200 => ("No answer came back. Try again.".into(), Fix::Retry),
                    s => (c.error_text(s, &body), Fix::Retry),
                };
                self.raw.clear();
                self.fail(&why, fix);
            }
            Cm::AiFailed(gen, why, fix) => {
                if gen == self.ai_gen && gen != 0 {
                    self.fail(&why, fix);
                }
            }
            Cm::Lens(gen, r) => {
                if gen == self.gen {
                    self.lens_done(st, r);
                }
            }
            Cm::Text(gen, r) => {
                if gen != self.gen {
                    return;
                }
                self.chips[2] = (false, false);
                match r {
                    Ok(t) if !t.is_empty() => {
                        crate::util::pipe_to(&["wl-copy"], t.clone().into_bytes());
                        self.chips[2] = (false, true);
                        self.status_fix = Fix::None;
                        self.set_status(&copied(&t), false);
                    }
                    Ok(_) => {
                        self.status_fix = Fix::None;
                        self.set_status("No text found in the circled area. Circle a larger area with clearer text.", true);
                    }
                    Err(e) => {
                        self.status_fix = Fix::Retry;
                        self.status_lens = false;
                        self.set_status(&e, true);
                    }
                }
                self.sync_chips();
            }
            Cm::Translated(gen, r) => {
                if gen != self.gen {
                    return;
                }
                match r {
                    Ok(t) => {
                        self.raw = t.clone();
                        self.answer = t;
                        self.sync_answer(2);
                    }
                    Err(e) => self.fail(&e, Fix::Retry),
                }
            }
            Cm::Hide(gen) => {
                if gen != self.gen || self.open {
                    return;
                }
                self.shot = None;
                self.crop = None;
                if let Some(w) = &mut self.win {
                    w.surface.hide();
                    st.shell.glass.set(&w.output, NS, &[]);
                    w.glass_sent.clear();
                    w.ui.set_chosen(false);
                    w.ui.set_card(false);
                    w.ui.set_stroke("".into());
                    w.ui.set_thumb(slint::Image::default());
                }
            }
        }
    }

    fn open_settings(&mut self, st: &mut State) {
        crate::shell::ask("panels.openSettings", json!(["ai"]));
        self.dismiss(st);
    }

    pub fn look(&mut self, st: &mut State) {
        if let Some(w) = &mut self.win {
            st.shell.apply_theme(&w.ui.global::<ui::Theme>());
            Self::icons(&w.ui, st.shell.look.material);
            w.ui.set_reduce_motion(st.shell.look.reduce_motion);
            w.glass_sent.clear();
        }
        self.sync_chips();
    }

    pub fn output_removed(&mut self, st: &mut State, name: &str) {
        if self.win.as_ref().is_some_and(|w| w.output == name) {
            let was = self.open;
            self.open = false;
            self.stop_ai();
            let w = self.win.take().unwrap();
            Glass::clear_now(&w.output, NS);
            st.drop_surface(&w.surface);
            if was {
                crate::shell::ask("screen.cancel", json!([]));
            }
        }
    }

    /// The glass under the bar and the card (Glass with the plugin), as they
    /// show: none once they've gone
    pub fn after_frame(&mut self, st: &mut State) {
        let kbd = st.keyboard_surface();
        let Some(w) = &mut self.win else { return };
        let f = kbd.as_ref().is_some_and(|k| Rc::ptr_eq(k, &w.surface));
        if let Some(a) = w.a11y.as_mut() {
            a.update(false, f);
        }
        if !w.surface.mapped.get() || !st.shell.plugin || st.shell.look.material {
            return;
        }
        let ui = &w.ui;
        let o = ui.get_bar_o();
        let mut shapes = Vec::new();
        if o > 0.0 && ui.get_chosen() {
            // (as the column grows from 96% round its middle)
            let s = 0.96 + 0.04 * o;
            let (bx, by, bw) = (ui.get_bar_x(), ui.get_bar_y(), ui.get_bar_w());
            let (cx, cy) = (bx + bw / 2.0, by + (ui.get_card_y() + ui.get_card_h() - by).max(ui.get_bar_h()) / 2.0);
            let sc = |x: f32, y: f32, w: f32, h: f32| (cx + (x - cx) * s, cy + (y - cy) * s, w * s, h * s);
            let r = ui.get_radius() * s;
            let (x, y, ww, hh) = sc(bx, by, bw, ui.get_bar_h());
            shapes.push(theme::glass_shape(&st.shell.look, x, y, ww, hh, r, o, 1.0));
            if ui.get_card() && ui.get_card_h() > 0.0 {
                let (x, y, ww, hh) = sc(bx, ui.get_card_y(), bw, ui.get_card_h());
                shapes.push(theme::glass_shape(&st.shell.look, x, y, ww, hh, r, o, 1.0));
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
            Glass::clear_now(&w.output, NS);
        }
    }
}

fn theme_material(ui: &ui::CircleSearch) -> bool {
    ui.global::<ui::Theme>().get_material()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_read_as_plain_text() {
        assert_eq!(plain("**A tram**\n\n- old\n- yellow\n# Head"), "A tram\n\n• old\n• yellow\nHead");
        assert_eq!(plain("```text\ncode\n```"), "code");
    }

    #[test]
    fn languages_and_counts() {
        assert_eq!(language("de"), "German");
        assert_eq!(language(""), "English");
        assert_eq!(language("xx"), "xx");
        assert_eq!(copied("one"), "Copied 1 word");
        assert_eq!(copied("a b  c"), "Copied 3 words");
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
    }

    #[test]
    fn the_circled_area_is_cut_at_the_screens_scale() {
        // a 2× screen: 100×50 logical, 200×100 pixels; the right half red
        let mut img = image::RgbaImage::new(200, 100);
        for (x, _, p) in img.enumerate_pixels_mut() {
            *p = if x >= 100 { image::Rgba([255, 0, 0, 255]) } else { image::Rgba([0, 0, 255, 255]) };
        }
        let dir = std::env::temp_dir().join(format!("tessera-circle-test-{}", std::process::id()));
        std::env::set_var("XDG_RUNTIME_DIR", &dir);
        let c = cut(&img, Rect::new(50.0, 0.0, 50.0, 50.0), 100.0).unwrap();
        let back = image::load_from_memory(&std::fs::read(&c.path).unwrap()).unwrap().to_rgb8();
        assert_eq!(back.dimensions(), (100, 100));
        assert!(back.get_pixel(50, 50)[0] > 200 && back.get_pixel(50, 50)[2] < 60);
        assert_eq!(c.thumb.0.min(c.thumb.1), 96);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

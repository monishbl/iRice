//! Quick Look (rust.surfaces: quicklook), both looks: a preview of a file
//! over whatever is open (shell/modules/quicklook/QuickLook.qml), opened as
//! before through Panels.quickLook (QuickLook.qml forwards the path as
//! `{"cmd": "open"|"close", "surface": "quicklook", "path", "query",
//! "screen"}`). Pictures (T: Live Text, the screenshot editor's), video and
//! audio, a PDF's first page, text and code, a folder's contents, or the
//! file's details. Space or Esc closes it, Return opens the file in its app.
//! Closing itself asks the shell (`panels.quickLook` with no path).
//!
//! Differences from the QML: Slint has no media player, so video plays as
//! frames ffmpeg decodes into the card (its sound through ffplay, when
//! installed) and audio through ffplay; pausing stops both processes
//! (SIGSTOP) and the progress follows the clock. The text view selects
//! but has no scroll bar.

use crate::services::hypr::Glass;
use crate::shell::{Command, Part};
use super::livetext::{self, Live, Ocr};
use super::{ic, ltui};
use crate::theme;
use crate::ui;
use crate::wayland::{Spec, State, Surface};
use crate::Msg;
use serde_json::{json, Value};
use slint::platform::Key;
use slint::{ComponentHandle, ModelRc, SharedPixelBuffer, SharedString, VecModel};
use smithay_client_toolkit::reexports::calloop::channel::Sender;
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use smithay_client_toolkit::shell::WaylandSurface;
use std::any::Any;
use std::cell::RefCell;
use std::process::{Child, Stdio};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const NAME: &str = "quicklook";
const NS: &str = "tessera:quicklook";
/// Material's paused play button (MaterialShape's "square": corners a quarter round)
const ROUNDED_SQUARE: &str = "M 26 2 L 74 2 Q 98 2 98 26 L 98 74 Q 98 98 74 98 L 26 98 Q 2 98 2 74 L 2 26 Q 2 2 26 2 Z";

/// What a file is: from its type (mime), else its extension
pub fn kind_of(mime: &str, ext: &str) -> &'static str {
    let e = ext.to_lowercase();
    let has = |l: &[&str]| l.contains(&e.as_str());
    if mime == "inode/directory" {
        "folder"
    } else if mime.starts_with("image/") || has(&["png", "jpg", "jpeg", "webp", "gif", "bmp", "svg", "avif"]) {
        "image"
    } else if mime.starts_with("video/") || has(&["mp4", "mkv", "webm", "mov", "avi"]) {
        "video"
    } else if mime.starts_with("audio/") || has(&["mp3", "flac", "ogg", "wav", "m4a", "opus"]) {
        "audio"
    } else if mime == "application/pdf" || e == "pdf" {
        "pdf"
    } else if mime.starts_with("text/")
        || mime == "application/json"
        || mime.ends_with("+xml")
        || mime == "application/javascript"
        || mime == "inode/x-empty"
        || has(&["md", "txt", "json", "yaml", "yml", "toml", "ini", "conf", "lua", "qml", "js", "ts", "py", "sh", "c", "cpp", "h", "rs", "go", "css", "html", "log", "csv"])
    {
        "text"
    } else if mime.is_empty() {
        ""
    } else {
        "other"
    }
}

/// A size as the shell writes it (Transfers.size: 1000s, one decimal under 10)
pub fn size_text(bytes: u64) -> String {
    let u = ["B", "KB", "MB", "GB", "TB"];
    let (mut i, mut v) = (0, bytes as f64);
    while v >= 1000.0 && i < u.len() - 1 {
        v /= 1000.0;
        i += 1;
    }
    if i == 0 {
        format!("{} {}", v as u64, u[i])
    } else if v < 10.0 {
        format!("{v:.1} {}", u[i])
    } else {
        format!("{v:.0} {}", u[i])
    }
}

/// The file icon's theme name (Search.iconForFile)
pub fn icon_name(path: &str, dir: bool) -> &'static str {
    if dir {
        return "folder";
    }
    let e = path.rsplit('.').next().unwrap_or("").to_lowercase();
    match e.as_str() {
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "svg" => "image-x-generic",
        "mp4" | "mkv" | "webm" | "mov" => "video-x-generic",
        "mp3" | "flac" | "ogg" | "wav" | "m4a" => "audio-x-generic",
        "pdf" => "application-pdf",
        "zip" | "tar" | "gz" | "xz" | "7z" | "rar" => "package-x-generic",
        _ => "text-x-generic",
    }
}

/// A folder's entries, folders first, then by name (no hidden ones)
pub fn entries(dir: &str) -> Vec<(String, bool)> {
    let mut v: Vec<(String, bool)> = std::fs::read_dir(dir)
        .map(|rd| rd.flatten().map(|e| (e.file_name().to_string_lossy().into_owned(), e.path().is_dir())).filter(|(n, _)| !n.starts_with('.')).collect())
        .unwrap_or_default();
    v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase())));
    v
}

/// Material's wavy progress, 0..1000 wide (`value` of it drawn)
pub fn wave(value: f32) -> String {
    let end = (value.clamp(0.0, 1.0) * 1000.0).max(1.0);
    let mut d = String::from("M 0 6");
    let mut x = 0.0f32;
    while x < end {
        x = (x + 4.0).min(end);
        d += &format!(" L {x:.1} {:.2}", 6.0 + 3.0 * (x / 40.0 * std::f32::consts::TAU).sin());
    }
    d
}

type Px = (u32, u32, Vec<u8>);

enum Q {
    Probed(String, String, u64, String),
    Picture(String, Option<(Px, f32, f32)>),
    Text(String, String),
    Folder(String, Vec<(String, Option<Px>)>),
    Icon(String, Option<Px>),
    Media(String, f32),
    Frame,
    Ended(String),
    Key(String, bool),
    Close,
    OpenIt,
    Share,
    ToggleText,
    TogglePlay,
    Closed,
    LtChip(i32),
    LtCopySel,
    LtCopyAll,
    LtRetry,
    SaidGone,
}

fn send(tx: &Sender<Msg>, q: Q) {
    let _ = tx.send(Msg::Part(NAME, Box::new(q)));
}

fn px_of(img: image::RgbaImage) -> Px {
    let (w, h) = img.dimensions();
    (w, h, img.into_raw())
}

fn image_of(p: &Px) -> slint::Image {
    slint::Image::from_rgba8(SharedPixelBuffer::clone_from_slice(&p.2, p.0, p.1))
}

/// A themed icon at `size` px
fn icon_px(name: &str, size: u32) -> Option<Px> {
    let path = crate::icon_theme::find(name, None).or_else(|| crate::icon_theme::find("text-x-generic", None))?;
    let img = crate::appicon::load(&path, size)?;
    let data: Vec<u8> = img.px.iter().flat_map(|p| p.iter().map(|c| (c * 255.0).round().clamp(0.0, 255.0) as u8).collect::<Vec<u8>>()).collect();
    Some((img.w as u32, img.h as u32, data))
}

/// Playing video or audio (ffmpeg's frames, ffplay's sound)
#[derive(Default)]
struct Player {
    /// (each with whether it was waited for: only one not yet waited for is
    /// signalled, its PID can't be anyone else's)
    kids: Vec<(Child, bool)>,
    playing: bool,
    paused_at: Option<Instant>,
    started: Option<Instant>,
    paused_total: Duration,
    duration: f32,
    ended: bool,
}

impl Player {
    fn signal(&mut self, sig: i32) {
        for (k, reaped) in &mut self.kids {
            if !*reaped && matches!(k.try_wait(), Ok(None)) {
                unsafe {
                    libc::kill(k.id() as i32, sig);
                }
            } else {
                *reaped = true;
            }
        }
    }
    /// Every process it started has ended
    fn finished(&mut self) -> bool {
        !self.kids.is_empty()
            && self.kids.iter_mut().all(|(k, reaped)| {
                if !*reaped && !matches!(k.try_wait(), Ok(None)) {
                    *reaped = true;
                }
                *reaped
            })
    }
    fn stop(&mut self) {
        self.signal(libc::SIGCONT);
        for (k, reaped) in &mut self.kids {
            if !*reaped {
                let _ = k.kill();
                let _ = k.wait();
            }
        }
        self.kids.clear();
        self.playing = false;
        self.started = None;
        self.paused_at = None;
        self.paused_total = Duration::ZERO;
    }
    fn position(&self) -> f32 {
        if self.ended {
            return 1.0;
        }
        let Some(s) = self.started else { return 0.0 };
        let now = self.paused_at.unwrap_or_else(Instant::now);
        let t = now.duration_since(s).saturating_sub(self.paused_total).as_secs_f32();
        if self.duration > 0.0 { (t / self.duration).min(1.0) } else { 0.0 }
    }
}

struct Win {
    output: String,
    surface: Rc<Surface>,
    ui: ui::QuickLook,
    a11y: Option<crate::a11y::A11y>,
    glass_sent: String,
    input_on: Option<bool>,
}

pub struct QuickLook {
    win: Option<Win>,
    open: bool,
    path: String,
    kind: &'static str,
    mime: String,
    ext: String,
    size: u64,
    modified: String,
    count: usize,
    picture_ready: bool,
    live: Rc<RefCell<Live>>,
    query: String,
    text_mode: bool,
    ocr: Rc<RefCell<Ocr>>,
    player: Player,
    frame: Arc<Mutex<Option<Px>>>,
    tick: slint::Timer,
    glow: slint::Timer,
    said: slint::Timer,
    tx: Option<Sender<Msg>>,
}

impl Default for QuickLook {
    fn default() -> Self {
        QuickLook {
            win: None,
            open: false,
            path: String::new(),
            kind: "",
            mime: String::new(),
            ext: String::new(),
            size: 0,
            modified: String::new(),
            count: 0,
            picture_ready: false,
            live: Rc::new(RefCell::new(Live::new())),
            query: String::new(),
            text_mode: false,
            ocr: Rc::new(RefCell::new(Ocr::default())),
            player: Player::default(),
            frame: Arc::new(Mutex::new(None)),
            tick: slint::Timer::default(),
            glow: slint::Timer::default(),
            said: slint::Timer::default(),
            tx: None,
        }
    }
}

impl QuickLook {
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
        let ui = ui::QuickLook::new().expect("QuickLook");
        st.shell.apply_theme(&ui.global::<ui::Theme>());
        let tx = st.tx.clone();
        macro_rules! on {
            ($set:ident, |$($a:ident),*| $m:expr) => {{
                let tx = tx.clone();
                ui.$set(move |$($a),*| send(&tx, $m));
            }};
        }
        on!(on_close_it, | | Q::Close);
        on!(on_open_it, | | Q::OpenIt);
        on!(on_share, | | Q::Share);
        on!(on_toggle_text, | | Q::ToggleText);
        on!(on_toggle_play, | | Q::TogglePlay);
        on!(on_closed, | | Q::Closed);
        on!(on_lt_chip, |i| Q::LtChip(i));
        on!(on_lt_copy_sel, | | Q::LtCopySel);
        on!(on_lt_copy_all, | | Q::LtCopyAll);
        on!(on_lt_retry, | | Q::LtRetry);
        ui.on_lt_install(|| crate::shell::ask("livetext.install", json!([])));
        {
            let (tx, weak, live) = (tx.clone(), ui.as_weak(), self.live.clone());
            ui.on_key(move |text, ctrl| {
                let Some(ui) = weak.upgrade() else { return false };
                let is = |k: Key| text == SharedString::from(k);
                let image = ui.get_kind() == "image";
                let ready = ui.get_text_mode() && live.borrow().status == "ready";
                let ours = (image && !ctrl && text.eq_ignore_ascii_case("t"))
                    || (ready && ctrl && (text.eq_ignore_ascii_case("c") || text.eq_ignore_ascii_case("a")))
                    || is(Key::Escape)
                    || text == " "
                    || is(Key::Return);
                if ours {
                    send(&tx, Q::Key(text.to_string(), ctrl));
                }
                ours
            });
        }
        {
            let (live, weak, ocr) = (self.live.clone(), ui.as_weak(), self.ocr.clone());
            ui.on_lt_press(move |x, y, s| {
                let Some(ui) = weak.upgrade() else { return };
                let (sx, ox, oy) = map(&live.borrow(), &ui);
                live.borrow_mut().press(x / sx + ox, y / sx + oy, s, sx);
                push_live(&live.borrow(), &ocr.borrow(), &ui);
            });
        }
        {
            let (live, weak, ocr) = (self.live.clone(), ui.as_weak(), self.ocr.clone());
            ui.on_lt_drag(move |x, y| {
                let Some(ui) = weak.upgrade() else { return };
                let (sx, ox, oy) = map(&live.borrow(), &ui);
                live.borrow_mut().drag(x / sx + ox, y / sx + oy, sx, sx);
                push_live(&live.borrow(), &ocr.borrow(), &ui);
            });
        }
        {
            let (live, weak) = (self.live.clone(), ui.as_weak());
            ui.on_lt_hover(move |x, y| {
                let Some(ui) = weak.upgrade() else { return };
                let (sx, ox, oy) = map(&live.borrow(), &ui);
                ui.set_over_word(live.borrow().over_word(x / sx + ox, y / sx + oy, sx));
            });
        }
        {
            let (live, weak, ocr) = (self.live.clone(), ui.as_weak(), self.ocr.clone());
            ui.on_lt_double(move |x, y| {
                let Some(ui) = weak.upgrade() else { return };
                let (sx, ox, oy) = map(&live.borrow(), &ui);
                live.borrow_mut().double(x / sx + ox, y / sx + oy, sx);
                push_live(&live.borrow(), &ocr.borrow(), &ui);
            });
        }
        let m = st.shell.look.material;
        ui.set_ic_text(ic("text-aa", m));
        ui.set_ic_copy(ic("copy", m));
        ui.set_ic_download(ic("download-simple", m));
        ui.set_ic_play(ic("play", m));
        ui.set_ic_pause(ic("pause", m));
        ui.set_no_icon(ui::StIc::default());
        ui.set_glyph_x(crate::icons::glyph("x").into());
        ui.set_glyph_share(crate::icons::glyph("share-network").into());
        ui.show().expect("show quick look");
        let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
        self.win = Some(Win { output: output.into(), surface, ui, a11y, glass_sent: String::new(), input_on: None });
        true
    }

    fn open(&mut self, st: &mut State, path: &str, query: &str, screen: Option<String>) {
        let Some(output) = screen.filter(|s| st.output_named(s).is_some()).or_else(|| st.outputs_now().first().map(|o| o.1.clone())) else { return };
        if !self.window(st, &output) {
            return;
        }
        self.tx = Some(st.tx.clone());
        self.player.stop();
        self.open = true;
        self.path = path.to_string();
        self.query = query.to_string();
        self.text_mode = !query.is_empty();
        let name = path.rsplit('/').next().unwrap_or(path).to_string();
        self.ext = if name.contains('.') { name.rsplit('.').next().unwrap_or("").to_lowercase() } else { String::new() };
        self.mime.clear();
        self.size = 0;
        self.modified.clear();
        self.count = 0;
        self.picture_ready = false;
        self.kind = kind_of("", &self.ext);
        {
            let mut l = self.live.borrow_mut();
            *l = Live::new();
            l.highlight = query.to_string();
        }
        let w = self.win.as_ref().unwrap();
        w.ui.set_file_name(name.into());
        w.ui.set_picture(slint::Image::default());
        w.ui.set_text_body("".into());
        w.ui.set_rows(ModelRc::new(VecModel::from(Vec::<ui::QlRow>::new())));
        w.ui.set_file_icon(slint::Image::default());
        w.ui.set_playing(false);
        w.ui.set_position(0.0);
        w.ui.set_shape(crate::surfaces::shapes::outline("cookie12").into());
        w.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        w.surface.show(0, 0);
        w.ui.set_shown(true);
        w.ui.invoke_focus_keys();
        self.push();
        // What it is: from the file's own type, and its size and date
        let (tx, p) = (st.tx.clone(), path.to_string());
        std::thread::spawn(move || {
            let mime = if std::path::Path::new(&p).is_dir() {
                "inode/directory".to_string()
            } else {
                std::process::Command::new("file").args(["-b", "--mime-type", "--", &p]).output().ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default()
            };
            let meta = std::fs::metadata(&p).ok();
            let size = meta.as_ref().map_or(0, |m| m.len());
            let modified = meta.and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| crate::util::strftime("%Y-%m-%d %H:%M", d.as_secs() as i64)).unwrap_or_default();
            send(&tx, Q::Probed(p, mime, size, modified));
        });
    }

    fn close(&mut self, tell: bool) {
        if !self.open {
            return;
        }
        self.open = false;
        self.player.stop();
        self.tick.stop();
        if let Some(w) = &self.win {
            w.ui.set_shown(false);
            w.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::None);
            w.surface.layer.commit();
        }
        if tell {
            crate::shell::ask("panels.quickLook", json!([""]));
        }
    }

    /// Everything about the file is known: load its preview
    fn load(&mut self, st: &State) {
        let Some(tx) = self.tx.clone() else { return };
        let p = self.path.clone();
        let (sw, sh) = self.win.as_ref().and_then(|w| st.output_named(&w.output)).and_then(|o| st.outputs.info(&o)).and_then(|i| i.logical_size).map_or((1920.0, 1080.0), |s| (s.0 as f32, s.1 as f32));
        match self.kind {
            "image" => {
                let (bw, bh) = ((sw * 0.8 * 1.5) as u32, (sh * 0.8 * 1.5) as u32);
                std::thread::spawn(move || {
                    let r = image::open(&p).ok().map(|i| {
                        let (w, h) = (i.width() as f32, i.height() as f32);
                        let t = if i.width() > bw || i.height() > bh { i.thumbnail(bw, bh) } else { i };
                        (px_of(t.to_rgba8()), w, h)
                    });
                    send(&tx, Q::Picture(p, r));
                });
            }
            "pdf" => {
                std::thread::spawn(move || {
                    // (the first page, rendered once into the cache, as the QML's)
                    let out = std::process::Command::new("sh")
                        .args(["-c", "command -v pdftoppm >/dev/null || exit 1; d=\"${XDG_CACHE_HOME:-$HOME/.cache}/tessera/quicklook\"; mkdir -p \"$d\"; o=\"$d/$(printf '%s' \"$1\" | md5sum | cut -c1-16)\"; [ -f \"$o.png\" ] || pdftoppm -png -r 110 -f 1 -l 1 -singlefile -- \"$1\" \"$o\" 2>/dev/null; echo \"$o.png\"", "sh", &p])
                        .output()
                        .ok()
                        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                        .unwrap_or_default();
                    let r = image::open(&out).ok().map(|i| {
                        let (w, h) = (i.width() as f32, i.height() as f32);
                        (px_of(i.to_rgba8()), w, h)
                    });
                    send(&tx, Q::Picture(p, r));
                });
            }
            "text" => {
                std::thread::spawn(move || {
                    let mut buf = Vec::new();
                    if let Ok(f) = std::fs::File::open(&p) {
                        use std::io::Read;
                        let _ = f.take(200_000).read_to_end(&mut buf);
                    }
                    send(&tx, Q::Text(p, String::from_utf8_lossy(&buf).into_owned()));
                });
            }
            "folder" => {
                std::thread::spawn(move || {
                    let rows = entries(&p).into_iter().map(|(n, dir)| {
                        let icon = icon_px(icon_name(&n, dir), 40);
                        (n, icon)
                    }).collect();
                    send(&tx, Q::Folder(p, rows));
                });
            }
            "video" | "audio" => {
                let (p2, tx2) = (p.clone(), tx.clone());
                std::thread::spawn(move || {
                    let icon = icon_px(icon_name(&p2, false), 96);
                    send(&tx2, Q::Icon(p2, icon));
                });
                std::thread::spawn(move || {
                    let d = std::process::Command::new("ffprobe")
                        .args(["-v", "error", "-show_entries", "format=duration", "-of", "default=nw=1:nk=1", "--", &p])
                        .output()
                        .ok()
                        .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse::<f32>().ok())
                        .unwrap_or(0.0);
                    send(&tx, Q::Media(p, d));
                });
            }
            _ => {
                std::thread::spawn(move || {
                    let icon = icon_px(icon_name(&p, false), 96);
                    send(&tx, Q::Icon(p, icon));
                });
            }
        }
    }

    /// Play from the start (video: frames into the card, sound through ffplay)
    fn play(&mut self) {
        self.player.stop();
        self.player.ended = false;
        let p = self.path.clone();
        if self.kind == "video" {
            let slot = self.frame.clone();
            let Some(tx) = self.tx.clone() else { return };
            let probe = std::process::Command::new("ffprobe")
                .args(["-v", "error", "-select_streams", "v:0", "-show_entries", "stream=width,height", "-of", "csv=p=0", "--", &p])
                .output()
                .ok()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_default();
            let mut it = probe.split(',').filter_map(|x| x.trim().parse::<u32>().ok());
            let (vw, vh) = (it.next().unwrap_or(1280), it.next().unwrap_or(720));
            let k = (1280.0 / vw as f32).min(720.0 / vh as f32).min(1.0);
            let (w, h) = (((vw as f32 * k) as u32 / 2 * 2).max(2), ((vh as f32 * k) as u32 / 2 * 2).max(2));
            let child = std::process::Command::new("ffmpeg")
                .args(["-loglevel", "quiet", "-re", "-i", &p, "-an", "-vf", &format!("scale={w}:{h}"), "-f", "rawvideo", "-pix_fmt", "rgba", "pipe:1"])
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn();
            if let Ok(mut c) = child {
                if let Some(mut out) = c.stdout.take() {
                    let path = p.clone();
                    std::thread::spawn(move || {
                        use std::io::Read;
                        let n = (w * h * 4) as usize;
                        loop {
                            let mut buf = vec![0u8; n];
                            if out.read_exact(&mut buf).is_err() {
                                break;
                            }
                            let was_empty = {
                                let mut s = slot.lock().unwrap();
                                let e = s.is_none();
                                *s = Some((w, h, buf));
                                e
                            };
                            if was_empty {
                                send(&tx, Q::Frame);
                            }
                        }
                        send(&tx, Q::Ended(path));
                    });
                }
                self.player.kids.push((c, false));
            }
            if let Ok(c) = std::process::Command::new("ffplay").args(["-nodisp", "-autoexit", "-vn", "-loglevel", "quiet", "--", &p]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
                self.player.kids.push((c, false));
            }
        } else {
            // (its end: the clock's tick sees the process gone)
            if let Ok(c) = std::process::Command::new("ffplay").args(["-nodisp", "-autoexit", "-loglevel", "quiet", "--", &p]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
                self.player.kids.push((c, false));
            }
        }
        self.player.playing = true;
        self.player.started = Some(Instant::now());
        self.start_tick();
    }

    fn start_tick(&mut self) {
        let tx = self.tx.clone();
        self.tick.start(slint::TimerMode::Repeated, Duration::from_millis(100), move || {
            if let Some(tx) = &tx {
                send(tx, Q::Frame);
            }
        });
    }

    fn toggle_play(&mut self) {
        if self.player.kids.is_empty() || self.player.ended {
            self.play();
        } else if self.player.playing {
            self.player.signal(libc::SIGSTOP);
            self.player.paused_at = Some(Instant::now());
            self.player.playing = false;
        } else {
            self.player.signal(libc::SIGCONT);
            if let Some(p) = self.player.paused_at.take() {
                self.player.paused_total += p.elapsed();
            }
            self.player.playing = true;
        }
        self.push();
    }

    fn live_on(&mut self) {
        let active = self.text_mode && self.kind == "image" && self.picture_ready;
        let start = {
            let mut l = self.live.borrow_mut();
            if l.active == active && l.path == self.path {
                None
            } else {
                l.active = active;
                l.path = self.path.clone();
                l.rev = String::new();
                l.start()
            }
        };
        if let Some(args) = start {
            crate::shell::ask("livetext.read", args);
        }
        if let Some(w) = &self.win {
            push_live(&self.live.borrow(), &self.ocr.borrow(), &w.ui);
        }
    }

    fn say(&mut self, what: &str) {
        if what.is_empty() {
            return;
        }
        self.live.borrow_mut().said = what.to_string();
        if let Some(w) = &self.win {
            push_live(&self.live.borrow(), &self.ocr.borrow(), &w.ui);
        }
        let tx = self.tx.clone();
        self.said.start(slint::TimerMode::SingleShot, Duration::from_millis(2000), move || {
            if let Some(tx) = &tx {
                send(tx, Q::SaidGone);
            }
        });
    }

    fn copy_text(&mut self, text: String, what: &str) {
        if !text.is_empty() {
            crate::util::pipe_to(&["wl-copy", "--type", "text/plain"], text.into_bytes());
            self.say(what);
        }
    }

    fn push(&self) {
        let Some(w) = &self.win else { return };
        let ui = &w.ui;
        ui.set_kind(self.kind.into());
        let first = if self.kind == "folder" { format!("{} {}", self.count, if self.count == 1 { "item" } else { "items" }) } else { size_text(self.size) };
        let parts: Vec<String> = [first, self.modified.clone()].into_iter().filter(|s| !s.is_empty()).collect();
        ui.set_subtitle(parts.join(" · ").into());
        ui.set_picture_ready(self.picture_ready);
        ui.set_text_mode(self.text_mode);
        ui.set_playing(self.player.playing);
        let pos = self.player.position();
        ui.set_position(pos);
        ui.set_wave(wave(pos).into());
        // (Material: a circle while it plays, a rounded square while paused)
        ui.set_play_shape(if self.player.playing { crate::surfaces::shapes::outline("circle") } else { ROUNDED_SQUARE.to_string() }.into());
        ui.set_shape(crate::surfaces::shapes::outline(if self.kind == "audio" { "sunny" } else { "cookie12" }).into());
        push_live(&self.live.borrow(), &self.ocr.borrow(), ui);
    }

    fn key(&mut self, text: &str, ctrl: bool) {
        let is = |k: Key| text == SharedString::from(k).as_str();
        if self.kind == "image" && !ctrl && text.eq_ignore_ascii_case("t") {
            self.text_mode = !self.text_mode;
            self.live_on();
            self.push();
            return;
        }
        let ready = self.text_mode && self.live.borrow().status == "ready";
        if ready {
            if ctrl && text.eq_ignore_ascii_case("c") {
                let (sel, t) = {
                    let l = self.live.borrow();
                    (l.has_selection(), if l.has_selection() { l.selected_text() } else { l.all_text() })
                };
                self.copy_text(t, if sel { "Text copied" } else { "All text copied" });
                return;
            }
            if ctrl && text.eq_ignore_ascii_case("a") {
                self.live.borrow_mut().select_all();
                self.push();
                return;
            }
            if is(Key::Escape) && self.live.borrow().has_selection() {
                self.live.borrow_mut().clear();
                self.push();
                return;
            }
        }
        if text == " " || is(Key::Escape) {
            self.close(true);
        } else if is(Key::Return) {
            crate::shell::ask("files.open", json!([self.path]));
            self.close(true);
        }
    }
}

/// The picture's pixels per display px, and the part shown's origin
fn map(live: &Live, ui: &ui::QuickLook) -> (f32, f32, f32) {
    let (shown, sx, _) = livetext::mapping(live, (0.0, 0.0, 0.0, 0.0), ui.get_shown_w(), ui.get_shown_h());
    (sx.max(0.0001), shown.0, shown.1)
}

fn push_live(live: &Live, ocr: &Ocr, ui: &ui::QuickLook) {
    ui.set_boxes(ltui::boxes(live, (0.0, 0.0, 0.0, 0.0), ui.get_shown_w(), ui.get_shown_h()));
    ui.set_lt(ltui::bar(live, ocr, ui.global::<ui::Theme>().get_material(), ui.get_card_w() - 24.0));
}

impl Part for QuickLook {
    fn name(&self) -> &'static str {
        NAME
    }

    fn command(&mut self, st: &mut State, cmd: &Command) {
        match cmd.cmd.as_str() {
            "open" => {
                let path = cmd.args["path"].as_str().unwrap_or("").to_string();
                if path.is_empty() {
                    self.close(false);
                } else {
                    let q = cmd.args["query"].as_str().unwrap_or("").to_string();
                    self.open(st, &path, &q, cmd.screen.clone());
                }
            }
            "close" => self.close(false),
            _ => {}
        }
    }

    fn config_changed(&mut self, st: &mut State, look: bool) {
        if look {
            if let Some(w) = &mut self.win {
                st.shell.apply_theme(&w.ui.global::<ui::Theme>());
                w.glass_sent.clear();
                let m = st.shell.look.material;
                w.ui.set_ic_text(ic("text-aa", m));
                w.ui.set_ic_copy(ic("copy", m));
                w.ui.set_ic_play(ic("play", m));
                w.ui.set_ic_pause(ic("pause", m));
            }
        }
    }

    fn plugin_changed(&mut self, st: &mut State) {
        self.config_changed(st, true);
    }

    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        let msg = match msg.downcast::<Value>() {
            Ok(v) => {
                // The shell's Live Text answers and state (RustBridge.qml)
                if let Some(k) = v["livetext"].as_str() {
                    let done = self.live.borrow_mut().done(k, &v["result"]);
                    if done {
                        if self.live.borrow().found {
                            self.live.borrow_mut().found = false;
                            let (weak, t0) = (self.win.as_ref().map(|w| w.ui.as_weak()), Instant::now());
                            self.glow.start(slint::TimerMode::Repeated, Duration::from_millis(16), move || {
                                if let Some(ui) = weak.as_ref().and_then(|w| w.upgrade()) {
                                    ui.set_glow(ltui::glow_at(t0.elapsed().as_secs_f32() * 1000.0));
                                }
                            });
                        }
                        self.push();
                    }
                } else if let Some(k) = v["wifi"].as_str() {
                    let same = { let l = self.live.borrow(); k == l.key() && l.active };
                    if same {
                        self.live.borrow_mut().wifi = v["found"].as_array().cloned().unwrap_or_default();
                        self.push();
                    }
                } else if let Some(s) = v.get("state") {
                    *self.ocr.borrow_mut() = Ocr::from_json(s);
                }
                return;
            }
            Err(m) => m,
        };
        let Ok(q) = msg.downcast::<Q>() else { return };
        match *q {
            Q::Probed(p, mime, size, modified) => {
                if p != self.path || !self.open {
                    return;
                }
                self.mime = mime;
                self.size = size;
                self.modified = modified;
                self.kind = kind_of(&self.mime, &self.ext);
                if self.kind == "folder" {
                    self.count = entries(&self.path).len();
                }
                self.push();
                self.load(st);
            }
            Q::Picture(p, r) => {
                if p != self.path {
                    return;
                }
                if let (Some(w), Some((px, nw, nh))) = (&self.win, r) {
                    w.ui.set_picture(image_of(&px));
                    w.ui.set_natural_w(nw);
                    w.ui.set_natural_h(nh);
                    self.picture_ready = true;
                }
                self.live_on();
                self.push();
            }
            Q::Text(p, t) => {
                if let (true, Some(w)) = (p == self.path, &self.win) {
                    w.ui.set_text_body(t.into());
                }
            }
            Q::Folder(p, rows) => {
                if let (true, Some(w)) = (p == self.path, &self.win) {
                    self.count = rows.len();
                    let v: Vec<ui::QlRow> = rows.iter().map(|(n, i)| ui::QlRow { icon: i.as_ref().map(image_of).unwrap_or_default(), name: n.as_str().into() }).collect();
                    w.ui.set_rows(ModelRc::new(VecModel::from(v)));
                    self.push();
                }
            }
            Q::Icon(p, i) => {
                if let (true, Some(w), Some(i)) = (p == self.path, &self.win, i) {
                    w.ui.set_file_icon(image_of(&i));
                }
            }
            Q::Media(p, d) => {
                if p == self.path && self.open {
                    self.player.duration = d;
                    self.play(); // (it plays as it opens, as the QML's)
                    self.push();
                }
            }
            Q::Frame => {
                if self.kind == "audio" && self.player.playing && self.player.finished() {
                    let p = self.path.clone();
                    let tx = self.tx.clone();
                    if let Some(tx) = tx {
                        send(&tx, Q::Ended(p));
                    }
                }
                if let Some(px) = self.frame.lock().unwrap().take() {
                    if let Some(w) = &self.win {
                        w.ui.set_natural_w(px.0 as f32);
                        w.ui.set_natural_h(px.1 as f32);
                        w.ui.set_picture(image_of(&px));
                    }
                    self.picture_ready = true;
                }
                if let Some(w) = &self.win {
                    let pos = self.player.position();
                    w.ui.set_position(pos);
                    w.ui.set_picture_ready(self.picture_ready);
                    w.ui.set_wave(wave(pos).into());
                }
            }
            Q::Ended(p) => {
                if p == self.path && self.player.playing {
                    self.player.stop();
                    self.player.ended = true;
                    self.tick.stop();
                    self.push();
                }
            }
            Q::Key(t, ctrl) => self.key(&t, ctrl),
            Q::Close => self.close(true),
            Q::OpenIt => {
                crate::shell::ask("files.open", json!([self.path]));
                self.close(true);
            }
            Q::Share => {
                let p = self.path.clone();
                self.close(true);
                crate::shell::ask("panels.share", json!([[p]]));
            }
            Q::ToggleText => {
                self.text_mode = !self.text_mode;
                self.live_on();
                self.push();
            }
            Q::TogglePlay => self.toggle_play(),
            Q::Closed => {
                if !self.open {
                    if let Some(w) = &mut self.win {
                        w.surface.hide();
                        st.shell.glass.set(&w.output, NS, &[]);
                        w.glass_sent.clear();
                        w.input_on = None;
                    }
                }
            }
            Q::LtChip(i) => {
                let a = self.live.borrow().actions().get(i.max(0) as usize).cloned();
                if let Some(a) = a {
                    crate::shell::ask("livetext.act", json!([livetext::act_json(&a)]));
                    let said = livetext::act_says(&a, &self.ocr.borrow());
                    self.say(said);
                }
            }
            Q::LtCopySel => {
                let t = self.live.borrow().selected_text();
                self.copy_text(t, "Text copied");
            }
            Q::LtCopyAll => {
                let t = self.live.borrow().all_text();
                self.copy_text(t, "All text copied");
            }
            Q::LtRetry => {
                let args = self.live.borrow_mut().start();
                if let Some(a) = args {
                    crate::shell::ask("livetext.read", a);
                }
                self.push();
            }
            Q::SaidGone => {
                self.live.borrow_mut().said.clear();
                self.push();
            }
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

    /// The card's glass (Glass), where it takes the pointer, the accessibility tree
    fn after_frame(&mut self, st: &mut State) {
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
        // (input only while shown: during the closing fade clicks go through)
        if w.input_on != Some(open) {
            st.set_input_region(&w.surface, if open { None } else { Some(&[]) });
            w.input_on = Some(open);
        }
        if !st.shell.plugin || st.shell.look.material {
            return;
        }
        let ui = &w.ui;
        let (p, s) = (ui.get_progress(), ui.get_card_scale());
        let (cw, ch) = (ui.get_card_w(), ui.get_card_h());
        let (x, y) = (ui.get_card_x() + cw * (1.0 - s) / 2.0, ui.get_card_y() + ch * (1.0 - s) / 2.0);
        let shapes = if p > 0.002 { vec![theme::glass_shape(&st.shell.look, x, y, cw * s, ch * s, 20.0 * s, p, 1.0)] } else { Vec::new() };
        let key = format!("{shapes:?}");
        if key != w.glass_sent {
            st.shell.glass.set(&w.output, NS, &shapes);
            w.glass_sent = key;
        }
    }

    fn a11y(&mut self) -> Vec<&mut crate::a11y::A11y> {
        self.win.as_mut().and_then(|w| w.a11y.as_mut()).into_iter().collect()
    }

    fn quit(&mut self, _st: &mut State) {
        self.player.stop();
        if let Some(w) = &self.win {
            Glass::clear_now(&w.output, NS);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_a_file_is() {
        assert_eq!(kind_of("inode/directory", ""), "folder");
        assert_eq!(kind_of("image/png", ""), "image");
        assert_eq!(kind_of("", "JPG"), "image");
        assert_eq!(kind_of("video/mp4", "mp4"), "video");
        assert_eq!(kind_of("audio/flac", ""), "audio");
        assert_eq!(kind_of("application/pdf", ""), "pdf");
        assert_eq!(kind_of("application/json", ""), "text");
        assert_eq!(kind_of("image/svg+xml", "svg"), "image");
        assert_eq!(kind_of("application/x-sharedlib", "so"), "other");
        assert_eq!(kind_of("", "so"), "");
        assert_eq!(kind_of("", "rs"), "text");
    }

    #[test]
    fn sizes_as_the_shell_writes_them() {
        assert_eq!(size_text(0), "0 B");
        assert_eq!(size_text(999), "999 B");
        assert_eq!(size_text(1500), "1.5 KB");
        assert_eq!(size_text(25_000_000), "25 MB");
    }

    #[test]
    fn folders_list_folders_first() {
        let d = std::env::temp_dir().join(format!("tessera-ql-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("zeta")).unwrap();
        std::fs::write(d.join("a.txt"), b"x").unwrap();
        std::fs::write(d.join(".hidden"), b"x").unwrap();
        std::fs::write(d.join("B.png"), b"x").unwrap();
        let e = entries(d.to_str().unwrap());
        assert_eq!(e, vec![("zeta".into(), true), ("a.txt".into(), false), ("B.png".into(), false)]);
        assert_eq!(icon_name("B.png", false), "image-x-generic");
        let _ = std::fs::remove_dir_all(&d);
        assert!(wave(0.5).starts_with("M 0 6 L 4.0"));
    }
}

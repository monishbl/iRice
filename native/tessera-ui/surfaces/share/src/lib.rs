//! Sharing (rust.surfaces: share), both looks: the share sheet, the card
//! that asks when someone nearby shares something, and the QR code card.
//! Compare shell/modules/share/ShareSheet.qml, IncomingCard.qml,
//! LinkShareCard.qml and ProgressRing.qml (and shell/glass/Glass.qml,
//! shell/common/WavyProgress.qml, FocusRing.qml, ElasticControl.qml).
//!
//!   the sheet   a panel ("share"), opened through Panels as the QML one
//!               (Panels.share(paths) from Files, the shelf, Spotlight…):
//!               RustUi.qml forwards the open and close (src/shell.rs).
//!               What's shared at the top, nearby devices as round avatars
//!               in a row (a ring while sending; Material: the device on
//!               its shape and a wavy bar), how each send ended, the
//!               looking state, then Copy / Show in Files / Share with a
//!               QR code. Esc, a click outside or the × closes it; Tab and
//!               the arrows move between the ×, the devices and the rows,
//!               Space or Enter does it.
//!   the card    while the feed has a request (Sharing.request): centred
//!               on the screen you're using, who, what, the PIN, Decline /
//!               Accept, then a ring (Material: a wavy bar) while it
//!               arrives, then Open / Show in Files. Enter accepts, Esc
//!               declines (or puts it away).
//!   QR card     while LinkShare shares (an iPhone, or any phone's
//!               browser): the code, what to do, how it's going, Done.
//!
//! The transfers stay the shell's (Sharing, LocalSend, QuickShare, Cast,
//! LinkShare: helpers this never runs or talks to). shell/modules/share/
//! ShareFeed.qml sends their state on stdin as `{"to":"share","feed":{…}}`
//! (`Feed` below, with the display strings the QML computes, as sizes), and
//! does what this asks with `shell::ask("share", [what, …])`: "send"
//! (a device's key), "copy", "reveal", "qr" (the other ways), "answer" (id,
//! accept), "cancel", "hide", "drop", "open", "show" (a request's id),
//! "linkClose", "installQr". The 9 s that a finished request stays is the
//! QML's Timer (IncomingCard.qml), as is Sharing.look while the sheet is open
//! (the feed follows Panels).
//!
//! Differences from the QML: Material's shapes switch outline without the
//! morph (Slint has no shaders), and the wavy bar is a row of dots on the
//! wave; Squircle corners are circular; the device names' line height is
//! Slint's (QML: 0.95); numbers aren't tabular (Slint sets no font
//! features); the QR card's title elides at the end (QML: the middle); the
//! sheet's keyboard focus is its own index (Tab, Shift+Tab, Left, Right),
//! not Qt's focus chain, so a click doesn't move it; Material's press swell
//! is a spring scale (no rubber band); Material draws its own tonal
//! surfaces, no glass (as the other Rust panels).

pub mod ui {
    slint::include_modules!();
}
tessera_core::theme_global!(ui);

// tessera-core's modules (shell, wayland, theme, services, util…) under the
// names they had when everything was one crate: `crate::shell::Part`
#[allow(unused_imports)]
use tessera_core::*;

use tessera_core::services::hypr::Glass;
use tessera_core::shell::{Command, Part};
use tessera_core::theme;
use tessera_core::wayland::{Spec, State, Surface};
use serde::Deserialize;
use serde_json::{json, Value};
use slint::platform::Key;
use slint::{ComponentHandle, Model, ModelRc, Rgba8Pixel, SharedPixelBuffer, SharedString, VecModel};
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use smithay_client_toolkit::shell::WaylandSurface;
use std::any::Any;
use std::collections::HashMap;
use std::rc::Rc;

const NAME: &str = "share";
const NS_SHEET: &str = "tessera:share";
const NS_CARD: &str = "tessera:share-incoming";
const NS_LINK: &str = "tessera:share-link";
/// Room around a card for the glass's shadow (Glass.qml's shadowMargin)
const MARGIN: f32 = 24.0;
/// A device in the row: 92 wide, 4 apart
const DEV_W: f32 = 92.0;
const DEV_STEP: f32 = 96.0;

// ── What the shell sends (ShareFeed.qml) ────────────────────────────────

#[derive(Deserialize, Default, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Feed {
    pub sheet: Sheet,
    pub request: Option<Request>,
    pub link: Option<Link>,
    /// the screen you're using (Hyprland's focused monitor)
    pub screen: String,
    /// CoolDown.motionScale (shorter animations)
    pub motion: Option<f32>,
}

#[derive(Deserialize, Default, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Sheet {
    pub paths: Vec<String>,
    pub note: String,
    /// the icon for what's shared (IconLookup.path: "file://…")
    pub icon: String,
    pub devices: Vec<Device>,
    pub available: bool,
    #[serde(rename = "portsError")]
    pub ports_error: String,
    pub errors: String,
}

#[derive(Deserialize, Default, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Device {
    pub key: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub cast: bool,
    #[serde(rename = "castIcon")]
    pub cast_icon: String,
    pub prefer: String,
    pub sending: Option<Progress>,
    pub ended: String,
}

#[derive(Deserialize, Default, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Progress {
    pub done: f64,
    pub total: f64,
    pub waiting: bool,
}

#[derive(Deserialize, Default, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct FileInfo {
    pub name: String,
    pub mime: String,
}

#[derive(Deserialize, Default, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Request {
    pub id: String,
    pub from: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub what: String,
    pub files: Vec<FileInfo>,
    pub total: f64,
    #[serde(rename = "totalText")]
    pub total_text: String,
    pub pin: String,
    pub state: String,
    pub message: String,
    pub paths: Vec<String>,
    /// where it goes (the sharing folder's last part, else Downloads)
    pub folder: String,
    pub item: Option<Item>,
}

#[derive(Deserialize, Default, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Item {
    pub done: f64,
    pub total: f64,
    pub waiting: bool,
    pub transport: String,
    #[serde(rename = "doneText")]
    pub done_text: String,
    #[serde(rename = "totalText")]
    pub total_text: String,
}

#[derive(Deserialize, Default, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Link {
    pub mode: String,
    pub stage: String,
    pub names: Vec<String>,
    pub qr: String,
    pub error: String,
    pub busy: bool,
    pub done: f64,
    pub total: f64,
    pub count: f64,
    pub installing: bool,
    pub updates: bool,
}

// ── Pure logic (tested below) ───────────────────────────────────────────

fn ext_in(path: &str, exts: &[&str]) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    match name.rsplit_once('.') {
        Some((_, e)) => exts.iter().any(|x| e.eq_ignore_ascii_case(x)),
        None => false,
    }
}

/// A picture the sheet previews (the QML's /\.(jpe?g|png|webp|gif|bmp|avif)$/i)
pub fn is_image(path: &str) -> bool {
    ext_in(path, &["jpg", "jpeg", "png", "webp", "gif", "bmp", "avif"])
}

pub fn is_video(path: &str) -> bool {
    ext_in(path, &["mp4", "m4v", "webm", "mkv", "mov"])
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// The sheet's title: the text in quotes (its spaces folded), the one
/// file's name, or "3 items"
pub fn sheet_title(paths: &[String], note: &str) -> String {
    if !note.is_empty() {
        let folded: Vec<&str> = note.split_whitespace().collect();
        // (the QML folds runs of whitespace to one space, keeping the ends)
        let mut t = folded.join(" ");
        if note.starts_with(char::is_whitespace) {
            t.insert(0, ' ');
        }
        if note.ends_with(char::is_whitespace) {
            t.push(' ');
        }
        return format!("“{t}”");
    }
    if paths.len() == 1 {
        return file_name(&paths[0]).to_string();
    }
    format!("{} items", paths.len())
}

/// Text.ElideMiddle, by count (Slint elides only at the end): at most `n`
/// characters, the middle swapped for "…" (a file name keeps its extension)
pub fn elide_middle(text: &str, n: usize) -> String {
    let count = text.chars().count();
    if count <= n || n < 3 {
        return text.to_string();
    }
    let keep = n - 1;
    let head = keep - keep / 2;
    let tail = keep / 2;
    let a: String = text.chars().take(head).collect();
    let b: String = text.chars().skip(count - tail).collect();
    format!("{a}…{b}")
}

/// About how many characters of a font size fit a width (the shell's sans)
pub fn fits(width: f32, size: f32) -> usize {
    ((width / (size * 0.56)).floor().max(4.0)) as usize
}

/// Under the title: what kind of thing it is
pub fn sheet_kind(paths: &[String], note: &str) -> &'static str {
    if !note.is_empty() {
        "Text · sent as a text file"
    } else if paths.len() > 1 {
        "Files"
    } else if paths.first().is_some_and(|p| is_image(p)) {
        "Photo"
    } else if paths.first().is_some_and(|p| is_video(p)) {
        "Video"
    } else {
        "Document"
    }
}

/// The other ways: (label, Phosphor name, what the shell does)
pub fn ways(note: &str) -> Vec<(&'static str, &'static str, &'static str)> {
    if !note.is_empty() {
        vec![("Copy", "copy", "copy")]
    } else {
        vec![("Copy", "copy", "copy"), ("Show in Files", "folder-open", "reveal"), ("Share with a QR code", "qr-code", "qr")]
    }
}

/// How far a send or a transfer is, 0..1
pub fn frac(done: f64, total: f64) -> f32 {
    if total > 0.0 {
        (done / total).clamp(0.0, 1.0) as f32
    } else {
        0.0
    }
}

/// Under a device: how its send is going, how it ended, or how it's reached
pub fn device_status(d: &Device) -> String {
    if let Some(s) = &d.sending {
        return if s.waiting { "Waiting…".into() } else { format!("Sending… {}%", (100.0 * frac(s.done, s.total)).round() as i32) };
    }
    match d.ended.as_str() {
        "sent" => "Sent".into(),
        "declined" => "Declined".into(),
        "failed" => "Didn't send".into(),
        _ if d.cast => "Cast".into(),
        _ if d.prefer == "quickshare" => "Quick Share".into(),
        _ => "LocalSend".into(),
    }
}

/// A device's icon (Phosphor name)
pub fn device_icon(d: &Device) -> &str {
    if d.cast {
        return if d.cast_icon.is_empty() { "television-simple" } else { &d.cast_icon };
    }
    match d.kind.as_str() {
        "mobile" => "device-mobile",
        "tablet" => "device-tablet",
        "laptop" => "laptop",
        _ => "desktop",
    }
}

/// Material: the shape its kind rests on (phones a cookie, tablets a
/// clover, computers a soft square, Chromecasts a sunny)
pub fn device_shape(d: &Device) -> &'static str {
    if d.cast {
        "sunny"
    } else if d.kind == "mobile" {
        "cookie9"
    } else if d.kind == "tablet" {
        "clover4"
    } else {
        "cookie4"
    }
}

/// Every file a picture (by type, or by name)
pub fn photos(files: &[FileInfo]) -> bool {
    !files.is_empty() && files.iter().all(|f| f.mime.starts_with("image/") || ext_in(&f.name, &["jpg", "jpeg", "png", "heic", "webp", "gif"]))
}

/// What's coming: “a.pdf”, "2 photos", "3 files", or what the protocol said
pub fn incoming_what(r: &Request) -> String {
    match r.files.len() {
        0 => r.what.clone(),
        1 => format!("“{}”", r.files[0].name),
        n => format!("{n} {}", if photos(&r.files) { "photos" } else { "files" }),
    }
}

/// The request's own progress, 0..1
pub fn request_frac(r: &Request) -> f32 {
    r.item.as_ref().map_or(0.0, |i| frac(i.done, i.total))
}

/// The card's line under who it is, by state
pub fn card_message(r: &Request) -> String {
    let what = incoming_what(r);
    match r.state.as_str() {
        "asking" => format!("would like to share {what}{}.", if r.total > 0.0 { format!(" ({})", r.total_text) } else { String::new() }),
        "receiving" => {
            if r.item.as_ref().is_some_and(|i| i.waiting) {
                "Starting…".into()
            } else {
                format!("Receiving {what}… {}%", (100.0 * request_frac(r)).round() as i32)
            }
        }
        "done" => format!("{what} {} in {}.", if r.files.len() == 1 { "is" } else { "are" }, r.folder),
        _ => {
            if r.message.is_empty() {
                "It stopped before everything arrived.".into()
            } else {
                r.message.clone()
            }
        }
    }
}

/// "1.2 MB of 6.2 MB" while it arrives
pub fn card_bytes(r: &Request) -> String {
    match &r.item {
        Some(i) if r.state == "receiving" && !i.waiting => format!("{} of {}", i.done_text, i.total_text),
        _ => String::new(),
    }
}

/// How it's arriving, shown on the phone's screen: Bluetooth, a direct
/// link (the phone's hotspot or ours) or the same Wi-Fi
pub fn link_icon(transport: &str) -> &'static str {
    let t = transport.to_lowercase();
    if t.is_empty() {
        ""
    } else if t.contains("bluetooth") {
        "bluetooth"
    } else if t.contains("direct") || t.contains("hotspot") {
        "broadcast"
    } else {
        "wifi-high"
    }
}

pub fn handheld(kind: &str) -> bool {
    matches!(kind, "phone" | "mobile" | "tablet")
}

pub fn kind_icon(kind: &str) -> &'static str {
    match kind {
        "phone" | "mobile" => "device-mobile",
        "tablet" => "device-tablet",
        "laptop" => "laptop",
        _ => "desktop",
    }
}

/// Material: the shape by state (a cookie while it asks, round while it
/// arrives, sunny when it's there, a square if it went wrong)
pub fn state_shape(state: &str) -> &'static str {
    match state {
        "receiving" => "circle",
        "done" => "sunny",
        "failed" => "square",
        _ => "cookie9",
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    Answer(bool),
    Cancel,
    Hide,
    Show,
    Open,
    Drop,
}

/// The card's buttons by state (iOS alert buttons: the one to do on the right)
pub fn card_buttons(state: &str, paths: usize) -> Vec<(&'static str, bool, Act)> {
    match state {
        "asking" => vec![("Decline", false, Act::Answer(false)), ("Accept", true, Act::Answer(true))],
        "receiving" => vec![("Cancel", false, Act::Cancel), ("Hide", false, Act::Hide)],
        "done" if paths == 1 => vec![("Show in Files", false, Act::Show), ("Open", true, Act::Open)],
        "done" => vec![("Done", false, Act::Drop), ("Show in Files", true, Act::Show)],
        _ => vec![("OK", true, Act::Drop)],
    }
}

/// What a button asks of the shell
pub fn act_call(act: Act, id: &str) -> Value {
    match act {
        Act::Answer(yes) => json!(["answer", id, yes]),
        Act::Cancel => json!(["cancel", id]),
        Act::Hide => json!(["hide", id]),
        Act::Show => json!(["show", id]),
        Act::Open => json!(["open", id]),
        Act::Drop => json!(["drop", id]),
    }
}

pub fn link_title(l: &Link) -> String {
    if l.mode == "send" {
        if l.names.len() == 1 {
            format!("Share “{}”", l.names[0])
        } else {
            format!("Share {} files", l.names.len())
        }
    } else {
        "Receive from a phone".into()
    }
}

pub fn link_help(l: &Link) -> &'static str {
    if l.mode == "send" {
        "Open the Camera on the iPhone and point it at the code, then tap Save for each file."
    } else {
        "Open the Camera on the iPhone and point it at the code, then choose what to send."
    }
}

pub fn link_status(l: &Link) -> String {
    let send = l.mode == "send";
    if l.busy {
        let pct = if l.total > 0.0 { (100.0 * l.done / l.total).round() as i64 } else { 0 };
        return format!("{}{pct}%", if send { "Sending… " } else { "Receiving… " });
    }
    let n = l.count.round() as i64;
    if n > 0 {
        return if send { format!("Saved {n}{}", if n == 1 { " time" } else { " times" }) } else { format!("Received {n}{}", if n == 1 { " file" } else { " files" }) };
    }
    "The phone needs the same Wi-Fi as this computer.".into()
}

/// The sheet's keys (answered at once, from the window): Esc, Tab and
/// Shift+Tab, Space and Enter always; Left and Right on a device
pub fn sheet_handles(text: &str, focus: i32, devices: usize) -> bool {
    let k = |key: Key| text == SharedString::from(key).as_str();
    let on_device = focus >= 1 && (focus as usize) <= devices;
    k(Key::Escape) || k(Key::Tab) || k(Key::Backtab) || k(Key::Return) || text == " " || (on_device && (k(Key::LeftArrow) || k(Key::RightArrow)))
}

/// The next place for the keyboard among `total` (wrapping; from none,
/// forward is the first and back the last)
pub fn next_focus(cur: i32, forward: bool, total: usize) -> i32 {
    if total == 0 {
        return -1;
    }
    let t = total as i32;
    if cur < 0 || cur >= t {
        return if forward { 0 } else { t - 1 };
    }
    (cur + if forward { 1 } else { -1 }).rem_euclid(t)
}

/// How far the devices' row scrolls so device `index` is in the middle,
/// within the row (`view`: its visible width)
pub fn row_scroll(index: usize, count: usize, view: f32) -> f32 {
    let content = count as f32 * DEV_W + count.saturating_sub(1) as f32 * (DEV_STEP - DEV_W);
    let x = index as f32 * DEV_STEP;
    (x + DEV_W / 2.0 - view / 2.0).clamp(0.0, (content - view).max(0.0))
}

/// Material shapes the QML names that src/surfaces/shapes.rs doesn't draw:
/// "cookie4" (four soft scallops) and "square" (a rounded square)
pub fn outline(name: &str) -> String {
    match name {
        "cookie4" => wave(4.0, 0.08, 44.0),
        "square" => "M 30 4 L 70 4 A 26 26 0 0 1 96 30 L 96 70 A 26 26 0 0 1 70 96 L 30 96 A 26 26 0 0 1 4 70 L 4 30 A 26 26 0 0 1 30 4 Z".into(),
        _ => crate::surfaces::shapes::outline(name),
    }
}

/// A polar wave r(θ) = R(1 + a·cos(nθ)) in a 0..100 box (as shapes.rs)
fn wave(n: f64, a: f64, r: f64) -> String {
    let steps = 180;
    let mut d = String::new();
    for i in 0..=steps {
        let t = i as f64 / steps as f64 * std::f64::consts::TAU;
        let rr = r * (1.0 + a * (n * t).cos());
        let (x, y) = (50.0 + rr * (t - std::f64::consts::FRAC_PI_2).cos(), 50.0 + rr * (t - std::f64::consts::FRAC_PI_2).sin());
        d += &format!("{}{x:.2} {y:.2} ", if i == 0 { "M " } else { "L " });
    }
    d + "Z"
}

fn glyph(name: &str) -> SharedString {
    crate::icons::glyph(name).into()
}

// ── The part ─────────────────────────────────────────────────────────────

enum M {
    SheetKey(String),
    Send(i32),
    Way(i32),
    Dismissed,
    SheetClosed,
    Thumb(String, Option<SharedPixelBuffer<Rgba8Pixel>>),
    CardButton(i32),
    CardKey(String),
    LinkDone,
    LinkInstall,
    LinkKey(String),
}

struct Win<T> {
    output: String,
    surface: Rc<Surface>,
    ui: T,
    a11y: Option<crate::a11y::A11y>,
    glass_sent: String,
    /// the size it was last shown at (cards)
    size: (u32, u32),
    /// shown now (cards)
    up: bool,
}

#[derive(Default)]
pub struct Share {
    sheet: Option<Win<ui::ShareSheet>>,
    card: Option<Win<ui::ShareCard>>,
    link: Option<Win<ui::LinkCard>>,
    open: bool,
    feed: Feed,
    /// keyboard: 0 the ×, 1.. the devices, then the other ways (-1 none)
    focus: i32,
    card_focus: i32,
    link_focus: bool,
    /// the header's picture: (its path, decoded) once asked for
    thumb: Option<(String, Option<slint::Image>)>,
    icons: HashMap<String, Option<slint::Image>>,
    qr: Option<(String, Option<slint::Image>)>,
}

/// Fill a window's globals: the look, and Material Symbols for Phosphor names
macro_rules! globals {
    ($st:expr, $ui:expr) => {{
        $st.shell.apply_theme(&$ui.global::<ui::Theme>());
        $ui.global::<ui::Symbols>().on_of(|g| crate::icons::symbol(&g).into());
    }};
}

impl Share {
    fn motion(&self) -> f32 {
        self.feed.motion.unwrap_or(1.0).clamp(0.1, 1.0)
    }

    fn first_output(st: &State) -> Option<String> {
        st.outputs_now().first().map(|o| o.1.clone())
    }

    /// The output the cards go on: the screen you're using, else the first
    fn card_output(&self, st: &State) -> Option<String> {
        let want = &self.feed.screen;
        if !want.is_empty() && st.output_named(want).is_some() {
            return Some(want.clone());
        }
        Self::first_output(st)
    }

    // ── The sheet ──────────────────────────────────────────────────────

    fn sheet_window(&mut self, st: &mut State, output: &str) -> bool {
        if self.sheet.as_ref().is_some_and(|w| w.output != output) {
            let w = self.sheet.take().unwrap();
            Glass::clear_now(&w.output, NS_SHEET);
            // (hidden first: Slint keeps a shown window, and with it the layer, alive)
            let _ = w.ui.hide();
            w.surface.hide();
            st.drop_surface(&w.surface);
        }
        if self.sheet.is_some() {
            return true;
        }
        let Some(wl_output) = st.output_named(output) else { return false };
        let surface = st.prepare_surface(Spec {
            layer: Layer::Overlay,
            namespace: NS_SHEET,
            output: Some((&wl_output, output)),
            anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
            size: (0, 0),
            exclusive: -1,
            keyboard: KeyboardInteractivity::None,
            margin_left: 0,
        });
        let ui = ui::ShareSheet::new().expect("ShareSheet");
        globals!(st, ui);
        let to = st.shell.sender(NAME);
        {
            let (to, weak) = (to.clone(), ui.as_weak());
            ui.on_key(move |text, _shift| {
                let (focus, n) = weak.upgrade().map_or((-1, 0), |u| (u.get_focus_at(), u.get_devices().row_count()));
                let ok = sheet_handles(&text, focus, n);
                if ok {
                    to(Box::new(M::SheetKey(text.to_string())));
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
        on!(on_send, |i| M::Send(i));
        on!(on_way, |i| M::Way(i));
        on!(on_dismissed, | | M::Dismissed);
        on!(on_closed, | | M::SheetClosed);
        ui.set_hover_shape(outline("cookie12").into());
        ui.set_circle_shape(outline("circle").into());
        ui.set_quiet_shape(outline("cookie9").into());
        ui.set_fallback_glyph(glyph("file"));
        ui.show().expect("show share sheet");
        let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
        self.sheet = Some(Win { output: output.into(), surface, ui, a11y, glass_sent: String::new(), size: (0, 0), up: false });
        true
    }

    fn open_sheet(&mut self, st: &mut State, screen: Option<String>) {
        let output = screen.or_else(|| self.sheet.as_ref().map(|w| w.output.clone())).or_else(|| Self::first_output(st));
        let Some(output) = output else { return };
        if !self.sheet_window(st, &output) {
            return;
        }
        self.open = true;
        self.focus = -1;
        let w = self.sheet.as_ref().unwrap();
        w.ui.set_people_x(0.0);
        w.ui.set_focus_at(-1);
        w.ui.invoke_open();
        w.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        w.surface.show(0, 0);
        self.push_sheet(st);
        self.sheet.as_ref().unwrap().ui.set_shown(true);
    }

    fn close_sheet(&mut self, tell: bool) {
        if !self.open {
            return;
        }
        self.open = false;
        if let Some(w) = &self.sheet {
            w.ui.set_shown(false);
            w.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::None);
            w.surface.layer.commit();
        }
        if tell {
            crate::shell::closed(NAME);
        }
    }

    /// Its fade is over: unmap
    fn sheet_closed(&mut self, st: &mut State) {
        if self.open {
            return;
        }
        if let Some(w) = &mut self.sheet {
            w.surface.hide();
            st.shell.glass.set(&w.output, NS_SHEET, &[]);
            w.glass_sent.clear();
        }
    }

    /// The icon IconLookup found ("file://" and its segments URI-encoded; an
    /// SVG most often), drawn as it is (AppIcon's `styled: false`) at 72 px
    fn icon_image(&mut self, path: &str) -> Option<slint::Image> {
        let file = tessera_core::services::mpris::percent_decode(path.strip_prefix("file://").unwrap_or(path));
        if !file.starts_with('/') {
            return None;
        }
        self.icons
            .entry(file.clone())
            .or_insert_with(|| {
                let plain = tessera_core::appicon::Style { mode: 0, tiled: false, dark: false, contrast: 60, tint: [0; 3], tile: None, tile_from: String::new(), accent: [0; 3] };
                tessera_core::appicon::render(std::path::Path::new(&file), 72, &plain, None).map(slint::Image::from_rgba8)
            })
            .clone()
    }

    /// Decode the shared picture off the UI thread, at most 104 px (the
    /// 52 px thumbnail at 2x)
    fn ask_thumb(&mut self, st: &State, path: &str) {
        if self.thumb.as_ref().is_some_and(|(p, _)| p == path) {
            return;
        }
        self.thumb = Some((path.to_string(), None));
        let to = st.shell.sender(NAME);
        let p = path.to_string();
        std::thread::spawn(move || {
            let px = image::open(&p).ok().map(|i| {
                let t = i.resize_to_fill(104, 104, image::imageops::FilterType::Triangle).to_rgba8();
                SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(t.as_raw(), t.width(), t.height())
            });
            to(Box::new(M::Thumb(p, px)));
        });
    }

    fn push_sheet(&mut self, st: &State) {
        if self.sheet.is_none() {
            return;
        }
        let s = self.feed.sheet.clone();
        let picture = s.note.is_empty() && s.paths.len() >= 1 && is_image(&s.paths[0]);
        if picture {
            self.ask_thumb(st, &s.paths[0]);
        }
        let icon = if picture { None } else { self.icon_image(&s.icon) };
        let thumb = self.thumb.as_ref().filter(|(p, _)| picture && s.paths.first() == Some(p)).and_then(|(_, i)| i.clone());
        let devices: Vec<ui::ShareDev> = s
            .devices
            .iter()
            .map(|d| ui::ShareDev {
                key: d.key.as_str().into(),
                name: d.name.as_str().into(),
                label: format!("{}{}", if d.cast { "Cast to " } else { "Send to " }, d.name).into(),
                status: device_status(d).into(),
                glyph: glyph(device_icon(d)),
                cast: d.cast,
                sending: d.sending.is_some(),
                frac: d.sending.as_ref().map_or(0.0, |p| if p.waiting { -1.0 } else { frac(p.done, p.total) }),
                ended: d.ended.as_str().into(),
                shape: outline(device_shape(d)).into(),
            })
            .collect();
        let ways: Vec<ui::ShareWay> = ways(&s.note).into_iter().map(|(l, g, _)| ui::ShareWay { label: l.into(), glyph: glyph(g) }).collect();
        let help = format!(
            "Phones show up with Quick Share open and visible to everyone; computers with LocalSend open.{}",
            if s.ports_error.is_empty() { String::new() } else { format!(" {}", s.ports_error) }
        );
        let ndev = devices.len();
        let nways = ways.len();
        let w = self.sheet.as_ref().unwrap();
        let ui = &w.ui;
        // (QML: elided in the middle; the title's width: the card's, less the
        // thumbnail, the × and the gaps)
        let material = st.shell.look.material;
        let room = if material { 432.0 - 48.0 - 64.0 - 40.0 - 8.0 } else { 420.0 - 32.0 - 64.0 - 30.0 - 8.0 };
        ui.set_title_text(elide_middle(&sheet_title(&s.paths, &s.note), fits(room, if material { 16.0 } else { 15.0 })).into());
        ui.set_kind_text(sheet_kind(&s.paths, &s.note).into());
        ui.set_thumb_is_image(picture);
        ui.set_thumb(thumb.unwrap_or_default());
        ui.set_has_icon(icon.is_some());
        ui.set_file_icon(icon.unwrap_or_default());
        ui.set_devices(ModelRc::new(VecModel::from(devices)));
        ui.set_available(s.available);
        ui.set_help(help.into());
        ui.set_errors(s.errors.as_str().into());
        ui.set_ways(ModelRc::new(VecModel::from(ways)));
        // (the keyboard's place, kept within what's there now)
        if self.focus >= (1 + ndev + nways) as i32 {
            self.focus = -1;
        }
        ui.set_focus_at(self.focus);
    }

    fn sheet_key(&mut self, st: &State, text: &str) {
        let is = |key: Key| text == SharedString::from(key).as_str();
        let ndev = self.feed.sheet.devices.len();
        let nways = ways(&self.feed.sheet.note).len();
        let total = 1 + ndev + nways;
        if is(Key::Escape) {
            self.close_sheet(true);
            return;
        }
        if is(Key::Tab) || is(Key::Backtab) || is(Key::LeftArrow) || is(Key::RightArrow) {
            let forward = is(Key::Tab) || is(Key::RightArrow);
            self.focus = next_focus(self.focus, forward, total);
            if let Some(w) = &self.sheet {
                w.ui.set_focus_at(self.focus);
                if self.focus >= 1 && (self.focus as usize) <= ndev {
                    let view = w.ui.get_people_width();
                    w.ui.set_people_x(-row_scroll(self.focus as usize - 1, ndev, view));
                }
            }
            return;
        }
        if is(Key::Return) || text == " " {
            match self.focus {
                0 => self.close_sheet(true),
                f if f >= 1 && (f as usize) <= ndev => self.send(f - 1),
                f if f > ndev as i32 && ((f as usize) - 1 - ndev) < nways => self.way(st, f - 1 - ndev as i32),
                _ => {}
            }
        }
    }

    fn send(&mut self, i: i32) {
        let Some(d) = usize::try_from(i).ok().and_then(|i| self.feed.sheet.devices.get(i)) else { return };
        if d.sending.is_some() {
            return;
        }
        crate::shell::ask("share", json!(["send", d.key]));
        // (Cast: the shell closes the sheet; the sheet stays for a send, the ring shows it)
    }

    fn way(&mut self, _st: &State, i: i32) {
        let list = ways(&self.feed.sheet.note);
        let Some((_, _, what)) = usize::try_from(i).ok().and_then(|i| list.get(i)) else { return };
        crate::shell::ask("share", json!([what]));
        self.close_sheet(true);
    }

    // ── The cards ──────────────────────────────────────────────────────

    fn card_surface(st: &mut State, output: &str, ns: &str, w: u32) -> Option<Rc<Surface>> {
        let wl_output = st.output_named(output)?;
        Some(st.prepare_surface(Spec {
            layer: Layer::Overlay,
            namespace: ns,
            output: Some((&wl_output, output)),
            anchor: Anchor::empty(),
            size: (w, 300),
            exclusive: -1,
            keyboard: KeyboardInteractivity::OnDemand,
            margin_left: 0,
        }))
    }

    fn update_card(&mut self, st: &mut State) {
        let Some(r) = self.feed.request.clone() else {
            if let Some(w) = &mut self.card {
                if w.up {
                    w.up = false;
                    w.ui.invoke_leave();
                    w.surface.hide();
                    st.shell.glass.set(&w.output, NS_CARD, &[]);
                    w.glass_sent.clear();
                    w.size = (0, 0);
                }
            }
            self.card_focus = -1;
            return;
        };
        let Some(output) = self.card_output(st) else { return };
        if self.card.as_ref().is_some_and(|w| w.output != output) {
            let w = self.card.take().unwrap();
            Glass::clear_now(&w.output, NS_CARD);
            // (hidden first: Slint keeps a shown window, and with it the layer, alive)
            let _ = w.ui.hide();
            w.surface.hide();
            st.drop_surface(&w.surface);
        }
        if self.card.is_none() {
            let width = if st.shell.look.material { 312 } else { 300 } + 2 * MARGIN as u32;
            let Some(surface) = Self::card_surface(st, &output, NS_CARD, width) else { return };
            let ui = ui::ShareCard::new().expect("ShareCard");
            globals!(st, ui);
            let to = st.shell.sender(NAME);
            {
                let to = to.clone();
                ui.on_key(move |text| {
                    let k = |key: Key| text == SharedString::from(key).as_str();
                    let ok = k(Key::Return) || k(Key::Escape) || k(Key::Tab) || k(Key::Backtab) || text == " ";
                    if ok {
                        to(Box::new(M::CardKey(text.to_string())));
                    }
                    ok
                });
            }
            {
                let to = to.clone();
                ui.on_pressed_button(move |i| to(Box::new(M::CardButton(i))));
            }
            ui.show().expect("show share card");
            let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
            self.card = Some(Win { output: output.clone(), surface, ui, a11y, glass_sent: String::new(), size: (0, 0), up: false });
        }
        let buttons = card_buttons(&r.state, r.paths.len());
        if self.card_focus >= buttons.len() as i32 {
            self.card_focus = -1;
        }
        let motion = self.motion();
        let w = self.card.as_mut().unwrap();
        let ui = &w.ui;
        ui.set_stage(r.state.as_str().into());
        ui.set_sender_name(r.from.as_str().into());
        ui.set_message(card_message(&r).into());
        ui.set_bytes(card_bytes(&r).into());
        ui.set_pin(r.pin.as_str().into());
        ui.set_frac(if r.item.as_ref().is_some_and(|i| i.waiting) { -1.0 } else { request_frac(&r) });
        ui.set_handheld(handheld(&r.kind));
        ui.set_tablet(r.kind == "tablet");
        ui.set_kind_glyph(glyph(kind_icon(&r.kind)));
        ui.set_link_glyph(glyph(link_icon(r.item.as_ref().map_or("", |i| i.transport.as_str()))));
        ui.set_buttons(ModelRc::new(VecModel::from(buttons.iter().map(|(l, p, _)| ui::ShareBtn { label: (*l).into(), primary: *p }).collect::<Vec<_>>())));
        ui.set_shape(outline(state_shape(&r.state)).into());
        ui.set_motion(motion);
        ui.set_focus_at(self.card_focus);
        Self::fit(w);
        if !w.up {
            w.up = true;
            w.ui.invoke_enter();
        }
    }

    /// A card's window follows its card's size (shown at it)
    fn fit<T: ComponentHandle + CardSize>(w: &mut Win<T>) {
        let size = (w.ui.card_w().round().max(1.0) as u32 + 2 * MARGIN as u32, w.ui.want_h().round().max(1.0) as u32);
        if size != w.size {
            w.size = size;
            w.surface.show(size.0, size.1);
        }
    }

    fn card_button(&mut self, i: i32) {
        let Some(r) = &self.feed.request else { return };
        let list = card_buttons(&r.state, r.paths.len());
        let Some((_, _, act)) = usize::try_from(i).ok().and_then(|i| list.get(i)) else { return };
        crate::shell::ask("share", act_call(*act, &r.id));
    }

    fn card_key(&mut self, text: &str) {
        let is = |key: Key| text == SharedString::from(key).as_str();
        let Some(r) = self.feed.request.clone() else { return };
        let n = card_buttons(&r.state, r.paths.len()).len();
        if is(Key::Return) {
            if r.state == "asking" {
                crate::shell::ask("share", act_call(Act::Answer(true), &r.id));
            } else if self.card_focus >= 0 {
                self.card_button(self.card_focus);
            }
        } else if is(Key::Escape) {
            let act = if r.state == "asking" { Act::Answer(false) } else { Act::Drop };
            crate::shell::ask("share", act_call(act, &r.id));
        } else if is(Key::Tab) || is(Key::Backtab) {
            self.card_focus = next_focus(self.card_focus, is(Key::Tab), n);
            if let Some(w) = &self.card {
                w.ui.set_focus_at(self.card_focus);
            }
        } else if text == " " && self.card_focus >= 0 {
            self.card_button(self.card_focus);
        }
    }

    fn update_link(&mut self, st: &mut State) {
        let Some(l) = self.feed.link.clone() else {
            if let Some(w) = &mut self.link {
                if w.up {
                    w.up = false;
                    w.ui.invoke_leave();
                    w.surface.hide();
                    st.shell.glass.set(&w.output, NS_LINK, &[]);
                    w.glass_sent.clear();
                    w.size = (0, 0);
                }
            }
            self.link_focus = false;
            // (a new code next time, even at the same path)
            self.qr = None;
            return;
        };
        let Some(output) = self.card_output(st) else { return };
        if self.link.as_ref().is_some_and(|w| w.output != output && !w.up) {
            let w = self.link.take().unwrap();
            Glass::clear_now(&w.output, NS_LINK);
            // (hidden first: Slint keeps a shown window, and with it the layer, alive)
            let _ = w.ui.hide();
            w.surface.hide();
            st.drop_surface(&w.surface);
        }
        if self.link.is_none() {
            let Some(surface) = Self::card_surface(st, &output, NS_LINK, 320 + 2 * MARGIN as u32) else { return };
            let ui = ui::LinkCard::new().expect("LinkCard");
            globals!(st, ui);
            let to = st.shell.sender(NAME);
            {
                let to = to.clone();
                ui.on_key(move |text| {
                    let k = |key: Key| text == SharedString::from(key).as_str();
                    let ok = k(Key::Return) || k(Key::Escape) || k(Key::Tab) || k(Key::Backtab) || text == " ";
                    if ok {
                        to(Box::new(M::LinkKey(text.to_string())));
                    }
                    ok
                });
            }
            {
                let to = to.clone();
                ui.on_done(move || to(Box::new(M::LinkDone)));
            }
            {
                let to = to.clone();
                ui.on_install(move || to(Box::new(M::LinkInstall)));
            }
            ui.show().expect("show QR card");
            let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
            self.link = Some(Win { output, surface, ui, a11y, glass_sent: String::new(), size: (0, 0), up: false });
        }
        // The code: read again whenever the file changes name (a new code)
        if !l.qr.is_empty() && self.qr.as_ref().map_or(true, |(p, _)| *p != l.qr) {
            let img = slint::Image::load_from_path(std::path::Path::new(&l.qr)).ok();
            self.qr = Some((l.qr.clone(), img));
        }
        let qr = self.qr.as_ref().filter(|(p, _)| *p == l.qr).and_then(|(_, i)| i.clone());
        let motion = self.motion();
        let w = self.link.as_mut().unwrap();
        let ui = &w.ui;
        ui.set_heading(elide_middle(&link_title(&l), fits(272.0, 18.0)).into());
        ui.set_help(link_help(&l).into());
        ui.set_stage(l.stage.as_str().into());
        ui.set_qr(qr.unwrap_or_default());
        ui.set_error(l.error.as_str().into());
        ui.set_can_install(l.error.contains("qrencode"));
        ui.set_installing(l.installing);
        ui.set_install_enabled(!l.updates);
        ui.set_status(link_status(&l).into());
        ui.set_status_strong(l.busy || l.count > 0.0);
        ui.set_busy(l.busy);
        ui.set_motion(motion);
        ui.set_button_focused(self.link_focus);
        Self::fit(w);
        if !w.up {
            w.up = true;
            w.ui.invoke_enter();
        }
    }

    fn link_key(&mut self, text: &str) {
        let is = |key: Key| text == SharedString::from(key).as_str();
        if is(Key::Escape) || is(Key::Return) || (text == " " && self.link_focus) {
            crate::shell::ask("share", json!(["linkClose"]));
        } else if is(Key::Tab) || is(Key::Backtab) {
            self.link_focus = !self.link_focus;
            if let Some(w) = &self.link {
                w.ui.set_button_focused(self.link_focus);
            }
        }
    }

    fn apply_look(&mut self, st: &mut State) {
        if let Some(w) = &mut self.sheet {
            globals!(st, w.ui);
            w.glass_sent.clear();
        }
        if let Some(w) = &mut self.card {
            globals!(st, w.ui);
            w.glass_sent.clear();
        }
        if let Some(w) = &mut self.link {
            globals!(st, w.ui);
            w.glass_sent.clear();
        }
    }

    fn refresh(&mut self, st: &mut State) {
        if self.open {
            self.push_sheet(st);
        }
        self.update_card(st);
        self.update_link(st);
    }
}

/// What the part reads of a card's window to size it
trait CardSize {
    fn card_w(&self) -> f32;
    fn want_h(&self) -> f32;
    fn radius(&self) -> f32;
}
impl CardSize for ui::ShareCard {
    fn card_w(&self) -> f32 {
        self.get_card_width()
    }
    fn want_h(&self) -> f32 {
        self.get_want_height()
    }
    fn radius(&self) -> f32 {
        self.global::<ui::Theme>().get_radius_panel()
    }
}
impl CardSize for ui::LinkCard {
    fn card_w(&self) -> f32 {
        self.get_card_width()
    }
    fn want_h(&self) -> f32 {
        self.get_want_height()
    }
    fn radius(&self) -> f32 {
        self.global::<ui::Theme>().get_radius_panel()
    }
}

/// A card's glass (Glass, the plugin there): the card's rectangle, scaled
/// about the window's centre with its entrance (scale `s0 + (1 - s0)·shown`)
fn card_glass<T: ComponentHandle + CardSize>(st: &mut State, w: &mut Win<T>, ns: &str, shown: f32, s0: f32) {
    if !w.up || !w.surface.mapped.get() {
        return;
    }
    let (ww, wh) = w.surface.logical_size();
    let s = s0 + (1.0 - s0) * shown;
    let (cx, cy) = (ww as f32 / 2.0, wh as f32 / 2.0);
    let (x, y) = (cx + (MARGIN - cx) * s, cy + (MARGIN - cy) * s);
    let (cw, ch) = (ww as f32 - 2.0 * MARGIN, wh as f32 - 2.0 * MARGIN);
    let radius = w.ui.radius();
    let o = shown.clamp(0.0, 1.0);
    let shape = theme::glass_shape(&st.shell.look, x, y, cw * s, ch * s, radius * s, o, 1.0);
    let key = if o > 0.002 { format!("{shape:?}") } else { String::new() };
    if key != w.glass_sent {
        st.shell.glass.set(&w.output, ns, if o > 0.002 { std::slice::from_ref(&shape) } else { &[] });
        w.glass_sent = key;
    }
}

impl Part for Share {
    fn name(&self) -> &'static str {
        NAME
    }

    fn start(&mut self, _st: &mut State) {
        self.focus = -1;
        self.card_focus = -1;
    }

    fn command(&mut self, st: &mut State, cmd: &Command) {
        match cmd.cmd.as_str() {
            "open" => self.open_sheet(st, cmd.screen.clone()),
            "close" => self.close_sheet(false),
            "toggle" => {
                if self.open {
                    self.close_sheet(true)
                } else {
                    self.open_sheet(st, cmd.screen.clone())
                }
            }
            _ => {}
        }
    }

    fn config_changed(&mut self, st: &mut State, look: bool) {
        if look {
            self.apply_look(st);
            if self.card.is_some() {
                // (Material's card is 12 px wider: a window of its new size)
                if let Some(w) = &mut self.card {
                    w.size = (0, 0);
                }
            }
            self.refresh(st);
        }
    }

    fn plugin_changed(&mut self, st: &mut State) {
        self.apply_look(st);
    }

    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        let msg = match msg.downcast::<M>() {
            Ok(m) => m,
            Err(other) => {
                // The shell's state (ShareFeed.qml): {"to":"share","feed":{…}}
                if let Ok(v) = other.downcast::<Value>() {
                    if let Ok(feed) = serde_json::from_value::<Feed>(v["feed"].clone()) {
                        if feed != self.feed {
                            self.feed = feed;
                            self.refresh(st);
                        }
                    }
                }
                return;
            }
        };
        match *msg {
            M::SheetKey(t) => self.sheet_key(st, &t),
            M::Send(i) => self.send(i),
            M::Way(i) => self.way(st, i),
            M::Dismissed => self.close_sheet(true),
            M::SheetClosed => self.sheet_closed(st),
            M::Thumb(p, px) => {
                if self.thumb.as_ref().is_some_and(|(q, _)| *q == p) {
                    self.thumb = Some((p, px.map(slint::Image::from_rgba8)));
                    if self.open {
                        self.push_sheet(st);
                    }
                }
            }
            M::CardButton(i) => self.card_button(i),
            M::CardKey(t) => self.card_key(&t),
            M::LinkDone => crate::shell::ask("share", json!(["linkClose"])),
            M::LinkInstall => {
                if self.feed.link.as_ref().is_some_and(|l| !l.updates && !l.installing) {
                    crate::shell::ask("share", json!(["installQr"]));
                }
            }
            M::LinkKey(t) => self.link_key(&t),
        }
    }

    fn keyboard_left(&mut self, _st: &mut State, s: &Rc<Surface>) {
        if self.open && self.sheet.as_ref().is_some_and(|w| Rc::ptr_eq(&w.surface, s)) {
            self.close_sheet(true);
        }
    }

    fn output_removed(&mut self, st: &mut State, name: &str) {
        if self.sheet.as_ref().is_some_and(|w| w.output == name) {
            let was = self.open;
            self.close_sheet(was);
            let w = self.sheet.take().unwrap();
            // (hidden first: Slint keeps a shown window, and with it the layer, alive)
            let _ = w.ui.hide();
            w.surface.hide();
            st.drop_surface(&w.surface);
        }
        if self.card.as_ref().is_some_and(|w| w.output == name) {
            let w = self.card.take().unwrap();
            // (hidden first: Slint keeps a shown window, and with it the layer, alive)
            let _ = w.ui.hide();
            w.surface.hide();
            st.drop_surface(&w.surface);
        }
        if self.link.as_ref().is_some_and(|w| w.output == name) {
            let w = self.link.take().unwrap();
            // (hidden first: Slint keeps a shown window, and with it the layer, alive)
            let _ = w.ui.hide();
            w.surface.hide();
            st.drop_surface(&w.surface);
        }
    }

    fn surface_closed(&mut self, st: &mut State, s: &Rc<Surface>) {
        let mine = self.sheet.as_ref().is_some_and(|w| Rc::ptr_eq(&w.surface, s))
            || self.card.as_ref().is_some_and(|w| Rc::ptr_eq(&w.surface, s))
            || self.link.as_ref().is_some_and(|w| Rc::ptr_eq(&w.surface, s));
        if mine {
            let name = s.output.clone();
            self.output_removed(st, &name);
            self.refresh(st);
        }
    }

    /// The glass under the sheet and the cards (Glass), following their
    /// animations; the cards' windows following their size; accessibility
    fn after_frame(&mut self, st: &mut State) {
        let kbd = st.keyboard_surface();
        let focused = |s: &Rc<Surface>| kbd.as_ref().is_some_and(|k| Rc::ptr_eq(k, s));
        let glass = st.shell.plugin && !st.shell.look.material;
        if let Some(w) = &mut self.sheet {
            if let Some(a) = w.a11y.as_mut() {
                a.update(false, focused(&w.surface));
            }
            if glass && w.surface.mapped.get() {
                let ui = &w.ui;
                let s = ui.get_card_scale();
                let (bw, bh) = (ui.get_card_width(), ui.get_card_height());
                let x = ui.get_card_x() + bw * (1.0 - s) / 2.0;
                let y = ui.get_card_y() + bh * (1.0 - s) / 2.0;
                let o = ui.get_card_opacity();
                let radius = ui.global::<ui::Theme>().get_radius_panel();
                let shape = theme::glass_shape(&st.shell.look, x, y, bw * s, bh * s, radius * s, o, 1.0);
                let key = if o > 0.002 { format!("{shape:?}") } else { String::new() };
                if key != w.glass_sent {
                    st.shell.glass.set(&w.output, NS_SHEET, if o > 0.002 { std::slice::from_ref(&shape) } else { &[] });
                    w.glass_sent = key;
                }
            }
        }
        if let Some(w) = &mut self.card {
            if let Some(a) = w.a11y.as_mut() {
                a.update(false, focused(&w.surface));
            }
            if w.up {
                Self::fit(w);
                if glass {
                    let shown = w.ui.get_shown();
                    card_glass(st, w, NS_CARD, shown, 0.9);
                }
            }
        }
        if let Some(w) = &mut self.link {
            if let Some(a) = w.a11y.as_mut() {
                a.update(false, focused(&w.surface));
            }
            if w.up {
                Self::fit(w);
                if glass {
                    let shown = w.ui.get_shown();
                    card_glass(st, w, NS_LINK, shown, 0.96);
                }
            }
        }
    }

    fn a11y(&mut self) -> Vec<&mut crate::a11y::A11y> {
        let mut out = Vec::new();
        if let Some(a) = self.sheet.as_mut().and_then(|w| w.a11y.as_mut()) {
            out.push(a);
        }
        if let Some(a) = self.card.as_mut().and_then(|w| w.a11y.as_mut()) {
            out.push(a);
        }
        if let Some(a) = self.link.as_mut().and_then(|w| w.a11y.as_mut()) {
            out.push(a);
        }
        out
    }

    fn stats(&self) {
        if let Some(w) = &self.sheet {
            eprintln!("tessera-ui: stats {}", w.surface.stats.borrow_mut().report("share"));
        }
    }

    fn quit(&mut self, _: &mut State) {
        if let Some(w) = &self.sheet {
            Glass::clear_now(&w.output, NS_SHEET);
        }
        if let Some(w) = &self.card {
            Glass::clear_now(&w.output, NS_CARD);
        }
        if let Some(w) = &self.link {
            Glass::clear_now(&w.output, NS_LINK);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(name: &str, kind: &str) -> Device {
        Device { key: name.to_lowercase(), name: name.into(), kind: kind.into(), ..Default::default() }
    }

    fn req(state: &str, files: &[(&str, &str)]) -> Request {
        Request {
            id: "demo:1".into(),
            from: "Ada's Pixel".into(),
            kind: "phone".into(),
            what: "something".into(),
            files: files.iter().map(|(n, m)| FileInfo { name: (*n).into(), mime: (*m).into() }).collect(),
            state: state.into(),
            folder: "Downloads".into(),
            ..Default::default()
        }
    }

    #[test]
    fn long_names_lose_their_middle() {
        assert_eq!(elide_middle("short.pdf", 20), "short.pdf");
        let e = elide_middle("a-very-long-holiday-photo-name.jpg", 15);
        assert_eq!(e.chars().count(), 15);
        assert!(e.starts_with("a-very-") && e.ends_with("e.jpg") && e.contains('…'), "{e}");
        assert!(fits(286.0, 15.0) >= 30);
    }

    #[test]
    fn what_the_sheet_shares() {
        let p = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(sheet_title(&p(&["/home/a/Pictures/beach.JPG"]), ""), "beach.JPG");
        assert_eq!(sheet_kind(&p(&["/home/a/Pictures/beach.JPG"]), ""), "Photo");
        assert_eq!(sheet_title(&p(&["/a/x.pdf", "/a/y.pdf"]), ""), "2 items");
        assert_eq!(sheet_kind(&p(&["/a/x.pdf", "/a/y.pdf"]), ""), "Files");
        assert_eq!(sheet_kind(&p(&["/a/clip.webm"]), ""), "Video");
        assert_eq!(sheet_kind(&p(&["/a/notes.txt"]), ""), "Document");
        assert_eq!(sheet_title(&p(&["/run/t.txt"]), "a  long\n\tline"), "“a long line”");
        assert_eq!(sheet_kind(&p(&["/run/t.txt"]), "hello"), "Text · sent as a text file");
        assert_eq!(ways("").len(), 3);
        assert_eq!(ways("hello"), vec![("Copy", "copy", "copy")]);
    }

    #[test]
    fn pictures_are_told_by_their_name() {
        for p in ["a.jpg", "b.JPEG", "c.png", "d.webp", "e.gif", "f.bmp", "g.avif"] {
            assert!(is_image(p), "{p}");
        }
        for p in ["a.heic", "b.pdf", "jpg", "/dir.png/file", "c.svg"] {
            assert!(!is_image(p), "{p}");
        }
    }

    #[test]
    fn a_device_says_how_its_send_went() {
        let mut d = dev("Ada's Pixel", "mobile");
        d.prefer = "quickshare".into();
        assert_eq!(device_status(&d), "Quick Share");
        assert_eq!(device_icon(&d), "device-mobile");
        assert_eq!(device_shape(&d), "cookie9");
        d.sending = Some(Progress { done: 0.0, total: 0.0, waiting: true });
        assert_eq!(device_status(&d), "Waiting…");
        d.sending = Some(Progress { done: 40.0, total: 100.0, waiting: false });
        assert_eq!(device_status(&d), "Sending… 40%");
        d.sending = None;
        d.ended = "sent".into();
        assert_eq!(device_status(&d), "Sent");
        d.ended = "declined".into();
        assert_eq!(device_status(&d), "Declined");
        d.ended = "failed".into();
        assert_eq!(device_status(&d), "Didn't send");
        let pc = dev("Grace's desk", "");
        assert_eq!((device_status(&pc), device_icon(&pc), device_shape(&pc)), ("LocalSend".to_string(), "desktop", "cookie4"));
        let tv = Device { cast: true, cast_icon: "speaker-hifi".into(), ..dev("Living room", "") };
        assert_eq!((device_status(&tv), device_icon(&tv), device_shape(&tv)), ("Cast".to_string(), "speaker-hifi", "sunny"));
        assert_eq!(device_shape(&dev("Tab", "tablet")), "clover4");
    }

    #[test]
    fn what_is_coming_and_how_it_goes() {
        let two = req("asking", &[("IMG_1.jpg", "image/jpeg"), ("IMG_2.heic", "")]);
        assert!(photos(&two.files));
        assert_eq!(incoming_what(&two), "2 photos");
        let mixed = req("asking", &[("a.jpg", "image/jpeg"), ("b.pdf", "application/pdf")]);
        assert_eq!(incoming_what(&mixed), "2 files");
        let one = req("asking", &[("plan.pdf", "")]);
        assert_eq!(incoming_what(&one), "“plan.pdf”");
        assert_eq!(incoming_what(&req("asking", &[])), "something");
        assert!(!photos(&[]));

        let mut r = two.clone();
        assert_eq!(card_message(&r), "would like to share 2 photos.");
        r.total = 6.2e6;
        r.total_text = "6.2 MB".into();
        assert_eq!(card_message(&r), "would like to share 2 photos (6.2 MB).");
        r.state = "receiving".into();
        r.item = Some(Item { done: 0.0, total: 0.0, waiting: true, ..Default::default() });
        assert_eq!(card_message(&r), "Starting…");
        assert_eq!(card_bytes(&r), "");
        r.item = Some(Item { done: 3.1e6, total: 6.2e6, waiting: false, transport: "Wi-Fi Direct".into(), done_text: "3.1 MB".into(), total_text: "6.2 MB".into() });
        assert_eq!(card_message(&r), "Receiving 2 photos… 50%");
        assert_eq!(card_bytes(&r), "3.1 MB of 6.2 MB");
        assert_eq!(link_icon("Wi-Fi Direct"), "broadcast");
        assert_eq!(link_icon("Bluetooth"), "bluetooth");
        assert_eq!(link_icon("Wi-Fi"), "wifi-high");
        assert_eq!(link_icon(""), "");
        r.state = "done".into();
        assert_eq!(card_message(&r), "2 photos are in Downloads.");
        assert_eq!(card_message(&Request { state: "done".into(), ..one.clone() }), "“plan.pdf” is in Downloads.");
        r.state = "failed".into();
        assert_eq!(card_message(&r), "It stopped before everything arrived.");
        r.message = "Ada's Pixel stopped sending.".into();
        assert_eq!(card_message(&r), "Ada's Pixel stopped sending.");
        assert_eq!(state_shape("asking"), "cookie9");
        assert_eq!(state_shape("failed"), "square");
        assert!(handheld("phone") && handheld("tablet") && !handheld("laptop"));
    }

    #[test]
    fn the_cards_buttons_by_state() {
        let labels = |s: &str, n: usize| card_buttons(s, n).iter().map(|b| b.0).collect::<Vec<_>>();
        assert_eq!(labels("asking", 0), ["Decline", "Accept"]);
        assert_eq!(labels("receiving", 0), ["Cancel", "Hide"]);
        assert_eq!(labels("done", 1), ["Show in Files", "Open"]);
        assert_eq!(labels("done", 3), ["Done", "Show in Files"]);
        assert_eq!(labels("failed", 0), ["OK"]);
        // the one to do is on the right, filled
        assert!(card_buttons("asking", 0)[1].1 && !card_buttons("asking", 0)[0].1);
        assert_eq!(act_call(Act::Answer(true), "qs:in-1"), json!(["answer", "qs:in-1", true]));
        assert_eq!(act_call(Act::Show, "x"), json!(["show", "x"]));
    }

    #[test]
    fn the_qr_cards_words() {
        let mut l = Link { mode: "send".into(), stage: "ready".into(), names: vec!["trip.mov".into()], ..Default::default() };
        assert_eq!(link_title(&l), "Share “trip.mov”");
        assert_eq!(link_status(&l), "The phone needs the same Wi-Fi as this computer.");
        l.busy = true;
        l.done = 1.0;
        l.total = 4.0;
        assert_eq!(link_status(&l), "Sending… 25%");
        l.busy = false;
        l.count = 1.0;
        assert_eq!(link_status(&l), "Saved 1 time");
        l.count = 3.0;
        assert_eq!(link_status(&l), "Saved 3 times");
        l.names.push("b.png".into());
        assert_eq!(link_title(&l), "Share 2 files");
        let r = Link { mode: "receive".into(), count: 2.0, ..Default::default() };
        assert_eq!((link_title(&r), link_status(&r)), ("Receive from a phone".to_string(), "Received 2 files".to_string()));
        assert!(link_help(&r).ends_with("choose what to send."));
    }

    #[test]
    fn keys_move_between_the_sheets_controls() {
        let tab = SharedString::from(Key::Tab);
        let left = SharedString::from(Key::LeftArrow);
        assert!(sheet_handles(&tab, -1, 0));
        assert!(sheet_handles(" ", 0, 2));
        assert!(!sheet_handles(&left, 0, 2)); // (on the ×: not a device)
        assert!(sheet_handles(&left, 2, 2));
        assert!(!sheet_handles(&left, 3, 2));
        assert!(!sheet_handles("a", 1, 2));
        assert_eq!(next_focus(-1, true, 5), 0);
        assert_eq!(next_focus(-1, false, 5), 4);
        assert_eq!(next_focus(4, true, 5), 0);
        assert_eq!(next_focus(0, false, 5), 4);
        assert_eq!(next_focus(2, true, 5), 3);
        assert_eq!(next_focus(0, true, 0), -1);
    }

    #[test]
    fn the_row_scrolls_a_device_into_the_middle() {
        // (388 px: Glass's row; 6 devices are 572 px)
        assert_eq!(row_scroll(0, 6, 388.0), 0.0);
        assert_eq!(row_scroll(3, 6, 388.0), 3.0 * 96.0 + 46.0 - 194.0);
        assert_eq!(row_scroll(5, 6, 388.0), 572.0 - 388.0);
        // fewer than fit: never scrolls
        assert_eq!(row_scroll(2, 3, 388.0), 0.0);
        assert_eq!(row_scroll(0, 0, 388.0), 0.0);
    }

    #[test]
    fn shapes_stay_in_their_box() {
        for name in ["cookie4", "square", "cookie9", "sunny", "circle"] {
            let d = outline(name);
            assert!(d.starts_with("M ") && d.ends_with('Z'), "{name}");
        }
        for v in outline("cookie4").split(|c: char| !(c.is_ascii_digit() || c == '.')).filter(|s| !s.is_empty()) {
            let n: f64 = v.parse().unwrap();
            assert!((0.0..=100.0).contains(&n));
        }
    }

    #[test]
    fn the_feed_reads_what_the_qml_sends() {
        let v = json!({
            "sheet": {"paths": ["/tmp/a.png"], "note": "", "icon": "file:///icons/image.svg", "available": true, "portsError": "", "errors": "",
                      "devices": [{"key": "adaspixel", "name": "Ada's Pixel", "type": "mobile", "cast": false, "castIcon": "", "prefer": "quickshare",
                                   "sending": {"done": 1, "total": 4, "waiting": false}, "ended": ""}]},
            "request": null,
            "link": {"mode": "send", "stage": "loading", "names": ["a.png"], "qr": "", "error": "", "busy": false, "done": 0, "total": 0, "count": 0, "installing": false, "updates": false},
            "screen": "DP-1",
            "motion": 1
        });
        let f: Feed = serde_json::from_value(v).unwrap();
        assert_eq!(f.sheet.devices[0].sending.as_ref().map(|s| frac(s.done, s.total)), Some(0.25));
        assert!(f.request.is_none());
        assert_eq!(f.link.as_ref().map(|l| l.stage.as_str()), Some("loading"));
        assert_eq!(f.screen, "DP-1");
        // (missing fields are their defaults)
        let f: Feed = serde_json::from_value(json!({})).unwrap();
        assert_eq!(f, Feed::default());
    }
}

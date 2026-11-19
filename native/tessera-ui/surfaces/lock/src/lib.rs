//! The lock screen (rust.surfaces: lock), as shell/modules/lock: Lock.qml's
//! rules, LockSurface.qml's screen, LockGlass.qml (iOS and macOS layouts)
//! and LockMaterial.qml (Android's: its clock faces, the unlock pill, the
//! fingerprint target, the light reveal), Bedside Mode (StandBy.qml: the
//! clock, the widgets, your photos, its night and its moves against burn-in,
//! the morph into it) and the login intro (LoginIntro.qml, intro.rs). Unlock
//! with the password, a finger (fprintd) or your face (Howdy), each its own
//! PAM conversation (auth.rs).
//!
//! ── Roles: which real protocol is taken when ──────────────────────────────
//! Nothing here acts on the system unless all of its conditions hold.
//! "Real" below means: not `--demo` (nor TESSERA_UI_DEMO=1), and not a
//! sandbox (TESSERA_SANDBOX ≠ 1).
//!   Session lock (ext-session-lock-v1, wayland.rs): taken when the shell
//!     says `held: true` (Lock.qml, while it locks and tessera-ui drew the
//!     lock when that lock began) and this isn't `--demo`, on whatever
//!     compositor WAYLAND_DISPLAY names (the sandbox's nested Hyprland
//!     there: a separate compositor). Let go (unlock_and_destroy) only when
//!     the shell says `held: false`: after an unlock here (said to the shell
//!     as `{"kind":"lock","unlocked":true}`), or the sandbox's own `ipc lock
//!     unlock`. `--demo` draws the same screens on overlay layer surfaces
//!     ("tessera:lock") instead: nothing is locked.
//!   Password (PAM): the system's `login` service only when real; else a
//!     test service of the test's own (TESSERA_LOCK_PAM_DIR +
//!     TESSERA_LOCK_PAM_SERVICE, pam_start_confdir), else a mock that knows
//!     only TESSERA_LOCK_TEST_PASSWORD (auth.rs). So a sandbox never runs
//!     PAM against your account.
//!   Fingerprint and face (PAM: tessera-fingerprint, tessera-face from
//!     shell/assets/pam): only when real and the shell says the reader or
//!     your faces are set up (`finger`, `face`: Fingerprint.lockScreen,
//!     Face.lockScreen). In a sandbox the `demo` lines show their states.
//!   logind's PrepareForSleep and LidClosed (system bus, signal watches,
//!     read-only), and a "sleep" delay inhibitor while locked: only when real
//!     and the reader or the face is on. fprintd's GetDevices and a Claim +
//!     Release of the reader before each fresh scan, and before a sleep is
//!     let go; /sys/class/drm's dpms (read-only) for a screen coming on;
//!     systemd's RestartUnit("fprintd.service"), never interactive, only when
//!     fprintd is stuck and at most once in 2 minutes (allowed by Tessera's
//!     polkit rule, else refused): reader.rs.
//!   faillock --user (read-only): only when real, after a wrong password.
//!   Idle inhibitor (zwp_idle_inhibit_v1) on a lock surface while Bedside
//!     Mode shows: whenever it shows (demo too: the arena's compositor).
//!
//! ── With the shell (Lock.qml, LoginIntro.qml; JSON lines) ─────────────────
//!   stdin  {"to":"lock","held":…,"leaving":…,"finger":…,"face":…}
//!          {"to":"lock","notes":[{key,app,icon,summary,body,at}…]}
//!          {"to":"lock","demo":"standby|wake|finger|reading|miss|match|face|face-reading|face-miss|face-match|typing|wrong|busy|lockout|off"}
//!          {"to":"lock","intro":"arm|play|stop","pace":1}
//!   stdout {"src":"ui","kind":"lock","secure":true|false}  the compositor locked (System.lockSecure)
//!          {"src":"ui","kind":"lock","unlocked":true}       a password, finger or face matched
//!          {"src":"ui","kind":"lock","refused":true}        the compositor refused the lock
//!          {"src":"ui","kind":"lock","sound":"error"}       a wrong password (Sounds.play)
//! What stays the shell's: when to lock (System.locked, the 5-minute idle
//! lock while charging), the veil and the desktop's settle (LockFx), the
//! startup chime.
//!
//! ── Not like the QML yet ──────────────────────────────────────────────────
//! Glass's cards are the wallpaper blurred under a tint and a light rim, not
//! the QML's refracting liquid glass (Slint has no shaders); the big clock
//! has no soft shadow; Material's clocks lose the font's width and roundness
//! axes (Slint sets only the weight), so "wide" is as wide as the default
//! width allows; Material's shapes morph by mixing outlines (close to
//! MaterialShape's); the large calendar in Bedside Mode is a month view
//! only (the QML's is Notification Center's, with Week and To-do); the wave
//! in Material's Now Playing is quadratic arcs (the QML's a sine); the
//! wallpaper is read again when the settings change, not as a time-of-day
//! picture turns over; in the sandbox the password is checked by the mock
//! ("mock"; the QML refuses every password there).
//! Parity notes, per QML file, with the tester's steps:
//! native/tessera-ui/parity/shell_modules_lock_*.md

pub mod ui {
    slint::include_modules!();
}
tessera_core::theme_global!(ui);

// tessera-core's modules (shell, wayland, theme, services, util…) under the
// names they had when everything was one crate: `crate::shell::Part`
#[allow(unused_imports)]
use tessera_core::*;

pub mod auth;
pub mod intro;
pub mod logic;
pub mod reader;
pub mod shape;
pub mod veil;

use tessera_core::services::calendar::Calendar;
use tessera_core::services::mpris::{self, Players};
use tessera_core::services::status::Battery;
use tessera_core::services::todos::Todos;
use tessera_core::services::weather::Weather;
use tessera_core::shell::Part;
use tessera_core::wayland::{LockEvent, Spec, State, Surface};
use auth::{Backend, Conv, Event as Auth, Kind, Outcome};
use slint::{ComponentHandle, Image, ModelRc, VecModel};
use tessera_core::rows::Rows;
use smithay_client_toolkit::reexports::client::protocol::wl_output::WlOutput;
use smithay_client_toolkit::reexports::protocols::wp::idle_inhibit::zv1::client::zwp_idle_inhibitor_v1::ZwpIdleInhibitorV1;
use smithay_client_toolkit::session_lock::SessionLock;
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use std::any::{Any, TypeId};
use std::rc::Rc;
use std::time::Duration;

const NAME: &str = "lock";

const NS_DEMO: &str = "tessera:lock";
const WAITING: &str = "Touch the fingerprint reader to unlock";

/// Real: neither `--demo` nor a sandbox (the top comment's roles)
pub fn demo_mode() -> bool {
    tessera_core::env::demo_mode()
}
pub fn sandboxed() -> bool {
    tessera_core::env::sandboxed()
}

/// The Material clock's widths and roundness (LockData.flex): Google Sans
/// Flex fixed at each (tools/gen-clock-fonts.sh), registered once
pub fn clock_fonts() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        use slint::fontique_011::fontique;
        let mut collection = slint::fontique_011::shared_collection();
        for w in [25, 50, 75, 100, 125, 151] {
            for round in ["", "-Round"] {
                let rel = format!("material/assets/fonts/clock/TesseraClock-{w}{round}.ttf");
                let Some(data) = tessera_core::services::apps::shell_file(&rel).and_then(|p| std::fs::read(p).ok()) else {
                    eprintln!("tessera-ui: font {rel} not found");
                    continue;
                };
                collection.register_fonts(fontique::Blob::new(std::sync::Arc::new(data)), None);
            }
        }
    });
}

/// Weather icon names -> the glyph each look draws (Phosphor; Material
/// Symbols), as Today draws them (its crate's `glyph`)
pub fn weather_glyph(name: &str, material: bool) -> &'static str {
    let (p, m) = match name {
        "sun" => ("\u{e472}", "\u{e518}"),
        "moon-stars" => ("\u{e58e}", "\u{f159}"),
        "cloud-moon" => ("\u{e53e}", "\u{f174}"),
        "cloud-sun" => ("\u{e540}", "\u{f172}"),
        "cloud-fog" => ("\u{e53c}", "\u{e818}"),
        "cloud-lightning" => ("\u{e1b2}", "\u{ebdb}"),
        "cloud-snow" => ("\u{e1b8}", "\u{e2cd}"),
        "cloud-rain" => ("\u{e1b4}", "\u{f176}"),
        _ => ("\u{e1aa}", "\u{f15c}"), // cloud
    };
    if material { m } else { p }
}

/// Your full name (passwd's), else the login (Account.qml)
pub fn account_name() -> String {
    unsafe {
        let uid = libc::getuid();
        let mut pw: libc::passwd = std::mem::zeroed();
        let mut out: *mut libc::passwd = std::ptr::null_mut();
        let mut buf = vec![0 as libc::c_char; 4096];
        if libc::getpwuid_r(uid, &mut pw, buf.as_mut_ptr(), buf.len(), &mut out) != 0 || out.is_null() {
            return std::env::var("USER").unwrap_or_default();
        }
        let s = |p: *const libc::c_char| if p.is_null() { String::new() } else { std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned() };
        let full = s(pw.pw_gecos).split(',').next().unwrap_or("").trim().to_string();
        if full.is_empty() { s(pw.pw_name) } else { full }
    }
}

/// An answer is the running password check's (ids start at 1; 0: none runs)
fn is_pass(id: u64, pass_id: u64) -> bool {
    pass_id != 0 && id == pass_id
}

enum M {
    /// (the screen typed on, the field's text)
    Edited(String, String),
    Submit,
    Woke,
    Step(i32),
    Player(String),
    Second,
    IdleOver,
    /// a reader's match has been seen (the island's green tick): unlock now
    MatchShown,
    /// a password check that never answered (its conversation id)
    PassTimeout(u64),
    /// the reader's wait is over (a pause, a back-off, a stuck reader's 30 s):
    /// a fresh scan, for `Reader::retry_why`
    FingerRetry,
    FingerMissOver,
    /// fprintd's answer before a fresh scan (its generation, why it was asked)
    Probed(u64, reader::Why, reader::Probe),
    /// logind's sleep delay inhibitor, taken off the UI thread
    Inhibitor(Option<zbus::zvariant::OwnedFd>),
    /// the lid was opened (logind)
    LidOpened,
    /// the camera's "not recognised" has shown long enough (the island's miss)
    FaceMissOver,
    FaceRetry,
    RestOver,
    PhotoNext,
    Sleep(bool),
    Lockout(logic::Faillock, Option<String>),
    Wallpaper(String, (u32, u32), Option<(slint::SharedPixelBuffer<slint::Rgb8Pixel>, slint::SharedPixelBuffer<slint::Rgb8Pixel>)>),
    Photo(String, Option<slint::SharedPixelBuffer<slint::Rgb8Pixel>>),
    Avatar(Option<(Vec<u8>, Vec<u8>)>),
    Art(String, Option<(Vec<u8>, Vec<u8>)>),
    /// the player's position (bus, µs), read once a second while its timeline shows
    Position(String, i64),
    /// the compositor ended the lock while it's still wanted: take it again
    Retake,
    /// the wallpaper is late: play the entrance without it
    RevealNow,
}

struct Win {
    output: String,
    surface: Rc<Surface>,
    ui: ui::LockScreen,
    a11y: Option<crate::a11y::A11y>,
    inhibitor: Option<ZwpIdleInhibitorV1>,
    revealed: bool,
    /// shown, its entrance waiting for the wallpaper (LockSurface.qml's
    /// Image is there before the lock shows; here it decodes after)
    waiting: bool,
    /// the wallpaper decoded for this size (path, size), and the picture (the veil shows it too)
    picture: (String, (u32, u32)),
    sharp: Option<Image>,
    own_fy: f64,
    /// the notification cards this screen shows (by its shape)
    cards: Rows<ui::LNote>,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Note {
    key: String,
    app: String,
    icon: String,
    summary: String,
    body: String,
    at: i64,
}

/// The fingerprint reader's scans (reader.rs has the rules)
struct Reader {
    text: String,
    state: String,
    /// real misses since the last wake or unlock (3: a pause; 6: the password only)
    tries: u32,
    asked_at: i64,
    started_at: i64,
    asked: bool,
    /// scans over at once in a row, before they asked for a finger
    busy: u32,
    conv: Option<Conv>,
    miss_shown: bool,
    /// the last finger, key, pointer, touch, lid or screen: 2 minutes without, no scan
    activity_at: i64,
    /// fprintd is being asked (a fresh scan's thread): `gen` tells its answer from an older one's
    probing: bool,
    gen: u64,
    /// what the next FingerRetry asks fprintd for
    retry_why: reader::Why,
    /// fprintd is stuck and couldn't be restarted: the hint shows
    stuck: bool,
    /// when fprintd was last restarted (at most once in 2 minutes)
    restarted_at: Option<i64>,
    /// the post-unlock notification is owed
    tell_after: bool,
    /// a scan was wanted at the last look (the 1 s tick sees idle begin)
    wanted: bool,
    /// the screens' power at the last tick (one coming on starts a scan)
    screens: Vec<(String, bool)>,
}

impl Default for Reader {
    fn default() -> Self {
        Self {
            text: String::new(),
            state: String::new(),
            tries: 0,
            asked_at: 0,
            started_at: 0,
            asked: false,
            busy: 0,
            conv: None,
            miss_shown: false,
            activity_at: crate::util::now_ms(),
            probing: false,
            gen: 0,
            retry_why: reader::Why::Fresh,
            stuck: false,
            restarted_at: None,
            tell_after: false,
            wanted: false,
            screens: Vec::new(),
        }
    }
}

#[derive(Default)]
struct FaceCam {
    text: String,
    tries: u32,
    broken: bool,
    armed: bool,
    stopped_at: i64,
    conv: Option<Conv>,
    /// the blink check spoke in this conversation (a regular camera)
    checked: bool,
    /// a "not recognised" showing (the island's miss, 1.3 s)
    missed: bool,
    /// it matched (the island's match, on the way out)
    matched: bool,
    /// the sandbox's `demo` phase (waiting, reading, match, miss)
    demo: String,
}

#[derive(Default)]
struct Bedside {
    demo_charging: bool,
    idle: bool,
    shown: bool,
    fy: f64,
    drifts: u32,
    resting: bool,
    minutes: u32,
    view: i32,
    dots: i32,
    photos: Vec<String>,
    photo: usize,
}

/// What a lock that panicked hands the next one (Part::hand_over)
struct HandOver {
    lock: Option<SessionLock>,
    finger_on: bool,
    face_on: bool,
    notes: Vec<Note>,
}

pub struct Lock {
    demo: bool,
    /// the look is Material (settings)
    material: bool,
    /// the wallpaper showing now (read with the settings)
    wall_path: String,
    real: bool,
    backend: Backend,
    held: bool,
    leaving: bool,
    finger_on: bool,
    face_on: bool,
    notes: Vec<Note>,
    lock: Option<SessionLock>,
    secure: bool,
    wins: Vec<Win>,
    password: String,
    /// the screen whose field the password came from (never written back there:
    /// its edits arrive later than its keys, and an older text would undo the newest)
    typed_on: Option<String>,
    error: String,
    busy: bool,
    fails: u32,
    pam_locked: bool,
    error_count: i32,
    conv_id: u64,
    /// the password check's own id. 20 Nov (system1): it was compared with
    /// conv_id, which the reader's and the camera's retries move on every
    /// few seconds; a check that took longer than the next retry had its
    /// answer (and its 20 s stop) dropped, so the lock said "checking"
    /// for good after the right password
    pass_id: u64,
    pass_conv: Option<Conv>,
    finger: Reader,
    face: FaceCam,
    sb: Bedside,
    held_at: i64,
    last_touch: i64,
    tick_ms: i64,
    minute: (i32, i32),
    sleeping: bool,
    sleep_watch: bool,
    /// logind's sleep delay inhibitor while a scan can run (reader.rs), and
    /// whether it's being taken
    sleep_fd: Option<zbus::zvariant::OwnedFd>,
    sleep_fd_asked: bool,
    faillock: logic::Faillock,
    avatar: Option<(Image, Image)>,
    avatar_asked: bool,
    art: (String, Option<(Image, Image)>),
    t_second: slint::Timer,
    t_idle: slint::Timer,
    t_finger: slint::Timer,
    t_finger_miss: slint::Timer,
    t_face_miss: slint::Timer,
    t_face: slint::Timer,
    t_rest: slint::Timer,
    t_photo: slint::Timer,
    intro: intro::Intro,
    veil: veil::Veil,
    /// the lists every screen shows, changed in place (tessera_core::rows)
    notes_rows: Rows<ui::LNote>,
    apps_rows: Rows<ui::LNote>,
    days_rows: Rows<ui::LDay>,
    /// notification icons, loaded once each
    icons: std::collections::HashMap<String, Option<Image>>,
    /// the hands' angles, only ever growing (they turn forward past 12)
    angles: (f32, f32),
    /// the player whose position is read now (the reader thread's target)
    position_for: std::sync::Arc<std::sync::Mutex<Option<String>>>,
    position_thread: bool,
    /// the position (µs) and the track's length (µs) of the shown player
    position: (String, i64),
    /// tries at taking the lock again after the compositor ended it
    retakes: u32,
    t_retake: slint::Timer,
    /// holds the unlock while a reader's match shows
    t_match: slint::Timer,
    /// stops a password check that never answers
    t_pass: slint::Timer,
}

impl Default for Lock {
    fn default() -> Self {
        let demo = demo_mode();
        let real = !demo && !sandboxed();
        Self {
            demo,
            material: false,
            wall_path: String::new(),
            real,
            backend: auth::backend(real),
            held: false,
            leaving: false,
            finger_on: false,
            face_on: false,
            notes: Vec::new(),
            lock: None,
            secure: false,
            wins: Vec::new(),
            password: String::new(),
            typed_on: None,
            error: String::new(),
            busy: false,
            fails: 0,
            pam_locked: false,
            error_count: 0,
            conv_id: 0,
            pass_id: 0,
            pass_conv: None,
            finger: Reader::default(),
            face: FaceCam::default(),
            sb: Bedside::default(),
            held_at: 0,
            last_touch: 0,
            tick_ms: 0,
            minute: (-1, -1),
            sleeping: false,
            sleep_watch: false,
            sleep_fd: None,
            sleep_fd_asked: false,
            faillock: logic::Faillock::default(),
            avatar: None,
            avatar_asked: false,
            art: (String::new(), None),
            t_second: slint::Timer::default(),
            t_idle: slint::Timer::default(),
            t_finger: slint::Timer::default(),
            t_finger_miss: slint::Timer::default(),
            t_face_miss: slint::Timer::default(),
            t_face: slint::Timer::default(),
            t_rest: slint::Timer::default(),
            t_photo: slint::Timer::default(),
            intro: intro::Intro::default(),
            veil: veil::Veil::default(),
            notes_rows: Rows::new(),
            apps_rows: Rows::new(),
            days_rows: Rows::new(),
            icons: Default::default(),
            angles: (-1.0, -1.0),
            position_for: Default::default(),
            position_thread: false,
            position: (String::new(), 0),
            retakes: 0,
            t_retake: slint::Timer::default(),
            t_match: slint::Timer::default(),
            t_pass: slint::Timer::default(),
        }
    }
}

/// An angle that only grows: `a` (0..360) the next turn on from `prev`
/// (RotationAnimation.Clockwise: 354° → 0° is 6° forward, not 354° back)
pub fn forward(prev: f32, a: f32) -> f32 {
    if prev < 0.0 {
        return a;
    }
    let base = prev - prev.rem_euclid(360.0);
    let mut v = base + a;
    if v < prev - 0.01 {
        v += 360.0;
    }
    v
}

/// The MPRIS player's Position (µs), read off the session bus
fn read_position(conn: &zbus::blocking::Connection, bus: &str) -> Option<i64> {
    let p = zbus::blocking::Proxy::new(conn, bus.to_string(), "/org/mpris/MediaPlayer2", "org.mpris.MediaPlayer2.Player").ok()?;
    p.get_property::<i64>("Position").ok()
}

fn cfg_lock(st: &State) -> &serde_json::Value {
    &st.shell.cfg["lock"]
}
fn s<'a>(v: &'a serde_json::Value, k: &str, d: &'a str) -> &'a str {
    v[k].as_str().unwrap_or(d)
}
fn b(v: &serde_json::Value, k: &str, d: bool) -> bool {
    v[k].as_bool().unwrap_or(d)
}

/// A screen's entrance, once it's shown (`waiting`)
fn reveal(w: &mut Win) {
    if w.waiting && !w.revealed {
        w.waiting = false;
        w.revealed = true;
        w.ui.global::<ui::LockData>().set_revealed(true);
    }
}

fn hex(c: &str) -> slint::Color {
    let v = u32::from_str_radix(c.trim_start_matches('#'), 16).unwrap_or(0);
    slint::Color::from_rgb_u8((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

/// A picture for the screen and its blurred copy (the cards' frost), for a
/// box `w`×`h` (pixels)
fn decode_pair(path: &str, w: u32, h: u32) -> Option<(slint::SharedPixelBuffer<slint::Rgb8Pixel>, slint::SharedPixelBuffer<slint::Rgb8Pixel>)> {
    let sharp = crate::picture::decode(path, w, h)?;
    let (sw, sh) = (sharp.width(), sharp.height());
    let img = image::RgbImage::from_raw(sw, sh, sharp.as_bytes().to_vec())?;
    // (a quarter size, blurred, a touch more saturated: LockSurface's frost)
    let small = image::imageops::resize(&img, (sw / 4).max(1), (sh / 4).max(1), image::imageops::FilterType::Triangle);
    let mut soft = image::imageops::blur(&small, 8.0);
    for p in soft.pixels_mut() {
        let l = 0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32;
        for c in 0..3 {
            p[c] = (l + (p[c] as f32 - l) * 1.2).clamp(0.0, 255.0) as u8;
        }
    }
    let soft = slint::SharedPixelBuffer::<slint::Rgb8Pixel>::clone_from_slice(soft.as_raw(), soft.width(), soft.height());
    Some((sharp, soft))
}

/// How light the blurred wallpaper is where Glass's password field sits
/// (the bottom middle): its darkest and brightest few percent, 0..1, so the
/// field's glass takes a tint just strong enough for its words to read
fn field_light(soft: &slint::SharedPixelBuffer<slint::Rgb8Pixel>) -> (f32, f32) {
    let (w, h) = (soft.width() as usize, soft.height() as usize);
    let px = soft.as_slice();
    let mut l: Vec<u8> = Vec::new();
    // (from the island's top, grown round a reader, to under the field)
    for y in (h * 72 / 100)..(h * 96 / 100).max(h * 72 / 100 + 1).min(h) {
        for x in (w * 40 / 100)..(w * 60 / 100).max(w * 40 / 100 + 1).min(w) {
            let p = px[y * w + x];
            l.push((0.2126 * p.r as f32 + 0.7152 * p.g as f32 + 0.0722 * p.b as f32) as u8);
        }
    }
    if l.is_empty() {
        return (0.1, 0.1);
    }
    l.sort_unstable();
    let at = |q: usize| l[(l.len() - 1) * q / 100] as f32 / 255.0;
    (at(3), at(97))
}

/// Your picture (~/.face, else AccountsService's), square and in a cookie
fn avatar_files() -> Option<(Vec<u8>, Vec<u8>)> {
    let home = std::env::var("HOME").unwrap_or_default();
    let user = auth::user_name();
    let files = [format!("{home}/.face"), format!("/var/lib/AccountsService/icons/{user}")];
    let side = 128;
    let px = files.iter().find_map(|f| shape::square_picture(f, side))?;
    let round = shape::cut(&px, side, "circle");
    let cookie = shape::cut(&px, side, "cookie9");
    Some((round, cookie))
}

fn art_files(url: &str) -> Option<(Vec<u8>, Vec<u8>)> {
    let path = url.strip_prefix("file://")?;
    let side = 120;
    let px = shape::square_picture(path, side)?;
    Some((shape::cut(&px, side, "cookie9"), shape::cut(&px, side, "circle")))
}

impl Lock {
    fn send(st: &State) -> impl Fn(Box<dyn Any + Send>) + Clone + Send + 'static {
        st.shell.sender(NAME)
    }

    fn each(&self, f: impl Fn(&ui::LockData)) {
        for w in &self.wins {
            f(&w.ui.global::<ui::LockData>());
        }
    }

    fn standby(&self, st: &State) -> bool {
        self.held && !self.leaving && self.can_standby(st) && self.sb.idle && self.password.is_empty() && !self.busy
    }

    fn can_standby(&self, st: &State) -> bool {
        let charging = st.shell.get::<Battery>().is_some_and(|b| b.present && (b.state == 1 || b.state == 4));
        b(cfg_lock(st), "standby", true) && (charging || self.sb.demo_charging)
    }

    // ── Lock and unlock ────────────────────────────────────────────────────

    fn set_held(&mut self, st: &mut State, held: bool) {
        if held == self.held {
            return;
        }
        self.held = held;
        // (a new lock, or the end of one: everything starts over)
        let now = crate::util::now_ms();
        self.held_at = now;
        self.last_touch = now;
        self.face = FaceCam::default();
        // (an unlock, or a new lock: the misses count from zero)
        self.finger.tries = 0;
        self.finger.busy = 0;
        self.finger.activity_at = now;
        if !held && self.finger.stuck && std::mem::take(&mut self.finger.tell_after) {
            // unlocked with the password while fprintd is stuck: offer the restart
            reader::offer_restart();
        }
        self.sb.idle = false;
        self.password.clear();
        self.typed_on = None;
        if held {
            eprintln!("tessera-ui: lock: held ({})", if self.demo { "demo: overlay surfaces" } else { "session lock" });
            self.leaving = false;
            self.take(st);
            self.t_second.stop();
            let to = Self::send(st);
            self.t_second.start(slint::TimerMode::Repeated, Duration::from_secs(1), move || to(Box::new(M::Second)));
            self.tick_ms = now;
        } else {
            self.release(st);
            self.t_second.stop();
            self.t_retake.stop();
            self.retakes = 0;
            self.pass_conv = None;
            self.busy = false;
            self.error.clear();
            self.angles = (-1.0, -1.0);
            // (the position reader rests)
            self.player(st);
        }
        self.rules(st);
    }

    /// Lock the session (or, in `--demo`, cover the screens) and make a
    /// screen for every output
    fn take(&mut self, st: &mut State) {
        // (a lock handed over by the lock before this one, which panicked:
        // still locked, it's kept: `start`)
        if !self.demo && self.lock.is_none() {
            match st.lock_session() {
                Some(l) => self.lock = Some(l),
                None => {
                    eprintln!("tessera-ui: lock: this compositor has no ext-session-lock-v1");
                    crate::shell::tell(NAME, serde_json::json!({"refused": true}));
                    return;
                }
            }
        }
        for (o, name) in st.outputs_now() {
            self.window(st, &o, &name);
        }
        if !self.avatar_asked {
            self.avatar_asked = true;
            let to = Self::send(st);
            std::thread::spawn(move || to(Box::new(M::Avatar(avatar_files()))));
        }
        self.photos(st);
    }

    fn release(&mut self, st: &mut State) {
        if let Some(l) = self.lock.take() {
            // (unlock_and_destroy once locked; a lock not yet granted is
            // only destroyed, as the protocol asks)
            l.unlock();
        }
        for w in std::mem::take(&mut self.wins) {
            if let Some(i) = w.inhibitor {
                i.destroy();
            }
            st.drop_surface(&w.surface);
        }
        if self.secure {
            self.secure = false;
            crate::shell::tell(NAME, serde_json::json!({"secure": false}));
        }
    }

    fn window(&mut self, st: &mut State, output: &WlOutput, name: &str) {
        if self.wins.iter().any(|w| w.output == name) {
            return;
        }
        let surface = match &self.lock {
            Some(l) => {
                let l = l.clone();
                st.prepare_lock_surface(&l, output, name)
            }
            None if self.demo => st.prepare_surface(Spec {
                layer: Layer::Overlay,
                namespace: NS_DEMO,
                output: Some((output, name)),
                anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
                size: (0, 0),
                exclusive: -1,
                keyboard: KeyboardInteractivity::Exclusive,
                margin_left: 0,
            }),
            None => return,
        };
        clock_fonts();
        let ui = ui::LockScreen::new().expect("LockScreen");
        st.shell.apply_theme(&ui.global::<ui::Theme>());
        ui.global::<ui::Symbols>().on_of(|g| crate::icons::symbol(&g).into());
        let d = ui.global::<ui::LockData>();
        let outline = |n: &str| shape::outline(n, n, 0.0, 0.0).into();
        d.set_cookie9(outline("cookie9"));
        d.set_cookie12(outline("cookie12"));
        d.set_sunny(outline("sunny"));
        d.set_circle(outline("circle"));
        let to = Self::send(st);
        macro_rules! on {
            ($set:ident, |$($a:ident),*| $m:expr) => {{
                let to = to.clone();
                d.$set(move |$($a),*| to(Box::new($m)));
            }};
        }
        {
            let to = to.clone();
            let on = name.to_string();
            d.on_edited(move |t| to(Box::new(M::Edited(on.clone(), t.to_string()))));
        }
        on!(on_submit, | | M::Submit);
        on!(on_woke, | | M::Woke);
        on!(on_step, |i| M::Step(i));
        on!(on_player, |a| M::Player(a.to_string()));
        // (the lists, handed over once: every refresh after changes them in place)
        let cards = Rows::new();
        d.set_notes(self.notes_rows.model());
        d.set_apps(self.apps_rows.model());
        d.set_days(self.days_rows.model());
        d.set_cards(cards.model());
        self.wins.push(Win { output: name.to_string(), surface, ui, a11y: None, inhibitor: None, revealed: false, waiting: false, picture: (String::new(), (0, 0)), sharp: None, own_fy: 0.0, cards });
        let i = self.wins.len() - 1;
        // (filled before its first frame: the hands and the clock start in place, not turning into it)
        self.fill(st, i);
        let w = &mut self.wins[i];
        w.ui.show().expect("show lock");
        w.a11y = crate::a11y::A11y::new(&w.surface, st.tx.clone());
    }

    /// Everything a new screen shows
    fn fill(&mut self, st: &mut State, i: usize) {
        self.settings(st);
        self.clock(st, true);
        self.widgets(st);
        self.notes_push();
        self.player(st);
        self.field();
        self.bedside(st);
        if let Some((round, cookie)) = &self.avatar {
            let d = self.wins[i].ui.global::<ui::LockData>();
            d.set_avatar(round.clone());
            d.set_avatar_cookie(cookie.clone());
            d.set_has_avatar(true);
        }
        if let Some((cookie, round)) = &self.art.1 {
            let d = self.wins[i].ui.global::<ui::LockData>();
            d.set_art_cookie(cookie.clone());
            d.set_art(round.clone());
            d.set_has_art(true);
        }
    }

    /// The settings and the look
    fn settings(&mut self, st: &State) {
        let c = cfg_lock(st).clone();
        let look = &st.shell.look;
        self.material = look.material;
        // (lock.material "system" follows the glass tier: Reduce
        // transparency or a solid preset is solid, a frosted one frost)
        let appearance = &st.shell.cfg["appearance"];
        let plate = match s(&c, "material", "system") {
            "system" if b(appearance, "reduceTransparency", false) => "solid",
            "system" => match s(appearance, "glass", "liquid") {
                "frosted" => "frost",
                "solid" => "solid",
                _ => "glass",
            },
            m => m,
        }
        .to_string();
        let top_light = st.shell.get::<crate::services::wallpaper::WallTop>().is_some_and(|t| t.light);
        let dark_ink = match s(&c, "text", "auto") {
            "dark" => true,
            "auto" => top_light,
            _ => false,
        };
        let roles: std::collections::HashMap<String, String> = if look.material { crate::theme::look_roles(&look).into_iter().collect() } else { Default::default() };
        let r = |k: &str| roles.get(k).map(|v| hex(v)).unwrap_or(slint::Color::from_rgb_u8(255, 255, 255));
        let name = account_name();
        let h24 = b(&st.shell.cfg["appearance"], "clock24", false);
        for w in &self.wins {
            st.shell.apply_theme(&w.ui.global::<ui::Theme>());
            let d = w.ui.global::<ui::LockData>();
            d.set_ios(s(&c, "layout", "ios") != "macos");
            d.set_plate(plate.clone().into());
            d.set_dark_ink(dark_ink);
            d.set_note_style(s(&c, "notifications", "shelf").into());
            d.set_previews(b(&c, "previews", false));
            d.set_lock_fx(s(&c, "lockTransition", "zoom").into());
            d.set_zoom_out(s(&c, "unlockTransition", "zoom") == "zoom");
            d.set_sb_mode(s(&c, "standbyTransition", "morph").into());
            d.set_sb_opacity(c["standbyClockOpacity"].as_f64().unwrap_or(0.7).clamp(0.2, 1.0) as f32);
            d.set_sb_weight(logic::standby_weight(s(&c, "standbyClockWeight", "thin"), look.material));
            d.set_clock_face(s(&c, "clockFace", "dynamic").into());
            d.set_compact(s(&c, "clockSize", "large") == "small");
            d.set_clock_weight(c["clockWeight"].as_i64().unwrap_or(640).clamp(100, 1000) as i32);
            d.set_clock_width(c["clockWidth"].as_i64().unwrap_or(112).clamp(25, 151) as i32);
            d.set_clock_round(b(&c, "clockRound", true));
            d.set_h24(h24);
            d.set_wake_on_mouse(b(&c, "standbyWakeOnMouse", true));
            d.set_name(name.clone().into());
            d.set_reduce_motion(b(&st.shell.cfg["appearance"], "reduceMotion", false));
            if look.material {
                d.set_m_clock_ink(if dark_ink { r("onPrimaryFixed") } else { r("primaryFixed") });
                d.set_m_clock_ink2(if dark_ink { r("onPrimaryFixedVariant") } else { r("primaryFixedDim") });
                d.set_m_clock_ink3(if dark_ink { r("onTertiaryFixed") } else { r("tertiaryFixed") });
                d.set_m_ink(if dark_ink { r("onSecondaryFixed") } else { r("secondaryFixed") });
                d.set_m_ink2(if dark_ink { r("onSecondaryFixedVariant") } else { r("secondaryFixedDim") });
                d.set_sb_m_ink(r("primaryFixed"));
                d.set_sb_m_ink2(r("primaryFixedDim"));
                d.set_sb_m_ink3(r("secondaryFixedDim"));
            }
        }
    }

    /// The time and date (`force`: even within the same minute)
    fn clock(&mut self, st: &State, force: bool) {
        let now = crate::util::now_ms();
        let t = crate::services::calendar::local(now);
        if !force && self.minute == (t.h, t.mi) {
            return;
        }
        self.minute = (t.h, t.mi);
        let h24 = b(&st.shell.cfg["appearance"], "clock24", false);
        let (h, m) = (t.h as u32, t.mi as u32);
        let date = logic::date_long(t.wday as u32, t.d as u32, t.mo as u32);
        let hh = logic::hour(h, h24);
        let half = if h24 { logic::date_short(t.d as u32, t.mo as u32) } else if h < 12 { "AM".into() } else { "PM".into() };
        let (sunrise, sunset) = st.shell.get::<Weather>().filter(|w| w.ready).map(|w| (w.sunrise, w.sunset)).unwrap_or_else(crate::wallpaper::sun);
        let day = crate::wallpaper::Day::now(sunrise, sunset).is_day();
        let night = logic::night(day, h);
        // the month for Bedside Mode's calendar
        self.calendar(st);
        self.angles = (forward(self.angles.0, ((h % 12) as f32 + m as f32 / 60.0) * 30.0), forward(self.angles.1, m as f32 * 6.0));
        let (ah, am) = self.angles;
        self.each(|d| {
            d.set_date(date.clone().into());
            d.set_time(logic::clock(h, m, h24).into());
            d.set_hh(hh.clone().into());
            d.set_hh2(format!("{:0>2}", hh).into());
            d.set_mm(format!("{m:02}").into());
            d.set_half(half.clone().into());
            d.set_angle_h(ah);
            d.set_angle_m(am);
            d.set_sb_night(night);
        });
        // (the notes' "5m ago" moves with the clock)
        if !force {
            self.notes_push();
        }
    }

    /// Bedside Mode's calendar (Notification Center's, large, as the desktop
    /// widget's preview: this month's six weeks with today chosen): the
    /// dates with their dots, what repeats every day, today's events and
    /// to-dos (host today/notif.rs calendar_payload, the same rules)
    fn calendar(&self, st: &State) {
        use crate::services::calendar as cal;
        let now = crate::util::now_ms();
        let t = cal::local(now);
        let h24 = b(&st.shell.cfg["appearance"], "clock24", false);
        let first = cal::local(cal::make(t.y, t.mo, 1, 12, 0, 0));
        let lead = logic::lead(first.wday);
        let (from, to) = (cal::make(t.y, t.mo, 1 - lead, 0, 0, 0), cal::make(t.y, t.mo, 1 - lead + 42, 0, 0, 0));
        let c = st.shell.get::<Calendar>();
        let list = c.map(|c| c.between(from, to)).unwrap_or_default();
        let daily = cal::daily_in(&list, from, to);
        let accent = st.shell.look.accent.clone();
        let colour = |e: &cal::Event| if e.color.is_empty() { accent.clone() } else { e.color.clone() };
        // day start -> (event colours, a to-do due)
        let mut marks: std::collections::HashMap<i64, (Vec<String>, bool)> = std::collections::HashMap::new();
        for e in list.iter().filter(|e| !daily.contains_key(&cal::series_of(e))) {
            let last = if e.all_day { e.end - 1 } else { e.end };
            let (mut d, end) = (cal::day_bounds(e.start).0, cal::day_bounds(last.max(e.start)).0);
            while d <= end {
                let m = marks.entry(d).or_default();
                let k = colour(e);
                if !m.0.contains(&k) {
                    m.0.push(k);
                }
                d = cal::day_bounds(d).1;
            }
        }
        let todos = st.shell.get::<Todos>().map(|t| t.list.clone()).unwrap_or_default();
        for td in todos.iter().filter(|td| td.due != 0 && !td.done) {
            marks.entry(cal::day_bounds(td.due).0).or_default().1 = true;
        }
        let days: Vec<ui::LDay> = (0..42)
            .map(|i| {
                let ms = cal::make(t.y, t.mo, 1 - lead + i, 12, 0, 0);
                let dt = cal::local(ms);
                let m = marks.get(&cal::day_bounds(ms).0);
                let colors: Vec<slint::Color> = m.map(|m| m.0.iter().take(3).map(|c| hex(c)).collect()).unwrap_or_default();
                ui::LDay {
                    label: dt.d.to_string().into(),
                    col: i % 7,
                    row: i / 7,
                    today: (dt.y, dt.mo, dt.d) == (t.y, t.mo, t.d),
                    in_month: dt.mo == t.mo,
                    colors: ModelRc::new(VecModel::from(colors)),
                    todos: m.is_some_and(|m| m.1),
                }
            })
            .collect();
        self.days_rows.set(days);
        // today: its events, then the to-dos due today
        let hm = |ms: i64| {
            let l = cal::local(ms);
            logic::hm(l.h as u32, l.mi as u32, h24)
        };
        let (a, z) = cal::day_bounds(now);
        let mut items: Vec<ui::LItem> = c
            .map(|c| {
                c.between(a, z)
                    .iter()
                    .map(|e| {
                        let when = if e.all_day { "All day".to_string() } else if e.end > e.start { format!("{} – {}", hm(e.start), hm(e.end)) } else { hm(e.start) };
                        let name = cal::name_of(c, e);
                        ui::LItem {
                            event: true,
                            title: e.title.clone().into(),
                            sub: if name.is_empty() { when } else { format!("{when} · {name}") }.into(),
                            color: hex(&colour(e)),
                            done: false,
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        items.extend(todos.iter().filter(|td| td.due >= a && td.due < z).map(|td| ui::LItem {
            event: false,
            title: td.text.clone().into(),
            sub: if td.all_day { "To-do".to_string() } else { format!("To-do · {}", hm(td.due)) }.into(),
            color: slint::Color::default(),
            done: td.done,
        }));
        items.truncate(8);
        let every = daily.values().cloned().collect::<Vec<_>>().join(", ");
        let month = format!("{} {}", &logic::month_name(t.mo as u32)[..3], t.y);
        self.each(|d| {
            d.set_month(month.clone().into());
            d.set_every_day(every.clone().into());
            d.set_cal_items(ModelRc::new(VecModel::from(items.clone())));
        });
    }

    /// Battery, weather, the next event and the open to-dos
    fn widgets(&self, st: &State) {
        let bat = st.shell.get::<Battery>().cloned().unwrap_or_default();
        let wx = st.shell.get::<Weather>().cloned().unwrap_or_default();
        let material = st.shell.look.material;
        let h24 = b(&st.shell.cfg["appearance"], "clock24", false);
        let now = crate::util::now_ms();
        let next = st.shell.get::<Calendar>().map(|c| c.between(now, now + 14 * 86_400_000)).unwrap_or_default().into_iter().find(|e| e.all_day || e.end > now);
        let todos = st.shell.get::<Todos>().map(|t| t.list.iter().filter(|t| !t.done).count()).unwrap_or(0);
        let event_time = next.as_ref().map(|e| {
            if e.all_day {
                "Today".to_string()
            } else {
                let t = crate::services::calendar::local(e.start);
                if material { logic::hm(t.h as u32, t.mi as u32, h24) } else { logic::clock(t.h as u32, t.mi as u32, h24) }
            }
        });
        let event_line = next.as_ref().map(|e| format!("{} · {}", if e.title.is_empty() { "Event" } else { &e.title }, event_time.clone().unwrap_or_default())).unwrap_or_default();
        // (Phosphor's for Glass; Material's own weather symbols, as Today's)
        let glyph = weather_glyph(wx.icon, false);
        let sym = weather_glyph(wx.icon, true);
        let _ = material;
        self.each(|d| {
            d.set_weather_sym(sym.into());
            d.set_has_battery(bat.present);
            d.set_level((bat.percent / 100.0).clamp(0.0, 1.0) as f32);
            d.set_charging(bat.present && (bat.state == 1 || bat.state == 4));
            d.set_full(bat.present && bat.state == 4);
            d.set_weather_ready(wx.ready);
            d.set_weather_glyph(glyph.into());
            d.set_temp(format!("{}°", wx.temp).into());
            d.set_condition(wx.condition.clone().into());
            d.set_has_event(next.is_some());
            d.set_event_time(event_time.clone().unwrap_or_default().into());
            d.set_event_line(event_line.clone().into());
            d.set_todos(todos as i32);
        });
    }

    /// The notifications: the shelf's apps, the newest cards
    fn notes_push(&mut self) {
        let now = crate::util::now_ms();
        let h24 = self.wins.first().is_some_and(|w| w.ui.global::<ui::LockData>().get_h24());
        // (Notifications.iconFor: a picture, the app's icon, its desktop
        // entry's Icon=, else dialog-information; each loaded once)
        for n in &self.notes {
            if !self.icons.contains_key(&n.icon) {
                let i = n.icon.as_str();
                let path = if let Some(f) = i.strip_prefix("file://") {
                    Some(std::path::PathBuf::from(f))
                } else if i.starts_with('/') {
                    Some(std::path::PathBuf::from(i))
                } else {
                    let name = i.strip_prefix("image://icon/").unwrap_or(i);
                    let name = name.split('?').next().unwrap_or(name);
                    let entry = (!name.is_empty()).then(|| crate::util::desktop_field(name, "Icon")).flatten();
                    (!name.is_empty()).then(|| crate::icon_theme::find(name, None)).flatten().or_else(|| entry.and_then(|e| crate::icon_theme::find(&e, None))).or_else(|| crate::icon_theme::find("dialog-information", None))
                };
                let img = path.and_then(|p| Image::load_from_path(&p).ok());
                self.icons.insert(n.icon.clone(), img);
            }
        }
        let newest: Vec<&Note> = self.notes.iter().rev().collect();
        let keys: Vec<String> = newest.iter().map(|n| n.key.clone()).collect();
        let apps = logic::shelf_apps(&keys);
        let icons = &self.icons;
        let make = |n: &Note| {
            let img = icons.get(&n.icon).cloned().flatten();
            let t = crate::services::calendar::local(n.at);
            ui::LNote {
                icon: img.clone().unwrap_or_default(),
                has_icon: img.is_some(),
                summary: n.summary.clone().into(),
                app: n.app.clone().into(),
                body: logic::plain(&n.body).into(),
                when: logic::time_label(n.at, now, &logic::hm(t.h as u32, t.mi as u32, h24)).into(),
            }
        };
        let all: Vec<ui::LNote> = newest.iter().take(4).map(|n| make(n)).collect();
        let shelf: Vec<ui::LNote> = apps.iter().map(|&i| make(newest[i])).collect();
        let count = newest.len() as i32;
        self.notes_rows.set(all.clone());
        self.apps_rows.set(shelf);
        for w in &self.wins {
            let d = w.ui.global::<ui::LockData>();
            // the cards this screen shows: the newest (shelf), the latest
            // three (Glass) or four (Material) beside the clock on a wide
            // screen (column), none (icons)
            let (sw, sh) = w.surface.logical_size();
            let style = d.get_note_style();
            let split = style == "column" && sw > sh;
            let n = if style == "icons" { 0 } else if !split { 1 } else if self.material { 4 } else { 3 };
            w.cards.set(all.iter().take(n).cloned().collect());
            d.set_note_count(count);
        }
        // (icons of notifications gone: let go)
        if self.icons.len() > 64 {
            let keep: std::collections::HashSet<&String> = self.notes.iter().map(|n| &n.icon).collect();
            self.icons.retain(|k, _| keep.contains(k));
        }
    }

    fn player(&mut self, st: &State) {
        let p = st.shell.get::<Players>().and_then(|p| p.chosen().cloned());
        let has = p.as_ref().is_some_and(|p| !p.track.title.is_empty());
        let url = p.as_ref().map(|p| p.track.art_url.clone()).unwrap_or_default();
        if url != self.art.0 {
            self.art = (url.clone(), None);
            self.each(|d| d.set_has_art(false));
            if !url.is_empty() {
                let to = Self::send(st);
                std::thread::spawn(move || {
                    let f = art_files(&url);
                    to(Box::new(M::Art(url, f)));
                });
            }
        }
        // The timeline (Material's LockPlayer): while the track's length is
        // known, its position read once a second off the bus (the QML's
        // positionChanged timer); a new player or track starts unknown
        let length = p.as_ref().map(|p| p.track.length).unwrap_or(0);
        let bus = p.as_ref().map(|p| p.bus.clone()).unwrap_or_default();
        let want = self.held && st.shell.look.material && has && length > 0;
        if self.position.0 != bus {
            self.position = (bus.clone(), -1);
        }
        if let Ok(mut t) = self.position_for.lock() {
            *t = want.then(|| bus.clone());
        }
        if want && !self.position_thread {
            self.position_thread = true;
            let (target, to) = (self.position_for.clone(), Self::send(st));
            std::thread::spawn(move || {
                let Ok(conn) = zbus::blocking::Connection::session() else { return };
                loop {
                    let bus = target.lock().ok().and_then(|t| t.clone());
                    if let Some(bus) = bus {
                        if let Some(pos) = read_position(&conn, &bus) {
                            to(Box::new(M::Position(bus, pos)));
                        }
                    }
                    std::thread::sleep(Duration::from_secs(1));
                }
            });
        }
        let known = want && self.position.1 >= 0;
        let progress = if known { (self.position.1 as f64 / length as f64).clamp(0.0, 1.0) as f32 } else { 0.0 };
        self.each(|d| {
            d.set_has_player(has);
            d.set_track(p.as_ref().map(|p| p.track.title.clone()).unwrap_or_default().into());
            d.set_artist(p.as_ref().map(|p| p.track.artist.clone()).unwrap_or_default().into());
            d.set_playing(p.as_ref().is_some_and(|p| p.playing));
            d.set_has_timeline(known);
            d.set_progress(progress);
        });
    }

    /// The field, the reader and the camera
    fn field(&self) {
        let face_looking = self.face.conv.is_some();
        let island = logic::island(&self.finger.state, self.face_phase(), !self.password.is_empty(), self.busy, self.leaving);
        let pw = self.password.clone();
        for w in &self.wins {
            let d = w.ui.global::<ui::LockData>();
            if self.typed_on.as_deref() != Some(w.output.as_str()) && d.get_password().as_str() != pw {
                d.set_password(pw.clone().into());
            }
        }
        self.each(|d| {
            d.set_error(self.error.clone().into());
            d.set_busy(self.busy);
            d.set_error_count(self.error_count);
            d.set_finger_text(self.finger.text.clone().into());
            d.set_finger_state(self.finger.state.clone().into());
            d.set_face_text(self.face.text.clone().into());
            d.set_face_looking(face_looking);
            d.set_finger_on(self.finger_on);
            d.set_face_on(self.face_on);
            d.set_leaving(self.leaving);
            d.set_island_kind(island.kind.into());
            d.set_island_phase(island.phase.into());
            // (the reader's third miss says "Place your finger again" there too)
            let hint = if island.kind == "finger" && island.phase == "miss" && self.finger.text == reader::AGAIN_TEXT { reader::AGAIN_TEXT } else { island.hint };
            d.set_island_hint(hint.into());
        });
    }

    /// The camera's phase for the island: a match, a miss showing, looking
    /// (its conversation running), between tries; the sandbox's demo first
    fn face_phase(&self) -> &str {
        if !self.face.demo.is_empty() {
            &self.face.demo
        } else if self.face.matched {
            "match"
        } else if self.face.missed {
            "miss"
        } else if self.face.conv.is_some() {
            "reading"
        } else if self.face.text.is_empty() {
            ""
        } else {
            "waiting"
        }
    }

    // ── Bedside Mode (StandBy.qml) ─────────────────────────────────────────

    fn views(&self) -> i32 {
        if self.sb.photos.is_empty() { 2 } else { 3 }
    }

    fn photos(&mut self, st: &State) {
        let folder = st.shell.cfg["desktop"]["photosFolder"].as_str().filter(|f| !f.is_empty()).map(str::to_string).unwrap_or_else(|| format!("{}/Pictures", std::env::var("HOME").unwrap_or_default()));
        let mut list: Vec<String> = std::fs::read_dir(&folder)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file() && p.extension().and_then(|e| e.to_str()).is_some_and(|e| ["jpg", "jpeg", "png", "webp"].contains(&e.to_lowercase().as_str())))
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        list.sort();
        self.sb.photos = list;
    }

    fn bedside(&mut self, st: &State) {
        let standby = self.standby(st);
        if standby != self.sb.shown {
            self.sb.shown = standby;
            self.sb.minutes = 0;
            self.sb.resting = false;
            self.t_rest.stop();
            if standby {
                self.drift(st, true);
                self.sb.dots += 1;
                // (keep the screen on while it shows)
                for w in &mut self.wins {
                    if w.inhibitor.is_none() {
                        w.inhibitor = st.inhibit_idle(&w.surface);
                    }
                }
            } else {
                for w in &mut self.wins {
                    if let Some(i) = w.inhibitor.take() {
                        i.destroy();
                    }
                }
            }
        }
        let c = cfg_lock(st);
        let screens = c["standbyScreens"].clone();
        let views = self.views();
        let photo_on = standby && self.sb.view == 2 && !self.sb.photos.is_empty();
        if photo_on && !self.t_photo.running() {
            let to = Self::send(st);
            self.t_photo.start(slint::TimerMode::Repeated, Duration::from_secs(20), move || to(Box::new(M::PhotoNext)));
            // (coming in: the photo fades in once loaded, as StandBy.qml's status)
            self.each(|d| d.set_has_photo(false));
            self.load_photo(st);
        } else if !photo_on && self.t_photo.running() {
            self.t_photo.stop();
            // (gone: the next time in fades in again, not a cut to the last one)
            self.each(|d| d.set_has_photo(false));
        }
        for w in &self.wins {
            let d = w.ui.global::<ui::LockData>();
            d.set_standby(standby);
            d.set_standby_here(crate::surfaces::on_screen(&screens, &w.output));
            d.set_sb_view(self.sb.view);
            d.set_sb_views(views);
            d.set_sb_resting(self.sb.resting);
            d.set_sb_dots(self.sb.dots);
        }
    }

    /// A new spot (every screen the same one with lock.standbySameClock)
    fn drift(&mut self, st: &State, jump: bool) {
        let c = cfg_lock(st);
        let shift = b(c, "standbyShift", true);
        let same = b(c, "standbySameClock", true);
        self.sb.fy = if shift { logic::next_drift(self.sb.fy, logic::random) } else { 0.0 };
        self.sb.drifts += 1;
        let material = st.shell.look.material;
        let view = self.sb.view;
        for w in &mut self.wins {
            let (sw, sh) = w.surface.logical_size();
            let (sw, sh) = (sw as f64, sh as f64);
            // the content's size: the clock, or the widgets row
            let (cw, ch) = if view == 1 {
                let side = (sh * 0.38).min(sw * 0.26);
                (side * 2.0 + sw * 0.03, side)
            } else if material {
                let m = (sh * 0.3).min(sw * 0.17);
                (m * 2.6, m * 1.6 + m * 0.14 + 30.0)
            } else {
                let size = (sh * 0.16).min(sw * 0.14);
                (size * 3.0, size * 1.2 + (size * 0.14).max(16.0) * 1.2)
            };
            let (_, room_y) = logic::room(sw, sh, cw, ch);
            let fy = if !shift {
                0.0
            } else if same {
                self.sb.fy
            } else {
                w.own_fy = logic::next_drift(w.own_fy, logic::random);
                w.own_fy
            };
            let d = w.ui.global::<ui::LockData>();
            d.set_sb_jump(jump);
            d.set_sb_dy(fy as f32 * room_y as f32);
        }
    }

    fn load_photo(&mut self, st: &State) {
        if self.sb.photos.is_empty() {
            return;
        }
        let path = self.sb.photos[self.sb.photo % self.sb.photos.len()].clone();
        // (a change every 20 s swaps in one frame once loaded, as StandBy.qml:
        // its Image goes Loading → Ready faster than its 800 ms fade moves)
        let (w, h) = self.wins.iter().map(|w| w.surface.logical_size()).max().unwrap_or((1920, 1080));
        let to = Self::send(st);
        std::thread::spawn(move || {
            let px = crate::picture::decode(&path, w, h);
            to(Box::new(M::Photo(path, px)));
        });
    }

    // ── Rules: what's wanted now (Lock.qml's bindings) ─────────────────────

    /// A scan should run: the lock shows, the reader's on, fewer than 6
    /// misses, something happened in the last 2 minutes (reader.rs)
    fn finger_wanted(&self) -> bool {
        self.real && self.held && !self.leaving && !self.sleeping && self.finger_on && reader::listening(self.finger.tries, crate::util::now_ms(), self.finger.activity_at)
    }

    /// End the running scan off the UI thread (SIGTERM: fprintd lets the
    /// reader go as its helper's connection closes)
    fn end_scan(&mut self) {
        if let Some(c) = self.finger.conv.take() {
            std::thread::spawn(move || {
                c.stop(Duration::from_secs(1));
            });
        }
    }

    /// A fresh scan: the old one ended, the reader confirmed (fprintd's
    /// GetDevices and a claim that works; after a wake, waited for), then
    /// `M::Probed` starts it
    fn fresh(&mut self, st: &State, why: reader::Why) {
        if !self.finger_wanted() {
            return;
        }
        self.t_finger.stop();
        self.finger.gen += 1;
        self.finger.probing = true;
        let old = self.finger.conv.take();
        let (to, gen) = (Self::send(st), self.finger.gen);
        std::thread::spawn(move || {
            let p = reader::fresh_scan(old, &reader::SystemFprint, why);
            to(Box::new(M::Probed(gen, why, p)));
        });
    }

    /// Start the scan itself (the PAM helper: pam_fprintd)
    fn start_scan(&mut self, st: &State) {
        let now = crate::util::now_ms();
        if !self.finger.miss_shown {
            self.finger.text = WAITING.into();
            self.finger.state = "waiting".into();
        }
        self.finger.asked_at = now;
        self.finger.started_at = now;
        self.finger.asked = false;
        self.conv_id += 1;
        self.finger.conv = Some(auth::start(&self.backend, Kind::Finger, self.conv_id, String::new(), Self::send(st)));
        self.field();
    }

    /// fprintd is stuck: a silent restart when it's allowed now (Tessera's
    /// polkit rule; at most once in 2 minutes), else the hint
    fn restart_reader(&mut self, st: &State) {
        let now = crate::util::now_ms();
        if !reader::can_restart(now, self.finger.restarted_at) {
            return self.reader_stuck(st);
        }
        self.finger.restarted_at = Some(now);
        self.t_finger.stop();
        self.finger.gen += 1;
        self.finger.probing = true;
        self.end_scan();
        eprintln!("tessera-ui: lock: the fingerprint reader is stuck: restarting fprintd");
        let (to, gen) = (Self::send(st), self.finger.gen);
        std::thread::spawn(move || {
            let p = match reader::Fprint::restart(&reader::SystemFprint) {
                Ok(()) => {
                    std::thread::sleep(Duration::from_millis(500));
                    reader::wait_for_reader(&reader::SystemFprint, reader::Why::Restarted.within(), Duration::from_millis(300))
                }
                // (no polkit rule: refused at once, never a prompt)
                Err(e) => reader::Probe::Stuck(format!("restart refused: {e}")),
            };
            to(Box::new(M::Probed(gen, reader::Why::Restarted, p)));
        });
    }

    /// Stuck and not restarted: the quiet hint under the field, a look every 30 s
    fn reader_stuck(&mut self, st: &State) {
        if !self.finger.stuck {
            eprintln!("tessera-ui: lock: the fingerprint reader needs a restart (fprintd); the password works");
        }
        self.finger.stuck = true;
        self.finger.tell_after = true;
        self.finger.text = reader::STUCK_TEXT.into();
        self.finger.state.clear();
        self.finger.retry_why = reader::Why::Stuck;
        self.finger_retry(st, reader::STUCK_EVERY_MS);
        self.field();
    }

    /// The lid opened, a screen came on, a key, the pointer, a touch: a
    /// fresh scan at once (a young one that's listening stays; a stuck
    /// reader keeps its own 30 s; a pause or a back-off runs out)
    fn reader_activity(&mut self, st: &mut State, always: bool) {
        let now = crate::util::now_ms();
        self.finger.activity_at = now;
        let since = self.finger.conv.as_ref().map(|_| self.finger.started_at);
        let free = self.finger_wanted() && !self.finger.probing && !self.finger.stuck && !self.t_finger.running();
        if free && (always || reader::activity_restarts(since, self.finger.asked, now)) {
            self.fresh(st, reader::Why::Fresh);
        }
        self.rules(st);
    }

    fn face_wanted(&self, st: &State) -> bool {
        self.real && self.held && !self.leaving && !self.standby(st) && self.face_on && self.face.armed && !self.face.broken && self.face.tries < 3
    }

    /// After anything changed: start or stop the reader, the camera, the
    /// idle timer, Bedside Mode; then show it all
    fn rules(&mut self, st: &mut State) {
        // the reader
        self.finger.wanted = self.finger_wanted();
        if self.finger.wanted {
            if self.finger.conv.is_none() && !self.t_finger.running() && !self.finger.probing {
                self.fresh(st, reader::Why::Fresh);
            }
        } else {
            self.t_finger.stop();
            self.finger.gen += 1;
            self.finger.probing = false;
            self.end_scan();
            // (the last miss stays while it shows)
            if self.real && !self.t_finger_miss.running() {
                self.finger.text.clear();
                self.finger.state.clear();
            }
        }
        // logind's sleep delay inhibitor, while a scan can run
        if reader::hold_inhibitor(self.real, self.held, self.sleeping, self.finger_on, self.face_on) {
            if self.sleep_fd.is_none() && !self.sleep_fd_asked {
                self.sleep_fd_asked = true;
                let to = Self::send(st);
                std::thread::spawn(move || to(Box::new(M::Inhibitor(tessera_core::inhibit::delay("sleep", "Stop the fingerprint reader and the camera before sleeping")))));
            }
        } else {
            self.sleep_fd = None;
            self.sleep_fd_asked = false;
        }
        // the camera
        if self.face_wanted(st) {
            if self.face.conv.is_none() && !self.t_face.running() {
                self.face_retry(st, 0);
            }
        } else {
            self.t_face.stop();
            if let Some(c) = self.face.conv.take() {
                c.abort();
            }
            if self.real {
                self.face.text.clear();
            }
        }
        // the logind watch (real only, while the reader or the camera may be used)
        if self.real && (self.finger_on || self.face_on) && !self.sleep_watch {
            self.sleep_watch = true;
            watch_sleep(Self::send(st));
        }
        // Bedside Mode's idle timer: 20 s after the last touch
        if self.held && self.can_standby(st) && !self.sb.idle {
            if !self.t_idle.running() {
                let to = Self::send(st);
                self.t_idle.start(slint::TimerMode::SingleShot, Duration::from_secs(20), move || to(Box::new(M::IdleOver)));
            }
        } else {
            self.t_idle.stop();
        }
        self.field();
        self.bedside(st);
    }

    fn finger_retry(&mut self, st: &State, ms: u64) {
        let to = Self::send(st);
        self.t_finger.start(slint::TimerMode::SingleShot, Duration::from_millis(ms), move || to(Box::new(M::FingerRetry)));
    }

    fn face_retry(&mut self, st: &State, ms: u64) {
        let to = Self::send(st);
        self.t_face.start(slint::TimerMode::SingleShot, Duration::from_millis(ms.max(1)), move || to(Box::new(M::FaceRetry)));
    }

    /// Someone at the keyboard or the pointer (Lock.qml's woke)
    fn woke(&mut self, st: &mut State) {
        let now = crate::util::now_ms();
        let back = logic::came_back(self.sb.idle, now, self.last_touch);
        self.last_touch = now;
        self.sb.idle = false;
        self.t_idle.stop();
        if !self.face.armed && now - self.held_at > 1000 {
            self.face.armed = true;
            self.face.tries = 0;
        }
        if self.face.tries >= 3 && now - self.face.stopped_at > 3000 {
            self.face.tries = 0;
            self.face.broken = false;
        }
        if back {
            // (the reader's misses last until the next wake or unlock)
            self.face.tries = 0;
        }
        self.reader_activity(st, false);
    }

    fn submit(&mut self, st: &mut State) {
        if self.busy || self.password.is_empty() || !self.held {
            return;
        }
        self.error.clear();
        self.face.tries = 0;
        self.busy = true;
        self.conv_id += 1;
        self.pass_id = self.conv_id;
        self.pass_conv = Some(auth::start(&self.backend, Kind::Password, self.pass_id, self.password.clone(), Self::send(st)));
        // A check never hangs: 20 s without an answer stops it (18 Nov: after
        // a sleep it said "checking" until a restart)
        let (to, id) = (Self::send(st), self.pass_id);
        self.t_pass.start(slint::TimerMode::SingleShot, Duration::from_secs(20), move || to(Box::new(M::PassTimeout(id))));
        self.rules(st);
    }

    /// Unlocked here: the shell lets the lock go (System.locked = false)
    /// A reader matched: the island shows it (ridges fill green, the tick;
    /// the face's green brackets) and the lock goes a moment later. 18 Nov:
    /// it unlocked in the same turn, so the match was never drawn
    fn unlock_after_match(&mut self, st: &mut State) {
        let reduced = b(&st.shell.cfg["appearance"], "reduceMotion", false) || tessera_core::saver::on();
        let to = Self::send(st);
        self.t_match.start(slint::TimerMode::SingleShot, Duration::from_millis(if reduced { 250 } else { 650 }), move || to(Box::new(M::MatchShown)));
    }

    fn unlocked(&mut self) {
        self.error.clear();
        self.fails = 0;
        crate::shell::tell(NAME, serde_json::json!({"unlocked": true}));
    }

    fn set_password(&mut self, st: &mut State, on: String, text: String) {
        self.typed_on = Some(on);
        if text == self.password {
            return;
        }
        let was_empty = self.password.is_empty();
        self.password = text;
        if !self.password.is_empty() {
            self.woke(st);
        } else if !was_empty {
            self.rules(st);
        }
        self.field();
    }

    fn auth(&mut self, st: &mut State, e: Auth) {
        let now = crate::util::now_ms();
        match e {
            Auth::Info(Kind::Password, id, m) | Auth::Error(Kind::Password, id, m) if is_pass(id, self.pass_id) => {
                if m.to_lowercase().contains("locked") {
                    self.pam_locked = true;
                }
            }
            Auth::Done(Kind::Password, id, out) if is_pass(id, self.pass_id) => {
                self.t_pass.stop();
                // (what happened, never what was typed)
                eprintln!("tessera-ui: lock: password check: {}", match &out { Outcome::Success => "success", Outcome::Failure(_) => "failure", Outcome::Error(_) => "error" });
                self.pass_conv = None;
                self.busy = false;
                match out {
                    Outcome::Success => self.unlocked(),
                    Outcome::Failure(_) => {
                        self.fails += 1;
                        self.error = if self.pam_locked { logic::locked_text(self.faillock.unlock) } else { "Incorrect password. Try again.".into() };
                        self.error_count += 1;
                        crate::shell::tell(NAME, serde_json::json!({"sound": "error"}));
                        self.check_lockout(st);
                    }
                    Outcome::Error(why) => {
                        eprintln!("tessera-ui: lock: password check couldn't run: {why}");
                        self.error = "Couldn't check your password. Try again.".into();
                        self.error_count += 1;
                    }
                }
                self.pam_locked = false;
                self.password.clear();
                self.typed_on = None;
                self.rules(st);
            }
            Auth::Info(Kind::Finger, id, m) | Auth::Error(Kind::Finger, id, m) if self.finger.conv.as_ref().is_some_and(|c| c.id == id) => {
                match logic::finger_message(&m) {
                    logic::Finger::Miss => {
                        if now - self.finger.asked_at < 700 {
                            return; // (not a touch: pam_fprintd asks again by itself)
                        }
                        self.finger.tries += 1;
                        // (a finger is activity: the idle 2 minutes start again)
                        self.finger.activity_at = now;
                        self.finger.state = "miss".into();
                        self.finger.miss_shown = true;
                        let after = reader::after_miss(self.finger.tries);
                        self.finger.text = if after == reader::AfterMiss::Pause { reader::AGAIN_TEXT } else { reader::MISS_TEXT }.into();
                        let to = Self::send(st);
                        let shown = if after == reader::AfterMiss::Pause { reader::PAUSE_MS } else { 1600 };
                        self.t_finger_miss.start(slint::TimerMode::SingleShot, Duration::from_millis(shown), move || to(Box::new(M::FingerMissOver)));
                        match after {
                            reader::AfterMiss::Again => {}
                            // 3 misses: this scan ends; a fresh one in 2 s, for 3 more
                            reader::AfterMiss::Pause => {
                                self.end_scan();
                                self.finger.retry_why = reader::Why::Fresh;
                                self.finger_retry(st, reader::PAUSE_MS);
                            }
                            // 6: the password only, until the next wake or unlock
                            reader::AfterMiss::Done => {
                                eprintln!("tessera-ui: lock: 6 fingerprint misses: the password only until the next wake or unlock");
                                self.end_scan();
                            }
                        }
                    }
                    logic::Finger::Ask => {
                        self.finger.asked_at = now;
                        self.finger.asked = true;
                        self.finger.busy = 0;
                        if !self.finger.miss_shown {
                            self.finger.text = WAITING.into();
                            self.finger.state = "waiting".into();
                        }
                    }
                    logic::Finger::Other => {}
                }
                self.rules(st);
            }
            Auth::Done(Kind::Finger, id, out) if self.finger.conv.as_ref().is_some_and(|c| c.id == id) => {
                self.finger.conv = None;
                if out == Outcome::Success {
                    self.finger.state = "match".into();
                    self.finger.text.clear();
                    self.finger.tries = 0;
                    self.finger.busy = 0;
                    self.field();
                    self.unlock_after_match(st);
                    return;
                }
                // over at once, before it asked: the reader refused (busy,
                // or fprintd stuck): back off, ask fprintd, restart it (reader.rs)
                if !self.finger.asked && now - self.finger.started_at < 2000 {
                    self.finger.busy += 1;
                    match reader::after_quick_fail(self.finger.busy) {
                        reader::Next::Restart => self.restart_reader(st),
                        reader::Next::Later(0) => {
                            self.finger.retry_why = reader::Why::Failing;
                            self.finger_retry(st, 1);
                        }
                        reader::Next::Later(ms) => {
                            self.finger.retry_why = reader::Why::Fresh;
                            self.finger_retry(st, ms);
                        }
                        _ => self.reader_stuck(st),
                    }
                    self.field();
                    return;
                }
                // (its 30 s are up, or PAM ended it: the next scan, renewed)
                if self.finger_wanted() && !self.t_finger.running() {
                    self.finger.retry_why = reader::Why::Fresh;
                    self.finger_retry(st, 400);
                }
                self.rules(st);
            }
            // The blink check's hints ("Blink to unlock"), shown as the face's
            // text; Howdy's own lines aren't (core::face::hint)
            Auth::Info(Kind::Face, id, m) | Auth::Error(Kind::Face, id, m) if self.face.conv.as_ref().is_some_and(|c| c.id == id) => {
                if let Some(h) = tessera_core::face::hint(&m) {
                    self.face.text = h.to_string();
                    self.face.checked = true;
                    self.field();
                }
            }
            Auth::Done(Kind::Face, id, out) if self.face.conv.as_ref().is_some_and(|c| c.id == id) => {
                self.face.conv = None;
                // (pam_exec says "system error" for any no from the blink
                // check; one that spoke looked, so it's a miss, not a broken camera)
                let out = match out {
                    Outcome::Error(why) if std::mem::take(&mut self.face.checked) => Outcome::Failure(why),
                    o => {
                        self.face.checked = false;
                        o
                    }
                };
                match out {
                    Outcome::Success => {
                        self.face.tries = 0;
                        self.face.text.clear();
                        self.face.matched = true;
                        self.unlock_after_match(st);
                    }
                    Outcome::Failure(_) => {
                        self.face.tries += 1;
                        // (the island shows the miss 1.3 s, over the next try's start)
                        self.face.missed = true;
                        let to = Self::send(st);
                        self.t_face_miss.start(slint::TimerMode::SingleShot, Duration::from_millis(1300), move || to(Box::new(M::FaceMissOver)));
                        self.face.text = if self.face.tries < 3 { "Face not recognised".into() } else { String::new() };
                        if self.face.tries >= 3 {
                            self.face.stopped_at = now;
                        }
                        if self.face_wanted(st) {
                            self.face_retry(st, 300);
                        }
                    }
                    // (Howdy or the camera isn't working: the password is there)
                    Outcome::Error(_) => {
                        self.face.broken = true;
                        self.face.text.clear();
                    }
                }
                self.rules(st);
            }
            _ => {}
        }
    }

    /// After a wrong password: is the account locked by too many attempts?
    /// (faillock.conf, then `faillock --user`; real only)
    fn check_lockout(&self, st: &State) {
        if !self.real {
            return;
        }
        let to = Self::send(st);
        std::thread::spawn(move || {
            let conf = std::fs::read_to_string("/etc/security/faillock.conf").map(|t| logic::faillock_conf(&t)).unwrap_or_default();
            let tally = std::process::Command::new("faillock").args(["--user", &auth::user_name()]).stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null()).output().ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).into_owned());
            to(Box::new(M::Lockout(conf, tally)));
        });
    }

    /// The `demo` lines (the sandbox's `ipc lock demo …`, forwarded by
    /// Lock.qml; never acting on anything)
    fn demo_state(&mut self, st: &mut State, what: &str) {
        match what {
            "standby" => {
                self.sb.demo_charging = true;
                self.sb.idle = true;
                self.password.clear();
                self.typed_on = None;
            }
            "wake" => self.woke(st),
            "finger" | "reading" => {
                self.face.text.clear();
                self.face.demo.clear();
                self.finger.text = WAITING.into();
                self.finger.state = if what == "finger" { "waiting" } else { "reading" }.into();
            }
            // the camera: waiting, looking, its miss and its match (the island)
            "face" | "face-reading" | "face-miss" | "face-match" => {
                self.finger.text.clear();
                self.finger.state.clear();
                self.face.text = "Looking for your face…".into();
                self.face.demo = match what {
                    "face" => "waiting",
                    "face-reading" => "reading",
                    "face-miss" => "miss",
                    _ => "match",
                }
                .into();
            }
            "match" => {
                self.finger.state = "match".into();
                self.finger.text.clear();
                self.face.text.clear();
                self.face.demo.clear();
            }
            "typing" => {
                self.sb.idle = false;
                self.password = "tessera".into();
                self.typed_on = None;
            }
            "wrong" => {
                self.password.clear();
                self.typed_on = None;
                self.error = "Incorrect password. Try again.".into();
                self.error_count += 1;
            }
            "busy" => {
                self.password = "tessera".into();
                self.typed_on = None;
                self.busy = true;
            }
            "lockout" => {
                self.password.clear();
                self.typed_on = None;
                self.busy = false;
                self.error = logic::locked_text(540);
                self.error_count += 1;
            }
            "miss" => {
                self.face.demo.clear();
                self.finger.text = "Not recognised. Try again".into();
                self.finger.state = "miss".into();
            }
            _ => {
                self.face.demo.clear();
                self.sb.demo_charging = false;
                self.sb.idle = false;
                self.finger.text.clear();
                self.finger.state.clear();
                self.face.text.clear();
                self.error.clear();
                self.busy = false;
                self.password.clear();
                self.typed_on = None;
            }
        }
        // (the face spinner while "Looking for your face…" shows)
        for w in &self.wins {
            w.ui.global::<ui::LockData>().set_demo_face(what.starts_with("face"));
        }
        self.rules(st);
    }
}

/// logind's PrepareForSleep, read-only (Lock.qml's EventWatch "login1")
fn watch_sleep(to: impl Fn(Box<dyn Any + Send>) + Send + Clone + 'static) {
    // the lid opening (logind's LidClosed turning false): a fresh scan
    let lid = to.clone();
    std::thread::spawn(move || {
        let run = || -> zbus::Result<()> {
            let conn = zbus::blocking::Connection::system()?;
            let rule = zbus::MatchRule::builder()
                .msg_type(zbus::message::Type::Signal)
                .sender("org.freedesktop.login1")?
                .path("/org/freedesktop/login1")?
                .interface("org.freedesktop.DBus.Properties")?
                .member("PropertiesChanged")?
                .build();
            let signals = zbus::blocking::MessageIterator::for_match_rule(rule, &conn, Some(8))?;
            for m in signals.flatten() {
                let body = m.body();
                let Ok((_, changed, _)) = body.deserialize::<(String, std::collections::HashMap<String, zbus::zvariant::OwnedValue>, Vec<String>)>() else { continue };
                if changed.get("LidClosed").and_then(|v| v.downcast_ref::<bool>().ok()) == Some(false) {
                    lid(Box::new(M::LidOpened));
                }
            }
            Ok(())
        };
        if let Err(e) = run() {
            eprintln!("tessera-ui: lock: no logind lid signal ({e})");
        }
    });
    std::thread::spawn(move || {
        let run = || -> zbus::Result<()> {
            let conn = zbus::blocking::Connection::system()?;
            let rule = zbus::MatchRule::builder()
                .msg_type(zbus::message::Type::Signal)
                .sender("org.freedesktop.login1")?
                .interface("org.freedesktop.login1.Manager")?
                .member("PrepareForSleep")?
                .build();
            let signals = zbus::blocking::MessageIterator::for_match_rule(rule, &conn, Some(8))?;
            for m in signals.flatten() {
                if let Ok(going) = m.body().deserialize::<bool>() {
                    to(Box::new(M::Sleep(going)));
                }
            }
            Ok(())
        };
        if let Err(e) = run() {
            eprintln!("tessera-ui: lock: no logind sleep signal ({e})");
        }
    });
}

impl Part for Lock {
    fn name(&self) -> &'static str {
        NAME
    }

    fn start(&mut self, st: &mut State) {
        for s in ["status", "mpris", "weather", "calendar", "todos"] {
            st.shell.want(s);
        }
        let path = crate::wallpaper::showing_now(&st.shell.cfg, st.shell.look.material);
        crate::services::wallpaper::sample(&path, st.tx.clone());
        self.wall_path = path;
        eprintln!(
            "tessera-ui: lock: {} (password: {})",
            if self.demo { "demo: overlay surfaces, no session lock" } else if self.real { "real: session lock, system PAM" } else { "sandbox: session lock, test PAM" },
            match &self.backend {
                Backend::System => "system login",
                Backend::TestService { .. } => "test service",
                Backend::Mock { .. } => "mock",
            }
        );
        // The lock before this one panicked while the session was locked
        // (tessera_core::contain): its session lock is ours now, still
        // locked; a screen on every output at once, as when it began
        if let Some(h) = crate::contain::take_handed::<HandOver>(NAME) {
            eprintln!("tessera-ui: lock: made again after a panic: {}", if h.lock.as_ref().is_some_and(|l| l.is_locked()) { "the session is still locked" } else { "locking" });
            self.finger_on = h.finger_on;
            self.face_on = h.face_on;
            self.notes = h.notes;
            self.secure = h.lock.as_ref().is_some_and(|l| l.is_locked());
            self.lock = h.lock;
            self.set_held(st, true);
        }
    }

    fn hand_over(&mut self) -> Option<Box<dyn Any>> {
        // (a session lock is never dropped here: destroying one that's
        // locked ends the connection; the next lock keeps it)
        if !self.held && self.lock.is_none() {
            return None;
        }
        Some(Box::new(HandOver { lock: self.lock.take(), finger_on: self.finger_on, face_on: self.face_on, notes: std::mem::take(&mut self.notes) }))
    }

    fn output_added(&mut self, st: &mut State, output: &WlOutput, name: &str) {
        if self.held && (self.lock.is_some() || self.demo) {
            self.window(st, output, name);
        }
        self.intro.output_added(st, output, name);
    }

    fn output_removed(&mut self, st: &mut State, name: &str) {
        if let Some(i) = self.wins.iter().position(|w| w.output == name) {
            let w = self.wins.remove(i);
            if let Some(i) = w.inhibitor {
                i.destroy();
            }
            st.drop_surface(&w.surface);
        }
        self.intro.output_removed(st, name);
        self.veil.output_removed(st, name);
    }

    fn config_changed(&mut self, st: &mut State, _look: bool) {
        self.settings(st);
        self.clock(st, true);
        self.widgets(st);
        self.drift(st, false);
        self.notes_push();
        self.rules(st);
        // (a new wallpaper: each screen decodes it again, in after_frame)
        let path = crate::wallpaper::showing_now(&st.shell.cfg, st.shell.look.material);
        if path != self.wall_path {
            self.wall_path = path.clone();
            crate::services::wallpaper::sample(&path, st.tx.clone());
        }
    }

    fn data(&mut self, st: &mut State, what: TypeId) {
        if what == TypeId::of::<Battery>() {
            self.widgets(st);
            self.rules(st);
        } else if [TypeId::of::<Weather>(), TypeId::of::<Calendar>(), TypeId::of::<Todos>()].contains(&what) {
            self.widgets(st);
            if what != TypeId::of::<Weather>() {
                self.calendar(st);
            }
        } else if what == TypeId::of::<Players>() {
            self.player(st);
        } else if what == TypeId::of::<crate::services::wallpaper::WallTop>() {
            self.settings(st);
        }
    }

    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        let msg = match msg.downcast::<LockEvent>() {
            Ok(e) => {
                match *e {
                    LockEvent::Locked => {
                        eprintln!("tessera-ui: lock: the session is locked");
                        self.secure = true;
                        crate::shell::tell(NAME, serde_json::json!({"secure": true}));
                    }
                    LockEvent::Finished => {
                        // (refused, or another client holds it: our surfaces are gone)
                        eprintln!("tessera-ui: lock: the compositor ended the lock");
                        self.lock = None;
                        for w in std::mem::take(&mut self.wins) {
                            if let Some(i) = w.inhibitor {
                                i.destroy();
                            }
                            st.drop_surface(&w.surface);
                        }
                        self.sb.shown = false;
                        if self.secure {
                            self.secure = false;
                            crate::shell::tell(NAME, serde_json::json!({"secure": false}));
                        }
                        // Still meant to be locked: take it again in a moment
                        // (Lock.qml locked again at once from the QML side;
                        // standalone there's no other locker), a few times,
                        // then say so
                        if self.held && !self.demo && self.retakes < 5 {
                            self.retakes += 1;
                            let to = Self::send(st);
                            self.t_retake.start(slint::TimerMode::SingleShot, Duration::from_millis(1000), move || to(Box::new(M::Retake)));
                        } else {
                            crate::shell::tell(NAME, serde_json::json!({"refused": true}));
                        }
                    }
                }
                return;
            }
            Err(m) => m,
        };
        let msg = match msg.downcast::<veil::Tick>() {
            Ok(_) => return self.veil.tick(st),
            Err(m) => m,
        };
        let msg = match msg.downcast::<Auth>() {
            Ok(e) => return self.auth(st, *e),
            Err(m) => m,
        };
        let msg = match msg.downcast::<serde_json::Value>() {
            Ok(v) => return self.told(st, &v),
            Err(m) => m,
        };
        let msg = match msg.downcast::<intro::Tick>() {
            Ok(_) => return self.intro.tick(st),
            Err(m) => m,
        };
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Edited(on, t) => self.set_password(st, on, t),
            M::Submit => self.submit(st),
            M::Woke => {
                // (a pointer moving says so many times a second: once in a
                // while is enough, outside Bedside Mode, whose own rules
                // already decided this is a wake)
                if !self.standby(st) && crate::util::now_ms() - self.last_touch < 250 {
                    return;
                }
                self.woke(st);
            }
            M::Step(dv) => {
                let views = self.views();
                self.sb.view = (self.sb.view + dv + views).rem_euclid(views);
                // (someone's using it during a rest: light it up again)
                if self.sb.resting {
                    self.t_rest.stop();
                    self.sb.minutes = 0;
                    self.sb.resting = false;
                }
                self.sb.dots += 1;
                self.drift(st, false);
                self.bedside(st);
            }
            M::Player(a) => {
                if let Some(p) = st.shell.get::<Players>().and_then(|p| p.chosen()) {
                    let method = match a.as_str() {
                        "prev" => "Previous",
                        "next" => "Next",
                        _ => "PlayPause",
                    };
                    mpris::control(&p.bus, method);
                }
            }
            M::Second => {
                let now = crate::util::now_ms();
                // a jump in the clock while locked (a sleep missed): a fresh reader
                if now - self.tick_ms > 10_000 && self.finger_on && self.real {
                    self.finger.tries = 0;
                    self.finger.busy = 0;
                    self.finger.activity_at = now;
                    self.finger.probing = false;
                    self.fresh(st, reader::Why::Wake);
                }
                self.tick_ms = now;
                if self.finger_on && self.real && self.held {
                    // a screen came on (DPMS): a fresh scan at once
                    let screens = reader::screens();
                    let on = reader::screen_came_on(&self.finger.screens, &screens);
                    self.finger.screens = screens;
                    if on {
                        self.reader_activity(st, true);
                    }
                    // the 2 idle minutes began (or ended): the scan stops (or starts)
                    if self.finger_wanted() != self.finger.wanted {
                        self.rules(st);
                    }
                    // one scan, renewed every 30 s
                    let since = self.finger.conv.as_ref().map(|_| self.finger.started_at);
                    if reader::renew_due(since, now) && !self.finger.probing {
                        self.fresh(st, reader::Why::Fresh);
                    }
                }
                let before = self.minute;
                self.clock(st, false);
                if before != self.minute && self.standby(st) && !self.sb.resting && b(cfg_lock(st), "standbyShift", true) {
                    self.sb.minutes += 1;
                    if self.sb.minutes >= 15 {
                        self.sb.minutes = 0;
                        self.sb.resting = true;
                        let to = Self::send(st);
                        self.t_rest.start(slint::TimerMode::SingleShot, Duration::from_secs(60), move || to(Box::new(M::RestOver)));
                    } else {
                        self.drift(st, false);
                    }
                    self.bedside(st);
                }
            }
            M::MatchShown => self.unlocked(),
            M::PassTimeout(id) => {
                if is_pass(id, self.pass_id) && self.busy {
                    eprintln!("tessera-ui: lock: password check: no answer in 20 s, stopped");
                    if let Some(c) = self.pass_conv.take() {
                        c.abort();
                    }
                    self.busy = false;
                    self.pass_id = 0; // (a late answer from it is ignored)
                    self.error = "Couldn't check your password. Try again.".into();
                    self.error_count += 1;
                    self.field();
                    self.rules(st);
                }
            }
            M::IdleOver => {
                if self.held && self.can_standby(st) {
                    self.sb.idle = true;
                }
                self.rules(st);
            }
            M::RestOver => {
                self.drift(st, true);
                self.sb.resting = false;
                self.bedside(st);
            }
            M::PhotoNext => {
                if self.sb.photos.len() > 1 {
                    let next = (logic::random() * self.sb.photos.len() as f64) as usize % self.sb.photos.len();
                    // (the same one again: it stays, as the QML's unchanged source)
                    if next != self.sb.photo % self.sb.photos.len() {
                        self.sb.photo = next;
                        self.load_photo(st);
                    }
                }
            }
            M::FingerRetry => {
                let why = std::mem::replace(&mut self.finger.retry_why, reader::Why::Fresh);
                if !self.finger_wanted() || self.finger.probing {
                    return;
                }
                self.fresh(st, why);
            }
            M::Probed(gen, why, p) => {
                if gen != self.finger.gen {
                    return; // (an older ask's answer)
                }
                self.finger.probing = false;
                if !self.finger_wanted() {
                    return self.rules(st);
                }
                if p != reader::Probe::Ready {
                    eprintln!("tessera-ui: lock: fingerprint reader ({why:?}): {p:?}");
                }
                match reader::after_probe(&p, why, self.finger.busy) {
                    reader::Next::Start => {
                        if std::mem::take(&mut self.finger.stuck) {
                            eprintln!("tessera-ui: lock: the fingerprint reader works again");
                            self.finger.tell_after = false;
                            self.finger.text.clear();
                        }
                        if why != reader::Why::Fresh {
                            self.finger.busy = 0;
                        }
                        self.start_scan(st);
                    }
                    reader::Next::Later(0) => {
                        self.finger.retry_why = reader::Why::Failing;
                        self.finger_retry(st, 1);
                    }
                    reader::Next::Later(ms) => self.finger_retry(st, ms),
                    reader::Next::Restart => self.restart_reader(st),
                    reader::Next::Hint => self.reader_stuck(st),
                }
                self.field();
            }
            M::Inhibitor(fd) => {
                // (asked stays true while it's wanted: a refusal isn't asked again
                // until the next lock or wake)
                if reader::hold_inhibitor(self.real, self.held, self.sleeping, self.finger_on, self.face_on) {
                    if fd.is_none() {
                        eprintln!("tessera-ui: lock: no logind sleep delay (the reader is stopped when the sleep signal comes)");
                    }
                    self.sleep_fd = fd;
                } else {
                    self.sleep_fd_asked = false;
                }
            }
            M::LidOpened => {
                if self.held && self.real {
                    self.reader_activity(st, true);
                }
            }
            M::FingerMissOver => {
                self.finger.miss_shown = false;
                if self.finger_wanted() && self.finger.conv.is_some() {
                    self.finger.text = WAITING.into();
                    self.finger.state = "waiting".into();
                } else if self.finger.state == "miss" && self.real && self.finger.tries >= reader::MAX_MISSES {
                    // (no more tries: the reader rests, the password is there;
                    // the island goes back to the field instead of a lasting miss)
                    self.finger.text.clear();
                    self.finger.state.clear();
                }
                self.field();
            }
            M::FaceMissOver => {
                self.face.missed = false;
                self.field();
            }
            M::FaceRetry => {
                if self.face_wanted(st) && self.face.conv.is_none() {
                    self.face.text = "Looking for your face…".into();
                    self.conv_id += 1;
                    self.face.conv = Some(auth::start(&self.backend, Kind::Face, self.conv_id, String::new(), Self::send(st)));
                    self.field();
                }
            }
            M::Sleep(going) => {
                // Readers stop before a sleep and start fresh after it: a
                // fingerprint check left running across one kept fprintd's
                // reader claimed, every new one was refused at once, and the
                // lock retried in a loop (18 Nov)
                // 18 Nov, the second time: fprintd couldn't close a reader that
                // left USB mid-scan, and stayed stuck. Now the scans end cleanly
                // while logind waits (our delay inhibitor), and the reader is
                // let go before the inhibitor is (reader::before_sleep).
                self.t_finger.stop();
                self.t_face.stop();
                self.finger.gen += 1;
                self.finger.probing = false;
                if going {
                    self.sleeping = true;
                    let convs: Vec<Conv> = [self.finger.conv.take(), self.face.conv.take()].into_iter().flatten().collect();
                    let fd = self.sleep_fd.take();
                    eprintln!("tessera-ui: lock: going to sleep: ending {} scan(s){}", convs.len(), if fd.is_some() { " before letting the sleep go" } else { "" });
                    std::thread::spawn(move || {
                        let t = std::time::Instant::now();
                        let s = reader::before_sleep(convs, fd, &reader::SystemFprint, Duration::from_secs(2));
                        eprintln!("tessera-ui: lock: ready to sleep after {} ms ({s:?})", t.elapsed().as_millis());
                    });
                } else {
                    eprintln!("tessera-ui: lock: awake: a fresh scan once the reader is back");
                    self.sleeping = false;
                    self.finger.tries = 0;
                    self.finger.busy = 0;
                    let now = crate::util::now_ms();
                    self.finger.activity_at = now;
                    self.tick_ms = now;
                    // (the USB reader comes back a moment after the system: waited for)
                    self.fresh(st, reader::Why::Wake);
                    if self.held {
                        self.face.armed = true;
                        self.face.tries = 0;
                        self.face.broken = false;
                    }
                }
                self.rules(st);
            }
            M::Lockout(conf, tally) => {
                self.faillock = conf;
                let left = logic::lockout(&conf, tally.as_deref(), self.fails, crate::util::now_ms(), |y, mo, d, h, mi, s| crate::services::calendar::make(y, mo - 1, d, h, mi, s));
                if let (Some(left), false) = (left, self.error.is_empty()) {
                    self.error = logic::locked_text(left);
                    self.field();
                }
            }
            M::Wallpaper(path, size, px) => {
                let Some((sharp, soft)) = px else {
                    // (no picture: the entrance over the plain colour)
                    self.wins.iter_mut().for_each(reveal);
                    return;
                };
                let (lo, hi) = field_light(&soft);
                let (sharp, soft) = (Image::from_rgb8(sharp), Image::from_rgb8(soft));
                for w in &mut self.wins {
                    if w.picture.0 == path && w.picture.1 == size {
                        let d = w.ui.global::<ui::LockData>();
                        d.set_wallpaper(sharp.clone());
                        d.set_soft(soft.clone());
                        d.set_field_lo(lo);
                        d.set_field_hi(hi);
                        d.set_has_wallpaper(true);
                        w.sharp = Some(sharp.clone());
                    }
                }
                self.intro.picture(&path, &sharp, &soft);
            }
            M::Photo(path, px) => {
                if self.sb.photos.get(self.sb.photo % self.sb.photos.len().max(1)) != Some(&path) {
                    return;
                }
                let img = px.map(Image::from_rgb8);
                self.each(|d| {
                    d.set_has_photo(img.is_some());
                    d.set_photo(img.clone().unwrap_or_default());
                });
            }
            M::Avatar(files) => {
                self.avatar = files.map(|(round, cookie)| (shape::to_image(round, 128), shape::to_image(cookie, 128)));
                if let Some((round, cookie)) = &self.avatar {
                    self.each(|d| {
                        d.set_avatar(round.clone());
                        d.set_avatar_cookie(cookie.clone());
                        d.set_has_avatar(true);
                    });
                }
            }
            M::Art(url, files) => {
                if url != self.art.0 {
                    return;
                }
                let imgs = files.map(|(cookie, round)| (shape::to_image(cookie, 120), shape::to_image(round, 120)));
                if let Some((cookie, round)) = &imgs {
                    self.each(|d| {
                        d.set_art_cookie(cookie.clone());
                        d.set_art(round.clone());
                        d.set_has_art(true);
                    });
                }
                self.art.1 = imgs;
            }
            M::Position(bus, pos) => {
                if bus == self.position.0 && pos != self.position.1 {
                    self.position.1 = pos;
                    self.player(st);
                }
            }
            M::RevealNow => self.wins.iter_mut().for_each(reveal),
            M::Retake => {
                if self.held && self.lock.is_none() && !self.demo {
                    eprintln!("tessera-ui: lock: taking the session lock again (try {})", self.retakes);
                    self.take(st);
                    self.rules(st);
                }
            }
        }
    }

    fn after_frame(&mut self, st: &mut State) {
        let kbd = st.keyboard_surface();
        // (the picture showing now, as of the last settings: not read each frame)
        if self.wall_path.is_empty() {
            self.wall_path = crate::wallpaper::showing_now(&st.shell.cfg, st.shell.look.material);
        }
        let path = self.wall_path.clone();
        let mut sized = false;
        for i in 0..self.wins.len() {
            let w = &mut self.wins[i];
            let f = kbd.as_ref().is_some_and(|k| Rc::ptr_eq(k, &w.surface));
            if let Some(a) = w.a11y.as_mut() {
                a.update(false, f);
            }
            // the entrance plays once the screen has shown its first frame
            // and has its wallpaper (else the circle opens over black and
            // the picture cuts in after); 800 ms at most
            if w.surface.mapped.get() && !w.revealed && !w.waiting {
                w.waiting = true;
                sized = true;
                let to = Self::send(st);
                slint::Timer::single_shot(Duration::from_millis(800), move || to(Box::new(M::RevealNow)));
            }
            if w.waiting && (w.sharp.is_some() || path.is_empty()) {
                reveal(w);
            }
            // the wallpaper at this screen's size
            let (lw, lh) = w.surface.logical_size();
            let scale = w.ui.window().scale_factor();
            let size = (((lw as f32) * scale) as u32, ((lh as f32) * scale) as u32);
            if lw > 1 && (w.picture.0 != path || w.picture.1 != size) {
                w.picture = (path.clone(), size);
                let to = Self::send(st);
                let p = path.clone();
                std::thread::spawn(move || {
                    let px = decode_pair(&p, size.0, size.1);
                    to(Box::new(M::Wallpaper(p, size, px)));
                });
            }
        }
        // (its size known: the cards that fit, Bedside Mode's spot)
        if sized {
            self.notes_push();
            self.drift(st, true);
        }
        self.intro.after_frame(st, &path);
    }

    fn command(&mut self, _st: &mut State, _cmd: &crate::shell::Command) {}

    fn a11y(&mut self) -> Vec<&mut crate::a11y::A11y> {
        self.wins.iter_mut().filter_map(|w| w.a11y.as_mut()).collect()
    }

    fn stats(&self) {
        for w in &self.wins {
            eprintln!("tessera-ui: stats {}", w.surface.stats.borrow_mut().report("lock"));
        }
    }

    fn quit(&mut self, _st: &mut State) {
        // (the lock itself: on the way out without an unlock, the compositor
        // keeps the session locked, as ext-session-lock asks)
        for c in [self.pass_conv.take(), self.finger.conv.take(), self.face.conv.take()].into_iter().flatten() {
            c.abort();
        }
    }
}

impl Lock {
    /// A line from the shell (Lock.qml, LoginIntro.qml)
    fn told(&mut self, st: &mut State, v: &serde_json::Value) {
        if let Some(f) = v["finger"].as_bool() {
            self.finger_on = f;
        }
        if let Some(f) = v["face"].as_bool() {
            self.face_on = f;
        }
        // (a regular camera's PAM file asks for a blink too: auth::service)
        if let Some(k) = v["faceKind"].as_str() {
            auth::set_face_camera(k, v["faceDevice"].as_str().unwrap_or(""));
        }
        if let Some(l) = v["leaving"].as_bool() {
            // (Material's fingerprint shows the check while it fades: lock.slint)
            self.leaving = l && self.held;
        }
        if let Some(a) = v["notes"].as_array() {
            self.notes = a
                .iter()
                .map(|n| Note {
                    key: n["key"].as_str().unwrap_or("").into(),
                    app: n["app"].as_str().unwrap_or("").into(),
                    icon: n["icon"].as_str().unwrap_or("").into(),
                    summary: n["summary"].as_str().unwrap_or("").into(),
                    body: n["body"].as_str().unwrap_or("").into(),
                    at: n["at"].as_f64().unwrap_or(0.0) as i64,
                })
                .collect();
            self.notes_push();
        }
        // The veil (Lock.qml's lockveil), as LockFx steps: made from the
        // lock's own pictures while they're still here (before `held` goes)
        if let Some(step) = v["veil"].as_str() {
            let pictures: Vec<(String, Image)> = self.wins.iter().filter_map(|w| w.sharp.clone().map(|p| (w.output.clone(), p))).collect();
            let (zoom, ms) = (v["zoom"].as_bool().unwrap_or(true), v["ms"].as_f64().unwrap_or(0.0));
            self.veil.step(st, step, zoom, ms, &pictures);
        }
        if let Some(h) = v["held"].as_bool() {
            self.set_held(st, h);
        }
        // Input at all while locked, heard by the compositor's idle notification
        // (host-shell, Lock.qml's IdleMonitor): someone is there, unless Bedside
        // Mode's own rules decide what wakes it
        if v["wake"].as_bool() == Some(true) && self.held && !self.standby(st) {
            self.woke(st);
        }
        if let Some(d) = v["demo"].as_str() {
            self.demo_state(st, d);
        }
        if let Some(i) = v["intro"].as_str() {
            let pace = v["pace"].as_f64().unwrap_or(1.0).max(1.0);
            self.intro.command(st, i, pace);
        }
        self.rules(st);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_password_answer_counts_whatever_the_reader_and_camera_started_since() {
        use super::is_pass;
        // the check is conversation 5; the reader's retry took 6, the camera's 7
        assert!(is_pass(5, 5));
        assert!(!is_pass(6, 5) && !is_pass(7, 5));
        // stopped after 20 s: a late answer is nobody's
        assert!(!is_pass(5, 0) && !is_pass(0, 0));
    }

    #[test]
    fn hands_only_turn_forward() {
        use super::forward;
        assert_eq!(forward(-1.0, 354.0), 354.0);
        // 354° → 0°: six degrees on, not round the other way
        assert_eq!(forward(354.0, 0.0), 360.0);
        assert_eq!(forward(360.0, 6.0), 366.0);
        assert_eq!(forward(366.0, 6.0), 366.0);
        assert_eq!(forward(725.0, 10.0), 730.0);
    }

    #[test]
    fn modes_from_the_environment() {
        // (the test binary has no --demo: real unless a sandbox says so)
        assert!(!super::demo_mode() || std::env::var("TESSERA_UI_DEMO").is_ok());
    }
}

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
//! sandbox (TESSERA_SANDBOX / IRICE_SANDBOX ≠ 1).
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
//!   logind's PrepareForSleep (system bus, a signal watch, read-only): only
//!     when real and the reader or the face is on.
//!   faillock --user (read-only): only when real, after a wrong password.
//!   Idle inhibitor (zwp_idle_inhibit_v1) on a lock surface while Bedside
//!     Mode shows: whenever it shows (demo too: the arena's compositor).
//!
//! ── With the shell (Lock.qml, LoginIntro.qml; JSON lines) ─────────────────
//!   stdin  {"to":"lock","held":…,"leaving":…,"finger":…,"face":…}
//!          {"to":"lock","notes":[{key,app,icon,summary,body,at}…]}
//!          {"to":"lock","demo":"standby|wake|finger|face|match|typing|wrong|busy|lockout|off"}
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
//! only (the QML's is Notification Center's, with Week and To-do); Bedside
//! Mode's photos change every 20 s without a cross-fade between pictures;
//! the wavy timeline in Material's Now Playing is a straight bar (and the
//! position isn't read, so it doesn't show yet); the small analog clock is
//! the dial without its hands; Glass's error text wraps under the field
//! (the QML's runs on one line, past the column); the wallpaper is read again
//! when the settings change, not as a time-of-day picture turns over; "touch"
//! that wakes the face camera is input on the lock's own surfaces (the QML
//! also counts the compositor's idle notification, e.g. with the screen off).

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
pub mod shape;

use tessera_core::services::calendar::Calendar;
use tessera_core::services::mpris::{self, Players};
use tessera_core::services::status::Battery;
use tessera_core::services::todos::Todos;
use tessera_core::services::weather::Weather;
use tessera_core::shell::Part;
use tessera_core::wayland::{LockEvent, Spec, State, Surface};
use auth::{Backend, Conv, Event as Auth, Kind, Outcome};
use slint::{ComponentHandle, Image, ModelRc, VecModel};
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

enum M {
    Edited(String),
    Submit,
    Woke,
    Step(i32),
    Player(String),
    Second,
    IdleOver,
    FingerRetry,
    FingerMissOver,
    FaceRetry,
    RestOver,
    PhotoNext,
    Sleep(bool),
    Lockout(logic::Faillock, Option<String>),
    Wallpaper(String, (u32, u32), Option<(slint::SharedPixelBuffer<slint::Rgb8Pixel>, slint::SharedPixelBuffer<slint::Rgb8Pixel>)>),
    Photo(String, Option<slint::SharedPixelBuffer<slint::Rgb8Pixel>>),
    Avatar(Option<(Vec<u8>, Vec<u8>)>),
    Art(String, Option<(Vec<u8>, Vec<u8>)>),
}

struct Win {
    output: String,
    surface: Rc<Surface>,
    ui: ui::LockScreen,
    a11y: Option<crate::a11y::A11y>,
    inhibitor: Option<ZwpIdleInhibitorV1>,
    revealed: bool,
    /// the wallpaper decoded for this size (path, size)
    picture: (String, (u32, u32)),
    own_fy: f64,
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

#[derive(Default)]
struct Reader {
    text: String,
    state: String,
    tries: u32,
    asked_at: i64,
    started_at: i64,
    asked: bool,
    busy: u32,
    conv: Option<Conv>,
    miss_shown: bool,
}

#[derive(Default)]
struct FaceCam {
    text: String,
    tries: u32,
    broken: bool,
    armed: bool,
    stopped_at: i64,
    conv: Option<Conv>,
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
    error: String,
    busy: bool,
    fails: u32,
    pam_locked: bool,
    error_count: i32,
    conv_id: u64,
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
    faillock: logic::Faillock,
    avatar: Option<(Image, Image)>,
    avatar_asked: bool,
    art: (String, Option<(Image, Image)>),
    t_second: slint::Timer,
    t_idle: slint::Timer,
    t_finger: slint::Timer,
    t_finger_miss: slint::Timer,
    t_face: slint::Timer,
    t_rest: slint::Timer,
    t_photo: slint::Timer,
    intro: intro::Intro,
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
            error: String::new(),
            busy: false,
            fails: 0,
            pam_locked: false,
            error_count: 0,
            conv_id: 0,
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
            faillock: logic::Faillock::default(),
            avatar: None,
            avatar_asked: false,
            art: (String::new(), None),
            t_second: slint::Timer::default(),
            t_idle: slint::Timer::default(),
            t_finger: slint::Timer::default(),
            t_finger_miss: slint::Timer::default(),
            t_face: slint::Timer::default(),
            t_rest: slint::Timer::default(),
            t_photo: slint::Timer::default(),
            intro: intro::Intro::default(),
        }
    }
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
        self.finger.tries = 0;
        self.finger.busy = 0;
        self.sb.idle = false;
        self.password.clear();
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
            self.pass_conv = None;
            self.busy = false;
            self.error.clear();
        }
        self.rules(st);
    }

    /// Lock the session (or, in `--demo`, cover the screens) and make a
    /// screen for every output
    fn take(&mut self, st: &mut State) {
        if !self.demo {
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
        on!(on_edited, |t| M::Edited(t.to_string()));
        on!(on_submit, | | M::Submit);
        on!(on_woke, | | M::Woke);
        on!(on_step, |i| M::Step(i));
        on!(on_player, |a| M::Player(a.to_string()));
        ui.show().expect("show lock");
        let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
        self.wins.push(Win { output: name.to_string(), surface, ui, a11y, inhibitor: None, revealed: false, picture: (String::new(), (0, 0)), own_fy: 0.0 });
        let i = self.wins.len() - 1;
        self.fill(st, i);
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
        let roles: std::collections::HashMap<String, String> = if look.material { crate::theme::material_roles(&look.seed, look.dark, &look.style).into_iter().collect() } else { Default::default() };
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
        let first = crate::services::calendar::local(crate::services::calendar::make(t.y, t.mo, 1, 12, 0, 0));
        let days: Vec<ui::LDay> = logic::month_grid(first.wday as u32, logic::days_in_month(t.y, t.mo as u32))
            .into_iter()
            .map(|(dd, col, row)| ui::LDay { label: dd.to_string().into(), col: col as i32, row: row as i32, today: dd == t.d as u32 })
            .collect();
        let month = format!("{} {}", &logic::month_name(t.mo as u32)[..3], t.y);
        self.each(|d| {
            d.set_date(date.clone().into());
            d.set_time(logic::clock(h, m, h24).into());
            d.set_hh(hh.clone().into());
            d.set_hh2(format!("{:0>2}", hh).into());
            d.set_mm(format!("{m:02}").into());
            d.set_half(half.clone().into());
            d.set_angle_h(((h % 12) as f32 + m as f32 / 60.0) * 30.0);
            d.set_angle_m(m as f32 * 6.0);
            d.set_sb_night(night);
            d.set_days(ModelRc::new(VecModel::from(days.clone())));
            d.set_month(month.clone().into());
        });
        // (the notes' "5m ago" moves with the clock)
        if !force {
            self.notes_push();
        }
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
    fn notes_push(&self) {
        let now = crate::util::now_ms();
        let h24 = self.wins.first().is_some_and(|w| w.ui.global::<ui::LockData>().get_h24());
        let newest: Vec<&Note> = self.notes.iter().rev().collect();
        let keys: Vec<String> = newest.iter().map(|n| n.key.clone()).collect();
        let apps = logic::shelf_apps(&keys);
        let icon = |n: &Note| -> Option<Image> {
            let i = n.icon.as_str();
            let path = if let Some(f) = i.strip_prefix("file://") {
                Some(std::path::PathBuf::from(f))
            } else if i.starts_with('/') {
                Some(std::path::PathBuf::from(i))
            } else {
                let name = i.strip_prefix("image://icon/").unwrap_or(i);
                let name = name.split('?').next().unwrap_or(name);
                crate::icon_theme::find(if name.is_empty() { "dialog-information" } else { name }, None)
            };
            path.and_then(|p| Image::load_from_path(&p).ok())
        };
        let make = |n: &Note| {
            let img = icon(n);
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
        for w in &self.wins {
            let d = w.ui.global::<ui::LockData>();
            // the cards this screen shows: the newest (shelf), the latest
            // three (Glass) or four (Material) beside the clock on a wide
            // screen (column), none (icons)
            let (sw, sh) = w.surface.logical_size();
            let style = d.get_note_style();
            let split = style == "column" && sw > sh;
            let n = if style == "icons" { 0 } else if !split { 1 } else if self.material { 4 } else { 3 };
            d.set_notes(ModelRc::new(VecModel::from(all.clone())));
            d.set_cards(ModelRc::new(VecModel::from(all.iter().take(n).cloned().collect::<Vec<_>>())));
            d.set_apps(ModelRc::new(VecModel::from(shelf.clone())));
            d.set_note_count(newest.len() as i32);
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
        self.each(|d| {
            d.set_has_player(has);
            d.set_track(p.as_ref().map(|p| p.track.title.clone()).unwrap_or_default().into());
            d.set_artist(p.as_ref().map(|p| p.track.artist.clone()).unwrap_or_default().into());
            d.set_playing(p.as_ref().is_some_and(|p| p.playing));
            // (the position isn't read here: the bar shows only while known)
            d.set_has_timeline(false);
        });
    }

    /// The field, the reader and the camera
    fn field(&self) {
        let face_looking = self.face.conv.is_some();
        let pw = self.password.clone();
        self.each(|d| {
            if d.get_password().as_str() != pw {
                d.set_password(pw.clone().into());
            }
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
        });
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
            self.load_photo(st);
        } else if !photo_on {
            self.t_photo.stop();
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
        let (w, h) = self.wins.iter().map(|w| w.surface.logical_size()).max().unwrap_or((1920, 1080));
        let to = Self::send(st);
        std::thread::spawn(move || {
            let px = crate::picture::decode(&path, w, h);
            to(Box::new(M::Photo(path, px)));
        });
    }

    // ── Rules: what's wanted now (Lock.qml's bindings) ─────────────────────

    fn finger_wanted(&self) -> bool {
        self.real && self.held && !self.leaving && !self.sleeping && self.finger_on && self.finger.tries < 5
    }

    fn face_wanted(&self, st: &State) -> bool {
        self.real && self.held && !self.leaving && !self.standby(st) && self.face_on && self.face.armed && !self.face.broken && self.face.tries < 3
    }

    /// After anything changed: start or stop the reader, the camera, the
    /// idle timer, Bedside Mode; then show it all
    fn rules(&mut self, st: &mut State) {
        // the reader
        if self.finger_wanted() {
            if self.finger.conv.is_none() && !self.t_finger.running() {
                self.finger_retry(st, 400);
            }
        } else {
            self.t_finger.stop();
            if let Some(c) = self.finger.conv.take() {
                c.abort();
            }
            if self.real {
                self.finger.text.clear();
                self.finger.state.clear();
            }
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
            self.finger.tries = 0;
            self.face.tries = 0;
            if self.finger.busy > 50 {
                self.finger.busy = 0;
                self.finger_retry(st, 400);
            }
        }
        self.rules(st);
    }

    fn submit(&mut self, st: &mut State) {
        if self.busy || self.password.is_empty() || !self.held {
            return;
        }
        self.error.clear();
        self.finger.tries = 0;
        self.face.tries = 0;
        self.busy = true;
        self.conv_id += 1;
        self.pass_conv = Some(auth::start(&self.backend, Kind::Password, self.conv_id, self.password.clone(), Self::send(st)));
        self.rules(st);
    }

    /// Unlocked here: the shell lets the lock go (System.locked = false)
    fn unlocked(&mut self) {
        self.error.clear();
        self.fails = 0;
        crate::shell::tell(NAME, serde_json::json!({"unlocked": true}));
    }

    fn set_password(&mut self, st: &mut State, text: String) {
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
            Auth::Info(Kind::Password, id, m) | Auth::Error(Kind::Password, id, m) if id == self.conv_id => {
                if m.to_lowercase().contains("locked") {
                    self.pam_locked = true;
                }
            }
            Auth::Done(Kind::Password, id, out) if id == self.conv_id => {
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
                    Outcome::Error(_) => {
                        self.error = "Couldn't check your password. Try again.".into();
                        self.error_count += 1;
                    }
                }
                self.pam_locked = false;
                self.password.clear();
                self.rules(st);
            }
            Auth::Info(Kind::Finger, id, m) | Auth::Error(Kind::Finger, id, m) if self.finger.conv.as_ref().is_some_and(|c| c.id == id) => {
                match logic::finger_message(&m) {
                    logic::Finger::Miss => {
                        if now - self.finger.asked_at < 700 {
                            return; // (not a touch: pam_fprintd asks again by itself)
                        }
                        self.finger.tries += 1;
                        self.finger.text = "Not recognised. Try again".into();
                        self.finger.state = "miss".into();
                        self.finger.miss_shown = true;
                        let to = Self::send(st);
                        self.t_finger_miss.start(slint::TimerMode::SingleShot, Duration::from_millis(1600), move || to(Box::new(M::FingerMissOver)));
                        if self.finger.tries >= 5 {
                            if let Some(c) = self.finger.conv.take() {
                                c.abort();
                            }
                        }
                    }
                    logic::Finger::Ask => {
                        self.finger.asked_at = now;
                        self.finger.asked = true;
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
                    self.unlocked();
                    self.field();
                    return;
                }
                // over at once, before it asked: the reader was busy
                if !self.finger.asked && now - self.finger.started_at < 2000 {
                    self.finger.busy += 1;
                    if self.finger.busy > 50 {
                        self.finger.text.clear();
                        self.finger.state.clear();
                    } else {
                        self.finger_retry(st, 300);
                    }
                    self.field();
                    return;
                }
                self.finger.busy = 0;
                if matches!(out, Outcome::Error(_)) {
                    self.finger.tries += 1;
                }
                if self.finger_wanted() {
                    self.finger_retry(st, 400);
                }
                self.rules(st);
            }
            Auth::Done(Kind::Face, id, out) if self.face.conv.as_ref().is_some_and(|c| c.id == id) => {
                self.face.conv = None;
                match out {
                    Outcome::Success => {
                        self.face.tries = 0;
                        self.face.text.clear();
                        self.unlocked();
                    }
                    Outcome::Failure(_) => {
                        self.face.tries += 1;
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
            }
            "wake" => self.woke(st),
            "finger" => {
                self.face.text.clear();
                self.finger.text = WAITING.into();
                self.finger.state = "waiting".into();
            }
            "face" => {
                self.finger.text.clear();
                self.face.text = "Looking for your face…".into();
            }
            "match" => {
                self.finger.state = "match".into();
                self.finger.text.clear();
                self.face.text.clear();
            }
            "typing" => {
                self.sb.idle = false;
                self.password = "tessera".into();
            }
            "wrong" => {
                self.password.clear();
                self.error = "Incorrect password. Try again.".into();
                self.error_count += 1;
            }
            "busy" => {
                self.password = "tessera".into();
                self.busy = true;
            }
            "lockout" => {
                self.password.clear();
                self.busy = false;
                self.error = logic::locked_text(540);
                self.error_count += 1;
            }
            "miss" => {
                self.finger.text = "Not recognised. Try again".into();
                self.finger.state = "miss".into();
            }
            _ => {
                self.sb.demo_charging = false;
                self.sb.idle = false;
                self.finger.text.clear();
                self.finger.state.clear();
                self.face.text.clear();
                self.error.clear();
                self.busy = false;
                self.password.clear();
            }
        }
        // (the face spinner while "Looking for your face…" shows)
        for w in &self.wins {
            w.ui.global::<ui::LockData>().set_demo_face(what == "face");
        }
        self.rules(st);
    }
}

/// logind's PrepareForSleep, read-only (Lock.qml's EventWatch "login1")
fn watch_sleep(to: impl Fn(Box<dyn Any + Send>) + Send + 'static) {
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
                            st.drop_surface(&w.surface);
                        }
                        if self.secure {
                            self.secure = false;
                            crate::shell::tell(NAME, serde_json::json!({"secure": false}));
                        }
                        crate::shell::tell(NAME, serde_json::json!({"refused": true}));
                    }
                }
                return;
            }
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
            M::Edited(t) => self.set_password(st, t),
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
                    if let Some(c) = self.finger.conv.take() {
                        c.abort();
                    }
                    self.finger_retry(st, 300);
                }
                self.tick_ms = now;
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
                    self.sb.photo = (logic::random() * self.sb.photos.len() as f64) as usize % self.sb.photos.len();
                    self.load_photo(st);
                }
            }
            M::FingerRetry => {
                if !self.finger_wanted() {
                    return;
                }
                if self.finger.conv.is_some() {
                    self.finger_retry(st, 400);
                    return;
                }
                let now = crate::util::now_ms();
                self.finger.text = WAITING.into();
                self.finger.state = "waiting".into();
                self.finger.asked_at = now;
                self.finger.started_at = now;
                self.finger.asked = false;
                self.conv_id += 1;
                self.finger.conv = Some(auth::start(&self.backend, Kind::Finger, self.conv_id, String::new(), Self::send(st)));
                self.field();
            }
            M::FingerMissOver => {
                self.finger.miss_shown = false;
                if self.finger_wanted() && self.finger.conv.is_some() {
                    self.finger.text = WAITING.into();
                    self.finger.state = "waiting".into();
                }
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
                if going {
                    self.sleeping = true;
                } else {
                    self.sleeping = false;
                    self.finger.tries = 0;
                    self.finger.busy = 0;
                    self.tick_ms = crate::util::now_ms();
                    if self.finger_wanted() {
                        self.finger_retry(st, 300);
                    }
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
                let Some((sharp, soft)) = px else { return };
                let (sharp, soft) = (Image::from_rgb8(sharp), Image::from_rgb8(soft));
                for w in &mut self.wins {
                    if w.picture.0 == path && w.picture.1 == size {
                        let d = w.ui.global::<ui::LockData>();
                        d.set_wallpaper(sharp.clone());
                        d.set_soft(soft.clone());
                        d.set_has_wallpaper(true);
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
            if w.surface.mapped.get() && !w.revealed {
                w.revealed = true;
                w.ui.global::<ui::LockData>().set_revealed(true);
                sized = true;
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
        if let Some(h) = v["held"].as_bool() {
            self.set_held(st, h);
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
    fn modes_from_the_environment() {
        // (the test binary has no --demo: real unless a sandbox says so)
        assert!(!super::demo_mode() || std::env::var("TESSERA_UI_DEMO").is_ok() || std::env::var("IRICE_UI_DEMO").is_ok());
    }
}

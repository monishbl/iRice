//! The login screen (greetd's greeter), in Rust: `tessera-ui --greeter`
//! (or the binary named tessera-greeter), in place of `qs -p
//! shell/greeter.qml`. Not a shell surface: it runs alone in the greeter's
//! own compositor (hypr/greeter.lua), before anyone has logged in, so it has
//! its own loop (`run`) and isn't in src/surfaces.rs's KNOWN.
//!
//! Its styles, as the QML greeter's (TESSERA_GREETER_STYLE, or --style):
//!   look   the look's own: Glass (GreeterSurface.qml) or Material
//!          (material/modules/greeter/GreeterSurface.qml), from config.json's
//!          appearance.look (greeter-start.sh writes one for the greeter)
//!   text   ly's box of monospace fields (GreeterText.qml)
//!   plain  black, your picture, the field (GreeterPlain.qml)
//! The logic is model.rs (GreeterAuth.qml and GreeterWindow.qml), the
//! protocol greetd.rs (greetd's JSON over $GREETD_SOCK), what's read from
//! the system system.rs, the words view.rs, the markup ui/greeter.slint.
//! A crate of its own (tessera-greeter); the binary's main.rs hands over to
//! `run` for `--greeter`, before anything else.
//!
//!   tessera-ui --greeter [--style look|text|plain] [--look glass|material]
//!              [--demo [STATE]] [--dry]
//! Without $GREETD_SOCK, or with TESSERA_GREETER_MOCK=1, it's a preview: the
//! password "tessera" "logs in" (it quits), Escape quits (Settings' Preview).
//! `--demo` draws it with made-up people and sessions (never anyone's real
//! account) in one of its states, for the sandbox and for review:
//!   users (default), typing, wrong, busy, finger, finger-miss, session,
//!   power, other, none (no one listed), single (one person)
//! A demo still talks to $GREETD_SOCK when it's set (the sandbox's fake
//! greetd: native/tessera-ui/tools/fake-greetd.py), else it's a preview.
//! `--dry` (and any demo or preview, and TESSERA_GREETER_DRY=1): Sleep,
//! Restart and Shut Down are only written to stderr.
//!
//! Not like the QML yet: Google Sans Flex's width and roundness axes (the
//! Material clock's wdth, ROND) and tabular figures can't be set in Slint,
//! so the clock's digits are the font's default width; Material Symbols draw
//! without their FILL axis; the Material spinner is the shared Busy arc,
//! not the morphing shape; the password's caret is drawn by hand (Slint's
//! own would sit after its own ● dots, not this style's •); the analog
//! hands animate the short way round at the hour.

pub mod ui {
    slint::include_modules!();
}
tessera_core::theme_global!(ui);

// tessera-core's modules (shell, wayland, theme, services, util…) under the
// names they had when everything was one crate: `crate::shell::Part`
#[allow(unused_imports)]
use tessera_core::*;

pub mod greetd;
pub mod model;
pub mod system;
pub mod view;

use tessera_core::shell::Part;
use tessera_core::wayland::{Spec, State, Surface};
use tessera_core::Msg;
use greetd::{Client, Event};
use model::{Effect, Finger, Model};
use slint::{ComponentHandle, Rgba8Pixel, SharedPixelBuffer};
use smithay_client_toolkit::reexports::client::protocol::wl_output::WlOutput;
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use smithay_client_toolkit::shell::WaylandSurface;
use std::any::Any;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;
use view::Style;

const NAME: &str = "greeter";
const NS: &str = "tessera-greeter";

/// Asked for: `--greeter`, or run as tessera-greeter
pub fn wanted(args: &[String]) -> bool {
    args.iter().skip(1).any(|a| a == "--greeter")
        || args.first().and_then(|a| std::path::Path::new(a).file_name()).is_some_and(|n| n == "tessera-greeter")
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Options {
    pub style: String,
    pub look: Option<String>,
    pub demo: Option<String>,
    pub dry: bool,
}

impl Options {
    pub fn from_args(args: &[String], env_style: Option<String>, env_dry: bool) -> Options {
        let mut o = Options { style: env_style.filter(|s| !s.is_empty()).unwrap_or_else(|| "look".into()), dry: env_dry, ..Default::default() };
        let mut it = args.iter().skip(1).peekable();
        while let Some(a) = it.next() {
            match a.as_str() {
                "--style" => o.style = it.next().cloned().unwrap_or_default(),
                "--look" => o.look = it.next().cloned(),
                "--dry" => o.dry = true,
                "--demo" => {
                    let state = it.next_if(|n| !n.starts_with("--")).cloned();
                    o.demo = Some(state.unwrap_or_else(|| "users".into()));
                }
                _ => {
                    if let Some(v) = a.strip_prefix("--style=") {
                        o.style = v.into();
                    } else if let Some(v) = a.strip_prefix("--look=") {
                        o.look = Some(v.into());
                    } else if let Some(v) = a.strip_prefix("--demo=") {
                        o.demo = Some(v.into());
                    }
                }
            }
        }
        if !["look", "text", "plain"].contains(&o.style.as_str()) {
            o.style = "look".into();
        }
        o.look = o.look.filter(|l| l == "glass" || l == "material");
        if o.demo.is_some() {
            o.dry = true;
        }
        o
    }
}

enum M {
    Submit(String),
    NextUser(i32),
    NextSession(i32),
    Cancel,
    EditName,
    ToPassword,
    CancelName,
    NameEdited(String),
    NameKey(String),
    Power(String),
    Row(i32),
    Greetd(Event),
    Begin,
    Mock(String),
    Quit,
    Tick,
    Picture(String, Option<SharedPixelBuffer<Rgba8Pixel>>),
    Primary(String),
}

struct Win {
    output: String,
    surface: Rc<Surface>,
    ui: ui::Greeter,
    a11y: Option<crate::a11y::A11y>,
    settled: bool,
}

pub struct Greeter {
    opts: Options,
    style: Style,
    model: Model,
    host: String,
    h24: bool,
    wins: Vec<Win>,
    /// the output with the keyboard and the controls (Hyprland's focused
    /// one, else the first)
    primary: Option<String>,
    sock: Option<PathBuf>,
    client: Option<Client>,
    begin_timer: slint::Timer,
    mock_timer: slint::Timer,
    quit_timer: slint::Timer,
    clock_timer: slint::Timer,
    clock: view::Clock,
    pictures: HashMap<String, Option<slint::Image>>,
    picture_asked: String,
    /// the typed username the fields show (what was typed there, or pushed)
    name_shown: String,
}

impl Greeter {
    pub fn new(opts: Options) -> Self {
        let demo = opts.demo.is_some();
        let sock = std::env::var_os("GREETD_SOCK").filter(|s| !s.is_empty()).map(PathBuf::from);
        let mock = crate::env::var("GREETER_MOCK").as_deref() == Some("1") || sock.is_none();
        let (users, sessions) = if demo { (system::demo_users(), system::demo_sessions()) } else { (system::users(), system::sessions()) };
        let mut model = Model::new(mock, users, sessions);
        let host = if demo { "tessera".to_string() } else { system::hostname() };
        if !demo {
            if let Some(j) = system::remembered() {
                model.restore(&j);
            }
        }
        if let Some(state) = opts.demo.clone() {
            demo_state(&mut model, &state);
        }
        model.restart();
        Greeter {
            style: Style::Glass,
            opts,
            model,
            host,
            h24: false,
            wins: Vec::new(),
            primary: None,
            sock: if mock { None } else { sock },
            client: None,
            begin_timer: slint::Timer::default(),
            mock_timer: slint::Timer::default(),
            quit_timer: slint::Timer::default(),
            clock_timer: slint::Timer::default(),
            clock: view::Clock::default(),
            pictures: HashMap::new(),
            picture_asked: String::new(),
            name_shown: String::new(),
        }
    }

    fn send(st: &State) -> impl Fn(M) + Clone + 'static {
        let to = st.shell.sender(NAME);
        move |m| to(Box::new(m))
    }

    fn is_primary(&self, output: &str) -> bool {
        self.primary.as_deref() == Some(output)
    }

    fn primary_ui(&self) -> Option<&ui::Greeter> {
        self.wins.iter().find(|w| self.is_primary(&w.output)).map(|w| &w.ui)
    }

    /// Do what the model asked for, until it asks for nothing more
    fn act(&mut self, st: &mut State) {
        loop {
            let effects = self.model.take();
            if effects.is_empty() {
                break;
            }
            for e in effects {
                self.effect(st, e);
            }
        }
        self.push(st);
    }

    fn effect(&mut self, st: &mut State, e: Effect) {
        let to = Self::send(st);
        match e {
            Effect::Send(r) => {
                if self.client.is_none() {
                    if let Some(path) = &self.sock {
                        let to = to.clone();
                        match Client::connect(path, move |ev| to(M::Greetd(ev))) {
                            Ok(c) => self.client = Some(c),
                            Err(err) => eprintln!("greeter: greetd at {}: {err}", path.display()),
                        }
                    }
                }
                let ok = self.client.as_mut().is_some_and(|c| c.send(&r).is_ok());
                if !ok {
                    self.client = None;
                    self.model.lost();
                }
            }
            Effect::BeginLater => {
                let to = to.clone();
                self.begin_timer.start(slint::TimerMode::SingleShot, Duration::from_millis(300), move || to(M::Begin));
            }
            Effect::MockCheck(p) => {
                let to = to.clone();
                self.mock_timer.start(slint::TimerMode::SingleShot, Duration::from_millis(600), move || to(M::Mock(p.clone())));
            }
            Effect::Remember { user, session } => {
                if self.opts.demo.is_none() {
                    system::remember(&user, &session);
                }
            }
            Effect::PreviewDone => {
                let to = to.clone();
                self.quit_timer.start(slint::TimerMode::SingleShot, Duration::from_millis(1200), move || to(M::Quit));
            }
            Effect::Quit => st.exit = true,
            Effect::Power(action) => {
                if self.opts.dry || self.model.mock {
                    eprintln!("greeter (preview): {action}");
                } else {
                    let _ = std::process::Command::new("systemctl").arg(&action).stdin(std::process::Stdio::null()).spawn();
                }
            }
            Effect::ClearField => self.wins.iter().for_each(|w| w.ui.invoke_clear_field()),
            Effect::Refused => self.wins.iter().for_each(|w| w.ui.invoke_shake()),
            Effect::FocusPassword => {
                if let Some(ui) = self.primary_ui() {
                    ui.invoke_focus_password();
                }
            }
            Effect::FocusName => {
                if let Some(ui) = self.primary_ui() {
                    ui.invoke_focus_name(self.model.other_name.clone().into());
                }
            }
        }
    }

    /// The person's picture, decoded off the UI thread (once per name)
    fn want_picture(&mut self, st: &State) {
        let name = self.model.picture_name();
        if name == self.picture_asked || self.style == Style::Text {
            return;
        }
        self.picture_asked = name.clone();
        if self.pictures.contains_key(&name) {
            return;
        }
        let path = if self.opts.demo.is_some() { system::demo_avatar(&name) } else { system::avatar(&name) };
        let Some(path) = path else {
            self.pictures.insert(name, None);
            return;
        };
        let (size, cookie) = match self.style {
            Style::Material => (120, true),
            Style::Plain => (192, false),
            _ => (144, false),
        };
        let to = Self::send(st);
        std::thread::spawn(move || to(M::Picture(name, picture(&path, size, cookie))));
    }

    /// Everything the windows show, from the model
    fn push(&mut self, st: &State) {
        self.want_picture(st);
        let m = &self.model;
        let (say, bad) = view::say(self.style, m);
        let pic = self.pictures.get(&m.picture_name()).cloned().flatten();
        // (a name set from here, not typed in the field: the field follows)
        let name_changed = m.other_name != self.name_shown;
        for w in &self.wins {
            let g = w.ui.global::<ui::Greet>();
            g.set_primary(self.is_primary(&w.output));
            g.set_full(m.person().map(|p| p.full.clone()).unwrap_or_default().into());
            g.set_initial(view::initial(m).into());
            g.set_has_picture(pic.is_some());
            if let Some(p) = &pic {
                g.set_picture(p.clone());
            }
            g.set_several(m.users.len() > 1);
            g.set_other(m.other);
            g.set_busy(m.busy);
            g.set_error(m.error.clone().into());
            g.set_finger_on(m.finger != Finger::None);
            g.set_finger_miss(m.finger == Finger::Miss);
            g.set_finger_say(if m.message.is_empty() { "Fingerprint".into() } else { m.message.clone().into() });
            g.set_power_choice(m.power_choice.clone().into());
            g.set_say(say.clone().into());
            g.set_say_bad(bad);
            g.set_placeholder(view::placeholder(self.style, m).into());
            g.set_session(view::session_label(self.style, m).into());
            g.set_login(m.user_name().into());
            g.set_host(self.host.clone().into());
            g.set_row(m.row);
            if name_changed {
                w.ui.invoke_set_name(m.other_name.clone().into());
            }
        }
        self.name_shown = self.model.other_name.clone();
    }

    fn push_clock(&mut self, st: &State) {
        let c = view::clock(&view::parts_now(), self.h24);
        if c == self.clock {
            return;
        }
        self.clock = c;
        let (face, weight, show_date) = view::material_clock(&st.shell.cfg, self.h24);
        for w in &self.wins {
            let g = w.ui.global::<ui::Greet>();
            let c = &self.clock;
            g.set_date(c.date.clone().into());
            g.set_time(c.time.clone().into());
            g.set_plain_top(c.plain_top.clone().into());
            g.set_text_top(c.text_top.clone().into());
            g.set_hours(c.hours.clone().into());
            g.set_hours_short(c.hours_short.clone().into());
            g.set_minutes(c.minutes.clone().into());
            g.set_half(c.half.clone().into());
            g.set_hour_angle(c.hour_angle);
            g.set_minute_angle(c.minute_angle);
            g.set_face(face.clone().into());
            g.set_weight(weight);
            g.set_show_date(show_date);
        }
    }

    /// What doesn't change: icons, colours, shapes, the shake
    fn dress(&self, st: &State, ui: &ui::Greeter) {
        let g = ui.global::<ui::Greet>();
        let p = |n: &str| slint::SharedString::from(crate::icons::glyph(n));
        g.set_ic(ui::GrIcons {
            user: p("user"),
            caret_left: p("caret-left"),
            caret_right: p("caret-right"),
            user_switch: p("user-switch"),
            x: p("x"),
            arrow_right: p("arrow-right"),
            fingerprint: p("fingerprint"),
            moon: p("moon"),
            restart: p("arrow-clockwise"),
            power: p("power"),
            pencil: p("pencil-simple"),
        });
        let s = |n: &str| {
            let glyph = crate::icons::glyph(n);
            let sym = crate::icons::symbol(glyph);
            if sym.is_empty() {
                ui::GrGlyph { t: glyph.into(), sym: false }
            } else {
                ui::GrGlyph { t: sym.into(), sym: true }
            }
        };
        g.set_sy(ui::GrSyms {
            user: s("user"),
            caret_left: s("caret-left"),
            caret_right: s("caret-right"),
            user_switch: s("user-switch"),
            x: s("x"),
            arrow_right: s("arrow-right"),
            fingerprint: s("fingerprint"),
            moon: s("moon"),
            restart: s("arrow-clockwise"),
            power: s("power"),
        });
        let look = &st.shell.look;
        g.set_m(colors(&crate::theme::material_roles(&look.seed, look.dark, &look.style), look.dark));
        g.set_cookie9(crate::surfaces::shapes::outline("cookie9").into());
        g.set_cookie12(crate::surfaces::shapes::outline("cookie12").into());
        let frames: Vec<f32> = view::shake_frames(self.style);
        g.set_shake_frames(slint::ModelRc::new(slint::VecModel::from(frames)));
        g.on_mask(|t, b| view::mask(&t, &b).into());
        g.on_printable(|t| view::printable(&t));
        let to = Self::send(st);
        macro_rules! on {
            ($set:ident, |$($a:ident),*| $m:expr) => {{
                let to = to.clone();
                g.$set(move |$($a),*| to($m));
            }};
        }
        on!(on_submit, |t| M::Submit(t.to_string()));
        on!(on_next_user, |d| M::NextUser(d));
        on!(on_next_session, |d| M::NextSession(d));
        on!(on_cancel, | | M::Cancel);
        on!(on_edit_name, | | M::EditName);
        on!(on_to_password, | | M::ToPassword);
        on!(on_cancel_name, | | M::CancelName);
        on!(on_name_edited, |t| M::NameEdited(t.to_string()));
        on!(on_name_key, |t| M::NameKey(t.to_string()));
        on!(on_power, |a| M::Power(a.to_string()));
        on!(on_row_changed, |r| M::Row(r));
    }

    /// A demo's state that needs the window: a typed password, an open pill
    fn demo_window(&self, ui: &ui::Greeter) {
        let Some(state) = self.opts.demo.as_deref() else { return };
        if matches!(state, "typing" | "busy") {
            ui.invoke_put("secret12".into());
        }
        if !matches!(state, "users" | "none" | "single") {
            ui.invoke_wake();
        }
        if state == "other" {
            ui.invoke_focus_name(self.model.other_name.clone().into());
        }
    }

    fn set_primary(&mut self, st: &mut State, name: String) {
        if self.primary.as_deref() == Some(name.as_str()) || !self.wins.iter().any(|w| w.output == name) {
            return;
        }
        self.primary = Some(name);
        for w in &self.wins {
            let on = self.is_primary(&w.output);
            w.surface.layer.set_keyboard_interactivity(if on { KeyboardInteractivity::Exclusive } else { KeyboardInteractivity::None });
            w.surface.layer.commit();
        }
        if let Some(ui) = self.primary_ui() {
            ui.invoke_focus_password();
        }
        self.push(st);
    }
}

/// The demo states (`--demo STATE`), set on the model
fn demo_state(m: &mut Model, state: &str) {
    match state {
        "wrong" => m.force(false, "Incorrect password. Try again.", "", Finger::None),
        "busy" => m.force(true, "", "", Finger::None),
        "finger" => m.force(false, "", "Place your finger on the fingerprint reader", Finger::Waiting),
        "finger-miss" => m.force(false, "", "Failed to match fingerprint", Finger::Miss),
        "session" => {
            m.next_session(1);
            m.set_row(0);
        }
        "power" => m.power_choice = "reboot".into(),
        "other" => m.edit_name(),
        "none" => {
            m.users.clear();
            m.edit_name();
            m.set_other_name("");
            m.other = true;
        }
        "single" => m.users.truncate(1),
        _ => {}
    }
    m.take();
}

/// Material's roles for the greeter (material/modules/greeter/GreeterSurface.qml)
fn colors(roles: &[(String, String)], dark: bool) -> ui::GrColors {
    let r = |k: &str| hex(roles.iter().find(|(n, _)| n == k).map_or("#ff00ff", |(_, v)| v.as_str()));
    let pick = |d: &str, l: &str| r(if dark { d } else { l });
    let high = r("surfaceContainerHigh");
    let on = r("onSurface");
    ui::GrColors {
        bg: pick("surfaceContainerLowest", "surfaceContainer"),
        clock1: pick("primaryFixed", "primary"),
        clock2: pick("primaryFixedDim", "onPrimaryFixedVariant"),
        clock3: pick("tertiaryFixed", "tertiary"),
        date: pick("secondaryFixed", "onSurfaceVariant"),
        on_surface: on,
        on_surface_variant: r("onSurfaceVariant"),
        high,
        // (Qt.tint(surfaceContainerHigh, onSurface at 8%))
        high_hover: mix(high, on, 0.08),
        primary: r("primary"),
        on_primary: r("onPrimary"),
        primary_container: r("primaryContainer"),
        on_primary_container: r("onPrimaryContainer"),
        secondary_container: r("secondaryContainer"),
        error: r("error"),
        error_container: r("errorContainer"),
        on_error_container: r("onErrorContainer"),
    }
}

fn hex(s: &str) -> slint::Color {
    let v = u32::from_str_radix(s.trim_start_matches('#'), 16).unwrap_or(0xff00ff);
    slint::Color::from_rgb_u8((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

fn mix(a: slint::Color, b: slint::Color, t: f32) -> slint::Color {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    slint::Color::from_rgb_u8(m(a.red(), b.red()), m(a.green(), b.green()), m(a.blue(), b.blue()))
}

/// How much of a pixel at (x, y) of an n×n square is inside Material's
/// cookie9 (shapes::outline's wave, the same shape the placeholder draws)
pub fn cookie_alpha(n: u32, x: u32, y: u32) -> f32 {
    let s = 100.0 / n as f32;
    let (u, v) = ((x as f32 + 0.5) * s - 50.0, (y as f32 + 0.5) * s - 50.0);
    let d = (u * u + v * v).sqrt();
    let t = v.atan2(u) + std::f32::consts::FRAC_PI_2;
    let r = 46.0 * (1.0 + 0.07 * (9.0 * t).cos());
    ((r - d) / s + 0.5).clamp(0.0, 1.0)
}

/// A picture as the greeter shows it: the middle square, `size` px, cut to
/// the cookie for Material (Glass and plain clip a circle in the markup)
fn picture(path: &std::path::Path, size: u32, cookie: bool) -> Option<SharedPixelBuffer<Rgba8Pixel>> {
    let img = image::ImageReader::open(path).ok()?.with_guessed_format().ok()?.decode().ok()?;
    let (w, h) = (img.width(), img.height());
    let side = w.min(h).max(1);
    let square = img.crop_imm((w - side) / 2, (h - side) / 2, side, side);
    let mut rgba = square.resize_exact(size, size, image::imageops::FilterType::Triangle).to_rgba8();
    if cookie {
        for (x, y, p) in rgba.enumerate_pixels_mut() {
            p.0[3] = (p.0[3] as f32 * cookie_alpha(size, x, y)).round() as u8;
        }
    }
    Some(SharedPixelBuffer::clone_from_slice(rgba.as_raw(), size, size))
}

impl Part for Greeter {
    fn name(&self) -> &'static str {
        NAME
    }

    fn start(&mut self, st: &mut State) {
        self.style = Style::pick(&self.opts.style, st.shell.look.material);
        self.h24 = view::h24(crate::env::var("GREETER_CLOCK24").as_deref(), &st.shell.cfg);
        eprintln!(
            "tessera-ui: greeter, style {}{}{}",
            self.style.key(),
            if self.model.mock { ", preview" } else { "" },
            self.opts.demo.as_deref().map(|d| format!(", demo {d}")).unwrap_or_default()
        );
        // The keyboard's screen: Hyprland's focused one (asked off the UI thread)
        let to = Self::send(st);
        std::thread::spawn(move || {
            let Ok(out) = crate::services::hypr::request("j/monitors") else { return };
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&out) else { return };
            if let Some(name) = v.as_array().and_then(|a| a.iter().find(|m| m["focused"].as_bool() == Some(true))).and_then(|m| m["name"].as_str()) {
                to(M::Primary(name.to_string()));
            }
        });
        let to = Self::send(st);
        self.clock_timer.start(slint::TimerMode::Repeated, Duration::from_secs(1), move || to(M::Tick));
        self.act(st);
    }

    fn output_added(&mut self, st: &mut State, output: &WlOutput, name: &str) {
        if self.wins.iter().any(|w| w.output == name) {
            return;
        }
        if self.primary.is_none() {
            self.primary = Some(name.to_string());
        }
        let primary = self.is_primary(name);
        // (a preview covers the menu bar too)
        let overlay = self.model.mock || self.opts.demo.is_some();
        let surface = st.prepare_surface(Spec {
            layer: if overlay { Layer::Overlay } else { Layer::Top },
            namespace: NS,
            output: Some((output, name)),
            anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
            size: (0, 0),
            exclusive: -1,
            keyboard: if primary { KeyboardInteractivity::Exclusive } else { KeyboardInteractivity::None },
            margin_left: 0,
        });
        let ui = ui::Greeter::new().expect("Greeter");
        st.shell.apply_theme(&ui.global::<ui::Theme>());
        ui.set_style(self.style.key().into());
        self.dress(st, &ui);
        ui.show().expect("show");
        let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
        if primary {
            ui.invoke_focus_password();
        }
        self.wins.push(Win { output: name.to_string(), surface, ui, a11y, settled: false });
        self.clock = view::Clock::default();
        self.push_clock(st);
        // (a new window's field shows the typed name too)
        self.name_shown = "\u{0}".into();
        self.push(st);
        if let Some(w) = self.wins.last() {
            self.demo_window(&w.ui);
        }
    }

    fn output_removed(&mut self, st: &mut State, name: &str) {
        if let Some(i) = self.wins.iter().position(|w| w.output == name) {
            let w = self.wins.remove(i);
            st.drop_surface(&w.surface);
        }
        if self.is_primary(name) {
            self.primary = None;
            if let Some(first) = self.wins.first().map(|w| w.output.clone()) {
                self.set_primary(st, first);
            }
        }
    }

    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Submit(p) => self.model.submit(&p),
            M::NextUser(d) => self.model.next_user(d),
            M::NextSession(d) => self.model.next_session(d),
            M::Cancel => self.model.cancel(),
            M::EditName => self.model.edit_name(),
            M::ToPassword => self.model.to_password(),
            M::CancelName => self.model.cancel_name(),
            M::NameEdited(t) => {
                // (typed in the field: it already shows it)
                self.name_shown = t.clone();
                self.model.set_other_name(&t);
            }
            M::NameKey(t) => self.model.name_key(&t),
            M::Power(a) => self.model.choose_power(&a),
            M::Row(r) => self.model.set_row(r),
            M::Greetd(Event::Response(r)) => self.model.heard(r),
            M::Greetd(Event::Closed) => {
                if self.client.take().is_some() {
                    self.model.lost();
                }
            }
            M::Begin => self.model.begin(),
            M::Mock(p) => self.model.mock_result(&p),
            M::Quit => st.exit = true,
            M::Tick => {
                self.push_clock(st);
                return;
            }
            M::Picture(name, buf) => {
                self.pictures.insert(name, buf.map(slint::Image::from_rgba8));
            }
            M::Primary(name) => {
                self.set_primary(st, name);
                return;
            }
        }
        self.act(st);
    }

    fn surface_closed(&mut self, st: &mut State, surface: &Rc<Surface>) {
        if let Some(i) = self.wins.iter().position(|w| Rc::ptr_eq(&w.surface, surface)) {
            let w = self.wins.remove(i);
            st.drop_surface(&w.surface);
        }
    }

    fn after_frame(&mut self, st: &mut State) {
        let kbd = st.keyboard_surface();
        for w in self.wins.iter_mut() {
            crate::picture::second_frame(&w.surface, &mut w.settled);
            let focused = kbd.as_ref().is_some_and(|k| Rc::ptr_eq(k, &w.surface));
            if let Some(a) = w.a11y.as_mut() {
                a.update(false, focused);
            }
        }
    }

    fn a11y(&mut self) -> Vec<&mut crate::a11y::A11y> {
        self.wins.iter_mut().filter_map(|w| w.a11y.as_mut()).collect()
    }

    fn quit(&mut self, _st: &mut State) {
        self.client = None;
    }
}

/// The greeter's own loop (main.rs's, with this one part and no shell on
/// stdin): ends when the session starts, a preview is done, or on SIGTERM.
/// Without a Wayland display it says so and exits 1 (greeter-start.sh then
/// starts the text one).
pub fn run(args: &[String], started: std::time::Instant) {
    use calloop::signals::{Signal, Signals};
    use calloop::EventLoop;
    use smithay_client_toolkit::reexports::calloop_wayland_source::WaylandSource;

    let opts = Options::from_args(args, crate::env::var("GREETER_STYLE"), crate::env::var("GREETER_DRY").as_deref() == Some("1"));
    if let Some(l) = &opts.look {
        // (the look to draw, whatever config.json says: theme::read follows it)
        std::env::set_var("TESSERA_LOOK", l);
    }
    let mut event_loop: EventLoop<'static, State> = EventLoop::try_new().expect("event loop");
    let lh = event_loop.handle();
    let signals = Signals::new(&[Signal::SIGTERM, Signal::SIGINT]).expect("signalfd");
    lh.insert_source(signals, |_, _, state: &mut State| state.exit = true).unwrap();

    let Ok(conn) = smithay_client_toolkit::reexports::client::Connection::connect_to_env() else {
        eprintln!("tessera-ui: greeter: no Wayland display");
        std::process::exit(1);
    };
    let Ok((globals, queue)) = smithay_client_toolkit::reexports::client::globals::registry_queue_init(&conn) else {
        eprintln!("tessera-ui: greeter: the compositor's registry didn't answer");
        std::process::exit(1);
    };
    let qh = queue.handle();
    WaylandSource::new(conn.clone(), queue).insert(lh.clone()).unwrap();
    let (tx, rx) = calloop::channel::channel::<Msg>();
    lh.insert_source(rx, |ev, _, state: &mut State| {
        if let calloop::channel::Event::Msg(m) = ev {
            tessera_core::runloop::handle(state, m);
        }
    })
    .unwrap();

    let egl = crate::egl::Egl::new(conn.backend().display_ptr() as *mut _).expect("EGL");
    slint::platform::set_platform(Box::new(crate::wayland::Platform { tx: tx.clone() })).expect("platform");
    tessera_core::runloop::register_fonts();
    let glass = crate::services::hypr::Glass::spawn(tx.clone());
    let parts: Vec<Box<dyn Part>> = vec![Box::new(Greeter::new(opts))];
    let shell = crate::shell::Shell::new(tx.clone(), glass, crate::surfaces::Surfaces::default());
    let mut state = State::new(&globals, qh, egl, lh.clone(), tx.clone(), shell, parts, started);
    state.each_part(|p, st| p.start(st));

    while !state.exit {
        let timeout = if state.wants_frame_soon() {
            Some(Duration::ZERO)
        } else if state.animating_unpaced() {
            Some(Duration::from_millis(16))
        } else {
            slint::platform::duration_until_next_timer_update()
        };
        if let Err(e) = event_loop.dispatch(timeout, &mut state) {
            eprintln!("tessera-ui: greeter: event loop: {e}");
            break;
        }
        slint::platform::update_timers_and_animations();
        state.render_all();
        state.each_part(|p, st| p.after_frame(st));
    }
    state.each_part(|p, st| p.quit(st));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(a: &[&str]) -> Vec<String> {
        std::iter::once("tessera-ui").chain(a.iter().copied()).map(String::from).collect()
    }

    #[test]
    fn asked_for_by_flag_or_by_name() {
        assert!(wanted(&args(&["--greeter"])));
        assert!(!wanted(&args(&["--surfaces", "greeter"])));
        assert!(wanted(&["/usr/bin/tessera-greeter".to_string()]));
        assert!(!wanted(&["/usr/bin/tessera-ui".to_string()]));
    }

    #[test]
    fn options() {
        let o = Options::from_args(&args(&["--greeter"]), None, false);
        assert_eq!(o, Options { style: "look".into(), look: None, demo: None, dry: false });
        let o = Options::from_args(&args(&["--greeter", "--style", "text", "--demo", "--look", "material"]), None, false);
        assert_eq!((o.style.as_str(), o.look.as_deref(), o.demo.as_deref(), o.dry), ("text", Some("material"), Some("users"), true));
        let o = Options::from_args(&args(&["--greeter", "--demo", "finger", "--style=plain"]), Some("text".into()), false);
        assert_eq!((o.style.as_str(), o.demo.as_deref()), ("plain", Some("finger")));
        let o = Options::from_args(&args(&["--greeter", "--look", "neon", "--style", "fancy"]), Some("plain".into()), true);
        assert_eq!((o.style.as_str(), o.look, o.dry), ("look", None, true));
        assert_eq!(Options::from_args(&args(&["--greeter"]), Some("plain".into()), false).style, "plain");
    }

    #[test]
    fn demo_states() {
        let fresh = || Model::new(true, system::demo_users(), system::demo_sessions());
        let mut m = fresh();
        demo_state(&mut m, "wrong");
        assert_eq!(m.error, "Incorrect password. Try again.");
        let mut m = fresh();
        demo_state(&mut m, "finger-miss");
        assert_eq!(m.finger, Finger::Miss);
        let mut m = fresh();
        demo_state(&mut m, "session");
        assert_eq!((m.session, m.row), (1, 0));
        let mut m = fresh();
        demo_state(&mut m, "other");
        assert!(m.other && m.other_name == "ada");
        let mut m = fresh();
        demo_state(&mut m, "none");
        assert!(m.other && m.users.is_empty() && m.other_name.is_empty());
        let mut m = fresh();
        demo_state(&mut m, "power");
        assert_eq!(m.power_choice, "reboot");
    }

    #[test]
    fn the_cookie_cuts_the_corners_and_keeps_the_middle() {
        assert_eq!(cookie_alpha(120, 60, 60), 1.0);
        assert_eq!(cookie_alpha(120, 0, 0), 0.0);
        assert_eq!(cookie_alpha(120, 119, 119), 0.0);
        // (the top: a scallop's peak reaches 46 × 1.07 of 50; 20° round, a valley at 46 × 0.93)
        assert!(cookie_alpha(100, 50, 0) < 0.5);
        assert_eq!(cookie_alpha(100, 50, 2), 1.0);
        assert_eq!(cookie_alpha(100, 50, 7), 1.0);
        assert_eq!(cookie_alpha(100, 65, 7), 0.0);
    }

    #[test]
    fn greeter_colours_follow_the_mode() {
        let roles = crate::theme::material_roles("#6750A4", true, "tonalSpot");
        let find = |k: &str| hex(&roles.iter().find(|(n, _)| n == k).unwrap().1);
        let dark = colors(&roles, true);
        assert_eq!(dark.bg, find("surfaceContainerLowest"));
        assert_eq!(dark.clock1, find("primaryFixed"));
        let light_roles = crate::theme::material_roles("#6750A4", false, "tonalSpot");
        let light = colors(&light_roles, false);
        assert_eq!(light.clock1, hex(&light_roles.iter().find(|(n, _)| n == "primary").unwrap().1));
        assert_ne!(dark.high_hover, dark.high);
    }
}

//! Today, the morning screen (rust.surfaces: today), as
//! shell/modules/today/TodayScreen.qml: the screen frosts over (your
//! wallpaper, blurred, under a veil) and a panel the size of the Settings
//! window holds the day: a greeting, today's events and to-dos, the weather.
//! Glass: TodayContent's rows on the plugin's glass; Material:
//! MaterialTodayContent's cards (the date in a cookie, the weather on a sunny
//! shape). Three entrances (bloom, shrink, drop; a fade with Reduce motion),
//! one exit; Esc, Enter or Close put it away, Pin for today keeps it as a
//! widget, Options shows TodayOptions over the panel.
//!
//! When it opens stays the shell's (services/Today.qml: the first unlock of
//! the day, Show Today in Search, `qs ipc call today preview`, the widget):
//! modules/today/TodayScreen.qml tells this process on stdin
//! (`{"to":"today","open":true,"effect":…,"screen":…,"options":…,
//! "motion":…,"accounts":{…}}`, `{"to":"today","open":false}`). What only the
//! shell does goes back as calls (RustUi.qml): today.close, today.pin,
//! today.set (its options), todos.toggle.
//!
//! Data: services::calendar (your events, file subscriptions), todos (your
//! to-dos) and weather (wttr.in) here; the accounts' events and tasks
//! (Google, iCloud, Microsoft 365, calendar links on the network) come from
//! the shell in `accounts`: it holds their sign-in.
//!
//! Not like the QML yet: the content's blur as the panel condenses is a fade
//! (Slint has no blur), the drop's ring has no blur either; Glass's corners
//! are round, not continuous (squircles); Material's icons are Material
//! Symbols without their FILL axis (outlined); Material's segmented buttons
//! don't widen while pressed; numbers aren't tabular (Slint can't ask for
//! the font's tnum); with Material's transparency the panel is its surface
//! container at the panel alpha, not the plugin's neutral "veil" glass (a
//! little warmer than the QML's); the stagger keeps the QML's numbers, so
//! rows ordered past 7 rest below full opacity there too (Glass's buttons
//! at 73%: a QML quirk worth fixing in both).

pub mod ui {
    slint::include_modules!();
}
tessera_core::theme_global!(ui);

// tessera-core's modules (shell, wayland, theme, services, util…) under the
// names they had when everything was one crate: `crate::shell::Part`
#[allow(unused_imports)]
use tessera_core::*;

pub mod model;
pub mod motion;

use tessera_core::services::calendar::{self, Calendar, Event};
use tessera_core::services::hypr::{Glass, Snapshot};
use tessera_core::services::todos::{Todo, Todos};
use tessera_core::services::weather::{self, Weather};
use tessera_core::shell::Part;
use tessera_core::theme;
use tessera_core::wayland::{Spec, State, Surface};
use motion::{Frame, Target, Timeline};
use slint::{ComponentHandle, ModelRc, VecModel};
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use smithay_client_toolkit::shell::WaylandSurface;
use std::any::{Any, TypeId};
use std::collections::HashSet;
use std::rc::Rc;
use std::time::{Duration, Instant};

const NAME: &str = "today";
pub const NS: &str = "tessera:today";

enum M {
    Tick,
    Minute,
    Close,
    Pin,
    Options,
    OptionsDone,
    Toggle(String),
    Mode(i32),
    Entrance(i32),
    Name(String),
    Chime(bool),
    Backdrop(String, Option<slint::SharedPixelBuffer<slint::Rgba8Pixel>>),
}

struct Win {
    output: String,
    surface: Rc<Surface>,
    ui: ui::TodayScreen,
    a11y: Option<crate::a11y::A11y>,
    glass_sent: String,
}

pub struct Today {
    win: Option<Win>,
    /// shown (or coming in); false while it goes and once gone
    open: bool,
    closing: bool,
    timeline: Option<Timeline>,
    started: Instant,
    frame: Frame,
    target: Target,
    fx: String,
    motion: f32,
    options: bool,
    accounts: (Vec<Event>, Vec<Todo>),
    /// ticked off here, until the shell's state says so
    ticked: HashSet<String>,
    account: String,
    weather_settings: (String, bool),
    tick: slint::Timer,
    minute: slint::Timer,
}

impl Default for Today {
    fn default() -> Self {
        Self {
            win: None,
            open: false,
            closing: false,
            timeline: None,
            started: Instant::now(),
            frame: Frame::default(),
            target: motion::target(1920.0, 1080.0, 26.0),
            fx: "bloom".into(),
            motion: 1.0,
            options: false,
            accounts: (Vec::new(), Vec::new()),
            ticked: HashSet::new(),
            account: String::new(),
            weather_settings: (String::new(), true),
            tick: slint::Timer::default(),
            minute: slint::Timer::default(),
        }
    }
}

/// Weather icon names -> the glyph each look draws (Phosphor; Material Symbols)
pub fn glyph(name: &str, material: bool) -> &'static str {
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

/// A Material 3 Expressive "star" shape (MaterialShapes.js: radius
/// 1 − a·((1 − cos nθ)/2)^b, turned and scaled), as an SVG outline in a
/// 0..100 box, as materialshape.frag draws it
pub fn shape(name: &str) -> String {
    let (n, a, b, rot, scale) = match name {
        "cookie12" => (12.0, 0.065, 1.0, -90.0, 1.0),
        "verySunny" => (8.0, 0.22, 0.6, 0.0, 1.0),
        "cookie4" => (4.0, 0.16, 1.0, 45.0, 1.0),
        _ => (1.0, 0.0, 1.0, 0.0, 1.0),
    };
    let rot = f64::to_radians(rot);
    let steps = 240;
    let mut d = String::new();
    for i in 0..=steps {
        let phi = i as f64 / steps as f64 * std::f64::consts::TAU;
        let th = phi - rot;
        let r = scale * (1.0 - a * (0.5 - 0.5 * (n * th).cos()).max(0.0).powf(b));
        let (x, y) = (50.0 + 50.0 * r * phi.cos(), 50.0 + 50.0 * r * phi.sin());
        d += &format!("{}{x:.2} {y:.2} ", if i == 0 { "M " } else { "L " });
    }
    d + "Z"
}

fn color(hex: &str) -> slint::Color {
    let h = hex.trim_start_matches('#');
    match (h.len(), u32::from_str_radix(h, 16)) {
        (6, Ok(v)) => slint::Color::from_rgb_u8((v >> 16) as u8, (v >> 8) as u8, v as u8),
        _ => slint::Color::from_argb_u8(0, 0, 0, 0),
    }
}

fn model<T: Clone + 'static>(v: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(v))
}

/// $XDG_CACHE_HOME/tessera (Paths.cache; iRice's while the settings are)
fn cache_dir() -> std::path::PathBuf {
    let base = std::env::var_os("XDG_CACHE_HOME").map(std::path::PathBuf::from).unwrap_or_else(|| std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".cache"));
    let name = crate::util::state_dir().file_name().map(|n| n.to_owned()).unwrap_or_else(|| "tessera".into());
    base.join(name)
}

impl Today {
    fn send(st: &State) -> impl Fn(Box<dyn Any + Send>) + Clone + 'static {
        st.shell.sender(NAME)
    }

    fn window(&mut self, st: &mut State, output: &str) -> bool {
        if self.win.as_ref().is_some_and(|w| w.output != output) {
            let w = self.win.take().unwrap();
            Glass::clear_now(&w.output, NS);
            st.drop_surface(&w.surface);
        }
        if self.win.is_some() {
            return true;
        }
        let Some(wl_output) = st.output_named(output) else { return false };
        let surface = st.prepare_surface(Spec {
            layer: Layer::Overlay,
            namespace: NS,
            output: Some((&wl_output, output)),
            anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
            size: (0, 0),
            exclusive: -1,
            keyboard: KeyboardInteractivity::None,
            margin_left: 0,
        });
        let ui = ui::TodayScreen::new().expect("TodayScreen");
        st.shell.apply_theme(&ui.global::<ui::Theme>());
        let to = Self::send(st);
        let d = ui.global::<ui::Day>();
        macro_rules! on {
            ($set:ident, |$($a:ident),*| $m:expr) => {{
                let to = to.clone();
                d.$set(move |$($a),*| to(Box::new($m)));
            }};
        }
        on!(on_close, | | M::Close);
        on!(on_pin, | | M::Pin);
        on!(on_options, | | M::Options);
        on!(on_options_done, | | M::OptionsDone);
        on!(on_toggle_todo, |k| M::Toggle(k.to_string()));
        on!(on_set_mode, |i| M::Mode(i));
        on!(on_set_entrance, |i| M::Entrance(i));
        on!(on_set_name, |t| M::Name(t.to_string()));
        on!(on_set_chime, |b| M::Chime(b));
        d.set_cookie_shape(shape("cookie12").into());
        ui.show().expect("show Today");
        let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
        self.win = Some(Win { output: output.into(), surface, ui, a11y, glass_sent: String::new() });
        true
    }

    /// The display it opens on: the one the shell named (the focused one),
    /// else Hyprland's focused one, else the first
    fn output_for(st: &State, asked: &str) -> Option<String> {
        let all = st.outputs_now();
        if all.iter().any(|(_, n)| n == asked) {
            return Some(asked.to_string());
        }
        let focused = st.shell.get::<Snapshot>().map(|s| s.focused.clone()).unwrap_or_default();
        all.iter().find(|(_, n)| *n == focused).or(all.first()).map(|(_, n)| n.clone())
    }

    fn open(&mut self, st: &mut State, v: &serde_json::Value) {
        let Some(output) = Self::output_for(st, v["screen"].as_str().unwrap_or("")) else { return };
        if !self.window(st, &output) {
            return;
        }
        let s = model::settings(&st.shell.cfg);
        self.fx = model::entrance(v["effect"].as_str().unwrap_or(""), &s.entrance).to_string();
        self.motion = v["motion"].as_f64().map_or(1.0, |m| m.clamp(0.1, 2.0) as f32);
        self.options = v["options"].as_bool().unwrap_or(false);
        self.ticked.clear();
        if self.account.is_empty() {
            self.account = model::account_name();
        }
        let (sw, sh) = {
            let w = self.win.as_ref().unwrap();
            let info = st.output_named(&output).and_then(|o| st.outputs.info(&o));
            let (lw, lh) = info.and_then(|i| i.logical_size).unwrap_or((1920, 1080));
            let _ = w;
            (lw.max(1) as f32, lh.max(1) as f32)
        };
        self.target = motion::target(sw, sh, if st.shell.look.material { 28.0 } else { 26.0 });
        let still = s.reduce_motion;
        let tl = motion::enter(&self.fx, still, self.motion, &self.target);
        // (from where it is when it's opened again while going)
        self.frame = tl.at(0.0);
        self.timeline = Some(tl);
        self.started = Instant::now();
        self.open = true;
        self.closing = false;
        {
            let ui = &self.win.as_ref().unwrap().ui;
            ui.set_tw(self.target.w);
            ui.set_th(self.target.h);
            ui.set_target_radius(self.target.r);
            ui.set_drop(self.fx == "drop" && !still);
            ui.set_on_glass(st.shell.plugin);
        }
        self.push_frame();
        self.refresh(st);
        let w = self.win.as_ref().unwrap();
        w.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        w.surface.show(0, 0);
        w.ui.invoke_focus_day();
        self.backdrop(st, &output);
        let to = Self::send(st);
        self.tick.start(slint::TimerMode::Repeated, Duration::from_millis(8), move || to(Box::new(M::Tick)));
        let to = Self::send(st);
        self.minute.start(slint::TimerMode::Repeated, Duration::from_secs(60), move || to(Box::new(M::Minute)));
    }

    /// Your wallpaper, blurred (the shell bakes it per display), read off the UI thread
    fn backdrop(&self, st: &State, output: &str) {
        let path = cache_dir().join(format!("backdrop-{output}.png"));
        let to = Self::send(st);
        let output = output.to_string();
        std::thread::spawn(move || {
            let px = image::open(&path).ok().map(|i| {
                let i = i.to_rgba8();
                slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(i.as_raw(), i.width(), i.height())
            });
            to(Box::new(M::Backdrop(output, px)));
        });
    }

    /// Put it away (`tell`: it closed itself, so the shell's Today follows)
    fn close(&mut self, st: &mut State, tell: bool) {
        if !self.open {
            return;
        }
        self.open = false;
        self.closing = true;
        self.options = false;
        let s = model::settings(&st.shell.cfg);
        let ms = if s.reduce_motion { 1.0 } else { self.motion };
        self.timeline = Some(motion::exit(self.frame, ms, &self.target));
        self.started = Instant::now();
        self.minute.stop();
        if let Some(w) = &self.win {
            w.ui.global::<ui::Day>().set_options_open(false);
            w.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::None);
            w.surface.layer.commit();
        }
        if !self.tick.running() {
            let to = Self::send(st);
            self.tick.start(slint::TimerMode::Repeated, Duration::from_millis(8), move || to(Box::new(M::Tick)));
        }
        if tell {
            crate::shell::ask("today.close", serde_json::json!([]));
        }
    }

    fn gone(&mut self, st: &mut State) {
        self.closing = false;
        self.tick.stop();
        if let Some(w) = &mut self.win {
            w.surface.hide();
            st.shell.glass.set(&w.output, NS, &[]);
            w.glass_sent.clear();
        }
    }

    fn push_frame(&self) {
        let Some(w) = &self.win else { return };
        let f = &self.frame;
        let ui = &w.ui;
        ui.set_px(f.x);
        ui.set_py(f.y);
        ui.set_pw(f.w);
        ui.set_ph(f.h);
        ui.set_pr(f.r);
        ui.set_panel_opacity(f.opacity);
        ui.set_frost(f.frost);
        ui.set_content_blur(f.blur);
        ui.set_ripple(f.ripple);
        ui.global::<ui::Day>().set_rows(f.rows);
    }

    fn tick(&mut self, st: &mut State) {
        let Some(tl) = &self.timeline else { return };
        let t = self.started.elapsed().as_secs_f32() * 1000.0;
        self.frame = tl.at(t);
        let over = t >= tl.length();
        self.push_frame();
        if over {
            self.tick.stop();
            if self.closing {
                self.gone(st);
            }
        }
    }

    /// What shows, from the services and settings
    fn refresh(&mut self, st: &State) {
        let Some(w) = &self.win else { return };
        let material = st.shell.look.material;
        let d = w.ui.global::<ui::Day>();
        let s = model::settings(&st.shell.cfg);
        let now = crate::util::now_ms();
        let (a, b) = calendar::day_bounds(now);
        let mine = st.shell.get::<Calendar>().map(|c| c.between(a, b)).unwrap_or_default();
        let events = model::merge(mine, &self.accounts.0, a, b);
        let mut all: Vec<Todo> = st.shell.get::<Todos>().map(|t| t.list.clone()).unwrap_or_default();
        all.extend(self.accounts.1.iter().cloned());
        for t in all.iter_mut() {
            if self.ticked.contains(&t.key) {
                t.done = !t.done;
            }
        }
        let open = model::open_todos(&all);
        let name = model::first_name(&s.name, &self.account);

        d.set_greeting(model::greeting(now, &name).into());
        d.set_date(model::long_date(now).into());
        let (mon, day) = model::cookie(now);
        d.set_cookie_month(mon.into());
        d.set_cookie_day(day.into());
        d.set_summary(model::summary(events.len(), open.len()).into());
        let rows: Vec<ui::TEvent> = model::event_rows(&events, now, s.clock24)
            .into_iter()
            .map(|r| ui::TEvent { time: r.time.into(), title: r.title.into(), color: color(&r.color), past: r.past })
            .collect();
        d.set_events_five(model(rows.iter().take(5).cloned().collect()));
        d.set_events(model(rows));
        d.set_todos(model(model::todo_rows(&open, now, s.clock24).into_iter().map(|r| ui::TTodo { key: r.key.into(), text: r.text.into(), due: r.due.into(), overdue: r.overdue }).collect()));
        d.set_todos_empty(model::todos_empty(!all.is_empty()).into());

        let wx = st.shell.get::<Weather>().cloned().unwrap_or_default();
        d.set_weather_ready(wx.ready);
        d.set_weather_icon(glyph(if wx.ready { wx.icon } else { "cloud" }, material).into());
        d.set_weather_temp(format!("{}°", wx.temp).into());
        d.set_weather_condition(wx.condition.as_str().into());
        d.set_weather_hilo(format!("High {}° · Low {}°", wx.high, wx.low).into());
        d.set_weather_rain(if wx.rain_chance > 0 { format!("{}% rain", wx.rain_chance) } else { String::new() }.into());
        d.set_weather_line(format!("H {}° · L {}°{}", wx.high, wx.low, if wx.rain_chance > 0 { format!(" · {}% rain", wx.rain_chance) } else { String::new() }).into());
        d.set_weather_problem(model::weather_problem(&wx.error).into());
        d.set_hours(model(
            wx.hourly
                .iter()
                .map(|h| ui::THour { time: model::hour(h.hour, s.clock24).into(), icon: glyph(h.icon, material).into(), rain: if h.rain >= 20 { format!("{}%", h.rain) } else { String::new() }.into(), temp: format!("{}°", h.temp).into() })
                .collect(),
        ));
        d.set_sun_shape(shape(if wx.ready { "verySunny" } else { "cookie4" }).into());
        d.set_rain_color(color(&theme::accent_hex("blue", st.shell.look.dark)));
        d.set_pin_shown(!s.widget_always());

        d.set_options_open(self.options);
        d.set_mode(model::MODES.iter().position(|m| *m == s.mode).unwrap_or(0) as i32);
        d.set_entrance(model::ENTRANCES.iter().position(|e| *e == s.entrance).unwrap_or(0) as i32);
        d.set_mode_hint(model::mode_hint(&s.mode).into());
        d.set_entrance_hint(model::entrance_hint(&s.entrance).into());
        d.set_name(s.name.as_str().into());
        let (placeholder, hint) = model::name_words(&self.account);
        d.set_name_placeholder(placeholder.into());
        d.set_name_hint(hint.into());
        d.set_chime(s.chime);
    }

    /// One of its options: to the shell (its settings), shown at once here
    fn set(&mut self, st: &mut State, key: &str, value: serde_json::Value) {
        crate::shell::ask("today.set", serde_json::json!([key, value]));
        // (config.json follows in a moment; until then this is what it says)
        let cfg = &mut st.shell.cfg;
        if !cfg.is_object() {
            *cfg = serde_json::json!({});
        }
        let (group, k) = if key == "chime" { ("sounds", "today") } else { ("today", key) };
        if !cfg[group].is_object() {
            cfg[group] = serde_json::json!({});
        }
        cfg[group][k] = value;
        self.refresh(st);
    }
}

impl Part for Today {
    fn name(&self) -> &'static str {
        NAME
    }

    fn start(&mut self, st: &mut State) {
        st.shell.want("hypr");
        st.shell.want("calendar");
        st.shell.want("todos");
        st.shell.want("weather");
        self.weather_settings = weather::settings(&st.shell.cfg);
    }

    fn config_changed(&mut self, st: &mut State, look: bool) {
        let ws = weather::settings(&st.shell.cfg);
        if ws != self.weather_settings {
            self.weather_settings = ws;
            weather::refresh();
        }
        if let Some(w) = &mut self.win {
            if look {
                st.shell.apply_theme(&w.ui.global::<ui::Theme>());
                w.glass_sent.clear();
            }
        }
        self.refresh(st);
    }

    fn plugin_changed(&mut self, st: &mut State) {
        if let Some(w) = &mut self.win {
            st.shell.apply_theme(&w.ui.global::<ui::Theme>());
            w.ui.set_on_glass(st.shell.plugin);
            w.glass_sent.clear();
        }
    }

    fn data(&mut self, st: &mut State, what: TypeId) {
        if what == TypeId::of::<Todos>() {
            // (the shell's state has the ticks now, or will on its next write)
            let list = st.shell.get::<Todos>().map(|t| t.list.clone()).unwrap_or_default();
            self.ticked.retain(|k| list.iter().any(|t| &t.key == k && !t.done));
        }
        if [TypeId::of::<Calendar>(), TypeId::of::<Todos>(), TypeId::of::<Weather>()].contains(&what) {
            self.refresh(st);
        }
    }

    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        // From the shell (stdin): open, close, its options, the accounts' items
        let msg = match msg.downcast::<serde_json::Value>() {
            Ok(v) => {
                if v.get("accounts").is_some() {
                    self.accounts = model::accounts(&v["accounts"]);
                }
                match v["open"].as_bool() {
                    Some(true) => self.open(st, &v),
                    Some(false) => self.close(st, false),
                    None => {
                        if v["options"].as_bool() == Some(true) && self.open {
                            self.options = true;
                            if let Some(w) = &self.win {
                                w.ui.invoke_begin_options();
                            }
                        }
                        self.refresh(st);
                    }
                }
                return;
            }
            Err(m) => m,
        };
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Tick => self.tick(st),
            M::Minute => self.refresh(st),
            M::Close => self.close(st, true),
            M::Pin => {
                crate::shell::ask("today.pin", serde_json::json!([]));
                self.close(st, false);
            }
            M::Options => {
                self.options = true;
                if let Some(w) = &self.win {
                    w.ui.global::<ui::Day>().set_options_open(true);
                    w.ui.invoke_begin_options();
                }
            }
            M::OptionsDone => {
                self.options = false;
                if let Some(w) = &self.win {
                    w.ui.global::<ui::Day>().set_options_open(false);
                    w.ui.invoke_focus_day();
                }
            }
            M::Toggle(key) => {
                if !key.is_empty() {
                    crate::shell::ask("todos.toggle", serde_json::json!([key]));
                    if !self.ticked.remove(&key) {
                        self.ticked.insert(key);
                    }
                    self.refresh(st);
                }
            }
            M::Mode(i) => {
                if let Some(m) = model::MODES.get(i.max(0) as usize) {
                    self.set(st, "mode", serde_json::json!(m));
                }
            }
            M::Entrance(i) => {
                if let Some(e) = model::ENTRANCES.get(i.max(0) as usize) {
                    self.set(st, "entrance", serde_json::json!(e));
                }
            }
            M::Name(t) => {
                let t = t.trim().to_string();
                if t != model::settings(&st.shell.cfg).name {
                    self.set(st, "name", serde_json::json!(t));
                }
            }
            M::Chime(on) => self.set(st, "chime", serde_json::json!(on)),
            M::Backdrop(output, px) => {
                if let Some(w) = self.win.as_ref().filter(|w| w.output == output) {
                    w.ui.set_backdrop(px.map(slint::Image::from_rgba8).unwrap_or_default());
                }
            }
        }
    }

    fn output_removed(&mut self, st: &mut State, name: &str) {
        if self.win.as_ref().is_some_and(|w| w.output == name) {
            let was = self.open;
            self.open = false;
            self.closing = false;
            self.tick.stop();
            self.minute.stop();
            let w = self.win.take().unwrap();
            st.shell.glass.set(&w.output, NS, &[]);
            st.drop_surface(&w.surface);
            if was {
                crate::shell::ask("today.close", serde_json::json!([]));
            }
        }
    }

    fn surface_closed(&mut self, st: &mut State, s: &Rc<Surface>) {
        if self.win.as_ref().is_some_and(|w| Rc::ptr_eq(&w.surface, s)) {
            let name = s.output.clone();
            self.output_removed(st, &name);
        }
    }

    /// The panel's glass for the plugin (Glass), as it moves; accessibility
    fn after_frame(&mut self, st: &mut State) {
        let kbd = st.keyboard_surface();
        let Some(w) = &mut self.win else { return };
        let f = kbd.as_ref().is_some_and(|k| Rc::ptr_eq(k, &w.surface));
        if let Some(a) = w.a11y.as_mut() {
            a.update(false, f);
        }
        if !w.surface.mapped.get() || !st.shell.plugin || st.shell.look.material {
            return;
        }
        let fr = &self.frame;
        let on = fr.opacity > 0.002;
        let shape = theme::glass_shape(&st.shell.look, fr.x, fr.y, fr.w, fr.h, fr.r, fr.opacity, 1.0);
        let key = if on { format!("{shape:?}") } else { String::new() };
        if key != w.glass_sent {
            st.shell.glass.set(&w.output, NS, if on { std::slice::from_ref(&shape) } else { &[] });
            w.glass_sent = key;
        }
    }

    fn a11y(&mut self) -> Vec<&mut crate::a11y::A11y> {
        self.win.as_mut().and_then(|w| w.a11y.as_mut()).into_iter().collect()
    }

    fn stats(&self) {
        if let Some(w) = &self.win {
            eprintln!("tessera-ui: stats {}", w.surface.stats.borrow_mut().report("today"));
        }
    }

    fn quit(&mut self, _: &mut State) {
        if let Some(w) = &self.win {
            Glass::clear_now(&w.output, NS);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shapes_stay_in_their_box() {
        for name in ["cookie12", "verySunny", "cookie4", "circle"] {
            let d = shape(name);
            let nums: Vec<f64> = d.split(|c: char| c == ' ' || c.is_ascii_alphabetic()).filter_map(|s| s.parse().ok()).collect();
            assert!(nums.len() > 400 && nums.iter().all(|v| (0.0..=100.0).contains(v)), "{name}");
        }
        // a cookie's scallops cut in, a circle's edge doesn't
        let min_r = |d: &str| {
            let n: Vec<f64> = d.split(|c: char| c == ' ' || c.is_ascii_alphabetic()).filter_map(|s| s.parse().ok()).collect();
            n.chunks(2).map(|p| ((p[0] - 50.0).powi(2) + (p[1] - 50.0).powi(2)).sqrt()).fold(f64::MAX, f64::min)
        };
        assert!(min_r(&shape("circle")) > 49.9);
        assert!((min_r(&shape("cookie12")) - 50.0 * (1.0 - 0.065)).abs() < 0.2);
        assert!((min_r(&shape("verySunny")) - 50.0 * (1.0 - 0.22)).abs() < 0.3);
    }

    #[test]
    fn weather_glyphs_for_each_look() {
        assert_eq!(glyph("sun", false), "\u{e472}");
        assert_eq!(glyph("sun", true), "\u{e518}");
        assert_eq!(glyph("anything", false), glyph("cloud", false));
    }

    #[test]
    fn colours_from_the_calendars() {
        assert_eq!(color("#6D7CDF"), slint::Color::from_rgb_u8(0x6d, 0x7c, 0xdf));
        assert_eq!(color("").alpha(), 0); // (the accent then)
    }
}

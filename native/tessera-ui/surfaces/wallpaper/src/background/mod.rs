//! The wallpaper (rust.surfaces: background): one window per screen on the
//! background layer, the picture cropped to cover it (PreserveAspectCrop) and
//! decoded off the UI thread at the screen's pixels, a new picture fading in
//! over the old one (700 ms), as shell/modules/background/Background.qml.
//! Which picture: shell/common/Wallpaper.qml's choice (wallpaper.rs): the
//! chosen one, the look's bundled default, the time of day's (minute by
//! minute), the light or dark one of a pair, or a live wallpaper's poster.
//!
//! What the QML window keeps doing while this draws (it moves up to the
//! bottom layer, clear, and stops loading pictures): the desktop's widgets
//! and icons, the blurred copy of the wallpaper that glass refracts, the
//! menu bar's light-or-dark sampling, and a live wallpaper's video (Slint has
//! no video element: this draws the video's poster frame, the same one the
//! QML makes with ffmpeg, and the QML plays the video over it).
//!
//! What the shell tells it (RustUi.tell, `{"to": "background", …}` lines):
//! the day's sunrise and sunset (`sunrise`, `sunset`: minutes; the weather
//! knows them) and the lock's settle (`settle`: LockFx's steps, "prepare",
//! "reveal" over `ms`, "rest"; `zoom` false: fade only), followed with
//! LockFx's own curve (`Settle`) so the lock's veil lines up with it.

/// (in tessera-core: the picker and the look read them too)
pub use tessera_core::{picture, wallpaper};

use crate::shell::Part;
use crate::ui;
use crate::wayland::{Spec, State, Surface};
use picture::{Decoded, Pool};
use slint::ComponentHandle;
use smithay_client_toolkit::reexports::client::protocol::wl_output::{Transform, WlOutput};
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use std::any::Any;
use std::rc::Rc;
use wallpaper::{Bundled, Context, Day};

const NAME: &str = "background";
/// (not the QML's tessera:wallpaper: that window stays, with the desktop's
/// widgets and their glass, which the glass plugin finds by namespace)
pub const NS: &str = "tessera:background";

/// Its own messages (Msg::Part("background", …))
enum M {
    Decoded(Decoded),
    Poster(String, bool),
    Faded(String),
    Tick,
    SettleEnd,
}

struct Win {
    surface: Rc<Surface>,
    ui: ui::Background,
    /// where each picture starts fading in, and where the fade is headed
    a_from: f32,
    b_from: f32,
    top: f32,
    /// drawn once more after its first frame (see `second_frame`)
    settled: bool,
}

/// LockFx's settle, step by step as the shell tells them: `desktop` runs 0
/// (set back) .. 1 (at rest), linearly over a reveal's ms; the wallpaper's
/// scale eases its share (LockFx.wallpaperScale). The same numbers as the
/// QML, so the lock's veil (its own copy of the wallpaper, fading out over
/// this one at LockFx's scale) lines up with it on every frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settle {
    pub zoom: bool,
    /// where `desktop` was when the reveal started (1 when at rest)
    pub from: f64,
    /// the reveal: when it started and how long it takes
    pub reveal: Option<(std::time::Instant, i64)>,
}

impl Default for Settle {
    fn default() -> Self {
        Self { zoom: true, from: 1.0, reveal: None }
    }
}

impl Settle {
    /// LockFx.desktop at `now`
    pub fn desktop(&self, now: std::time::Instant) -> f64 {
        match self.reveal {
            Some((start, ms)) => desktop_at(self.from, now.saturating_duration_since(start).as_secs_f64() * 1000.0, ms),
            None => self.from,
        }
    }

    /// One of LockFx's steps (prepare / reveal / rest); false for anything else
    pub fn step(&mut self, step: &str, zoom: bool, ms: i64, now: std::time::Instant) -> bool {
        match step {
            // set back at once
            "prepare" => *self = Self { zoom, from: 0.0, reveal: None },
            // from wherever it is, to rest over `ms`
            "reveal" => {
                let d = self.desktop(now);
                *self = Self { zoom: self.zoom, from: d, reveal: Some((now, ms.clamp(0, 10_000))) };
            }
            "rest" => *self = Self { zoom: self.zoom, from: 1.0, reveal: None },
            _ => return false,
        }
        true
    }
}

/// How long ago the shell took a step it stamped `at` (its Date.now(), ms):
/// 0 without a stamp, and never more than half a second (a clock that moved)
pub fn lag_ms(now_ms: i64, at: Option<f64>) -> f64 {
    at.map_or(0.0, |at| (now_ms as f64 - at).clamp(0.0, 500.0))
}

/// LockFx's NumberAnimation on `desktop` (linear): from `from` to 1 over `ms`
pub fn desktop_at(from: f64, elapsed_ms: f64, ms: i64) -> f64 {
    if ms <= 0 {
        return 1.0;
    }
    (from + (1.0 - from) * (elapsed_ms / ms as f64)).clamp(0.0, 1.0)
}

/// LockFx.ease: ease-out cubic (background.slint draws the scale with it;
/// here for the tests that hold it to LockFx's)
#[cfg_attr(not(test), allow(dead_code))]
pub fn ease(t: f64) -> f64 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

/// LockFx.wallpaperScale (as background.slint's `scale`)
#[cfg_attr(not(test), allow(dead_code))]
pub fn wallpaper_scale(zoom: bool, desktop: f64) -> f64 {
    if zoom {
        1.0 + 0.05 * (1.0 - ease(desktop))
    } else {
        1.0
    }
}

struct Screen {
    output: String,
    /// its pixels (the output's mode, then the window's own size)
    px: (u32, u32),
    /// the picture and size last asked for, and whether it's still on its way
    asked: (String, (u32, u32)),
    pending: bool,
    win: Option<Win>,
}

#[derive(Default)]
pub struct Background {
    screens: Vec<Screen>,
    pool: Option<Pool>,
    /// the time-of-day folder and its pictures
    folder: (String, Vec<String>),
    /// the live wallpaper's video and its poster, once made
    poster: (String, Option<String>),
    bundled: Option<Bundled>,
    clock: slint::Timer,
    /// the first picture is up (a screen that comes later fades in quickly
    /// from black, as the QML's: its connect effect already covers it)
    started: bool,
    /// the lock's settle (a window made meanwhile joins in)
    settle: Settle,
    /// ends a reveal: the windows stop redrawing every frame for it
    settle_end: slint::Timer,
}

/// An output's pixels: its current mode, turned as the output is
fn output_pixels(st: &State, output: &WlOutput) -> (u32, u32) {
    let Some(info) = st.outputs.info(output) else { return (1920, 1080) };
    let (w, h) = info.modes.iter().find(|m| m.current).or(info.modes.first()).map_or((1920, 1080), |m| m.dimensions);
    let (w, h) = (w.max(1) as u32, h.max(1) as u32);
    match info.transform {
        Transform::_90 | Transform::_270 | Transform::Flipped90 | Transform::Flipped270 => (h, w),
        _ => (w, h),
    }
}

impl Background {
    fn day(&self) -> Day {
        let (r, s) = wallpaper::sun();
        Day::now(r, s)
    }

    /// The picture to show now (Wallpaper.qml's `path`)
    fn wanted(&mut self, st: &State) -> String {
        let cfg = &st.shell.cfg;
        let material = st.shell.look.material;
        let tf = wallpaper::time_folder(cfg);
        if tf != self.folder.0 {
            self.folder = (tf.to_string(), if tf.is_empty() { Vec::new() } else { wallpaper::folder_pictures(tf) });
        }
        let live = wallpaper::live(cfg);
        if live != self.poster.0 {
            self.poster = (live.to_string(), None);
            if !live.is_empty() {
                let (video, tx) = (live.to_string(), st.tx.clone());
                let out = wallpaper::poster_path(&wallpaper::cache_dir(), &video);
                std::thread::spawn(move || {
                    let ok = wallpaper::make_poster(&video, &out);
                    let _ = tx.send(crate::Msg::Part(NAME, Box::new(M::Poster(video, ok))));
                });
            }
        }
        let day = self.day();
        let bundled = self.bundled.get_or_insert_with(Bundled::find);
        let file = |n: &str| bundled.file(material, n);
        let c = Context { material, day, bundled: &file, folder: &self.folder.1, poster: self.poster.1.as_deref() };
        wallpaper::picture(cfg, &c)
    }

    /// Ask for the picture on every screen that doesn't have it at its size
    fn refresh(&mut self, st: &mut State) {
        let want = self.wanted(st);
        if crate::env::is_set("DEBUG") {
            eprintln!("debug: wallpaper wanted {want:?} on {:?}", self.screens.iter().map(|s| (&s.output, s.px)).collect::<Vec<_>>());
        }
        if want.is_empty() {
            return;
        }
        for sc in &mut self.screens {
            if sc.asked != (want.clone(), sc.px) {
                sc.asked = (want.clone(), sc.px);
                sc.pending = true;
            }
        }
        // (one decode per picture and size, the newest wishes only)
        let mut jobs: Vec<(String, (u32, u32))> = Vec::new();
        for sc in self.screens.iter().filter(|s| s.pending) {
            if !jobs.contains(&sc.asked) {
                jobs.push(sc.asked.clone());
            }
        }
        if jobs.is_empty() {
            return;
        }
        let pool = self.pool.get_or_insert_with(|| Pool::new(1, "tessera-wallpaper"));
        pool.replace(
            jobs.into_iter()
                .map(|(path, (w, h))| {
                    let tx = st.tx.clone();
                    Box::new(move || {
                        let pixels = picture::decode(&path, w, h);
                        if pixels.is_none() {
                            eprintln!("tessera-ui: wallpaper {path} can't be read");
                        }
                        let _ = tx.send(crate::Msg::Part(NAME, Box::new(M::Decoded(Decoded { path, size: (w, h), pixels }))));
                    }) as Box<dyn FnOnce() + Send>
                })
                .collect(),
        );
    }

    fn decoded(&mut self, st: &mut State, d: Decoded) {
        if crate::env::is_set("DEBUG") {
            eprintln!("debug: wallpaper {} for {:?}: {:?}", d.path, d.size, d.pixels.as_ref().map(|p| (p.width(), p.height())));
        }
        let image = d.pixels.map(slint::Image::from_rgb8);
        for i in 0..self.screens.len() {
            let sc = &self.screens[i];
            if !sc.pending || sc.asked != (d.path.clone(), d.size) {
                continue;
            }
            self.screens[i].pending = false;
            match &image {
                Some(img) => self.show(st, i, img.clone()),
                // (unreadable: the picture before stays, as the QML's; with none
                // before, black rather than the QML's window showing through)
                None if self.screens[i].win.is_none() => self.show(st, i, slint::Image::default()),
                None => {}
            }
        }
    }

    /// A picture on a screen: the window made with it, or faded in over the one there
    fn show(&mut self, st: &mut State, i: usize, img: slint::Image) {
        let sc = &mut self.screens[i];
        let Some(w) = sc.win.as_mut() else {
            let Some(output) = st.output_named(&sc.output) else { return };
            let surface = st.prepare_surface(Spec {
                layer: Layer::Background,
                namespace: NS,
                output: Some((&output, &sc.output)),
                anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
                size: (0, 0),
                exclusive: -1,
                keyboard: KeyboardInteractivity::None,
                margin_left: 0,
            });
            let ui = ui::Background::new().expect("Background");
            // The first picture is there at once: the QML's own (the same
            // picture) is still under it when this window appears. A screen
            // that comes later fades in from black, quickly (Background.qml's
            // firstFade once Wallpaper.started: 120 ms)
            let later = self.started;
            ui.set_a(img);
            ui.set_a_from(if later { 0.0 } else { -1.0 });
            ui.set_b_from(-2.0);
            ui.set_quick(later);
            apply_settle(&ui, &self.settle);
            ui.set_target(if later { 1.0 } else { 0.0 });
            let (to, out) = (st.shell.sender(NAME), sc.output.clone());
            ui.on_faded(move || to(Box::new(M::Faded(out.clone()))));
            ui.show().expect("show background");
            let (a_from, top) = if later { (0.0, 1.0) } else { (-1.0, 0.0) };
            sc.win = Some(Win { surface, ui, a_from, b_from: -2.0, top, settled: false });
            self.started = true;
            return;
        };
        w.ui.set_quick(false);
        // Into the older slot, which then starts fading in from where the fade is
        if w.a_from <= w.b_from {
            w.ui.set_a(img);
            w.a_from = w.top;
            w.ui.set_a_from(w.a_from);
        } else {
            w.ui.set_b(img);
            w.b_from = w.top;
            w.ui.set_b_from(w.b_from);
        }
        w.top += 1.0;
        // (from now: an idle window's animation clock is from its last frame)
        slint::platform::update_timers_and_animations();
        w.ui.set_target(w.top);
    }

    /// The new picture is fully in: let the one below go
    fn faded(&mut self, output: &str) {
        let Some(w) = self.screens.iter_mut().find(|s| s.output == output).and_then(|s| s.win.as_mut()) else { return };
        if w.a_from < w.b_from {
            w.ui.set_a(slint::Image::default());
        } else if w.b_from < w.a_from {
            w.ui.set_b(slint::Image::default());
        }
    }

    /// A settle step: every window follows from now, and a reveal's end is
    /// timed (its last frame is exactly at rest)
    fn settled(&mut self, st: &State) {
        // (Slint's animation clock moves only when it's asked to: an idle
        // window's is from its last frame, and a reveal timed from that
        // would be over before it began. This is what put the wallpaper
        // and the lock's veil at different scales mid-settle)
        slint::platform::update_timers_and_animations();
        for w in self.screens.iter().filter_map(|s| s.win.as_ref()) {
            apply_settle(&w.ui, &self.settle);
        }
        match self.settle.reveal {
            Some((_, ms)) => {
                let tx = st.tx.clone();
                self.settle_end.start(slint::TimerMode::SingleShot, std::time::Duration::from_millis(ms as u64 + 20), move || {
                    let _ = tx.send(crate::Msg::Part(NAME, Box::new(M::SettleEnd)));
                });
            }
            None => self.settle_end.stop(),
        }
    }

    /// Again on the next minute (the time of day's picture, Auto's light and dark)
    fn tick(&mut self, st: &mut State) {
        self.refresh(st);
        let secs = crate::util::now("%S").parse::<u64>().unwrap_or(0);
        let tx = st.tx.clone();
        self.clock.start(slint::TimerMode::SingleShot, std::time::Duration::from_millis((60 - secs.min(59)) * 1000 + 50), move || {
            let _ = tx.send(crate::Msg::Part(NAME, Box::new(M::Tick)));
        });
    }
}

/// A window's share of the settle: where `desktop` is, and a reveal running
/// from now for what's left of it (Slint draws its frames: background.slint)
fn apply_settle(ui: &ui::Background, s: &Settle) {
    let now = std::time::Instant::now();
    ui.set_zoom_on(s.zoom);
    match s.reveal {
        Some((start, ms)) => {
            let left = (ms as f64 - now.saturating_duration_since(start).as_secs_f64() * 1000.0).max(0.0);
            ui.set_desk_from(s.desktop(now) as f32);
            ui.set_desk_ms(left.round() as i32);
            ui.invoke_settle_start();
            ui.set_settling(left > 0.0);
        }
        None => {
            ui.set_settling(false);
            ui.set_desk_from(s.from as f32);
        }
    }
}

pub use tessera_core::picture::second_frame;

impl Part for Background {
    fn name(&self) -> &'static str {
        NAME
    }

    fn start(&mut self, st: &mut State) {
        self.tick(st);
    }

    fn output_added(&mut self, st: &mut State, output: &WlOutput, name: &str) {
        if self.screens.iter().any(|s| s.output == name) {
            return;
        }
        let px = output_pixels(st, output);
        self.screens.push(Screen { output: name.to_string(), px, asked: (String::new(), (0, 0)), pending: false, win: None });
        self.refresh(st);
    }

    fn output_removed(&mut self, st: &mut State, name: &str) {
        if let Some(i) = self.screens.iter().position(|s| s.output == name) {
            let sc = self.screens.remove(i);
            if let Some(w) = sc.win {
                st.drop_surface(&w.surface);
            }
        }
    }

    fn config_changed(&mut self, st: &mut State, _look: bool) {
        self.refresh(st);
    }

    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        let msg = match msg.downcast::<serde_json::Value>() {
            // From the shell (RustUi.tell): its daylight, the lock's settle
            Ok(v) => {
                if wallpaper::told_sun(&v) {
                    self.refresh(st);
                }
                // (the step's own moment: the shell says when it took it, so
                // the time its line took to get here isn't lost to the curve)
                let now = std::time::Instant::now() - std::time::Duration::from_secs_f64(lag_ms(crate::util::now_ms(), v["at"].as_f64()) / 1000.0);
                if let Some(step) = v["settle"].as_str() {
                    if crate::env::is_set("DEBUG") {
                        eprintln!("debug: settle {step} {:?}, {:.0} ms after the shell", v["ms"].as_i64(), lag_ms(crate::util::now_ms(), v["at"].as_f64()));
                    }
                    if self.settle.step(step, v["zoom"].as_bool().unwrap_or(true), v["ms"].as_i64().unwrap_or(0), now) {
                        self.settled(st);
                    }
                }
                return;
            }
            Err(m) => m,
        };
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Decoded(d) => self.decoded(st, d),
            M::Poster(video, ok) => {
                if video == self.poster.0 {
                    self.poster.1 = ok.then(|| wallpaper::poster_path(&wallpaper::cache_dir(), &video).to_string_lossy().into_owned());
                    self.refresh(st);
                }
            }
            M::Faded(output) => self.faded(&output),
            M::Tick => self.tick(st),
            M::SettleEnd => {
                if self.settle.reveal.is_some_and(|(start, ms)| start.elapsed().as_millis() as i64 >= ms) {
                    self.settle.reveal = None;
                    self.settle.from = 1.0;
                    for w in self.screens.iter().filter_map(|s| s.win.as_ref()) {
                        apply_settle(&w.ui, &self.settle);
                    }
                }
            }
        }
    }

    fn surface_closed(&mut self, st: &mut State, s: &Rc<Surface>) {
        let name = s.output.clone();
        self.output_removed(st, &name);
    }

    /// The window's own pixels, once the compositor sized it (a fractional
    /// scale, a mode change): the picture again at that size if it differs
    fn after_frame(&mut self, st: &mut State) {
        let mut changed = false;
        for sc in &mut self.screens {
            let Some(w) = &mut sc.win else { continue };
            second_frame(&w.surface, &mut w.settled);
            let size = w.surface.window().size();
            if size.width > 0 && size.height > 0 && (size.width, size.height) != sc.px {
                sc.px = (size.width, size.height);
                changed = true;
            }
        }
        if changed {
            self.refresh(st);
        }
    }

    fn stats(&self) {
        for sc in &self.screens {
            if let Some(w) = &sc.win {
                eprintln!("tessera-ui: stats {}", w.surface.stats.borrow_mut().report(&format!("background:{}", sc.output)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn the_settle_is_lockfx_frame_for_frame() {
        // LockFx: wallpaperScale = 1 + 0.05 (1 - ease(desktop)), ease-out cubic
        assert_eq!(wallpaper_scale(true, 0.0), 1.05);
        assert_eq!(wallpaper_scale(true, 1.0), 1.0);
        assert!((wallpaper_scale(true, 0.5) - (1.0 + 0.05 * 0.125)).abs() < 1e-12);
        assert_eq!(wallpaper_scale(false, 0.0), 1.0);
        // desktop runs linearly over the reveal
        assert_eq!(desktop_at(0.0, 0.0, 560), 0.0);
        assert!((desktop_at(0.0, 140.0, 560) - 0.25).abs() < 1e-12);
        assert!((desktop_at(0.5, 280.0, 560) - 0.75).abs() < 1e-12);
        assert_eq!(desktop_at(0.0, 600.0, 560), 1.0);
        assert_eq!(desktop_at(0.0, 0.0, 0), 1.0);
    }

    #[test]
    fn settle_steps_as_lockfx() {
        let t0 = Instant::now();
        let mut s = Settle::default();
        assert_eq!(s.desktop(t0), 1.0);
        // unlock: set back, then revealed over 560 ms
        assert!(s.step("prepare", true, 0, t0));
        assert_eq!((s.desktop(t0), wallpaper_scale(s.zoom, s.desktop(t0))), (0.0, 1.05));
        let t1 = t0 + Duration::from_millis(100);
        assert!(s.step("reveal", true, 560, t1));
        let at = |ms: u64| wallpaper_scale(s.zoom, s.desktop(t1 + Duration::from_millis(ms)));
        assert_eq!(at(0), 1.05);
        assert!((at(280) - (1.0 + 0.05 * 0.125)).abs() < 1e-9);
        assert_eq!(at(560), 1.0);
        // a reveal mid-reveal goes on from where it is
        let mid = s.desktop(t1 + Duration::from_millis(280));
        assert!(s.step("reveal", true, 300, t1 + Duration::from_millis(280)));
        assert!((s.from - mid).abs() < 1e-9);
        // rest: at once; fade only: never zoomed
        assert!(s.step("rest", true, 0, t1));
        assert_eq!(s.desktop(t1), 1.0);
        assert!(s.step("prepare", false, 0, t1));
        assert_eq!(wallpaper_scale(s.zoom, s.desktop(t1)), 1.0);
        assert!(!s.step("wobble", true, 0, t1));
    }

    #[test]
    fn a_step_counts_from_when_the_shell_took_it() {
        assert_eq!(lag_ms(10_000, Some(9_960.0)), 40.0);
        assert_eq!(lag_ms(10_000, None), 0.0);
        // (clocks: never ahead, never more than half a second)
        assert_eq!(lag_ms(10_000, Some(10_050.0)), 0.0);
        assert_eq!(lag_ms(10_000, Some(1_000.0)), 500.0);
    }
}

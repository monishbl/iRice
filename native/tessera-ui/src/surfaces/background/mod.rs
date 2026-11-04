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
//! QML makes with ffmpeg, and the QML plays the video over it). Not here
//! yet: the lock screen's settle (LockFx: the wallpaper eases in from a touch
//! larger on lock and unlock).

pub mod picture;
pub mod wallpaper;

use crate::shell::{FromShell, Part};
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
    /// today's sunrise and sunset, minutes (the QML shell's weather knows them)
    sun: Option<(i32, i32)>,
    /// the time-of-day folder and its pictures
    folder: (String, Vec<String>),
    /// the live wallpaper's video and its poster, once made
    poster: (String, Option<String>),
    bundled: Option<Bundled>,
    clock: slint::Timer,
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
        let (r, s) = self.sun.unwrap_or((wallpaper::SUNRISE, wallpaper::SUNSET));
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
            // picture) is still under it when this window appears
            ui.set_a(img);
            ui.set_a_from(-1.0);
            ui.set_b_from(-2.0);
            ui.set_target(0.0);
            let (to, out) = (st.shell.sender(NAME), sc.output.clone());
            ui.on_faded(move || to(Box::new(M::Faded(out.clone()))));
            ui.show().expect("show background");
            sc.win = Some(Win { surface, ui, a_from: -1.0, b_from: -2.0, top: 0.0, settled: false });
            return;
        };
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

/// A window's first frame is drawn before its GL buffer has the window's
/// size (the EGL window starts at 1×1 and is resized as that frame is drawn):
/// the compositor stretches a 1×1 frame over the screen. Something that moves
/// draws again at once; a still picture wouldn't, so it's drawn once more.
/// (A platform matter, egl.rs/wayland.rs; this keeps still surfaces right)
pub fn second_frame(surface: &Surface, settled: &mut bool) {
    if !*settled && surface.mapped.get() {
        *settled = true;
        surface.window().request_redraw();
    }
}

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
        let msg = match msg.downcast::<FromShell>() {
            // The shell's daylight (Daylight.qml, from the weather)
            Ok(m) => {
                let (r, s) = (m.0["sunrise"].as_i64(), m.0["sunset"].as_i64());
                if let (Some(r), Some(s)) = (r, s) {
                    self.sun = Some((r as i32, s as i32));
                    self.refresh(st);
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

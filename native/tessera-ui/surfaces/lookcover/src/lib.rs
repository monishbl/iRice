//! The look-switch cover, in Rust: `tessera-ui --look-cover [-p] <dir>`, in
//! place of `qs -p <dir>` with shell/assets/lookswitch/shell.qml. Shown by
//! switch-look.sh while the shell restarts into the other look (Glass or
//! Material): your blurred wallpaper on every display, the Tessera mark on
//! Material's shapes, morphing, and the look's name. Not a shell surface: its
//! own process and loop (`run`, like the greeter's), so it stays up while the
//! shell restarts; src/main.rs hands over to it before anything else.
//!
//! <dir> is the switch's copy of the screen (`$T/ui`); like the QML, it reads
//!   <dir>/../look.json  {"to": "glass"|"material", "dark": bool,
//!                        "backdrops": {output name: blurred wallpaper (PNG)}}
//!   <dir>/../status     "run", then "done" (fade out and quit)
//!   <dir>/Phosphor-Fill.ttf, <dir>/GoogleSansFlex.ttf  (else the repository's)
//! The `-p` is accepted (and passed by switch-look.sh) so the process reads
//! like the QML one's (`… -p $T/ui`): the script's wait for it to go
//! (`pgrep -f -- "-p $T/ui"`) finds either. The path holds `/tessera-look.`,
//! which sandbox/lib.sh's stop_sandbox_shells skips. `--probe` only says
//! it's there (exit 0, nothing drawn).
//!
//! As the QML: Overlay, namespace "tesseralookswitch" (not "tessera:": the
//! compositor doesn't blur it), the keyboard held (keys don't reach apps
//! meanwhile), 320 ms in, 450 ms out; a new shape every 420 ms, sprung
//! (SpringAnimation spring 3.2, damping 0.27), the whole turning once in
//! 3.6 s; gone by itself after 45 s.

pub mod ui {
    slint::include_modules!();
}

use slint::ComponentHandle;
use smithay_client_toolkit::reexports::client::protocol::wl_output::WlOutput;
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use std::any::Any;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};
use tessera_core::shell::Part;
use tessera_core::wayland::{Spec, State, Surface};
use tessera_core::Msg;

const NAME: &str = "lookcover";
const NS: &str = "tesseralookswitch";
/// The mark's box (logical px)
const MARK: f32 = 168.0;

/// Asked for: `--look-cover`
pub fn wanted(args: &[String]) -> bool {
    args.iter().skip(1).any(|a| a == "--look-cover")
}

#[derive(Debug, PartialEq)]
pub enum Ask {
    Probe,
    Show(PathBuf),
    Bad,
}

/// `--look-cover [-p] <dir>`; `--probe` anywhere
pub fn ask(args: &[String]) -> Ask {
    if args.iter().any(|a| a == "--probe") {
        return Ask::Probe;
    }
    let mut it = args.iter().skip_while(|a| *a != "--look-cover").skip(1).filter(|a| *a != "-p" && *a != "--path");
    match it.next() {
        Some(d) if !d.starts_with("--") => Ask::Show(PathBuf::from(d)),
        _ => Ask::Bad,
    }
}

/// look.json, as the QML reads it (anything unreadable: Material, dark, no pictures)
#[derive(Clone, Debug, PartialEq)]
pub struct Look {
    pub material: bool,
    pub dark: bool,
    pub backdrops: HashMap<String, String>,
}

pub fn read_look(text: Option<&str>) -> Look {
    let mut l = Look { material: true, dark: true, backdrops: HashMap::new() };
    let Some(j) = text.and_then(|t| serde_json::from_str::<serde_json::Value>(t).ok()) else { return l };
    l.material = j["to"].as_str() != Some("glass");
    l.backdrops = j["backdrops"].as_object().map(|o| o.iter().filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string()))).collect()).unwrap_or_default();
    l.dark = j["dark"].as_bool() != Some(false);
    l
}

// ── The mark's shapes (shell/common/materialshape.frag, in Rust) ───────────

/// [kind, n, a, b, rotation°, scale], as material/common/MaterialShapes.js:
/// Glass's squircle, then Material's cookies and flowers
pub const SHAPES: [[f32; 6]; 6] = [[2.0, 0.0, 0.3, 1.0, 0.0, 0.92], [0.0, 4.0, 0.16, 1.0, 45.0, 1.0], [0.0, 4.0, 0.34, 3.0, 45.0, 1.0], [0.0, 8.0, 0.11, 0.55, 0.0, 1.0], [0.0, 8.0, 0.16, 1.6, 0.0, 1.0], [0.0, 9.0, 0.08, 1.0, -90.0, 1.0]];

/// The shader's `field`: a distance-like value in unit coordinates
/// (kinds 0 star and 2 rounded box: the ones SHAPES has; others a circle)
pub fn field(px: f32, py: f32, s: &[f32; 6]) -> f32 {
    let (rot, scale) = (s[4].to_radians(), s[5]);
    let (c, sn) = (rot.cos(), rot.sin());
    let k = scale.max(1e-3);
    // (GLSL's mat2(c, -sn, sn, c) * p: columns (c, -sn) and (sn, c))
    let (x, y) = ((c * px + sn * py) / k, (-sn * px + c * py) / k);
    let (kind, n, a, b) = (s[0], s[1], s[2], s[3]);
    let d = if kind < 0.5 {
        let th = y.atan2(x);
        let r = 1.0 - a * (0.5 - 0.5 * (n * th).cos()).max(0.0).powf(b);
        ((x * x + y * y).sqrt() - r) * r
    } else if (1.5..2.5).contains(&kind) {
        let r = a.min(b);
        let (qx, qy) = (x.abs() - 1.0 + r, y.abs() - b + r);
        let (mx, my) = (qx.max(0.0), qy.max(0.0));
        (mx * mx + my * my).sqrt() + qx.max(qy).min(0.0) - r
    } else {
        (x * x + y * y).sqrt() - 1.0
    };
    d * scale
}

/// The mark's shape at `px` pixels square for a `MARK`-px box: a morph
/// from `a` to `b`, the fill and the 1.5 px border inside its edge, as the
/// shader draws it (premultiplied; edges and border in logical px)
pub fn raster(px: u32, a: &[f32; 6], b: &[f32; 6], morph: f32, dark: bool) -> slint::SharedPixelBuffer<slint::Rgba8Pixel> {
    let px = px.max(1);
    let mut buf = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(px, px);
    let unit = 0.5 * MARK;
    let (fill_a, border_a) = if dark { (0.2, 0.35) } else { (0.8, 0.9) };
    let border = 1.5;
    let data = buf.make_mut_slice();
    for j in 0..px {
        for i in 0..px {
            let x = (((i as f32 + 0.5) / px as f32) * MARK - unit) / unit;
            let y = (((j as f32 + 0.5) / px as f32) * MARK - unit) / unit;
            let fa = field(x, y, a);
            let d = (fa + (field(x, y, b) - fa) * morph) * unit;
            let cov = (0.5 - d).clamp(0.0, 1.0);
            let inner = (0.5 - (d + border)).clamp(0.0, 1.0);
            // (white at the fill's or the border's alpha, premultiplied)
            let alpha = (border_a + (fill_a - border_a) * inner) * cov;
            let v = (alpha * 255.0).round() as u8;
            data[(j * px + i) as usize] = slint::Rgba8Pixel { r: v, g: v, b: v, a: v };
        }
    }
    buf
}

/// QML's SpringAnimation (spring, damping, mass 1, epsilon), stepped as Qt
/// steps it: 16 ms at a time
#[derive(Clone, Debug)]
pub struct Spring {
    pub value: f32,
    pub velocity: f32,
    pub to: f32,
    pub spring: f32,
    pub damping: f32,
    pub epsilon: f32,
    pub running: bool,
}

impl Spring {
    pub fn new(value: f32) -> Self {
        Spring { value, velocity: 0.0, to: value, spring: 3.2, damping: 0.27, epsilon: 0.002, running: false }
    }

    /// One 16 ms step; false once it's settled (it then sits on `to`)
    pub fn step(&mut self) -> bool {
        if !self.running {
            return false;
        }
        let diff = self.to - self.value;
        self.velocity += self.spring * diff - self.damping * self.velocity;
        self.value += self.velocity * 16.0 / 1000.0;
        if self.velocity.abs() < self.epsilon && (self.to - self.value).abs() < self.epsilon {
            self.value = self.to;
            self.velocity = 0.0;
            self.running = false;
        }
        self.running
    }
}

enum M {
    Poll,
    Next,
    Tick,
    Leave,
    Quit,
}

struct Win {
    output: String,
    surface: Rc<Surface>,
    ui: ui::LookCover,
    opened: bool,
    settled: bool,
    /// the pixels the shape was drawn at for this window
    shape_px: u32,
}

pub struct LookCover {
    dir: PathBuf,
    look: Look,
    wins: Vec<Win>,
    leaving: bool,
    step: usize,
    from: usize,
    to: usize,
    morph: Spring,
    /// the spring's clock: time not yet stepped
    spring_at: Instant,
    shapes: HashMap<u32, slint::Image>,
    poll: slint::Timer,
    next: slint::Timer,
    tick: slint::Timer,
    give_up: slint::Timer,
    quit: slint::Timer,
}

impl LookCover {
    pub fn new(dir: PathBuf) -> Self {
        let look = read_look(std::fs::read_to_string(dir.join("../look.json")).ok().as_deref());
        LookCover {
            dir,
            look,
            wins: Vec::new(),
            leaving: false,
            step: 0,
            from: 0,
            to: 0,
            morph: Spring::new(1.0),
            spring_at: Instant::now(),
            shapes: HashMap::new(),
            poll: slint::Timer::default(),
            next: slint::Timer::default(),
            tick: slint::Timer::default(),
            give_up: slint::Timer::default(),
            quit: slint::Timer::default(),
        }
    }

    fn send(st: &State) -> impl Fn(M) + Clone + 'static {
        let to = st.shell.sender(NAME);
        move |m| to(Box::new(m))
    }

    /// The shape now, for every window (once per pixel size)
    fn draw_shape(&mut self) {
        self.shapes.clear();
        for w in self.wins.iter_mut() {
            let px = (MARK * w.ui.window().scale_factor()).round() as u32;
            let img = self
                .shapes
                .entry(px)
                .or_insert_with(|| slint::Image::from_rgba8_premultiplied(raster(px, &SHAPES[self.from], &SHAPES[self.to], self.morph.value, self.look.dark)))
                .clone();
            w.shape_px = px;
            w.ui.global::<ui::Lc>().set_shape(img);
        }
    }

    fn leave(&mut self, st: &State) {
        if self.leaving {
            return;
        }
        self.leaving = true;
        self.next.stop();
        self.poll.stop();
        for w in &self.wins {
            w.ui.global::<ui::Lc>().set_leaving(true);
        }
        // (QML: quits once the fade reaches 0)
        let to = Self::send(st);
        self.quit.start(slint::TimerMode::SingleShot, Duration::from_millis(450), move || to(M::Quit));
    }

    /// A backdrop for this output, decoded off the UI thread at its size
    fn backdrop(&self, st: &State, output: &WlOutput, name: &str) {
        let Some(path) = self.look.backdrops.get(name).cloned() else { return };
        let size = st.outputs.info(output).and_then(|i| i.modes.iter().find(|m| m.current).map(|m| m.dimensions)).unwrap_or((3840, 2160));
        let (w, h) = (size.0.max(1) as u32, size.1.max(1) as u32);
        let tx = st.tx.clone();
        let name = name.to_string();
        std::thread::spawn(move || {
            let Some(buf) = tessera_core::picture::decode(&path, w, h) else { return };
            let _ = tx.send(Msg::Part(NAME, Box::new(Backdrop(name, buf))));
        });
    }
}

struct Backdrop(String, slint::SharedPixelBuffer<slint::Rgb8Pixel>);

impl Part for LookCover {
    fn name(&self) -> &'static str {
        NAME
    }

    fn start(&mut self, st: &mut State) {
        eprintln!("tessera-ui: look cover, to {}, {}", if self.look.material { "material" } else { "glass" }, self.dir.display());
        let to = Self::send(st);
        self.poll.start(slint::TimerMode::Repeated, Duration::from_millis(100), move || to(M::Poll));
        let to = Self::send(st);
        self.next.start(slint::TimerMode::Repeated, Duration::from_millis(420), move || to(M::Next));
        // Never up for good (the script ends it; this is in case it can't)
        let to = Self::send(st);
        self.give_up.start(slint::TimerMode::SingleShot, Duration::from_secs(45), move || to(M::Leave));
        let to = Self::send(st);
        self.tick.start(slint::TimerMode::Repeated, Duration::from_millis(16), move || to(M::Tick));
    }

    fn output_added(&mut self, st: &mut State, output: &WlOutput, name: &str) {
        if self.wins.iter().any(|w| w.output == name) {
            return;
        }
        let surface = st.prepare_surface(Spec {
            layer: Layer::Overlay,
            namespace: NS,
            output: Some((output, name)),
            anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
            size: (0, 0),
            exclusive: -1,
            keyboard: KeyboardInteractivity::Exclusive,
            margin_left: 0,
        });
        let ui = ui::LookCover::new().expect("LookCover");
        let g = ui.global::<ui::Lc>();
        g.set_material(self.look.material);
        g.set_dark(self.look.dark);
        g.set_leaving(self.leaving);
        ui.show().expect("show");
        self.wins.push(Win { output: name.to_string(), surface, ui, opened: false, settled: false, shape_px: 0 });
        self.backdrop(st, output, name);
        self.draw_shape();
    }

    fn output_removed(&mut self, st: &mut State, name: &str) {
        if let Some(i) = self.wins.iter().position(|w| w.output == name) {
            let w = self.wins.remove(i);
            st.drop_surface(&w.surface);
        }
    }

    fn surface_closed(&mut self, st: &mut State, surface: &Rc<Surface>) {
        if let Some(i) = self.wins.iter().position(|w| Rc::ptr_eq(&w.surface, surface)) {
            let w = self.wins.remove(i);
            st.drop_surface(&w.surface);
        }
    }

    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        let msg = match msg.downcast::<Backdrop>() {
            Ok(b) => {
                let Backdrop(name, buf) = *b;
                if let Some(w) = self.wins.iter().find(|w| w.output == name) {
                    w.ui.set_backdrop(slint::Image::from_rgb8(buf));
                    w.ui.set_has_backdrop(true);
                }
                return;
            }
            Err(m) => m,
        };
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Poll => {
                if std::fs::read_to_string(self.dir.join("../status")).is_ok_and(|t| t.trim() == "done") {
                    self.leave(st);
                }
            }
            M::Next => {
                if self.leaving {
                    return;
                }
                // (QML: from = the shape it was going to, to = the new one, morph from 0)
                self.step = (self.step + 1) % SHAPES.len();
                self.from = self.to;
                self.to = self.step;
                self.morph = Spring { value: 0.0, velocity: 0.0, to: 1.0, running: true, ..Spring::new(0.0) };
                self.spring_at = Instant::now();
                self.draw_shape();
            }
            M::Tick => {
                if !self.morph.running {
                    // (a window whose scale changed draws its shape again)
                    if self.wins.iter().any(|w| w.shape_px != (MARK * w.ui.window().scale_factor()).round() as u32) {
                        self.draw_shape();
                    }
                    return;
                }
                let mut moved = false;
                while self.spring_at.elapsed() >= Duration::from_millis(16) {
                    self.spring_at += Duration::from_millis(16);
                    moved = true;
                    if !self.morph.step() {
                        break;
                    }
                }
                if moved {
                    self.draw_shape();
                }
            }
            M::Leave => self.leave(st),
            M::Quit => st.exit = true,
        }
    }

    fn after_frame(&mut self, _st: &mut State) {
        for w in self.wins.iter_mut() {
            tessera_core::picture::second_frame(&w.surface, &mut w.settled);
            // (fades in once it's on screen: QML's shown = 1 at start)
            if !w.opened && w.surface.mapped.get() {
                w.opened = true;
                w.ui.global::<ui::Lc>().set_opened(true);
            }
        }
    }
}

/// A font file from the screen's copy (read, not mapped: the copy goes
/// when the switch ends); false: not there
pub fn register_font(path: &Path) -> bool {
    use slint::fontique_011::fontique;
    let Ok(data) = std::fs::read(path) else { return false };
    if data.is_empty() {
        return false;
    }
    let mut collection = slint::fontique_011::shared_collection();
    !collection.register_fonts(fontique::Blob::new(std::sync::Arc::new(data)), None).is_empty()
}

/// The cover's own loop (main.rs's, with this one part and no shell on
/// stdin): ends when switch-look.sh says done, after 45 s, or on SIGTERM.
pub fn run(args: &[String], started: Instant) {
    use calloop::signals::{Signal, Signals};
    use calloop::EventLoop;
    use smithay_client_toolkit::reexports::calloop_wayland_source::WaylandSource;

    let dir = match ask(args) {
        Ask::Probe => return,
        Ask::Show(d) => d,
        Ask::Bad => {
            eprintln!("tessera-ui: usage: tessera-ui --look-cover [-p] <dir> (the switch's copy of the screen: <dir>/../look.json, <dir>/../status)");
            std::process::exit(2);
        }
    };
    let mut event_loop: EventLoop<'static, State> = EventLoop::try_new().expect("event loop");
    let lh = event_loop.handle();
    let signals = Signals::new(&[Signal::SIGTERM, Signal::SIGINT]).expect("signalfd");
    lh.insert_source(signals, |_, _, state: &mut State| state.exit = true).unwrap();

    let Ok(conn) = smithay_client_toolkit::reexports::client::Connection::connect_to_env() else {
        eprintln!("tessera-ui: look cover: no Wayland display");
        std::process::exit(1);
    };
    let Ok((globals, queue)) = smithay_client_toolkit::reexports::client::globals::registry_queue_init(&conn) else {
        eprintln!("tessera-ui: look cover: the compositor's registry didn't answer");
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

    let egl = tessera_core::egl::Egl::new(conn.backend().display_ptr() as *mut _).expect("EGL");
    slint::platform::set_platform(Box::new(tessera_core::wayland::Platform { tx: tx.clone() })).expect("platform");
    // The fonts from the screen's copy; the repository's for any it lacks
    // (a Glass install's copy may have no Google Sans Flex)
    let phosphor = register_font(&dir.join("Phosphor-Fill.ttf"));
    let flex = register_font(&dir.join("GoogleSansFlex.ttf"));
    if !phosphor || !flex {
        tessera_core::runloop::register_fonts();
    }
    let glass = tessera_core::services::hypr::Glass::spawn(tx.clone());
    let parts: Vec<Box<dyn Part>> = vec![Box::new(LookCover::new(dir))];
    let shell = tessera_core::shell::Shell::new(tx.clone(), glass, tessera_core::surfaces::Surfaces::default());
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
            eprintln!("tessera-ui: look cover: event loop: {e}");
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
    fn asked_for_with_its_folder() {
        assert!(wanted(&args(&["--look-cover", "-p", "/r/tessera-look.x/ui"])));
        assert!(!wanted(&args(&["--update-screen", "/r/x"])));
        assert_eq!(ask(&args(&["--look-cover", "-p", "/r/tessera-look.x/ui"])), Ask::Show("/r/tessera-look.x/ui".into()));
        assert_eq!(ask(&args(&["--look-cover", "/r/tessera-look.x/ui"])), Ask::Show("/r/tessera-look.x/ui".into()));
        assert_eq!(ask(&args(&["--look-cover", "--probe"])), Ask::Probe);
        assert_eq!(ask(&args(&["--look-cover"])), Ask::Bad);
    }

    #[test]
    fn look_json_as_switch_look_writes_it() {
        let l = read_look(Some(r#"{"to": "glass", "dark": false, "backdrops": {"DP-1": "/c/backdrop-DP-1.png"}}"#));
        assert!(!l.material && !l.dark);
        assert_eq!(l.backdrops.get("DP-1").map(String::as_str), Some("/c/backdrop-DP-1.png"));
        let l = read_look(Some(r#"{"to": "material"}"#));
        assert!(l.material && l.dark && l.backdrops.is_empty());
        let l = read_look(None);
        assert!(l.material && l.dark);
        assert!(read_look(Some("not json")).material);
    }

    #[test]
    fn shapes_fill_the_middle_and_leave_the_corners() {
        for s in SHAPES.iter() {
            assert!(field(0.0, 0.0, s) < 0.0, "{s:?}");
            assert!(field(0.99, 0.99, s) > 0.0, "{s:?}");
        }
        let buf = raster(168, &SHAPES[0], &SHAPES[1], 0.5, true);
        let px = buf.as_slice();
        // (middle: the fill, white at 20%; corner: nothing; the edge: the brighter border)
        assert_eq!(px[84 * 168 + 84].a, 51);
        assert_eq!(px[0].a, 0);
        assert!(px.iter().any(|p| p.a == 89));
        let light = raster(168, &SHAPES[0], &SHAPES[0], 1.0, false);
        assert_eq!(light.as_slice()[84 * 168 + 84].a, 204);
    }

    #[test]
    fn the_spring_overshoots_a_little_and_settles() {
        let mut s = Spring { value: 0.0, to: 1.0, running: true, ..Spring::new(0.0) };
        let mut top = 0.0f32;
        let mut steps = 0;
        while s.step() {
            top = top.max(s.value);
            steps += 1;
            assert!(steps < 200);
        }
        assert!(top > 1.0 && top < 1.2, "{top}");
        assert_eq!(s.value, 1.0);
        // (within epsilon in under a second: the next shape comes after 420 ms, while it still wobbles a little)
        assert!(steps * 16 < 1000, "{steps}");
    }
}

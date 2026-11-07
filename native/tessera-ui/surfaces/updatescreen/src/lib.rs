//! The update screen, in Rust: `tessera-ui --update-screen [-p] <dir>`, in
//! place of `qs -p <dir>` with shell/assets/updater/shell.qml. A full-screen
//! screen on every display while update-run.sh stops the shell, updates,
//! rebuilds and starts the new shell. Not a shell surface: its own process
//! and loop (`run`, like the greeter's), so it stays up while the shell
//! restarts; src/main.rs hands over to it before anything else.
//!
//! <dir> is the update's copy of the screen (`$T/ui`); like the QML, it reads
//!   <dir>/../status     update-run.sh's: state \t percent \t text \t detail[\x1f backup folder]
//!                       (state run | restart | done: fade out, quit | error: wait for Close)
//!   <dir>/../look.json  the menu icon: {"glyph": Phosphor glyph, "image": a picture's path}
//!   <dir>/Phosphor-Fill.ttf  the glyph's font (else the repository's, as the shell finds it)
//! The `-p` is accepted (and passed by the scripts) so the process reads
//! like the QML one's (`… -p $T/ui`): update-run.sh's own checks for its
//! screen (`pgrep -f -- "-p $T/ui"`, is_tessera) find either. The path holds
//! `/tessera-update.`, which every script that stops shells skips.
//! `--probe` only says it's there (exit 0, nothing drawn): the scripts ask
//! before choosing it over the QML (an older tessera-ui exits 2).
//!
//! As the QML: up at least 5 s; the status read every 250 ms until done or
//! error; done: 600 ms or more of "up to date", then a 400 ms fade and quit;
//! error: the keyboard (Return, Escape) and the pointer (Close), else clicks
//! pass through; gone after 15 minutes unless it's showing an error.

pub mod ui {
    slint::include_modules!();
}

use slint::ComponentHandle;
use smithay_client_toolkit::reexports::client::protocol::wl_output::WlOutput;
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use smithay_client_toolkit::shell::WaylandSurface;
use std::any::Any;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};
use tessera_core::shell::Part;
use tessera_core::wayland::{Spec, State, Surface};
use tessera_core::Msg;

const NAME: &str = "updatescreen";
const NS: &str = "tessera:update";

/// Asked for: `--update-screen`
pub fn wanted(args: &[String]) -> bool {
    args.iter().skip(1).any(|a| a == "--update-screen")
}

/// `--update-screen [-p] <dir>`: the dir; `--probe` anywhere: none needed
#[derive(Debug, PartialEq)]
pub enum Ask {
    Probe,
    Show(PathBuf),
    Bad,
}

pub fn ask(args: &[String]) -> Ask {
    if args.iter().any(|a| a == "--probe") {
        return Ask::Probe;
    }
    let mut it = args.iter().skip_while(|a| *a != "--update-screen").skip(1).filter(|a| *a != "-p" && *a != "--path");
    match it.next() {
        Some(d) if !d.starts_with("--") => Ask::Show(PathBuf::from(d)),
        _ => Ask::Bad,
    }
}

/// update-run.sh's status line, as the QML reads it
#[derive(Clone, Debug, PartialEq)]
pub struct Status {
    pub phase: String,
    pub percent: f32,
    pub step: String,
    pub detail: String,
    pub backup: String,
}

impl Default for Status {
    fn default() -> Self {
        Status { phase: "run".into(), percent: 0.0, step: "Preparing".into(), detail: String::new(), backup: String::new() }
    }
}

/// JavaScript's parseFloat: the number at the start, else None
fn parse_float(s: &str) -> Option<f32> {
    let s = s.trim_start();
    let mut end = 0;
    let b = s.as_bytes();
    let (mut dot, mut digits) = (false, false);
    if end < b.len() && (b[end] == b'+' || b[end] == b'-') {
        end += 1;
    }
    while end < b.len() {
        match b[end] {
            b'0'..=b'9' => digits = true,
            b'.' if !dot => dot = true,
            _ => break,
        }
        end += 1;
    }
    if !digits {
        return None;
    }
    s[..end].parse().ok()
}

/// The status after reading `text` (the file), from `prev`: None when it
/// has fewer than three fields (it's left as it was). The percent only rises.
pub fn read_status(text: &str, prev: &Status, home: &str) -> Option<Status> {
    let f: Vec<&str> = text.trim().split('\t').collect();
    if f.len() < 3 {
        return None;
    }
    let pct = parse_float(f[1]).filter(|v| !v.is_nan()).unwrap_or(0.0);
    let mut d = f.get(3).copied().unwrap_or("").split('\u{1f}');
    let detail = d.next().unwrap_or("").to_string();
    let backup = d.next().unwrap_or("");
    // (JavaScript's replace: the first one only)
    let backup = if home.is_empty() { backup.to_string() } else { backup.replacen(home, "~", 1) };
    Some(Status { phase: f[0].to_string(), percent: prev.percent.max(pct.min(100.0)), step: f[2].to_string(), detail, backup })
}

/// How long "up to date" stays before the fade: the screen is up at least
/// 5 s in all, however quick the update (and the message at least 600 ms)
pub fn finish_pause(open_for: Duration) -> Duration {
    Duration::from_millis(5000).saturating_sub(open_for).max(Duration::from_millis(600))
}

enum M {
    Poll,
    Close,
    Fade,
    Quit,
    GiveUp,
}

struct Win {
    output: String,
    surface: Rc<Surface>,
    ui: ui::UpdateScreen,
    opened: bool,
    settled: bool,
}

pub struct UpdateScreen {
    dir: PathBuf,
    status: Status,
    home: String,
    glyph: String,
    image: Option<slint::Image>,
    wins: Vec<Win>,
    opened_at: Instant,
    finishing: bool,
    poll: slint::Timer,
    finish: slint::Timer,
    quit: slint::Timer,
    give_up: slint::Timer,
}

impl UpdateScreen {
    pub fn new(dir: PathBuf, font: bool) -> Self {
        let (mut glyph, mut image) = (String::new(), None);
        if let Some(j) = std::fs::read_to_string(dir.join("../look.json")).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()) {
            glyph = j["glyph"].as_str().unwrap_or("").to_string();
            let path = j["image"].as_str().unwrap_or("");
            if !path.is_empty() {
                image = picture(Path::new(path));
            }
        }
        if !font {
            glyph.clear();
        }
        UpdateScreen {
            dir,
            status: Status::default(),
            home: std::env::var("HOME").unwrap_or_default(),
            glyph,
            image,
            wins: Vec::new(),
            opened_at: Instant::now(),
            finishing: false,
            poll: slint::Timer::default(),
            finish: slint::Timer::default(),
            quit: slint::Timer::default(),
            give_up: slint::Timer::default(),
        }
    }

    fn send(st: &State) -> impl Fn(M) + Clone + 'static {
        let to = st.shell.sender(NAME);
        move |m| to(Box::new(m))
    }

    fn error(&self) -> bool {
        self.status.phase == "error"
    }

    fn reload(&mut self, st: &mut State) {
        let Ok(text) = std::fs::read_to_string(self.dir.join("../status")) else { return };
        let Some(s) = read_status(&text, &self.status, &self.home) else { return };
        let was_error = self.error();
        self.status = s;
        self.push();
        if self.error() != was_error {
            self.input(st);
        }
        if self.status.phase == "done" || self.error() {
            self.poll.stop();
        }
        if self.status.phase == "done" && !self.finishing {
            self.finishing = true;
            let to = Self::send(st);
            self.finish.start(slint::TimerMode::SingleShot, finish_pause(self.opened_at.elapsed()), move || to(M::Fade));
        }
    }

    fn push(&self) {
        for w in &self.wins {
            let g = w.ui.global::<ui::Up>();
            g.set_phase(self.status.phase.clone().into());
            g.set_percent(self.status.percent);
            g.set_step(self.status.step.clone().into());
            g.set_detail(self.status.detail.clone().into());
            g.set_backup(self.status.backup.clone().into());
        }
    }

    /// Only the error screen takes input (QML: its mask and keyboard focus)
    fn input(&self, st: &mut State) {
        let error = self.error();
        for w in &self.wins {
            Self::input_of(st, w, error);
        }
    }

    fn input_of(st: &mut State, w: &Win, error: bool) {
        st.set_input_region(&w.surface, if error { None } else { Some(&[]) });
        w.surface.layer.set_keyboard_interactivity(if error { KeyboardInteractivity::Exclusive } else { KeyboardInteractivity::None });
        w.surface.layer.commit();
        if error {
            w.ui.invoke_focus_keys();
        }
    }
}

/// The menu icon's picture (the distro's logo: SVG or a bitmap), 52 px at 2×
fn picture(path: &Path) -> Option<slint::Image> {
    let img = tessera_core::appicon::load(path, 104)?;
    let mut buf = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(img.w as u32, img.h as u32);
    for (d, s) in buf.make_mut_slice().iter_mut().zip(img.px.iter()) {
        let c = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        *d = slint::Rgba8Pixel { r: c(s[0]), g: c(s[1]), b: c(s[2]), a: c(s[3]) };
    }
    Some(slint::Image::from_rgba8_premultiplied(buf))
}

impl Part for UpdateScreen {
    fn name(&self) -> &'static str {
        NAME
    }

    fn start(&mut self, st: &mut State) {
        eprintln!("tessera-ui: update screen, {}", self.dir.display());
        self.opened_at = Instant::now();
        let to = Self::send(st);
        self.poll.start(slint::TimerMode::Repeated, Duration::from_millis(250), move || to(M::Poll));
        // A last way out if the updater itself is gone
        let to = Self::send(st);
        self.give_up.start(slint::TimerMode::SingleShot, Duration::from_secs(15 * 60), move || to(M::GiveUp));
        self.reload(st);
    }

    fn output_added(&mut self, st: &mut State, output: &WlOutput, name: &str) {
        if self.wins.iter().any(|w| w.output == name) {
            return;
        }
        let error = self.error();
        let surface = st.prepare_surface(Spec {
            layer: Layer::Overlay,
            namespace: NS,
            output: Some((output, name)),
            anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
            size: (0, 0),
            exclusive: -1,
            keyboard: if error { KeyboardInteractivity::Exclusive } else { KeyboardInteractivity::None },
            margin_left: 0,
        });
        let ui = ui::UpdateScreen::new().expect("UpdateScreen");
        let g = ui.global::<ui::Up>();
        g.set_glyph(self.glyph.clone().into());
        g.set_has_image(self.image.is_some());
        if let Some(i) = &self.image {
            g.set_image(i.clone());
        }
        let to = Self::send(st);
        g.on_close(move || to(M::Close));
        // (a window made while it fades out starts out faded)
        g.set_leaving(self.quit.running());
        ui.show().expect("show");
        let w = Win { output: name.to_string(), surface, ui, opened: false, settled: false };
        st.set_input_region(&w.surface, if error { None } else { Some(&[]) });
        if error {
            w.ui.invoke_focus_keys();
        }
        self.wins.push(w);
        self.push();
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
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Poll => self.reload(st),
            M::Close => st.exit = true,
            M::GiveUp => {
                if !self.error() {
                    st.exit = true;
                }
            }
            M::Fade => {
                for w in &self.wins {
                    w.ui.global::<ui::Up>().set_leaving(true);
                }
                let to = Self::send(st);
                self.quit.start(slint::TimerMode::SingleShot, Duration::from_millis(450), move || to(M::Quit));
            }
            M::Quit => st.exit = true,
        }
    }

    fn after_frame(&mut self, _st: &mut State) {
        for w in self.wins.iter_mut() {
            tessera_core::picture::second_frame(&w.surface, &mut w.settled);
            // (fades in once it's on screen: QML's shown = 1 at start)
            if !w.opened && w.surface.mapped.get() {
                w.opened = true;
                w.ui.global::<ui::Up>().set_opened(true);
            }
        }
    }
}

/// A font file from the screen's copy, for Slint's text; false: not there
pub fn register_font(path: &Path) -> bool {
    use slint::fontique_011::fontique;
    let Ok(file) = std::fs::File::open(path) else { return false };
    // (read, not mapped: the copy is removed while the screen may still run)
    let mut data = Vec::new();
    if std::io::Read::read_to_end(&mut &file, &mut data).is_err() || data.is_empty() {
        return false;
    }
    let mut collection = slint::fontique_011::shared_collection();
    !collection.register_fonts(fontique::Blob::new(std::sync::Arc::new(data)), None).is_empty()
}

/// The screen's own loop (main.rs's, with this one part and no shell on
/// stdin): ends when it's done, closed, after 15 minutes, or on SIGTERM.
pub fn run(args: &[String], started: Instant) {
    use calloop::signals::{Signal, Signals};
    use calloop::EventLoop;
    use smithay_client_toolkit::reexports::calloop_wayland_source::WaylandSource;

    let dir = match ask(args) {
        Ask::Probe => return,
        Ask::Show(d) => d,
        Ask::Bad => {
            eprintln!("tessera-ui: usage: tessera-ui --update-screen [-p] <dir> (the update's copy of the screen: <dir>/../status)");
            std::process::exit(2);
        }
    };
    let mut event_loop: EventLoop<'static, State> = EventLoop::try_new().expect("event loop");
    let lh = event_loop.handle();
    let signals = Signals::new(&[Signal::SIGTERM, Signal::SIGINT]).expect("signalfd");
    lh.insert_source(signals, |_, _, state: &mut State| state.exit = true).unwrap();

    let Ok(conn) = smithay_client_toolkit::reexports::client::Connection::connect_to_env() else {
        eprintln!("tessera-ui: update screen: no Wayland display");
        std::process::exit(1);
    };
    let Ok((globals, queue)) = smithay_client_toolkit::reexports::client::globals::registry_queue_init(&conn) else {
        eprintln!("tessera-ui: update screen: the compositor's registry didn't answer");
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
    // The glyph's font from the screen's copy (the repository is being
    // rewritten meanwhile); the repository's when the copy has none
    let font = register_font(&dir.join("Phosphor-Fill.ttf")) || {
        tessera_core::runloop::register_fonts();
        true
    };
    let glass = tessera_core::services::hypr::Glass::spawn(tx.clone());
    let parts: Vec<Box<dyn Part>> = vec![Box::new(UpdateScreen::new(dir, font))];
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
            eprintln!("tessera-ui: update screen: event loop: {e}");
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
        assert!(wanted(&args(&["--update-screen", "/run/user/1000/tessera-update.ab12/ui"])));
        assert!(!wanted(&args(&["--surfaces", "osd"])));
        assert_eq!(ask(&args(&["--update-screen", "-p", "/r/tessera-update.x/ui"])), Ask::Show("/r/tessera-update.x/ui".into()));
        assert_eq!(ask(&args(&["--update-screen", "/r/tessera-update.x/ui"])), Ask::Show("/r/tessera-update.x/ui".into()));
        assert_eq!(ask(&args(&["--update-screen", "--probe"])), Ask::Probe);
        assert_eq!(ask(&args(&["--update-screen"])), Ask::Bad);
        assert_eq!(ask(&args(&["--update-screen", "-p"])), Ask::Bad);
    }

    #[test]
    fn the_status_line_as_update_run_writes_it() {
        let s0 = Status::default();
        let s = read_status("run\t42\tDownloading\t\n", &s0, "/home/ada").unwrap();
        assert_eq!((s.phase.as_str(), s.percent, s.step.as_str(), s.detail.as_str()), ("run", 42.0, "Downloading", ""));
        // (the percent never goes back, and stops at 100)
        let s2 = read_status("run\t10\tBuilding", &s, "/home/ada").unwrap();
        assert_eq!(s2.percent, 42.0);
        assert_eq!(read_status("done\t250\tUpdate installed\t", &s2, "").unwrap().percent, 100.0);
        assert_eq!(read_status("run\tabc\tX", &s0, "").unwrap().percent, 0.0);
        assert_eq!(read_status("run\t92.5%\tX", &s0, "").unwrap().percent, 92.5);
        // too few fields: left as it was
        assert_eq!(read_status("run\t5", &s, ""), None);
        assert_eq!(read_status("", &s, ""), None);
        let e = read_status("error\t100\tThe update didn't finish\tIt broke.\u{1f}/home/ada/.local/state/tessera/backup-1\n", &s, "/home/ada").unwrap();
        assert_eq!((e.phase.as_str(), e.detail.as_str(), e.backup.as_str()), ("error", "It broke.", "~/.local/state/tessera/backup-1"));
    }

    #[test]
    fn up_at_least_five_seconds() {
        assert_eq!(finish_pause(Duration::from_millis(1000)), Duration::from_millis(4000));
        assert_eq!(finish_pause(Duration::from_millis(4800)), Duration::from_millis(600));
        assert_eq!(finish_pause(Duration::from_secs(90)), Duration::from_millis(600));
    }
}

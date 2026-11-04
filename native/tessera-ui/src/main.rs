//! tessera-ui: Tessera's interface in Rust (docs/rust-ui-plan.md), Slint on
//! wlr-layer-shell, drawing the shell's surfaces one by one in place of the
//! QML shell's (src/surfaces.rs). One process, one thread for the UI (a
//! calloop event loop: Wayland, Slint's timers and animations, signals,
//! stdin, and messages from the data threads).
//!
//!   tessera-ui [--surfaces a,b]  draw these (default: all it has); ends on SIGTERM/SIGINT,
//!                                or when stdin, a pipe, closes (the shell that started it quit)
//!   tessera-ui --material-roles SEED dark|light [style]   print Material roles (for checking colours)
//!   tessera-ui --seed PICTURE    print the Material seed a wallpaper gives (as matugen)
//!   tessera-ui --version
//! SIGUSR1 prints frame statistics.

mod a11y;
mod app;
mod egl;
mod env;
mod hypr;
mod seed;
mod settings;
mod stats;
mod status;
mod surfaces;
mod theme;
mod wayland;

pub mod ui {
    slint::include_modules!();
}

pub use app::App;
use calloop::signals::{Signal, Signals};
use calloop::{generic::Generic, EventLoop, Interest, Mode, PostAction};
use smithay_client_toolkit::reexports::calloop_wayland_source::WaylandSource;
use std::time::{Duration, Instant};
use wayland::State;

pub enum Msg {
    Run(Box<dyn FnOnce() + Send>),
    Quit,
    Hypr(hypr::Snapshot),
    Battery(status::Battery),
    Wifi(status::Wifi),
    GlassPlugin(bool),
    ConfigChanged,
    Tick,
    ToggleMenu(String),
    MenuActivated(i32, String),
    MenuDismissed,
    MenuClosed,
    A11yActivate(u64),
    A11yAction(u64, accesskit::ActionRequest),
}

fn handle(state: &mut State, msg: Msg) {
    if crate::env::is_set("DEBUG") {
        match &msg {
            Msg::ToggleMenu(o) => eprintln!("debug: toggle menu on {o}"),
            Msg::MenuClosed => eprintln!("debug: menu closed"),
            Msg::MenuDismissed => eprintln!("debug: menu dismissed"),
            _ => {}
        }
    }
    match msg {
        Msg::Run(f) => f(),
        Msg::Quit => state.exit = true,
        Msg::Hypr(s) => {
            state.app.hypr = s;
            state.app_refresh();
        }
        Msg::Battery(b) => {
            state.app.battery = b;
            state.app_refresh();
        }
        Msg::Wifi(w) => {
            state.app.wifi = w;
            state.app_refresh();
        }
        Msg::GlassPlugin(ok) => state.app_glass_plugin(ok),
        Msg::ConfigChanged => state.app_look_changed(),
        Msg::Tick => state.app_tick(),
        Msg::ToggleMenu(o) => state.app_toggle_menu(o),
        Msg::MenuActivated(i, t) => state.app_menu_activated(i, t),
        Msg::MenuDismissed => state.app_close_menu(),
        Msg::MenuClosed => state.app_menu_closed(),
        Msg::A11yActivate(id) => {
            for a in state.a11y_all() {
                if a.id == id {
                    a.activate();
                }
            }
        }
        Msg::A11yAction(id, r) => {
            for a in state.a11y_all() {
                if a.id == id {
                    a.action(r.clone());
                }
            }
        }
    }
}

impl State {
    fn a11y_all(&mut self) -> Vec<&mut a11y::A11y> {
        let mut v: Vec<&mut a11y::A11y> = self.app.bars.iter_mut().filter_map(|b| b.a11y.as_mut()).collect();
        if let Some(m) = self.app.menu.as_mut().and_then(|m| m.a11y.as_mut()) {
            v.push(m);
        }
        v
    }
}

/// Fonts that aren't system fonts: Phosphor (icons) and Google Sans Flex
/// ("Tessera Sans Flex", Material's), mapped from the repo, registered with
/// Slint's font collection at run time (not compiled in)
fn register_fonts() {
    use slint::fontique_011::fontique;
    let roots = [
        std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../..")),
        std::path::PathBuf::from(crate::env::var("ROOT").unwrap_or_default()),
    ];
    let mut collection = slint::fontique_011::shared_collection();
    for f in ["shell/assets/fonts/Phosphor.ttf", "shell/assets/fonts/Phosphor-Fill.ttf", "shell/assets/fonts/Phosphor-Bold.ttf", "material/assets/fonts/GoogleSansFlex.ttf"] {
        let Some(file) = roots.iter().find_map(|r| std::fs::File::open(r.join(f)).ok()) else {
            eprintln!("tessera-ui: font {f} not found");
            continue;
        };
        if let Ok(map) = unsafe { memmap2::Mmap::map(&file) } {
            collection.register_fonts(fontique::Blob::new(std::sync::Arc::new(map)), None);
        }
    }
}

/// Watch config.json (the shell writes it whole, then renames or closes it)
fn watch_config(tx: calloop::channel::Sender<Msg>) {
    let path = theme::config_path();
    let Some(dir) = path.parent().map(|d| d.to_owned()) else { return };
    std::thread::spawn(move || unsafe {
        let fd = libc::inotify_init1(libc::IN_CLOEXEC);
        let c = std::ffi::CString::new(dir.as_os_str().as_encoded_bytes()).unwrap();
        if fd < 0 || libc::inotify_add_watch(fd, c.as_ptr(), libc::IN_CLOSE_WRITE | libc::IN_MOVED_TO) < 0 {
            return;
        }
        let mut buf = [0u8; 4096];
        loop {
            let n = libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len());
            if n <= 0 {
                return;
            }
            let mut off = 0usize;
            let mut hit = false;
            while off + std::mem::size_of::<libc::inotify_event>() <= n as usize {
                let ev = &*(buf.as_ptr().add(off) as *const libc::inotify_event);
                let name = std::slice::from_raw_parts(buf.as_ptr().add(off + std::mem::size_of::<libc::inotify_event>()), ev.len as usize);
                hit |= name.starts_with(b"config.json\0") || name == b"config.json";
                off += std::mem::size_of::<libc::inotify_event>() + ev.len as usize;
            }
            if hit && tx.send(Msg::ConfigChanged).is_err() {
                return;
            }
        }
    });
}

fn main() {
    let started = Instant::now();
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--material-roles") {
        let seed = args.get(2).map_or("#6750A4", String::as_str);
        let dark = args.get(3).map_or("dark", String::as_str) == "dark";
        for (k, v) in theme::material_roles(seed, dark, args.get(4).map_or("tonalSpot", String::as_str)) {
            println!("{k} {v}");
        }
        return;
    }
    if args.get(1).map(String::as_str) == Some("--seed") {
        match args.get(2).and_then(|p| seed::from_picture(std::path::Path::new(p))) {
            Some(s) => println!("{s}"),
            None => std::process::exit(1),
        }
        return;
    }
    if args.get(1).map(String::as_str) == Some("--version") {
        println!("tessera-ui {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    let surfaces = surfaces::Surfaces::from_args(&args);
    if surfaces.on.is_empty() {
        // (nothing it can draw: say so, and leave the shell drawing them)
        surfaces::announce(&surfaces);
        return;
    }

    let mut event_loop: EventLoop<'static, State> = EventLoop::try_new().expect("event loop");
    let lh = event_loop.handle();
    // Signals first: the threads started below inherit the blocked mask, so
    // SIGTERM always reaches this loop (and the bar ends cleanly)
    let signals = Signals::new(&[Signal::SIGTERM, Signal::SIGINT, Signal::SIGUSR1]).expect("signalfd");
    lh
        .insert_source(signals, |ev, _, state: &mut State| match ev.signal() {
            Signal::SIGUSR1 => state.app_dump_stats(),
            _ => state.exit = true,
        })
        .unwrap();

    let conn = smithay_client_toolkit::reexports::client::Connection::connect_to_env().expect("no Wayland display");
    let (globals, queue) = smithay_client_toolkit::reexports::client::globals::registry_queue_init(&conn).expect("registry");
    let qh = queue.handle();
    WaylandSource::new(conn.clone(), queue).insert(lh.clone()).unwrap();

    let (tx, rx) = calloop::channel::channel::<Msg>();
    lh
        .insert_source(rx, |ev, _, state: &mut State| {
            if let calloop::channel::Event::Msg(m) = ev {
                handle(state, m);
            }
        })
        .unwrap();

    // Ends when stdin (a pipe from whoever started us) closes
    let stdin_is_pipe = unsafe {
        let mut st: libc::stat = std::mem::zeroed();
        libc::fstat(0, &mut st) == 0 && matches!(st.st_mode & libc::S_IFMT, libc::S_IFIFO | libc::S_IFSOCK)
    };
    if stdin_is_pipe {
        let src = Generic::new(unsafe { calloop::generic::FdWrapper::new(0) }, Interest::READ, Mode::Level);
        lh
            .insert_source(src, |_, _, state: &mut State| {
                let mut buf = [0u8; 256];
                let n = unsafe { libc::read(0, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
                if n <= 0 {
                    state.exit = true;
                    return Ok(PostAction::Remove);
                }
                Ok(PostAction::Continue)
            })
            .unwrap();
    }

    let egl = egl::Egl::new(conn.backend().display_ptr() as *mut _).expect("EGL");
    slint::platform::set_platform(Box::new(wayland::Platform { tx: tx.clone() })).expect("platform");
    register_fonts();

    let glass = hypr::Glass::spawn(tx.clone());
    let mut state = State::new(&globals, qh, egl, lh.clone(), tx.clone(), App::new(glass, surfaces), started);
    hypr::spawn_events(tx.clone());
    status::spawn(tx.clone());
    watch_config(tx.clone());
    state.app_tick();

    while !state.exit {
        let timeout = if state.wants_frame_soon() {
            Some(Duration::ZERO)
        } else if state.animating_unpaced() {
            Some(Duration::from_millis(16))
        } else {
            slint::platform::duration_until_next_timer_update()
        };
        if let Err(e) = event_loop.dispatch(timeout, &mut state) {
            eprintln!("tessera-ui: event loop: {e}");
            break;
        }
        slint::platform::update_timers_and_animations();
        state.render_all();
        state.app_sync_glass();
        let kbd = state.keyboard_surface();
        for b in &mut state.app.bars {
            let f = kbd.as_ref().is_some_and(|k| std::rc::Rc::ptr_eq(k, &b.surface));
            if let Some(a) = b.a11y.as_mut() {
                a.update(false, f);
            }
        }
        if let Some(m) = state.app.menu.as_mut() {
            let f = kbd.as_ref().is_some_and(|k| std::rc::Rc::ptr_eq(k, &m.surface));
            if let Some(a) = m.a11y.as_mut() {
                a.update(false, f);
            }
        }
    }
    state.app_quit();
    eprintln!("tessera-ui: bye");
}

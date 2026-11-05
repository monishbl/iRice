//! tessera-ui: Tessera's interface in Rust (docs/rust-ui-plan.md), Slint on
//! wlr-layer-shell, drawing the shell's surfaces one by one in place of the
//! QML shell's (src/surfaces.rs: one crate each, native/tessera-ui/surfaces/;
//! what they share is tessera-core, native/tessera-ui/core/). One process,
//! one thread for the UI (a calloop event loop: Wayland, Slint's timers and
//! animations, signals, stdin, and messages from the data threads).
//!
//!   tessera-ui [--surfaces a,b]  draw these (default: all it has); ends on SIGTERM/SIGINT,
//!                                or when stdin, a pipe, closes (the shell that started it quit);
//!                                the shell's JSON lines on stdin: commands
//!                                `{"cmd":…,"surface":…}` (src/shell.rs) and
//!                                `{"to":"osd",…}` for that surface (RustUi.tell)
//!   tessera-ui --material-roles SEED dark|light [style]   print Material roles (for checking colours)
//!   tessera-ui --seed PICTURE    print the Material seed a wallpaper gives (as matugen)
//!   tessera-ui --version
//!   tessera-ui --greeter [--style …] [--demo [STATE]]   the login screen (surfaces/greeter)
//! SIGUSR1 prints frame statistics.

mod surfaces;

// (the shared crate's modules, by the names they had in one crate)
use tessera_core::{egl, seed, services, shell, theme, wayland, Msg};
// (the loop's messages and the fonts: core's, shared with the greeter's own loop)
use tessera_core::runloop::{handle, register_fonts};

use calloop::signals::{Signal, Signals};
use calloop::{generic::Generic, EventLoop, Interest, Mode, PostAction};
use smithay_client_toolkit::reexports::calloop_wayland_source::WaylandSource;
use std::time::{Duration, Instant};
use wayland::State;

/// A line from the shell on stdin (RustUi.tell): `{"to": "<surface>", …}`
/// reaches that part's `message` as the serde_json::Value
fn shell_said(state: &mut State, line: &[u8]) {
    let Ok(v) = serde_json::from_slice::<serde_json::Value>(line) else { return };
    let Some(to) = v["to"].as_str().and_then(|t| surfaces::KNOWN.iter().find(|k| **k == t)) else { return };
    let mut v = Some(v);
    state.part(to, |p, st| p.message(st, Box::new(v.take().unwrap())));
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
    // The login screen (greetd's greeter): a mode of its own, before anything
    // else reads the arguments (surfaces/greeter)
    if tessera_greeter::wanted(&args) {
        return tessera_greeter::run(&args, started);
    }
    // An argument it doesn't know (a test harness's `--list`, a typo) ends
    // it here, before it opens a display: started by mistake, it never draws
    // on whatever session it was started from
    if let Some(bad) = surfaces::unknown_arg(&args) {
        eprintln!("tessera-ui: unknown argument {bad} (tessera-ui [--surfaces a,b] | --material-roles SEED dark|light [style] | --seed PICTURE | --version)");
        std::process::exit(2);
    }
    let surfaces = surfaces::from_args(&args);
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
            Signal::SIGUSR1 => state.parts.iter().for_each(|p| p.stats()),
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

    // The shell's commands, a JSON line each (src/shell.rs), and its state
    // for the surfaces (services::bridge); ends when stdin (a pipe from
    // whoever started us) closes
    let stdin_is_pipe = unsafe {
        let mut st: libc::stat = std::mem::zeroed();
        libc::fstat(0, &mut st) == 0 && matches!(st.st_mode & libc::S_IFMT, libc::S_IFIFO | libc::S_IFSOCK)
    };
    if stdin_is_pipe {
        let src = Generic::new(unsafe { calloop::generic::FdWrapper::new(0) }, Interest::READ, Mode::Level);
        let mut lines = shell::Lines::default();
        lh
            .insert_source(src, move |_, _, state: &mut State| {
                let mut buf = [0u8; 4096];
                let n = unsafe { libc::read(0, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
                if n <= 0 {
                    state.exit = true;
                    return Ok(PostAction::Remove);
                }
                slint::platform::update_timers_and_animations();
                for l in lines.feed(&buf[..n as usize]) {
                    match shell::Command::parse(&l) {
                        Some(c) => state.command(c),
                        None => match services::bridge::parse_line(&l) {
                            // (the shell's state for the menu bar: services::bridge)
                            Some(v) => handle(state, Msg::Data(v)),
                            None => shell_said(state, l.as_bytes()),
                        },
                    }
                }
                Ok(PostAction::Continue)
            })
            .unwrap();
    }

    let egl = egl::Egl::new(conn.backend().display_ptr() as *mut _).expect("EGL");
    slint::platform::set_platform(Box::new(wayland::Platform { tx: tx.clone() })).expect("platform");
    register_fonts();

    let glass = services::hypr::Glass::spawn(tx.clone());
    let parts: Vec<Box<dyn shell::Part>> = surfaces.on.iter().filter_map(|n| surfaces::make(n)).collect();
    let shell = shell::Shell::new(tx.clone(), glass, surfaces);
    let mut state = State::new(&globals, qh, egl, lh.clone(), tx.clone(), shell, parts, started);
    watch_config(tx.clone());
    state.each_part(|p, st| p.start(st));
    // (parts that show nothing until something happens, as the OSD: no first
    // frame to wait for, they draw from now; the shell steps aside at once)
    if state.surfaces.is_empty() {
        surfaces::announce(&state.shell.surfaces);
    }

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
        state.each_part(|p, st| p.after_frame(st));
    }
    state.each_part(|p, st| p.quit(st));
    eprintln!("tessera-ui: bye");
}

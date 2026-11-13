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
//!   tessera-ui --update-screen [-p] DIR   the update screen update-run.sh shows (surfaces/updatescreen)
//!   tessera-ui --look-cover [-p] DIR      the cover switch-look.sh shows (surfaces/lookcover)
//!                                (either with --probe: exit 0, nothing drawn: "this one has it")
//!   tessera-ui … --demo          the lock, session and polkit surfaces without their real
//!                                roles (no session lock, no PAM, no polkit agent: their top comments)
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
fn shell_said(state: &mut State, v: serde_json::Value) {
    let Some(to) = v["to"].as_str().and_then(|t| surfaces::KNOWN.iter().find(|k| **k == t)) else { return };
    let mut v = Some(v);
    state.part(to, |p, st| p.message(st, Box::new(v.take().unwrap())));
}

/// One line from the shell (stdin), or from the host standalone (Msg::Line):
/// a command, the shell's state for the surfaces, or a message for one part
fn shell_line(state: &mut State, l: &str) {
    // (read once: a command, the shell's state, or a part's message)
    let Ok(v) = serde_json::from_str::<serde_json::Value>(l.trim()) else { return };
    match shell::Command::from_value(v) {
        Ok(c) => state.command(c),
        Err(v) => match services::bridge::from_value(v) {
            // (the shell's state for the menu bar: services::bridge)
            Ok(d) => handle(state, Msg::Data(d)),
            Err(v) => shell_said(state, v),
        },
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

/// Every host service (wave 4: the QML shell's services, in Rust), from
/// the host crates (native/tessera-ui/host/*)
fn host_services() -> Vec<Box<dyn tessera_core::host::Service>> {
    let mut v = tessera_host_shell::services();
    v.extend(tessera_host_system::services());
    v.extend(tessera_host_desktop::services());
    v.extend(tessera_host_sharing::services());
    v
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
    // (the lock's PAM conversations run in a child of their own: surfaces/lock/auth.rs)
    if args.get(1).map(String::as_str) == Some("--pam-helper") {
        std::process::exit(tessera_lock::auth::helper_main(&args));
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
    // The update screen and the look-switch cover: full-screen covers in
    // processes of their own, which outlive the shell they replace
    if tessera_updatescreen::wanted(&args) {
        return tessera_updatescreen::run(&args, started);
    }
    if tessera_lookcover::wanted(&args) {
        return tessera_lookcover::run(&args, started);
    }
    // `tessera-ui ipc call <target> <function> [args…]`: the standalone
    // shell's `qs ipc call` (keybinds, scripts); prints the answer
    if args.get(1).map(String::as_str) == Some("ipc") {
        std::process::exit(tessera_host_shell::ipc::client(&args[2..]));
    }
    // --standalone: no QML shell; every surface, and the host in its place
    let standalone = args.iter().any(|a| a == "--standalone");
    let args: Vec<String> = args.into_iter().filter(|a| a != "--standalone").collect();
    // An argument it doesn't know (a test harness's `--list`, a typo) ends
    // it here, before it opens a display: started by mistake, it never draws
    // on whatever session it was started from
    if let Some(bad) = surfaces::unknown_arg(&args) {
        eprintln!("tessera-ui: unknown argument {bad} (tessera-ui [--surfaces a,b] [--demo] | --material-roles SEED dark|light [style] | --seed PICTURE | --version)");
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
            Signal::SIGUSR1 => {
                state.parts.iter().for_each(|p| p.stats());
                let (reloads, stalls) = tessera_core::perf::counts();
                eprintln!("{{\"reloads\":{reloads},\"stalls\":{stalls}}}");
            }
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
                match m {
                    Msg::Line(l) => tessera_core::perf::time(|| format!("line {}", l.chars().take(60).collect::<String>()), || shell_line(state, &l)),
                    m => handle(state, m),
                }
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
    // (standalone: stdin is whatever started it, not the QML's pipe; its end
    // means nothing, so it isn't read: the sandbox's reload closed it at once,
    // and the shell took that for "the QML quit" and left)
    if stdin_is_pipe && !standalone {
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
                    shell_line(state, &l);
                }
                Ok(PostAction::Continue)
            })
            .unwrap();
    }

    let egl = egl::Egl::new(conn.backend().display_ptr() as *mut _).expect("EGL");
    slint::platform::set_platform(Box::new(wayland::Platform { tx: tx.clone() })).expect("platform");
    register_fonts();

    let glass = services::hypr::Glass::spawn(tx.clone());
    let mut parts: Vec<Box<dyn shell::Part>> = surfaces.on.iter().filter_map(|n| surfaces::make(n)).collect();
    if standalone {
        tessera_core::host::set_standalone(tx.clone());
        // (launch-shell.sh counts standalone starts and falls back to the QML
        // shell after three that didn't last: up 30 s, this one counts as good)
        std::thread::spawn(|| {
            std::thread::sleep(std::time::Duration::from_secs(30));
            let state = std::env::var_os("XDG_STATE_HOME")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/state"));
            let _ = std::fs::remove_file(state.join("tessera/standalone-starts"));
        });
        // (first: its services are up before any surface asks them)
        parts.insert(0, Box::new(tessera_core::host::HostPart::new(host_services())));
    }
    let shell = shell::Shell::new(tx.clone(), glass, surfaces);
    let mut state = State::new(&globals, qh, egl, lh.clone(), tx.clone(), shell, parts, started);
    // (this loop drains held-back readings: runloop::drain_deferred below)
    state.shell.defer_data = true;
    watch_config(tx.clone());
    state.each_part(|p, st| {
        // (a slow start holds up the first frame: say which, and how long)
        let t = std::time::Instant::now();
        p.start(st);
        let ms = t.elapsed().as_millis();
        if ms >= 25 {
            eprintln!("tessera-ui: slow start: {} {ms} ms", p.name());
        }
    });
    eprintln!("tessera-ui: started every part {} ms after main", started.elapsed().as_millis());
    // (parts that show nothing until something happens, as the OSD: no first
    // frame to wait for, they draw from now; the shell steps aside at once)
    if state.surfaces.is_empty() {
        surfaces::announce(&state.shell.surfaces);
    }

    // (the windows parts keep while closed, made ahead: Part::prewarm, one
    // part per turn of the loop, from 2 s after the first frame)
    let mut prewarm: Option<usize> = Some(0);
    let mut first_frame_at: Option<Instant> = None;
    while !state.exit {
        if first_frame_at.is_none() && state.first_frame_logged {
            first_frame_at = Some(Instant::now());
        }
        let prewarm_due = prewarm.is_some() && first_frame_at.is_some_and(|t| t.elapsed() >= Duration::from_secs(2));
        let timeout = if state.wants_frame_soon() || !state.shell.deferred.is_empty() {
            Some(Duration::ZERO)
        } else if state.animating_unpaced() {
            Some(Duration::from_millis(16))
        } else if state.shell.glass.has_held() {
            // (glass shapes waiting for their frame: they go at the latest
            // after Glass's MAX_HOLD, so wake for it)
            Some(slint::platform::duration_until_next_timer_update().map_or(Duration::from_millis(16), |d| d.min(Duration::from_millis(16))))
        } else if state.waits_on_frame() {
            // (a frame callback that never comes is taken as lost after a
            // while: wayland.rs's LOST_FRAME; wake for it)
            Some(slint::platform::duration_until_next_timer_update().map_or(Duration::from_millis(50), |d| d.min(Duration::from_millis(50))))
        } else if prewarm.is_some() && first_frame_at.is_some() {
            // (wake for the next prewarm even when nothing else happens)
            Some(slint::platform::duration_until_next_timer_update().map_or(Duration::from_millis(100), |d| d.min(Duration::from_millis(100))))
        } else {
            slint::platform::duration_until_next_timer_update()
        };
        if let Err(e) = event_loop.dispatch(timeout, &mut state) {
            eprintln!("tessera-ui: event loop: {e}");
            break;
        }
        tessera_core::perf::time(|| "slint timers".into(), slint::platform::update_timers_and_animations);
        tessera_core::perf::time(|| "render".into(), || state.render_all());
        tessera_core::perf::time(|| "after_frame".into(), || state.each_part(|p, st| p.after_frame(st)));
        // (the glass shapes parts set go out with the frames they belong to)
        state.flush_glass();
        // (readings held back while a window animated: a few each turn, ~4 ms)
        tessera_core::runloop::drain_deferred(&mut state, Duration::from_millis(4));
        // (only on a quiet turn: nothing animating, no frame waiting)
        if prewarm_due && !state.wants_frame_soon() && !state.surfaces.iter().any(|s| s.window().has_active_animations()) {
            let i = prewarm.unwrap();
            prewarm = state.prewarm_part(i).then_some(i + 1);
        }
    }
    state.each_part(|p, st| p.quit(st));
    // (an orderly exit — a look switch, a restart asked for — isn't a failed
    // start: launch-shell.sh's count of them fell back to the QML shell after
    // three look switches within a minute)
    {
        let state = std::env::var_os("XDG_STATE_HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/state"));
        let _ = std::fs::remove_file(state.join("tessera/standalone-starts"));
    }
    eprintln!("tessera-ui: bye");
}

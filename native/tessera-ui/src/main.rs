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
//!   tessera-ui --version [--json]   which build: (channel, commit, date, variant, [profile,] signed release | built locally)
//!   tessera-ui --greeter [--style …] [--demo [STATE]]   the login screen (surfaces/greeter)
//!   tessera-ui --update-screen [-p] DIR   the update screen update-run.sh shows (surfaces/updatescreen)
//!   tessera-ui --look-cover [-p] DIR      the cover switch-look.sh shows (surfaces/lookcover)
//!                                (either with --probe: exit 0, nothing drawn: "this one has it")
//!   tessera-ui … --demo          the lock, session and polkit surfaces without their real
//!                                roles (no session lock, no PAM, no polkit agent: their top comments)
//! SIGUSR1 prints frame statistics.

mod surfaces;

// (the shared crate's modules, by the names they had in one crate)
use tessera_core::{egl, pacing, seed, services, shell, theme, wayland, Msg};
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

/// Watch config.json (the shell writes it whole, then renames or closes it).
/// Its folder is made first (a first start has none, and a folder that isn't
/// there can't be watched: no setting would ever apply live); a config.json
/// that's a link (dotfiles) is watched where it points too. A folder
/// replaced (moved away, restored from dotfiles) is watched again, and the
/// settings read again then (audit #50: the watch stayed on the old folder
/// and no change applied until a restart)
fn watch_config(tx: calloop::channel::Sender<Msg>) {
    let path = theme::config_path();
    if path.parent().is_none() {
        return;
    }
    std::thread::spawn(move || {
        let mut first = true;
        let mut said = false;
        loop {
            let Some(dir) = path.parent().map(|d| d.to_owned()) else { return };
            let _ = std::fs::create_dir_all(&dir);
            let target = tessera_core::util::link_target(&path);
            let names: Vec<Vec<u8>> = [&path, &target].iter().filter_map(|p| p.file_name()).map(|n| n.as_encoded_bytes().to_vec()).collect();
            let mut dirs = vec![dir];
            if let Some(t) = target.parent().filter(|t| *t != dirs[0]) {
                dirs.push(t.to_owned());
            }
            // SAFETY: plain inotify calls on an fd this loop owns and closes
            let fd = unsafe { libc::inotify_init1(libc::IN_CLOEXEC) };
            if fd < 0 {
                return;
            }
            let mut watching = 0;
            for d in &dirs {
                let Ok(c) = std::ffi::CString::new(d.as_os_str().as_encoded_bytes()) else { continue };
                if unsafe { libc::inotify_add_watch(fd, c.as_ptr(), libc::IN_CLOSE_WRITE | libc::IN_MOVED_TO | libc::IN_DELETE_SELF | libc::IN_MOVE_SELF) } >= 0 {
                    watching += 1;
                }
            }
            if watching == 0 {
                if !said {
                    eprintln!("tessera-ui: can't watch {}: trying again; settings changes apply once it's watched", dirs[0].display());
                    said = true;
                }
                unsafe { libc::close(fd) };
                std::thread::sleep(Duration::from_secs(5));
                continue;
            }
            // (watched again after the folder was replaced: what's there now applies)
            if !first && tx.send(Msg::ConfigChanged).is_err() {
                unsafe { libc::close(fd) };
                return;
            }
            first = false;
            let mut buf = [0u8; 4096];
            loop {
                let n = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
                if n <= 0 {
                    if n < 0 && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
                        continue;
                    }
                    break;
                }
                let (mut hit, mut gone) = (false, false);
                for (mask, name) in tessera_core::util::inotify_events(&buf[..n as usize]) {
                    hit |= names.iter().any(|n| n.as_slice() == name);
                    gone |= mask & (libc::IN_DELETE_SELF | libc::IN_MOVE_SELF | libc::IN_IGNORED) != 0;
                }
                if hit && tx.send(Msg::ConfigChanged).is_err() {
                    unsafe { libc::close(fd) };
                    return;
                }
                if gone {
                    break;
                }
            }
            unsafe { libc::close(fd) };
            // (a moment for whatever replaces the folder to finish)
            std::thread::sleep(Duration::from_millis(200));
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

/// A part, made again after it panicked (tessera_core::contain): a surface,
/// or the host
fn make_part(name: &str) -> Option<Box<dyn shell::Part>> {
    if name == tessera_core::host::NAME {
        return Some(Box::new(tessera_core::host::HostPart::new(host_services())));
    }
    surfaces::make(name)
}

/// What build.rs worked out (core/src/build_info.rs)
fn build_info() -> tessera_core::build_info::BuildInfo {
    tessera_core::build_info::BuildInfo {
        version: env!("CARGO_PKG_VERSION"),
        commit: env!("TESSERA_BI_COMMIT"),
        date: env!("TESSERA_BI_DATE"),
        dirty: !env!("TESSERA_BI_DIRTY").is_empty(),
        branch: env!("TESSERA_BI_BRANCH"),
        channel: env!("TESSERA_BI_CHANNEL"),
        variant: env!("TESSERA_BI_VARIANT"),
        kind: env!("TESSERA_BI_KIND"),
        profile: env!("TESSERA_BI_PROFILE"),
    }
}

fn main() {
    let started = Instant::now();
    // (malloc: two arenas, a fixed mmap threshold; before any thread: tessera_core::memory)
    tessera_core::memory::tune_allocator();
    // (the environment as given, before anything here changes it: a restart
    // on another GPU runs with exactly this; core/src/gpu.rs)
    tessera_core::gpu::remember_env();
    // (an XDG folder set but empty or relative means "the default" by the
    // spec: taken out before anything reads it, so Mesa's shader cache, the
    // shell's children and every crate here use ~/.cache and the rest, not
    // "/mesa_shader_cache" or a path from wherever they started)
    for v in ["XDG_CACHE_HOME", "XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_STATE_HOME", "XDG_DATA_DIRS", "XDG_CONFIG_DIRS"] {
        if std::env::var_os(v).is_some_and(|p| !std::path::Path::new(&p).is_absolute()) {
            std::env::remove_var(v);
        }
    }
    // Which build this is (build.rs): --version, Settings → About, Software Update
    tessera_core::build_info::set(build_info());
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
    // (whether a GPU other than the desktop's can draw for it: core/src/gpu.rs probe)
    if args.get(1).map(String::as_str) == Some("--gpu-probe") {
        std::process::exit(tessera_core::gpu::probe_main(args.get(2)));
    }
    // (pam_exec's program in tessera-face-rgb: the blink check, core/face.rs)
    if args.get(1).map(String::as_str) == Some("--face-live") {
        std::process::exit(tessera_core::face::live_main());
    }
    if args.get(1).map(String::as_str) == Some("--version") {
        let b = tessera_core::build_info::current();
        if args.iter().skip(2).any(|a| a == "--json") {
            println!("{}", b.json());
        } else {
            println!("{}", b.line());
        }
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
    // The settings, when this is the whole shell (standalone: it owns them):
    // none yet, a first file is made; a broken one is kept aside, the person
    // told, a fresh one written (tessera_core::cfgfile)
    if standalone {
        let said = tessera_core::cfgfile::check_at_start();
        if said != "fine" {
            eprintln!("tessera-ui: settings: {said}");
        }
        // (where its scripts, fonts and pictures come from: tessera_core::env::root)
        eprintln!("tessera-ui: files from {}", tessera_core::env::root().display());
    }
    let surfaces = surfaces::from_args(&args);
    if surfaces.on.is_empty() {
        // (nothing it can draw: say so, and leave the shell drawing them)
        surfaces::announce(&surfaces);
        return;
    }
    // (a panic in one part is caught, the part made again; crash reports:
    // tessera_core::contain, crash)
    tessera_core::contain::install(make_part);

    // (this thread is the UI thread: blocking work goes elsewhere, tessera_core::work)
    tessera_core::work::mark_ui_thread();
    let mut event_loop: EventLoop<'static, State> = EventLoop::try_new().expect("event loop");
    let lh = event_loop.handle();
    // Signals first: the threads started below inherit the blocked mask, so
    // SIGTERM always reaches this loop (and the bar ends cleanly)
    let signals = Signals::new(&[Signal::SIGTERM, Signal::SIGINT, Signal::SIGUSR1, Signal::SIGUSR2]).expect("signalfd");
    lh
        .insert_source(signals, |ev, _, state: &mut State| match ev.signal() {
            // (TESSERA_CRASH_TEST only: a panic in a part, contain.rs)
            Signal::SIGUSR2 => tessera_core::contain::crash_test_signal(),
            Signal::SIGUSR1 => {
                state.parts.iter().for_each(|p| p.stats());
                let (reloads, stalls) = tessera_core::perf::counts();
                eprintln!("{{\"reloads\":{reloads},\"stalls\":{stalls}}}");
                eprintln!("tessera-ui: {}", tessera_core::pacing::report());
                let (used, held) = tessera_core::memory::malloc_use();
                eprintln!("tessera-ui: malloc in use {:.1} of {:.1} MB, RssAnon {:.1} MB", used as f64 / 1048576.0, held as f64 / 1048576.0, tessera_core::memory::rss_anon_kb() as f64 / 1024.0);
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

    // (the low-power GPU, normally the compositor's too: not NVIDIA's beside an Intel one
    // driving the screen, whose frames it then copied across; core/src/gpu.rs)
    let vendor_set = tessera_core::gpu::prefer_gpu(&conn);
    let egl = match egl::Egl::new(conn.backend().display_ptr() as *mut _) {
        Ok(egl) => egl,
        // (a GPU other than the desktop's that can't draw for it: started
        // again, once, on the desktop's, with the environment as it was
        // given and the choice set back on disk; core/src/gpu.rs fall_back)
        Err(e) => match tessera_core::gpu::fall_back() {
            Some(env) => {
                eprintln!("tessera-ui: EGL didn't start ({e})");
                use std::os::unix::process::CommandExt;
                // (as it was started, argv[0] and all: a launcher's name for it stays)
                let mut args = std::env::args_os();
                let argv0 = args.next().unwrap_or_default();
                let exe = if std::path::Path::new(&argv0).components().count() > 1 { argv0.into() } else { std::env::current_exe().unwrap_or_else(|_| argv0.into()) };
                // (the signals blocked for this loop's signalfd would stay
                // blocked in the new process: unblocked first)
                unblock_signals();
                let err = std::process::Command::new(exe).args(args).env_clear().envs(env).exec();
                no_gl(&format!("couldn't start again: {err}"), standalone)
            }
            None => no_gl(&e.to_string(), standalone),
        },
    };
    tessera_core::gpu::restore(vendor_set);
    slint::platform::set_platform(Box::new(wayland::Platform { tx: tx.clone() })).expect("platform");
    register_fonts();

    let glass = services::hypr::Glass::spawn(tx.clone());
    let mut parts: Vec<Box<dyn shell::Part>> = surfaces.on.iter().filter_map(|n| surfaces::make(n)).collect();
    if standalone {
        tessera_core::host::set_standalone(tx.clone());
        // (launch-shell.sh counts standalone starts: a crash is retried once,
        // a second start that didn't last is the QML fallback. Up a minute,
        // this one counts as good: the count and the fallback's note go.
        // 18 Nov: 5 minutes let restarts and reboots inside that window add up)
        std::thread::spawn(|| {
            std::thread::sleep(std::time::Duration::from_secs(60));
            let state = tessera_core::util::xdg("XDG_STATE_HOME", ".local/state");
            let _ = std::fs::remove_file(state.join("tessera/standalone-starts"));
            let _ = std::fs::remove_file(state.join("tessera/fallback"));
            // (an update's new shell stayed up: no going back to the
            // previous build for it any more, launch-shell.sh's rollback_ui)
            let _ = std::fs::remove_file(state.join("tessera/fresh-update.json"));
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

    // (the windows parts keep while closed, made ahead: Part::prewarm, from
    // the first quiet turn after the start's intro (PREWARM_AFTER the first
    // frame at the soonest), as many parts per turn as fit PREWARM_BUDGET.
    // It was one part per turn from 2 s on: the made-ahead windows, the
    // emoji picker's unseen frame among them, kept the compositor drawing
    // until ~4 s after the bar, the desktop "settling" that late)
    const PREWARM_AFTER: Duration = Duration::from_millis(500);
    const PREWARM_BUDGET: Duration = Duration::from_millis(8);
    let mut prewarm: Option<usize> = Some(0);
    let mut first_frame_at: Option<Instant> = None;
    let mut wakes = tessera_core::wayland::Wakes::default();
    // (the health record, Settings → Health: the loop's turns timed from
    // the poll's return, a sample each 5 minutes; the whole shell only)
    if standalone {
        let _ = lh.insert_source(tessera_core::health::Woke, |_, _, _| {});
        tessera_core::health::start();
    }
    // (freed memory given back once the shell is quiet after drawing: memory.rs)
    let mut idle_trim = tessera_core::memory::IdleTrim::new(Instant::now());
    while !state.exit {
        if first_frame_at.is_none() && state.first_frame_logged {
            first_frame_at = Some(Instant::now());
        }
        let prewarm_due = prewarm.is_some() && first_frame_at.is_some_and(|t| t.elapsed() >= PREWARM_AFTER);
        // How long the loop may sleep: until the first thing due. Frames are
        // paced by the compositor's frame callbacks, which wake the loop as
        // events; the deadlines below are only for what no event brings
        // (core/src/pacing.rs).
        let (why, timeout) = if state.wants_frame_soon() || !state.shell.deferred.is_empty() {
            (if state.shell.deferred.is_empty() { "frame" } else { "deferred" }, Some(Duration::ZERO))
        } else {
            let now = Instant::now();
            let mut next: Option<(Instant, &'static str)> = None;
            let mut due = |at: Option<Instant>, why: &'static str| {
                if let Some(at) = at {
                    if next.is_none_or(|(n, _)| at < n) {
                        next = Some((at, why));
                    }
                }
            };
            // (Slint's timers; with nothing moving, on the idle grid: every
            // clock and poll due within the same 250 ms wakes the loop once)
            let quiet = pacing::quiet(now) && !state.surfaces.iter().any(|s| s.window().has_active_animations());
            due(slint::platform::duration_until_next_timer_update().map(|d| now + if quiet { pacing::coalesce(d) } else { d }), "timers");
            // (an animation no frame callback will wake: at the display's refresh)
            due(state.animating_unpaced().then(|| now + pacing::frame_period()), "animating");
            // (battery saver: a surface held back to 60 frames a second draws when it may)
            due(state.held_frame_deadline(), "saver");
            // (what a part animates by hand: each frame, or by the refresh when no frame comes)
            due(pacing::frame_timer_deadline(state.surfaces.iter().any(|s| s.frame_pending())), "frame-timers");
            // (glass shapes waiting for their frame go at the latest after Glass's MAX_HOLD)
            due(state.shell.glass.held_deadline(), "glass");
            // (a frame callback that never comes is taken as lost: wayland.rs's LOST_FRAME)
            due(state.lost_frame_deadline(), "frame-wait");
            // (the windows made ahead: from PREWARM_AFTER the first frame, on quiet turns)
            due(first_frame_at.filter(|_| prewarm.is_some()).map(|t| (t + PREWARM_AFTER).max(now + Duration::from_millis(50))), "prewarm");
            // (memory given back after a quiet spell)
            due(idle_trim.deadline(), "trim");
            match next {
                Some((at, why)) => (why, Some(at.saturating_duration_since(now))),
                None => ("idle", None),
            }
        };
        // (a part that panicked is taken down at once, one waiting is made again on time)
        let timeout = tessera_core::contain::wake_by(timeout);
        wakes.note(why, &state);
        if let Err(e) = event_loop.dispatch(timeout, &mut state) {
            eprintln!("tessera-ui: event loop: {e}");
            break;
        }
        // (what this turn's events sent on — a panel closing tells Panels,
        // Panels says the shell's state — handled before the frame is drawn,
        // as the QML's bindings follow at once, not a frame or two later:
        // the menu bar's highlight ran ~35 ms past a panel's close. At most
        // two quick passes, only on a turn that draws.)
        for _ in 0..2 {
            if !state.wants_frame_soon() {
                break;
            }
            if let Err(e) = event_loop.dispatch(Some(Duration::ZERO), &mut state) {
                eprintln!("tessera-ui: event loop: {e}");
                break;
            }
        }
        state.tend_parts();
        tessera_core::perf::time(|| "slint timers".into(), slint::platform::update_timers_and_animations);
        // (what parts animate by hand moves on once per frame, before the frame is drawn)
        tessera_core::perf::time(|| "frame timers".into(), pacing::run_frame_timers);
        tessera_core::perf::time(|| "render".into(), || state.render_all());
        let now = Instant::now();
        idle_trim.turn(now, state.drew_this_turn());
        if idle_trim.due(now) {
            tessera_core::memory::trim_in_background();
        }
        tessera_core::perf::time(|| "after_frame".into(), || state.each_part(|p, st| {
            let n = p.name();
            tessera_core::perf::time(|| format!("after_frame {n}"), || p.after_frame(st))
        }));
        // (the glass shapes parts set go out with the frames they belong to)
        state.flush_glass();
        // (readings held back while a window animated: a few each turn, ~4 ms)
        tessera_core::runloop::drain_deferred(&mut state, Duration::from_millis(4));
        // (only on a quiet turn: nothing animating, no frame waiting)
        if prewarm_due && !state.wants_frame_soon() && !state.surfaces.iter().any(|s| s.window().has_active_animations()) {
            // (most parts make nothing ahead, or a window in a few ms: several
            // per turn; one that took long, or a frame now wanted, ends the turn)
            let t = Instant::now();
            while let Some(i) = prewarm {
                prewarm = state.prewarm_part(i).then_some(i + 1);
                if t.elapsed() >= PREWARM_BUDGET || state.wants_frame_soon() {
                    break;
                }
            }
        }
        tessera_core::health::turn_end();
    }
    tessera_core::work::set_leaving();
    state.each_part(|p, st| p.quit(st));
    tessera_core::health::stop();
    // (settings and state queued from the loop: written before we go; and
    // Hyprland's dispatches asked last)
    tessera_core::cfgfile::flush();
    tessera_core::services::hypr::IN_ORDER.flush(Duration::from_secs(2));
    // (an orderly exit — a look switch, a restart asked for — isn't a failed
    // start: launch-shell.sh's count of them fell back to the QML shell after
    // three look switches within a minute)
    {
        let state = tessera_core::util::xdg("XDG_STATE_HOME", ".local/state");
        let _ = std::fs::remove_file(state.join("tessera/standalone-starts"));
    }
    eprintln!("tessera-ui: bye");
}

/// EGL didn't start (no GPU driver's EGL, a broken install, a driver that
/// refuses Wayland): this shell can't draw. Never no shell at all: started
/// standalone (launch-shell.sh exec'd it, TESSERA_QML_FALLBACK names the QML
/// shell's folder), this process becomes the QML shell, and the start count
/// is set so the next starts are QML too until tessera-ui stays up again
/// (launch-shell.sh's safety net). Beside the QML shell: it just ends, and
/// the QML shell draws on.
fn no_gl(e: &str, standalone: bool) -> ! {
    eprintln!("tessera-ui: can't draw: EGL didn't start ({e})");
    if !standalone {
        std::process::exit(1);
    }
    let state = std::env::var_os("XDG_STATE_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/state"));
    let _ = std::fs::create_dir_all(state.join("tessera"));
    // (with this session's name: launch-shell.sh starts the count again on a new login)
    let _ = std::fs::write(state.join("tessera/standalone-starts"), format!("2\n{}\n", std::env::var("HYPRLAND_INSTANCE_SIGNATURE").unwrap_or_default()));
    // (the QML fallback shows why: modules/fallback)
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let _ = std::fs::write(state.join("tessera/fallback"), format!("The Rust shell can't draw on this computer's graphics (EGL: {})\n{now}\n{}\n", e.chars().take(120).collect::<String>(), std::env::var("HYPRLAND_INSTANCE_SIGNATURE").unwrap_or_default()));
    // (Settings → Health lists it, and the next Rust start counts as a
    // retry, as launch-shell.sh's own fallback writes it; audit #87)
    tessera_core::health::event_now("fallback", serde_json::json!({"why": format!("EGL: {}", e.chars().take(160).collect::<String>())}));
    let Some(dir) = std::env::var_os("TESSERA_QML_FALLBACK").map(std::path::PathBuf::from).filter(|d| d.join("assets/scripts/launch-shell.sh").is_file()) else {
        eprintln!("tessera-ui: no QML shell to fall back to (TESSERA_QML_FALLBACK); the next start is the QML shell");
        std::process::exit(1);
    };
    eprintln!("tessera-ui: starting the QML shell instead ({})", dir.display());
    // (the signals this process blocked for its signalfd would stay blocked
    // in the shell it becomes: unblocked first)
    unblock_signals();
    use std::os::unix::process::CommandExt;
    let err = std::process::Command::new(dir.join("assets/scripts/launch-shell.sh")).arg("-p").arg(&dir).env("TESSERA_STANDALONE", "0").env_remove("TESSERA_QML_FALLBACK").exec();
    eprintln!("tessera-ui: couldn't start the QML shell: {err}");
    std::process::exit(1);
}

/// Every signal unblocked on this thread (before exec: a new process keeps the mask)
fn unblock_signals() {
    unsafe {
        let mut none: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut none);
        libc::pthread_sigmask(libc::SIG_SETMASK, &none, std::ptr::null_mut());
    }
}

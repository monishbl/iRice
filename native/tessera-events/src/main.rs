//! tessera-events: one process that follows the system for the shell, in
//! place of a monitor process (and a bash wrapper) per thing watched.
//!
//! The shell (services/Events.qml) starts exactly one. It writes one JSON
//! object per line on stdout, e.g.
//!     {"src":"login1","kind":"PrepareForSleep","args":[true]}
//! and reads commands on stdin, one per line:
//!     {"cmd":"watch","src":"udev","on":true,"args":{"subsystems":["usb"]}}
//!     {"cmd":"inhibit","what":"idle","why":"Caffeine","on":true}
//! Every source starts off and runs only while the shell asks for it. It ends
//! when stdin closes (the shell quit), and with the shell even if that
//! crashes (PR_SET_PDEATHSIG).

mod camera;
mod cmd;
mod dbus;
mod inotify;
mod json;
mod net;
mod procs;
mod pulse;
mod stop;
mod udev;

use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::BufRead;
use std::thread;

use stop::Stop;

pub const VERSION: u32 = 1;

const HELP: &str = "tessera-events: system events for the Tessera shell, as JSON lines

Usage: tessera-events          run (the shell starts it; commands on stdin)
       tessera-events --help   this text
       tessera-events --version

Sources (each off until a watch command turns it on):
  login1       logind's signals (PrepareForSleep, LidClosed changes)
  unity        app badges (com.canonical.Unity.LauncherEntry Update)
  nm           NetworkManager state changes
  net          the kernel's links and addresses (without NetworkManager)
  avahi        Google Cast devices on the network (_googlecast._tcp)
  location     apps using GeoClue
  udev         udev devices, with their properties (args: subsystems)
  pulse        PulseAudio / PipeWire-pulse subscribe events
  camera       apps holding a camera open
  clipboard    cliphist's database changing (args: path)
  copies       file copies in progress (cp, mv, rsync, dd...)
  screenshare  the screen-sharing portal's PipeWire streams
Commands:
  {\"cmd\":\"watch\",\"src\":SOURCE,\"on\":BOOL[,\"args\":{...}]}
  {\"cmd\":\"inhibit\",\"what\":\"idle\",\"why\":TEXT,\"on\":BOOL}   (a logind inhibitor, held while on)
Ends when stdin closes.
";

/// Writes one event line. A shell that's gone (a closed pipe) ends us.
pub fn emit(v: Value) {
    use std::io::Write;
    let mut line = v.to_string();
    line.push('\n');
    let out = std::io::stdout();
    let mut out = out.lock();
    if out.write_all(line.as_bytes()).and_then(|_| out.flush()).is_err() {
        std::process::exit(0);
    }
}

pub fn emit_error(src: &str, msg: &str) {
    emit(json!({"src": src, "kind": "error", "msg": msg}));
}

/// A source that runs on a thread of its own while it's on: turned off (or
/// given other args) it's told to stop, and a new one starts.
struct Threaded {
    run: fn(Stop, Value),
    running: Option<(Stop, Value)>,
}

impl Threaded {
    fn set(&mut self, on: bool, args: Value) {
        if let Some((stop, was)) = &self.running {
            if on && *was == args {
                return;
            }
            stop.set();
            self.running = None;
        }
        if on {
            let stop = Stop::new();
            let (s, a, run) = (stop.clone(), args.clone(), self.run);
            thread::spawn(move || run(s, a));
            self.running = Some((stop, args));
        }
    }
}

struct Hub {
    threaded: HashMap<&'static str, Threaded>,
}

impl Hub {
    fn new() -> Hub {
        let mut threaded = HashMap::new();
        let mut add = |name: &'static str, run: fn(Stop, Value)| {
            threaded.insert(name, Threaded { run, running: None });
        };
        add("udev", udev::run);
        add("net", net::run);
        add("pulse", pulse::run);
        add("camera", camera::run);
        add("clipboard", inotify::run_clipboard);
        add("copies", procs::run_copies);
        add("screenshare", procs::run_screenshare);
        Hub { threaded }
    }

    fn command(&mut self, line: &str) {
        match cmd::parse(line) {
            Ok(None) => {}
            Ok(Some(cmd::Command::Watch { src, on, args })) => {
                if let Some(t) = self.threaded.get_mut(src.as_str()) {
                    t.set(on, args);
                } else {
                    dbus::watch(&src, on);
                }
            }
            Ok(Some(cmd::Command::Inhibit { what, why, on })) => dbus::inhibit(&what, &why, on),
            Err(e) => emit_error("events", &e),
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print!("{HELP}");
        return;
    }
    if args.iter().any(|a| a == "--version") {
        println!("tessera-events {} (protocol {VERSION})", env!("CARGO_PKG_VERSION"));
        return;
    }
    if let Some(a) = args.first() {
        eprintln!("tessera-events: unknown argument {a} (see --help)");
        std::process::exit(2);
    }
    // Ends with the shell, however the shell ends
    unsafe {
        libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM as libc::c_ulong, 0, 0, 0);
    }
    emit(json!({"src": "events", "kind": "ready", "version": VERSION, "sources": cmd::SOURCES}));
    let mut hub = Hub::new();
    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        match line {
            Ok(l) => hub.command(&l),
            Err(_) => break,
        }
    }
    // stdin closed: the shell quit. Inhibitors go with the process.
    std::process::exit(0);
}

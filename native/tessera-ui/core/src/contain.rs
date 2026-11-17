//! A panic in one part never takes the whole shell down.
//!
//! Every way the loop goes into a part runs under `guard` (shell.rs's
//! `each_part` and `part`: messages, data, commands, outputs, after_frame,
//! prewarm…; wayland.rs's input: a Slint callback runs inside the window's
//! `dispatch_event`, so the pointer, the keyboard, touch, text input and
//! resizes reach a surface through `Surface::dispatch`, guarded for the part
//! that made it; each surface's frame, whose bindings call parts' pure
//! callbacks; host.rs runs each host service under its own name). A panic
//! there is caught (panic = "unwind"): logged with the part's name and a
//! backtrace, written as a crash report (crash.rs), and the part is marked
//! failed: nothing goes into it any more this turn. After the turn
//! (`State::tend_parts`, from the binary's loop) its windows are dropped, it
//! is dropped, and a new one is made (the binary's constructor, `install`)
//! after a short wait (1 s, then 3 s), started, given the outputs and every
//! service's latest reading. A part that fails 3 times in 2 minutes stays
//! off until the next login, and one notification says so (with "Send
//! report": Settings → Report a Bug, where the reports are).
//!
//! The lock is never left off, nor a locked session without its screen: the
//! failed lock hands its session lock over (`Part::hand_over`), the new lock
//! takes it at once in the same turn and shows on every output, still
//! locked. A lock that keeps failing (5 times in 2 minutes) ends the shell
//! instead: the compositor keeps the session locked, and launch-shell.sh
//! starts it again (or the QML shell).
//!
//! A host service (host.rs) can't be made again on its own: it stays off.
//! A panic outside every part (the loop itself) still ends the shell, after
//! the panic hook wrote its report; launch-shell.sh starts it again (or the
//! QML shell). A panic on another thread ends only that thread, with a report.
//!
//! Test it (only when TESSERA_CRASH_TEST is set at start): SIGUSR2 makes the
//! next call into the part named in $XDG_STATE_HOME/tessera/crash-test (else
//! TESSERA_CRASH_TEST's value) panic.

use crate::shell::Part;
use crate::wayland::State;
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Makes a part by name (the binary's: its surfaces, the host)
pub type Factory = fn(&str) -> Option<Box<dyn Part>>;

/// Failures within this long count together
const WINDOW: Duration = Duration::from_secs(120);
/// …and this many of them turn a part off
const LIMIT: usize = 3;
/// The lock is never turned off: this many end the shell instead
const LOCK_LIMIT: usize = 5;
const LOCK: &str = "lock";

static FACTORY: std::sync::OnceLock<Factory> = std::sync::OnceLock::new();
static MAIN: std::sync::OnceLock<std::thread::ThreadId> = std::sync::OnceLock::new();

thread_local! {
    /// containment is on (the shell's loop; not the greeter's, which ends
    /// on a panic as before so greetd starts it again)
    static ON: Cell<bool> = const { Cell::new(false) };
    /// the parts being called now, innermost last
    static STACK: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
    /// parts that panicked, waiting for `tend_parts`
    static FAILED: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
    static ANY_FAILED: Cell<bool> = const { Cell::new(false) };
    /// turned off until the next login
    static OFF: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
    /// when each part failed (the last 2 minutes)
    static HISTORY: RefCell<HashMap<&'static str, Vec<Instant>>> = RefCell::new(HashMap::new());
    /// parts to make again: (where it was in the list, its name, when)
    static DUE: RefCell<Vec<(usize, &'static str, Instant)>> = const { RefCell::new(Vec::new()) };
    /// what failed parts handed over for the new ones
    static HANDED: RefCell<HashMap<&'static str, Box<dyn Any>>> = RefCell::new(HashMap::new());
    /// the panic being caught, as the hook saw it
    static CAUGHT: RefCell<Option<crate::crash::Panic>> = const { RefCell::new(None) };
    /// the reports being written (a fatal end waits for them)
    static WRITING: RefCell<Vec<std::thread::JoinHandle<Option<std::path::PathBuf>>>> = const { RefCell::new(Vec::new()) };
    /// TESSERA_CRASH_TEST: the part whose next call panics
    static ARMED: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Containment on, for the shell's loop on this thread: `make` makes a
/// part again by its name. Also sets the panic hook (reports, the log)
pub fn install(make: Factory) {
    let _ = FACTORY.set(make);
    let _ = MAIN.set(std::thread::current().id());
    ON.with(|o| o.set(true));
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let message = match info.payload().downcast_ref::<&str>() {
            Some(s) => s.to_string(),
            None => info.payload().downcast_ref::<String>().cloned().unwrap_or_else(|| "(not a text)".into()),
        };
        let location = info.location().map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column())).unwrap_or_default();
        let backtrace = crate::crash::trim_backtrace(&std::backtrace::Backtrace::force_capture().to_string());
        let main = MAIN.get().is_some_and(|m| *m == std::thread::current().id());
        let part = if main { STACK.try_with(|s| s.borrow().last().copied()).ok().flatten() } else { None };
        let mut p = crate::crash::Panic { part: String::new(), message, location, backtrace, fatal: false };
        match part {
            Some(part) => {
                // (caught by `guard`: it writes the report; said once here, with the backtrace)
                eprintln!("tessera-ui: {part} panicked at {}: {}\n{}", p.location, p.message, p.backtrace.trim_end());
                p.part = part.to_string();
                let _ = CAUGHT.try_with(|c| *c.borrow_mut() = Some(p));
            }
            None => {
                p.fatal = main;
                p.part = if main { "shell".into() } else { format!("thread-{}", std::thread::current().name().unwrap_or("unnamed")) };
                if main {
                    eprintln!("tessera-ui: fatal: the loop panicked (not in a part): the shell ends");
                    // (written before the end: the reports already on their way first)
                    let _ = WRITING.try_with(|w| w.borrow_mut().drain(..).for_each(|h| drop(h.join())));
                }
                let path = crate::crash::write(&p);
                eprintln!("tessera-ui: crash report: {}", path.as_ref().map_or("(not written)".into(), |p| p.display().to_string()));
                // (the health record: the shell's own end, read by the next start)
                if main && crate::health::on() {
                    crate::health::event_now("crash", serde_json::json!({"part": p.part, "fatal": true, "report": report_name(path.as_deref())}));
                }
                default(info);
            }
        }
    }));
}

/// Is containment on (on this thread)?
pub fn on() -> bool {
    ON.with(Cell::get)
}

/// The part being called now (surfaces it makes are its own)
pub fn current() -> &'static str {
    STACK.with(|s| s.borrow().last().copied().unwrap_or(""))
}

fn is_failed(name: &str) -> bool {
    ANY_FAILED.with(Cell::get) && FAILED.with(|f| f.borrow().contains(&name))
}

/// Is this part off until the next login?
pub fn is_off(name: &str) -> bool {
    OFF.with(|o| o.borrow().contains(&name))
}

/// Run part code: None when it panicked (it is marked failed and its calls
/// skipped from now: `tend_parts` makes it again), or it failed already
pub fn guard<R>(name: &'static str, f: impl FnOnce() -> R) -> Option<R> {
    if !on() || name.is_empty() {
        return Some(f());
    }
    // (a host service that's off stays in the host's list: never called again)
    if is_failed(name) || OFF.with(|o| !o.borrow().is_empty() && o.borrow().contains(&name)) {
        return None;
    }
    STACK.with(|s| s.borrow_mut().push(name));
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        crash_test(name);
        f()
    }));
    STACK.with(|s| s.borrow_mut().pop());
    match r {
        Ok(v) => Some(v),
        Err(payload) => {
            failed(name, payload);
            None
        }
    }
}

fn failed(name: &'static str, payload: Box<dyn Any + Send>) {
    let mut p = CAUGHT.with(|c| c.borrow_mut().take()).unwrap_or_default();
    if p.message.is_empty() {
        p.message = payload.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| payload.downcast_ref::<String>().cloned()).unwrap_or_default();
    }
    p.part = name.to_string();
    eprintln!("tessera-ui: {name} failed (a panic): taken down after this turn");
    if !cfg!(test) {
        WRITING.with(|w| {
            let mut w = w.borrow_mut();
            w.retain(|h| !h.is_finished());
            w.push(std::thread::spawn(move || {
                let path = crate::crash::write(&p);
                crate::health::event("crash", serde_json::json!({"part": p.part, "report": report_name(path.as_deref())}));
                path
            }));
        });
    }
    // (dropping the panic's payload can panic too: not here)
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || drop(payload)));
    FAILED.with(|f| {
        let mut f = f.borrow_mut();
        if !f.contains(&name) {
            f.push(name);
        }
    });
    ANY_FAILED.with(|a| a.set(true));
    crate::health::part_crashed();
}

/// A report's file name for the health record ("" when none was written)
fn report_name(path: Option<&std::path::Path>) -> String {
    path.and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// What a failed part handed over (`Part::hand_over`), for the new one
pub fn take_handed<T: 'static>(name: &'static str) -> Option<Box<T>> {
    HANDED.with(|h| h.borrow_mut().remove(name)).and_then(|b| b.downcast::<T>().ok())
}

/// How long the loop may sleep: no longer than until a part is due again
/// (at once while one waits to be taken down)
pub fn wake_by(timeout: Option<Duration>) -> Option<Duration> {
    if !on() {
        return timeout;
    }
    if ANY_FAILED.with(Cell::get) {
        return Some(Duration::ZERO);
    }
    let due = DUE.with(|d| d.borrow().iter().map(|(_, _, t)| *t).min());
    match (timeout, due) {
        (t, None) => t,
        (None, Some(d)) => Some(d.saturating_duration_since(Instant::now())),
        (Some(t), Some(d)) => Some(t.min(d.saturating_duration_since(Instant::now()))),
    }
}

/// A part's name as the notification says it
pub fn display_name(name: &str) -> String {
    match name {
        "menubar" => "The menu bar".into(),
        "dock" => "The dock".into(),
        "desktop" => "The desktop".into(),
        "background" => "The wallpaper".into(),
        "controlcenter" => "Control Center".into(),
        "notifications" => "Notifications".into(),
        "spotlight" => "Spotlight".into(),
        "sidebar" => "The sidebar".into(),
        "osd" => "The volume and brightness display".into(),
        "settings" => "Settings".into(),
        "launchpad" => "Launchpad".into(),
        "missioncontrol" => "Mission Control".into(),
        "overview" => "Overview".into(),
        "switcher" => "The app switcher".into(),
        "clipboard" => "Clipboard history".into(),
        "emoji" => "The emoji picker".into(),
        "screentools" => "Screenshots".into(),
        "statusmenus" => "The menu bar's menus".into(),
        "network" => "The network menu".into(),
        n if n.chars().next().is_some_and(|c| c.is_ascii_uppercase()) => format!("Tessera's {n} service"),
        n => {
            let mut c = n.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
        }
    }
}

/// The one notification for a part turned off; "Send report" opens Settings
/// at Report a Bug, where the reports are
fn tell_off(name: &str) {
    let title = format!("{} stopped working", display_name(name));
    let body = "It was turned off until you log in again.".to_string();
    if !crate::util::on_path("notify-send") {
        return crate::util::notify("Tessera", "dialog-warning", "normal", &title, &body);
    }
    std::thread::spawn(move || {
        let out = std::process::Command::new("timeout")
            .args(["3600", "notify-send", "-a", "Tessera", "-i", "dialog-warning", "-A", "report=Send report", "--", &title, &body])
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output();
        if out.is_ok_and(|o| String::from_utf8_lossy(&o.stdout).trim() == "report") {
            {
                // (/proc/self/exe: still this binary after an update replaced the file)
                let _ = std::process::Command::new("/proc/self/exe").args(["ipc", "call", "panels", "openSettings", "bugreport"]).stdout(std::process::Stdio::null()).status();
            }
        }
    });
}

impl State {
    /// After a turn of the loop: take failed parts down, make the due ones again
    pub fn tend_parts(&mut self) {
        if !on() {
            return;
        }
        let now = Instant::now();
        if ANY_FAILED.with(Cell::get) {
            let failed = FAILED.with(|f| std::mem::take(&mut *f.borrow_mut()));
            ANY_FAILED.with(|a| a.set(false));
            for name in failed {
                self.take_down(name, now);
            }
        }
        let due: Vec<(usize, &'static str)> = DUE.with(|d| {
            let mut d = d.borrow_mut();
            let (now_due, later): (Vec<_>, Vec<_>) = d.drain(..).partition(|(_, _, t)| *t <= now);
            *d = later;
            now_due.into_iter().map(|(i, n, _)| (i, n)).collect()
        });
        for (at, name) in due {
            self.make_again(at, name);
        }
    }

    fn take_down(&mut self, name: &'static str, now: Instant) {
        let count = HISTORY.with(|h| {
            let mut h = h.borrow_mut();
            let v = h.entry(name).or_default();
            v.retain(|t| now.duration_since(*t) < WINDOW);
            v.push(now);
            v.len()
        });
        let Some(at) = self.parts.iter().position(|p| p.name() == name) else {
            // (a host service, host.rs: it can't be made again on its own)
            if !is_off(name) {
                OFF.with(|o| o.borrow_mut().push(name));
                eprintln!("tessera-ui: {name} is off until the next login");
                crate::health::event("off", serde_json::json!({"part": name}));
                tell_off(name);
            }
            return;
        };
        let mut part = self.parts.remove(at);
        // (what the new one carries on with: the lock's session lock)
        let handed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| part.hand_over()));
        let handed_ok = handed.is_ok();
        if let Ok(Some(h)) = handed {
            HANDED.with(|m| m.borrow_mut().insert(name, h));
        }
        // (its glass shapes, its PAM conversations…: what quit leaves nothing of)
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| part.quit(self)));
        let mine: Vec<_> = self.surfaces.iter().filter(|s| s.owner == name).cloned().collect();
        for s in &mine {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.drop_surface(s)));
        }
        drop(mine);
        if handed_ok {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || drop(part)));
        } else {
            // (it couldn't hand over: what it holds — a session lock — is
            // never let go of, as destroying a lock that's locked would end
            // the connection, and with it the whole shell)
            std::mem::forget(part);
        }
        if name == LOCK {
            if count >= LOCK_LIMIT {
                eprintln!("tessera-ui: the lock failed {count} times in 2 minutes: the shell ends (the session stays locked)");
                WRITING.with(|w| w.borrow_mut().drain(..).for_each(|h| drop(h.join())));
                if crate::health::on() {
                    crate::health::event_now("crash", serde_json::json!({"part": LOCK, "fatal": true, "report": ""}));
                }
                std::process::exit(1);
            }
            // (at once: a locked session is never without its screen)
            DUE.with(|d| d.borrow_mut().push((at, name, now)));
            return;
        }
        if count >= LIMIT {
            OFF.with(|o| o.borrow_mut().push(name));
            eprintln!("tessera-ui: {name} failed {count} times in 2 minutes: off until the next login");
            crate::health::event("off", serde_json::json!({"part": name}));
            tell_off(name);
            return;
        }
        let wait = if count <= 1 { Duration::from_secs(1) } else { Duration::from_secs(3) };
        eprintln!("tessera-ui: {name} starts again in {} s", wait.as_secs());
        DUE.with(|d| d.borrow_mut().push((at, name, now + wait)));
    }

    fn make_again(&mut self, at: usize, name: &'static str) {
        let Some(part) = FACTORY.get().and_then(|f| f(name)) else {
            eprintln!("tessera-ui: {name} can't be made again: off");
            OFF.with(|o| o.borrow_mut().push(name));
            return;
        };
        let at = at.min(self.parts.len());
        self.parts.insert(at, part);
        let t = Instant::now();
        // (as at the start: started, then every output, the glass plugin,
        // and every service's latest reading, as if each had just come)
        self.part(name, |p, st| p.start(st));
        for (o, n) in self.outputs_now() {
            self.part(name, |p, st| p.output_added(st, &o, &n));
        }
        if self.shell.plugin {
            self.part(name, |p, st| p.plugin_changed(st));
        }
        for what in self.shell.data_types() {
            self.part(name, |p, st| p.data(st, what));
        }
        HANDED.with(|m| m.borrow_mut().remove(name));
        if is_failed(name) {
            eprintln!("tessera-ui: {name} failed again as it started");
        } else {
            eprintln!("tessera-ui: {name} started again ({} ms)", t.elapsed().as_millis());
        }
    }
}

/// TESSERA_CRASH_TEST set at start: SIGUSR2 arms a panic in the next call
/// into a part ($XDG_STATE_HOME/tessera/crash-test names it, else the
/// variable's value)
pub fn crash_test_signal() {
    let Some(default) = crate::env::var("CRASH_TEST").filter(|v| !v.is_empty()) else { return };
    let file = crate::util::xdg("XDG_STATE_HOME", ".local/state").join("tessera/crash-test");
    let part = std::fs::read_to_string(file).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).unwrap_or(default);
    eprintln!("tessera-ui: crash test: the next call into {part} panics");
    ARMED.with(|a| *a.borrow_mut() = Some(part));
}

fn crash_test(name: &str) {
    if ARMED.with(|a| a.borrow().as_deref() == Some(name)) {
        ARMED.with(|a| *a.borrow_mut() = None);
        panic!("crash test (TESSERA_CRASH_TEST) in {name}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_for_people() {
        assert_eq!(display_name("dock"), "The dock");
        assert_eq!(display_name("Network"), "Tessera's Network service");
        assert_eq!(display_name("tiling"), "Tiling");
    }

    #[test]
    fn a_panic_is_caught_and_the_part_skipped_after() {
        ON.with(|o| o.set(true));
        let _ = MAIN.set(std::thread::current().id());
        assert_eq!(guard("a", || 1), Some(1));
        assert_eq!(guard("b", || -> i32 { panic!("boom") }), None);
        // (failed: skipped until taken down; others go on)
        let mut ran = false;
        assert_eq!(guard("b", || ran = true), None);
        assert!(!ran);
        assert_eq!(guard("a", || 2), Some(2));
        // (a nested part's panic is that part's)
        assert_eq!(guard("a", || guard("c", || -> i32 { panic!("inner") }).is_none()), Some(true));
        assert!(is_failed("c") && !is_failed("a"));
        assert_eq!(current(), "");
        // (the loop wakes at once to take them down)
        assert_eq!(wake_by(None), Some(Duration::ZERO));
        // (the crash test panics once, in the part named)
        ARMED.with(|a| *a.borrow_mut() = Some("a".into()));
        assert_eq!(guard("d", || 3), Some(3));
        assert_eq!(guard("a", || 4), None);
        FAILED.with(|f| f.borrow_mut().clear());
        ANY_FAILED.with(|a| a.set(false));
        assert_eq!(guard("a", || 5), Some(5));
        // (off — a host service — is never called again)
        OFF.with(|o| o.borrow_mut().push("Svc"));
        assert_eq!(guard("Svc", || 6), None);
        OFF.with(|o| o.borrow_mut().clear());
        ON.with(|o| o.set(false));
    }
}

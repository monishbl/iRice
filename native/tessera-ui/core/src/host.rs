//! The host: what the QML shell does for the Rust surfaces today, in Rust
//! (wave 4, docs/rust-ui-plan.md). Standalone (`tessera-ui --standalone`, or
//! TESSERA_STANDALONE=1) there is no QML shell: every surface is drawn here,
//! and the host's services take the QML's place on both ends of the seam the
//! surfaces already use:
//!
//!   Out (a surface asks): shell::ask(call, args), e.g. a Wi-Fi join, and
//!   services::bridge::call(&["panels", "toggle", …]) reach the service that
//!   answers that call (Service::calls / Service::ipc), not RustUi.qml.
//!
//!   In (the shell tells): a service says the same JSON lines RustUi.qml
//!   writes to stdin today (Host::say: `{"cmd":"open","surface":…}`,
//!   `{"to":"<surface>",…}`, `{"src":"shell","kind":"state",…}`…), and they
//!   are handled exactly as if they came on stdin (Msg::Line). So no surface
//!   changes: each one keeps its seam, and the host fills the other side.
//!
//! Beside the QML shell (not standalone) the host is never made: ask() and
//! bridge::call() go to the QML as before.
//!
//! The services live in the host crates (native/tessera-ui/host/*), one
//! module per QML service they replace; the binary collects them
//! (src/main.rs: tessera_host_*::services()).

use crate::wayland::State;
use crate::Msg;
use serde_json::{json, Value};
use std::any::Any;
use std::sync::OnceLock;

static STANDALONE: OnceLock<bool> = OnceLock::new();
static TX: OnceLock<std::sync::Mutex<calloop::channel::Sender<Msg>>> = OnceLock::new();

/// No QML shell: tessera-ui is the whole shell (set once, at start)
pub fn standalone() -> bool {
    // (=1 only: launch-shell.sh and tessera-ui's QML fallback set
    // TESSERA_STANDALONE=0 to mean "the QML shell", and the tessera-ui that
    // shell starts inherits it; any value counted, so that one routed its
    // calls to a host it doesn't have and took the standalone-only roles,
    // as the polkit agent beside the QML's own. 17 Nov)
    *STANDALONE.get_or_init(|| crate::env::var("STANDALONE").as_deref() == Some("1"))
}

/// This process is the whole shell (`--standalone`: the host is made), not
/// only one that inherited TESSERA_STANDALONE=1 (a tessera-ui run by hand
/// from a terminal the shell opened): the session-wide roles, such as the
/// polkit agent, are the whole shell's
pub fn whole_shell() -> bool {
    TX.get().is_some()
}

/// Called by the binary for `--standalone`, before anything asks
pub fn set_standalone(tx: calloop::channel::Sender<Msg>) {
    let _ = STANDALONE.set(true);
    let _ = TX.set(std::sync::Mutex::new(tx));
}

fn send(m: Msg) -> bool {
    TX.get().and_then(|t| t.lock().ok().map(|t| t.send(m).is_ok())).unwrap_or(false)
}

/// A surface's call (shell::ask), on its way to the service that answers it
pub struct Ask {
    pub call: String,
    pub args: Value,
}

/// A shell-ipc call (services::bridge::call, or `tessera-ui ipc call …`):
/// target, function, arguments, as `qs ipc call` takes them
pub struct Ipc {
    pub args: Vec<String>,
    /// where the answer goes (the IPC socket's client), if anyone waits
    pub reply: Option<std::sync::mpsc::Sender<String>>,
}

/// A message for one service (from its own threads: Host::sender)
pub struct ToService(pub &'static str, pub Box<dyn Any + Send>);

/// shell::ask's way in standalone; false when not standalone
pub fn route_ask(call: &str, args: &Value) -> bool {
    standalone() && send(Msg::Part(NAME, Box::new(Ask { call: call.into(), args: args.clone() })))
}

/// bridge::call's way in standalone; false when not standalone
pub fn route_ipc(args: &[String], reply: Option<std::sync::mpsc::Sender<String>>) -> bool {
    standalone() && send(Msg::Part(NAME, Box::new(Ipc { args: args.to_vec(), reply })))
}

pub const NAME: &str = "host";

/// What a service may do while it runs (borrowed for one call)
pub struct Host<'a> {
    pub st: &'a mut State,
    /// the shell's state as the surfaces read it (services::bridge::ShellState):
    /// services set their fields, the host says it once per turn when changed
    state: &'a mut Value,
    state_dirty: &'a mut bool,
    /// calls one service makes to another, run after the current one
    pending: &'a mut Vec<Ask>,
}

impl Host<'_> {
    /// A line as RustUi.qml would write it to stdin (handled as one)
    pub fn say(&mut self, line: Value) {
        if crate::wayland::wakes_on() {
            let what = ["to", "kind", "cmd"].iter().find_map(|k| line[*k].as_str()).unwrap_or("?").to_string();
            crate::wayland::WAKE_MSGS.with(|w| *w.borrow_mut().entry(format!("say/{what}")).or_default() += 1);
        }
        let _ = self.st.tx.send(Msg::Line(line.to_string()));
    }
    /// A command for a surface ("open" on a screen, "close", or its own)
    pub fn command(&mut self, cmd: &str, surface: &str, screen: Option<&str>, extra: Value) {
        let mut v = json!({"cmd": cmd, "surface": surface});
        if let Some(s) = screen {
            v["screen"] = json!(s);
        }
        if let Value::Object(m) = extra {
            for (k, x) in m {
                v[k] = x;
            }
        }
        self.say(v);
    }
    /// A message for one surface's part (RustUi.tell(surface, obj))
    pub fn tell(&mut self, surface: &str, mut obj: Value) {
        obj["to"] = json!(surface);
        self.say(obj);
    }
    /// One field of the shell's state ("open", "focus.mode", "unread"…), as
    /// RustUi.qml's state object names them
    pub fn set_state(&mut self, path: &str, v: Value) {
        let mut at = &mut *self.state;
        let keys: Vec<&str> = path.split('.').collect();
        for k in &keys[..keys.len() - 1] {
            if !at.get(*k).is_some_and(Value::is_object) {
                at[*k] = json!({});
            }
            at = &mut at[*k];
        }
        if at[keys[keys.len() - 1]] != v {
            at[keys[keys.len() - 1]] = v;
            *self.state_dirty = true;
            if crate::wayland::wakes_on() {
                crate::wayland::WAKE_MSGS.with(|w| *w.borrow_mut().entry(format!("state/{path}")).or_default() += 1);
            }
        }
    }
    pub fn state(&self, path: &str) -> &Value {
        path.split('.').fold(&*self.state, |v, k| &v[k])
    }
    /// Another service's call (answered after this one returns)
    pub fn ask(&mut self, call: &str, args: Value) {
        self.pending.push(Ask { call: call.into(), args });
    }
    /// For a service's threads: sends `msg` to that service's `message`
    pub fn sender(&self, service: &'static str) -> impl Fn(Box<dyn Any + Send>) + Send + Clone + 'static {
        let tx = self.st.tx.clone();
        move |m| {
            let _ = tx.send(Msg::Part(NAME, Box::new(ToService(service, m))));
        }
    }
    /// The settings (config.json as last read)
    pub fn cfg(&self) -> &Value {
        &self.st.shell.cfg
    }
}

/// One QML service, in Rust
#[allow(unused_variables)]
pub trait Service {
    /// its name (the QML service's: "Network", "Panels"…)
    fn name(&self) -> &'static str;
    /// the call prefixes it answers: "network" answers "network.*" and "network"
    fn calls(&self) -> &'static [&'static str] {
        &[]
    }
    /// the shell-ipc targets it answers (`qs ipc call <target> <fn> …`)
    fn ipc_targets(&self) -> &'static [&'static str] {
        &[]
    }
    fn start(&mut self, h: &mut Host) {}
    fn call(&mut self, h: &mut Host, call: &str, args: &Value) {}
    /// An IPC call for one of its targets; the answer (if any) goes back to
    /// the caller (`tessera-ui ipc call` prints it)
    fn ipc(&mut self, h: &mut Host, target: &str, func: &str, args: &[String]) -> Option<String> {
        None
    }
    fn config_changed(&mut self, h: &mut Host) {}
    fn outputs_changed(&mut self, h: &mut Host) {}
    /// a message from its own threads (Host::sender)
    fn message(&mut self, h: &mut Host, msg: Box<dyn Any + Send>) {}
    fn quit(&mut self, h: &mut Host) {}
    /// After every turn of the host (a call, an IPC call, a message): a
    /// service that mirrors what it keeps into the shell's state does it here
    fn settled(&mut self, h: &mut Host) {}
    /// A core service's new reading (`h.st.shell.get::<T>()` has it): the
    /// Part::data seam, so a service that wants the battery, the sound, the
    /// clients… need not poll for them (Island's 3 s battery look)
    fn data(&mut self, h: &mut Host, what: std::any::TypeId) {}
    /// The glass plugin was found or lost (`h.st.shell.plugin`; a plugin that
    /// has just loaded starts blank: GlassPlugin.qml's `ready`)
    fn plugin_changed(&mut self, h: &mut Host) {}
    /// The host's one shared second (`n` counts them from 1): a service
    /// that polls once a second (or every few: `n % k == 0`) does it here
    /// instead of keeping a timer of its own. Each own timer woke the loop
    /// at its own phase and cost a whole host turn apiece (every service's
    /// `settled`, the state sent); this is one turn for all of them, just
    /// after the wall clock's second.
    fn every_second(&mut self, h: &mut Host, n: u64) {}
}

/// The shared second's message to the host (HostPart::arm_second)
struct Second;

fn prefix(call: &str) -> &str {
    call.split('.').next().unwrap_or(call)
}

/// The host, as a part of the loop (made only standalone)
pub struct HostPart {
    services: Vec<Box<dyn Service>>,
    state: Value,
    state_dirty: bool,
    pending: Vec<Ask>,
    /// the shared second (Service::every_second)
    second: slint::Timer,
    seconds: u64,
}

/// How long until just after the wall clock's next whole second (a few ms
/// after, so a clock read then shows the new second)
fn to_next_second(now: std::time::SystemTime) -> std::time::Duration {
    let ms = now.duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_millis()).unwrap_or(0) as u64;
    std::time::Duration::from_millis(1000 - ms + crate::pacing::SECOND_AFTER_MS)
}

impl HostPart {
    pub fn new(services: Vec<Box<dyn Service>>) -> Self {
        Self { services, state: json!({}), state_dirty: false, pending: Vec::new(), second: slint::Timer::default(), seconds: 0 }
    }

    /// The next shared second (single shot, armed again each time: it stays
    /// on the wall clock's second, and a loop that stalled doesn't queue up
    /// ticks behind it)
    fn arm_second(&mut self, st: &State) {
        let tx = st.tx.clone();
        self.second.start(slint::TimerMode::SingleShot, to_next_second(std::time::SystemTime::now()), move || {
            let _ = tx.send(Msg::Part(NAME, Box::new(Second)));
        });
    }

    fn each(&mut self, st: &mut State, mut f: impl FnMut(&mut dyn Service, &mut Host)) {
        for s in self.services.iter_mut() {
            let mut h = Host { st, state: &mut self.state, state_dirty: &mut self.state_dirty, pending: &mut self.pending };
            let n = s.name();
            // (each service under its own name: one that panics stays off, the rest go on: contain.rs)
            crate::perf::time(|| format!("host {n}"), || crate::contain::guard(n, || f(s.as_mut(), &mut h)));
        }
        self.settle(st);
    }

    fn one(&mut self, st: &mut State, pick: impl Fn(&dyn Service) -> bool, f: impl FnOnce(&mut dyn Service, &mut Host)) -> bool {
        let Some(s) = self.services.iter_mut().find(|s| pick(s.as_ref())) else { return false };
        let mut h = Host { st, state: &mut self.state, state_dirty: &mut self.state_dirty, pending: &mut self.pending };
        let n = s.name();
        crate::perf::time(|| format!("host {n}"), || crate::contain::guard(n, || f(s.as_mut(), &mut h)));
        true
    }

    fn answer(&mut self, st: &mut State, a: Ask) {
        let p = prefix(&a.call).to_string();
        let found = crate::perf::time(|| format!("ask {}", a.call), || self.one(st, |s| s.calls().contains(&p.as_str()), |s, h| s.call(h, &a.call, &a.args)));
        if !found {
            eprintln!("tessera-ui: host: nothing answers {} (args {})", a.call, a.args);
        }
    }

    /// Calls services made to each other, then the state if it changed
    fn settle(&mut self, st: &mut State) {
        // (a `settled` hook may ask too: those are answered before the state goes out)
        for _ in 0..4 {
            for _ in 0..16 {
                if self.pending.is_empty() {
                    break;
                }
                for a in std::mem::take(&mut self.pending) {
                    self.answer(st, a);
                }
            }
            for s in self.services.iter_mut() {
                let mut h = Host { st, state: &mut self.state, state_dirty: &mut self.state_dirty, pending: &mut self.pending };
                let n = s.name();
                crate::perf::time(|| format!("settled {n}"), || crate::contain::guard(n, || s.settled(&mut h)));
            }
            if self.pending.is_empty() {
                break;
            }
        }
        if self.state_dirty {
            self.state_dirty = false;
            let mut v = self.state.clone();
            v["src"] = json!("shell");
            v["kind"] = json!("state");
            // (as the line RustUi.qml writes would arrive, but not written out
            // and read back in: the same queue, so it still comes after the
            // commands this turn said)
            let _ = st.tx.send(Msg::Data(Box::new(crate::services::bridge::ShellState(v))));
        }
    }
}

impl crate::shell::Part for HostPart {
    fn name(&self) -> &'static str {
        NAME
    }
    fn start(&mut self, st: &mut State) {
        // (a slow start holds up the first frame: say which, and how long)
        self.each(st, |s, h| {
            let t = std::time::Instant::now();
            s.start(h);
            let ms = t.elapsed().as_millis();
            if ms >= 25 {
                eprintln!("tessera-ui: slow start: host {} {ms} ms", s.name());
            }
        });
        self.arm_second(st);
    }
    fn config_changed(&mut self, st: &mut State, _look: bool) {
        self.each(st, |s, h| s.config_changed(h));
    }
    fn output_added(&mut self, st: &mut State, _: &smithay_client_toolkit::reexports::client::protocol::wl_output::WlOutput, _: &str) {
        self.each(st, |s, h| s.outputs_changed(h));
    }
    fn output_removed(&mut self, st: &mut State, _: &str) {
        self.each(st, |s, h| s.outputs_changed(h));
    }
    fn data(&mut self, st: &mut State, what: std::any::TypeId) {
        self.each(st, |s, h| s.data(h, what));
    }
    fn plugin_changed(&mut self, st: &mut State) {
        self.each(st, |s, h| s.plugin_changed(h));
    }
    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        let msg = match msg.downcast::<Second>() {
            Ok(_) => {
                if crate::wayland::wakes_on() {
                    crate::wayland::WAKE_MSGS.with(|w| *w.borrow_mut().entry("host/second".into()).or_default() += 1);
                }
                self.seconds += 1;
                let n = self.seconds;
                self.arm_second(st);
                return self.each(st, |s, h| s.every_second(h, n));
            }
            Err(m) => m,
        };
        let msg = match msg.downcast::<Ask>() {
            Ok(a) => {
                self.answer(st, *a);
                return self.settle(st);
            }
            Err(m) => m,
        };
        let msg = match msg.downcast::<Ipc>() {
            Ok(i) => {
                let (target, func) = (i.args.first().cloned().unwrap_or_default(), i.args.get(1).cloned().unwrap_or_default());
                let rest: Vec<String> = i.args.iter().skip(2).cloned().collect();
                let mut out = None;
                if crate::perf::on() {
                    eprintln!("debug: ipc {}", i.args.join(" "));
                }
                // `ipc show`: every target some service answers, one `target X` line each
                if target == "show" && i.args.len() == 1 {
                    let mut all: Vec<&'static str> = self.services.iter().flat_map(|s| s.ipc_targets().iter().copied()).collect();
                    all.sort_unstable();
                    all.dedup();
                    if let Some(r) = i.reply {
                        let _ = r.send(all.iter().map(|t| format!("target {t}\n")).collect());
                    }
                    return self.settle(st);
                }
                let found = self.one(st, |s| s.ipc_targets().contains(&target.as_str()), |s, h| out = s.ipc(h, &target, &func, &rest));
                if !found {
                    eprintln!("tessera-ui: host: no ipc target {target} ({func})");
                }
                if let Some(r) = i.reply {
                    let _ = r.send(out.unwrap_or_default());
                }
                return self.settle(st);
            }
            Err(m) => m,
        };
        if let Ok(t) = msg.downcast::<ToService>() {
            let ToService(name, m) = *t;
            if crate::wayland::wakes_on() {
                crate::wayland::WAKE_MSGS.with(|w| *w.borrow_mut().entry(format!("host/{name}")).or_default() += 1);
            }
            self.one(st, |s| s.name() == name, |s, h| s.message(h, m));
            self.settle(st);
        }
    }
    fn quit(&mut self, st: &mut State) {
        self.second.stop();
        self.each(st, |s, h| s.quit(h));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_second_lands_just_after_the_clocks() {
        let at = |ms: u64| std::time::UNIX_EPOCH + std::time::Duration::from_millis(1_700_000_000_000 + ms);
        assert_eq!(to_next_second(at(0)).as_millis(), 1005);
        assert_eq!(to_next_second(at(250)).as_millis(), 755);
        assert_eq!(to_next_second(at(999)).as_millis(), 6);
    }

    #[test]
    fn calls_go_by_their_first_word() {
        assert_eq!(prefix("network.connect"), "network");
        assert_eq!(prefix("panels"), "panels");
    }
}

//! What every surface shares, and how surfaces plug in.
//!
//! A surface (the menu bar, the dock, the OSD…) is a `Part`: it lives in
//! src/surfaces/<name>/, makes its own layer surfaces and Slint components,
//! and hears about outputs, settings, data and its own messages through the
//! methods below. `Shell` is the common state: the look and settings from
//! config.json, the glass plugin, and the latest reading from every service
//! (src/services/), started the first time a part asks for it.
//!
//! Data: a service sends `Msg::Data(value)`; the shell keeps the newest value
//! of each type and calls every part's `data` with its TypeId; a part reads
//! it with `shell.get::<T>()`. Messages for one part: `Msg::Part(name, value)`.
//!
//! Talking with the QML shell (services/RustUi.qml), one JSON object a line
//! each way, so a panel drawn here opens and closes exactly as the QML one
//! did (its shortcut, IPC, Spotlight and Control Center all go through
//! Panels.show/toggle/close, and RustUi forwards those):
//!   stdin, commands for a part (`Part::command`), from the shell:
//!     {"cmd":"open","surface":"emoji","screen":"DP-1"}  Panels opened it there
//!     {"cmd":"close","surface":"emoji"}                 Panels closed it (another
//!                                                       panel opened, its shortcut again…)
//!     (any other "cmd" reaches the part too, the whole object in `Command::args`)
//!   stdout, events for the shell (`closed`, `ask`), besides surfaces/mod.rs's
//!   ready/unknown:
//!     {"src":"ui","kind":"closed","surface":"emoji"}    it closed itself (Esc, a
//!                                                       click outside, a pick):
//!                                                       Panels.close() if still open
//!     {"src":"ui","kind":"call","call":"clipboard.pin","args":[…]}
//!                                                       something only the shell
//!                                                       does (its state files,
//!                                                       Settings): RustUi.qml's
//!                                                       short list of calls
//! A panel moves with one line in its QML twin, its PanelLoader stepping aside
//! (`elsewhere: RustUi.drawing("<name>")`), and `command` here. stdin closing
//! still means the shell quit.

use crate::services::hypr::Glass;
use crate::theme::{self, Look};
use crate::wayland::{State, Surface};
use crate::Msg;
use smithay_client_toolkit::reexports::calloop::channel::Sender;
use smithay_client_toolkit::reexports::client::protocol::wl_output::WlOutput;
use std::any::{Any, TypeId};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

pub struct Shell {
    pub look: Look,
    /// config.json, as last read
    pub cfg: serde_json::Value,
    pub glass: Glass,
    /// the glass plugin answered (Glass panels then draw on its glass)
    pub plugin: bool,
    pub tx: Sender<Msg>,
    /// what this process draws (--surfaces)
    pub surfaces: crate::surfaces::Surfaces,
    data: HashMap<TypeId, Box<dyn Any>>,
    started: HashSet<&'static str>,
}

impl Shell {
    pub fn new(tx: Sender<Msg>, glass: Glass, surfaces: crate::surfaces::Surfaces) -> Self {
        Self { look: theme::read(), cfg: theme::read_json(), glass, plugin: false, tx, surfaces, data: HashMap::new(), started: HashSet::new() }
    }

    /// The newest reading of a service's type, if one came yet
    pub fn get<T: 'static>(&self) -> Option<&T> {
        self.data.get(&TypeId::of::<T>()).and_then(|b| b.downcast_ref())
    }

    /// Start a service (once, however many parts want it): its readings
    /// arrive as `data` calls. Names: crate::services::start
    pub fn want(&mut self, service: &'static str) {
        if self.started.insert(service) {
            crate::services::start(service, self.tx.clone());
        }
    }

    /// Fill a component's Theme global for the current look
    pub fn apply_theme(&self, t: &crate::ui::Theme) {
        theme::apply(t, &self.look, self.plugin);
    }

    /// A message to one part, from a callback (any thread)
    pub fn sender(&self, part: &'static str) -> impl Fn(Box<dyn Any + Send>) + Clone + 'static {
        let tx = self.tx.clone();
        move |m| {
            let _ = tx.send(Msg::Part(part, m));
        }
    }

    pub(crate) fn store(&mut self, value: Box<dyn Any + Send>) -> TypeId {
        let id = (*value).type_id();
        self.data.insert(id, value);
        id
    }
}

/// A command from the shell for one part (a line on stdin)
#[derive(Clone, Debug, PartialEq)]
pub struct Command {
    /// what to do: "open", "close", or a part's own
    pub cmd: String,
    /// the part it's for (its name, as in surfaces::KNOWN)
    pub surface: String,
    /// the output it's for, when the shell said ("open": Panels.screen)
    pub screen: Option<String>,
    /// the whole object, for anything else a part's command carries
    pub args: serde_json::Value,
}

impl Command {
    /// One line from the shell; None for anything that isn't a command
    pub fn parse(line: &str) -> Option<Self> {
        let v: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
        let s = |k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_string);
        let (cmd, surface) = (s("cmd")?, s("surface")?);
        if cmd.is_empty() || surface.is_empty() {
            return None;
        }
        Some(Self { cmd, surface, screen: s("screen").filter(|x| !x.is_empty()), args: v })
    }
}

/// Splits what comes on stdin into lines, keeping a partial last line for
/// the next read (a pipe can cut a line anywhere)
#[derive(Default)]
pub struct Lines(Vec<u8>);

impl Lines {
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<String> {
        self.0.extend_from_slice(bytes);
        let mut out = Vec::new();
        while let Some(i) = self.0.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = self.0.drain(..=i).collect();
            out.push(String::from_utf8_lossy(&line[..line.len() - 1]).into_owned());
        }
        // (a megabyte without a newline: not a protocol we speak)
        if self.0.len() > 1 << 20 {
            self.0.clear();
        }
        out
    }
}

/// An event for the shell: `{"src":"ui","kind":…, …}`
pub fn event(kind: &str, mut fields: serde_json::Value) -> String {
    let mut v = serde_json::json!({"src": "ui", "kind": kind});
    if let (Some(o), Some(f)) = (v.as_object_mut(), fields.as_object_mut()) {
        o.append(f);
    }
    v.to_string()
}

/// Say it to the shell (a line on stdout)
pub fn tell(kind: &str, fields: serde_json::Value) {
    use std::io::Write;
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{}", event(kind, fields));
    let _ = out.flush();
}

/// A panel closed itself: the shell's Panels follows (Panels.close() while
/// it's still the open one)
pub fn closed(surface: &str) {
    tell("closed", serde_json::json!({"surface": surface}));
}

/// Ask the shell to do something it owns (RustUi.qml's list of calls)
pub fn ask(call: &str, args: serde_json::Value) {
    tell("call", serde_json::json!({"call": call, "args": args}));
}

/// A surface of the shell, drawn in Rust. Every method has a default: a part
/// implements what it needs. `st` is the platform (layer surfaces, outputs,
/// the seat) and `st.shell` the shared state.
#[allow(unused_variables)]
pub trait Part {
    fn name(&self) -> &'static str;
    /// The platform is up: ask for services, make what doesn't follow outputs
    fn start(&mut self, st: &mut State) {}
    /// An output appeared (or one it skipped is wanted after a settings change)
    fn output_added(&mut self, st: &mut State, output: &WlOutput, name: &str) {}
    fn output_removed(&mut self, st: &mut State, name: &str) {}
    /// config.json changed (st.shell.cfg and .look are already the new ones);
    /// `look`: the look itself changed (colours, Glass/Material)
    fn config_changed(&mut self, st: &mut State, look: bool) {}
    /// The glass plugin was found or lost (st.shell.plugin)
    fn plugin_changed(&mut self, st: &mut State) {}
    /// A service's new reading (read it with st.shell.get::<T>())
    fn data(&mut self, st: &mut State, what: TypeId) {}
    /// Its own message (Msg::Part with its name)
    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {}
    /// A command from the shell (stdin) with its name: "open" (on
    /// `cmd.screen`), "close", or its own
    fn command(&mut self, st: &mut State, cmd: &Command) {}
    /// One of its surfaces lost the keyboard (another window was clicked)
    fn keyboard_left(&mut self, st: &mut State, surface: &Rc<Surface>) {}
    /// The compositor closed one of its surfaces
    fn surface_closed(&mut self, st: &mut State, surface: &Rc<Surface>) {}
    /// After each turn of the loop, once drawn: glass shapes, accessibility
    fn after_frame(&mut self, st: &mut State) {}
    /// Its accessibility trees (for AT-SPI's actions)
    fn a11y(&mut self) -> Vec<&mut crate::a11y::A11y> {
        Vec::new()
    }
    /// SIGUSR1: frame statistics
    fn stats(&self) {}
    /// On the way out: leave nothing behind (glass shapes)
    fn quit(&mut self, st: &mut State) {}
}

impl State {
    /// Call every part, each with the whole state (parts are taken out while
    /// they run, so a part can use the platform freely)
    pub fn each_part(&mut self, mut f: impl FnMut(&mut dyn Part, &mut State)) {
        let mut parts = std::mem::take(&mut self.parts);
        for p in parts.iter_mut() {
            f(p.as_mut(), self);
        }
        parts.append(&mut self.parts);
        self.parts = parts;
    }

    pub fn part(&mut self, name: &str, f: impl FnOnce(&mut dyn Part, &mut State)) {
        let mut parts = std::mem::take(&mut self.parts);
        if let Some(p) = parts.iter_mut().find(|p| p.name() == name) {
            f(p.as_mut(), self);
        }
        parts.append(&mut self.parts);
        self.parts = parts;
    }

    /// Every output there is now, by name
    pub fn outputs_now(&self) -> Vec<(WlOutput, String)> {
        self.outputs.outputs().filter_map(|o| self.outputs.info(&o).and_then(|i| i.name).map(|n| (o, n))).collect()
    }

    pub fn output_named(&self, name: &str) -> Option<WlOutput> {
        self.outputs_now().into_iter().find(|(_, n)| n == name).map(|(o, _)| o)
    }

    /// config.json changed: read it, tell every part
    pub fn config_changed(&mut self) {
        self.shell.cfg = theme::read_json();
        let look = theme::read();
        let changed = look != self.shell.look;
        self.shell.look = look;
        self.each_part(|p, st| p.config_changed(st, changed));
    }

    /// A line from the shell: a command, to the part it names
    pub fn command(&mut self, cmd: Command) {
        if crate::env::is_set("DEBUG") {
            eprintln!("debug: command {} {}", cmd.cmd, cmd.surface);
        }
        let name = cmd.surface.clone();
        self.part(&name, |p, st| p.command(st, &cmd));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn commands_from_the_shell() {
        let c = Command::parse(r#"{"cmd":"open","surface":"emoji","screen":"DP-1"}"#).unwrap();
        assert_eq!((c.cmd.as_str(), c.surface.as_str(), c.screen.as_deref()), ("open", "emoji", Some("DP-1")));
        let c = Command::parse(r#" {"cmd":"close","surface":"clipboard"} "#).unwrap();
        assert_eq!((c.cmd.as_str(), c.screen), ("close", None));
        // a part's own command keeps everything it carried
        let c = Command::parse(r#"{"cmd":"query","surface":"emoji","text":"cat","screen":""}"#).unwrap();
        assert_eq!((c.args["text"].as_str(), c.screen), (Some("cat"), None));
        for bad in ["", "open emoji", "{}", r#"{"cmd":"open"}"#, r#"{"surface":"emoji"}"#, r#"{"cmd":"","surface":"emoji"}"#, r#"{"cmd":1,"surface":"emoji"}"#, "[1]"] {
            assert_eq!(Command::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn lines_cut_anywhere_come_out_whole() {
        let mut l = Lines::default();
        assert!(l.feed(br#"{"cmd":"op"#).is_empty());
        assert_eq!(l.feed(b"en\"}\n{\"a\":1}\n{\"b"), vec![r#"{"cmd":"open"}"#.to_string(), r#"{"a":1}"#.to_string()]);
        assert_eq!(l.feed(b"\":2}\n"), vec![r#"{"b":2}"#.to_string()]);
        assert!(l.feed(b"").is_empty());
        // (not UTF-8: replaced, never a panic)
        assert_eq!(l.feed(b"\xff\n").len(), 1);
    }

    #[test]
    fn events_for_the_shell() {
        let e: serde_json::Value = serde_json::from_str(&event("closed", json!({"surface": "emoji"}))).unwrap();
        assert_eq!(e, json!({"src": "ui", "kind": "closed", "surface": "emoji"}));
        let e: serde_json::Value = serde_json::from_str(&event("call", json!({"call": "clipboard.pin", "args": [{"id": "3"}]}))).unwrap();
        assert_eq!((e["src"].as_str(), e["args"][0]["id"].as_str()), (Some("ui"), Some("3")));
    }
}

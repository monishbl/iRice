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

/// A message from the QML shell for one part: a JSON line on tessera-ui's
/// stdin, `{"to": "<part>", …}` (services/RustUi.qml's `tell`), handed to
/// that part's `message` (e.g. a panel the shell opened: the wallpaper picker)
pub struct FromShell(pub serde_json::Value);

/// One stdin line: the part it's for, and the message
pub fn from_shell(line: &[u8]) -> Option<(String, FromShell)> {
    let v: serde_json::Value = serde_json::from_slice(line).ok()?;
    let to = v["to"].as_str()?.to_string();
    Some((to, FromShell(v)))
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_lines_name_their_part() {
        let (to, m) = from_shell(br#"{"to": "wallpaper", "open": "DP-1"}"#).unwrap();
        assert_eq!((to.as_str(), m.0["open"].as_str()), ("wallpaper", Some("DP-1")));
        assert!(from_shell(b"{\"open\": \"DP-1\"}").is_none());
        assert!(from_shell(b"not json").is_none());
    }
}

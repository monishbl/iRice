//! The shell's keyboard shortcuts as Hyprland's global shortcuts
//! (hyprland-global-shortcuts-v1), as Quickshell registers the QML's
//! GlobalShortcut items: a bind `hl.dsp.global("tessera:<name>")` reaches us
//! as an event on an open connection, with no process started per key press
//! (`tessera-ui ipc call shortcut <name>` started the whole 90 MB binary for
//! every press, ~10–30 ms before the shell even heard of it).
//!
//! A thread of its own with its own Wayland connection (as services/capture.rs):
//! `register(app_id, names, on)` registers each name once and calls `on(name,
//! pressed)` on that thread for every press and release. Standalone only
//! (beside the QML shell, Quickshell owns these names).

use wayland_client::globals::{registry_queue_init, GlobalListContents};
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::{Connection, Dispatch, QueueHandle};

#[allow(dead_code, unused_imports, non_camel_case_types, non_upper_case_globals, clippy::all)]
pub mod proto {
    use wayland_client;
    use wayland_client::protocol::*;
    pub mod __interfaces {
        use wayland_client::protocol::__interfaces::*;
        wayland_scanner::generate_interfaces!("protocols/hyprland-global-shortcuts-v1.xml");
    }
    use self::__interfaces::*;
    wayland_scanner::generate_client_code!("protocols/hyprland-global-shortcuts-v1.xml");
}

use proto::hyprland_global_shortcut_v1::{self as shortcut, HyprlandGlobalShortcutV1};
use proto::hyprland_global_shortcuts_manager_v1::HyprlandGlobalShortcutsManagerV1;

type On = Box<dyn Fn(&str, bool) + Send>;

struct Keys {
    on: On,
}

/// Register `names` under `app_id` (Hyprland's `app_id:name`) and listen, on
/// a thread (nothing happens when the compositor has no global shortcuts)
pub fn register(app_id: &'static str, names: Vec<String>, on: impl Fn(&str, bool) + Send + 'static) {
    std::thread::spawn(move || {
        let Ok(conn) = Connection::connect_to_env() else { return };
        let Ok((globals, mut queue)) = registry_queue_init::<Keys>(&conn) else { return };
        let qh = queue.handle();
        let Ok(manager) = globals.bind::<HyprlandGlobalShortcutsManagerV1, _, _>(&qh, 1..=1, ()) else {
            eprintln!("tessera-ui: no global shortcuts (hyprland-global-shortcuts-v1 isn't offered)");
            return;
        };
        let mut keys = Keys { on: Box::new(on) };
        // (the objects must live for their events to reach us)
        let _held: Vec<HyprlandGlobalShortcutV1> = names
            .into_iter()
            .map(|n| manager.register_shortcut(n.clone(), app_id.into(), n.clone(), String::new(), &qh, n))
            .collect();
        let _ = conn.flush();
        while queue.blocking_dispatch(&mut keys).is_ok() {}
        eprintln!("tessera-ui: global shortcuts: the connection ended");
    });
}

impl Dispatch<WlRegistry, GlobalListContents> for Keys {
    fn event(_: &mut Self, _: &WlRegistry, _: wayland_client::protocol::wl_registry::Event, _: &GlobalListContents, _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<HyprlandGlobalShortcutsManagerV1, ()> for Keys {
    fn event(_: &mut Self, _: &HyprlandGlobalShortcutsManagerV1, _: proto::hyprland_global_shortcuts_manager_v1::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<HyprlandGlobalShortcutV1, String> for Keys {
    fn event(k: &mut Self, _: &HyprlandGlobalShortcutV1, e: shortcut::Event, name: &String, _: &Connection, _: &QueueHandle<Self>) {
        match e {
            shortcut::Event::Pressed { .. } => (k.on)(name, true),
            shortcut::Event::Released { .. } => (k.on)(name, false),
        }
    }
}

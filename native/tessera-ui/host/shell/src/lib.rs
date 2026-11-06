//! The host's shell services (wave 4, core/src/host.rs): what
//! shell/TesseraShell.qml, shell/common/ and the shell-wide services do,
//! standalone. Also the IPC socket and `tessera-ui ipc call` (ipc.rs).
//! Each module names the QML file(s) it replaces; PORT-NOTES.md says what
//! is ported, what differs, and how to check it.
//!
//!   common      Paths, Config's reading and writing, UiState, Launch, SettingsMap's
//!               aliases, CoolDown's rules, KeptProcess, the wallpaper's tone
//!   panels      Panels (which panel is open and where) with RustUi's forwarding,
//!               the `panels`, `shell` and `rustui` IPC targets
//!   island      Island and IslandCycle
//!   bridges     Welcome, Overlay, Lock and the login intro, hot corners, OSD and
//!               Polkit IPC, Launch/FilePicker/Wallpaper calls, Settings' look and notify groups
//!   appearance  CoolDown and the glass line
//!   events      Events, EventWatch, Startup, StartupSounds
//!   sounds      Sounds
//!   intents     Intents
//!   probes      Backends, Needs, ShellPriority, Conflicts, Health, Keybinds, HyprlandLink, BugReport
//!   shortcuts   the GlobalShortcut lists, by name
//!   ipc         the socket and the client

pub mod appearance;
pub mod bridges;
pub mod common;
pub mod events;
pub mod intents;
pub mod ipc;
pub mod island;
pub mod panels;
pub mod probes;
pub mod shortcuts;
pub mod sounds;

use tessera_core::host::Service;

/// Every service of this crate (the binary collects them: src/main.rs).
/// Bridges is before Sounds: it looks for this session's mark before the
/// startup chime makes it (the login intro plays only on the first start).
pub fn services() -> Vec<Box<dyn Service>> {
    vec![
        Box::new(ipc::IpcServer::default()),
        Box::new(events::Events::default()),
        Box::new(events::Startup::default()),
        Box::new(panels::Panels::new()),
        Box::new(island::Island::default()),
        Box::new(appearance::Appearance::default()),
        Box::new(bridges::Bridges::default()),
        Box::new(sounds::Sounds::default()),
        Box::new(intents::Intents),
        Box::new(probes::Probes::default()),
        Box::new(shortcuts::Shortcuts::default()),
    ]
}

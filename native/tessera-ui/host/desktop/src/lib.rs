//! The host's desktop services (wave 4, core/src/host.rs): the QML shell's
//! services, standalone. Each module names the QML file(s) it replaces;
//! PORT-NOTES.md says what is ported, what differs, and how to check it.
//!
//!   wm/      windows, spaces, tiling, focus, apps, desktop, switcher
//!   screen/  search, files, clipboard, screen tools, recorder, live text, captions
//!   today/   Today, todos, timers, alarms, weather, calendar, notes, notifications, media, calls, badges
//!   ai/      the AI chat (Ai.qml and its helpers)

use tessera_core::host::Service;

mod ai;
mod screen;
mod today;
mod wm;

/// Every service of this crate (the binary collects them: src/main.rs)
pub fn services() -> Vec<Box<dyn Service>> {
    let mut v = Vec::new();
    v.extend(wm::services());
    v.extend(screen::services());
    v.extend(today::services());
    v.extend(ai::services());
    v
}

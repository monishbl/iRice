//! Idle notifications (ext-idle-notify-v1): what Quickshell's IdleMonitor gave
//! the QML services, as the shell's data. A service asks with
//! `h.st.idle_set(ID, Some((milliseconds, respect_inhibitors)))` (State, in
//! wayland.rs: asking again with the same numbers does nothing, so it may do
//! it after every turn, and `None` stops the watch); the compositor's answers
//! arrive as `Idle` data (`h.st.shell.get::<Idle>()`, and Service::data /
//! Part::data with its TypeId). A watch not asked, or a compositor without the
//! protocol, reads as not idle.

use std::collections::HashMap;

/// Lock.qml: charging and left alone for 5 minutes, the screen locks (and the
/// lock then shows Bedside Mode after 20 s more)
pub const STANDBY: u32 = 1;
/// Lock.qml: any input at all while locked (a key, the pointer, even with the
/// screen off) wakes the face and fingerprint tries. Inhibitors don't count.
pub const LOCK_WAKE: u32 = 2;
/// ScreenTime.qml: 2 minutes without input stops the clock
pub const SCREEN_TIME: u32 = 3;
/// Google.qml and CloudAccounts.qml: 5 minutes without input skips syncs
pub const LONG: u32 = 4;

/// What every watch last said (true: idle). Newest picture replaces the old.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Idle(pub HashMap<u32, bool>);

impl Idle {
    pub fn is(&self, id: u32) -> bool {
        self.0.get(&id).copied().unwrap_or(false)
    }
}

//! The host's system services (wave 4, core/src/host.rs): the QML shell's
//! services, standalone. Each module names the QML file(s) it replaces;
//! PORT-NOTES.md says what is ported, what differs, and how to check it.
//!
//! Four groups (one directory each): net (network, audio, brightness, power,
//! resources), input (displays, mouse, keyboard, devices bridges), sysinfo
//! (privacy, system, about, updates, root helper), accounts (login, boot,
//! fingerprint, cloud, accounts bridges, control centre bridge).

#[allow(unused_imports)]
use tessera_core::*;
use tessera_core::host::Service;

pub mod accounts;
pub mod bus;
pub mod input;
pub mod net;
pub mod sysinfo;
pub mod util;

/// Every service of this crate (the binary collects them: src/main.rs)
pub fn services() -> Vec<Box<dyn Service>> {
    let mut v = Vec::new();
    v.extend(net::services());
    v.extend(input::services());
    v.extend(sysinfo::services());
    v.extend(accounts::services());
    v
}

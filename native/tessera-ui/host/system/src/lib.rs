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
    v.push(Box::new(Mirror));
    v
}

/// What other crates read of this one's services, mirrored from the bus into
/// the shell's state after every turn (the lock reads `fingerprint.lockScreen`
/// and `face.lockScreen`, as Lock.qml read Fingerprint and Face)
struct Mirror;

impl Service for Mirror {
    fn name(&self) -> &'static str {
        "SystemMirror"
    }
    fn settled(&mut self, h: &mut tessera_core::host::Host) {
        for (svc, key) in [("fingerprint", "lockScreen"), ("face", "lockScreen")] {
            let v = bus::field(svc, key);
            h.set_state(&format!("{svc}.{key}"), if v.is_null() { serde_json::json!(false) } else { v });
        }
    }
}

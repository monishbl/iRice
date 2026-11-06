//! Island alerts, as this group posts them: the QML's `Island.post(event, ms)`
//! is host-shell's; here a call "island.post" [event, ms] reaches it (the host
//! says the same `{"src":"shell","kind":"island","op":"post",…}` line).
//! `enabled`/`wants` read the island settings the way Island.qml does.

use serde_json::{json, Value};
use tessera_core::host::Host;

/// Island.enabled
pub fn enabled(h: &Host) -> bool {
    h.cfg()["island"]["enabled"].as_bool().unwrap_or(true)
}

/// Island.wants(kind): kinds added after the setting existed count as on until touched
pub fn wants(h: &Host, kind: &str) -> bool {
    const NEW: [&str; 8] = ["usb", "displays", "audio", "temperature", "network", "transfers", "calls", "sysupdate"];
    let cfg = &h.cfg()["island"];
    let has = |k: &str, l: &str| cfg[l].as_array().is_some_and(|a| a.iter().any(|x| x == k));
    has(kind, "events") || (NEW.contains(&kind) && !has(kind, "known"))
}

pub fn post(h: &mut Host, event: Value, ms: u64) {
    h.ask("island.post", json!([event, ms]));
}

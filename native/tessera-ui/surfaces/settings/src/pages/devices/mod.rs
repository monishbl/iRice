//! The "devices" page group: Displays, Sound, Wi-Fi & Bluetooth (network.qml
//! with bluetooth.qml as its part), Phone & Sharing (phone.qml with
//! sharing.qml). Its own views: ui/settings-devices.slint. The sound devices
//! and Bluetooth's are tessera-ui's own readings (services/audio.rs,
//! bluez.rs: real, read-only); what only the QML shell knows (the displays
//! as Displays.qml has them, Wi-Fi's networks and its join state, the speed
//! test, the phone, AirPlay, sharing) comes from
//! shell/modules/settings/RustBridgeDevices.qml (`cx.feed["devices"]`), and
//! everything that changes the system goes there (`cx.ask("devices", …)`).
//! The volumes and the default output are set on the sound server here, as
//! the Control Center in Rust does.

pub mod displays;
pub mod network;
pub mod phone;
pub mod sound;

use super::super::model::{Ctx, Page};
use serde_json::Value;

/// Build `page` if it's one of this group's; false when it isn't
pub fn build(page: &str, cx: &mut Ctx, p: &mut Page) -> bool {
    match page {
        "displays" => displays::displays(cx, p),
        "sound" => sound::sound(cx, p),
        "network" => network::network(cx, p),
        "phone" => phone::phone(cx, p),
        _ => return false,
    }
    true
}

/// The shell's side for this group (RustBridgeDevices.qml's feed)
pub fn feed(cx: &Ctx) -> Value {
    cx.feed["devices"].clone()
}

/// Ask the shell's side (RustBridgeDevices.qml's act)
pub fn ask(cx: &Ctx, what: &str, args: Value) {
    cx.ask("devices", what, args);
}

/// Material: a custom row's tile, by where it sits in its group (0
/// between, 1 first, 2 last, 3 alone)
pub fn place(first: bool, last: bool) -> f32 {
    match (first, last) {
        (true, true) => 3.0,
        (true, false) => 1.0,
        (false, true) => 2.0,
        _ => 0.0,
    }
}

/// A string from the feed ("" when missing)
pub fn s(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        v => v.to_string(),
    }
}

/// Copy text (wl-copy, as Quickshell.execDetached(["wl-copy", "--", …]))
pub fn copy(text: &str) {
    Ctx::detach(vec!["wl-copy".into(), "--".into(), text.into()]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn places_and_strings() {
        assert_eq!(place(true, true), 3.0);
        assert_eq!(place(false, false), 0.0);
        assert_eq!(s(&json!("a")), "a");
        assert_eq!(s(&json!(null)), "");
        assert_eq!(s(&json!(3)), "3");
    }
}

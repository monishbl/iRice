//! group: input. Displays, mouse, keyboard, shortcuts and devices, and the
//! Settings bridges for Devices and Input (see lib.rs). Each submodule names
//! the QML files it replaces.

use tessera_core::host::Service;

pub mod common;
pub mod common_island;
pub mod device_alerts;
pub mod devices_bridge;
pub mod displays;
pub mod displays_feed;
pub mod input_bridge;
pub mod keyboard_layout;
pub mod keyfmt;
pub mod keymap;
pub mod mouse;

/// This group's services
pub fn services() -> Vec<Box<dyn Service>> {
    vec![
        Box::new(displays::Displays::default()),
        Box::new(displays_feed::DisplaysFeed::default()),
        Box::new(mouse::Mouse::default()),
        Box::new(keyboard_layout::KeyboardLayout::default()),
        Box::new(input_bridge::Input::default()),
        Box::new(device_alerts::DeviceAlerts::default()),
        Box::new(devices_bridge::DevicesBridge::default()),
    ]
}

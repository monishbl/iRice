//! group: sysinfo (see lib.rs). Each submodule replaces QML files named in
//! its header:
//!   root_helper      services/RootHelper.qml
//!   system_info      services/SystemInfo.qml
//!   privacy          services/Privacy.qml
//!   system           services/System.qml (+ the prefix "system")
//!   bridge_system    modules/settings/RustBridgeSystem.qml (part of system)
//!   updates          services/Updates.qml
//!   tessera_update   services/TesseraUpdate.qml
//!   software_update  services/SoftwareUpdate.qml
//!   bridge_about     modules/settings/RustBridgeAbout.qml (owns the three
//!                    update models; prefixes about/updates/tesseraupdate/
//!                    softwareupdate)

pub mod bridge_about;
pub mod bridge_system;
pub mod helpers;
pub mod privacy;
pub mod root_helper;
pub mod software_update;
pub mod system;
pub mod system_info;
pub mod tessera_update;
pub mod updates;

use tessera_core::host::Service;

/// This group's services
pub fn services() -> Vec<Box<dyn Service>> {
    vec![
        Box::new(root_helper::RootHelper),
        Box::new(system_info::SystemInfo),
        Box::new(privacy::Privacy::default()),
        Box::new(system::System::default()),
        Box::new(bridge_about::About::default()),
    ]
}

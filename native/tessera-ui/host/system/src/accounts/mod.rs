//! group: accounts (see lib.rs). Each submodule replaces QML files named in
//! its header:
//!   account            services/Account.qml
//!   fingerprint        services/Fingerprint.qml
//!   face               services/Face.qml
//!   login_screen       services/LoginScreen.qml
//!   boot_splash        services/BootSplash.qml
//!   boot_snapshots     services/BootSnapshots.qml
//!   screen_time        services/ScreenTime.qml
//!   cloud_drives       services/CloudDrives.qml
//!   cloud_accounts     services/CloudAccounts.qml
//!   google             services/Google.qml
//!   accounts_bridge    modules/settings/RustBridgeAccounts.qml  (prefix "accounts")
//!   cc                 modules/controlcenter/RustBridge.qml + ModuleDefs.qml  (prefix "cc")

use tessera_core::host::Service;

pub mod account;
pub mod accounts_bridge;
pub mod boot_snapshots;
pub mod boot_splash;
pub mod cc;
pub mod cloud_accounts;
pub mod cloud_drives;
pub mod common;
pub mod face;
pub mod fingerprint;
pub mod google;
pub mod login_screen;
pub mod screen_time;

/// This group's services
pub fn services() -> Vec<Box<dyn Service>> {
    vec![
        Box::new(account::Account::default()),
        Box::new(fingerprint::Fingerprint::default()),
        Box::new(face::Face::default()),
        Box::new(login_screen::LoginScreen::default()),
        Box::new(boot_splash::BootSplash::default()),
        Box::new(boot_snapshots::BootSnapshots::default()),
        Box::new(screen_time::ScreenTime::default()),
        Box::new(cloud_drives::CloudDrives::default()),
        Box::new(cloud_accounts::CloudAccounts::default()),
        Box::new(google::Google::default()),
        Box::new(accounts_bridge::AccountsBridge::default()),
        Box::new(cc::ControlCenter::default()),
    ]
}

//! The system, read for the surfaces: each service runs on its own thread(s)
//! and sends its readings as `Msg::Data(value)`; the shell keeps the newest of
//! each type (`shell.get::<T>()`). A part asks for a service by name with
//! `shell.want(name)`; it starts once, however many parts want it.
//!
//! Services so far:
//!   hypr     Hyprland's state (services::hypr::Snapshot): workspaces, the
//!            active window, every window, per monitor; from its event socket
//!   status   battery (status::Battery, UPower) and Wi-Fi (status::Wifi,
//!            NetworkManager), over the system bus
//!   mpris    media players (mpris::Players), session bus
//!   tray     the apps' tray items (tray::Tray, StatusNotifierItem), session bus
//!   bluez    Bluetooth (bluez::Bluetooth), system bus
//! Asked for directly, not started by name:
//!   wallpaper::sample   is the wallpaper light under the bar (wallpaper::WallTop)
//!   bridge   the QML shell's own state, from its lines on stdin (main.rs),
//!            and calls back into it (shell-ipc.sh)
//! (The glass plugin's client, hypr::Glass, starts with the shell itself.)

pub mod bluez;
pub mod bridge;
pub mod hypr;
pub mod mpris;
pub mod status;
pub mod tray;
pub mod wallpaper;

use crate::Msg;
use smithay_client_toolkit::reexports::calloop::channel::Sender;

pub fn start(name: &str, tx: Sender<Msg>) {
    match name {
        "hypr" => hypr::spawn_events(tx),
        "status" => status::spawn(tx),
        "mpris" => mpris::spawn(tx),
        "tray" => tray::spawn(tx),
        "bluez" => bluez::spawn(tx),
        _ => eprintln!("tessera-ui: no service called {name}"),
    }
}

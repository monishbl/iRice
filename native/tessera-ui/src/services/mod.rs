//! The system, read for the surfaces: each service runs on its own thread(s)
//! and sends its readings as `Msg::Data(value)`; the shell keeps the newest of
//! each type (`shell.get::<T>()`). A part asks for a service by name with
//! `shell.want(name)`; it starts once, however many parts want it.
//!
//! Services so far:
//!   hypr     Hyprland's state (services::hypr::Snapshot): workspaces, the
//!            active window, per monitor; from its event socket
//!   status   battery (status::Battery, UPower) and Wi-Fi (status::Wifi,
//!            NetworkManager), over the system bus
//! (The glass plugin's client, hypr::Glass, starts with the shell itself.)

pub mod hypr;
pub mod status;

use crate::Msg;
use smithay_client_toolkit::reexports::calloop::channel::Sender;

pub fn start(name: &str, tx: Sender<Msg>) {
    match name {
        "hypr" => hypr::spawn_events(tx),
        "status" => status::spawn(tx),
        _ => eprintln!("tessera-ui: no service called {name}"),
    }
}

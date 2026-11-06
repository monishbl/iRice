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
//!   clipboard  the clipboard history (clipboard::History: cliphist, the
//!            shell's pins and times) and its pictures (clipboard::Previews)
//!   clients  Hyprland's windows and monitors (clients::Windows): what's open,
//!            where, in front, asking for attention
//!   apps     the installed apps' desktop entries (apps::Apps), and starting them
//!   icons    icon names to files in the shell's icon theme (icons::Icons)
//!   notify   the notification server, org.freedesktop.Notifications on the
//!            session bus (notify::Event; answers through notify::bus())
//!   ai       the AI chat's credentials (ai::keys::Keys, from the keyring);
//!            the chat, its history and its requests are ai::Chat and ai::stream
//!   mpris    media players (mpris::Players), session bus
//!   tray     the apps' tray items (tray::Tray, StatusNotifierItem), session bus
//!   bluez    Bluetooth (bluez::Bluetooth), system bus
//! Asked for directly, not started by name:
//!   wallpaper::sample   is the wallpaper light under the bar (wallpaper::WallTop)
//!   bridge   the QML shell's own state, from its lines on stdin (main.rs),
//!            and calls back into it (shell-ipc.sh)
//!   calendar your events and file subscriptions (calendar::Calendar)
//!   todos    your to-dos on this computer (todos::Todos)
//!   weather  wttr.in's weather (weather::Weather)
//! (The glass plugin's client, hypr::Glass, starts with the shell itself.)

pub mod ai;
pub mod apps;
pub mod audio;
pub mod bluez;
pub mod bridge;
pub mod brightness;
pub mod clients;
pub mod clipboard;
pub mod hypr;
pub mod icons;
pub mod mpris;
pub mod pulse;
pub mod notify;
pub mod status;
pub mod tray;
pub mod wallpaper;
pub mod calendar;
pub mod capture;
pub mod todos;
pub mod weather;

use crate::Msg;
use smithay_client_toolkit::reexports::calloop::channel::Sender;

pub fn start(name: &str, tx: Sender<Msg>) {
    match name {
        "hypr" => hypr::spawn_events(tx),
        "status" => status::spawn(tx),
        "mpris" => mpris::spawn(tx),
        "tray" => tray::spawn(tx),
        "bluez" => bluez::spawn(tx),
        "clipboard" => clipboard::spawn(tx),
        "clients" => clients::spawn(tx),
        "apps" => apps::spawn(tx),
        "icons" => icons::spawn(tx),
        "audio" => audio::spawn(tx),
        "brightness" => brightness::spawn(tx),
        "notify" => notify::spawn(tx),
        "ai" => ai::spawn(tx),
        "calendar" => calendar::spawn(tx),
        "todos" => todos::spawn(tx),
        "weather" => weather::spawn(tx),
        _ => eprintln!("tessera-ui: no service called {name}"),
    }
}

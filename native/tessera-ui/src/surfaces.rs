//! The surfaces this binary has, one crate each (native/tessera-ui/surfaces/),
//! and making their parts by name. A new surface: its crate in the workspace
//! (Cargo.toml's `members` and `[dependencies]`), its name in KNOWN, its line in
//! `make()`. What's drawn and what's said to the shell about it:
//! tessera_core::surfaces.

pub use tessera_core::surfaces::*;

/// The surfaces that exist in Rust so far (each has a QML twin that checks
/// RustUi.drawing with the same name: tests/test_rust_ui.py)
pub const KNOWN: &[&str] = &["menubar", "dock", "emoji", "clipboard", "osd", "background", "wallpaper", "today", "desktop", "notifications", "spotlight", "controlcenter", "sidebar", "screentools", "quicklook", "lock", "session", "polkit", "overlay", "welcome", "resources", "cheatsheet", "osk", "hotcorners", "tiling", "decorations", "share", "appstore", "shelf", "network", "captions", "notes", "settings", "overview", "missioncontrol", "switcher", "stagemanager", "launchpad", "tabs", "downloadsstack", "widgetoptions", "screencorners", "findpointer", "spaceswitch", "displayconnect", "displayoverlay", "shelfprobes"];

/// A surface's part, by name (one line per surface)
pub fn make(name: &str) -> Option<Box<dyn tessera_core::shell::Part>> {
    match name {
        "menubar" => Some(Box::new(tessera_menubar::Menubar::default())),
        "dock" => Some(Box::new(tessera_dock::Dock::default())),
        "emoji" => Some(Box::new(tessera_emoji::Emoji::default())),
        "clipboard" => Some(Box::new(tessera_clipboard::Clipboard::default())),
        "osd" => Some(Box::new(tessera_osd::Osd::default())),
        "notifications" => Some(Box::new(tessera_notifications::Notifications::default())),
        "background" => Some(Box::new(tessera_wallpaper::Background::default())),
        "wallpaper" => Some(Box::new(tessera_wallpaper::WallpaperPicker::default())),
        "today" => Some(Box::new(tessera_today::Today::default())),
        "desktop" => Some(Box::new(tessera_desktop::Desktop::default())),
        "spotlight" => Some(Box::new(tessera_spotlight::Spotlight::default())),
        "controlcenter" => Some(Box::new(tessera_controlcenter::ControlCenter::default())),
        "sidebar" => Some(Box::new(tessera_sidebar::Sidebar::default())),
        "screentools" => Some(Box::new(tessera_screentools::ScreenTools::default())),
        "quicklook" => Some(Box::new(tessera_screentools::quicklook::QuickLook::default())),
        "overview" => Some(Box::new(tessera_overview::Overview::default())),
        "missioncontrol" => Some(Box::new(tessera_missioncontrol::MissionControl::default())),
        "switcher" => Some(Box::new(tessera_switcher::Switcher::default())),
        "stagemanager" => Some(Box::new(tessera_stagemanager::StageManager::default())),
        "launchpad" => Some(Box::new(tessera_launchpad::Launchpad::default())),
        "tabs" => Some(Box::new(tessera_tabs::Tabs::default())),
        // (wave 4b: no QML twin gated by RustUi.drawing; the host's panels and feeds start them)
        "downloadsstack" => Some(Box::new(tessera_dock::DownloadsStack::default())),
        "widgetoptions" => Some(Box::new(tessera_desktop::WidgetOptions::default())),
        "screencorners" => Some(Box::new(tessera_screenfx::ScreenCorners::default())),
        "findpointer" => Some(Box::new(tessera_screenfx::FindPointer::default())),
        "spaceswitch" => Some(Box::new(tessera_screenfx::SpaceSwitch::default())),
        "displayconnect" => Some(Box::new(tessera_screenfx::DisplayConnect::default())),
        "displayoverlay" => Some(Box::new(tessera_screenfx::DisplayOverlay::default())),
        "shelfprobes" => Some(Box::new(tessera_screenfx::ShelfProbes::default())),
        "lock" => Some(Box::new(tessera_lock::Lock::default())),
        "session" => Some(Box::new(tessera_session::Session::default())),
        "polkit" => Some(Box::new(tessera_polkit::Polkit::default())),
        "overlay" => Some(Box::new(tessera_overlay::Overlay::default())),
        "welcome" => Some(Box::new(tessera_welcome::Welcome::default())),
        "resources" => Some(Box::new(tessera_resources::Resources::default())),
        "cheatsheet" => Some(Box::new(tessera_cheatsheet::Cheatsheet::default())),
        "osk" => Some(Box::new(tessera_osk::Osk::default())),
        "hotcorners" => Some(Box::new(tessera_hotcorners::HotCorners::default())),
        "tiling" => Some(Box::new(tessera_tiling::Tiling::default())),
        "decorations" => Some(Box::new(tessera_decorations::Decorations::default())),
        "share" => Some(Box::new(tessera_share::Share::default())),
        "appstore" => Some(Box::new(tessera_appstore::AppStore::default())),
        "shelf" => Some(Box::new(tessera_shelf::Shelf::default())),
        "network" => Some(Box::new(tessera_network::Network::default())),
        "captions" => Some(Box::new(tessera_captions::Captions::default())),
        "notes" => Some(Box::new(tessera_notes::Notes::default())),
        "settings" => Some(Box::new(tessera_settings::Settings::default())),
        _ => None,
    }
}

/// From the arguments, among KNOWN (tessera_core::surfaces::Surfaces::from_args)
pub fn from_args(args: &[String]) -> Surfaces {
    Surfaces::from_args(args, KNOWN)
}

/// The first argument that isn't `--surfaces a,b` / `--surfaces=a,b` (the
/// other options are handled before this), if any
pub fn unknown_arg(args: &[String]) -> Option<&str> {
    let mut it = args.iter().skip(1);
    while let Some(a) = it.next() {
        if a == "--surfaces" {
            it.next();
        } else if a == "--demo" {
            // (the security surfaces without their real roles: tessera_core::env::demo_mode)
        } else if !a.starts_with("--surfaces=") {
            return Some(a);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn args(a: &[&str]) -> Vec<String> {
        std::iter::once("tessera-ui").chain(a.iter().copied()).map(String::from).collect()
    }

    #[test]
    fn every_surface_without_the_option() {
        assert_eq!(from_args(&args(&[])).on, KNOWN.to_vec());
    }

    #[test]
    fn only_the_ones_asked_for() {
        let s = from_args(&args(&["--surfaces", "menubar,nosuch"]));
        assert_eq!(s.on, vec!["menubar"]);
        assert_eq!(s.unknown, vec!["nosuch"]);
        let s = from_args(&args(&["--surfaces=nosuch"]));
        assert!(s.on.is_empty());
        // (in KNOWN's order, whatever order they're asked in)
        let s = from_args(&args(&["--surfaces", "dock,menubar"]));
        assert_eq!(s.on, vec!["menubar", "dock"]);
        let s = from_args(&args(&["--surfaces", ""]));
        assert!(s.on.is_empty() && s.unknown.is_empty());
    }

    #[test]
    fn says_what_it_draws_as_json_lines() {
        let s = from_args(&args(&["--surfaces", "menubar,nosuch"]));
        let lines: Vec<serde_json::Value> = s.ready_lines().iter().map(|l| serde_json::from_str(l).unwrap()).collect();
        assert_eq!(lines[0], json!({"src": "ui", "kind": "unknown", "surface": "nosuch"}));
        assert_eq!(lines[1], json!({"src": "ui", "kind": "ready", "surfaces": ["menubar"]}));
    }

    #[test]
    fn a_held_surface_isnt_ready_yet() {
        let mut s = from_args(&args(&["--surfaces", "menubar,notifications"]));
        s.hold("notifications");
        let last: serde_json::Value = serde_json::from_str(s.ready_lines().last().unwrap()).unwrap();
        assert_eq!(last, json!({"src": "ui", "kind": "ready", "surfaces": ["menubar"]}));
        s.release("notifications");
        let last: serde_json::Value = serde_json::from_str(s.ready_lines().last().unwrap()).unwrap();
        assert_eq!(last, json!({"src": "ui", "kind": "ready", "surfaces": ["menubar", "notifications"]}));
    }

    #[test]
    fn anything_else_on_the_command_line_is_refused() {
        // (before a display is opened: a harness's `--list` never draws)
        assert_eq!(unknown_arg(&args(&[])), None);
        assert_eq!(unknown_arg(&args(&["--surfaces", "menubar,dock"])), None);
        assert_eq!(unknown_arg(&args(&["--surfaces=menubar"])), None);
        assert_eq!(unknown_arg(&args(&["--surfaces"])), None);
        assert_eq!(unknown_arg(&args(&["--list"])), Some("--list"));
        assert_eq!(unknown_arg(&args(&["--surfaces", "lock", "--demo"])), None);
        assert_eq!(unknown_arg(&args(&["--surfaces", "dock", "--format", "terse"])), Some("--format"));
        assert_eq!(unknown_arg(&args(&["menubar"])), Some("menubar"));
    }

    #[test]
    fn every_known_surface_can_be_made() {
        for k in KNOWN {
            assert_eq!(make(k).map(|p| p.name()), Some(*k));
        }
        assert!(make("nothing").is_none());
    }
}

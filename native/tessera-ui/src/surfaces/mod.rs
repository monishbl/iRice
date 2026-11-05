//! Which of the shell's surfaces this process draws, and what it tells the
//! shell about them. The QML shell (shell/services/RustUi.qml) starts it with
//! `--surfaces menubar,…` (Settings' `rust.surfaces`) and hides its own copy
//! of a surface only once this process says it draws it, a JSON line on
//! stdout per event:
//!   {"src":"ui","kind":"ready","surfaces":["menubar"]}   drawing these
//!   {"src":"ui","kind":"unknown","surface":"lock"}       not in Rust yet
//!   {"src":"ui","kind":"osd","shown":true}               a surface's own news (`tell`)
//! If this process ends, the shell draws them again itself.

pub mod clipboard;
pub mod desktop;
pub mod dock;
pub mod emoji;
pub mod menubar;
pub mod notifications;
pub mod osd;
pub mod shapes;
pub mod background;
pub mod today;
pub mod wallpaper;

/// The surfaces that exist in Rust so far (each has a QML twin that checks
/// RustUi.drawing with the same name: tests/test_rust_ui.py)
pub const KNOWN: &[&str] = &["menubar", "dock", "emoji", "clipboard", "osd", "background", "wallpaper", "today", "desktop", "notifications"];

/// A surface's part, by name (one line per surface)
pub fn make(name: &str) -> Option<Box<dyn crate::shell::Part>> {
    match name {
        "menubar" => Some(Box::new(menubar::Menubar::default())),
        "dock" => Some(Box::new(dock::Dock::default())),
        "emoji" => Some(Box::new(emoji::Emoji::default())),
        "clipboard" => Some(Box::new(clipboard::Clipboard::default())),
        "osd" => Some(Box::new(osd::Osd::default())),
        "notifications" => Some(Box::new(notifications::Notifications::default())),
        "background" => Some(Box::new(background::Background::default())),
        "wallpaper" => Some(Box::new(wallpaper::WallpaperPicker::default())),
        "today" => Some(Box::new(today::Today::default())),
        "desktop" => Some(Box::new(desktop::Desktop::default())),
        _ => None,
    }
}

/// A line for the shell (RustUi.qml reads it), from a surface
pub fn tell(v: serde_json::Value) {
    use std::io::Write;
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{v}");
    let _ = out.flush();
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Surfaces {
    /// asked for and known, in KNOWN's order
    pub on: Vec<&'static str>,
    /// asked for, but not in Rust yet
    pub unknown: Vec<String>,
    /// drawn here but not ready to take over yet (left out of "ready")
    pub held: Vec<&'static str>,
}

impl Surfaces {
    /// From the arguments: `--surfaces a,b` (or `--surfaces=a,b`). Without the
    /// option, every known surface (running it by hand draws them all)
    pub fn from_args(args: &[String]) -> Self {
        let mut list: Option<String> = None;
        let mut it = args.iter();
        while let Some(a) = it.next() {
            if let Some(v) = a.strip_prefix("--surfaces=") {
                list = Some(v.to_string());
            } else if a == "--surfaces" {
                list = Some(it.next().cloned().unwrap_or_default());
            }
        }
        let Some(list) = list else {
            return Self { on: KNOWN.to_vec(), unknown: Vec::new(), held: Vec::new() };
        };
        let asked: Vec<&str> = list.split(',').map(str::trim).filter(|s| !s.is_empty()).collect();
        Self {
            on: KNOWN.iter().copied().filter(|k| asked.contains(k)).collect(),
            unknown: asked.iter().filter(|a| !KNOWN.contains(a)).map(|s| s.to_string()).collect(),
            held: Vec::new(),
        }
    }

    /// Not ready to take over: the shell keeps drawing it
    pub fn hold(&mut self, name: &'static str) {
        if !self.held.contains(&name) {
            self.held.push(name);
        }
    }

    pub fn release(&mut self, name: &str) {
        self.held.retain(|h| *h != name);
    }

    /// The lines that say what's drawn (on stdout, for the shell)
    pub fn ready_lines(&self) -> Vec<String> {
        let mut out: Vec<String> = self.unknown.iter().map(|s| serde_json::json!({"src": "ui", "kind": "unknown", "surface": s}).to_string()).collect();
        let ready: Vec<&str> = self.on.iter().copied().filter(|s| !self.held.contains(s)).collect();
        out.push(serde_json::json!({"src": "ui", "kind": "ready", "surfaces": ready}).to_string());
        out
    }
}

/// Say it, once, to whoever started this process
pub fn announce(s: &Surfaces) {
    use std::io::Write;
    let mut out = std::io::stdout().lock();
    for l in s.ready_lines() {
        let _ = writeln!(out, "{l}");
    }
    let _ = out.flush();
}

/// The screens a surface goes on: a list in config.json (`menubar.screens`,
/// `dock.screens`…; empty or missing: every screen), as the QML surfaces' models
pub fn on_screen(list: &serde_json::Value, output: &str) -> bool {
    match list.as_array() {
        Some(list) if !list.is_empty() => list.iter().any(|s| s.as_str() == Some(output)),
        _ => true,
    }
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
        assert_eq!(Surfaces::from_args(&args(&[])).on, KNOWN.to_vec());
    }

    #[test]
    fn only_the_ones_asked_for() {
        let s = Surfaces::from_args(&args(&["--surfaces", "menubar,lock"]));
        assert_eq!(s.on, vec!["menubar"]);
        assert_eq!(s.unknown, vec!["lock"]);
        let s = Surfaces::from_args(&args(&["--surfaces=lock"]));
        assert!(s.on.is_empty());
        // (in KNOWN's order, whatever order they're asked in)
        let s = Surfaces::from_args(&args(&["--surfaces", "dock,menubar"]));
        assert_eq!(s.on, vec!["menubar", "dock"]);
        let s = Surfaces::from_args(&args(&["--surfaces", ""]));
        assert!(s.on.is_empty() && s.unknown.is_empty());
    }

    #[test]
    fn says_what_it_draws_as_json_lines() {
        let s = Surfaces::from_args(&args(&["--surfaces", "menubar,lock"]));
        let lines: Vec<serde_json::Value> = s.ready_lines().iter().map(|l| serde_json::from_str(l).unwrap()).collect();
        assert_eq!(lines[0], json!({"src": "ui", "kind": "unknown", "surface": "lock"}));
        assert_eq!(lines[1], json!({"src": "ui", "kind": "ready", "surfaces": ["menubar"]}));
    }

    #[test]
    fn a_held_surface_isnt_ready_yet() {
        let mut s = Surfaces::from_args(&args(&["--surfaces", "menubar,notifications"]));
        s.hold("notifications");
        let last: serde_json::Value = serde_json::from_str(s.ready_lines().last().unwrap()).unwrap();
        assert_eq!(last, json!({"src": "ui", "kind": "ready", "surfaces": ["menubar"]}));
        s.release("notifications");
        let last: serde_json::Value = serde_json::from_str(s.ready_lines().last().unwrap()).unwrap();
        assert_eq!(last, json!({"src": "ui", "kind": "ready", "surfaces": ["menubar", "notifications"]}));
    }

    #[test]
    fn surfaces_follow_a_screens_setting() {
        assert!(on_screen(&json!(null), "DP-1"));
        assert!(on_screen(&json!([]), "DP-1"));
        assert!(on_screen(&json!(["DP-1"]), "DP-1"));
        assert!(!on_screen(&json!(["DP-1"]), "HDMI-A-1"));
    }

    #[test]
    fn every_known_surface_can_be_made() {
        for k in KNOWN {
            assert_eq!(make(k).map(|p| p.name()), Some(*k));
        }
        assert!(make("nothing").is_none());
    }
}

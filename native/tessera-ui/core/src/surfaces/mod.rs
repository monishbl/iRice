//! Which of the shell's surfaces this process draws, and what it tells the
//! shell about them (the list of surfaces itself, KNOWN, and making their
//! parts are the binary's: src/surfaces.rs). The QML shell
//! (shell/services/RustUi.qml) starts it with `--surfaces menubar,…` (Settings' `rust.surfaces`) and hides its own copy
//! of a surface only once this process says it draws it, a JSON line on
//! stdout per event:
//!   {"src":"ui","kind":"ready","surfaces":["menubar"]}   drawing these
//!   {"src":"ui","kind":"unknown","surface":"lock"}       not in Rust yet
//!   {"src":"ui","kind":"osd","shown":true}               a surface's own news (`tell`)
//! If this process ends, the shell draws them again itself.

pub mod shapes;
// (polar outlines that morph, and pictures cut to a shape: the lock, the session menu, polkit)
pub mod polar;
pub mod events; // (Hyprland's socket2 lines a part follows itself: tiling, decorations)
pub mod kit; // (small helpers: overlay, welcome, resources, cheatsheet, osk, hotcorners, tiling, decorations)
pub mod zones; // (Tiling.qml's layouts: tiling's preview, decorations' tile grid)

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Surfaces {
    /// asked for and known, in `known`'s order (the binary's KNOWN)
    pub on: Vec<&'static str>,
    /// asked for, but not in Rust yet
    pub unknown: Vec<String>,
    /// drawn here but not ready to take over yet (left out of "ready")
    pub held: Vec<&'static str>,
}

impl Surfaces {
    /// From the arguments: `--surfaces a,b` (or `--surfaces=a,b`). Without the
    /// option, every known surface (running it by hand draws them all)
    pub fn from_args(args: &[String], known: &[&'static str]) -> Self {
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
            return Self { on: known.to_vec(), unknown: Vec::new(), held: Vec::new() };
        };
        let asked: Vec<&str> = list.split(',').map(str::trim).filter(|s| !s.is_empty()).collect();
        Self {
            on: known.iter().copied().filter(|k| asked.contains(k)).collect(),
            unknown: asked.iter().filter(|a| !known.contains(a)).map(|s| s.to_string()).collect(),
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

/// One event for the shell, a JSON line on stdout (`{"src":"ui","kind":…}`)
pub fn tell(event: serde_json::Value) {
    use std::io::Write;
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{event}");
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

    #[test]
    fn surfaces_follow_a_screens_setting() {
        assert!(on_screen(&json!(null), "DP-1"));
        assert!(on_screen(&json!([]), "DP-1"));
        assert!(on_screen(&json!(["DP-1"]), "DP-1"));
        assert!(!on_screen(&json!(["DP-1"]), "HDMI-A-1"));
    }
}

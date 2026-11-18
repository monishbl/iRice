//! A popover closes on a press anywhere, on any screen.
//!
//! Each popover (Control Center, the Notification Center, the status menus,
//! a dock or menu bar menu…) covers only its own output with a window that
//! takes every press there: a press outside the panel on that screen closes
//! it. A press on another screen reached an app, or another screen's bar or
//! dock, and the popover stayed open (18 Nov, two screens).
//!
//! So, while one is open: the screens without a popover get a transparent
//! catcher over their apps (screenfx's `clickcatcher` part: no keyboard, the
//! whole screen bar the bars' reserved strips; gone the moment the popover
//! closes), and a press on any of the shell's surfaces on another output (a
//! catcher, a bar, a dock, the desktop) closes the popovers that aren't on it.
//! The press that closes is taken, as one outside the panel on its own
//! screen is; nothing else is blocked once it closes.
//!
//! What is open: Panels' open panel (the shell state's `open` and `screen`,
//! for the panels in `PANELS`), and the parts' own menus, said here with
//! `open` / `close`. A part's menu closed this way hears `Elsewhere` (a
//! `Part::message`).

use std::cell::RefCell;

/// The part that keeps the catchers (surfaces/screenfx catcher.rs)
pub const CATCHER: &str = "clickcatcher";

/// Panels' panels that close on a press outside them (the others are whole
/// screen modes, windows, or close only on Escape)
pub const PANELS: &[&str] = &[
    "controlCenter",
    "notificationCenter",
    "bluetoothMenu",
    "soundMenu",
    "privacyMenu",
    "batteryMenu",
    "resourcesMenu",
    "weatherMenu",
    "nowPlaying",
    "wifiMenu",
    "session",
    "about",
    "clipboard",
    "spotlight",
    "emoji",
    "quickNote",
    "share",
    "cheatsheet",
    "sidebar",
    "systemMenu",
    "windowMenu",
    "widgetOptions",
];

/// The owner name of Panels' open panel among the popovers
pub const PANELS_OWNER: &str = "panels";

/// To a part whose menu a press on another screen closed
pub struct Elsewhere;

thread_local! {
    static OPEN: RefCell<Vec<(&'static str, String)>> = const { RefCell::new(Vec::new()) };
}

/// A part's popover is open on `output` (again: it moved there)
pub fn open(tx: &calloop::channel::Sender<crate::Msg>, owner: &'static str, output: &str) {
    let changed = OPEN.with(|o| {
        let mut o = o.borrow_mut();
        if o.iter().any(|(w, out)| *w == owner && out == output) {
            return false;
        }
        o.retain(|(w, _)| *w != owner);
        o.push((owner, output.to_string()));
        true
    });
    if changed {
        let _ = tx.send(crate::Msg::Part(CATCHER, Box::new(Changed)));
    }
}

/// A part's popover closed (however: a pick, Escape, a press)
pub fn close(tx: &calloop::channel::Sender<crate::Msg>, owner: &'static str) {
    let changed = OPEN.with(|o| {
        let mut o = o.borrow_mut();
        let n = o.len();
        o.retain(|(w, _)| *w != owner);
        n != o.len()
    });
    if changed {
        let _ = tx.send(crate::Msg::Part(CATCHER, Box::new(Changed)));
    }
}

/// The parts' popovers open now: (owner, output)
pub fn parts_open() -> Vec<(&'static str, String)> {
    OPEN.with(|o| o.borrow().clone())
}

/// To the catcher part: what's open changed
pub struct Changed;

/// Panels' open panel as a popover: (owner, output), when it's one that
/// closes on a press outside it
pub fn panel_popover(open: &str, screen: &str) -> Option<(&'static str, String)> {
    (PANELS.contains(&open) && !screen.is_empty()).then(|| (PANELS_OWNER, screen.to_string()))
}

/// The outputs that get a catcher: those with no popover, while any is open
pub fn catch_on(popovers: &[(&'static str, String)], outputs: &[String]) -> Vec<String> {
    if popovers.is_empty() {
        return Vec::new();
    }
    outputs.iter().filter(|o| !popovers.iter().any(|(_, p)| p == *o)).cloned().collect()
}

/// A press on one of the shell's surfaces on `on`: the popovers it closes
/// (those on other outputs; a surface on no output, a popup or a window,
/// closes none: its own parent decides)
pub fn closed_by(popovers: &[(&'static str, String)], on: &str) -> Vec<&'static str> {
    if on.is_empty() {
        return Vec::new();
    }
    let mut v: Vec<&'static str> = popovers.iter().filter(|(_, p)| p != on).map(|(w, _)| *w).collect();
    v.dedup();
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outs() -> Vec<String> {
        vec!["eDP-1".to_string(), "DP-1".to_string()]
    }

    #[test]
    fn catchers_only_while_open_and_only_elsewhere() {
        assert!(catch_on(&[], &outs()).is_empty());
        assert_eq!(catch_on(&[("panels", "DP-1".into())], &outs()), vec!["eDP-1".to_string()]);
        // (a popover on each screen: every press lands on one of them)
        assert!(catch_on(&[("panels", "DP-1".into()), ("dock", "eDP-1".into())], &outs()).is_empty());
        // (one screen: nothing to catch)
        assert!(catch_on(&[("panels", "DP-1".into())], &["DP-1".to_string()]).is_empty());
    }

    #[test]
    fn a_press_closes_what_is_on_other_screens() {
        let p = vec![("panels", "DP-1".to_string()), ("dock", "eDP-1".to_string())];
        assert_eq!(closed_by(&p, "eDP-1"), vec!["panels"]);
        assert_eq!(closed_by(&p, "DP-1"), vec!["dock"]);
        // (its own screen: the popover's own window decides, as before)
        assert!(closed_by(&[("panels", "DP-1".to_string())], "DP-1").is_empty());
        // (a popup or a window has no output: none)
        assert!(closed_by(&p, "").is_empty());
    }

    #[test]
    fn panels_that_close_on_an_outside_press() {
        assert_eq!(panel_popover("controlCenter", "DP-1"), Some(("panels", "DP-1".to_string())));
        assert_eq!(panel_popover("notificationCenter", "eDP-1").map(|p| p.1), Some("eDP-1".to_string()));
        assert!(panel_popover("soundMenu", "DP-1").is_some());
        assert!(panel_popover("wifiMenu", "DP-1").is_some());
        assert!(panel_popover("session", "DP-1").is_some());
        assert!(panel_popover("clipboard", "DP-1").is_some());
        assert!(panel_popover("spotlight", "DP-1").is_some());
        // (whole-screen modes stay until left their own way)
        for mode in ["launchpad", "overview", "missionControl", "switcher", "screenTools", ""] {
            assert_eq!(panel_popover(mode, "DP-1"), None, "{mode}");
        }
        assert_eq!(panel_popover("controlCenter", ""), None);
    }

    #[test]
    fn the_registry_keeps_one_entry_per_part() {
        let (tx, _rx) = calloop::channel::channel();
        open(&tx, "dock", "DP-1");
        open(&tx, "dock", "eDP-1");
        open(&tx, "menubar", "DP-1");
        assert_eq!(parts_open(), vec![("dock", "eDP-1".to_string()), ("menubar", "DP-1".to_string())]);
        close(&tx, "dock");
        close(&tx, "dock");
        assert_eq!(parts_open(), vec![("menubar", "DP-1".to_string())]);
        close(&tx, "menubar");
        assert!(parts_open().is_empty());
    }
}

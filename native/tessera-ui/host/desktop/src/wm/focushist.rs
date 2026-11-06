//! FocusHistory.qml and FocusDecision.js. The history keeps windows in the
//! order they were last focused, across every space and monitor, from
//! Hyprland's activewindowv2 and closewindow events (addresses without "0x",
//! lower case); windows it hasn't seen follow by Hyprland's focusHistoryID.
//! The decisions behind focus that follows the pointer (Mouse.qml, in
//! host/system) are pure here, `after_events` and `on_urgent`.

use super::hx;
use tessera_core::services::clients::Client;

#[derive(Default)]
pub struct FocusHistory {
    /// addresses, most recently focused first
    pub order: Vec<String>,
}

const LIMIT: usize = 200;

impl FocusHistory {
    pub fn touch(&mut self, address: &str) {
        let k = hx::key(address);
        if k.is_empty() || self.order.first() == Some(&k) {
            return;
        }
        self.order.retain(|a| *a != k);
        self.order.insert(0, k);
        self.order.truncate(LIMIT);
    }
    pub fn forget(&mut self, address: &str) {
        let k = hx::key(address);
        self.order.retain(|a| *a != k);
    }

    /// The windows, most recently focused first; unseen ones follow by
    /// focusHistoryID (0 = latest), then in Hyprland's order
    pub fn sorted<'a>(&self, clients: &'a [Client]) -> Vec<&'a Client> {
        let mut v: Vec<(usize, Option<usize>, i64, &Client)> = clients
            .iter()
            .enumerate()
            .map(|(i, c)| (i, self.order.iter().position(|a| *a == hx::key(&c.address)), c.focus_history_id as i64, c))
            .collect();
        v.sort_by(|a, b| {
            let ra = a.1.unwrap_or(usize::MAX);
            let rb = b.1.unwrap_or(usize::MAX);
            if a.1.is_some() || b.1.is_some() {
                ra.cmp(&rb).then(a.2.cmp(&b.2)).then(a.0.cmp(&b.0))
            } else {
                a.2.cmp(&b.2).then(a.0.cmp(&b.0))
            }
        });
        v.into_iter().map(|x| x.3).collect()
    }

    pub fn event(&mut self, name: &str, data: &str) {
        // activewindowv2>>ADDRESS (empty or "," when nothing is focused)
        match name {
            "activewindowv2" => {
                let a = data.trim();
                if !a.is_empty() && a != "," {
                    self.touch(a);
                }
            }
            "closewindow" => self.forget(data),
            _ => {}
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum Decision {
    /// a window just opened and has focus: the pointer moves onto it
    Warp(String),
    /// the scene changed under a still pointer: focus goes to what's under it again
    Refocus,
    None,
}

const SCENE_CHANGES: &[&str] = &["workspace", "workspacev2", "activespecial", "activespecialv2", "closewindow", "movewindowv2", "changefloatingmode"];

/// After a burst of Hyprland events (name, data), decided once
pub fn after_events(events: &[(String, String)]) -> Decision {
    let mut opened = String::new();
    let mut changed = false;
    for (name, data) in events {
        if name == "openwindow" {
            let address = data.split(',').next().unwrap_or("");
            let hex = address.trim_start_matches("0x");
            if !hex.is_empty() && hex.chars().all(|c| c.is_ascii_hexdigit()) {
                opened = hx::addr(address);
            }
        } else if SCENE_CHANGES.contains(&name.as_str()) {
            changed = true;
        }
    }
    if !opened.is_empty() {
        Decision::Warp(opened)
    } else if changed {
        Decision::Refocus
    } else {
        Decision::None
    }
}

#[derive(Debug, PartialEq)]
pub enum Urgent {
    None,
    Bring,
    Notify,
}

/// A window asking for attention: right after the user clicked or typed it's
/// their own request (bring it forward); otherwise just show it wants attention
pub fn on_urgent(input_age_ms: Option<f64>, already_active: bool) -> Urgent {
    if already_active {
        return Urgent::None;
    }
    match input_age_ms {
        Some(a) if (0.0..=3000.0).contains(&a) => Urgent::Bring,
        _ => Urgent::Notify,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(a: &str, h: i32) -> Client {
        Client { address: a.into(), focus_history_id: h, ..Default::default() }
    }

    #[test]
    fn recently_focused_first() {
        let mut f = FocusHistory::default();
        f.touch("0xAA");
        f.touch("bb");
        f.touch("bb");
        assert_eq!(f.order, vec!["bb", "aa"]);
        let cs = [c("0xcc", 1), c("0xaa", 5), c("0xbb", 4), c("0xdd", 0)];
        let s: Vec<&str> = f.sorted(&cs).iter().map(|c| c.address.as_str()).collect();
        assert_eq!(s, vec!["0xbb", "0xaa", "0xdd", "0xcc"]);
        f.forget("0xbb");
        assert_eq!(f.order, vec!["aa"]);
    }

    #[test]
    fn a_new_window_warps_and_a_scene_change_refocuses() {
        let e = |n: &str, d: &str| (n.to_string(), d.to_string());
        assert_eq!(after_events(&[e("openwindow", "5a1,1,foot,foot"), e("workspace", "1")]), Decision::Warp("0x5a1".into()));
        assert_eq!(after_events(&[e("closewindow", "5a1")]), Decision::Refocus);
        assert_eq!(after_events(&[e("activewindow", "x")]), Decision::None);
    }

    #[test]
    fn urgent_windows_come_forward_only_after_input() {
        assert_eq!(on_urgent(Some(500.0), false), Urgent::Bring);
        assert_eq!(on_urgent(Some(9000.0), false), Urgent::Notify);
        assert_eq!(on_urgent(None, false), Urgent::Notify);
        assert_eq!(on_urgent(Some(10.0), true), Urgent::None);
    }
}

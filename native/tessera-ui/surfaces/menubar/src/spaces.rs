//! The spaces on one display's bar, as shell/modules/menubar/Workspaces.qml
//! and shell/services/SpaceMap.js list them: each display owns a block of
//! 100 ids (its base, from the QML Spaces service: `spaces.bases`); its own
//! spaces (at least menubar.workspaceCount, no gaps), then spaces of an
//! unplugged display staying here, then (menubar.otherSpaces) the other
//! displays' spaces, each group after the first led by its display's name.
//! More own spaces than the count: a window of chips that slides (arrows), or
//! (dynamic) chips that shrink and fade past its ends.

use crate::services::hypr::Snapshot;
use std::collections::BTreeMap;

// (each display's block of spaces and its groups: tessera-core's spacemap, which
// the Overview, Mission Control and the switcher read too)
pub use tessera_core::spacemap::{base_of, groups, Group, Kind, STRIDE};

/// One chip on the strip
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Chip {
    pub id: i32,
    pub active: bool,
    /// lives on another display
    pub elsewhere: bool,
    /// …and is that display's current space
    pub shown_there: bool,
    pub occupied: bool,
    /// its apps' classes (up to three, for the icons style)
    pub apps: Vec<String>,
    /// the name of the display this group belongs to, in front of its first chip
    pub mark: String,
    /// dynamic overflow: how far past the window (0 inside; 3+ hidden)
    pub past: i32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Arrow {
    pub more: bool,
    /// something past it has windows
    pub hot: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Strip {
    /// the chips in order: the window over this display's own, then the others'
    pub chips: Vec<Chip>,
    /// arrows around the own chips (Some when they overflow, in arrows mode)
    pub arrows: Option<(Arrow, Arrow)>,
    /// how many of `chips` come before the right arrow
    pub own_shown: usize,
    pub start: i32,
}

/// The strip for one display. `start`: where the window over its own spaces
/// was (kept so the current space stays in view as Workspaces.keepActive)
pub fn strip(name: &str, groups: &[Group], h: &Snapshot, cap: i32, dynamic: bool, start: i32) -> Strip {
    let cap = cap.max(1);
    let own: Vec<i32> = groups.first().map(|g| g.ids.clone()).unwrap_or_default();
    let rest: Vec<i32> = groups.iter().skip(1).flat_map(|g| g.ids.clone()).collect();
    let mut marks: BTreeMap<i32, String> = BTreeMap::new();
    for g in groups.iter().skip(1) {
        if let (Some(&first), false) = (g.ids.first(), g.kind == Kind::Guest) {
            marks.insert(first, g.label.clone());
        }
    }
    let active = h.active.get(name).copied().unwrap_or(0);
    let visible: Vec<i32> = h.active.values().copied().filter(|&i| i > 0).collect();
    let monitor_of: BTreeMap<i32, &String> = h.workspaces.iter().map(|w| (w.id, &w.monitor)).collect();
    let occupied = |id: i32| h.workspaces.iter().any(|w| w.id == id && w.windows > 0) || h.clients.iter().any(|c| c.workspace.id == id);
    let overflow = own.len() as i32 > cap;
    // keep the current space in view
    let clamp = |v: i32| v.max(0).min((own.len() as i32 - cap).max(0));
    let mut st = clamp(start);
    if let Some(i) = own.iter().position(|&x| x == active).map(|i| i as i32) {
        if i < st {
            st = i;
        } else if i >= st + cap {
            st = i - cap + 1;
        }
    }
    let st = clamp(st);
    let in_page = |i: usize| (i as i32) >= st && (i as i32) < st + cap;
    let chip = |id: i32, past: i32| {
        let mon = monitor_of.get(&id).map(|m| m.as_str()).unwrap_or("");
        let elsewhere = !mon.is_empty() && mon != name;
        let mut apps: Vec<String> = Vec::new();
        for c in h.classes_on(id) {
            if !apps.contains(&c) && apps.len() < 3 {
                apps.push(c);
            }
        }
        Chip {
            id,
            active: id == active,
            elsewhere,
            shown_there: elsewhere && h.active.get(mon).copied() == Some(id),
            occupied: occupied(id),
            apps,
            mark: marks.get(&id).cloned().unwrap_or_default(),
            past,
        }
    };
    let mut chips = Vec::new();
    if !overflow || dynamic {
        for (i, &id) in own.iter().enumerate() {
            let past = if !overflow || !dynamic || visible.contains(&id) {
                0
            } else if (i as i32) < st {
                st - i as i32
            } else {
                (i as i32 - (st + cap - 1)).max(0)
            };
            chips.push(chip(id, past));
        }
    } else {
        for (i, &id) in own.iter().enumerate() {
            if in_page(i) || visible.contains(&id) {
                chips.push(chip(id, 0));
            }
        }
    }
    let own_shown = chips.len();
    let arrows = (overflow && !dynamic).then(|| {
        let hidden = |range: &mut dyn Iterator<Item = (usize, &i32)>| -> Vec<i32> { range.filter(|(_, id)| !visible.contains(id)).map(|(_, &id)| id).collect() };
        let left = hidden(&mut own.iter().enumerate().filter(|(i, _)| (*i as i32) < st));
        let right = hidden(&mut own.iter().enumerate().filter(|(i, _)| (*i as i32) >= st + cap));
        (
            Arrow { more: !left.is_empty(), hot: left.iter().any(|&i| occupied(i)) },
            Arrow { more: !right.is_empty(), hot: right.iter().any(|&i| occupied(i)) },
        )
    });
    for id in rest {
        chips.push(chip(id, 0));
    }
    Strip { chips, arrows, own_shown, start: st }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::hypr::{Client, Workspace, WsRef};

    fn ws(id: i32, monitor: &str, windows: u32) -> Workspace {
        Workspace { id, monitor: monitor.into(), windows, ..Default::default() }
    }
    fn snap(w: Vec<Workspace>, active: &[(&str, i32)]) -> Snapshot {
        Snapshot { workspaces: w, active: active.iter().map(|(n, i)| (n.to_string(), *i)).collect(), ..Default::default() }
    }

    #[test]
    fn at_least_the_count_and_no_gaps() {
        let h = snap(vec![ws(1, "DP-1", 2), ws(7, "DP-1", 1)], &[("DP-1", 7)]);
        let g = groups("DP-1", &BTreeMap::new(), &h, 5, true, &BTreeMap::new());
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].ids, vec![1, 2, 3, 4, 5, 6, 7]);
    }

    #[test]
    fn each_display_its_own_block_and_the_others_after() {
        let bases = BTreeMap::from([("eDP-1".to_string(), 0), ("DP-1".to_string(), 100)]);
        let labels = BTreeMap::from([(0, "Built-in".to_string()), (100, "U2720Q".to_string())]);
        let h = snap(vec![ws(1, "eDP-1", 1), ws(2, "eDP-1", 0), ws(101, "DP-1", 3), ws(103, "DP-1", 1), ws(205, "eDP-1", 1)], &[("eDP-1", 1), ("DP-1", 101)]);
        let g = groups("eDP-1", &bases, &h, 3, true, &labels);
        assert_eq!(g[0].ids, vec![1, 2, 3]);
        // a space of an unplugged display (base 200) staying here
        assert_eq!((g[1].kind.clone(), g[1].label.as_str(), g[1].ids.clone()), (Kind::Guest, "Display 3", vec![205]));
        assert_eq!((g[2].kind.clone(), g[2].label.as_str(), g[2].ids.clone()), (Kind::Other, "U2720Q", vec![101, 103]));
        // without otherSpaces, only its own (and guests)
        assert_eq!(groups("eDP-1", &bases, &h, 3, false, &labels).len(), 2);
        // one of its own sitting on the other display isn't an empty slot here
        let h = snap(vec![ws(2, "DP-1", 1)], &[("eDP-1", 1), ("DP-1", 2)]);
        let g = groups("eDP-1", &bases, &h, 3, true, &labels);
        assert_eq!(g[0].ids, vec![1, 3]);
    }

    #[test]
    fn chips_mark_the_other_displays() {
        let bases = BTreeMap::from([("eDP-1".to_string(), 0), ("DP-1".to_string(), 100)]);
        let labels = BTreeMap::from([(100, "U2720Q".to_string())]);
        let mut h = snap(vec![ws(1, "eDP-1", 1), ws(101, "DP-1", 2), ws(102, "DP-1", 0)], &[("eDP-1", 1), ("DP-1", 101)]);
        h.clients = vec![
            Client { class: "firefox".into(), workspace: WsRef { id: 1 }, ..Default::default() },
            Client { class: "kitty".into(), workspace: WsRef { id: 1 }, ..Default::default() },
            Client { class: "firefox".into(), workspace: WsRef { id: 1 }, ..Default::default() },
        ];
        let g = groups("eDP-1", &bases, &h, 2, true, &labels);
        let s = strip("eDP-1", &g, &h, 2, false, 0);
        let ids: Vec<i32> = s.chips.iter().map(|c| c.id).collect();
        assert_eq!(ids, vec![1, 2, 101, 102]);
        assert!(s.chips[0].active && s.chips[0].occupied && s.chips[0].apps == vec!["firefox", "kitty"]);
        assert_eq!(s.chips[2].mark, "U2720Q");
        assert!(s.chips[2].elsewhere && s.chips[2].shown_there && !s.chips[3].shown_there);
        assert!(s.arrows.is_none());
    }

    #[test]
    fn overflow_slides_a_window_with_arrows() {
        let h = snap((1..=9).map(|i| ws(i, "DP-1", (i == 2 || i == 9) as u32)).collect(), &[("DP-1", 7)]);
        let g = groups("DP-1", &BTreeMap::new(), &h, 5, true, &BTreeMap::new());
        let s = strip("DP-1", &g, &h, 5, false, 0);
        // the current space (7) pulls the window to 3..7
        assert_eq!(s.start, 2);
        assert_eq!(s.chips.iter().map(|c| c.id).collect::<Vec<_>>(), vec![3, 4, 5, 6, 7]);
        let (l, r) = s.arrows.clone().unwrap();
        assert!(l.more && l.hot, "1 and 2 are left of it, 2 has windows");
        assert!(r.more && r.hot, "8 and 9, 9 has windows");
        // the arrows move it (and it stays within the spaces)
        let s2 = strip("DP-1", &g, &h, 5, false, 99);
        assert_eq!(s2.start, 4);
    }

    #[test]
    fn dynamic_overflow_shrinks_past_the_ends() {
        let h = snap((1..=9).map(|i| ws(i, "DP-1", 0)).collect(), &[("DP-1", 1)]);
        let g = groups("DP-1", &BTreeMap::new(), &h, 4, true, &BTreeMap::new());
        let s = strip("DP-1", &g, &h, 4, true, 0);
        let past: Vec<i32> = s.chips.iter().map(|c| c.past).collect();
        assert_eq!(past, vec![0, 0, 0, 0, 1, 2, 3, 4, 5]);
        assert!(s.arrows.is_none());
    }
}

//! Each display's block of spaces, as shell/services/SpaceMap.js lists them
//! (moved here from the menu bar's spaces.rs, which re-exports it): every
//! display owns a block of 100 ids (its base, from the QML Spaces service:
//! `spaces.bases`); its own spaces (at least a count, no gaps), then spaces of
//! an unplugged display staying here, then (with `others`) the other
//! displays' spaces. The menu bar, the Overview, Mission Control and the
//! switcher read it.

use crate::services::hypr::Snapshot;
use std::collections::BTreeMap;

pub const STRIDE: i32 = 100;

pub fn base_of(id: i32) -> i32 {
    id.div_euclid(STRIDE) * STRIDE
}

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Own,
    Guest,
    Other,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub kind: Kind,
    pub base: i32,
    pub label: String,
    pub ids: Vec<i32>,
}

/// SpaceMap.groups: this display's spaces, its guests', the others'
pub fn groups(name: &str, bases: &BTreeMap<String, i32>, h: &Snapshot, count: i32, others: bool, labels: &BTreeMap<i32, String>) -> Vec<Group> {
    let single;
    let bases = if bases.is_empty() {
        single = BTreeMap::from([(name.to_string(), 0)]);
        &single
    } else {
        bases
    };
    let base = bases.get(name).copied().unwrap_or(0);
    let connected: BTreeMap<i32, &String> = bases.iter().map(|(n, b)| (*b, n)).collect();
    let (mut own, mut away) = (std::collections::BTreeSet::new(), std::collections::BTreeSet::new());
    let mut guests: BTreeMap<i32, std::collections::BTreeSet<i32>> = BTreeMap::new();
    let mut other: BTreeMap<i32, std::collections::BTreeSet<i32>> = BTreeMap::new();
    for w in &h.workspaces {
        if w.id <= 0 {
            continue;
        }
        let b = base_of(w.id);
        let elsewhere = !w.monitor.is_empty() && w.monitor != name && bases.contains_key(&w.monitor);
        if b == base {
            if elsewhere {
                away.insert(w.id);
            } else {
                own.insert(w.id);
            }
        } else if w.monitor == name {
            guests.entry(b).or_default().insert(w.id);
        } else if others && connected.contains_key(&b) {
            other.entry(b).or_default().insert(w.id);
        }
    }
    for i in 1..=count.max(0) {
        if !away.contains(&(base + i)) {
            own.insert(base + i);
        }
    }
    if let Some(&act) = h.active.get(name) {
        if act > 0 && base_of(act) == base {
            own.insert(act);
        }
    }
    // no gaps: an empty space between two in use stays reachable
    let top = own.iter().copied().max().unwrap_or(base + 1).max(base + 1);
    for id in base + 1..top {
        if !away.contains(&id) {
            own.insert(id);
        }
    }
    let mut out = vec![Group { kind: Kind::Own, base, label: labels.get(&base).cloned().unwrap_or_default(), ids: own.into_iter().collect() }];
    for (b, ids) in guests {
        out.push(Group { kind: Kind::Guest, base: b, label: labels.get(&b).cloned().unwrap_or_else(|| format!("Display {}", b / STRIDE + 1)), ids: ids.into_iter().collect() });
    }
    for (b, ids) in other {
        out.push(Group { kind: Kind::Other, base: b, label: labels.get(&b).cloned().unwrap_or_default(), ids: ids.into_iter().collect() });
    }
    out
}


//! Notification Center's logic (NotificationCenter.qml, NotificationShade.qml,
//! NotificationGroups.js, CalendarWidget.qml's dates): the groups, the rows
//! each look lists, and the calendar's days. Pure functions; the window is in
//! mod.rs, the drawing in ui/notification-center.slint.

use super::model::{self, Note};
use serde::Deserialize;
use std::collections::HashSet;

/// One of the island's alerts in the Tessera group (Island.history), as the
/// shell sends it: everything a card shows, already worded
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Alert {
    pub hid: i64,
    /// its caption ("Bluetooth", "Battery"…: NotificationGroups.kindOf)
    pub kind: String,
    /// the Phosphor glyph and its colour ("#rrggbb")
    pub glyph: String,
    pub color: String,
    /// a device's picture instead of the glyph (a file)
    pub picture: String,
    pub title: String,
    /// the detail line (subtitle · battery · times)
    pub detail: String,
    /// ms since the epoch
    pub at: i64,
    /// its own button's label (Open…), "" none
    pub act: String,
}

/// A group: an app's notifications (newest first), or the island's alerts
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub app: String,
    pub island: bool,
    /// note ids, or alert hids
    pub items: Vec<i64>,
    pub at: i64,
}

impl Group {
    /// What Notification Center remembers it open by
    pub fn key(&self) -> String {
        if self.island {
            format!("island:{}", self.app)
        } else {
            self.app.clone()
        }
    }
}

/// NotificationGroups.byApp: by app, newest group first; the alerts one more
/// group ("Tessera"), placed by its newest. `notes` oldest first, `alerts`
/// newest first (Island.history)
pub fn groups(notes: &[Note], alerts: &[Alert]) -> Vec<Group> {
    let mut out: Vec<Group> = model::groups(notes)
        .into_iter()
        .map(|(app, items)| Group { app, island: false, at: items.first().map_or(0, |n| n.at), items: items.iter().map(|n| n.id as i64).collect() })
        .collect();
    if let Some(first) = alerts.first() {
        let i = out.iter().position(|g| g.at < first.at).unwrap_or(out.len());
        out.insert(i, Group { app: "Tessera".into(), island: true, items: alerts.iter().map(|a| a.hid).collect(), at: first.at });
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Note,
    Alert,
    /// an open group's name, Clear and Show less
    Header,
}

/// A row of the list
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub kind: Kind,
    /// its group (index into `groups`)
    pub group: usize,
    /// the note's id or the alert's hid (a header: the group's newest)
    pub item: i64,
    /// how many its card stands for: the group's size when collapsed, else 1
    pub count: usize,
    /// its group is open
    pub open: bool,
    /// Glass: a collapsed stack's edges peeking below (0..=2)
    pub edges: usize,
    /// Material: the ends of a block of joined cards (round there, tight
    /// between)
    pub first: bool,
    pub last: bool,
}

/// The rows (NotificationCenter.qml's Repeaters; the shade's `entries`): a
/// collapsed group is its newest card (with its count; Glass, two edges
/// peeking); an open one is a header and every card. Material: an open group
/// is a block of its own; everything else joins into blocks between them.
pub fn entries(groups: &[Group], open: &HashSet<String>) -> Vec<Entry> {
    let mut out: Vec<(Entry, usize)> = Vec::new();
    let (mut block, mut last_open) = (0usize, false);
    for (gi, g) in groups.iter().enumerate() {
        let kind = if g.island { Kind::Alert } else { Kind::Note };
        let n = g.items.len();
        let is_open = n > 1 && open.contains(&g.key());
        let row = |kind, item, count, edges| Entry { kind, group: gi, item, count, open: is_open, edges, first: false, last: false };
        if is_open {
            block += 1;
            out.push((row(Kind::Header, g.items[0], n, 0), block));
            for &it in &g.items {
                out.push((row(kind, it, 1, 0), block));
            }
        } else {
            if last_open || out.is_empty() {
                block += 1;
            }
            out.push((row(kind, g.items[0], n, (n - 1).min(2)), block));
        }
        last_open = is_open;
    }
    let blocks: Vec<usize> = out.iter().map(|e| e.1).collect();
    out.into_iter()
        .enumerate()
        .map(|(i, (mut e, b))| {
            e.first = i == 0 || blocks[i - 1] != b;
            e.last = i + 1 == blocks.len() || blocks[i + 1] != b;
            e
        })
        .collect()
}

// ── The calendar's dates ────────────────────────────────────────────────────

/// A day: (year, month 1..=12, day 1..=31)
pub type Day = (i32, u32, u32);

/// Days since 1970-01-01 (Howard Hinnant's days_from_civil)
pub fn days_from_civil((y, m, d): Day) -> i64 {
    let y = if m <= 2 { y - 1 } else { y } as i64;
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

pub fn civil_from_days(z: i64) -> Day {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    ((yoe + era * 400 + if m <= 2 { 1 } else { 0 }) as i32, m, d)
}

pub fn add_days(d: Day, n: i64) -> Day {
    civil_from_days(days_from_civil(d) + n)
}

/// 0 Monday … 6 Sunday
pub fn weekday(d: Day) -> u32 {
    ((days_from_civil(d) + 3).rem_euclid(7)) as u32
}

/// The month's grid: 42 days from the Monday on or before the 1st
pub fn month_grid(y: i32, m: u32) -> Vec<Day> {
    let first = (y, m, 1);
    let start = add_days(first, -(weekday(first) as i64));
    (0..42).map(|i| add_days(start, i)).collect()
}

/// The week (Monday first) a day is in
pub fn week_of(d: Day) -> Vec<Day> {
    let start = add_days(d, -(weekday(d) as i64));
    (0..7).map(|i| add_days(start, i)).collect()
}

/// A month on: (year, month) `n` months from this one
pub fn add_months((y, m): (i32, u32), n: i32) -> (i32, u32) {
    let k = y * 12 + m as i32 - 1 + n;
    (k.div_euclid(12), (k.rem_euclid(12) + 1) as u32)
}

pub const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
pub const WEEKDAYS: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];

/// "2026-11-05", as the shell's lines key days
pub fn key(d: Day) -> String {
    format!("{:04}-{:02}-{:02}", d.0, d.1, d.2)
}

/// The header: "November 2026", or the week "2 Nov – 8 Nov"
pub fn title(shown: (i32, u32), week: Option<&[Day]>) -> String {
    match week {
        Some(w) if w.len() == 7 => format!("{} {} – {} {}", w[0].2, &MONTHS[w[0].1 as usize - 1][..3], w[6].2, &MONTHS[w[6].1 as usize - 1][..3]),
        _ => format!("{} {}", MONTHS[shown.1 as usize - 1], shown.0),
    }
}

/// The chosen day's title (CalendarWidget.dayTitle)
pub fn day_title(d: Day, today: Day) -> String {
    if d == today {
        "Today".into()
    } else if d == add_days(today, 1) {
        "Tomorrow".into()
    } else {
        format!("{} {} {}", WEEKDAYS[weekday(d) as usize], d.2, MONTHS[d.1 as usize - 1])
    }
}

/// Material's big date under the time ("Thursday, 5 November")
pub fn long_date(d: Day) -> String {
    format!("{}, {} {}", WEEKDAYS[weekday(d) as usize], d.2, MONTHS[d.1 as usize - 1])
}

// ── The focus timer's ring (FocusWidget.qml's Canvas) ───────────────────────

/// An arc on the ring (110 px box, radius 49, clockwise from `a0` to `a1`,
/// radians with 0 at three o'clock) as SVG path commands
fn arc(a0: f64, a1: f64) -> String {
    let (c, r) = (55.0, 49.0);
    let p = |a: f64| (c + r * a.cos(), c + r * a.sin());
    let sweep = a1 - a0;
    if sweep >= std::f64::consts::TAU - 1e-6 {
        // (a whole circle: two halves)
        let (x0, y0) = p(a0);
        let (x1, y1) = p(a0 + std::f64::consts::PI);
        return format!("M {x0:.2} {y0:.2} A {r} {r} 0 0 1 {x1:.2} {y1:.2} A {r} {r} 0 0 1 {x0:.2} {y0:.2}");
    }
    let ((x0, y0), (x1, y1)) = (p(a0), p(a1));
    let large = if sweep > std::f64::consts::PI { 1 } else { 0 };
    format!("M {x0:.2} {y0:.2} A {r} {r} 0 {large} 1 {x1:.2} {y1:.2}")
}

/// The ring's track and its progress (`p` 0..1) as path commands ("" none).
/// Material: the track leaves a gap either side of the progress
pub fn ring(p: f64, material: bool) -> (String, String) {
    use std::f64::consts::{FRAC_PI_2, PI, TAU};
    let p = p.clamp(0.0, 1.0);
    let start = -FRAC_PI_2;
    let end = start + TAU * p;
    let track = if material && p > 0.0 && p < 1.0 {
        let gap = 16.0 / 49.0;
        if TAU * (1.0 - p) > 2.0 * gap { arc(end + gap, 3.0 * PI / 2.0 - gap) } else { String::new() }
    } else {
        arc(0.0, TAU)
    };
    let fill = if p > 0.0 { arc(start, end.min(start + TAU)) } else { String::new() };
    (track, fill)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(id: u32, app: &str, at: i64) -> Note {
        Note { id, app_name: app.into(), at, ..Default::default() }
    }

    #[test]
    fn the_island_is_one_more_group_by_its_newest() {
        let notes = vec![note(1, "Mail", 100), note(2, "Chat", 200), note(3, "Mail", 300)];
        let alerts = vec![Alert { hid: 9, at: 250, ..Default::default() }, Alert { hid: 8, at: 50, ..Default::default() }];
        let g = groups(&notes, &alerts);
        assert_eq!(g.iter().map(|g| g.app.as_str()).collect::<Vec<_>>(), vec!["Mail", "Tessera", "Chat"]);
        assert_eq!(g[0].items, vec![3, 1]);
        assert_eq!(g[1].items, vec![9, 8]);
        assert!(g[1].island && g[1].key() == "island:Tessera");
        // (older than everything: last; nothing: no group)
        assert_eq!(groups(&notes, &[Alert { hid: 1, at: 1, ..Default::default() }]).last().unwrap().app, "Tessera");
        assert_eq!(groups(&notes, &[]).len(), 2);
    }

    #[test]
    fn rows_for_closed_and_open_groups() {
        let notes = vec![note(1, "Mail", 1), note(2, "Chat", 2), note(3, "Mail", 3), note(4, "Mail", 4), note(5, "Mail", 5), note(6, "Files", 6)];
        let g = groups(&notes, &[]);
        // Files (1), Mail (4: 5 4 3 1), Chat (1)
        let rows = entries(&g, &HashSet::new());
        assert_eq!(rows.iter().map(|e| (e.item, e.count, e.edges)).collect::<Vec<_>>(), vec![(6, 1, 0), (5, 4, 2), (2, 1, 0)]);
        assert!(rows[0].first && !rows[0].last && !rows[1].first && rows[2].last);
        let open: HashSet<String> = ["Mail".to_string()].into();
        let rows = entries(&g, &open);
        let kinds: Vec<Kind> = rows.iter().map(|e| e.kind).collect();
        assert_eq!(kinds, vec![Kind::Note, Kind::Header, Kind::Note, Kind::Note, Kind::Note, Kind::Note, Kind::Note]);
        // the open group is a block of its own: Files ends one, Chat starts one
        assert!(rows[0].first && rows[0].last);
        assert!(rows[1].first && !rows[1].last && rows[5].last);
        assert!(rows[6].first && rows[6].last);
        assert!(rows[2..6].iter().all(|e| e.open && e.count == 1));
        // (a group of one never opens)
        let open: HashSet<String> = ["Files".to_string()].into();
        assert_eq!(entries(&g, &open).len(), 3);
    }

    #[test]
    fn the_ring() {
        let (t, f) = ring(0.0, false);
        assert!(t.matches(" A ").count() == 2 && f.is_empty());
        let (t, f) = ring(0.25, false);
        assert!(t.contains(" A ") && f.starts_with("M 55.00 6.00 A 49 49 0 0 1 104.00 55.00"));
        let (_, f) = ring(0.75, false);
        assert!(f.contains(" 0 1 1 "), "{f}"); // (the long way round)
        // Material: the track stops short of the progress either side
        let (t, _) = ring(0.25, true);
        assert!(t.starts_with("M ") && !t.starts_with("M 104.00 55.00"), "{t}");
        assert_eq!(ring(0.995, true).0, "");
        assert_eq!(ring(1.0, true).1.matches(" A ").count(), 2);
    }

    #[test]
    fn dates() {
        assert_eq!(days_from_civil((1970, 1, 1)), 0);
        for d in [(2026, 11, 5), (2024, 2, 29), (2000, 1, 1), (1999, 12, 31)] {
            assert_eq!(civil_from_days(days_from_civil(d)), d);
        }
        assert_eq!(weekday((2026, 11, 5)), 3); // a Thursday
        assert_eq!(weekday((2026, 11, 2)), 0);
        let g = month_grid(2026, 11);
        assert_eq!((g.len(), g[0], g[6], g[41]), (42, (2026, 10, 26), (2026, 11, 1), (2026, 12, 6)));
        assert_eq!(week_of((2026, 11, 5)), (2..=8).map(|d| (2026, 11, d)).collect::<Vec<_>>());
        assert_eq!(add_months((2026, 11), 2), (2027, 1));
        assert_eq!(add_months((2026, 1), -1), (2025, 12));
        assert_eq!(key((2026, 1, 5)), "2026-01-05");
        assert_eq!(title((2026, 11), None), "November 2026");
        assert_eq!(title((2026, 11), Some(&week_of((2026, 11, 30)))), "30 Nov – 6 Dec");
        assert_eq!(day_title((2026, 11, 5), (2026, 11, 5)), "Today");
        assert_eq!(day_title((2026, 11, 6), (2026, 11, 5)), "Tomorrow");
        assert_eq!(day_title((2026, 11, 9), (2026, 11, 5)), "Monday 9 November");
        assert_eq!(long_date((2026, 11, 5)), "Thursday, 5 November");
    }
}

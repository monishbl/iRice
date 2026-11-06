//! Where everything in the Overview goes (shell/modules/overview/Overview.qml's
//! numbers), as pure functions: the grid of spaces (a row per display with
//! several), the macOS-style strip, each window's miniature inside its
//! space, the edge zones, and what's under a point.

pub use crate::wm::Rect;
use crate::wm::spaces::{Geo, Side, Zone};
use crate::services::clients::Client;

/// Text heights at the shell's sizes (small 12 px, body 13 px)
pub const SMALL_H: f32 = 15.0;
pub const BODY_H: f32 = 16.0;

/// The search field's box (with the glass's shadow margin of 20 round it)
pub const SEARCH_W: f32 = 560.0;
pub const SEARCH_H: f32 = 52.0;
pub const SEARCH_M: f32 = 20.0;

pub fn search_y(mac: bool, h: f32) -> f32 {
    if mac {
        (h * 0.36).round()
    } else {
        64.0
    }
}

/// Overview.placeIn: a window's place within a space on display `geo`, from
/// that display's corner. A window sent to a space another display isn't
/// showing keeps its old place (Hyprland), so it's mapped across from the
/// display it's on.
pub fn place_in(at: (f32, f32), size: (f32, f32), geo: Geo, displays: &[Geo]) -> Rect {
    let (cx, cy) = (at.0 + size.0 / 2.0, at.1 + size.1 / 2.0);
    let inside = |g: &Geo| cx >= g.x && cx < g.x + g.w && cy >= g.y && cy < g.y + g.h;
    let src = if inside(&geo) { geo } else { displays.iter().find(|g| inside(g)).copied().unwrap_or(geo) };
    let (sx, sy) = (geo.w / src.w.max(1.0), geo.h / src.h.max(1.0));
    Rect { x: (at.0 - src.x) * sx, y: (at.1 - src.y) * sy, w: size.0 * sx, h: size.1 * sy }
}

/// fullscreen: 0 none, 1 maximized, 2 fullscreen (bit flags)
pub fn is_full(c: &Client) -> bool {
    c.fullscreen & 2 != 0
}

/// Stacking like Hyprland: tiled, then floating by focus (recent on top),
/// then fullscreen and maximized
pub fn stack(c: &Client) -> i32 {
    if c.fullscreen > 0 {
        3000
    } else if c.floating {
        2000 - c.focus_history_id.clamp(0, 999)
    } else {
        1
    }
}

/// Spaces per line: up to 5 (6 with a row per display)
pub fn columns(rows_mode: bool, most: usize) -> usize {
    most.max(1).min(if rows_mode { 6 } else { 5 })
}

/// One cell width for every row, each row keeping its display's shape:
/// `rows` (spaces, aspect h/w) per row
pub fn cell_w(w: f32, h: f32, columns: usize, rows_mode: bool, side_reserve: f32, rows: &[(usize, f32)]) -> f32 {
    let c = columns.max(1) as f32;
    let by_width = (w - 160.0 - side_reserve - (c - 1.0) * 24.0) / c;
    if !rows_mode {
        return 300f32.min(by_width);
    }
    // rows below the search: label 28, title 26 + gap 24 a line, 32 between rows
    let avail = h - 64.0 - 52.0 - 40.0 - 48.0;
    let (mut fixed, mut per_w) = (0.0, 0.0);
    for (n, aspect) in rows {
        let lines = ((*n).max(1) as f32 / c).ceil();
        fixed += 28.0 + lines * 50.0 + 32.0;
        per_w += lines * aspect;
    }
    80f32.max(300f32.min(by_width).min((avail - fixed) / per_w.max(0.1)))
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CellBox {
    pub id: i32,
    pub row: usize,
    pub rect: Rect,
    /// the title under it
    pub title: Rect,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RowBox {
    pub name: String,
    /// the display's label (rows mode)
    pub label: (f32, f32),
    pub cells: Vec<CellBox>,
}

/// The grid: `rows` (display, its spaces, its aspect) under the search
pub fn grid(w: f32, h: f32, rows: &[(String, Vec<i32>, f32)], columns: usize, cell_w: f32, rows_mode: bool, material: bool) -> Vec<RowBox> {
    let cols = columns.max(1);
    let line_h = |aspect: f32| cell_w * aspect + 8.0 + SMALL_H;
    let row_h = |n: usize, aspect: f32| {
        let lines = n.max(1).div_ceil(cols) as f32;
        (if rows_mode { BODY_H + 8.0 } else { 0.0 }) + lines * line_h(aspect) + (lines - 1.0) * 24.0
    };
    let total: f32 = rows.iter().map(|(_, ids, a)| row_h(ids.len(), *a)).sum::<f32>() + 32.0 * (rows.len().max(1) - 1) as f32;
    let search_bottom = search_y(false, h) + SEARCH_H + 2.0 * SEARCH_M;
    let mut y = (search_bottom + 40.0).max((h - total) / 2.0);
    let mut out = Vec::new();
    for (ri, (name, ids, aspect)) in rows.iter().enumerate() {
        let shown = ids.len().min(cols).max(1) as f32;
        let gw = shown * cell_w + (shown - 1.0) * 24.0;
        let x0 = ((w - gw) / 2.0).round();
        let label = (x0 + if material { 12.0 } else { 0.0 }, y);
        let top = y + if rows_mode { BODY_H + 8.0 } else { 0.0 };
        let ch = cell_w * aspect;
        let cells = ids
            .iter()
            .enumerate()
            .map(|(i, &id)| {
                let (col, line) = ((i % cols) as f32, (i / cols) as f32);
                let r = Rect { x: x0 + col * (cell_w + 24.0), y: top + line * (line_h(*aspect) + 24.0), w: cell_w, h: ch };
                CellBox { id, row: ri, rect: r, title: Rect { x: r.x, y: r.y + ch + 8.0, w: cell_w, h: SMALL_H } }
            })
            .collect();
        out.push(RowBox { name: name.clone(), label, cells });
        y += row_h(ids.len(), *aspect) + 32.0;
    }
    out
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MacRow {
    pub name: String,
    pub label: (f32, f32),
    pub tiles: Vec<CellBox>,
    /// the + button: x, y, size
    pub plus: Rect,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mac {
    pub bar_h: f32,
    pub rows: Vec<MacRow>,
}

/// The macOS style: a strip of spaces per display on a bar at the top
pub fn mac(w: f32, h: f32, rows: &[(String, Vec<i32>, f32)], rows_mode: bool, material: bool) -> Mac {
    let n_rows = rows.len().max(1) as f32;
    let strip_h = if rows_mode { 48f32.max(96f32.min((h * 0.42) / n_rows - 40.0)) } else { 120.0 };
    let mut y = if rows_mode { 40.0 } else { 20.0 };
    let mut out = Vec::new();
    for (name, ids, aspect) in rows {
        let n = ids.len().max(1) as f32;
        let tile_w = (strip_h / aspect.max(0.01)).min(170.0).min((w - 220.0 - (n - 1.0) * 24.0) / n);
        let tile_h = (tile_w * aspect).round();
        let shown = ids.len() as f32;
        let sw = shown * tile_w + (shown - 1.0).max(0.0) * 24.0;
        let x0 = ((w - sw) / 2.0).round();
        let sy = y + if rows_mode { 20.0 } else { 6.0 };
        let tiles = ids
            .iter()
            .enumerate()
            .map(|(i, &id)| {
                let r = Rect { x: x0 + i as f32 * (tile_w + 24.0), y: sy, w: tile_w, h: tile_h };
                CellBox { id, row: out.len(), rect: r, title: Rect { x: r.x, y: sy + tile_h + 6.0, w: tile_w, h: SMALL_H } }
            })
            .collect();
        let size = if material { 40.0 } else { 36.0 };
        let plus = Rect { x: w - 28.0 - size, y: sy + tile_h / 2.0 - size / 2.0, w: size, h: size };
        out.push(MacRow { name: name.clone(), label: (28.0, y), tiles, plus });
        y += (if rows_mode { 20.0 } else { 0.0 }) + strip_h + 26.0 + 16.0;
    }
    Mac { bar_h: y - 16.0 + 20.0, rows: out }
}

/// One edge zone's pill: where, its head's height, its chips' grid
#[derive(Clone, Debug, PartialEq)]
pub struct ZoneBox {
    pub name: String,
    pub side: Side,
    pub rect: Rect,
    pub head_h: f32,
    /// the space chips' places (relative to the pill), when it's open
    pub chips: Vec<(i32, Rect)>,
}

/// Each other display's zone on the edge that faces it; `label_w`: its
/// label's width; `expanded`: the zone whose spaces are open, with them
pub fn zones(w: f32, h: f32, top_y: f32, zones: &[Zone], label_w: impl Fn(&str) -> f32, expanded: Option<(&str, &[i32])>) -> Vec<ZoneBox> {
    zones
        .iter()
        .map(|z| {
            let vertical = z.side.vertical();
            let head_h = if vertical { 68.0 } else { 44.0 };
            let open = expanded.filter(|(n, _)| *n == z.name).map(|(_, ids)| ids).unwrap_or(&[]);
            let cols = if vertical { 2 } else { 9 };
            let (cw, chh) = if open.is_empty() {
                (0.0, 0.0)
            } else {
                let c = open.len().min(cols) as f32;
                let lines = open.len().div_ceil(cols) as f32;
                (c * 36.0 + (c - 1.0) * 8.0, lines * 36.0 + (lines - 1.0) * 8.0)
            };
            let head_w = if vertical { 96.0 } else { 20.0 + 8.0 + label_w(&z.name) + 32.0 };
            let pill_w = head_w.max(if open.is_empty() { 0.0 } else { cw + 16.0 });
            let pill_h = head_h + if open.is_empty() { 0.0 } else { chh + 8.0 };
            let along = (z.index as f32 - (z.count as f32 - 1.0) / 2.0) * if vertical { 80.0 } else { 200.0 };
            let x = match z.side {
                Side::Left => 16.0,
                Side::Right => w - 16.0 - pill_w,
                _ => (w / 2.0 - pill_w / 2.0 + along).round(),
            };
            let y = match z.side {
                Side::Top => top_y,
                Side::Bottom => h - 16.0 - pill_h,
                _ => (h / 2.0 - pill_h / 2.0 + along).round(),
            };
            let x0 = ((pill_w - cw) / 2.0).round();
            let chips = open.iter().enumerate().map(|(i, &id)| (id, Rect { x: x0 + (i % cols) as f32 * 44.0, y: head_h + (i / cols) as f32 * 44.0, w: 36.0, h: 36.0 })).collect();
            ZoneBox { name: z.name.clone(), side: z.side, rect: Rect { x, y, w: pill_w, h: pill_h }, head_h, chips }
        })
        .collect()
}

/// The space nearest a point, among a row's (not the one dragged)
pub fn nearest(cells: &[CellBox], row: usize, skip: i32, px: f32, py: f32) -> i32 {
    let mut best = (0, f32::MAX);
    for c in cells.iter().filter(|c| c.row == row && c.id != skip) {
        let (cx, cy) = c.rect.center();
        let d = (cx - px).hypot(cy - py);
        if d < best.1 {
            best = (c.id, d);
        }
    }
    best.0
}

/// The macOS strip: the space nearest along x
pub fn nearest_x(cells: &[CellBox], row: usize, px: f32) -> i32 {
    let mut best = (0, f32::MAX);
    for c in cells.iter().filter(|c| c.row == row) {
        let d = (c.rect.x + c.rect.w / 2.0 - px).abs();
        if d < best.1 {
            best = (c.id, d);
        }
    }
    best.0
}

/// Move the picked space by `n` places
pub fn step(ids: &[i32], picked: i32, n: i32) -> i32 {
    if ids.is_empty() {
        return picked;
    }
    let i = ids.iter().position(|&x| x == picked).unwrap_or(0) as i32;
    ids[(i + n).clamp(0, ids.len() as i32 - 1) as usize]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wm::spaces::Geo;

    fn g(x: f32, y: f32, w: f32, h: f32) -> Geo {
        Geo { x, y, w, h }
    }

    #[test]
    fn windows_sit_where_they_are_on_their_display() {
        let here = g(0.0, 0.0, 1920.0, 1080.0);
        let r = place_in((100.0, 50.0), (800.0, 600.0), here, &[here]);
        assert_eq!(r, Rect { x: 100.0, y: 50.0, w: 800.0, h: 600.0 });
        // still on the other (larger) display: mapped across
        let other = g(1920.0, 0.0, 3840.0, 2160.0);
        let r = place_in((1920.0 + 400.0, 200.0), (800.0, 400.0), here, &[here, other]);
        assert_eq!(r, Rect { x: 200.0, y: 100.0, w: 400.0, h: 200.0 });
    }

    #[test]
    fn stacking_like_hyprland() {
        let tiled = Client::default();
        let floating = Client { floating: true, focus_history_id: 2, ..Default::default() };
        let newer = Client { floating: true, focus_history_id: 0, ..Default::default() };
        let full = Client { fullscreen: 2, ..Default::default() };
        assert!(stack(&tiled) < stack(&floating) && stack(&floating) < stack(&newer) && stack(&newer) < stack(&full));
        assert!(is_full(&full) && !is_full(&Client { fullscreen: 1, ..Default::default() }));
    }

    #[test]
    fn the_grid_under_the_search() {
        assert_eq!(columns(false, 3), 3);
        assert_eq!(columns(false, 9), 5);
        assert_eq!(columns(true, 9), 6);
        assert_eq!(cell_w(1920.0, 1080.0, 5, false, 0.0, &[]), 300.0);
        assert_eq!(cell_w(1280.0, 800.0, 5, false, 0.0, &[]), (1280.0 - 160.0 - 96.0) / 5.0);
        let rows = vec![("DP-1".to_string(), vec![1, 2, 3, 4, 5, 6, 7], 0.5625)];
        let gr = grid(1920.0, 1080.0, &rows, 5, 300.0, false, false);
        let c = &gr[0].cells;
        assert_eq!(c.len(), 7);
        // centred, five a line
        assert_eq!(c[0].rect.x, ((1920.0 - (5.0 * 300.0 + 4.0 * 24.0)) / 2.0f32).round());
        assert_eq!(c[5].rect.x, c[0].rect.x);
        assert!(c[5].rect.y > c[0].rect.y + c[0].rect.h);
        // never above the search
        assert!(c[0].rect.y >= 64.0 + 92.0 + 40.0);
        assert_eq!(nearest(c, 0, 1, c[2].rect.x + 5.0, c[2].rect.y), 3);
        assert_eq!(nearest(c, 0, 3, c[2].rect.x + 150.0, c[2].rect.y + 80.0), 2);
    }

    #[test]
    fn rows_per_display_shrink_to_fit() {
        let rows = [(5, 0.5625), (5, 0.5625), (5, 0.5625), (5, 0.5625)];
        let w = cell_w(1920.0, 1080.0, 5, true, 0.0, &rows);
        assert!(w < 300.0 && w >= 80.0);
    }

    #[test]
    fn the_mac_strip() {
        let rows = vec![("DP-1".to_string(), vec![1, 2, 3], 0.5625)];
        let m = mac(1920.0, 1080.0, &rows, false, false);
        let t = &m.rows[0].tiles;
        assert_eq!(t[0].rect.w, 170.0); // (120 / 0.5625 is wider: capped)
        assert_eq!(t[1].rect.x - t[0].rect.x, 194.0);
        assert_eq!(m.rows[0].plus.x, 1920.0 - 28.0 - 36.0);
        assert_eq!(m.bar_h, 20.0 + 120.0 + 26.0 + 20.0);
        assert_eq!(nearest_x(t, 0, t[2].rect.x), 3);
    }

    #[test]
    fn zones_on_their_edges() {
        let z = vec![Zone { name: "DP-1".into(), side: Side::Right, index: 0, count: 1 }, Zone { name: "DP-2".into(), side: Side::Top, index: 0, count: 1 }];
        let b = zones(1920.0, 1080.0, 12.0, &z, |_| 60.0, None);
        assert_eq!(b[0].rect, Rect { x: 1920.0 - 16.0 - 96.0, y: (540.0f32 - 34.0).round(), w: 96.0, h: 68.0 });
        assert_eq!((b[1].rect.y, b[1].rect.w, b[1].rect.h), (12.0, 120.0, 44.0));
        // held over: its spaces open below the head
        let b = zones(1920.0, 1080.0, 12.0, &z, |_| 60.0, Some(("DP-1", &[101, 102, 103])));
        assert_eq!(b[0].chips.len(), 3);
        assert_eq!(b[0].chips[2].1, Rect { x: 8.0, y: 68.0 + 44.0, w: 36.0, h: 36.0 });
        assert_eq!(b[0].rect.h, 68.0 + 80.0 + 8.0);
    }

    #[test]
    fn arrows_step_within_the_spaces() {
        assert_eq!(step(&[1, 2, 3], 2, 1), 3);
        assert_eq!(step(&[1, 2, 3], 3, 5), 3);
        assert_eq!(step(&[1, 2, 3], 1, -5), 1);
        assert_eq!(step(&[], 4, 1), 4);
    }
}

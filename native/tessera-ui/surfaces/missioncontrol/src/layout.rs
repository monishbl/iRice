//! Mission Control's places (shell/modules/missioncontrol/MissionControl.qml):
//! the spaces bar along the top and the current space's windows spread out
//! below in the grid that keeps them largest.

pub use crate::wm::Rect;

/// The spaces bar's cards: 150 px wide, the screen's shape
pub const CARD_W: f32 = 150.0;
pub const BAR_Y: f32 = 44.0;
pub const TITLE_H: f32 = 15.0;
pub const GAP: f32 = 32.0;
/// room under each window for its icon or chip
pub const UNDER: f32 = 28.0;

pub fn card_h(screen: (f32, f32)) -> f32 {
    (screen.1 * CARD_W / screen.0.max(1.0)).round()
}

/// The cards, centred, 16 px apart
pub fn cards(n: usize, screen: (f32, f32)) -> Vec<Rect> {
    let h = card_h(screen);
    let total = n as f32 * CARD_W + (n.max(1) - 1) as f32 * 16.0;
    let x0 = ((screen.0 - total) / 2.0).round();
    (0..n).map(|i| Rect { x: x0 + i as f32 * (CARD_W + 16.0), y: BAR_Y, w: CARD_W, h }).collect()
}

/// Where the windows go: below the bar, 48 px in from the edges
pub fn stage(screen: (f32, f32)) -> Rect {
    let top = BAR_Y + card_h(screen) + 6.0 + TITLE_H + 40.0;
    Rect { x: 48.0, y: top, w: screen.0 - 96.0, h: screen.1 - top - 48.0 }
}

/// The columns and rows that keep the smallest window largest, and that scale
pub fn fit(sizes: &[(f32, f32)], w: f32, h: f32) -> (usize, usize, f32) {
    let n = sizes.len();
    if n == 0 {
        return (1, 1, 0.0);
    }
    let mut best = (1, n, 0.0f32);
    for cols in 1..=n {
        let rows = n.div_ceil(cols);
        let cw = (w - GAP * (cols - 1) as f32) / cols as f32;
        let ch = (h - GAP * (rows - 1) as f32 - UNDER * rows as f32) / rows as f32;
        let worst = sizes.iter().map(|(sw, sh)| (cw / sw.max(1.0)).min(ch / sh.max(1.0))).fold(f32::INFINITY, f32::min);
        if worst > best.2 {
            best = (cols, rows, worst);
        }
    }
    best
}

/// Each window's place on the stage (a short last row is centred)
pub fn tiles(sizes: &[(f32, f32)], stage: Rect) -> Vec<Rect> {
    let (cols, rows, scale) = fit(sizes, stage.w, stage.h);
    let k = scale.min(0.9);
    let cell_w = (stage.w - GAP * (cols - 1) as f32) / cols as f32;
    let cell_h = (stage.h - GAP * (rows - 1) as f32) / rows as f32;
    let n = sizes.len();
    sizes
        .iter()
        .enumerate()
        .map(|(i, (sw, sh))| {
            let (col, row) = (i % cols, i / cols);
            let in_row = cols.min(n - row * cols);
            let offset = (cols - in_row) as f32 * (cell_w + GAP) / 2.0;
            let (w, h) = (sw * k, sh * k);
            Rect { x: stage.x + offset + col as f32 * (cell_w + GAP) + (cell_w - w) / 2.0, y: stage.y + row as f32 * (cell_h + GAP) + (cell_h - UNDER - h) / 2.0, w, h }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bar() {
        assert_eq!(card_h((1920.0, 1080.0)), 84.0);
        let c = cards(3, (1920.0, 1080.0));
        assert_eq!(c[0].x, ((1920.0 - 482.0) / 2.0f32).round());
        assert_eq!(c[1].x - c[0].x, 166.0);
        let s = stage((1920.0, 1080.0));
        assert_eq!((s.x, s.y, s.w), (48.0, 44.0 + 84.0 + 6.0 + 15.0 + 40.0, 1824.0));
    }

    #[test]
    fn windows_as_large_as_they_fit() {
        // one window: alone, never larger than 90 %
        let t = tiles(&[(1920.0, 1080.0)], Rect { x: 0.0, y: 0.0, w: 1824.0, h: 843.0 });
        assert!(t[0].w <= 1920.0 * 0.9 + 0.01);
        // two wide windows on a wide stage: side by side
        let (cols, rows, _) = fit(&[(1600.0, 900.0), (1600.0, 900.0)], 1824.0, 600.0);
        assert_eq!((cols, rows), (2, 1));
        // three: the short last row is centred
        let t = tiles(&[(800.0, 800.0), (800.0, 800.0), (800.0, 800.0)], Rect { x: 0.0, y: 0.0, w: 1000.0, h: 1000.0 });
        let (cols, _, _) = fit(&[(800.0, 800.0), (800.0, 800.0), (800.0, 800.0)], 1000.0, 1000.0);
        assert_eq!(cols, 2);
        assert!((t[2].x + t[2].w / 2.0 - 500.0).abs() < 0.5);
        assert_eq!(fit(&[], 100.0, 100.0), (1, 1, 0.0));
    }
}

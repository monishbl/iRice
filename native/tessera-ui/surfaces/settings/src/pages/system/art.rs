//! The fingerprint in the add-a-finger sheet (FingerprintArt.qml): a loop
//! print drawn like the familiar fingerprint icon, hairpins nested round a
//! small core inside wider arcs, broken into dashes, cut into 11 sections
//! (0 the centre; 1-4 the ring around it: above, right, below, left; 5-10
//! the edges: the tip, then clockwise). Made once, as SVG path commands per
//! section, in a `W`×`H` box; ui/settings-system.slint strokes each section
//! in its colour (taken, the next one, the rest).

use std::sync::OnceLock;

pub const W: f32 = 200.0;
pub const H: f32 = 248.0;
pub const STROKE: f32 = 7.0;
pub const SECTIONS: usize = 11;

/// The ridges' segments: (x1, y1, x2, y2, section)
pub fn segments(width: f32, height: f32) -> Vec<(f32, f32, f32, f32, usize)> {
    let mut lines: Vec<Vec<(f64, f64)>> = Vec::new();
    let gap = 13.0f64;
    let tilt = 24.0f64.to_radians();
    let rot = |x: f64, y: f64| (x * tilt.cos() - y * tilt.sin(), x * tilt.sin() + y * tilt.cos());
    // the hairpins: over the top, down both legs (the left one longer)
    for k in 0..6 {
        let r = 6.0 + k as f64 * gap;
        let mut pts = Vec::new();
        let leg_l = 24.0 + k as f64 * 17.0 + (k % 2) as f64 * 10.0;
        let leg_r = 6.0 + k as f64 * 10.0;
        let mut y = leg_l;
        while y > 0.0 {
            pts.push(rot(-r, y));
            y -= 3.0;
        }
        let mut d = 180.0f64;
        while d <= 360.0 {
            let t = d.to_radians();
            pts.push(rot(r * t.cos(), r * 1.3 * t.sin()));
            d += 4.0;
        }
        let mut y = 0.0;
        while y <= leg_r {
            pts.push(rot(r, y));
            y += 3.0;
        }
        lines.push(pts);
    }
    // the outer ridges: over the top and round the right, down the left
    for k in 6..9 {
        let r = 6.0 + k as f64 * gap;
        let mut pts = Vec::new();
        let mut d = 152.0 + (k - 6) as f64 * 8.0;
        let end = 424.0 - (k - 6) as f64 * 14.0;
        while d <= end {
            let t = d.to_radians();
            let sx = if t.sin() > 0.0 { 1.15 } else { 1.3 };
            pts.push(rot(r * t.cos(), r * sx * t.sin()));
            d += 3.0;
        }
        lines.push(pts);
    }
    // fit into the box, leaving room for the stroke
    let (mut x0, mut y0, mut x1, mut y1) = (1e9f64, 1e9f64, -1e9f64, -1e9f64);
    for l in &lines {
        for &(x, y) in l {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
    }
    let (w, h, m) = (width as f64, height as f64, STROKE as f64);
    let sc = ((w - 2.0 * m) / (x1 - x0)).min((h - 2.0 * m) / (y1 - y0));
    let ox = (w - (x1 - x0) * sc) / 2.0 - x0 * sc;
    let oy = (h - (y1 - y0) * sc) / 2.0 - y0 * sc;
    let cx = ox + (x0 + x1) / 2.0 * sc;
    let cy = oy + (y0 + y1) / 2.0 * sc;
    let rx_max = (x1 - x0) / 2.0 * sc;
    let ry_max = (y1 - y0) / 2.0 * sc;
    let section_of = |x: f64, y: f64| -> usize {
        let dx = (x - cx) / rx_max;
        let dy = (y - cy) / ry_max;
        let r = (dx * dx + dy * dy).sqrt();
        let a = dy.atan2(dx).to_degrees(); // 0 = right, -90 = up
        if r < 0.27 {
            return 0;
        }
        if r < 0.6 {
            return if (-135.0..-45.0).contains(&a) {
                1
            } else if (-45.0..45.0).contains(&a) {
                2
            } else if (45.0..135.0).contains(&a) {
                3
            } else {
                4
            };
        }
        let b = (a + 120.0 + 360.0) % 360.0; // 0 at the top-left end of the tip's sector
        5 + ((b / 60.0).floor() as usize).min(5)
    };
    // dashes: lengths cycling per ridge (the core two unbroken)
    let dashes = [62.0, 30.0, 48.0, 22.0, 70.0, 36.0, 54.0];
    let mut out = Vec::new();
    for (k, l) in lines.iter().enumerate() {
        let mut along = 0.0;
        let mut di = k * 3;
        let mut until = if k < 2 { 1e9 } else { dashes[di % dashes.len()] };
        let mut on = true;
        for i in 1..l.len() {
            let (ax, ay) = (ox + l[i - 1].0 * sc, oy + l[i - 1].1 * sc);
            let (bx, by) = (ox + l[i].0 * sc, oy + l[i].1 * sc);
            along += ((bx - ax).powi(2) + (by - ay).powi(2)).sqrt();
            if along > until {
                on = !on;
                along = 0.0;
                if on {
                    di += 1;
                    until = dashes[di % dashes.len()];
                } else {
                    until = 11.0;
                }
            }
            if on {
                out.push((ax as f32, ay as f32, bx as f32, by as f32, section_of((ax + bx) / 2.0, (ay + by) / 2.0)));
            }
        }
    }
    out
}

/// Each section's ridges as SVG path commands ("M x y L x y …")
pub fn section_paths() -> &'static [String; SECTIONS] {
    static P: OnceLock<[String; SECTIONS]> = OnceLock::new();
    P.get_or_init(|| {
        let mut p: [String; SECTIONS] = Default::default();
        for (x1, y1, x2, y2, s) in segments(W, H) {
            p[s].push_str(&format!("M {x1:.1} {y1:.1} L {x2:.1} {y2:.1} "));
        }
        p
    })
}

/// Sections taken so far: the reader's stages spread over the 11 (or one
/// each, if it doesn't say how many it wants); all of them when done
pub fn taken(done: bool, count: i64, stages: i64) -> usize {
    if done {
        return SECTIONS;
    }
    let c = count.max(0) as usize;
    if stages > 0 {
        (SECTIONS * c / stages as usize).min(SECTIONS - 1)
    } else {
        c.min(SECTIONS - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_section_has_ridges_inside_the_box() {
        let segs = segments(W, H);
        assert!(segs.len() > 200);
        for s in 0..SECTIONS {
            assert!(segs.iter().any(|g| g.4 == s), "section {s} is empty");
        }
        for g in &segs {
            for (x, y) in [(g.0, g.1), (g.2, g.3)] {
                assert!(x >= 0.0 && x <= W && y >= 0.0 && y <= H, "{x},{y}");
            }
        }
        assert!(section_paths()[0].starts_with("M "));
    }

    #[test]
    fn readers_stages_spread_over_the_sections() {
        assert_eq!(taken(false, 0, 0), 0);
        assert_eq!(taken(false, 3, 0), 3);
        assert_eq!(taken(false, 20, 0), 10);
        assert_eq!(taken(false, 5, 10), 5);
        assert_eq!(taken(false, 10, 10), 10);
        assert_eq!(taken(true, 0, 10), 11);
    }
}

//! The dock's geometry, in logical px, as DockGlass.qml, MaterialDock.qml,
//! DockItem.qml and DockLogic.qml compute it: sizes per look, where each
//! icon sits (Glass's magnification: a cosine falloff over 2.5 icons), which
//! one the pointer is on, the input shapes, and auto-hide's decisions.
//!
//! The pointer is given relative to the window's bottom centre (x right of
//! the centre, y up is negative), so nothing here needs the window's width.

use super::settings::{AutoHide, Settings};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metrics {
    pub material: bool,
    /// an icon's slot (Glass: at rest)
    pub base: f32,
    /// Material: the icon drawn inside its slot
    pub glyph: f32,
    pub pad: f32,
    /// Glass: the slab's corners
    pub corner: f32,
    pub gap: f32,
    pub bar_h: f32,
    pub bottom_gap: f32,
    pub divider_w: f32,
    pub max_scale: f32,
}

impl Metrics {
    pub fn new(s: &Settings, material: bool) -> Self {
        let gap_k = s.gap.clamp(0.05, 0.4);
        if material {
            // a Pixel tablet's taskbar: icons 36..56, a 64 px pill at the default
            let base = s.icon_size.clamp(36.0, 56.0).round();
            let pad = 8.0;
            let gap = (base * gap_k).round();
            Metrics { material, base, glyph: base - 4.0, pad, corner: (base + 2.0 * pad) / 2.0, gap, bar_h: base + 2.0 * pad, bottom_gap: 16.0, divider_w: 1.0 + 2.0 * gap, max_scale: 1.0 }
        } else {
            let base = s.icon_size.round();
            let pad = (base * 0.26).round();
            let gap = (base * gap_k).round();
            Metrics {
                material,
                base,
                glyph: base,
                pad,
                corner: (base * 0.32).round(),
                gap,
                bar_h: base + 2.0 * pad,
                bottom_gap: 6.0,
                divider_w: 1.0 + 2.0 * gap,
                max_scale: if s.magnification { s.magnified_scale.max(1.0) } else { 1.0 },
            }
        }
    }

    /// How far magnified icons reach above the pill
    pub fn extra(&self) -> f32 {
        self.base * (self.max_scale - 1.0)
    }

    /// The window's height: room for magnified icons, a bounce and the name label
    pub fn window_height(&self) -> u32 {
        if self.material {
            (self.bar_h + self.bottom_gap + 48.0) as u32
        } else {
            (self.base * self.max_scale).ceil() as u32 + 2 * self.pad as u32 + self.bottom_gap as u32 + 44
        }
    }

    /// The space windows keep clear of (only when it doesn't auto-hide)
    pub fn exclusive(&self, hide: AutoHide) -> i32 {
        if hide == AutoHide::Off {
            (self.bar_h + self.bottom_gap) as i32
        } else {
            0
        }
    }

    pub fn rest_width(&self, dividers: &[bool]) -> f32 {
        let n = dividers.len() as f32;
        n * self.base + (n - 1.0).max(0.0) * self.gap + dividers.iter().filter(|d| **d).count() as f32 * self.divider_w
    }

    /// The pill at rest (Material: always), its width
    pub fn pill_rest(&self, dividers: &[bool]) -> f32 {
        self.rest_width(dividers) + 2.0 * self.pad
    }

    /// The largest the pill gets (magnified), so the zones never depend on it
    pub fn max_width(&self, dividers: &[bool]) -> f32 {
        self.pill_rest(dividers) + if self.material { 0.0 } else { self.extra() * 2.6 }
    }
}

/// One icon's place: its left edge within the pill, its size, and the
/// divider's room before it
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub x: f32,
    pub size: f32,
    pub lead: f32,
}

/// How much an icon grows with the pointer this far from its resting centre
pub fn proximity(pointer_x: f32, rest_center: f32, base: f32) -> f32 {
    ((pointer_x - rest_center).abs() / (base * 2.5)).min(1.0).mul_add(std::f32::consts::FRAC_PI_2, 0.0).cos()
}

/// Every icon's place, and the pill's width. `pointer_x`: relative to the
/// dock's centre while it magnifies (None at rest)
pub fn place(m: &Metrics, dividers: &[bool], pointer_x: Option<f32>) -> (Vec<Placed>, f32) {
    let rest = m.rest_width(dividers);
    let mut seen_dividers = 0.0;
    let mut x = 0.0; // within the row
    let mut out = Vec::with_capacity(dividers.len());
    for (i, &d) in dividers.iter().enumerate() {
        if d {
            seen_dividers += 1.0;
        }
        let lead = if d { m.divider_w } else { 0.0 };
        let center = i as f32 * (m.base + m.gap) + seen_dividers * m.divider_w + m.base / 2.0 - rest / 2.0;
        let size = match pointer_x {
            Some(p) if m.max_scale > 1.0 => m.base * (1.0 + (m.max_scale - 1.0) * proximity(p, center, m.base)),
            _ => m.base,
        };
        out.push(Placed { x: m.pad + x + lead, size, lead });
        x += lead + size + m.gap;
    }
    let row = (x - m.gap).max(0.0);
    (out, row + 2.0 * m.pad)
}

/// The pointer, relative to the window's bottom centre; `None` off the window
pub type Pointer = Option<(f32, f32)>;

/// Where the pointer is on the dock: whether it's on the pill (`hovered`),
/// which icon it's on, and the layout that goes with it
#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    pub placed: Vec<Placed>,
    pub pill_w: f32,
    pub hovered: bool,
    pub hot: Option<usize>,
}

/// `settled`: fully shown (no magnification or hover while it slides);
/// `menu_open`: a right-click menu is up (no icon is hot meanwhile)
pub fn hit(m: &Metrics, dividers: &[bool], pointer: Pointer, settled: bool, menu_open: bool) -> Hit {
    let at_rest = || place(m, dividers, None);
    let Some((px, py)) = pointer.filter(|_| settled) else {
        let (placed, pill_w) = at_rest();
        return Hit { placed, pill_w, hovered: false, hot: None };
    };
    let (placed, pill_w, hovered) = if m.material {
        // (the pill's resting rectangle: nothing that moves decides hover)
        let (placed, w) = at_rest();
        let on = py >= -(m.bottom_gap + m.bar_h) && py < -m.bottom_gap && px >= -w / 2.0 && px < w / 2.0;
        (placed, w, on)
    } else {
        let (placed, w) = place(m, dividers, Some(px));
        let on = py >= -(m.bottom_gap + m.bar_h + m.extra() + 12.0) && py < -m.bottom_gap && px >= -w / 2.0 && px < w / 2.0;
        if on {
            (placed, w, true)
        } else {
            let (placed, w) = at_rest();
            (placed, w, false)
        }
    };
    let local = px + pill_w / 2.0;
    let slot = |p: &Placed| if m.material { m.base } else { p.size };
    let hot = if hovered && !menu_open { placed.iter().position(|p| local >= p.x - m.gap / 2.0 && local < p.x + slot(p) + m.gap / 2.0) } else { None };
    Hit { placed, pill_w, hovered, hot }
}

/// A rectangle in window (or screen) coordinates: x, y, w, h
pub type Rect = (f32, f32, f32, f32);

/// The zone that keeps a revealed dock up, and the pill's largest extent
/// (input when it doesn't auto-hide), for a window `w`×`h`
pub fn zones(m: &Metrics, dividers: &[bool], w: f32, h: f32) -> (Rect, Rect) {
    if m.material {
        let pw = m.pill_rest(dividers);
        let rest = ((w - pw) / 2.0, h - m.bottom_gap - m.bar_h, pw, m.bar_h);
        ((rest.0 - 24.0, rest.1 - 44.0, rest.2 + 48.0, m.bar_h + m.bottom_gap + 44.0), rest)
    } else {
        let mw = m.max_width(dividers);
        let e = m.extra();
        let zone = ((w - mw) / 2.0 - 24.0, h - (m.bar_h + m.bottom_gap + e + 44.0), mw + 48.0, m.bar_h + m.bottom_gap + e + 44.0);
        let pill = ((w - mw) / 2.0, h - m.bottom_gap - m.bar_h - e - 12.0, mw, m.bar_h + e + 12.0);
        (zone, pill)
    }
}

/// Where the pill rests on a screen `sw`×`sh` (for "hide when a window overlaps it")
pub fn cover_rect(m: &Metrics, dividers: &[bool], sw: f32, sh: f32) -> Rect {
    let pw = m.pill_rest(dividers);
    ((sw - pw) / 2.0, sh - m.bar_h - m.bottom_gap, pw, m.bar_h + m.bottom_gap)
}

/// The window's input shape (DockLogic's mask): the 2 px edge strip while it
/// auto-hides, the zone once revealed, else the pill
pub fn input(m: &Metrics, dividers: &[bool], w: f32, h: f32, auto_hiding: bool, hidden: bool) -> Vec<(i32, i32, i32, i32)> {
    let (zone, pill) = zones(m, dividers, w, h);
    let r = |r: Rect| (r.0.floor() as i32, r.1.floor() as i32, r.2.ceil() as i32, r.3.ceil() as i32);
    let mut out = Vec::new();
    if auto_hiding || hidden {
        out.push((0, h as i32 - 2, w as i32, 2));
    }
    if !hidden && auto_hiding {
        out.push(r(zone));
    }
    if !auto_hiding {
        out.push(r(pill));
    }
    out
}

/// Is the pointer on the bottom edge (reveals the dock), or in the zone
/// that keeps it up? (window coordinates)
pub fn edge_and_zone(m: &Metrics, dividers: &[bool], w: f32, h: f32, pointer: Option<(f32, f32)>) -> (bool, bool) {
    let Some((x, y)) = pointer else { return (false, false) };
    let (zone, _) = zones(m, dividers, w, h);
    (y >= h - 2.0, x >= zone.0 && x < zone.0 + zone.2 && y >= zone.1)
}

// ── Auto-hide (DockLogic.qml) ───────────────────────────────────────────────

pub const REVEAL_DELAY_MS: u64 = 120;
pub const HIDE_DELAY_MS: u64 = 300;

/// What a change asks of the two timers
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Timers {
    pub start_reveal: bool,
    pub stop_reveal: bool,
    pub start_hide: bool,
    pub stop_hide: bool,
}

/// macOS-style auto-hide: resting on the bottom edge for a moment reveals
/// the dock; the zone keeps it up; leaving it hides the dock after a grace
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Reveal {
    pub revealed: bool,
    on_edge: bool,
    wants_up: bool,
}

impl Reveal {
    /// The pointer moved (or left: both false)
    pub fn pointer(&mut self, on_edge: bool, in_zone: bool) -> Timers {
        let mut t = Timers::default();
        if on_edge != self.on_edge {
            self.on_edge = on_edge;
            if on_edge && !self.revealed {
                t.start_reveal = true;
            } else {
                t.stop_reveal = true;
            }
        }
        let wants = on_edge || in_zone;
        if wants != self.wants_up {
            self.wants_up = wants;
            if wants {
                t.stop_hide = true;
            } else if self.revealed {
                t.start_hide = true;
            }
        }
        t
    }
    /// The reveal delay ran out
    pub fn reveal(&mut self) -> Timers {
        if self.revealed {
            return Timers::default();
        }
        self.revealed = true;
        Timers { start_hide: !self.wants_up, ..Default::default() }
    }
    /// The hide delay ran out
    pub fn hide(&mut self) {
        self.revealed = false;
    }
}

/// Whether it auto-hides now: always, or while a window overlaps it
pub fn auto_hiding(mode: AutoHide, overlapped: bool) -> bool {
    mode == AutoHide::Always || (mode == AutoHide::Windows && overlapped)
}

/// Hidden: auto-hiding, not revealed, and no menu or Downloads stack open
pub fn hidden(auto_hiding: bool, revealed: bool, popup_open: bool) -> bool {
    auto_hiding && !revealed && !popup_open
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn glass(cfg: serde_json::Value) -> Metrics {
        Metrics::new(&Settings::read(&cfg), false)
    }

    #[test]
    fn sizes_follow_the_qml() {
        let g = glass(json!({}));
        assert_eq!((g.base, g.pad, g.corner, g.gap, g.bar_h, g.bottom_gap, g.divider_w), (48.0, 12.0, 15.0, 9.0, 72.0, 6.0, 19.0));
        assert_eq!(g.window_height(), 48 + 24 + 6 + 44);
        assert_eq!(g.exclusive(AutoHide::Off), 78);
        assert_eq!(g.exclusive(AutoHide::Always), 0);
        let m = Metrics::new(&Settings::read(&json!({})), true);
        assert_eq!((m.base, m.glyph, m.pad, m.bar_h, m.bottom_gap, m.max_scale), (48.0, 44.0, 8.0, 64.0, 16.0, 1.0));
        assert_eq!(m.window_height(), 64 + 16 + 48);
        // Material keeps icons 36..56 and never magnifies
        let m = Metrics::new(&Settings::read(&json!({"dock": {"iconSize": 80, "magnification": true}})), true);
        assert_eq!((m.base, m.max_scale), (56.0, 1.0));
        // the gap stays within 5%..40% of an icon
        assert_eq!(glass(json!({"dock": {"gap": 2.0}})).gap, (48.0f32 * 0.4).round());
    }

    #[test]
    fn icons_in_a_row_with_dividers() {
        let g = glass(json!({}));
        let d = [false, false, true, false];
        assert_eq!(g.rest_width(&d), 4.0 * 48.0 + 3.0 * 9.0 + 19.0);
        let (p, w) = place(&g, &d, None);
        assert_eq!(w, g.pill_rest(&d));
        assert_eq!(p[0], Placed { x: 12.0, size: 48.0, lead: 0.0 });
        assert_eq!(p[1].x, 12.0 + 48.0 + 9.0);
        // the divider's room comes before the third
        assert_eq!(p[2], Placed { x: 12.0 + 2.0 * 57.0 + 19.0, size: 48.0, lead: 19.0 });
    }

    #[test]
    fn magnification_is_a_cosine_over_two_and_a_half_icons() {
        assert_eq!(proximity(0.0, 0.0, 48.0), 1.0);
        assert!((proximity(60.0, 0.0, 48.0) - (0.5f32 * std::f32::consts::FRAC_PI_2).cos()).abs() < 1e-5);
        assert!(proximity(120.0, 0.0, 48.0).abs() < 1e-6);
        assert!(proximity(500.0, 0.0, 48.0).abs() < 1e-6);
        let g = glass(json!({"dock": {"magnification": true}}));
        let d = [false; 5];
        // the pointer over the middle icon: it's the biggest, neighbours less, ends least
        let (p, w) = place(&g, &d, Some(0.0));
        assert!((p[2].size - 48.0 * 1.45).abs() < 1e-3);
        assert!(p[1].size < p[2].size && p[1].size > p[0].size && (p[1].size - p[3].size).abs() < 1e-3);
        // (the zones' "largest" width is the QML's estimate: a row of five reaches about it)
        assert!(w > g.pill_rest(&d) && w <= g.max_width(&d) + 2.0);
        // off the dock: at rest
        let h = hit(&g, &d, Some((0.0, -300.0)), true, false);
        assert!(!h.hovered && h.placed.iter().all(|p| p.size == 48.0));
        // without magnification nothing grows
        let g = glass(json!({}));
        assert!(place(&g, &d, Some(0.0)).0.iter().all(|p| p.size == 48.0));
    }

    #[test]
    fn the_icon_under_the_pointer() {
        let g = glass(json!({}));
        let d = [false, false, true];
        let w = g.pill_rest(&d);
        let y = -(g.bottom_gap + g.bar_h / 2.0);
        // the first icon's middle, from the dock's centre
        let first = -w / 2.0 + 12.0 + 24.0;
        assert_eq!(hit(&g, &d, Some((first, y)), true, false).hot, Some(0));
        // the gap between icons belongs half to each
        let edge = -w / 2.0 + 12.0 + 48.0;
        assert_eq!(hit(&g, &d, Some((edge + 4.0, y)), true, false).hot, Some(0));
        assert_eq!(hit(&g, &d, Some((edge + 5.0, y)), true, false).hot, Some(1));
        // over the divider, nothing; on the pill's padding, nothing
        let p = place(&g, &d, None).0;
        assert_eq!(hit(&g, &d, Some((-w / 2.0 + p[2].x - 12.0, y)), true, false).hot, None);
        assert_eq!(hit(&g, &d, Some((-w / 2.0 + 2.0, y)), true, false).hot, None);
        // not while it slides, nor while a menu is open; nor below the pill
        assert_eq!(hit(&g, &d, Some((first, y)), false, false).hot, None);
        assert!(hit(&g, &d, Some((first, y)), true, true).hovered);
        assert_eq!(hit(&g, &d, Some((first, y)), true, true).hot, None);
        assert!(!hit(&g, &d, Some((first, -2.0)), true, false).hovered);
        // Glass counts the room above the pill (the lifted icon), Material only the pill
        assert!(hit(&g, &d, Some((first, -(g.bottom_gap + g.bar_h + 10.0))), true, false).hovered);
        let m = Metrics::new(&Settings::read(&json!({})), true);
        assert!(!hit(&m, &d, Some((0.0, -(m.bottom_gap + m.bar_h + 10.0))), true, false).hovered);
        assert!(hit(&m, &d, Some((0.0, -(m.bottom_gap + 2.0))), true, false).hovered);
    }

    #[test]
    fn input_shapes_per_auto_hide_state() {
        let g = glass(json!({}));
        let d = [false; 4];
        let (w, h) = (1920.0, g.window_height() as f32);
        // shown, never hides: just the pill's largest extent
        let r = input(&g, &d, w, h, false, false);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].1 + r[0].3, h as i32 - 6);
        // hidden: only the 2 px strip along the bottom edge
        assert_eq!(input(&g, &d, w, h, true, true), vec![(0, h as i32 - 2, 1920, 2)]);
        // revealed: the strip and the zone
        assert_eq!(input(&g, &d, w, h, true, false).len(), 2);
        let (on_edge, _) = edge_and_zone(&g, &d, w, h, Some((10.0, h - 1.0)));
        assert!(on_edge);
        let (on_edge, in_zone) = edge_and_zone(&g, &d, w, h, Some((960.0, h - 40.0)));
        assert!(!on_edge && in_zone);
        assert_eq!(edge_and_zone(&g, &d, w, h, Some((10.0, h - 40.0))), (false, false));
    }

    #[test]
    fn auto_hide_reveals_after_a_moment_and_hides_after_a_grace() {
        assert!(auto_hiding(AutoHide::Always, false));
        assert!(!auto_hiding(AutoHide::Windows, false) && auto_hiding(AutoHide::Windows, true));
        assert!(!auto_hiding(AutoHide::Off, true));
        assert!(hidden(true, false, false) && !hidden(true, true, false) && !hidden(true, false, true) && !hidden(false, false, false));

        let mut r = Reveal::default();
        // the pointer reaches the edge: the reveal delay starts
        let t = r.pointer(true, false);
        assert!(t.start_reveal && t.stop_hide);
        assert!(r.reveal().start_hide == false && r.revealed);
        // up into the zone: stays up
        assert_eq!(r.pointer(false, true), Timers { stop_reveal: true, ..Default::default() });
        // out of the zone: the grace starts; back in, it stops
        assert!(r.pointer(false, false).start_hide);
        assert!(r.pointer(false, true).stop_hide);
        assert!(r.pointer(false, false).start_hide);
        r.hide();
        assert!(!r.revealed);
        // passing over the edge and away before the delay: never revealed
        let mut r = Reveal::default();
        assert!(r.pointer(true, false).start_reveal);
        assert!(r.pointer(false, false).stop_reveal);
        assert!(!r.revealed);
    }
}

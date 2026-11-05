//! TodayScreen.qml's entrances and its exit, as values over time: the frost
//! behind, the panel's rectangle, radius and opacity, the content's blur
//! (drawn here as a fade: Slint has no blur), the rows' stagger and the
//! drop's ripple. Each is a property with segments, each segment easing
//! from where the last one left it to its own value, as the QML's
//! Parallel/SequentialAnimations of NumberAnimations without `from` do.

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Frame {
    pub frost: f32,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub r: f32,
    pub opacity: f32,
    pub blur: f32,
    pub rows: f32,
    pub ripple: f32,
}

/// Where the panel ends up: the Settings window's size, centred
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Target {
    pub sw: f32,
    pub sh: f32,
    pub w: f32,
    pub h: f32,
    pub x: f32,
    pub y: f32,
    pub r: f32,
}

/// The panel for a screen of `sw`×`sh` (radius: Theme.radius.panel + 8)
pub fn target(sw: f32, sh: f32, panel_radius: f32) -> Target {
    let w = 860f32.min(sw - 64.0);
    let h = 620f32.min(sh - 96.0);
    Target { sw, sh, w, h, x: ((sw - w) / 2.0).round(), y: ((sh - h) / 2.0).round(), r: panel_radius + 8.0 }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ease {
    Linear,
    OutCubic,
    InCubic,
    InOutCubic,
    InQuad,
}

impl Ease {
    pub fn at(self, x: f32) -> f32 {
        let x = x.clamp(0.0, 1.0);
        match self {
            Ease::Linear => x,
            Ease::OutCubic => 1.0 - (1.0 - x).powi(3),
            Ease::InCubic => x * x * x,
            Ease::InOutCubic => {
                if x < 0.5 {
                    4.0 * x * x * x
                } else {
                    1.0 - (-2.0 * x + 2.0).powi(3) / 2.0
                }
            }
            Ease::InQuad => x * x,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum P {
    Frost,
    X,
    Y,
    W,
    H,
    R,
    Opacity,
    Blur,
    Rows,
    Ripple,
}

#[derive(Clone, Copy, Debug)]
struct Seg {
    p: P,
    start: f32,
    dur: f32,
    to: f32,
    ease: Ease,
    /// ("from" given: the ripple starts at 0 each time)
    from: Option<f32>,
}

#[derive(Clone, Debug)]
pub struct Timeline {
    init: Frame,
    segs: Vec<Seg>,
}

fn get(f: &Frame, p: P) -> f32 {
    match p {
        P::Frost => f.frost,
        P::X => f.x,
        P::Y => f.y,
        P::W => f.w,
        P::H => f.h,
        P::R => f.r,
        P::Opacity => f.opacity,
        P::Blur => f.blur,
        P::Rows => f.rows,
        P::Ripple => f.ripple,
    }
}

fn set(f: &mut Frame, p: P, v: f32) {
    match p {
        P::Frost => f.frost = v,
        P::X => f.x = v,
        P::Y => f.y = v,
        P::W => f.w = v,
        P::H => f.h = v,
        P::R => f.r = v,
        P::Opacity => f.opacity = v,
        P::Blur => f.blur = v,
        P::Rows => f.rows = v,
        P::Ripple => f.ripple = v,
    }
}

impl Timeline {
    fn new(init: Frame) -> Self {
        Self { init, segs: Vec::new() }
    }
    fn to(&mut self, p: P, start: f32, dur: f32, to: f32, ease: Ease) {
        self.segs.push(Seg { p, start, dur, to, ease, from: None });
    }

    /// The values `t` ms in
    pub fn at(&self, t: f32) -> Frame {
        let mut f = self.init;
        // (segments of one property run one after another, in order)
        for p in [P::Frost, P::X, P::Y, P::W, P::H, P::R, P::Opacity, P::Blur, P::Rows, P::Ripple] {
            let mut v = get(&self.init, p);
            for s in self.segs.iter().filter(|s| s.p == p) {
                if t < s.start {
                    break;
                }
                let from = s.from.unwrap_or(v);
                let k = if s.dur <= 0.0 { 1.0 } else { (t - s.start) / s.dur };
                v = from + (s.to - from) * s.ease.at(k);
            }
            set(&mut f, p, v);
        }
        f
    }

    /// How long it runs, ms
    pub fn length(&self) -> f32 {
        self.segs.iter().map(|s| s.start + s.dur).fold(0.0, f32::max)
    }
}

/// An entrance: "bloom", "shrink" or "drop"; `still`: Reduce motion (a
/// 200 ms fade); `ms`: the motion scale (cool-down's shorter animations)
pub fn enter(fx: &str, still: bool, ms: f32, t: &Target) -> Timeline {
    let d = |v: f32| v * ms;
    if still || fx == "bloom" {
        let grow = if still { 1.0 } else { 1.06 };
        let (w, h) = (t.w * grow, t.h * grow);
        let init = Frame { x: t.x - (w - t.w) / 2.0, y: t.y - (h - t.h) / 2.0, w, h, r: t.r, blur: if still { 0.0 } else { 1.0 }, ..Default::default() };
        let mut tl = Timeline::new(init);
        if still {
            for p in [P::Frost, P::Opacity, P::Rows] {
                tl.to(p, 0.0, 200.0, 1.0, Ease::Linear);
            }
            return tl;
        }
        tl.to(P::Frost, 0.0, d(380.0), 1.0, Ease::OutCubic);
        let s = d(140.0);
        tl.to(P::Opacity, s, d(320.0), 1.0, Ease::OutCubic);
        for (p, v) in [(P::W, t.w), (P::H, t.h), (P::X, t.x), (P::Y, t.y), (P::Blur, 0.0)] {
            tl.to(p, s, d(520.0), v, Ease::OutCubic);
        }
        tl.to(P::Rows, s + d(120.0), d(620.0), 1.0, Ease::Linear);
        tl
    } else if fx == "shrink" {
        let mut tl = Timeline::new(Frame { w: t.sw, h: t.sh, opacity: 1.0, ..Default::default() });
        tl.to(P::Frost, 0.0, d(260.0), 1.0, Ease::OutCubic);
        for (p, v) in [(P::W, t.w), (P::H, t.h), (P::X, t.x), (P::Y, t.y), (P::R, t.r)] {
            tl.to(p, 0.0, d(640.0), v, Ease::InOutCubic);
        }
        tl.to(P::Rows, d(420.0), d(620.0), 1.0, Ease::Linear);
        tl
    } else {
        // drop: a 44 px drop falls from above the top, lands mid-screen
        let dd = 44.0;
        let mut tl = Timeline::new(Frame { w: dd, h: dd, r: dd / 2.0, x: t.sw / 2.0 - dd / 2.0, y: -dd, opacity: 1.0, ..Default::default() });
        tl.to(P::Frost, 0.0, d(420.0), 1.0, Ease::OutCubic);
        tl.to(P::Y, 0.0, d(420.0), t.sh / 2.0 - 22.0, Ease::InQuad);
        let s = d(420.0);
        tl.segs.push(Seg { p: P::Ripple, start: s, dur: d(700.0), to: 1.0, ease: Ease::OutCubic, from: Some(0.0) });
        for (p, v) in [(P::W, t.w), (P::H, t.h), (P::X, t.x), (P::Y, t.y), (P::R, t.r)] {
            tl.to(p, s, d(560.0), v, Ease::OutCubic);
        }
        tl.to(P::Rows, s + d(300.0), d(620.0), 1.0, Ease::Linear);
        tl
    }
}

/// The exit, from wherever it is: it fades and settles a little smaller
pub fn exit(from: Frame, ms: f32, t: &Target) -> Timeline {
    let d = |v: f32| v * ms;
    let mut tl = Timeline::new(from);
    tl.to(P::Rows, 0.0, d(140.0), 0.0, Ease::InCubic);
    tl.to(P::Opacity, 0.0, d(240.0), 0.0, Ease::InCubic);
    tl.to(P::W, 0.0, d(240.0), t.w * 0.97, Ease::InCubic);
    tl.to(P::H, 0.0, d(240.0), t.h * 0.97, Ease::InCubic);
    tl.to(P::X, 0.0, d(240.0), t.x + t.w * 0.015, Ease::InCubic);
    tl.to(P::Y, 0.0, d(240.0), t.y + t.h * 0.015, Ease::InCubic);
    tl.to(P::Frost, 0.0, d(280.0), 0.0, Ease::InCubic);
    tl
}

/// Row n of the stagger (TodayContent's appear): 40 ms apart over the
/// 620 ms of `rows`, each fading in over 300 ms
pub fn appear(rows: f32, order: i32) -> f32 {
    ((rows * 620.0 - order as f32 * 40.0) / 300.0).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.01
    }

    #[test]
    fn the_panel_is_the_settings_windows_size() {
        let t = target(1920.0, 1080.0, 26.0);
        assert_eq!((t.w, t.h, t.x, t.y, t.r), (860.0, 620.0, 530.0, 230.0, 34.0));
        let small = target(800.0, 600.0, 28.0);
        assert_eq!((small.w, small.h, small.r), (736.0, 504.0, 36.0));
    }

    #[test]
    fn every_entrance_ends_on_the_panel() {
        let t = target(1920.0, 1080.0, 26.0);
        for fx in ["bloom", "shrink", "drop"] {
            for (still, ms) in [(false, 1.0), (false, 0.6), (true, 1.0)] {
                let tl = enter(fx, still, ms, &t);
                let f = tl.at(tl.length() + 1.0);
                assert!(close(f.x, t.x) && close(f.y, t.y) && close(f.w, t.w) && close(f.h, t.h) && close(f.r, t.r), "{fx} {still}: {f:?}");
                assert!(close(f.frost, 1.0) && close(f.opacity, 1.0) && close(f.rows, 1.0) && close(f.blur, 0.0), "{fx}: {f:?}");
            }
        }
        // the lengths the chime is timed to (Today.qml: bloom 660 settles, the rows after)
        assert!(close(enter("bloom", false, 1.0, &t).length(), 880.0));
        assert!(close(enter("shrink", false, 1.0, &t).length(), 1040.0));
        assert!(close(enter("drop", false, 1.0, &t).length(), 1340.0));
        assert!(close(enter("drop", true, 1.0, &t).length(), 200.0));
    }

    #[test]
    fn how_each_entrance_begins() {
        let t = target(1920.0, 1080.0, 26.0);
        let b = enter("bloom", false, 1.0, &t).at(0.0);
        assert!(close(b.w, 860.0 * 1.06) && b.opacity == 0.0 && b.blur == 1.0);
        // bloom: the panel waits 140 ms for the frost
        assert_eq!(enter("bloom", false, 1.0, &t).at(140.0).opacity, 0.0);
        let s = enter("shrink", false, 1.0, &t).at(0.0);
        assert_eq!((s.w, s.h, s.r, s.opacity), (1920.0, 1080.0, 0.0, 1.0));
        let d = enter("drop", false, 1.0, &t);
        assert_eq!((d.at(0.0).w, d.at(0.0).y), (44.0, -44.0));
        // it lands mid-screen at 420 ms, still a drop, and the ring starts
        let landed = d.at(420.0);
        assert!(close(landed.y, 518.0) && close(landed.w, 44.0) && landed.ripple == 0.0);
        assert!(d.at(700.0).ripple > 0.5 && d.at(700.0).w > 44.0);
    }

    #[test]
    fn the_exit_settles_smaller_and_fades() {
        let t = target(1920.0, 1080.0, 26.0);
        let from = enter("bloom", false, 1.0, &t).at(2000.0);
        let tl = exit(from, 1.0, &t);
        assert!(close(tl.length(), 280.0));
        let f = tl.at(300.0);
        assert!(close(f.w, 860.0 * 0.97) && close(f.x, 530.0 + 860.0 * 0.015) && f.opacity == 0.0 && f.frost == 0.0 && f.rows == 0.0);
        // (from halfway in: it starts from there, no jump)
        let mid = enter("shrink", false, 1.0, &t).at(300.0);
        assert!(close(exit(mid, 1.0, &t).at(0.0).w, mid.w));
    }

    #[test]
    fn rows_rise_one_after_another() {
        assert_eq!(appear(0.0, 0), 0.0);
        assert_eq!(appear(1.0, 0), 1.0);
        assert!(appear(0.25, 0) > appear(0.25, 2));
        // (as the QML: rows ordered past 7 never quite arrive; Glass's
        // buttons, order 10, rest at 73%)
        assert!(close(appear(1.0, 7), 1.0));
        assert!(close(appear(1.0, 10), 0.7333));
    }
}

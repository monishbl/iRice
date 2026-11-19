//! A group's cards stacked as iOS does (19 Nov: "make it like iOS stack"):
//! the newest card whole, up to two older ones peeking out under it, each
//! narrower (8 px a side a step), a shorter peek (8 px, then 6) and fainter,
//! with the same corners and surface as the front card. Tapping the stack
//! fans the cards out in order; Show less folds them back.
//!
//! What's here is the maths, pure: the depths, peeks, fades and heights; the
//! outline of each peeking card (what shows of it under the card in front,
//! never a flat bar, never drawn under a translucent card, so nothing shows
//! twice), as SVG path commands for Slint's Path; the fan-out's starting
//! sizes and timing; and which group is open or folding. Notification
//! Center (panel.rs) and the banners (lib.rs) both use it.

use std::collections::HashSet;

/// How far in each side a card one step back sits
pub const INSET: f32 = 8.0;
/// How much of each card behind shows under the one in front of it
pub const PEEKS: [f32; 2] = [8.0, 6.0];
/// Each depth's opacity (0 the front card)
pub const FADES: [f32; 3] = [1.0, 0.8, 0.6];
/// At most two cards peek out
pub const MAX_DEPTH: usize = 2;

/// How many cards peek out under the front one of a stack of `count`
pub fn depth(count: usize) -> usize {
    count.saturating_sub(1).min(MAX_DEPTH)
}

/// How far below the front card the stack reaches with `depth` cards behind
pub fn peek(depth: usize) -> f32 {
    PEEKS[..depth.min(MAX_DEPTH)].iter().sum()
}

/// A card's opacity at its depth (past the second: not drawn)
pub fn fade(depth: usize) -> f32 {
    FADES.get(depth).copied().unwrap_or(0.0)
}

/// The scale that makes a card `depth` steps back `INSET` px narrower a side
pub fn scale(width: f32, depth: usize) -> f32 {
    if width <= 0.0 {
        return 1.0;
    }
    1.0 - 2.0 * INSET * depth.min(MAX_DEPTH) as f32 / width
}

/// A folded stack's height: the front card and what peeks out under it
pub fn collapsed_height(front: f32, count: usize) -> f32 {
    if count == 0 {
        return 0.0;
    }
    front + peek(depth(count))
}

/// An open group's height: its header, then each card with the gap above it
pub fn expanded_height(header: f32, heights: &[f32], gap: f32) -> f32 {
    header + heights.iter().map(|h| h + gap).sum::<f32>()
}

/// The count on a folded stack ("2 more"), "" for a single card
pub fn more(count: usize) -> String {
    if count > 1 { format!("{} more", count - 1) } else { String::new() }
}

// ── Fanning out and folding back ────────────────────────────────────────────

/// The height a card `k` places down an opening group starts at (0 the front
/// card, which doesn't move): what showed of it in the folded stack, so the
/// list doesn't jump when the rows come; it grows to its own height
pub fn fan_from(k: usize) -> f32 {
    match k {
        1 => PEEKS[0],
        2 => PEEKS[1],
        _ => 0.0,
    }
}

/// The scale and opacity a card `k` down starts at: as it sat in the stack
pub fn fan_start(width: f32, k: usize) -> (f32, f32) {
    match k {
        0 => (1.0, 1.0),
        _ => (scale(width, k), fade(k.min(MAX_DEPTH))),
    }
}

/// How long card `k` (of `n`) waits before it moves: opening, each a beat
/// after the one above it; folding, the last first
pub fn stagger_ms(k: usize, n: usize, folding: bool) -> i32 {
    if k == 0 {
        return 0;
    }
    if folding { 20 * (n.saturating_sub(1 + k)).min(6) as i32 } else { 30 * (k - 1).min(6) as i32 }
}

/// A fold's length (the cards' 220 ms ease-in plus the longest wait), after
/// which the group is closed for real; 0 with Reduce motion (it snaps)
pub fn fold_ms(n: usize, still: bool) -> u64 {
    if still {
        return 0;
    }
    220 + 20 * n.saturating_sub(2).min(6) as u64 + 30
}

/// Which groups are open in Notification Center, and the one folding back
/// (its rows stay while they fold, then it closes)
#[derive(Debug, Default)]
pub struct Groups {
    pub open: HashSet<String>,
    pub folding: Option<String>,
    /// the fold's number: a fold cancelled (opened again, cleared) and
    /// started anew isn't closed by the old one's timer
    pub generation: u64,
}

/// What a tap on a group's stack or its Show less did
#[derive(Debug, PartialEq, Eq)]
pub enum Toggled {
    Opened,
    /// folding: close it when the fold is over (its number, after ms)
    Folding(u64, u64),
    Closed,
    /// tapped again while folding: it opens back out
    Reopened,
}

impl Groups {
    pub fn is_open(&self, key: &str) -> bool {
        self.open.contains(key)
    }

    pub fn is_folding(&self, key: &str) -> bool {
        self.folding.as_deref() == Some(key)
    }

    /// The stack (of `n`) tapped, or Show less
    pub fn toggle(&mut self, key: &str, n: usize, still: bool) -> Toggled {
        if self.is_folding(key) {
            self.folding = None;
            self.generation += 1;
            return Toggled::Reopened;
        }
        if !self.open.contains(key) {
            if n > 1 {
                self.open.insert(key.to_string());
            }
            return Toggled::Opened;
        }
        let ms = fold_ms(n, still);
        if ms == 0 {
            self.open.remove(key);
            return Toggled::Closed;
        }
        self.generation += 1;
        self.folding = Some(key.to_string());
        Toggled::Folding(self.generation, ms)
    }

    /// A fold's time is up: closed (true) unless it was cancelled since
    pub fn folded(&mut self, generation: u64) -> bool {
        if generation != self.generation {
            return false;
        }
        match self.folding.take() {
            Some(k) => {
                self.open.remove(&k);
                true
            }
            None => false,
        }
    }

    /// The group cleared: neither open nor folding
    pub fn clear(&mut self, key: &str) {
        self.open.remove(key);
        if self.is_folding(key) {
            self.folding = None;
            self.generation += 1;
        }
    }

    /// Notification Center opened anew: every group folded
    pub fn reset(&mut self) {
        self.open.clear();
        if self.folding.take().is_some() {
            self.generation += 1;
        }
    }
}

// ── The cards' outlines ─────────────────────────────────────────────────────

/// A card's box and its corner radius
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RBox {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub r: f32,
}

impl RBox {
    pub fn new(x: f32, y: f32, w: f32, h: f32, r: f32) -> Self {
        Self { x, y, w, h, r }
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
}

/// The card `depth` steps behind `front` in Notification Center: as tall,
/// INSET px in a side a step, its foot the peeks below the front's (the
/// same corners: its top is hidden under the cards in front)
pub fn behind(front: RBox, depth: usize) -> RBox {
    let d = depth.min(MAX_DEPTH) as f32;
    RBox::new(front.x + INSET * d, front.y + peek(depth), front.w - 2.0 * INSET * d, front.h, front.r)
}

/// A banner's card `depth` steps behind, as drawn: the front card's plate
/// scaled about its bottom centre (narrower by INSET a side a step; its
/// corners scale with it) and moved down the peeks
pub fn behind_scaled(front: RBox, depth: usize) -> RBox {
    let s = scale(front.w, depth);
    let (w, h) = (front.w * s, front.h * s);
    RBox::new(front.x + (front.w - w) / 2.0, front.y + peek(depth) + front.h - h, w, h, front.r * s)
}

/// The corner's shape (core/ui/squircle.slint's SquirclePath, which draws
/// the cards, and the glass plugin's sdRoundBox): how far along each side
/// it reaches, and its cubic's handle point (from the corner)
fn corner(b: &RBox, circular: bool) -> (f32, f32) {
    let hm = (b.w.min(b.h) / 2.0).max(0.001);
    let r = b.r.min(hm).max(0.0);
    let t = if circular { 1.0 } else { ((r / hm - 0.65) / 0.35).clamp(0.0, 1.0) };
    let k = if circular { 1.0 } else { t * t * (3.0 - 2.0 * t) };
    let n = 4.0 - 2.0 * k;
    let re = (r * (1.28 - 0.28 * k)).min(hm);
    let c = re * (1.0 - (8.0 * 2f32.powf(-1.0 / n) - 4.0) / 3.0);
    (re, c)
}

fn cubic(a: f32, b: f32, c: f32, d: f32, t: f32) -> f32 {
    let u = 1.0 - t;
    u * u * u * a + 3.0 * u * u * t * b + 3.0 * u * t * t * c + t * t * t * d
}

/// How far the outline is in from the box's top or bottom `dx` px in from
/// its side (the corner's cubic: x re → 0, the edge's distance 0 → re)
fn corner_in(dx: f32, re: f32, c: f32) -> f32 {
    if dx >= re {
        return 0.0;
    }
    if dx <= 0.0 {
        return re;
    }
    // (x falls from re to 0 along the cubic: halve the interval)
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for _ in 0..28 {
        let mid = (lo + hi) / 2.0;
        if cubic(re, c, 0.0, 0.0, mid) > dx {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    cubic(0.0, 0.0, c, re, (lo + hi) / 2.0)
}

/// The outline's top and bottom at `x` (none outside the box)
fn span(b: &RBox, x: f32, circular: bool) -> Option<(f32, f32)> {
    if x < b.x - 1e-3 || x > b.x + b.w + 1e-3 || b.w <= 0.0 || b.h <= 0.0 {
        return None;
    }
    let (re, c) = corner(b, circular);
    let dx = (x - b.x).min(b.x + b.w - x).max(0.0);
    let i = corner_in(dx, re, c);
    Some((b.y + i, b.bottom() - i))
}

/// One piece of what shows of a card: its outline's top (left to right) and
/// bottom (left to right) at the same x's
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Piece {
    pub top: Vec<(f32, f32)>,
    pub bottom: Vec<(f32, f32)>,
}

/// What shows of `own` behind the cards `over` (in front of it, covering
/// its top): its outline less theirs. Sampled every half pixel along the
/// corners (straight between them), the meeting points found exactly
pub fn visible(own: RBox, over: &[RBox], circular: bool) -> Vec<Piece> {
    if own.w <= 0.0 || own.h <= 0.0 {
        return Vec::new();
    }
    // where the outlines bend: every box's corners, inside this one's width
    let (x0, x1) = (own.x, own.x + own.w);
    let mut xs = vec![x0, x1];
    for b in std::iter::once(&own).chain(over) {
        let (re, _) = corner(b, circular);
        for (a, z) in [(b.x, b.x + re), (b.x + b.w - re, b.x + b.w)] {
            let mut x = a;
            while x <= z + 1e-3 {
                if x >= x0 && x <= x1 {
                    xs.push(x);
                }
                x += 0.5;
            }
        }
    }
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    xs.dedup_by(|a, b| (*a - *b).abs() < 0.01);
    // at x: the top of what shows (under every card in front reaching down
    // past this one's top) and its bottom
    let at = |x: f32| -> Option<(f32, f32)> {
        let (t, b) = span(&own, x, circular)?;
        let mut top = t;
        for o in over {
            if let Some((ot, ob)) = span(o, x, circular) {
                if ot <= top + 0.01 && ob > top {
                    top = ob;
                }
            }
        }
        Some((top, b))
    };
    let shows = |x: f32| at(x).is_some_and(|(t, b)| b - t > 0.01);
    // (where it starts or stops showing between two samples)
    let edge = |a: f32, b: f32| -> f32 {
        let (mut lo, mut hi) = (a, b);
        let start = shows(a);
        for _ in 0..24 {
            let mid = (lo + hi) / 2.0;
            if shows(mid) == start {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        if start { lo } else { hi }
    };
    let mut out = Vec::new();
    let mut cur: Option<Piece> = None;
    let push = |p: &mut Piece, x: f32| {
        if let Some((t, b)) = at(x) {
            p.top.push((x, t.min(b)));
            p.bottom.push((x, b));
        }
    };
    // (the point where it comes into view or goes, both outlines meeting)
    let meet = |p: &mut Piece, a: f32, b: f32| {
        let e = edge(a, b);
        if let Some((_, y)) = at(e) {
            p.top.push((e, y));
            p.bottom.push((e, y));
        }
    };
    for (i, &x) in xs.iter().enumerate() {
        match (cur.is_some(), shows(x)) {
            (false, true) => {
                let mut p = Piece::default();
                if i > 0 {
                    meet(&mut p, xs[i - 1], x);
                }
                push(&mut p, x);
                cur = Some(p);
            }
            (true, true) => push(cur.as_mut().unwrap(), x),
            (true, false) => {
                let mut p = cur.take().unwrap();
                meet(&mut p, xs[i - 1], x);
                out.push(p);
            }
            (false, false) => {}
        }
    }
    if let Some(p) = cur {
        out.push(p);
    }
    out
}

fn fmt(p: (f32, f32)) -> String {
    format!("{:.2} {:.2}", p.0, p.1)
}

/// The pieces' outlines as path commands, each point moved by `map`
pub fn fill_path(pieces: &[Piece], map: impl Fn((f32, f32)) -> (f32, f32)) -> String {
    let mut s = String::new();
    for p in pieces {
        if p.top.is_empty() {
            continue;
        }
        s.push_str(&format!("M {}", fmt(map(p.top[0]))));
        for &q in &p.top[1..] {
            s.push_str(&format!(" L {}", fmt(map(q))));
        }
        for &q in p.bottom.iter().rev() {
            s.push_str(&format!(" L {}", fmt(map(q))));
        }
        s.push_str(" Z ");
    }
    if s.is_empty() { EMPTY.into() } else { s.trim_end().to_string() }
}

/// Only the pieces' lower outlines (a hairline along the card's edge; not
/// along the card in front of it)
pub fn edge_path(pieces: &[Piece], map: impl Fn((f32, f32)) -> (f32, f32)) -> String {
    let mut s = String::new();
    for p in pieces {
        if p.bottom.len() < 2 {
            continue;
        }
        s.push_str(&format!("M {}", fmt(map(p.bottom[0]))));
        for &q in &p.bottom[1..] {
            s.push_str(&format!(" L {}", fmt(map(q))));
        }
        s.push(' ');
    }
    if s.is_empty() { EMPTY.into() } else { s.trim_end().to_string() }
}

/// Nothing to draw (a path Slint still reads)
pub const EMPTY: &str = "M 0 0 Z";

/// What a folded stack's card `depth` back shows in Notification Center, in
/// the stack's own pixels (the front card at 0,0 `w`×`h`): its fill, or
/// (`edge`) the hairline along its foot, half a pixel in
pub fn nc_path(w: f32, h: f32, r: f32, depth: usize, circular: bool, edge: bool) -> String {
    if depth == 0 || depth > MAX_DEPTH || w <= 0.0 || h <= 0.0 {
        return EMPTY.into();
    }
    let front = RBox::new(0.0, 0.0, w, h, r);
    let over: Vec<RBox> = (0..depth).map(|d| behind(front, d)).collect();
    let mut own = behind(front, depth);
    if edge {
        own = RBox::new(own.x + 0.5, own.y + 0.5, own.w - 1.0, own.h - 1.0, (own.r - 0.5).max(0.0));
    }
    let pieces = visible(own, &over, circular);
    if edge { edge_path(&pieces, |p| p) } else { fill_path(&pieces, |p| p) }
}

/// The same for a banner's back card, in its own pixels before its scale
/// (the card is a plate `w`×`h` scaled about its bottom centre, which sits
/// the peeks under the front card's foot)
pub fn banner_path(w: f32, h: f32, r: f32, depth: usize, circular: bool, edge: bool) -> String {
    if depth == 0 || depth > MAX_DEPTH || w <= 0.0 || h <= 0.0 {
        return EMPTY.into();
    }
    let front = RBox::new(0.0, 0.0, w, h, r);
    let over: Vec<RBox> = (0..depth).map(|d| if d == 0 { front } else { behind_scaled(front, d) }).collect();
    let mut own = behind_scaled(front, depth);
    let s = scale(w, depth);
    if edge {
        let i = 0.5 * s;
        own = RBox::new(own.x + i, own.y + i, own.w - 2.0 * i, own.h - 2.0 * i, (own.r - i).max(0.0));
    }
    let pieces = visible(own, &over, circular);
    // (on screen → the card's own pixels: undo the scale about its foot)
    let foot = peek(depth) + h;
    let map = |(x, y): (f32, f32)| ((x - w / 2.0) / s + w / 2.0, (y - foot) / s + h);
    if edge { edge_path(&pieces, map) } else { fill_path(&pieces, map) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depths_peeks_and_fades() {
        assert_eq!((depth(0), depth(1), depth(2), depth(3), depth(9)), (0, 0, 1, 2, 2));
        assert_eq!((peek(0), peek(1), peek(2), peek(5)), (0.0, 8.0, 14.0, 14.0));
        assert_eq!((fade(0), fade(1), fade(2), fade(3)), (1.0, 0.8, 0.6, 0.0));
        // narrower by 8 px a side a step
        assert!((340.0 * scale(340.0, 1) - 324.0).abs() < 1e-3);
        assert!((340.0 * scale(340.0, 2) - 308.0).abs() < 1e-3);
        assert_eq!(scale(340.0, 0), 1.0);
    }

    #[test]
    fn heights_and_counts() {
        assert_eq!(collapsed_height(70.0, 1), 70.0);
        assert_eq!(collapsed_height(70.0, 2), 78.0);
        assert_eq!(collapsed_height(70.0, 3), 84.0);
        assert_eq!(collapsed_height(70.0, 12), 84.0);
        assert_eq!(collapsed_height(70.0, 0), 0.0);
        assert_eq!(expanded_height(24.0, &[70.0, 90.0, 60.0], 6.0), 24.0 + 70.0 + 90.0 + 60.0 + 18.0);
        assert_eq!(more(1), "");
        assert_eq!(more(3), "2 more");
    }

    #[test]
    fn fanning_starts_where_the_stack_was() {
        // the rows' heights at the start add up to the folded stack's peek
        assert_eq!(fan_from(1) + fan_from(2) + fan_from(3) + fan_from(4), peek(2));
        assert_eq!(fan_from(0), 0.0);
        assert_eq!(fan_start(340.0, 0), (1.0, 1.0));
        let (s, o) = fan_start(340.0, 1);
        assert!((s - scale(340.0, 1)).abs() < 1e-6 && o == 0.8);
        assert_eq!(fan_start(340.0, 5).1, 0.6);
        // opening: top down; folding: the last first
        assert_eq!((stagger_ms(1, 4, false), stagger_ms(2, 4, false), stagger_ms(3, 4, false)), (0, 30, 60));
        assert_eq!((stagger_ms(1, 4, true), stagger_ms(2, 4, true), stagger_ms(3, 4, true)), (40, 20, 0));
        assert_eq!(stagger_ms(0, 4, true), 0);
        assert!(stagger_ms(40, 50, false) <= 180);
        assert_eq!(fold_ms(4, true), 0);
        assert!(fold_ms(4, false) >= 220 + stagger_ms(1, 4, true) as u64);
    }

    #[test]
    fn open_fold_close_and_clear() {
        let mut g = Groups::default();
        assert_eq!(g.toggle("Mail", 3, false), Toggled::Opened);
        assert!(g.is_open("Mail"));
        // Show less: it folds, still open, then closes when the time's up
        let Toggled::Folding(n, ms) = g.toggle("Mail", 3, false) else { panic!() };
        assert!(ms > 0 && g.is_open("Mail") && g.is_folding("Mail"));
        assert!(g.folded(n));
        assert!(!g.is_open("Mail") && g.folding.is_none());
        assert!(!g.folded(n)); // (once)
        // tapped again while folding: back open, and the old timer does nothing
        g.toggle("Mail", 3, false);
        let Toggled::Folding(n, _) = g.toggle("Mail", 3, false) else { panic!() };
        assert_eq!(g.toggle("Mail", 3, false), Toggled::Reopened);
        assert!(!g.folded(n) && g.is_open("Mail"));
        // Reduce motion: it closes at once
        assert_eq!(g.toggle("Mail", 3, true), Toggled::Closed);
        assert!(!g.is_open("Mail"));
        // cleared while folding: gone, and the timer does nothing
        g.toggle("Chat", 2, false);
        let Toggled::Folding(n, _) = g.toggle("Chat", 2, false) else { panic!() };
        g.clear("Chat");
        assert!(!g.folded(n) && !g.is_open("Chat") && g.folding.is_none());
        // a single card never opens; opening anew folds everything
        g.toggle("Files", 1, false);
        assert!(!g.is_open("Files"));
        g.toggle("Mail", 2, false);
        g.reset();
        assert!(g.open.is_empty());
    }

    fn area(pieces: &[Piece]) -> f32 {
        pieces
            .iter()
            .map(|p| p.top.windows(2).zip(p.bottom.windows(2)).map(|(t, b)| (t[1].0 - t[0].0) * ((b[0].1 - t[0].1) + (b[1].1 - t[1].1)) / 2.0).sum::<f32>())
            .sum()
    }

    /// The bug (19 Nov): the cards behind were flat 12 px bars, drawn half
    /// under the translucent front card (showing through it), cut square.
    /// Now each shows only below the card in front, as wide as its own
    /// outline, with the same rounded foot
    #[test]
    fn what_peeks_out_is_only_below_the_card_in_front() {
        for circular in [false, true] {
            let (w, h, r) = (340.0, 70.0, 18.0);
            let front = RBox::new(0.0, 0.0, w, h, r);
            let b1 = behind(front, 1);
            let b2 = behind(front, 2);
            assert_eq!((b1.x, b1.w, b1.bottom()), (8.0, 324.0, 78.0));
            assert_eq!((b2.x, b2.w, b2.bottom()), (16.0, 308.0, 84.0));
            for (own, over) in [(b1, vec![front]), (b2, vec![front, b1])] {
                let p = visible(own, &over, circular);
                assert_eq!(p.len(), 1, "one piece");
                let p = &p[0];
                for (&(x, t), &(_, b)) in p.top.iter().zip(&p.bottom) {
                    assert!(t <= b + 1e-3);
                    // never inside a card in front
                    for o in &over {
                        if let Some((ot, ob)) = span(o, x, circular) {
                            assert!(t >= ob - 0.05 || t <= ot, "x {x}: {t} inside a front card ({ot}..{ob})");
                        }
                    }
                    // never outside its own outline
                    let (st, sb) = span(&own, x, circular).unwrap();
                    assert!(t >= st - 1e-3 && b <= sb + 1e-3);
                }
                // its foot is its own: the middle shows the whole peek
                assert!(p.bottom.iter().any(|q| (q.1 - own.bottom()).abs() < 1e-3));
                let peek_here = own.bottom() - over.last().unwrap().bottom();
                let a = area(&[p.clone()]);
                // nearly its width × its peek, less the rounded ends
                assert!(a > peek_here * (own.w - 2.0 * 23.1) && a < peek_here * own.w, "area {a}");
                // left and right mirror each other
                let (l, rgt) = (p.top.first().unwrap(), p.top.last().unwrap());
                assert!((l.0 - own.x - (own.x + own.w - rgt.0)).abs() < 0.05, "{l:?} {rgt:?}");
            }
        }
    }

    #[test]
    fn a_card_with_nothing_in_front_is_whole() {
        let own = RBox::new(0.0, 0.0, 100.0, 50.0, 12.0);
        let p = visible(own, &[], true);
        assert_eq!(p.len(), 1);
        let a = area(&p);
        let whole = 100.0 * 50.0 - (4.0 - std::f32::consts::PI) * 144.0;
        assert!((a - whole).abs() < 6.0, "{a} vs {whole}");
        // covered whole: nothing
        assert!(visible(own, &[RBox::new(-10.0, -10.0, 120.0, 80.0, 12.0)], true).is_empty());
    }

    #[test]
    fn paths_for_slint() {
        assert_eq!(nc_path(340.0, 70.0, 18.0, 0, false, false), EMPTY);
        assert_eq!(nc_path(0.0, 70.0, 18.0, 1, false, false), EMPTY);
        let f = nc_path(340.0, 70.0, 18.0, 1, false, false);
        assert!(f.starts_with("M ") && f.ends_with('Z') && f.matches('M').count() == 1, "{f}");
        let e = nc_path(340.0, 70.0, 18.0, 2, false, true);
        assert!(e.starts_with("M ") && !e.contains('Z'));
        // every point inside the stack's own box (the Path's viewbox)
        for s in [f, e, nc_path(360.0, 96.0, 24.0, 2, true, false)] {
            let nums: Vec<f32> = s.split(' ').filter_map(|t| t.parse().ok()).collect();
            for xy in nums.chunks(2) {
                assert!(xy[0] >= -0.01 && xy[0] <= 360.01 && xy[1] >= 0.0 && xy[1] <= 96.0 + 14.01, "{xy:?}");
            }
        }
    }

    #[test]
    fn a_banners_back_card_in_its_own_pixels() {
        // drawn at its scale about its foot, the path lands under the front
        // card: back on screen, its points are below the card in front
        let (w, h, r) = (360.0, 90.0, 22.0);
        for d in 1..=2 {
            let s = scale(w, d);
            let foot = peek(d) + h;
            let p = banner_path(w, h, r, d, false, false);
            let nums: Vec<f32> = p.split(' ').filter_map(|t| t.parse().ok()).collect();
            assert!(nums.len() > 20);
            let front_foot = if d == 1 { h } else { peek(1) + h };
            for xy in nums.chunks(2) {
                let (x, y) = ((xy[0] - w / 2.0) * s + w / 2.0, (xy[1] - h) * s + foot);
                assert!(x >= INSET * d as f32 - 0.05 && x <= w - INSET * d as f32 + 0.05, "{x}");
                assert!(y >= front_foot - 30.0 && y <= foot + 0.05, "{y}");
                // in its own pixels it stays on its plate
                assert!(xy[0] >= -0.05 && xy[0] <= w + 0.05 && xy[1] >= 0.0 && xy[1] <= h + 0.05);
            }
        }
    }
}

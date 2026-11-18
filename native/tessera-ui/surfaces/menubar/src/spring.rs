//! The HUD level's spring, stepped here rather than by a Slint `animate`.
//!
//! IslandEvent.qml's hudBar width runs on a Qt SpringAnimation (spring 5,
//! damping 0.4, epsilon 0.5 px, mass 1). A new target while it moves keeps
//! the speed it has: a held volume key (a step every ~40 ms) glides on. A
//! Slint `animate` starts over at every change instead, from rest on its
//! curve, so the bar braked at each step and the HUD stuttered while a key
//! was held (18 Nov). This is Qt's own step (16 ms; the value moves
//! `velocity * 16 / 1000` each step) with the frames in between drawn
//! between two steps, so 60 and 120 Hz show the same motion.

/// Qt's step, ms
pub const STEP_MS: f64 = 16.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Spring {
    pub spring: f64,
    pub damping: f64,
    /// stops (at its target) once nearer than this and slower than this
    pub epsilon: f64,
    /// where the last whole step left it, and where the one before did
    x: f64,
    prev: f64,
    v: f64,
    target: f64,
    /// ms into the next step
    acc: f64,
    moving: bool,
}

impl Spring {
    /// At rest at `x`. Units are the caller's (Qt's are pixels: its epsilon
    /// 0.5 px is given in the same unit)
    pub fn at(x: f64, spring: f64, damping: f64, epsilon: f64) -> Self {
        Self { spring, damping, epsilon, x, prev: x, v: 0.0, target: x, acc: 0.0, moving: false }
    }

    /// A new target: from where it is, with the speed it has
    pub fn retarget(&mut self, to: f64) {
        if (to - self.target).abs() > f64::EPSILON {
            self.target = to;
            self.moving = true;
        }
    }

    /// Straight there, at rest (Material's slider, Reduce motion)
    pub fn jump(&mut self, to: f64) {
        *self = Self::at(to, self.spring, self.damping, self.epsilon);
    }

    pub fn target(&self) -> f64 {
        self.target
    }

    pub fn moving(&self) -> bool {
        self.moving
    }

    /// `ms` later: the whole steps that fit, the remainder kept for next time
    pub fn advance(&mut self, ms: f64) {
        if !self.moving {
            return;
        }
        // (a frame after a long gap, a hidden window: at most a second of steps)
        self.acc += ms.clamp(0.0, 1000.0);
        while self.acc >= STEP_MS {
            self.acc -= STEP_MS;
            self.prev = self.x;
            let diff = self.target - self.x;
            self.v += self.spring * diff - self.damping * self.v;
            self.x += self.v * STEP_MS / 1000.0;
            if (self.target - self.x).abs() < self.epsilon && self.v.abs() < self.epsilon {
                self.jump(self.target);
                return;
            }
        }
    }

    /// The value to draw now: between the last two steps
    pub fn value(&self) -> f64 {
        if !self.moving {
            return self.x;
        }
        let f = self.acc / STEP_MS;
        self.prev + (self.x - self.prev) * f
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// IslandEvent.qml's hudBar, in pixels of a 280 px bar
    fn hud(x: f64) -> Spring {
        Spring::at(x, 5.0, 0.4, 0.5)
    }

    fn settle(s: &mut Spring, frame_ms: f64) -> f64 {
        let mut t = 0.0;
        while s.moving() && t < 5000.0 {
            s.advance(frame_ms);
            t += frame_ms;
        }
        t
    }

    #[test]
    fn a_key_step_settles_as_qt_does() {
        // (a 5% step of a 280 px bar, 14 px: Qt stops it at 256 ms)
        let mut s = hud(100.0);
        s.retarget(114.0);
        let t = settle(&mut s, STEP_MS);
        assert!((t - 256.0).abs() <= STEP_MS, "{t}");
        assert_eq!(s.value(), 114.0);
    }

    #[test]
    fn sixty_and_one_twenty_hz_follow_the_same_curve() {
        let (mut a, mut b) = (hud(0.0), hud(0.0));
        a.retarget(50.0);
        b.retarget(50.0);
        for _ in 0..20 {
            a.advance(1000.0 / 60.0);
            b.advance(1000.0 / 120.0);
            b.advance(1000.0 / 120.0);
            assert!((a.value() - b.value()).abs() < 1e-9);
        }
    }

    #[test]
    fn frames_between_steps_move_on_smoothly() {
        let mut s = hud(0.0);
        s.retarget(100.0);
        let mut last = s.value();
        // 120 Hz: every frame is a little further, none goes back or jumps
        for _ in 0..12 {
            s.advance(1000.0 / 120.0);
            let v = s.value();
            assert!(v >= last, "{v} < {last}");
            assert!(v - last < 25.0, "a jump of {}", v - last);
            last = v;
        }
    }

    #[test]
    fn a_new_target_keeps_the_speed() {
        // a held key: a step every 40 ms; the bar never slows to a stop
        // between them (the Slint animation began each step from rest)
        let mut s = hud(280.0);
        let mut target = 280.0;
        let mut speeds = Vec::new();
        let mut last = s.value();
        for frame in 0..60 {
            if frame % 5 == 0 && frame < 40 {
                target -= 14.0;
                s.retarget(target);
            }
            s.advance(8.0);
            let v = s.value();
            if (10..40).contains(&frame) {
                speeds.push(last - v);
            }
            last = v;
        }
        // (while the key is held it keeps moving down, every frame)
        assert!(speeds.iter().all(|&d| d > 0.2), "{speeds:?}");
    }

    #[test]
    fn jump_and_rest() {
        let mut s = hud(10.0);
        assert!(!s.moving());
        s.retarget(10.0);
        assert!(!s.moving());
        s.retarget(20.0);
        s.jump(30.0);
        assert!(!s.moving());
        assert_eq!(s.value(), 30.0);
        // (a huge gap between frames: at most a second of steps, then rest)
        s.retarget(200.0);
        s.advance(60_000.0);
        assert!(!s.moving());
        assert_eq!(s.value(), 200.0);
    }
}

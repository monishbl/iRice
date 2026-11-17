//! Frame pacing: the display's refresh, how frames really reached the screen,
//! ticks for animations driven from Rust, and the idle loop's timer grid.
//!
//! - **Refresh**: the outputs' modes say it (wl_output, 60/120/144 Hz), and
//!   wp_presentation's feedback confirms it per frame. `refresh()` is the
//!   fastest output's period: what an animation steps by, never a fixed 16 ms.
//! - **Presented frames** (wp_presentation, wayland.rs): a frame drawn as soon
//!   as the surface was allowed to (its frame callback had just come, the
//!   drawing took under half a frame) is part of a running animation; the time
//!   between two of those on screen is a frame interval, and an interval of
//!   two refreshes is one missed frame. The wake log (TESSERA_WAKES) prints
//!   them each second; SIGUSR1 prints the totals since the last one.
//! - **Frame timers** (`FrameTimer`): what a Rust part animates by hand (a
//!   glow, a ring) ticks once per frame, just before the surfaces draw, at
//!   the display's rate: the loop runs them when a frame callback came, or
//!   by the refresh when no surface is drawing. Never a 16 ms slint::Timer,
//!   which ran at 60 Hz on a 120 Hz screen and out of step with its frames.
//! - **Idle**: with no input for a while and nothing animating, Slint's timers
//!   (clocks, polls) wake the loop together, on a 250 ms grid, not each on
//!   its own (`coalesce`).
//! - **Slow GPU**: on the low-power GPU, more than 10% of animation frames
//!   missed over 30 s of animation asks once whether to use the other GPU
//!   (gpu.rs `slow`).

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

/// 60 Hz, until an output says otherwise
const DEFAULT_REFRESH_NS: u64 = 16_666_667;

/// Idle: the timers' grid
const GRID_MS: u64 = 250;
/// …while battery saver is on (saver.rs): half as many idle wakeups
const SAVER_GRID_MS: u64 = 500;
/// Idle: no input for this long…
const QUIET_INPUT: Duration = Duration::from_secs(2);
/// …and no animation frame for this long
const QUIET_FRAMES: Duration = Duration::from_millis(500);

/// The slow-GPU watch: frames of the first seconds after start (everything
/// loading at once) don't count
const SLOW_SKIP: Duration = Duration::from_secs(20);
/// …over this much animation (summed frame intervals)…
const SLOW_WINDOW_NS: u64 = 30_000_000_000;
/// …more than this share missed
const SLOW_SHARE: f64 = 0.10;

/// The clock Slint's timers and animations run on (wayland.rs Platform's
/// duration_since_start): the idle grid is laid on it
pub fn epoch() -> Instant {
    static E: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    *E.get_or_init(Instant::now)
}

thread_local! {
    /// the outputs' refresh periods (ns), by output name
    static OUTPUTS: RefCell<Vec<(String, u64)>> = const { RefCell::new(Vec::new()) };
    static STATS: RefCell<Stats> = RefCell::new(Stats::default());
    static TOTAL: RefCell<Stats> = RefCell::new(Stats::default());
    static SLOW: RefCell<Slow> = RefCell::new(Slow::default());
    static LAST_INPUT: Cell<Option<Instant>> = const { Cell::new(None) };
    static LAST_ANIM: Cell<Option<Instant>> = const { Cell::new(None) };
    static TIMERS: RefCell<Timers> = RefCell::new(Timers::default());
}

/// An output's mode (wl_output, mHz); 0 or less: unknown, forgotten
pub fn set_output_refresh(name: &str, mhz: i32) {
    OUTPUTS.with(|o| {
        let mut o = o.borrow_mut();
        o.retain(|(n, _)| n != name);
        if mhz > 0 {
            o.push((name.to_string(), 1_000_000_000_000 / mhz as u64));
        }
    });
}

/// One output's refresh period (the fastest's when not known)
pub fn refresh_of(output: &str) -> Duration {
    OUTPUTS.with(|o| o.borrow().iter().find(|(n, _)| n == output).map(|(_, p)| Duration::from_nanos(*p))).unwrap_or_else(refresh)
}

/// The fastest output's refresh period: what animations step by
pub fn refresh() -> Duration {
    Duration::from_nanos(OUTPUTS.with(|o| o.borrow().iter().map(|(_, p)| *p).min()).unwrap_or(DEFAULT_REFRESH_NS))
}

/// What animations driven from here step by: the refresh, but never faster
/// than battery saver's 60 Hz while it is on (saver.rs)
pub fn frame_period() -> Duration {
    let r = refresh();
    crate::saver::min_frame().map_or(r, |m| r.max(m))
}

/// Input came (pointer, keys, touch) or a window was shown (a panel opened
/// by a shortcut is no input to us): the timers' grid is off for a while
pub fn input() {
    LAST_INPUT.with(|l| l.set(Some(Instant::now())));
}

/// Nothing is moving: no input for a while, no animation frame lately
pub fn quiet(now: Instant) -> bool {
    let long_ago = |c: &'static std::thread::LocalKey<Cell<Option<Instant>>>, d: Duration| c.with(|l| l.get().is_none_or(|t| now.duration_since(t) >= d));
    long_ago(&LAST_INPUT, QUIET_INPUT) && long_ago(&LAST_ANIM, QUIET_FRAMES)
}

/// A Slint timer due in `d` (ms, as Slint counts), moved onto the idle grid:
/// every timer due within the same 250 ms wakes the loop once. A Repeated
/// timer re-arms from when it fired, so a 1 s clock stays on the grid.
pub fn coalesce(d: Duration) -> Duration {
    let now_ms = epoch().elapsed().as_millis() as u64;
    let target = now_ms + d.as_millis() as u64;
    let step = if crate::saver::on() { SAVER_GRID_MS } else { GRID_MS };
    Duration::from_millis(grid_of(target, step) - now_ms)
}

/// The grid point at or after `target` (ms): within 2 ms after a grid point
/// counts as on it (Slint's ms and ours can differ by one), never earlier
/// than the target itself
#[cfg(test)]
fn grid(target: u64) -> u64 {
    grid_of(target, GRID_MS)
}

fn grid_of(target: u64, step: u64) -> u64 {
    let g = target.saturating_sub(2).div_ceil(step) * step;
    g.max(target)
}

// ── Presented frames ────────────────────────────────────────────────────────

#[derive(Default)]
struct Stats {
    /// intervals between animation frames on screen, µs
    intervals_us: Vec<u32>,
    missed: u32,
    discarded: u32,
    /// the refresh the compositor reported, ns (the last one)
    refresh_ns: u64,
}

impl Stats {
    fn add(&mut self, interval_ns: u64, refresh_ns: u64, missed: u32) {
        // (SIGUSR1's totals wait for a signal that may never come: at most
        // ~4 minutes of 120 Hz frames kept, then the oldest go)
        if self.intervals_us.len() >= 30_000 {
            self.intervals_us.drain(..10_000);
        }
        self.intervals_us.push((interval_ns / 1000).min(u32::MAX as u64) as u32);
        self.missed += missed;
        self.refresh_ns = refresh_ns;
    }
    fn summary(&mut self) -> Option<String> {
        if self.intervals_us.is_empty() && self.discarded == 0 {
            return None;
        }
        let v = &mut self.intervals_us;
        v.sort_unstable();
        let p = |q: f64| if v.is_empty() { 0.0 } else { v[((v.len() - 1) as f64 * q).round() as usize] as f64 / 1000.0 };
        let hz = if self.refresh_ns > 0 { 1e9 / self.refresh_ns as f64 } else { 0.0 };
        Some(format!(
            "{{\"frames\":{},\"p50_ms\":{:.2},\"p95_ms\":{:.2},\"p99_ms\":{:.2},\"max_ms\":{:.2},\"missed\":{},\"discarded\":{},\"hz\":{hz:.1}}}",
            v.len(),
            p(0.5),
            p(0.95),
            p(0.99),
            p(1.0),
            self.missed,
            self.discarded
        ))
    }
}

/// How many refreshes a frame interval missed (0: on time)
pub fn missed_in(interval_ns: u64, refresh_ns: u64) -> u32 {
    if refresh_ns == 0 {
        return 0;
    }
    // (round: a frame shown a little late by the clock's jitter is on time)
    let k = (interval_ns + refresh_ns / 2) / refresh_ns;
    k.saturating_sub(1).min(u32::MAX as u64) as u32
}

/// wp_presentation said an animation frame reached the screen `interval_ns`
/// after the one before it on the same surface (`gpu_bound`: drawing it
/// took the processor little, so if it was late the GPU was slow)
pub fn presented(interval_ns: u64, refresh_ns: u64, gpu_bound: bool) {
    let refresh_ns = if refresh_ns > 0 { refresh_ns } else { refresh().as_nanos() as u64 };
    // (battery saver's 60 Hz on a faster display: every other refresh is the
    // pace asked for, not a missed frame)
    let refresh_ns = crate::saver::min_frame().map_or(refresh_ns, |m| refresh_ns.max(m.as_nanos() as u64 * 15 / 16));
    // (an interval of many refreshes: the animation paused, not missed frames)
    if interval_ns > refresh_ns * 8 {
        return;
    }
    let missed = missed_in(interval_ns, refresh_ns);
    LAST_ANIM.with(|l| l.set(Some(Instant::now())));
    STATS.with(|s| s.borrow_mut().add(interval_ns, refresh_ns, missed));
    TOTAL.with(|s| s.borrow_mut().add(interval_ns, refresh_ns, missed));
    if gpu_bound {
        SLOW.with(|s| s.borrow_mut().frame(interval_ns, missed));
    }
}

/// An animation frame never reached the screen (a newer one replaced it)
pub fn discarded() {
    STATS.with(|s| s.borrow_mut().discarded += 1);
    TOTAL.with(|s| s.borrow_mut().discarded += 1);
    SLOW.with(|s| s.borrow_mut().frame(0, 1));
}

/// An animation frame was drawn (before it's on screen): the idle grid stays off
pub fn animating() {
    LAST_ANIM.with(|l| l.set(Some(Instant::now())));
}

/// The wake log's: this second's animation frames (taken)
pub fn take_second() -> Option<String> {
    STATS.with(|s| std::mem::take(&mut *s.borrow_mut()).summary())
}

/// SIGUSR1's: the animation frames since the last report (taken)
pub fn report() -> String {
    let s = TOTAL.with(|s| std::mem::take(&mut *s.borrow_mut()).summary());
    format!("{{\"pacing\":{},\"refresh_ms\":{:.3}}}", s.unwrap_or_else(|| "null".into()), refresh().as_secs_f64() * 1000.0)
}

// ── Too slow on this GPU? ───────────────────────────────────────────────────

#[derive(Default)]
struct Slow {
    started: Option<Instant>,
    frames: u64,
    missed: u64,
    anim_ns: u64,
    done: bool,
}

impl Slow {
    fn frame(&mut self, interval_ns: u64, missed: u32) {
        let started = *self.started.get_or_insert_with(Instant::now);
        // (TESSERA_GPU_SLOW_TEST=1: asks at the first animation frames after 2 s, to try the question)
        let test = crate::env::is_set("GPU_SLOW_TEST");
        let (skip, window) = if test { (Duration::from_secs(2), 50_000_000) } else { (SLOW_SKIP, SLOW_WINDOW_NS) };
        if self.done || started.elapsed() < skip || !crate::gpu::watching() {
            return;
        }
        self.frames += 1;
        self.missed += missed as u64;
        self.anim_ns += interval_ns;
        if self.anim_ns < window {
            return;
        }
        let share = self.missed as f64 / (self.frames + self.missed) as f64;
        eprintln!("tessera-ui: frames over {:.0} s of animation: {:.1}% missed", self.anim_ns as f64 / 1e9, share * 100.0);
        if share > SLOW_SHARE || test {
            self.done = true;
            crate::gpu::slow(share);
        }
        (self.frames, self.missed, self.anim_ns) = (0, 0, 0);
    }
}

// ── Frame timers ────────────────────────────────────────────────────────────

type Tick = Rc<RefCell<Box<dyn FnMut() -> bool>>>;

#[derive(Default)]
struct Timers {
    next_id: u64,
    list: Vec<(u64, Tick)>,
    last: Option<Instant>,
}

/// A callback run once per frame, at the display's refresh, just before the
/// surfaces draw (in place of a Repeated slint::Timer of 16 ms): for what a
/// part animates by hand. Stops when dropped.
#[derive(Default)]
pub struct FrameTimer {
    id: Cell<Option<u64>>,
}

impl FrameTimer {
    /// Runs `f` every frame from the next one on (replacing what ran before)
    pub fn start(&self, mut f: impl FnMut() + 'static) {
        self.start_while(move || {
            f();
            true
        });
    }
    /// Runs `f` every frame until it returns false (an animation that ends
    /// by itself: no more ticks once it's over)
    pub fn start_while(&self, f: impl FnMut() -> bool + 'static) {
        self.stop();
        let id = TIMERS.with(|t| {
            let mut t = t.borrow_mut();
            t.next_id += 1;
            let id = t.next_id;
            t.list.push((id, Rc::new(RefCell::new(Box::new(f)))));
            id
        });
        self.id.set(Some(id));
    }
    pub fn stop(&self) {
        if let Some(id) = self.id.take() {
            TIMERS.with(|t| t.borrow_mut().list.retain(|(i, _)| *i != id));
        }
    }
    pub fn running(&self) -> bool {
        self.id.get().is_some_and(|id| TIMERS.with(|t| t.borrow().list.iter().any(|(i, _)| *i == id)))
    }
}

impl Drop for FrameTimer {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Frame timers wait to run
pub fn frame_timers() -> bool {
    TIMERS.with(|t| !t.borrow().list.is_empty())
}

/// When the frame timers run next if no frame callback comes first (the
/// loop's deadline): a refresh after they last ran, or, while a frame
/// callback is awaited (`frame_coming`), two: that callback ticks them, in
/// step with the frames, and this is only for when it doesn't come
pub fn frame_timer_deadline(frame_coming: bool) -> Option<Instant> {
    TIMERS.with(|t| {
        let t = t.borrow();
        (!t.list.is_empty()).then(|| t.last.map_or_else(Instant::now, |l| l + frame_period() * if frame_coming { 2 } else { 1 }))
    })
}

/// Run the frame timers if a frame's worth of time has passed since they
/// last ran (most of one: a frame callback a little early still ticks).
/// Called each turn of the loop before the surfaces draw.
pub fn run_frame_timers() {
    let now = Instant::now();
    let due: Vec<(u64, Tick)> = TIMERS.with(|t| {
        let mut t = t.borrow_mut();
        if t.list.is_empty() || t.last.is_some_and(|l| now.duration_since(l) < frame_period() * 3 / 4) {
            return Vec::new();
        }
        t.last = Some(now);
        t.list.clone()
    });
    for (id, f) in due {
        // (a callback that stops or starts timers changes the list, not this copy;
        // one that's running already, re-entered, is skipped)
        let more = f.try_borrow_mut().map_or(true, |mut f| f());
        if !more {
            TIMERS.with(|t| t.borrow_mut().list.retain(|(i, _)| *i != id));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missed_frames_by_interval() {
        let r = 8_333_333; // 120 Hz
        assert_eq!(missed_in(8_300_000, r), 0);
        assert_eq!(missed_in(9_900_000, r), 0); // (a little late by jitter: on time)
        assert_eq!(missed_in(16_600_000, r), 1);
        assert_eq!(missed_in(25_000_000, r), 2);
        assert_eq!(missed_in(5_000_000, r), 0);
        assert_eq!(missed_in(5_000_000, 0), 0);
    }

    #[test]
    fn idle_timers_fall_on_the_grid() {
        assert_eq!(grid(1000), 1000);
        assert_eq!(grid(1001), 1001); // (just after a grid point: kept, not a whole step later)
        assert_eq!(grid(1002), 1002);
        assert_eq!(grid(1003), 1250);
        assert_eq!(grid(1249), 1250);
        assert_eq!(grid(0), 0);
        // a 1 s timer fired on the grid re-arms on it
        assert_eq!(grid(5250 + 1000), 6250);
        // a 400 ms one moves to the next point
        assert_eq!(grid(5250 + 400), 5750);
        // battery saver's coarser grid
        assert_eq!(grid_of(5250 + 400, SAVER_GRID_MS), 6000);
        assert_eq!(grid_of(6000 + 1000, SAVER_GRID_MS), 7000);
    }

    #[test]
    fn refresh_follows_the_fastest_output() {
        set_output_refresh("A", 60_000);
        assert_eq!(refresh().as_micros(), 16_666);
        set_output_refresh("B", 119_999);
        assert_eq!(refresh().as_micros(), 8_333);
        assert_eq!(refresh_of("A").as_micros(), 16_666);
        set_output_refresh("B", 0);
        assert_eq!(refresh().as_micros(), 16_666);
        set_output_refresh("A", 0);
        assert_eq!(refresh().as_nanos() as u64, DEFAULT_REFRESH_NS);
    }

    #[test]
    fn frame_timers_tick_once_per_frame_and_stop() {
        let n = Rc::new(Cell::new(0));
        let t = FrameTimer::default();
        let m = n.clone();
        t.start(move || m.set(m.get() + 1));
        assert!(t.running() && frame_timers());
        run_frame_timers();
        run_frame_timers(); // (same frame: not again)
        assert_eq!(n.get(), 1);
        drop(t);
        assert!(!frame_timers());
        // (one that ends by itself)
        let t = FrameTimer::default();
        let m = n.clone();
        t.start_while(move || {
            m.set(m.get() + 1);
            false
        });
        TIMERS.with(|t| t.borrow_mut().last = None);
        run_frame_timers();
        assert_eq!(n.get(), 2);
        assert!(!t.running() && !frame_timers());
        n.set(1);
        TIMERS.with(|t| t.borrow_mut().last = None);
        run_frame_timers();
        assert_eq!(n.get(), 1);
    }

    #[test]
    fn stats_summarise_intervals() {
        let mut s = Stats::default();
        for _ in 0..98 {
            s.add(8_333_000, 8_333_333, 0);
        }
        s.add(16_666_000, 8_333_333, 1);
        s.add(25_000_000, 8_333_333, 2);
        let line = s.summary().unwrap();
        assert!(line.contains("\"frames\":100"), "{line}");
        assert!(line.contains("\"p50_ms\":8.33"), "{line}");
        assert!(line.contains("\"missed\":3"), "{line}");
        assert!(line.contains("\"hz\":120.0"), "{line}");
        assert!(Stats::default().summary().is_none());
    }
}

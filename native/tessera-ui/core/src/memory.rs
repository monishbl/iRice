//! What the shell keeps of the memory it once used (docs/gate.md, "Memory").
//!
//! A real session's shell held 200-400 MB where it used ~50-70 MB: glibc's
//! malloc and the kernel's transparent huge pages kept what had been freed.
//!
//! - `tune_allocator` (first thing in main, before any thread): at most two
//!   malloc arenas (glibc's default is 8 per core, one per thread here, ~90
//!   threads: each keeps its own freed memory), and a fixed mmap threshold
//!   (big blocks, decoded pictures, go back to the system when freed;
//!   glibc's own threshold grows with every such free, up to 32 MB, and then
//!   keeps them in the heap). An env setting (MALLOC_ARENA_MAX,
//!   MALLOC_MMAP_THRESHOLD_: the gate's knobs) wins.
//! - `IdleTrim`: once the shell has been quiet for 10 s after drawing
//!   something (an animation, a panel), a work-pool thread marks the malloc
//!   heaps "no huge pages" and gives their free pages back (`malloc_trim`).
//!   With huge pages (THP "always", CachyOS's default) a heap's 2 MB pages
//!   stay whole while a few bytes in each are used, and khugepaged collapses
//!   trimmed ranges back into whole huge pages (max_ptes_none): the laptop's
//!   heap was 177 MB, 176 MB of it huge pages. Never on the UI thread; at
//!   most once per quiet spell.
//!
//! THP isn't switched off for the process (prctl PR_SET_THP_DISABLE): that
//! flag is inherited by every app the shell starts.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// glibc's mallopt parameters (malloc.h)
#[cfg(target_env = "gnu")]
const M_MMAP_THRESHOLD: libc::c_int = -3;
#[cfg(target_env = "gnu")]
const M_ARENA_MAX: libc::c_int = -8;

/// Arenas: the UI thread's (glibc's main arena) and one for the rest
pub const ARENAS: i32 = 2;
/// Blocks this big or bigger are their own mappings, unmapped when freed
pub const MMAP_THRESHOLD: i32 = 128 * 1024;

/// Before any thread starts: the arenas and the mmap threshold (each only
/// when the environment doesn't already set it)
pub fn tune_allocator() {
    #[cfg(target_env = "gnu")]
    unsafe {
        if std::env::var_os("MALLOC_ARENA_MAX").is_none() {
            libc::mallopt(M_ARENA_MAX, ARENAS);
        }
        if std::env::var_os("MALLOC_MMAP_THRESHOLD_").is_none() {
            libc::mallopt(M_MMAP_THRESHOLD, MMAP_THRESHOLD);
        }
    }
}

/// A quiet spell this long after the last busy moment: trim
pub const QUIET: Duration = Duration::from_secs(10);
/// Busy: this many frames within a second (an animation, a panel; the bar's
/// clock and the desktop's minute redraw are a frame now and then)
const BUSY_FRAMES: usize = 3;

/// When to give memory back: armed by a busy moment, fired once the shell
/// has been quiet for `QUIET` (the loop's deadline), then not again until
/// the next busy moment
pub struct IdleTrim {
    /// the frames of the last second
    frames: std::collections::VecDeque<Instant>,
    /// the last busy moment, while not yet trimmed after it
    armed: Option<Instant>,
}

impl Default for IdleTrim {
    fn default() -> Self {
        Self::new(Instant::now())
    }
}

impl IdleTrim {
    /// Armed from the start: starting up is the busiest moment of all
    pub fn new(now: Instant) -> Self {
        Self { frames: Default::default(), armed: Some(now) }
    }

    /// After each turn of the loop: whether anything drew
    pub fn turn(&mut self, now: Instant, drew: bool) {
        if !drew {
            return;
        }
        while self.frames.front().is_some_and(|t| now.duration_since(*t) >= Duration::from_secs(1)) {
            self.frames.pop_front();
        }
        self.frames.push_back(now);
        if self.frames.len() >= BUSY_FRAMES {
            self.armed = Some(now);
        }
    }

    /// When the loop should wake to trim (None: nothing to do)
    pub fn deadline(&self) -> Option<Instant> {
        self.armed.map(|t| t + QUIET)
    }

    /// Whether to trim now (then disarmed until the next busy moment)
    pub fn due(&mut self, now: Instant) -> bool {
        if self.deadline().is_some_and(|d| now >= d) {
            self.armed = None;
            return true;
        }
        false
    }
}

static TRIMMING: AtomicBool = AtomicBool::new(false);

/// Give freed memory back, on the work pool (one at a time). Logs one line:
/// `memory: trim … ms, RssAnon a → b MB, malloc in use u of h MB`.
pub fn trim_in_background() {
    if TRIMMING.swap(true, Ordering::SeqCst) {
        return;
    }
    crate::work::spawn(|| {
        // (pages of ours in swap: walking the free chunks would read them
        // back in while holding malloc's locks, 56-99 ms under pressure on
        // server1; what's in swap costs no memory now anyway)
        let swap = status_field_kb("VmSwap");
        if swap > 1024 {
            eprintln!("memory: trim skipped, {:.1} MB in swap", swap as f64 / 1024.0);
            TRIMMING.store(false, Ordering::SeqCst);
            return;
        }
        let before = rss_anon_kb();
        let t = Instant::now();
        let advised = no_huge_pages();
        let t_advise = t.elapsed();
        trim_now();
        let took = t.elapsed();
        let after = rss_anon_kb();
        let (used, held) = malloc_use();
        eprintln!(
            "memory: trim {:.1} ms (no-huge-pages {:.1} ms, {advised} ranges), RssAnon {:.1} → {:.1} MB, malloc in use {:.1} of {:.1} MB",
            took.as_secs_f64() * 1000.0,
            t_advise.as_secs_f64() * 1000.0,
            before as f64 / 1024.0,
            after as f64 / 1024.0,
            used as f64 / 1048576.0,
            held as f64 / 1048576.0
        );
        TRIMMING.store(false, Ordering::SeqCst);
    });
}

/// glibc's malloc_trim(0): every arena's free pages back to the system
pub fn trim_now() {
    #[cfg(target_env = "gnu")]
    unsafe {
        libc::malloc_trim(0);
    }
}

/// malloc's bytes in use and bytes it holds (heap, arenas, mmapped blocks)
pub fn malloc_use() -> (usize, usize) {
    #[cfg(target_env = "gnu")]
    unsafe {
        let m = libc::mallinfo2();
        return (m.uordblks + m.hblkhd, m.arena + m.hblkhd);
    }
    #[allow(unreachable_code)]
    (0, 0)
}

/// RssAnon of this process (kB; 0 if unreadable)
pub fn rss_anon_kb() -> u64 {
    status_field_kb("RssAnon")
}

fn status_field_kb(key: &str) -> u64 {
    std::fs::read_to_string("/proc/self/status").ok().and_then(|s| status_kb(&s, key)).unwrap_or(0)
}

fn status_kb(status: &str, key: &str) -> Option<u64> {
    status.lines().find_map(|l| l.strip_prefix(key)?.strip_prefix(':')?.split_whitespace().next()?.parse().ok())
}

/// The malloc heaps in /proc/self/maps: the main arena ([heap]) and glibc's
/// other arenas (anonymous, 64 MiB-aligned, at most 64 MiB with the
/// reserved rest ---p right after)
pub fn malloc_heaps(maps: &str) -> Vec<(usize, usize)> {
    const ARENA: usize = 64 << 20;
    let mut out: Vec<(usize, usize)> = Vec::new();
    for l in maps.lines() {
        let mut f = l.split_whitespace();
        let (Some(range), Some(perms)) = (f.next(), f.next()) else { continue };
        let name = f.nth(3).unwrap_or("");
        let Some((a, b)) = range.split_once('-') else { continue };
        let (Ok(a), Ok(b)) = (usize::from_str_radix(a, 16), usize::from_str_radix(b, 16)) else { continue };
        if name == "[heap]" {
            out.push((a, b));
        } else if name.is_empty() {
            // (an arena's start, or its reserved rest right after it)
            let is_start = a % ARENA == 0 && b - a <= ARENA && perms.starts_with("rw");
            let is_rest = perms.starts_with("---") && out.last().is_some_and(|&(s, e)| e == a && s % ARENA == 0 && b - s <= ARENA);
            if is_rest {
                out.last_mut().unwrap().1 = b;
            } else if is_start {
                out.push((a, b));
            }
        }
    }
    out
}

/// MADV_NOHUGEPAGE on the malloc heaps (the mark stays with the mapping as
/// an arena grows into its reserved rest; a [heap] grown since gets it at
/// the next trim). Only what wasn't marked before: madvise takes the
/// process's mmap lock for writing. Returns how many ranges were marked.
pub fn no_huge_pages() -> usize {
    static DONE: std::sync::Mutex<Vec<(usize, usize)>> = std::sync::Mutex::new(Vec::new());
    let Ok(maps) = std::fs::read_to_string("/proc/self/maps") else { return 0 };
    let Ok(mut done) = DONE.lock() else { return 0 };
    let todo = not_marked(&malloc_heaps(&maps), &done);
    let mut n = 0;
    for &(a, b) in &todo {
        if unsafe { libc::madvise(a as *mut libc::c_void, b - a, libc::MADV_NOHUGEPAGE) } == 0 {
            n += 1;
        }
    }
    *done = malloc_heaps(&maps);
    n
}

/// The parts of `heaps` not inside a range marked before (a heap that grew
/// keeps its start: only the new end)
fn not_marked(heaps: &[(usize, usize)], done: &[(usize, usize)]) -> Vec<(usize, usize)> {
    heaps
        .iter()
        .filter_map(|&(a, b)| {
            let from = done.iter().filter(|&&(s, e)| s <= a && e > a).map(|&(_, e)| e).max().unwrap_or(a);
            (from < b).then_some((from, b))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn armed_at_start_fires_once_after_quiet() {
        let t0 = Instant::now();
        let mut t = IdleTrim::new(t0);
        assert!(!t.due(t0 + Duration::from_secs(9)));
        assert!(t.due(t0 + QUIET));
        assert!(!t.due(t0 + Duration::from_secs(60)));
        assert_eq!(t.deadline(), None);
    }

    #[test]
    fn a_frame_now_and_then_isnt_busy() {
        let t0 = Instant::now();
        let mut t = IdleTrim::new(t0);
        assert!(t.due(t0 + QUIET));
        // (the clock: a frame every 2 s for a minute)
        for s in (12..72).step_by(2) {
            t.turn(t0 + Duration::from_secs(s), true);
        }
        assert_eq!(t.deadline(), None);
    }

    #[test]
    fn an_animation_arms_it_and_quiet_after_it_fires() {
        let t0 = Instant::now();
        let mut t = IdleTrim::new(t0);
        assert!(t.due(t0 + QUIET));
        let busy = t0 + Duration::from_secs(30);
        for i in 0..20 {
            t.turn(busy + Duration::from_millis(16 * i), true);
        }
        let last = busy + Duration::from_millis(16 * 19);
        assert_eq!(t.deadline(), Some(last + QUIET));
        assert!(!t.due(last + Duration::from_secs(5)));
        // (turns that drew nothing change nothing)
        t.turn(last + Duration::from_secs(6), false);
        assert!(t.due(last + QUIET));
        assert!(!t.due(last + QUIET + Duration::from_secs(1)));
    }

    #[test]
    fn heaps_found_in_maps() {
        let maps = "\
55d0a0000000-55d0a1000000 r--p 00000000 103:02 123 /usr/bin/tessera-ui
55d0b0000000-55d0b8000000 rw-p 00000000 00:00 0                          [heap]
7f1400000000-7f1401a00000 rw-p 00000000 00:00 0
7f1401a00000-7f1404000000 ---p 00000000 00:00 0
7f1404000000-7f1404021000 rw-p 00000000 00:00 0
7f1404021000-7f1408000000 ---p 00000000 00:00 0
7f1408100000-7f1408200000 rw-p 00000000 00:00 0
7f140c000000-7f140c800000 rw-p 00000000 00:00 0
7f14b5c00000-7f14b5fbb000 r--p 00000000 103:02 9 /usr/share/fonts/a.ttf
";
        assert_eq!(
            malloc_heaps(maps),
            vec![(0x55d0b0000000, 0x55d0b8000000), (0x7f1400000000, 0x7f1404000000), (0x7f1404000000, 0x7f1408000000), (0x7f140c000000, 0x7f140c800000)]
        );
    }

    #[test]
    fn only_new_ranges_are_marked() {
        let done = vec![(0x1000, 0x5000), (0x40000000, 0x44000000)];
        // (the heap grew from 0x5000 to 0x9000; the arena is as it was; a new arena)
        let heaps = vec![(0x1000, 0x9000), (0x40000000, 0x44000000), (0x48000000, 0x48100000)];
        assert_eq!(not_marked(&heaps, &done), vec![(0x5000, 0x9000), (0x48000000, 0x48100000)]);
        assert_eq!(not_marked(&heaps, &heaps), vec![]);
        assert_eq!(not_marked(&heaps, &[]), heaps);
    }

    #[test]
    fn status_read() {
        assert_eq!(status_kb("VmRSS:\t  352816 kB\nRssAnon:\t  231080 kB\n", "RssAnon"), Some(231080));
        assert_eq!(status_kb("VmRSS:\t 1 kB\n", "RssAnon"), None);
    }

    #[test]
    fn trimming_here_works() {
        // (glibc: the calls themselves; the numbers are the process's)
        trim_now();
        let (used, held) = malloc_use();
        assert!(used <= held || held == 0);
        assert!(no_huge_pages() >= 1 || cfg!(not(target_env = "gnu")));
    }
}

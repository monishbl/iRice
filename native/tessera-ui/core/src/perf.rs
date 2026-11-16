//! Debug measurements for "it feels slow" (TESSERA_DEBUG, or TESSERA_PERF):
//!
//! - **Stalls** (the tripwire): any piece of a loop turn that holds the UI
//!   thread longer than half a frame (`STEP_MS`, 8 ms) is logged with what it
//!   was: the message and the part it went to, the host service and its
//!   step (a Wm tick, event or answer), a pointer press on a surface, Slint's
//!   timers, a surface's render, a part's after_frame.
//!   `debug: stall 42.1 ms: msg host > part host > host Wm > tick feed.send`
//!   Only the piece that holds the time logs: the innermost one over the
//!   tripwire, and an outer one only when its own code beyond that took
//!   long too (`… (own 12.0 ms)`), or when it is the loop's outermost piece
//!   and a stall (over `STALL_MS`, its pieces logged): that line closes the
//!   chain, so a turn spent in several 8-16 ms steps still reads as one
//!   stall of its whole time (tools/gate/timinglog.py).
//!   Also on (without TESSERA_DEBUG) while $XDG_RUNTIME_DIR/tessera-wakes exists.
//! - **Reloads**: a surface that hands a list a new model (every element
//!   made again) or makes a window again says so:
//!   `debug: reload list notifications.cards (12 rows)` / `debug: reload window controlcenter`.
//!   The count (`reloads()`) is printed with SIGUSR1's frame statistics.
//!
//! Off (neither variable set) every call here is one cached bool test.

use std::cell::{Cell, RefCell};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::Instant;

/// TESSERA_DEBUG or TESSERA_PERF is set (read once)
pub fn on() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| crate::env::is_set("DEBUG") || crate::env::is_set("PERF"))
}

/// Longer than this (ms) is a stall: a 60 Hz frame
pub const STALL_MS: f64 = 16.0;

/// The tripwire: a single step of the UI thread longer than this (ms) is
/// logged with its path, half a frame (the UI thread may only handle input,
/// update models and draw; blocking work goes to crate::work)
pub const STEP_MS: f64 = 8.0;

/// One piece of work running now: its name, the time its pieces took, and
/// whether one of them was logged already
struct Step {
    label: String,
    inner_ms: f64,
    inner_logged: bool,
}

thread_local! {
    // what the loop is doing now, outermost first ("msg host", "host Panels"…)
    static WHAT: RefCell<Vec<Step>> = const { RefCell::new(Vec::new()) };
    static DEPTH: Cell<usize> = const { Cell::new(0) };
}

/// Should a step that took `ms` be logged? Only the step that holds the time:
/// the innermost one over the tripwire, and an outer one only for what its
/// own code took beyond its logged pieces (so one slow call is one line, not
/// one per level it ran under)
fn worth_logging(ms: f64, inner_ms: f64, inner_logged: bool, outermost: bool) -> bool {
    ms > STEP_MS && (!inner_logged || ms - inner_ms > STEP_MS || (outermost && ms > STALL_MS))
}

/// Run `f` as the named piece of work; logged when it holds the UI thread
/// longer than the tripwire (`STEP_MS`)
#[inline]
pub fn time<R>(what: impl FnOnce() -> String, f: impl FnOnce() -> R) -> R {
    let acc = crate::wayland::wakes_on();
    if !on() && !acc {
        return f();
    }
    WHAT.with(|w| w.borrow_mut().push(Step { label: what(), inner_ms: 0.0, inner_logged: false }));
    DEPTH.with(|d| d.set(d.get() + 1));
    let t = Instant::now();
    let r = f();
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    DEPTH.with(|d| d.set(d.get() - 1));
    let path = WHAT.with(|w| {
        let mut w = w.borrow_mut();
        let Some(top) = w.last() else { return None };
        let log = worth_logging(ms, top.inner_ms, top.inner_logged, w.len() == 1);
        let own = ms - top.inner_ms;
        let p = log.then(|| {
            let p = w.iter().map(|s| s.label.as_str()).collect::<Vec<_>>().join(" > ");
            if top.inner_logged {
                format!("{p} (own {own:.1} ms)")
            } else {
                p
            }
        });
        let step = w.pop()?;
        if let Some(parent) = w.last_mut() {
            parent.inner_ms += ms;
            parent.inner_logged |= log || step.inner_logged;
        }
        if acc {
            SPENT.with(|m| *m.borrow_mut().entry(step.label).or_default() += ms);
        }
        p
    });
    if let Some(p) = path {
        STALLS.fetch_add(1, Ordering::Relaxed);
        eprintln!("debug: stall {ms:.1} ms: {p}");
    }
    r
}

thread_local! {
    /// The wake log's (TESSERA_WAKES): ms spent this second, by what
    pub static SPENT: RefCell<std::collections::HashMap<String, f64>> = RefCell::new(Default::default());
}

/// The wake log's: the costliest things this second (taken)
pub fn spent_top(n: usize) -> Vec<(String, f64)> {
    let mut v: Vec<(String, f64)> = SPENT.with(|m| std::mem::take(&mut *m.borrow_mut())).into_iter().collect();
    v.sort_by(|a, b| b.1.total_cmp(&a.1));
    v.truncate(n);
    v.iter_mut().for_each(|x| x.1 = (x.1 * 10.0).round() / 10.0);
    v
}

static STALLS: AtomicU64 = AtomicU64::new(0);
static RELOADS: AtomicU64 = AtomicU64::new(0);

/// A whole list handed a new model: every row's element made again
pub fn reload_list(surface: &str, list: &str, rows: usize) {
    if on() {
        RELOADS.fetch_add(1, Ordering::Relaxed);
        eprintln!("debug: reload list {surface}.{list} ({rows} rows)");
    }
}

/// A window (component and surface) made again where one existed before
pub fn reload_window(surface: &str) {
    if on() {
        RELOADS.fetch_add(1, Ordering::Relaxed);
        eprintln!("debug: reload window {surface}");
    }
}

/// How many reloads and stalls so far
pub fn counts() -> (u64, u64) {
    (RELOADS.load(Ordering::Relaxed), STALLS.load(Ordering::Relaxed))
}

/// A fresh model for a list that is built once (a window being made), or,
/// counted as a reload, one that replaces a list already shown
pub fn fresh<T: Clone + 'static>(surface: &str, list: &str, v: Vec<T>, replacing: bool) -> slint::ModelRc<T> {
    if replacing {
        reload_list(surface, list, v.len());
    }
    slint::ModelRc::new(slint::VecModel::from(v))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_returns_the_value() {
        assert_eq!(time(|| "x".into(), || 3), 3);
    }

    #[test]
    fn only_the_step_that_holds_the_time_is_logged() {
        // quick: never
        assert!(!worth_logging(5.0, 0.0, false, false));
        // slow, nothing inside it logged: it is the culprit
        assert!(worth_logging(30.0, 2.0, false, false));
        // slow because a piece inside was (logged already): not again
        assert!(!worth_logging(31.0, 30.0, true, false));
        // slow beyond its logged piece: its own code is slow too
        assert!(worth_logging(60.0, 30.0, true, false));
        // the loop's outermost piece over a frame closes the chain (the stall's whole time)
        assert!(worth_logging(20.0, 19.5, true, true));
        // …but not under a frame
        assert!(!worth_logging(15.0, 14.5, true, true));
    }
}

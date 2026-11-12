//! Debug measurements for "it feels slow" (TESSERA_DEBUG, or TESSERA_PERF):
//!
//! - **Stalls**: any piece of a loop turn that holds the UI thread longer
//!   than a frame (16 ms) is logged with what it was: the message and the
//!   part it went to, the host service and its call, a pointer press on a
//!   surface, Slint's timers, a surface's render, a part's after_frame.
//!   `debug: stall 42.1 ms: msg host ipc panels toggle controlCenter > host Panels`
//!   (nested pieces each log, innermost first, so the chain reads outwards).
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

thread_local! {
    // what the loop is doing now, outermost first ("msg host", "host Panels"…)
    static WHAT: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static DEPTH: Cell<usize> = const { Cell::new(0) };
}

/// Run `f` as the named piece of work; logged when it takes longer than a frame
#[inline]
pub fn time<R>(what: impl FnOnce() -> String, f: impl FnOnce() -> R) -> R {
    if !on() {
        return f();
    }
    WHAT.with(|w| w.borrow_mut().push(what()));
    DEPTH.with(|d| d.set(d.get() + 1));
    let t = Instant::now();
    let r = f();
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    DEPTH.with(|d| d.set(d.get() - 1));
    let path = WHAT.with(|w| {
        let mut w = w.borrow_mut();
        let p = if ms > STALL_MS { Some(w.join(" > ")) } else { None };
        w.pop();
        p
    });
    if let Some(p) = path {
        STALLS.fetch_add(1, Ordering::Relaxed);
        eprintln!("debug: stall {ms:.1} ms: {p}");
    }
    r
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
}

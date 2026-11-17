//! Blocking work kept off the UI thread (the loop may only handle input,
//! update models and draw). Two kinds of long-lived workers, never a thread
//! per event:
//!
//! - `spawn`: a small fixed pool for jobs that don't depend on each other
//!   (a Hyprland snapshot, a plugin check, an icon lookup). A job's answer
//!   goes back the way the codebase already does it: a `Msg` on the loop's
//!   calloop channel, or a host service's own sender.
//! - `Lane`: one thread that runs its jobs in the order they came (Hyprland
//!   dispatches, where a move followed by a focus must stay in that order;
//!   the settings and state files, where the last write must win).
//!
//! `on_ui_thread()` says whether the caller is the loop (marked at start):
//! the settings' and state's writers go to their lane from there and stay
//! as they were elsewhere.

use std::cell::Cell;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

type Job = Box<dyn FnOnce() + Send>;

thread_local! {
    static UI: Cell<bool> = const { Cell::new(false) };
}

/// This thread is the UI thread (the binary's loop; once, at start)
pub fn mark_ui_thread() {
    UI.with(|u| u.set(true));
}

/// Whether the caller runs on the UI thread
pub fn on_ui_thread() -> bool {
    UI.with(|u| u.get())
}

static LEAVING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The loop has ended and the parts are quitting: what they do on the way
/// out may wait (their glass taken off before the process goes)
pub fn set_leaving() {
    LEAVING.store(true, std::sync::atomic::Ordering::SeqCst);
}

pub fn leaving() -> bool {
    LEAVING.load(std::sync::atomic::Ordering::SeqCst)
}

/// How many pool threads: jobs block on sockets and processes, not CPU
const POOL: usize = 4;

/// Run `f` on the pool (it may block; its answer goes back by message)
pub fn spawn(f: impl FnOnce() + Send + 'static) {
    static TX: OnceLock<Mutex<Sender<Job>>> = OnceLock::new();
    let tx = TX.get_or_init(|| {
        let (tx, rx) = channel::<Job>();
        let rx = Arc::new(Mutex::new(rx));
        for i in 0..POOL {
            let rx = rx.clone();
            let _ = std::thread::Builder::new().name(format!("tessera-work{i}")).spawn(move || loop {
                // (the lock is held only while one job is taken)
                let job = match rx.lock() {
                    Ok(r) => r.recv(),
                    Err(_) => return,
                };
                match job {
                    Ok(j) => run_caught(j),
                    Err(_) => return,
                }
            });
        }
        Mutex::new(tx)
    });
    if let Ok(t) = tx.lock() {
        let _ = t.send(Box::new(f));
    }
}

/// A job that panics costs that job, not the worker
fn run_caught(j: Job) {
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(j)).is_err() {
        eprintln!("tessera-ui: a background job failed");
    }
}

/// One thread running its jobs in order (made on first use)
pub struct Lane {
    name: &'static str,
    tx: OnceLock<Mutex<Sender<Job>>>,
}

impl Lane {
    pub const fn new(name: &'static str) -> Self {
        Self { name, tx: OnceLock::new() }
    }

    fn sender(&self) -> &Mutex<Sender<Job>> {
        self.tx.get_or_init(|| {
            let (tx, rx): (Sender<Job>, Receiver<Job>) = channel();
            let _ = std::thread::Builder::new().name(format!("tessera-{}", self.name)).spawn(move || {
                for j in rx {
                    run_caught(j);
                }
            });
            Mutex::new(tx)
        })
    }

    /// Run `f` after everything queued on this lane before it
    pub fn run(&self, f: impl FnOnce() + Send + 'static) {
        if let Ok(t) = self.sender().lock() {
            let _ = t.send(Box::new(f));
        }
    }

    /// Wait (at most `max`) until what's queued now has run: on the way out,
    /// so a setting written a moment ago isn't lost
    pub fn flush(&self, max: Duration) {
        if self.tx.get().is_none() {
            return;
        }
        let (tx, rx) = channel::<()>();
        self.run(move || {
            let _ = tx.send(());
        });
        let _ = rx.recv_timeout(max);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lane_keeps_order() {
        static L: Lane = Lane::new("test");
        let seen = Arc::new(Mutex::new(Vec::new()));
        for i in 0..50 {
            let s = seen.clone();
            L.run(move || s.lock().unwrap().push(i));
        }
        L.flush(Duration::from_secs(5));
        assert_eq!(*seen.lock().unwrap(), (0..50).collect::<Vec<_>>());
    }

    #[test]
    fn the_pool_answers() {
        let (tx, rx) = channel();
        for i in 0..10 {
            let tx = tx.clone();
            spawn(move || tx.send(i).unwrap());
        }
        let mut got: Vec<i32> = (0..10).map(|_| rx.recv_timeout(Duration::from_secs(5)).unwrap()).collect();
        got.sort();
        assert_eq!(got, (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn a_panicking_job_keeps_the_lane() {
        static L: Lane = Lane::new("test2");
        L.run(|| panic!("boom"));
        let (tx, rx) = channel();
        L.run(move || tx.send(1).unwrap());
        assert_eq!(rx.recv_timeout(Duration::from_secs(5)).unwrap(), 1);
    }

    #[test]
    fn only_the_marked_thread_is_the_ui() {
        assert!(!on_ui_thread());
        std::thread::spawn(|| {
            mark_ui_thread();
            assert!(on_ui_thread());
        })
        .join()
        .unwrap();
        assert!(!on_ui_thread());
    }
}

//! One `pactl subscribe` for the whole shell (replaces services/PulseEvents.qml):
//! Privacy's microphone dot and Devices' output alerts get each line through
//! `subscribe`. Started again after it ends (the sound server restarted, or
//! isn't up yet): waiting longer each time it ends quickly (5 s, 10 s… up to
//! 5 min; back to 5 s after a run of a minute or more), as KeptProcess does.
//! (tessera-events' own subscription, which the QML prefers, is host-shell's.)
//!
//! For another service of this crate: `pulse_events::subscribe(Box::new(|line| …))`
//! at start; the callback runs on the pactl thread (forward with Host::sender).
//! State on the bus as "pulseevents": {running}.

use super::common::*;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tessera_core::host::{Host, Service};

type Sub = Box<dyn Fn(&str) + Send>;

fn subs() -> &'static Mutex<Vec<Sub>> {
    static S: OnceLock<Mutex<Vec<Sub>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(Vec::new()))
}

/// PulseEvents.event(line)'s listeners
pub fn subscribe(f: Sub) {
    if let Ok(mut s) = subs().lock() {
        s.push(f);
    }
}

/// KeptProcess's wait before starting again
pub fn rest_ms(first_wait: u64, quick_ends: u32, max_wait: u64) -> u64 {
    (first_wait.saturating_mul(1u64 << quick_ends.min(20))).min(max_wait)
}

#[derive(Default)]
pub struct PulseEvents {
    stop: Arc<AtomicBool>,
    current: Arc<Mutex<Option<Watch>>>,
}

impl Service for PulseEvents {
    fn name(&self) -> &'static str {
        "PulseEvents"
    }
    fn start(&mut self, _h: &mut Host) {
        if !has_tool("pactl") {
            crate::bus::put("pulseevents", json!({"running": false}));
            return;
        }
        let (stop, current) = (self.stop.clone(), self.current.clone());
        std::thread::spawn(move || {
            let mut quick = 0u32;
            while !stop.load(Ordering::SeqCst) {
                let started = Instant::now();
                let (tx, rx) = std::sync::mpsc::channel();
                let w = Watch::start(
                    &["pactl", "subscribe"],
                    None,
                    |line| {
                        if let Ok(s) = subs().lock() {
                            s.iter().for_each(|f| f(&line));
                        }
                    },
                    move |_, _| {
                        let _ = tx.send(());
                    },
                );
                *current.lock().unwrap() = Some(w);
                crate::bus::put("pulseevents", json!({"running": true}));
                let _ = rx.recv();
                crate::bus::put("pulseevents", json!({"running": false}));
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                // (ended, or never started: a missing server)
                if started.elapsed() >= Duration::from_secs(60) {
                    quick = 0;
                }
                let wait = rest_ms(5000, quick, 300_000);
                quick += 1;
                let until = Instant::now() + Duration::from_millis(wait);
                while Instant::now() < until && !stop.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(200));
                }
            }
        });
    }
    fn call(&mut self, _h: &mut Host, _call: &str, _args: &Value) {}
    fn quit(&mut self, _h: &mut Host) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(w) = self.current.lock().unwrap().as_ref() {
            w.stop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_waits_longer_each_quick_end() {
        assert_eq!(rest_ms(5000, 0, 300_000), 5000);
        assert_eq!(rest_ms(5000, 3, 300_000), 40_000);
        assert_eq!(rest_ms(5000, 10, 300_000), 300_000);
    }
}

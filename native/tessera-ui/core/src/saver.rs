//! Battery saver, as the shell sees it: one flag the host's Power service
//! sets (host/system/src/net/power.rs decides when, power_policy::Saver),
//! read everywhere else. While it is on the shell runs lighter:
//!
//! - **Frames**: no surface presents more often than 60 times a second, and
//!   frame timers and unpaced animations tick at most at 60 Hz, also on a
//!   120 Hz display (wayland.rs `Surface::render`, pacing.rs `frame_period`).
//! - **Motion**: the settings every part reads are shown with
//!   `appearance.reduceMotion` on (`overlay`), so every Reduce motion path
//!   (lock screen loops, the fingerprint, wavy progress, the island's rings,
//!   bounces, slides) is taken; the file itself is never changed, and
//!   Settings reads it as it is (`raw`).
//! - **Glass**: Cool-down's frosted glass (no lens), unless Cool-down or the
//!   person already chose something plainer (host/shell appearance.rs).
//! - **Background work**: refreshes that run on a clock wait `FACTOR` times
//!   longer (`stretch`): weather, Google and cloud accounts (calendars, tasks); the
//!   resource graphs sample less often.
//!
//! Only the shell: never Hyprland's settings, never the system's power
//! profile (that is a separate, opt-in setting in Battery & Power).

use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

static ON: AtomicBool = AtomicBool::new(false);

/// How many times longer clock-driven refreshes wait while it is on
pub const FACTOR: u32 = 4;

/// The most frames a second a surface presents while it is on
pub const MAX_HZ: u64 = 60;

/// Is battery saver on (any thread)
pub fn on() -> bool {
    ON.load(Ordering::Relaxed)
}

/// Turn it on or off; true when that changed something (the caller then
/// has the settings read again: Msg::ConfigChanged)
pub fn set(on: bool) -> bool {
    ON.swap(on, Ordering::Relaxed) != on
}

/// A refresh interval, as it is now: `FACTOR` times longer while on
pub fn stretch(d: Duration) -> Duration {
    stretch_if(on(), d)
}

fn stretch_if(on: bool, d: Duration) -> Duration {
    if on {
        d * FACTOR
    } else {
        d
    }
}

/// The same for a count of ticks (a 1 s poll that acts every `n`)
pub fn stretch_ticks(n: u64) -> u64 {
    if on() {
        n * FACTOR as u64
    } else {
        n
    }
}

/// The shortest time between two frames of one surface: None at full speed
pub fn min_frame() -> Option<Duration> {
    min_frame_if(on())
}

fn min_frame_if(on: bool) -> Option<Duration> {
    on.then(|| Duration::from_nanos(1_000_000_000 / MAX_HZ))
}

/// The settings as the parts see them: Reduce motion on while saver is
pub fn overlay(cfg: Value) -> Value {
    overlay_if(on(), cfg)
}

fn overlay_if(on: bool, mut cfg: Value) -> Value {
    if on {
        if !cfg.is_object() {
            cfg = Value::Object(Default::default());
        }
        if !cfg["appearance"].is_object() {
            cfg["appearance"] = Value::Object(Default::default());
        }
        cfg["appearance"]["reduceMotion"] = Value::Bool(true);
    }
    cfg
}

/// The settings as the file has them (for Settings, which shows and
/// changes the person's own choices): `shown` as the parts have it
pub fn raw(shown: &Value) -> Value {
    if on() {
        crate::theme::read_json()
    } else {
        shown.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // (the flag itself is the process's: never set here, other tests run
    // beside these and read it)
    #[test]
    fn on_and_off() {
        assert_eq!(stretch_if(true, Duration::from_secs(60)), Duration::from_secs(240));
        assert_eq!(stretch_if(false, Duration::from_secs(60)), Duration::from_secs(60));
        assert_eq!(min_frame_if(true).map(|d| d.as_micros()), Some(16_666));
        assert_eq!(min_frame_if(false), None);
        let c = overlay_if(true, json!({"appearance": {"look": "glass", "reduceMotion": false}}));
        assert_eq!(c["appearance"]["reduceMotion"], true);
        assert_eq!(c["appearance"]["look"], "glass");
        assert_eq!(overlay_if(true, Value::Null)["appearance"]["reduceMotion"], true);
        assert_eq!(overlay_if(true, json!({"appearance": 3}))["appearance"]["reduceMotion"], true);
        assert_eq!(overlay_if(false, json!({"appearance": {"reduceMotion": false}}))["appearance"]["reduceMotion"], false);
        assert!(!on());
    }
}

//! ScreenTime.qml: how long each app was in front today (and the six days
//! before), for the Screen Time widget. Counts only while you're not idle
//! (2 minutes without input stops the clock), adding time when the front app
//! changes and once a minute. Kept in state.json ("screenTime").
//!
//! Bus "screentime": { today, todayTotal, todayApps [{id, seconds}], week
//! [{label, seconds}], total, apps } (total/apps/week as the desktop widget's
//! feed names them). Differences: the front app is polled every 2 s (the
//! QML reacts to a binding); state.json is read once at start and its
//! "screenTime" key written directly, at most every 30 s (UiState's lazy
//! merge), and on quit; see CORE-NEEDS (a UiState service to write through).

use super::common::*;
use serde_json::{json, Map, Value};
use std::any::Any;
use std::time::Duration;
use tessera_core::host::{Host, Service};

enum M {
    Tick,
}

#[derive(Default)]
pub struct ScreenTime {
    all: Map<String, Value>, // "YYYY-MM-DD" -> { app id: seconds }
    since: i64,
    counting: String,
    app: String,
    was_idle: bool,
    ticks: u32,
    dirty: bool,
    last_write: i64,
    loaded: bool,
    timer: slint::Timer,
}

fn secs(m: &Value) -> i64 {
    m.as_object().map_or(0, |o| o.values().filter_map(Value::as_i64).sum())
}

/// Screen Time's `format(s)`: "2h 5m" / "7m"
pub fn format(s: f64) -> String {
    let m = (s / 60.0).round() as i64;
    if m >= 60 { format!("{}h {}m", m / 60, m % 60) } else { format!("{m}m") }
}

impl ScreenTime {
    fn path() -> std::path::PathBuf {
        tessera_core::util::state_dir().join("state.json")
    }
    fn publish(&self, now: i64) {
        let today = self.all.get(&day_of(now)).cloned().unwrap_or_else(|| json!({}));
        let total = secs(&today);
        let mut apps: Vec<Value> = today.as_object().map(|o| o.iter().map(|(k, v)| json!({"id": k, "seconds": v})).collect()).unwrap_or_default();
        apps.sort_by_key(|a| std::cmp::Reverse(a["seconds"].as_i64().unwrap_or(0)));
        let week: Vec<Value> = (0..7)
            .rev()
            .map(|i| {
                let t = now - i * 86_400_000;
                json!({"label": weekday_letter(t), "seconds": secs(self.all.get(&day_of(t)).unwrap_or(&Value::Null))})
            })
            .collect();
        put("screentime", json!({"today": today, "todayTotal": total, "todayApps": apps, "week": week, "total": total, "apps": apps}));
    }
    fn flush(&mut self) {
        let now = now_ms();
        let s = ((now - self.since) as f64 / 1000.0).round() as i64;
        self.since = now;
        if self.counting.is_empty() || s <= 0 || s > 600 {
            return;
        }
        let d = day_of(now);
        let mut day = self.all.get(&d).and_then(|v| v.as_object().cloned()).unwrap_or_default();
        let e = day.get(&self.counting).and_then(Value::as_i64).unwrap_or(0) + s;
        day.insert(self.counting.clone(), json!(e));
        self.all.insert(d, Value::Object(day));
        // Keep a week
        let keep: Vec<String> = (0..7).map(|i| day_of(now - i * 86_400_000)).collect();
        self.all.retain(|k, _| keep.contains(k));
        self.dirty = true;
    }
    fn restart(&mut self, idle: bool) {
        self.flush();
        self.counting = if idle || !self.loaded { String::new() } else { self.app.clone() };
    }
    fn save(&mut self) {
        if !self.dirty {
            return;
        }
        self.dirty = false;
        self.last_write = now_ms();
        let p = Self::path();
        let mut j = crate::util::read_json(&p);
        if !j.is_object() {
            j = json!({});
        }
        j["screenTime"] = Value::Object(self.all.clone());
        let _ = tessera_core::util::write_atomic(&p, serde_json::to_string(&j).unwrap_or_default().as_bytes());
    }
    /// Apps.activeEntry?.id (Tessera's own windows don't count)
    fn front(h: &Host) -> String {
        let id = crate::bus::field("apps", "activeId").as_str().map(str::to_string).or_else(|| {
            h.st.shell.get::<tessera_core::services::clients::Windows>().and_then(|w| w.active_client().map(|c| c.class.to_lowercase()))
        });
        let id = id.unwrap_or_default();
        if id == "org.quickshell" { String::new() } else { id }
    }
}

impl Service for ScreenTime {
    fn name(&self) -> &'static str {
        "ScreenTime"
    }
    fn start(&mut self, h: &mut Host) {
        h.st.shell.want("clients");
        let j = crate::util::read_json(&Self::path());
        if let Some(o) = j["screenTime"].as_object() {
            self.all = o.clone();
        }
        self.loaded = true;
        self.since = now_ms();
        self.app = Self::front(h);
        self.counting = if idle() { String::new() } else { self.app.clone() };
        self.publish(self.since);
        let tx = Tx::new(h.sender("ScreenTime"));
        self.timer.start(slint::TimerMode::Repeated, Duration::from_secs(2), move || tx.send(M::Tick));
    }
    fn message(&mut self, h: &mut Host, msg: Box<dyn Any + Send>) {
        if msg.downcast::<M>().is_err() {
            return;
        }
        let app = Self::front(h);
        let idle_now = idle();
        if app != self.app {
            self.app = app;
            self.restart(idle_now);
        } else if idle_now != self.was_idle {
            self.restart(idle_now);
        }
        self.was_idle = idle_now;
        self.ticks += 1;
        // once a minute while counting
        if self.ticks % 30 == 0 && !self.counting.is_empty() {
            self.flush();
        }
        let now = now_ms();
        if self.dirty && now - self.last_write >= 30_000 {
            self.save();
        }
        self.publish(now);
    }
    fn quit(&mut self, _h: &mut Host) {
        self.flush();
        self.save();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn minutes_and_hours() {
        assert_eq!(format(0.0), "0m");
        assert_eq!(format(3000.0), "50m");
        assert_eq!(format(7500.0), "2h 5m");
    }
    #[test]
    fn totals_sum_a_days_apps() {
        assert_eq!(secs(&json!({"a": 60, "b": 30})), 90);
        assert_eq!(secs(&Value::Null), 0);
    }
}

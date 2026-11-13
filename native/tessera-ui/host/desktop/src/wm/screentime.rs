//! ScreenTime.qml: how long each app was in front today (and the six days
//! before), for the Screen Time widget. Counts only while you're not idle (2
//! minutes without input stops the clock; a playing video's inhibitor counts
//! as use), adding time when the front app changes and once a minute. Kept
//! in state.json's `screenTime` ({"yyyy-MM-dd": {appId: seconds}}), a week.

use super::hx;
use serde_json::{json, Value};
use tessera_core::host::Host;
use tessera_core::services::apps::Apps as Entries;
use tessera_core::services::clients::Windows;
use tessera_core::util::{now_ms, strftime};

pub struct ScreenTime {
    since: i64,
    counting: String,
}

impl Default for ScreenTime {
    fn default() -> Self {
        Self { since: now_ms(), counting: String::new() }
    }
}

pub fn day(ms: i64) -> String {
    strftime("%Y-%m-%d", ms / 1000)
}

/// Seconds added to `all[day][app]`, days older than a week dropped
pub fn add(all: &Value, today_ms: i64, app: &str, secs: i64) -> Value {
    let mut all = all.as_object().cloned().unwrap_or_default();
    let d = day(today_ms);
    let mut today = all.get(&d).and_then(Value::as_object).cloned().unwrap_or_default();
    let had = today.get(app).and_then(Value::as_i64).unwrap_or(0);
    today.insert(app.to_string(), json!(had + secs));
    all.insert(d, Value::Object(today));
    let keep: Vec<String> = (0..7).map(|i| day(today_ms - i * 86_400_000)).collect();
    all.retain(|k, _| keep.contains(k));
    Value::Object(all)
}

fn total(v: &Value) -> i64 {
    v.as_object().map(|m| m.values().filter_map(Value::as_i64).sum()).unwrap_or(0)
}

impl ScreenTime {
    /// The front app's entry id ("" for none, or Tessera's own windows)
    fn app(h: &Host) -> String {
        let Some(w) = h.st.shell.get::<Windows>() else { return String::new() };
        let Some(c) = w.active_client() else { return String::new() };
        let id = h.st.shell.get::<Entries>().and_then(|a| a.entry_for(&c.class)).map(|e| e.id.clone()).unwrap_or_default();
        if tessera_core::wm::own_window(&id) {
            String::new()
        } else {
            id
        }
    }

    fn flush(&mut self) {
        let now = now_ms();
        let secs = ((now - self.since) as f64 / 1000.0).round() as i64;
        self.since = now;
        if self.counting.is_empty() || secs <= 0 || secs > 600 {
            return;
        }
        hx::state_set("screenTime", add(&hx::state_get("screenTime"), now, &self.counting, secs));
    }

    /// Every couple of seconds: the idle watch asked for, the front app and
    /// idleness followed (restart on a change), and a flush once a minute
    pub fn tick(&mut self, h: &mut Host) {
        use tessera_core::idle;
        h.st.idle_set(idle::SCREEN_TIME, Some((120_000, true)));
        let idle_now = h.st.shell.get::<idle::Idle>().is_some_and(|i| i.is(idle::SCREEN_TIME));
        let want = if idle_now { String::new() } else { Self::app(h) };
        if want != self.counting {
            self.flush();
            self.counting = want;
        } else if !self.counting.is_empty() && now_ms() - self.since >= 60_000 {
            self.flush();
        }
    }

    /// The feed's `screenTime`: today's total, the week's totals (oldest
    /// first, a weekday letter each) and today's top three apps
    pub fn feed(&self, h: &Host) -> Value {
        let all = hx::state_get("screenTime");
        let now = now_ms();
        let today = &all[day(now)];
        let week: Vec<Value> = (0..7).rev().map(|i| {
            let t = now - i * 86_400_000;
            json!({"label": strftime("%a", t / 1000).chars().next().map(String::from).unwrap_or_default(), "seconds": total(&all[day(t)])})
        }).collect();
        let mut apps: Vec<(String, i64)> = today.as_object().map(|m| m.iter().filter_map(|(k, v)| v.as_i64().map(|s| (k.clone(), s))).collect()).unwrap_or_default();
        apps.sort_by(|a, b| b.1.cmp(&a.1));
        let entries = h.st.shell.get::<Entries>();
        let apps: Vec<Value> = apps.into_iter().take(3).map(|(id, s)| {
            let e = entries.and_then(|x| x.by_id(&id));
            json!({"id": id, "name": e.map(|e| e.name.clone()).unwrap_or_else(|| id.clone()), "icon": e.map(|e| e.icon.clone()).unwrap_or_else(|| id.clone()), "seconds": s})
        }).collect();
        json!({"total": total(today), "week": week, "apps": apps})
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_and_keeps_a_week() {
        let now = 1_800_000_000_000;
        let old = day(now - 9 * 86_400_000);
        let all = json!({ old.clone(): {"a": 5} });
        let v = add(&all, now, "firefox", 30);
        let v = add(&v, now, "firefox", 15);
        assert_eq!(v[day(now)]["firefox"], 45);
        assert!(v.get(&old).is_none());
    }
}

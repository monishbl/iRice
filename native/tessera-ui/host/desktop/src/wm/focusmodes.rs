//! Focus.qml: Focus modes: Do Not Disturb, Work, Personal, Sleep and any you
//! add. A mode silences banners except from allowed apps (the notifications
//! surface reads focus.json for that), and can switch the wallpaper and
//! light/dark mode while it's on. Modes run on a schedule or for a while.
//! Modes and the current state live in <config>/focus.json. The menu bar
//! shows it through the shell's state `focus` {on, icon, name, status}.
//! (This is not the focus history: that's focushist.rs.)

use super::hx;
use super::Io;
use serde_json::{json, Value};
use tessera_core::host::Host;
use tessera_core::util::{now_ms, strftime};

fn defaults() -> Vec<Value> {
    let m = |id: &str, name: &str, icon: &str, appearance: &str, from: &str, to: &str, days: Value| {
        json!({"id": id, "name": name, "icon": icon, "allowApps": [], "wallpaper": "", "appearance": appearance, "schedule": {"enabled": false, "from": from, "to": to, "days": days}})
    };
    vec![
        m("dnd", "Do Not Disturb", "moon", "", "22:00", "07:00", json!([0, 1, 2, 3, 4, 5, 6])),
        m("work", "Work", "briefcase", "", "09:00", "17:00", json!([1, 2, 3, 4, 5])),
        m("personal", "Personal", "user", "", "18:00", "22:00", json!([0, 1, 2, 3, 4, 5, 6])),
        m("sleep", "Sleep", "bed", "dark", "22:30", "07:00", json!([0, 1, 2, 3, 4, 5, 6])),
    ]
}

pub fn minutes(hhmm: &str) -> i64 {
    let mut p = hhmm.split(':').map(|x| x.trim().parse::<i64>().unwrap_or(0));
    p.next().unwrap_or(0) * 60 + p.next().unwrap_or(0)
}

/// Is a mode's schedule on at this local time? (minutes since midnight, weekday 0 = Sunday)
/// Windows may cross midnight.
pub fn scheduled_at(mode: &Value, now_min: i64, weekday: i64) -> bool {
    let s = &mode["schedule"];
    if s["enabled"] != true {
        return false;
    }
    let days: Vec<i64> = s["days"].as_array().map(|a| a.iter().filter_map(|d| d.as_i64()).collect()).unwrap_or_default();
    let (from, to) = (minutes(s["from"].as_str().unwrap_or("0:0")), minutes(s["to"].as_str().unwrap_or("0:0")));
    if from <= to {
        return days.contains(&weekday) && now_min >= from && now_min < to;
    }
    // Overnight: started today after `from`, or yesterday and before `to`
    let yesterday = (weekday + 6) % 7;
    (days.contains(&weekday) && now_min >= from) || (days.contains(&yesterday) && now_min < to)
}

fn local_now() -> (i64, i64, i64) {
    let secs = now_ms() / 1000;
    let f = |fmt: &str| strftime(fmt, secs).trim().parse::<i64>().unwrap_or(0);
    (f("%H") * 60 + f("%M"), f("%w"), secs)
}
fn midnight(secs: i64) -> i64 {
    let f = |fmt: &str| strftime(fmt, secs).trim().parse::<i64>().unwrap_or(0);
    secs - (f("%H") * 3600 + f("%M") * 60 + f("%S"))
}

#[derive(Default)]
pub struct FocusModes {
    modes: Vec<Value>,
    pub active: String,
    /// ms timestamp; 0 = until turned off (or the schedule ends)
    until: i64,
    /// manual | schedule
    source: String,
    /// A schedule turned off by hand stays off until its window ends
    skip_until: i64,
    /// what a mode changed, to put back afterwards
    restore: Value,
    loaded: bool,
    clock24: bool,
}

impl FocusModes {
    fn mode(&self) -> Option<&Value> {
        self.modes.iter().find(|m| m["id"] == self.active.as_str())
    }
    fn on(&self) -> bool {
        self.mode().is_some()
    }
    fn by_id(&self, id: &str) -> Option<Value> {
        self.modes.iter().find(|m| m["id"] == id).cloned()
    }
    fn scheduled_now(m: &Value) -> bool {
        let (min, wd, _) = local_now();
        scheduled_at(m, min, wd)
    }
    fn schedule_end(m: &Value) -> i64 {
        let (_, _, now) = local_now();
        let to = minutes(m["schedule"]["to"].as_str().unwrap_or("0:0"));
        let mut end = midnight(now) + to * 60;
        if end <= now {
            end += 86400;
        }
        end * 1000
    }
    fn until_evening() -> i64 {
        let (_, _, now) = local_now();
        let m = midnight(now);
        let h: i64 = strftime("%H", now).trim().parse().unwrap_or(0);
        (if h < 19 { m + 19 * 3600 } else { m + 86400 + 8 * 3600 }) * 1000
    }

    fn fmt_time(&self, ms: i64) -> String {
        strftime(if self.clock24 { "%H:%M" } else { "%-I:%M %p" }, ms / 1000)
    }
    fn fmt_hhmm(&self, hhmm: &str) -> String {
        let (_, _, now) = local_now();
        let t = midnight(now) + minutes(hhmm) * 60;
        self.fmt_time(t * 1000)
    }
    /// "On until 7:00 PM" style status
    pub fn status(&self) -> String {
        let Some(m) = self.mode() else { return "Off".into() };
        if self.until > 0 {
            format!("On until {}", self.fmt_time(self.until))
        } else if self.source == "schedule" {
            format!("On until {}", self.fmt_hhmm(m["schedule"]["to"].as_str().unwrap_or("0:0")))
        } else {
            "On".into()
        }
    }

    pub fn publish(&self, h: &mut Host) {
        let m = self.mode().cloned();
        h.set_state("focus.on", json!(m.is_some()));
        h.set_state("focus.icon", json!(m.as_ref().and_then(|m| m["icon"].as_str()).unwrap_or("moon")));
        h.set_state("focus.name", json!(m.as_ref().and_then(|m| m["name"].as_str()).unwrap_or("Focus")));
        h.set_state("focus.status", json!(self.status()));
    }

    // ── effects ──────────────────────────────────────────────────────────────
    fn apply_effects(&mut self, h: &mut Host, m: &Value) {
        let mut r = serde_json::Map::new();
        if let Some(wp) = m["wallpaper"].as_str().filter(|s| !s.is_empty()) {
            let w = &h.cfg()["wallpaper"];
            r.insert("wallpaper".into(), json!({"path": w["path"], "dynamic": w["dynamic"], "live": w["live"]}));
            h.ask("wallpaper.set", json!([wp]));
        }
        if let Some(a) = m["appearance"].as_str().filter(|s| !s.is_empty()) {
            let cur = h.cfg()["appearance"]["mode"].clone();
            if cur != a {
                r.insert("appearance".into(), cur);
                hx::set_cfg("appearance", "mode", json!(a));
            }
        }
        self.restore = Value::Object(r);
    }
    fn undo_effects(&mut self) {
        let r = std::mem::replace(&mut self.restore, json!({}));
        if let Some(w) = r.get("wallpaper") {
            let path = if w.is_string() { w.clone() } else { w["path"].clone() };
            hx::set_cfg("wallpaper", "path", path);
            hx::set_cfg("wallpaper", "dynamic", if w["dynamic"].is_null() { json!("off") } else { w["dynamic"].clone() });
            hx::set_cfg("wallpaper", "live", if w["live"].is_null() { json!("") } else { w["live"].clone() });
        }
        if let Some(a) = r.get("appearance") {
            hx::set_cfg("appearance", "mode", a.clone());
        }
    }

    // ── actions ──────────────────────────────────────────────────────────────
    /// duration_ms: 0 = until turned off; -1 = until this evening (or tomorrow morning)
    pub fn activate(&mut self, h: &mut Host, io: &mut Io, id: &str, duration_ms: i64, from_schedule: bool) {
        let Some(m) = self.by_id(id) else { return };
        let again = self.on() && self.active == id; // same mode, new duration
        if self.on() && !again {
            self.undo_effects();
        }
        self.active = id.to_string();
        self.source = if from_schedule { "schedule" } else { "manual" }.into();
        self.until = if duration_ms > 0 { now_ms() + duration_ms } else if duration_ms == -1 { Self::until_evening() } else { 0 };
        // Re-applying would overwrite what to restore with the mode's own look
        if !again {
            self.apply_effects(h, &m);
        }
        self.changed(h, io);
    }
    pub fn deactivate(&mut self, h: &mut Host, io: &mut Io) {
        let Some(m) = self.mode().cloned() else { return };
        if self.source == "schedule" || Self::scheduled_now(&m) {
            self.skip_until = Self::schedule_end(&m);
        }
        self.undo_effects();
        self.active.clear();
        self.until = 0;
        self.source.clear();
        self.changed(h, io);
    }
    pub fn toggle(&mut self, h: &mut Host, io: &mut Io, id: &str) {
        let id = if id.is_empty() { "dnd" } else { id };
        if self.active == id {
            self.deactivate(h, io);
        } else {
            self.activate(h, io, id, 0, false);
        }
    }

    pub fn tick(&mut self, h: &mut Host, io: &mut Io) {
        let now = now_ms();
        if self.on() && self.until > 0 && now >= self.until {
            return self.deactivate(h, io);
        }
        if let Some(m) = self.mode().cloned() {
            if self.source == "schedule" && !Self::scheduled_now(&m) {
                self.undo_effects();
                self.active.clear();
                self.source.clear();
                return self.changed(h, io);
            }
        } else if now >= self.skip_until {
            if let Some(id) = self.modes.iter().find(|m| Self::scheduled_now(m)).and_then(|m| m["id"].as_str().map(String::from)) {
                self.activate(h, io, &id, 0, true);
            }
        }
        self.publish(h); // (the status text ages)
    }

    /// Edit modes from Settings
    pub fn set_mode(&mut self, h: &mut Host, io: &mut Io, id: &str, patch: &Value) {
        for m in self.modes.iter_mut().filter(|m| m["id"] == id) {
            if let (Some(o), Some(p)) = (m.as_object_mut(), patch.as_object()) {
                for (k, v) in p {
                    o.insert(k.clone(), v.clone());
                }
            }
        }
        self.changed(h, io);
    }
    pub fn add_mode(&mut self, h: &mut Host, io: &mut Io, name: &str) {
        let slug: String = name.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
        let id = format!("{slug}-{}", now_ms());
        self.modes.push(json!({"id": id, "name": name, "icon": "circle", "allowApps": [], "wallpaper": "", "appearance": "", "schedule": {"enabled": false, "from": "09:00", "to": "17:00", "days": [1, 2, 3, 4, 5]}}));
        self.changed(h, io);
    }
    /// Undo of remove_mode: the mode comes back as it was (unless one with its id is there)
    pub fn restore_mode(&mut self, h: &mut Host, io: &mut Io, mode: &Value) {
        let Some(id) = mode["id"].as_str() else { return };
        if self.modes.iter().any(|m| m["id"] == id) {
            return;
        }
        self.modes.push(mode.clone());
        self.changed(h, io);
    }
    pub fn remove_mode(&mut self, h: &mut Host, io: &mut Io, id: &str) {
        if self.active == id {
            self.deactivate(h, io);
        }
        self.modes.retain(|m| m["id"] != id);
        self.changed(h, io);
    }

    // ── persistence ──────────────────────────────────────────────────────────
    fn path() -> std::path::PathBuf {
        hx::config_dir().join("focus.json")
    }
    fn changed(&mut self, h: &mut Host, io: &mut Io) {
        self.publish(h);
        if self.loaded {
            io.arm("focus.save", 300);
        }
    }
    pub fn save(&self) {
        let text = serde_json::to_string_pretty(&json!({"modes": self.modes, "active": self.active, "until": self.until, "source": self.source, "restore": self.restore, "skipScheduleUntil": self.skip_until})).unwrap_or_default();
        let path = Self::path();
        std::thread::spawn(move || {
            let _ = tessera_core::util::write_atomic(&path, text.as_bytes());
        });
    }
    /// Read the file again (Settings → About restored it)
    pub fn reload(&mut self, h: &mut Host, io: &mut Io) {
        self.modes = defaults();
        match std::fs::read_to_string(Self::path()).ok().and_then(|s| serde_json::from_str::<Value>(&s).ok()) {
            Some(j) => {
                if let Some(a) = j["modes"].as_array().filter(|a| !a.is_empty()) {
                    self.modes = a.clone();
                }
                self.active = j["active"].as_str().unwrap_or("").into();
                self.until = j["until"].as_f64().unwrap_or(0.0) as i64;
                self.source = j["source"].as_str().unwrap_or("").into();
                self.restore = if j["restore"].is_object() { j["restore"].clone() } else { json!({}) };
                self.skip_until = j["skipScheduleUntil"].as_f64().unwrap_or(0.0) as i64;
                self.loaded = true;
            }
            None => {
                self.loaded = true;
                self.save();
            }
        }
        self.clock24 = h.cfg()["appearance"]["clock24"].as_bool().unwrap_or(false);
        self.tick(h, io);
    }
    pub fn begin(&mut self, h: &mut Host, io: &mut Io) {
        self.restore = json!({});
        self.reload(h, io);
        io.every("focus.tick", 30000);
    }
    pub fn config_changed(&mut self, h: &mut Host) {
        let c = h.cfg()["appearance"]["clock24"].as_bool().unwrap_or(false);
        if c != self.clock24 {
            self.clock24 = c;
            self.publish(h);
        }
    }

    pub fn ipc(&mut self, h: &mut Host, io: &mut Io, func: &str, args: &[String]) -> Option<String> {
        let id = args.first().map(String::as_str).filter(|s| !s.is_empty()).unwrap_or("dnd").to_string();
        match func {
            "on" => self.activate(h, io, &id, 0, false),
            "off" => self.deactivate(h, io),
            "toggle" => self.toggle(h, io, &id),
            _ => {}
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mode(from: &str, to: &str, days: Value) -> Value {
        json!({"schedule": {"enabled": true, "from": from, "to": to, "days": days}})
    }

    #[test]
    fn a_daytime_window_is_on_between_its_times_on_its_days() {
        let m = mode("09:00", "17:00", json!([1, 2, 3, 4, 5]));
        assert!(scheduled_at(&m, 9 * 60, 1) && scheduled_at(&m, 16 * 60 + 59, 5));
        assert!(!scheduled_at(&m, 17 * 60, 1) && !scheduled_at(&m, 12 * 60, 0));
    }

    #[test]
    fn an_overnight_window_runs_past_midnight() {
        let m = mode("22:00", "07:00", json!([5]));
        assert!(scheduled_at(&m, 23 * 60, 5)); // Friday night
        assert!(scheduled_at(&m, 6 * 60, 6)); // Saturday early morning, started Friday
        assert!(!scheduled_at(&m, 6 * 60, 5)); // Friday early morning: started Thursday
        let off = json!({"schedule": {"enabled": false}});
        assert!(!scheduled_at(&off, 600, 1));
    }

    #[test]
    fn hh_mm_reads_as_minutes() {
        assert_eq!(minutes("22:30"), 1350);
        assert_eq!(minutes("7"), 420);
    }
}

//! Today.qml (when the morning screen opens, what it greets with, the widget)
//! and modules/today/TodayScreen.qml's seam (it tells the Today surface when
//! to open and close: {"to":"today","open":true,"effect":…,"screen":…,
//! "options":…,"motion":…,"accounts":{…}}, {"to":"today","open":false}).
//!
//! Days start at 4 AM. Modes (Settings → Desktop & Widgets → Today): ask (the
//! morning screen; "Pin for today" keeps it as a widget until the day ends),
//! widget, both, off. The first time in a day that the desktop is there to
//! see (the screen not locked) it opens by itself, after the unlock
//! animation has settled (900 ms; 2.8 s in the first 10 s after login, so the
//! startup chime isn't overlapped). The chime is timed to land as the panel
//! settles.
//!
//! Calls from the surface: today.close, today.pin, today.set [key, value],
//! today [options?] (the widget), today.show [effect] (Spotlight's Show
//! Today). Ipc target "today": preview <bloom|shrink|drop|"">, close.
//!
//! Differences: the accounts' events and tasks (`accounts`) are empty (the
//! system host holds the sign-ins); Wallpaper.started is taken as true.

use super::common::*;
use serde_json::{json, Value};
use tessera_core::host::Host;
use tessera_core::services::clients::Windows;

pub const EFFECTS: [&str; 3] = ["bloom", "shrink", "drop"];

pub struct Morning {
    pub open: bool,
    pub options_open: bool,
    pub effect: String,
    pub day: String,
    /// the screen it opened on
    pub screen: String,
    pub started_at: i64,
    /// last seen "ready" (the screen isn't locked)
    pub was_ready: bool,
    pub chime: slint::Timer,
    pub delayed: slint::Timer,
}

impl Default for Morning {
    fn default() -> Self {
        Self {
            open: false,
            options_open: false,
            effect: "bloom".into(),
            day: day_key(now()),
            screen: String::new(),
            started_at: now(),
            was_ready: false,
            chime: slint::Timer::default(),
            delayed: slint::Timer::default(),
        }
    }
}

/// today.mode, one of ask | widget | both | off (ask when unset or odd)
pub fn mode() -> String {
    let m = cfg_get("today.mode");
    match m.as_str() {
        Some(x) if ["ask", "widget", "both", "off"].contains(&x) => x.to_string(),
        _ => "ask".into(),
    }
}

pub fn screen_on() -> bool {
    matches!(mode().as_str(), "ask" | "both")
}

pub fn widget_always() -> bool {
    matches!(mode().as_str(), "widget" | "both")
}

/// The first name, from Settings → About (Your name) or the account
pub fn first_name() -> String {
    let n = cfg_get("today.name");
    let full = n.as_str().filter(|s| !s.trim().is_empty()).map(str::to_string).unwrap_or_else(account_name);
    full.split_whitespace().next().unwrap_or("").to_string()
}

/// The account's name (its full name, else its login)
fn account_name() -> String {
    unsafe {
        let pw = libc::getpwuid(libc::getuid());
        if !pw.is_null() {
            let g = std::ffi::CStr::from_ptr((*pw).pw_gecos).to_string_lossy().into_owned();
            let g = g.split(',').next().unwrap_or("").trim().to_string();
            if !g.is_empty() {
                return g;
            }
            return std::ffi::CStr::from_ptr((*pw).pw_name).to_string_lossy().into_owned();
        }
    }
    std::env::var("USER").unwrap_or_default()
}

/// Today.greeting
pub fn greeting(hour: i32, name: &str) -> String {
    let part = if (4..12).contains(&hour) {
        "Good morning"
    } else if (12..17).contains(&hour) {
        "Good afternoon"
    } else {
        "Good evening"
    };
    if name.is_empty() { part.into() } else { format!("{part}, {name}") }
}

fn has_widget() -> bool {
    cfg_get("desktop.widgets").as_array().is_some_and(|a| a.iter().any(|e| e.as_str().is_some_and(|s| s.split(':').next() == Some("today"))))
}

impl Morning {
    pub fn start(&mut self, h: &mut Host) {
        self.day = day_key(now());
        self.tidy_widgets(h);
        self.was_ready = !self.locked(h);
        if self.was_ready {
            self.maybe_show(h);
        }
    }

    fn locked(&self, h: &Host) -> bool {
        h.state("locked").as_bool().unwrap_or(false)
    }

    fn motion_scale(&self) -> f64 {
        1.0
    }

    /// Today.show(fx)
    pub fn show(&mut self, h: &mut Host, fx: &str) {
        let entrance = cfg_get("today.entrance");
        self.effect = if EFFECTS.contains(&fx) {
            fx.to_string()
        } else if let Some(e) = entrance.as_str().filter(|e| EFFECTS.contains(e)) {
            e.to_string()
        } else {
            "bloom".into()
        };
        // The chime climbs to its peak 0.66 s in: start it so the peak lands
        // as the panel settles (bloom 660, shrink 640, drop 980 ms, times the
        // motion scale; reduced motion: a 200 ms fade)
        let still = cfg_get("appearance.reduceMotion").as_bool() == Some(true);
        let settle = if still {
            0.0
        } else {
            (match self.effect.as_str() {
                "shrink" => 640.0,
                "drop" => 980.0,
                _ => 660.0,
            }) * self.motion_scale()
        };
        let wait = (settle - 660.0).max(0.0).round() as u64;
        let send = h.sender(super::NAME);
        self.chime.start(slint::TimerMode::SingleShot, std::time::Duration::from_millis(wait), move || send(Box::new(super::Msg::TodayChime)));
        self.screen = h.st.shell.get::<Windows>().and_then(|w| w.focused_monitor()).map(|m| m.name.clone()).unwrap_or_default();
        self.open = true;
        self.tell_open(h);
        h.set_state("today", json!({"open": true}));
    }

    pub fn chime(&mut self, h: &mut Host) {
        if self.open {
            h.ask("sound.play", json!(["today"]));
        }
    }

    fn tell_open(&self, h: &mut Host) {
        h.tell(
            "today",
            json!({"open": true, "effect": self.effect, "screen": self.screen, "options": self.options_open, "motion": self.motion_scale(), "accounts": {"events": [], "todos": []}}),
        );
    }

    pub fn close(&mut self, h: &mut Host) {
        let was = self.open;
        self.open = false;
        self.options_open = false;
        self.chime.stop();
        if was {
            h.tell("today", json!({"open": false}));
        }
        h.set_state("today", json!({"open": false}));
    }

    pub fn open_options(&mut self, h: &mut Host) {
        self.options_open = true;
        if self.open {
            h.tell("today", json!({"options": true}));
        }
    }

    // ── When it opens by itself ─────────────────────────────────────────

    pub fn maybe_show(&mut self, h: &mut Host) {
        if self.locked(h) || !screen_on() || self.open || state_get("todayShown").as_str() == Some(self.day.as_str()) {
            return;
        }
        let wait = if now() - self.started_at < 10_000 { 2800 } else { 900 };
        let send = h.sender(super::NAME);
        self.delayed.start(slint::TimerMode::SingleShot, std::time::Duration::from_millis(wait), move || send(Box::new(super::Msg::TodayDelayed)));
    }

    /// After the delay: the day only counts as shown once it actually opened
    /// (locking again within the delay leaves it for the next unlock)
    pub fn delayed(&mut self, h: &mut Host) {
        if !self.locked(h) && screen_on() && state_get("todayShown").as_str() != Some(self.day.as_str()) {
            state_set("todayShown", json!(self.day));
            self.show(h, "");
        }
    }

    /// The 1 s tick: the day turning over (checked each minute), the screen
    /// unlocking
    pub fn tick(&mut self, h: &mut Host, second: u64) {
        let ready = !self.locked(h);
        if ready && !self.was_ready {
            self.maybe_show(h);
        }
        self.was_ready = ready;
        if second % 60 == 0 {
            let d = day_key(now());
            if d != self.day {
                self.day = d;
                self.tidy_widgets(h);
            }
        }
    }

    // ── The widget ──────────────────────────────────────────────────────

    pub fn pinned(&self) -> bool {
        state_get("todayPinned").as_str() == Some(self.day.as_str())
    }

    pub fn pin(&mut self, h: &mut Host) {
        state_set("todayPinned", json!(self.day));
        self.ensure_widget(h);
        self.close(h);
    }

    pub fn ensure_widget(&mut self, h: &mut Host) {
        if !has_widget() {
            // (the desktop adds it in a free spot; Panels.addWidget("today", "l"))
            h.tell("desktop", json!({"add": ["today", "l"]}));
        }
    }

    /// The day changed (or the setting): a widget pinned for an earlier day
    /// goes, unless Today is meant to be there every day
    pub fn tidy_widgets(&mut self, h: &mut Host) {
        let was = state_get("todayPinned");
        if let Some(w) = was.as_str().filter(|w| !w.is_empty() && *w != self.day) {
            let _ = w;
            state_set("todayPinned", json!(""));
            if !widget_always() {
                let keep: Vec<Value> = cfg_get("desktop.widgets")
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|e| !matches!(e.as_str(), Some(s) if s.split(':').next() == Some("today")))
                    .collect();
                cfg_set("desktop.widgets", Value::Array(keep));
            }
        }
        // Once a day: removing it keeps it gone until tomorrow
        if widget_always() && state_get("todayEnsured").as_str() != Some(self.day.as_str()) {
            state_set("todayEnsured", json!(self.day));
            self.ensure_widget(h);
        }
    }

    pub fn config_changed(&mut self, h: &mut Host, before_always: bool) {
        if widget_always() && !before_always {
            state_set("todayEnsured", json!(self.day));
            self.ensure_widget(h);
        }
    }

    /// today.set [key, value]: Today's own options
    pub fn set(&mut self, args: &Value) {
        let a = args_of(args);
        let key = a.first().and_then(Value::as_str).unwrap_or("");
        let v = a.get(1).cloned().unwrap_or(Value::Null);
        match key {
            "mode" if v.as_str().is_some_and(|m| ["ask", "widget", "both", "off"].contains(&m)) => cfg_set("today.mode", v),
            "entrance" if v.as_str().is_some_and(|m| EFFECTS.contains(&m)) => cfg_set("today.entrance", v),
            "name" if v.is_string() => cfg_set("today.name", json!(v.as_str().unwrap_or("").trim().chars().take(100).collect::<String>())),
            "chime" if v.is_boolean() => cfg_set("sounds.today", v),
            _ => {}
        }
    }

    pub fn call(&mut self, h: &mut Host, call: &str, args: &Value) {
        match call {
            "today.close" => self.close(h),
            "today.pin" => self.pin(h),
            "today.set" => self.set(args),
            "today.show" => self.show(h, &arg_str(args, 0)),
            _ => {
                // today [options?]: the widget's
                if args_of(args).first().and_then(Value::as_bool) == Some(true) {
                    self.options_open = true;
                }
                self.show(h, "");
            }
        }
    }

    pub fn ipc(&mut self, h: &mut Host, func: &str, args: &[String]) -> Option<String> {
        match func {
            "preview" => self.show(h, args.first().map_or("", String::as_str)),
            "close" => self.close(h),
            _ => {}
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greets_by_the_hour() {
        assert_eq!(greeting(8, "Ana"), "Good morning, Ana");
        assert_eq!(greeting(13, ""), "Good afternoon");
        assert_eq!(greeting(3, "Ana"), "Good evening, Ana");
        assert_eq!(greeting(17, "x"), "Good evening, x");
    }

    #[test]
    fn the_day_starts_at_four() {
        let late = tessera_core::services::calendar::make(2026, 10, 6, 2, 30, 0);
        let day = tessera_core::services::calendar::make(2026, 10, 6, 5, 0, 0);
        assert_eq!(day_key(late), "2026-11-05");
        assert_eq!(day_key(day), "2026-11-06");
    }
}

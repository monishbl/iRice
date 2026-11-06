//! The day's services (wave 4, host/desktop): Today, Todos, Timers, Alarms,
//! Weather, Calendar, Notes, Notifications, Media, Calls and Badges of the
//! QML shell, plus the seams of modules/today/TodayScreen.qml and
//! modules/notes/NotesFeed.qml. One host service (`Today`) holds them: they
//! share one clock (a one-second tick on the loop) and call each other
//! directly. Each module says which QML file it replaces.
//!
//! What it answers (Service::calls): today, todos, todo, todoAdd, notes,
//! notify, and besides (see PORT-NOTES-today.md, "Unowned/added"): alarms,
//! notifications, nc, media, badges. Ipc targets: today, timer, calendar,
//! calls.
//!
//! Other host modules may use the plain functions: `parse::parse_quick`,
//! `parse::parse_duration`, `parse::parse_clock`, `parse::describe`,
//! `notes::search`, `notes::list`, `notes::save_link`, `todos::add`,
//! `timers::add_timer`, `calendar::add`.

pub mod badges;
pub mod calendar;
pub mod calls;
pub mod common;
pub mod media;
pub mod morning;
pub mod notes;
pub mod notif;
pub mod parse;
pub mod timers;
pub mod todos;
pub mod weather;

use common::*;
use serde_json::{json, Value};
use std::any::Any;
use tessera_core::host::{Host, Service};

pub const NAME: &str = "Today";

/// Messages for the service from its timers and threads (Host::sender)
pub enum Msg {
    /// the one-second tick
    Second,
    /// the stopwatch's hundredths, while a view shows them
    Fine,
    TodayChime,
    TodayDelayed,
    /// the ringtone again
    Ring,
    RingGiveUp,
    NoteSaved(notes::Saved),
    Badge(badges::Update),
}

pub struct Hub {
    tick: u64,
    master: slint::Timer,
    fine: slint::Timer,
    fine_on: bool,
    morning: morning::Morning,
    timers: timers::Timers,
    cal: calendar::Cal,
    notif: notif::Notif,
    calls: calls::Calls,
    media: media::Media,
    badges: badges::Badges,
    weather: weather::W,
    widget_always: bool,
}

/// Every service of this module (host/desktop's lib.rs collects them)
pub fn services() -> Vec<Box<dyn Service>> {
    vec![Box::new(Hub {
        tick: 0,
        master: slint::Timer::default(),
        fine: slint::Timer::default(),
        fine_on: false,
        morning: morning::Morning::default(),
        timers: timers::Timers::default(),
        cal: calendar::Cal::default(),
        notif: notif::Notif::default(),
        calls: calls::Calls::default(),
        media: media::Media::default(),
        badges: badges::Badges::default(),
        weather: weather::W::default(),
        widget_always: false,
    })]
}

/// The event a call carries: the fields as an object (a lone argument array
/// holding it counts too)
fn event_of(args: &Value) -> Value {
    if args.is_object() { args.clone() } else { args.get(0).cloned().unwrap_or(Value::Null) }
}

impl Hub {
    /// The stopwatch's fast tick runs only while a view shows hundredths
    fn sync_fine(&mut self, h: &mut Host) {
        let want = self.timers.sw_fine > 0 && self.timers.sw_running;
        if want == self.fine_on {
            return;
        }
        self.fine_on = want;
        if want {
            let send = h.sender(NAME);
            self.fine.start(slint::TimerMode::Repeated, std::time::Duration::from_millis(50), move || send(Box::new(Msg::Fine)));
        } else {
            self.fine.stop();
        }
    }

    fn second(&mut self, h: &mut Host) {
        self.tick += 1;
        let t = self.tick;
        let now = now();
        self.timers.tick(h, now);
        timers::check(h);
        self.calls.tick(h);
        self.morning.tick(h, t);
        self.notif.tell_shell(h);
        self.notif.center_changed(h, &mut self.timers);
        self.notif.undo_expired(now);
        self.notif.push_widgets(h, &self.timers, now);
        self.sync_fine(h);
        if t % 5 == 1 {
            self.weather.publish(h);
            self.media.publish(h);
            self.badges.publish(h);
            timers::publish_alarms(h);
        }
        if t % 30 == 1 {
            todos::check_due();
            self.cal.check_alerts(h);
        }
    }

    /// Notification Center's asks (`{"kind":"nc","act":…}`)
    fn nc_act(&mut self, h: &mut Host, e: &Value) {
        let now = now();
        let s = |k: &str| e[k].as_str().unwrap_or("").to_string();
        match e["act"].as_str().unwrap_or("") {
            "close" => {
                if h.state("open").as_str() == Some("notificationCenter") {
                    h.ask("panels.close", json!([]));
                }
            }
            "manage" => h.ask("panels.openSettings", json!(["notifications"])),
            "tab" => {
                self.notif.tab = e["tab"].as_i64().unwrap_or(0);
                h.ask("panels.ncTab", json!([self.notif.tab]));
            }
            "calendar" => {
                self.notif.nc_cal = Some((s("from"), s("to"), s("day")));
                self.notif.last_calendar.clear();
                self.notif.send_calendar(h);
            }
            "todo.add" => {
                if !s("text").trim().is_empty() {
                    todos::add(h, &s("text"));
                }
            }
            "todo.toggle" => todos::toggle(&s("key")),
            "todo.rename" => {
                if !s("text").trim().is_empty() {
                    todos::update(&s("key"), &json!({"text": s("text").trim()}));
                }
            }
            "todo.remove" => {
                self.notif.undo = todos::remove(&s("key"));
                self.notif.undo_at = now;
            }
            "todo.restore" => {
                if let Some(u) = self.notif.undo.take() {
                    todos::restore(&u);
                }
            }
            "timer.toggle" => self.timers.toggle(now),
            "timer.skip" => self.timers.advance(h, now),
            "timer.reset" => self.timers.reset(now),
            "sw.toggle" => self.timers.sw_toggle(now),
            "sw.lap" => self.timers.sw_lap(now),
            "sw.reset" => self.timers.sw_reset(),
            "sw.fine" => {
                let on = e["on"].as_bool().unwrap_or(false) && self.notif.center_open;
                self.notif.set_fine(on, &mut self.timers);
            }
            "island.forget" => h.ask("island.forget", json!([e["hid"].clone()])),
            "island.clear" => h.ask("island.clear", json!([])),
            "alert.act" => h.ask("island.alertAct", json!([e["hid"].clone()])),
            other => eprintln!("tessera-ui: host: no Notification Center act {other}"),
        }
        self.timers.publish(h, now);
        self.sync_fine(h);
        self.notif.push_widgets(h, &self.timers, now);
    }

    /// `islandAct <uid> <which>` for an alert made here
    fn island_act(&mut self, h: &mut Host, text: &str) {
        let mut p = text.splitn(2, ' ');
        let uid = p.next().and_then(|u| u.parse::<i64>().ok()).unwrap_or(0);
        let which = p.next().unwrap_or("");
        // the ringing card: Answer is "1", Decline "2" (what the island's buttons say)
        if self.calls.ringing_uid() == Some(uid) {
            match which {
                "1" => self.calls.answer(h),
                "2" => self.calls.decline(h),
                _ => {}
            }
            return;
        }
        self.notif.island_act(h, uid, which);
    }
}

impl Service for Hub {
    fn name(&self) -> &'static str {
        NAME
    }

    fn calls(&self) -> &'static [&'static str] {
        &["today", "todos", "todo", "todoAdd", "notes", "notify", "alarms", "notifications", "nc", "media", "badges", "calendar", "calls"]
    }

    fn ipc_targets(&self) -> &'static [&'static str] {
        &["today", "timer", "calendar", "calls"]
    }

    fn start(&mut self, h: &mut Host) {
        h.st.shell.want("calendar");
        h.st.shell.want("clients");
        h.st.shell.want("apps");
        todos::give_ids();
        self.widget_always = morning::widget_always();
        self.weather.start(h);
        self.media.start(h);
        self.badges.start(h);
        self.notif.start(h);
        notes::send_folder(h);
        self.morning.start(h);
        self.timers.publish(h, now());
        timers::publish_alarms(h);
        let send = h.sender(NAME);
        self.master.start(slint::TimerMode::Repeated, std::time::Duration::from_secs(1), move || send(Box::new(Msg::Second)));
    }

    fn call(&mut self, h: &mut Host, call: &str, args: &Value) {
        let prefix = call.split('.').next().unwrap_or(call);
        match prefix {
            "today" => {
                if call == "today.islandAct" {
                    self.island_act(h, &arg_str(args, 0));
                } else {
                    self.morning.call(h, call, args);
                }
            }
            "todos" | "todo" | "todoAdd" => todos::call(h, call, args),
            "notes" => notes::call(h, call, args),
            "notify" => self.notif.settings_call(h, call, args),
            "alarms" => timers::call(call, args),
            "notifications" => {
                let e = event_of(args);
                self.notif.from_surface(h, &e["list"], &mut self.calls);
            }
            "nc" => {
                let e = event_of(args);
                self.nc_act(h, &e);
            }
            "calendar" => calendar::call(call, args),
            // `calls.answer|decline|open|leave|openApp [entry, key]` (the island's buttons, Calls' IPC ones)
            "calls" => {
                let a: Vec<String> = args.as_array().map(|l| l.iter().map(|v| v.as_str().map(String::from).unwrap_or_else(|| v.to_string())).collect()).unwrap_or_default();
                self.calls.ipc(h, call.strip_prefix("calls.").unwrap_or(""), &a);
            }
            "media" => self.media.call(h, call, args),
            "badges" => {
                if self.badges.call(args) {
                    self.badges.publish(h);
                }
            }
            _ => {}
        }
    }

    fn ipc(&mut self, h: &mut Host, target: &str, func: &str, args: &[String]) -> Option<String> {
        match target {
            "today" if func == "islandAct" => {
                self.island_act(h, &args.join(" "));
                None
            }
            "today" => self.morning.ipc(h, func, args),
            "timer" => {
                let r = self.timers.ipc(h, func, now());
                self.sync_fine(h);
                r
            }
            "calendar" => calendar::ipc(func, args),
            "calls" => self.calls.ipc(h, func, args),
            _ => None,
        }
    }

    fn config_changed(&mut self, h: &mut Host) {
        let always = morning::widget_always();
        self.morning.config_changed(h, self.widget_always);
        self.widget_always = always;
        self.weather.config_changed(h);
        notes::send_folder(h);
    }

    fn message(&mut self, h: &mut Host, msg: Box<dyn Any>) {
        let Ok(m) = msg.downcast::<Msg>() else { return };
        match *m {
            Msg::Second => self.second(h),
            Msg::Fine => self.notif.push_widgets(h, &self.timers, now()),
            Msg::TodayChime => self.morning.chime(h),
            Msg::TodayDelayed => self.morning.delayed(h),
            Msg::Ring => self.calls.ring_tick(),
            Msg::RingGiveUp => self.calls.missed(h),
            Msg::NoteSaved(s) => notes::saved(h, s),
            Msg::Badge(u) => {
                if self.badges.apply(u) {
                    self.badges.publish(h);
                }
            }
        }
    }
}

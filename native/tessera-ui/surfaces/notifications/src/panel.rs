//! Notification Center's panel (NotificationCenter.qml in Glass,
//! NotificationShade.qml in Material): the list grouped by app with the
//! island's alerts as one more group, Clear all (Material: Manage too), and
//! the Calendar, To-do and Focus widgets. Opened through the shell's Panels
//! as before ("notificationCenter"): services/Notifications.qml forwards each
//! open and close as a command line ({"cmd":"open","surface":"notifications",
//! "screen":…}), and hears this panel's acts as {"src":"ui","kind":"nc",
//! "act":…} lines (close, tab, calendar, todo.*, timer.*, sw.*, island.*,
//! alert.act, manage). The widgets' data stays the shell's (Calendar, Todos,
//! Timers, Island): it sends it as {"to":"notifications","do":"widgets",…}
//! while the panel is open, and the calendar's days on request
//! ({"do":"calendar",…}).
//!
//! The calendar's editor (+, double-clicking a date, clicking an item,
//! dragging one to another date, Delete with Undo) is the shared CalendarTab
//! (core/ui/calendar.slint): this checks what it saves (CalendarWidget's
//! editor.save: a title, times it can read), asks "this or all?" for a
//! repeating event, and sends the writes as acts (cal.save, cal.remove,
//! cal.restore, cal.move); a write the host can't do comes back as
//! {"do":"calendarError","text":…} and reopens the editor with it.
//!
//! Not like the QML yet: a swipe's fling speed (only how far it went counts), Esc while a field has
//! the keys (it leaves the field first), and Material's Phosphor icons where
//! the QML draws Material Symbols.

use super::center::{self, Alert, Day, Entry, Group, Kind};
use super::{clock_at, markup, model, now_ms, Notifications, Shown, NAME};
use crate::services::hypr::Glass;
use crate::shell::{tell, Command};
use crate::theme;
use crate::ui;
use crate::wayland::{Spec, State, Surface};
use serde_json::{json, Value};
use slint::{ComponentHandle, Model};
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use smithay_client_toolkit::shell::WaylandSurface;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::Duration;
use tessera_core::rows::Rows;

const NS: &str = "tessera:notification-center";

/// The panel's own messages (from its callbacks)
pub enum C {
    Dismissed,
    Closed,
    Activated(i32),
    Close(i32),
    Toggle(String),
    ClearGroup(String),
    Action(i32, String),
    ReplyOpen(i32),
    ReplyCancel(i32),
    ReplySend(i32, String),
    Draft(i32, String),
    Link(String),
    ClearAll,
    Manage,
    Tab(i32),
    CalStep(i32),
    CalWeek(bool),
    CalPick(String),
    TodoAdd(String),
    TodoToggle(String),
    TodoRemove(String),
    TodoRestore,
    TodoRename(String, String),
    CalSave(ui::NcDraft, i32),
    CalRemove(String, i32),
    CalMove(String, String, i32),
    CalRestore,
    FocusMode(i32),
    Act(String),
    Card(i32, [f32; 4]),
    Minute,
}

struct Win {
    output: String,
    surface: Rc<Surface>,
    ui: ui::Center,
    /// the list, updated in place and matched by card (a card keeps its
    /// hover, its composer's focus and its swipe while the widgets tick,
    /// and while others arrive or leave)
    rows: Rows<ui::NcRow>,
    /// the widgets' lists, in place too (a to-do being renamed keeps its field)
    days: Rows<ui::NcDay>,
    day_items: Rows<ui::NcItem>,
    todos: Rows<ui::NcTodo>,
    laps: Rows<ui::NcLap>,
    targets: Rows<ui::NcTarget>,
    a11y: Option<crate::a11y::A11y>,
    glass_sent: String,
}

pub struct Panel {
    win: Option<Win>,
    groups_open: HashSet<String>,
    groups: Vec<Group>,
    rows: Vec<Entry>,
    alerts: Vec<Alert>,
    replies: HashMap<u32, Shown>,
    tab: i32,
    focus_mode: i32,
    shown: (i32, u32),
    selected: Day,
    week: bool,
    /// what the shell last said: the widgets, the calendar's days
    widgets: Value,
    calendar: Value,
    /// each row's card as laid out (y in the list, height, width, edges)
    cards: HashMap<i32, [f32; 4]>,
    minute: slint::Timer,
    /// small lists inside rows, the same model while the same (stable.rs)
    alert_buttons: super::stable::Stable<ui::NoteButton>,
    day_colors: super::stable::Stable<slint::Color>,
    day_lines: super::stable::Stable<ui::NcItem>,
}

impl Default for Panel {
    fn default() -> Self {
        let today = today();
        Self {
            win: None,
            groups_open: HashSet::new(),
            groups: Vec::new(),
            rows: Vec::new(),
            alerts: Vec::new(),
            replies: HashMap::new(),
            tab: 0,
            focus_mode: 0,
            shown: (today.0, today.1),
            selected: today,
            week: false,
            widgets: Value::Null,
            calendar: Value::Null,
            cards: HashMap::new(),
            minute: slint::Timer::default(),
            alert_buttons: Default::default(),
            day_colors: Default::default(),
            day_lines: Default::default(),
        }
    }
}

impl Panel {
    pub(super) fn output(&self) -> Option<String> {
        self.win.as_ref().map(|w| w.output.clone())
    }

    pub(super) fn win_a11y(&mut self) -> Option<&mut crate::a11y::A11y> {
        self.win.as_mut().and_then(|w| w.a11y.as_mut())
    }
}

fn today() -> Day {
    let t = crate::util::now("%Y %m %d");
    let mut it = t.split(' ').filter_map(|x| x.parse::<i64>().ok());
    match (it.next(), it.next(), it.next()) {
        (Some(y), Some(m), Some(d)) => (y as i32, m as u32, d as u32),
        _ => (1970, 1, 1),
    }
}

/// Glass's material is solid (Theme.glass.tier): then Notification Center
/// keeps its panel; otherwise, unless Settings asks for the panel
/// (notificationCenter.background), each piece is glass of its own
pub fn bare(cfg: &Value, material: bool, transparency: f32) -> bool {
    let a = &cfg["appearance"];
    let solid = a["reduceTransparency"].as_bool().unwrap_or(false)
        || if material {
            // (Material's veil: as solid as its panels)
            1.0 - 0.55 * transparency >= 0.97
        } else {
            match a["glass"].as_str().unwrap_or("auto") {
                "solid" => true,
                "custom" => a["glassSolid"].as_f64().unwrap_or(0.0) >= 0.97,
                _ => false,
            }
        };
    !solid && !cfg["notificationCenter"]["background"].as_bool().unwrap_or(false)
}

fn nc(act: &str, mut more: Value) {
    let mut v = json!({"act": act});
    if let (Some(o), Some(m)) = (v.as_object_mut(), more.as_object_mut()) {
        o.append(m);
    }
    tell("nc", v);
}

fn color(s: &str) -> slint::Color {
    let h = s.trim_start_matches('#');
    let p = |i: usize| u8::from_str_radix(h.get(i..i + 2).unwrap_or("ff"), 16).unwrap_or(255);
    match h.len() {
        8 => slint::Color::from_argb_u8(p(0), p(2), p(4), p(6)),
        6 => slint::Color::from_rgb_u8(p(0), p(2), p(4)),
        _ => slint::Color::from_rgb_u8(0x0a, 0x84, 0xff),
    }
}

/// An alert's glyph: the island sends a Phosphor name ("usb"); one already a
/// glyph (a single private-use character) is kept
pub(crate) fn alert_glyph(name: &str) -> String {
    let mut c = name.chars();
    match (c.next(), c.next()) {
        (Some(ch), None) if ('\u{e000}'..='\u{f8ff}').contains(&ch) => name.to_string(),
        _ => crate::icons::glyph_or(name, "bell").to_string(),
    }
}

/// A card's act, for the timing log (its kind and id only)
fn act_name(c: &C) -> Option<String> {
    Some(match c {
        C::Activated(i) => format!("activate {i}"),
        C::Close(i) => format!("close {i}"),
        C::ClearGroup(_) => "clear-group".into(),
        C::Action(i, _) => format!("action {i}"),
        C::ClearAll => "clear-all".into(),
        C::Toggle(_) => "toggle-group".into(),
        _ => return None,
    })
}

fn escape(t: &str) -> String {
    t.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn s(v: &Value) -> String {
    v.as_str().unwrap_or_default().to_string()
}

/// Material's big time (TimeFormat.hmShort: Qt's "h:mm" without AM/PM
/// reads the hours 0-23, so it's never a 12-hour clock)
fn clock_short(h24: bool) -> String {
    crate::util::now(if h24 { "%H:%M" } else { "%-H:%M" })
}

impl Notifications {
    pub(super) fn nc_command(&mut self, st: &mut State, cmd: &Command) {
        match cmd.cmd.as_str() {
            "open" => self.nc_open_on(st, cmd.screen.clone()),
            "close" => self.nc_close(st, false),
            _ => {}
        }
    }

    /// The Notification Center's window, made ahead and left unmapped (Part::prewarm)
    pub(crate) fn nc_prewarm(&mut self, st: &mut State) {
        let Some(out) = st.likely_output() else { return };
        if self.nc.win.is_none() && self.nc_window(st, &out) {
            if let Some(w) = &self.nc.win {
                w.surface.hide();
            }
        }
    }

    fn nc_window(&mut self, st: &mut State, output: &str) -> bool {
        if self.nc.win.as_ref().is_some_and(|w| w.output != output) {
            let w = self.nc.win.take().unwrap();
            Glass::clear_now(&w.output, NS);
            st.drop_surface(&w.surface);
        }
        if self.nc.win.is_some() {
            return true;
        }
        let Some(wl_output) = st.output_named(output) else { return false };
        let surface = st.prepare_surface(Spec {
            layer: Layer::Overlay,
            namespace: NS,
            output: Some((&wl_output, output)),
            anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
            size: (0, 0),
            exclusive: -1,
            keyboard: KeyboardInteractivity::None,
            margin_left: 0,
        });
        let ui = ui::Center::new().expect("Center");
        st.shell.apply_theme(&ui.global::<ui::Theme>());
        let to = st.shell.sender(NAME);
        macro_rules! on {
            ($set:ident, |$($a:ident),*| $m:expr) => {{
                let to = to.clone();
                ui.$set(move |$($a),*| to(Box::new($m)));
            }};
        }
        on!(on_dismissed, | | C::Dismissed);
        on!(on_closed, | | C::Closed);
        on!(on_row_activated, |i| C::Activated(i));
        on!(on_row_close, |i| C::Close(i));
        on!(on_alert_close, |i| C::Close(i));
        on!(on_toggle_group, |g| C::Toggle(g.to_string()));
        on!(on_clear_group, |g| C::ClearGroup(g.to_string()));
        on!(on_action, |i, k| C::Action(i, k.to_string()));
        on!(on_reply_open, |i| C::ReplyOpen(i));
        on!(on_reply_cancel, |i| C::ReplyCancel(i));
        on!(on_reply_send, |i, t| C::ReplySend(i, t.to_string()));
        on!(on_draft, |i, t| C::Draft(i, t.to_string()));
        on!(on_link, |u| C::Link(u.to_string()));
        on!(on_clear_all, | | C::ClearAll);
        on!(on_manage, | | C::Manage);
        on!(on_set_tab, |i| C::Tab(i));
        on!(on_cal_step, |n| C::CalStep(n));
        on!(on_cal_week_set, |w| C::CalWeek(w));
        on!(on_cal_pick, |k| C::CalPick(k.to_string()));
        on!(on_todo_add, |t| C::TodoAdd(t.to_string()));
        on!(on_todo_toggle, |k| C::TodoToggle(k.to_string()));
        on!(on_todo_remove, |k| C::TodoRemove(k.to_string()));
        on!(on_todo_restore, | | C::TodoRestore);
        on!(on_todo_rename, |k, t| C::TodoRename(k.to_string(), t.to_string()));
        on!(on_cal_save, |d, every| C::CalSave(d, every));
        on!(on_cal_remove, |k, every| C::CalRemove(k.to_string(), every));
        on!(on_cal_move, |k, d, every| C::CalMove(k.to_string(), d.to_string(), every));
        on!(on_cal_restore, | | C::CalRestore);
        on!(on_focus_mode_set, |i| C::FocusMode(i));
        on!(on_act, |a| C::Act(a.to_string()));
        on!(on_card, |i, y, h, w, e| C::Card(i, [y, h, w, e]));
        ui.set_outline(crate::surfaces::shapes::outline("cookie9").into());
        let (rows, days, day_items, todos, laps, targets) = (Rows::new(), Rows::new(), Rows::new(), Rows::new(), Rows::new(), Rows::new());
        ui.set_rows(rows.model());
        ui.set_days(days.model());
        ui.set_day_items(day_items.model());
        ui.set_todos(todos.model());
        ui.set_laps(laps.model());
        ui.set_cal_targets(targets.model());
        ui.show().expect("show notification center");
        let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
        // (a new window's rows say their places as they're made)
        self.nc.cards.clear();
        self.nc.win = Some(Win { output: output.into(), surface, ui, rows, days, day_items, todos, laps, targets, a11y, glass_sent: String::new() });
        true
    }

    fn nc_open_on(&mut self, st: &mut State, screen: Option<String>) {
        let output = screen.or_else(|| self.nc.win.as_ref().map(|w| w.output.clone())).or_else(|| st.outputs_now().first().map(|o| o.1.clone()));
        let Some(output) = output else { return };
        if !self.nc_window(st, &output) {
            return;
        }
        let was = self.nc_open;
        self.nc_open = true;
        if !was {
            // (each opening starts on today, as the QML widget's new copy)
            let t = today();
            self.nc.selected = t;
            self.nc.shown = (t.0, t.1);
            self.nc.groups_open.clear();
            // (not the cards' places: the rows stay made while it's closed,
            // so they don't say them again; cleared, the glass under each
            // card was gone after the first opening: 18 Nov)
            // (the QML widgets are made anew: month view, the Focus timer)
            self.nc.week = false;
            self.nc.focus_mode = 0;
            self.nc_ask_calendar();
        }
        let w = self.nc.win.as_ref().unwrap();
        w.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        w.surface.show(0, 0);
        self.nc_refresh(st);
        let ui = &self.nc.win.as_ref().unwrap().ui;
        ui.set_shown(true);
        if !was {
            ui.invoke_take_keys();
            self.nc_fine();
        }
        let tx = st.tx.clone();
        self.nc.minute.start(slint::TimerMode::Repeated, Duration::from_secs(20), move || {
            let _ = tx.send(crate::Msg::Part(NAME, Box::new(C::Minute)));
        });
        // (the banners hide under it)
        self.refresh(st);
    }

    fn nc_close(&mut self, st: &mut State, tell_shell: bool) {
        if !self.nc_open {
            return;
        }
        self.nc_open = false;
        self.nc.minute.stop();
        if let Some(w) = &self.nc.win {
            w.ui.set_shown(false);
            w.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::None);
            w.surface.layer.commit();
        }
        if tell_shell {
            nc("close", json!({}));
        }
        self.refresh(st);
    }

    fn nc_closed(&mut self, st: &mut State) {
        if self.nc_open {
            return;
        }
        if let Some(w) = &mut self.nc.win {
            w.surface.hide();
            st.shell.glass.set(&w.output, NS, &[]);
            w.glass_sent.clear();
        }
    }

    /// The stopwatch's hundredths tick only while it's shown (FocusWidget's
    /// fineClock)
    fn nc_fine(&self) {
        nc("sw.fine", json!({"on": self.nc_open && self.nc.tab == 2 && self.nc.focus_mode == 1}));
    }

    fn nc_ask_calendar(&self) {
        let p = &self.nc;
        let days = if p.week { center::week_of(p.selected) } else { center::month_grid(p.shown.0, p.shown.1) };
        let (first, last) = (days[0], days[days.len() - 1]);
        nc("calendar", json!({"from": center::key(first), "to": center::key(center::add_days(last, 1)), "day": center::key(p.selected)}));
    }

    /// A row's card
    fn nc_banner(&mut self, st: &State, e: &Entry) -> ui::Banner {
        if self.nc.groups[e.group].island {
            let i = self.nc.alerts.iter().position(|a| a.hid == e.item).unwrap_or(0);
            let a = self.nc.alerts.get(i).cloned().unwrap_or_default();
            let h24 = st.shell.cfg["appearance"]["clock24"].as_bool().unwrap_or(false);
            let pic = if a.picture.is_empty() { None } else { self.image(a.picture.trim_start_matches("file://")) };
            let kind = if a.kind.is_empty() { "Tessera".to_string() } else { a.kind.clone() };
            return ui::Banner {
                id: -(i as i32) - 1,
                app_upper: kind.to_uppercase().into(),
                app: kind.into(),
                time: model::time_label(a.at, now_ms(), |t| clock_at(t, h24)).into(),
                summary: a.title.clone().into(),
                body: if a.detail.is_empty() { slint::StyledText::default() } else { markup::styled(&escape(&a.detail)) },
                body_plain: a.detail.clone().into(),
                has_icon: pic.is_some(),
                icon: pic.unwrap_or_default(),
                picture: !a.picture.is_empty(),
                buttons: self.nc.alert_buttons.get(&format!("a{}", a.hid), if a.act.is_empty() { vec![] } else { vec![ui::NoteButton { key: "act".into(), text: a.act.clone().into() }] }),
                progress: -1.0,
                history: self.history(&format!("a{}", a.hid), &a.changes, h24),
                alert: true,
                // (the island names its icon, "battery-warning": the card
                // draws the Phosphor glyph, which the name alone left blank)
                glyph: alert_glyph(&a.glyph).into(),
                glyph_color: color(&a.color),
                ..Default::default()
            };
        }
        let n = self.store.get(e.item as u32).cloned().unwrap_or_default();
        let r = self.nc.replies.get(&n.id).map(|s| Shown { id: s.id, entered: true, leaving: false, deadline: None, replying: s.replying, sent: s.sent, error: s.error.clone(), place: None });
        self.card(st, &n, r.as_ref())
    }

    /// Everything the panel shows, from the state
    pub(super) fn nc_refresh(&mut self, st: &mut State) {
        if self.nc.win.is_none() {
            return;
        }
        let material = st.shell.look.material;
        self.nc.groups = center::groups(&self.store.list, &self.nc.alerts);
        self.nc.rows = center::entries(&self.nc.groups, &self.nc.groups_open);
        let entries = self.nc.rows.clone();
        let mut rows = Vec::new();
        for (i, e) in entries.iter().enumerate() {
            let same = i > 0 && entries[i - 1].group == e.group;
            let gap = match (i, material) {
                (0, _) => 0.0,
                (_, true) => if e.first { 12.0 } else { 2.0 },
                (_, false) => if same { 6.0 } else { 8.0 },
            };
            let app = self.nc.groups[e.group].app.clone();
            let gkey = self.nc.groups[e.group].key();
            let b = self.nc_banner(st, e);
            rows.push(ui::NcRow {
                kind: match e.kind {
                    Kind::Note => 0,
                    Kind::Alert => 1,
                    Kind::Header => 2,
                },
                group: app.into(),
                gkey: gkey.into(),
                count: e.count as i32,
                open: e.open,
                edges: e.edges as i32,
                first: e.first,
                last: e.last,
                gap,
                b,
            });
        }
        let anything = !rows.is_empty();
        // (places of rows no longer there)
        let n = rows.len() as i32;
        self.nc.cards.retain(|i, _| *i < n);
        let h24 = st.shell.cfg["appearance"]["clock24"].as_bool().unwrap_or(false);
        let today = today();
        let w = &self.nc.widgets;
        let focus_text = s(&w["focus"]);
        let p = &self.nc;
        // the calendar
        let days = if p.week { center::week_of(p.selected) } else { center::month_grid(p.shown.0, p.shown.1) };
        let marks = &p.calendar["marks"];
        let cal_days: Vec<ui::NcDay> = days
            .iter()
            .map(|&d| {
                let k = center::key(d);
                let m = &marks[&k];
                let lines: Vec<ui::NcItem> = m["lines"].as_array().map(|a| a.iter().map(item).collect()).unwrap_or_default();
                ui::NcDay {
                    label: d.2.to_string().into(),
                    key: k.as_str().into(),
                    today: d == today,
                    selected: d == p.selected,
                    in_month: p.week || d.1 == p.shown.1,
                    colors: p.day_colors.get(&k, m["colors"].as_array().map(|a| a.iter().take(3).map(|c| color(c.as_str().unwrap_or(""))).collect()).unwrap_or_else(Vec::<slint::Color>::new)),
                    todos: m["todos"].as_i64().unwrap_or(0) > 0,
                    name: center::WEEKDAYS[center::weekday(d) as usize][..3].to_string().into(),
                    lines: p.day_lines.get(&k, lines),
                    more: m["more"].as_i64().unwrap_or(0) as i32,
                    long: center::short_date(d).into(),
                }
            })
            .collect();
        let targets: Vec<ui::NcTarget> = p.calendar["targets"]
            .as_array()
            .map(|a| a.iter().map(|t| ui::NcTarget { value: s(&t["value"]).into(), label: s(&t["label"]).into() }).collect())
            .unwrap_or_else(|| vec![ui::NcTarget { value: "".into(), label: "This computer".into() }]);
        let day_items: Vec<ui::NcItem> = p.calendar["items"].as_array().map(|a| a.iter().take(8).map(item).collect()).unwrap_or_default();
        let todos: Vec<ui::NcTodo> = w["todos"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|t| ui::NcTodo { key: s(&t["key"]).into(), text: s(&t["text"]).into(), done: t["done"].as_bool().unwrap_or(false), due: s(&t["due"]).into(), overdue: t["overdue"].as_bool().unwrap_or(false) })
                    .collect()
            })
            .unwrap_or_default();
        let laps: Vec<ui::NcLap> = w["sw"]["laps"].as_array().map(|a| a.iter().map(|l| ui::NcLap { name: s(&l["name"]).into(), time: s(&l["time"]).into() }).collect()).unwrap_or_default();
        let (track, progress) = center::ring(w["timer"]["progress"].as_f64().unwrap_or(0.0), material);
        let completed = w["timer"]["completed"].as_i64().unwrap_or(0);
        let win = self.nc.win.as_ref().unwrap();
        let ui = &win.ui;
        // (by card: a header by its group, a card by its note or alert)
        // A card that left (dismissed, an action done, Clear all), came or
        // changed: the next frame is drawn and damaged whole. Its partial
        // damage left the laptop's screen as it was (18 Nov: X, Clear all and
        // the actions acted, the list emptied, the cards stayed drawn)
        if win.rows.set_keyed(rows, |r| format!("{}:{}:{}", r.kind, r.gkey, if r.kind == 2 { 0 } else { r.b.id })) {
            win.surface.redraw_all();
        }
        ui.set_anything(anything);
        ui.set_bare(bare(&st.shell.cfg, material, st.shell.look.transparency));
        ui.set_glass_plugin(st.shell.plugin);
        ui.set_focus_text(focus_text.into());
        ui.set_clock(clock_short(h24).into());
        ui.set_date(center::long_date(today).into());
        ui.set_tab(p.tab);
        ui.set_cal_title(center::title(p.shown, if p.week { Some(&days) } else { None }).into());
        ui.set_cal_week(p.week);
        win.days.set(cal_days);
        ui.set_day_title(center::day_title(p.selected, today).into());
        win.day_items.set(day_items);
        ui.set_every_day(s(&p.calendar["everyDay"]).into());
        ui.set_day_key(center::key(p.selected).into());
        ui.set_day_short(center::short_date(p.selected).into());
        win.targets.set(targets);
        ui.set_cal_target(s(&p.calendar["target"]).into());
        ui.set_cal_target_label(p.calendar["targetLabel"].as_str().unwrap_or("This computer").into());
        ui.set_cal_default_alert(p.calendar["defaultAlert"].as_i64().unwrap_or(10) as i32);
        ui.set_cal_h24(h24);
        ui.set_cal_undo(s(&p.calendar["undo"]).into());
        win.todos.set_keyed(todos, |t| t.key.clone());
        ui.set_todo_undo(s(&w["undo"]).into());
        ui.set_focus_mode(p.focus_mode);
        ui.set_remaining(s(&w["timer"]["remaining"]).into());
        ui.set_focus_phase(w["timer"]["phase"].as_str().unwrap_or("focus") == "focus");
        ui.set_running(w["timer"]["running"].as_bool().unwrap_or(false));
        ui.set_sessions(format!("{completed} {} today", if completed == 1 { "session" } else { "sessions" }).into());
        ui.set_ring_track(track.into());
        ui.set_ring_progress(progress.into());
        ui.set_sw(s(&w["sw"]["elapsed"]).into());
        ui.set_sw_running(w["sw"]["running"].as_bool().unwrap_or(false));
        win.laps.set(laps);
        // (an accent that doesn't read on white: near-black, as macOS)
        let accent = ui.global::<ui::Theme>().get_accent();
        ui.set_accent_ink(if theme::contrast_ratio(accent, slint::Color::from_rgb_u8(255, 255, 255)) >= 3.0 { accent } else { slint::Color::from_rgb_u8(0x1d, 0x1d, 0x1f) });
    }

    /// What the shell said for the panel ({"do":"widgets"|"calendar",…})
    pub(super) fn nc_shell(&mut self, st: &mut State, v: &Value) {
        match v["do"].as_str().unwrap_or("") {
            "widgets" => {
                let alerts: Vec<Alert> = serde_json::from_value(v["alerts"].clone()).unwrap_or_default();
                if let Some(t) = v["tab"].as_i64() {
                    self.nc.tab = t as i32;
                }
                self.nc.alerts = alerts;
                self.nc.widgets = v.clone();
            }
            "calendar" => self.nc.calendar = v.clone(),
            // (a write the host couldn't do: the editor again, saying why)
            "calendarError" => {
                if let Some(w) = &self.nc.win {
                    w.ui.invoke_cal_fail(s(&v["text"]).into());
                }
                return;
            }
            _ => return,
        }
        self.nc_refresh(st);
    }

    /// "Sent" is over: Reply shows again; replies to gone notes forgotten
    pub(super) fn nc_sent_done(&mut self, id: u32) {
        if self.nc.replies.get(&id).is_some_and(|r| !r.replying) {
            self.nc.replies.remove(&id);
        }
        let store = &self.store;
        self.nc.replies.retain(|k, _| store.get(*k).is_some());
    }

    pub(super) fn nc_message(&mut self, st: &mut State, c: C) {
        // (the timing log, TESSERA_WAKES or $XDG_RUNTIME_DIR/tessera-wakes:
        // which act a card's click made, never what the card says)
        if crate::wayland::wakes_on() {
            if let Some(what) = act_name(&c) {
                eprintln!("nc act {what} at {}", crate::util::now_ms());
            }
        }
        let alert = |i: i32| if i < 0 { self.nc.alerts.get((-i - 1) as usize).map(|a| a.hid) } else { None };
        match c {
            C::Dismissed => self.nc_close(st, true),
            C::Closed => self.nc_closed(st),
            C::Activated(i) => {
                if i >= 0 {
                    let id = i as u32;
                    if self.store.get(id).is_some_and(|n| n.default_action().is_some()) {
                        if let Some(b) = super::notify::bus() {
                            b.action(id, "default");
                        }
                    }
                    self.dismiss(st, id, super::notify::DISMISSED);
                }
            }
            C::Close(i) => match alert(i) {
                Some(hid) => nc("island.forget", json!({"hid": hid})),
                None if i >= 0 => self.dismiss(st, i as u32, super::notify::DISMISSED),
                None => {}
            },
            C::Toggle(key) => {
                if !self.nc.groups_open.remove(&key) {
                    self.nc.groups_open.insert(key);
                }
                self.nc_refresh(st);
            }
            C::ClearGroup(g) => {
                let Some(group) = self.nc.groups.iter().find(|x| x.key() == g).cloned() else { return };
                self.nc.groups_open.remove(&group.key());
                if group.island {
                    nc("island.clear", json!({}));
                } else {
                    for id in group.items {
                        self.dismiss(st, id as u32, super::notify::DISMISSED);
                    }
                }
            }
            C::Action(i, k) => match alert(i) {
                Some(hid) => nc("alert.act", json!({"hid": hid})),
                None if i >= 0 => self.invoke(st, i as u32, &k),
                None => {}
            },
            C::ReplyOpen(i) if i >= 0 => {
                let id = i as u32;
                self.nc.replies.insert(id, Shown { id, entered: true, leaving: false, deadline: None, replying: true, sent: false, error: String::new(), place: None });
                self.nc_refresh(st);
            }
            C::ReplyCancel(i) => {
                self.nc.replies.remove(&(i.max(0) as u32));
                self.nc_refresh(st);
                // (the panel's keys again: Escape closes it)
                if let Some(w) = &self.nc.win {
                    w.ui.invoke_take_keys();
                }
            }
            C::ReplySend(i, t) if i >= 0 => {
                let id = i as u32;
                let err = self.reply(st, id, &t);
                let ok = err.is_empty();
                if let Some(r) = self.nc.replies.get_mut(&id) {
                    r.replying = !ok;
                    r.sent = ok;
                    r.error = err;
                }
                self.nc_refresh(st);
                if ok {
                    // ("Sent" for 2.5 s, then Reply again: NotificationReply)
                    self.send_later(st, 2500, super::M::SentDone(id));
                    if let Some(w) = &self.nc.win {
                        w.ui.invoke_take_keys();
                    }
                }
            }
            C::Draft(i, t) if i >= 0 => {
                if t.is_empty() {
                    self.drafts.remove(&(i as u32));
                } else {
                    self.drafts.insert(i as u32, t);
                }
            }
            C::Link(u) => super::open_link(&u),
            C::ClearAll => {
                self.clear_all(st);
                nc("island.clear", json!({}));
            }
            C::Manage => {
                self.nc_close(st, true);
                nc("manage", json!({}));
            }
            C::Tab(i) => {
                self.nc.tab = i;
                nc("tab", json!({"tab": i}));
                self.nc_fine();
                self.nc_refresh(st);
            }
            C::CalStep(n) => {
                let p = &mut self.nc;
                if p.week {
                    p.selected = center::add_days(p.selected, 7 * n as i64);
                    p.shown = (p.selected.0, p.selected.1);
                } else {
                    p.shown = center::add_months(p.shown, n);
                }
                self.nc_ask_calendar();
                self.nc_refresh(st);
            }
            C::CalWeek(w) => {
                let p = &mut self.nc;
                p.week = w;
                if !w {
                    p.shown = (p.selected.0, p.selected.1);
                }
                self.nc_ask_calendar();
                self.nc_refresh(st);
            }
            C::CalPick(k) => {
                let mut it = k.split('-').filter_map(|x| x.parse::<i64>().ok());
                if let (Some(y), Some(m), Some(d)) = (it.next(), it.next(), it.next()) {
                    self.nc.selected = (y as i32, m as u32, d as u32);
                    self.nc_ask_calendar();
                    self.nc_refresh(st);
                }
            }
            // (TodoWidget: blanks aren't added)
            C::TodoAdd(t) if t.trim().is_empty() => {}
            C::TodoAdd(t) => nc("todo.add", json!({"text": t.trim()})),
            C::TodoToggle(k) => nc("todo.toggle", json!({"key": k})),
            C::TodoRemove(k) => nc("todo.remove", json!({"key": k})),
            C::TodoRestore => nc("todo.restore", json!({})),
            C::TodoRename(k, t) => nc("todo.rename", json!({"key": k, "text": t})),
            C::CalSave(d, every) => self.nc_cal_save(st, d, every),
            C::CalRemove(key, every) => {
                if self.nc_recurring(&key) && every < 0 {
                    if let Some(w) = &self.nc.win {
                        w.ui.invoke_cal_ask(2, "".into());
                    }
                    return;
                }
                nc("cal.remove", json!({"key": key, "every": every == 1}));
                if let Some(w) = &self.nc.win {
                    w.ui.invoke_cal_close();
                }
            }
            C::CalMove(key, day, every) => {
                let Some(to) = center::parse_key(&day) else { return };
                if self.nc_recurring(&key) && every < 0 {
                    // (this one, or every one? asked in its editor)
                    if let (Some(w), Some(it)) = (&self.nc.win, self.nc_item(&key)) {
                        w.ui.invoke_cal_edit(item(&it));
                        w.ui.invoke_cal_ask(3, day.as_str().into());
                    }
                    return;
                }
                nc("cal.move", json!({"key": key, "day": day, "every": every == 1}));
                if let Some(w) = &self.nc.win {
                    w.ui.invoke_cal_close();
                }
                self.nc.selected = to;
                self.nc_ask_calendar();
                self.nc_refresh(st);
            }
            C::CalRestore => nc("cal.restore", json!({})),
            C::FocusMode(i) => {
                self.nc.focus_mode = i;
                self.nc_fine();
                self.nc_refresh(st);
            }
            C::Act(a) => nc(&a, json!({})),
            C::Card(i, g) => {
                self.nc.cards.insert(i, g);
            }
            C::Minute => self.nc_refresh(st),
            _ => {}
        }
    }

    /// An item of the chosen day, as the host sent it
    fn nc_item(&self, key: &str) -> Option<Value> {
        self.nc.calendar["items"].as_array()?.iter().find(|i| i["key"].as_str() == Some(key)).cloned()
    }

    fn nc_recurring(&self, key: &str) -> bool {
        self.nc_item(key).is_some_and(|i| i["recurring"].as_bool().unwrap_or(false))
    }

    /// The editor's Save (CalendarWidget editor.save): a title, times it can
    /// read; a repeating event asks "this or all?" first
    fn nc_cal_save(&mut self, st: &mut State, d: ui::NcDraft, every: i32) {
        let Some(w) = &self.nc.win else { return };
        let fail = |t: &str| w.ui.set_cal_error(t.into());
        let name = d.title.trim().to_string();
        if name.is_empty() {
            return fail("Give it a title.");
        }
        let Some(day) = center::parse_key(&d.day) else { return };
        let (start, end) = if d.todo {
            let blank = d.start.trim().is_empty();
            let t = if blank { -1 } else { center::minutes(&d.start) };
            if !blank && t < 0 {
                return fail("Use a time like 9:30 or 5pm, or leave it empty.");
            }
            (t, -1)
        } else if d.all_day {
            (0, 0)
        } else {
            let (a, b) = (center::minutes(&d.start), center::minutes(&d.end));
            if a < 0 || b < 0 {
                return fail("Use times like 9:30 or 5pm.");
            }
            (a, b.max(a))
        };
        if !d.todo && !d.key.is_empty() && self.nc_recurring(&d.key) && every < 0 {
            w.ui.invoke_cal_ask(1, "".into());
            return;
        }
        w.ui.set_cal_error("".into());
        nc(
            "cal.save",
            json!({"key": d.key.as_str(), "todo": d.todo, "day": d.day.as_str(), "title": name, "allDay": d.all_day, "start": start, "end": end,
                   "alert": d.alert, "notes": d.notes.as_str(), "location": d.location.as_str(), "target": d.target.as_str(), "every": every == 1}),
        );
        w.ui.invoke_cal_close();
        // (the day it's on is the chosen one)
        self.nc.selected = day;
        if !self.nc.week {
            self.nc.shown = (day.0, day.1);
        }
        self.nc_ask_calendar();
        self.nc_refresh(st);
    }

    /// The glass under it (Glass): the panel's, or (no panel) each card's
    /// and the widgets' card, following the list's scroll; accessibility
    pub(super) fn nc_after_frame(&mut self, st: &mut State) {
        let kbd = st.keyboard_surface();
        let Some(w) = &mut self.nc.win else { return };
        let f = kbd.as_ref().is_some_and(|k| Rc::ptr_eq(k, &w.surface));
        if let Some(a) = w.a11y.as_mut() {
            a.update(false, f);
        }
        if !w.surface.mapped.get() || !st.shell.plugin || st.shell.look.material {
            return;
        }
        let ui = &w.ui;
        let look = &st.shell.look;
        let o = ui.get_body_opacity();
        let (bx, by, bw, bh) = (ui.get_body_x(), ui.get_body_y(), ui.get_body_width(), ui.get_body_height());
        let mut shapes = Vec::new();
        if o > 0.002 {
            if !ui.get_bare() {
                shapes.push(theme::glass_shape(look, bx, by, bw, bh, ui.get_radius(), o, 1.0));
            } else {
                let pad = 16.0;
                let x = bx + pad;
                if ui.get_anything() {
                    let (top, scroll, lh) = (ui.get_list_top(), ui.get_list_scroll(), ui.get_list_height());
                    let rows = ui.get_rows();
                    for i in 0..rows.row_count() {
                        let Some(r) = rows.row_data(i) else { continue };
                        if r.kind == 2 {
                            continue;
                        }
                        let Some(&[y, h, cw, _]) = self.nc.cards.get(&(i as i32)) else { continue };
                        // (8 px of room above the list for the close badges)
                        let v = 8.0 + y - scroll;
                        let fade = ((v + 8.0) / 8.0).min((lh + 8.0 - (v + h)) / 8.0).clamp(0.0, 1.0);
                        if fade <= 0.0 {
                            continue;
                        }
                        shapes.push(theme::glass_shape(look, x, top + v, cw, h, 18.0, o * fade, 1.0));
                        for k in 0..r.edges.max(0) as usize {
                            let inset = (k as f32 + 1.0) * 8.0;
                            shapes.push(theme::glass_shape(look, x + inset, top + v + h - 4.0 + k as f32 * 8.0, cw - 2.0 * inset, 12.0, 6.0, o * fade * (0.75 - 0.2 * k as f32), 1.0));
                        }
                    }
                } else {
                    shapes.push(theme::glass_shape(look, x, by + pad + 28.0 + 10.0, bw - 2.0 * pad, 84.0, 18.0, o, 1.0));
                }
                let (wy, ww, wh) = (ui.get_widgets_y(), bw - 2.0 * pad, ui.get_widgets_h());
                if wh > 0.0 {
                    shapes.push(theme::glass_shape(look, x, by + wy, ww, wh, 36f32.min(ww.min(wh) / 2.0), o, 1.0));
                }
            }
        }
        let key = tessera_core::theme::glass_key(&shapes);
        if key != w.glass_sent {
            st.shell.glass.set(&w.output, NS, &shapes);
            w.glass_sent = key;
        }
    }

    pub(super) fn nc_keyboard_left(&mut self, st: &mut State, s: &Rc<Surface>) {
        if self.nc_open && self.nc.win.as_ref().is_some_and(|w| Rc::ptr_eq(&w.surface, s)) {
            self.nc_close(st, true);
        }
    }

    pub(super) fn nc_output_removed(&mut self, st: &mut State, name: &str) {
        if self.nc.win.as_ref().is_some_and(|w| w.output == name) {
            let was = self.nc_open;
            self.nc_close(st, was);
            let w = self.nc.win.take().unwrap();
            st.drop_surface(&w.surface);
        }
    }

    pub(super) fn nc_owns(&self, s: &Rc<Surface>) -> bool {
        self.nc.win.as_ref().is_some_and(|w| Rc::ptr_eq(&w.surface, s))
    }

    pub(super) fn nc_theme(&mut self, st: &mut State) {
        if let Some(w) = &mut self.nc.win {
            st.shell.apply_theme(&w.ui.global::<ui::Theme>());
            st.shell.glass.set(&w.output, NS, &[]);
            w.glass_sent.clear();
        }
        // (the cards' places stay: a row that moves says its new one)
        self.nc_refresh(st);
    }

    pub(super) fn nc_quit(&self) {
        if let Some(w) = &self.nc.win {
            Glass::clear_now(&w.output, NS);
        }
    }
}

/// A calendar line from the shell: an event or a to-do
fn item(v: &Value) -> ui::NcItem {
    let b = |k: &str| v[k].as_bool().unwrap_or(false);
    ui::NcItem {
        event: v["kind"].as_str() != Some("todo"),
        title: s(&v["title"]).into(),
        sub: s(&v["sub"]).into(),
        color: color(v["color"].as_str().unwrap_or("#0a84ff")),
        done: b("done"),
        key: s(&v["key"]).into(),
        day: s(&v["day"]).into(),
        all_day: b("allDay"),
        start: s(&v["start"]).into(),
        end: s(&v["end"]).into(),
        location: s(&v["location"]).into(),
        notes: s(&v["notes"]).into(),
        alert: v["alert"].as_i64().unwrap_or(10) as i32,
        editable: b("editable"),
        movable: b("movable"),
        recurring: b("recurring"),
        account: b("account"),
        info: s(&v["info"]).into(),
        cal_line: s(&v["calLine"]).into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn its_own_panel_only_when_solid_or_asked() {
        assert!(bare(&json!({}), false, 0.0));
        assert!(!bare(&json!({"notificationCenter": {"background": true}}), false, 0.0));
        assert!(!bare(&json!({"appearance": {"glass": "solid"}}), false, 0.0));
        assert!(!bare(&json!({"appearance": {"reduceTransparency": true}}), false, 0.0));
        assert!(!bare(&json!({"appearance": {"glass": "custom", "glassSolid": 1.0}}), false, 0.0));
        // Material: bare once its panels see through
        assert!(!bare(&json!({}), true, 0.0));
        assert!(bare(&json!({}), true, 0.5));
    }

    /// The island sends an icon's name; the card draws its glyph (the name
    /// itself drew as an empty badge)
    #[test]
    fn alert_glyphs_from_names() {
        let usb = alert_glyph("usb");
        assert_eq!(usb, crate::icons::glyph("usb"));
        assert_eq!(usb.chars().count(), 1);
        assert!(('\u{e000}'..='\u{f8ff}').contains(&usb.chars().next().unwrap()));
        // already a glyph: kept; unknown or empty: the bell
        assert_eq!(alert_glyph(&usb), usb);
        assert_eq!(alert_glyph("no-such-icon"), crate::icons::glyph("bell"));
        assert_eq!(alert_glyph(""), crate::icons::glyph("bell"));
    }

    /// The timing log names a card's act by kind and id, never its text
    #[test]
    fn card_acts_in_the_log() {
        assert_eq!(act_name(&C::Close(3)).as_deref(), Some("close 3"));
        assert_eq!(act_name(&C::Action(-2, "secret-key".into())).as_deref(), Some("action -2"));
        assert_eq!(act_name(&C::ClearAll).as_deref(), Some("clear-all"));
        assert_eq!(act_name(&C::ClearGroup("Ann's app".into())).as_deref(), Some("clear-group"));
        assert!(act_name(&C::Minute).is_none());
    }

    #[test]
    fn colours_from_the_shell() {
        assert_eq!(color("#ff0000"), slint::Color::from_rgb_u8(255, 0, 0));
        assert_eq!(color("#80ff0000"), slint::Color::from_argb_u8(0x80, 255, 0, 0));
        assert_eq!(color(""), slint::Color::from_rgb_u8(0x0a, 0x84, 0xff));
        assert_eq!(escape("a<b>&"), "a&lt;b&gt;&amp;");
    }
}

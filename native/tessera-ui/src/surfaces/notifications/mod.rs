//! Notifications (rust.surfaces: notifications): the server (services/notify.rs
//! owns org.freedesktop.Notifications), the list, and the banners in the
//! top-right corner of the focused screen, both looks. Compare
//! shell/services/Notifications.qml (the server and its rules),
//! shell/modules/notifications/NotificationPopups.qml (the stack) and its
//! cards (NotificationCard.qml, MaterialNotificationCard.qml).
//!
//! Banners: newest in front, two peeking behind; the "N more" pill fans them
//! out into a list ("Clear all", "Show less"). Each leaves after the app's
//! timeout or 5 s (held while the pointer is on them, while fanned out, while
//! a reply is written; critical ones and ringing calls stay); closing a banner
//! leaves the notification in the list, clicking it runs the app's default
//! action and dismisses it. A Focus mode (focus.json) holds banners back
//! except critical ones and its allowed apps. Messaging apps' Reply opens a
//! composer in the card (the banner takes the keyboard meanwhile).
//!
//! Notification Center is drawn here too (panel.rs, center.rs). The QML shell
//! while this serves (the hand-over, services/notify.rs): its lock screen,
//! menu bar bell and island still read Notifications.qml, which mirrors this
//! part's list instead of running its own server:
//!   - out, on stdout (RustUi.qml passes it on), whenever the list changes,
//!     once the shell said hello:
//!       {"src":"ui","kind":"notifications","list":[Note…]}
//!     each Note with Quickshell's names (model.rs) and `route`, where it went
//!     as it arrived: "banner" (drawn here; the shell plays its sound),
//!     "island" (the shell posts the island's card), "call" (the island's
//!     call card) or "quiet" (a Focus mode held it);
//!   - in, a JSON line on stdin each (main.rs hands them to this part):
//!       {"to":"notifications","do":"hello"}            the shell listens
//!       {"to":"notifications","do":"dismiss","id":N}
//!       {"to":"notifications","do":"invoke","id":N,"action":"key"}
//!       {"to":"notifications","do":"reply","id":N,"text":"…"}
//!       {"to":"notifications","do":"clear"}
//!       {"to":"notifications","do":"banner","id":N}    the island couldn't take it
//!       {"to":"notifications","do":"widgets"|"calendar",…}  Notification
//!                    Center's widgets (panel.rs)
//!       {"to":"notifications","do":"shell","covered":bool,"osd":bool}
//!                    Notification Center or Control Center is open on the
//!                    banners' screen (they hide), the OSD shows (they move down)
//! The list is also kept in $XDG_STATE_HOME/tessera/notifications.json for
//! the next run on the same bus (model.rs, History).
//!
//! Not here yet (the QML does it): the island's duplicate checks (a
//! notification about what the island just said, or from the playing app,
//! stays quiet), "Only important" over a fullscreen app, sounds when no QML
//! shell listens, Material's wavy progress (a straight track here), and
//! Glass's own shadow around each card (the plugin's glass only).

pub mod center;
pub mod markup;
pub mod model;
mod panel;

use crate::services::hypr::{Glass, Snapshot};
use crate::services::notify::{self, Event};
use crate::shell::Part;
use crate::theme;
use crate::ui;
use crate::wayland::{Spec, State, Surface};
use model::{Note, Route, Store};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

pub const NS: &str = "tessera:notifications";
const NAME: &str = "notifications";
const MARGIN: f32 = 24.0; // room round the cards in the window
const CARD_W: f32 = 360.0;

/// Its own messages (from the cards, timers and the shell's stdin lines)
enum M {
    Expire,
    Clock,
    Enter,
    Gone(u32),
    Publish,
    Measured(u32, f32),
    Moved(u32, [f32; 5]),
    Activated(u32),
    Close(u32),
    Action(u32, String),
    ReplyOpen(u32),
    ReplyCancel(u32),
    ReplySend(u32, String),
    Draft(u32, String),
    SentDone(u32),
    DismissAfterReply(u32),
    Link(String),
    Pill(String),
}

/// A banner on screen
struct Shown {
    id: u32,
    entered: bool,
    leaving: bool,
    /// running down: when it leaves
    deadline: Option<Instant>,
    replying: bool,
    sent: bool,
    error: String,
}

struct Win {
    output: String,
    surface: Rc<Surface>,
    ui: ui::Banners,
    model: Rc<VecModel<ui::Banner>>,
    a11y: Option<crate::a11y::A11y>,
    size: (u32, u32),
    keyboard: bool,
    shown: bool,
    glass_sent: String,
    region: String,
}

#[derive(Default)]
pub struct Notifications {
    store: Store,
    restored: bool,
    owner: bool,
    /// the QML shell listens (it said hello)
    shell: bool,
    covered: bool,
    osd: bool,
    /// on screen, the front (newest) first
    banners: Vec<Shown>,
    expanded: bool,
    hovered: bool,
    heights: HashMap<u32, f32>,
    moved: HashMap<u32, [f32; 5]>,
    drafts: HashMap<u32, String>,
    icons: HashMap<String, Option<String>>,
    images: HashMap<String, slint::Image>,
    win: Option<Win>,
    focused: String,
    expire: slint::Timer,
    clock: slint::Timer,
    enter: slint::Timer,
    publish: slint::Timer,
    save: slint::Timer,
    // Notification Center (panel.rs)
    nc: panel::Panel,
    nc_open: bool,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

/// Focus modes' file (Focus.qml: next to config.json)
fn focus_json() -> serde_json::Value {
    let p = theme::config_path().with_file_name("focus.json");
    std::fs::read_to_string(p).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(serde_json::Value::Null)
}

/// A time of day as the shell writes it (TimeFormat.hm)
fn clock_at(ms: i64, h24: bool) -> String {
    let fmt = std::ffi::CString::new(if h24 { "%H:%M" } else { "%-I:%M %p" }).unwrap();
    let mut buf = [0u8; 64];
    unsafe {
        let t: libc::time_t = (ms / 1000) as libc::time_t;
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        let n = libc::strftime(buf.as_mut_ptr() as *mut libc::c_char, buf.len(), fmt.as_ptr(), &tm);
        String::from_utf8_lossy(&buf[..n]).into_owned()
    }
}

// ── Icons: a file for a name, as Notifications.qml's iconFor ────────────────

fn data_dirs() -> Vec<String> {
    let home = std::env::var("XDG_DATA_HOME").unwrap_or_else(|_| format!("{}/.local/share", std::env::var("HOME").unwrap_or_default()));
    let dirs = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    std::iter::once(home).chain(dirs.split(':').map(str::to_string)).filter(|d| !d.is_empty()).collect()
}

/// An app's icon name from its desktop entry (by id)
fn desktop_icon(entry: &str) -> Option<String> {
    let id = entry.trim_end_matches(".desktop");
    for d in data_dirs() {
        let Ok(text) = std::fs::read_to_string(format!("{d}/applications/{id}.desktop")) else { continue };
        let mut in_entry = false;
        for l in text.lines() {
            if l.starts_with('[') {
                in_entry = l == "[Desktop Entry]";
            } else if in_entry {
                if let Some(i) = l.strip_prefix("Icon=") {
                    return Some(i.trim().to_string());
                }
            }
        }
    }
    None
}

/// The icon theme the shell uses: the one it was started with
/// (launch-shell.sh sets QS_ICON_THEME; tessera-ui inherits it), else
/// Settings' choice, else the theme the shell last indexed
fn icon_theme(cache: &std::path::Path) -> String {
    if let Some(t) = std::env::var("QS_ICON_THEME").ok().filter(|t| !t.is_empty()) {
        return t;
    }
    if let Some(t) = crate::theme::read_json()["appearance"]["iconTheme"].as_str().filter(|t| !t.is_empty()) {
        return t.to_string();
    }
    let newest = std::fs::read_dir(cache).into_iter().flatten().flatten().filter_map(|e| {
        let n = e.file_name().to_string_lossy().into_owned();
        let t = n.strip_prefix("icon-index-")?.strip_suffix(".tsv")?.to_string();
        Some((e.metadata().ok()?.modified().ok()?, t))
    });
    newest.max().map_or_else(|| "hicolor".into(), |(_, t)| t)
}

/// A theme icon's file: the shell's own index of the theme first
/// (IconLookup.qml: ~/.cache/tessera/icon-index-<theme>.tsv), then the usual
/// folders of that theme and the common ones, then pixmaps
fn theme_icon(name: &str) -> Option<String> {
    let cache = std::env::var_os("XDG_CACHE_HOME").map(std::path::PathBuf::from).unwrap_or_else(|| std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".cache"));
    let folder = crate::theme::config_path().parent().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "tessera".into());
    let cache = cache.join(folder);
    let theme = icon_theme(&cache);
    if let Ok(index) = std::fs::read_to_string(cache.join(format!("icon-index-{theme}.tsv"))) {
        for l in index.lines() {
            if let Some((n, f)) = l.split_once('\t') {
                if n == name && std::path::Path::new(f).exists() {
                    return Some(f.to_string());
                }
            }
        }
    }
    let sizes = ["scalable", "48x48", "48", "64x64", "64", "128x128", "96x96", "256x256", "32x32", "512x512"];
    for d in data_dirs() {
        for t in [theme.as_str(), "hicolor", "Papirus", "breeze", "Adwaita"] {
            for s in sizes {
                for kind in ["apps", "status", "devices", "categories", "places", "actions", "mimetypes"] {
                    for ext in ["svg", "png"] {
                        let f = format!("{d}/icons/{t}/{s}/{kind}/{name}.{ext}");
                        if std::path::Path::new(&f).exists() {
                            return Some(f);
                        }
                    }
                }
            }
        }
        for ext in ["png", "svg", "xpm"] {
            let f = format!("{d}/pixmaps/{name}.{ext}");
            if std::path::Path::new(&f).exists() {
                return Some(f);
            }
        }
    }
    None
}

/// What a card shows as its picture: (file, whether it's the sender's picture)
fn icon_source(n: &Note, lookup: &mut impl FnMut(&str) -> Option<String>) -> Option<(String, bool)> {
    let file = |s: &str| s.strip_prefix("file://").map(str::to_string).unwrap_or_else(|| s.to_string());
    if !n.image.is_empty() {
        let f = file(&n.image);
        let found = if f.starts_with('/') { Some(f) } else { lookup(&f) };
        if let Some(f) = found.or_else(|| lookup("dialog-information")) {
            return Some((f, true));
        }
    }
    let icon = if !n.app_icon.is_empty() { n.app_icon.clone() } else { desktop_icon(&n.desktop_entry).unwrap_or_default() };
    let f = file(&icon);
    if f.starts_with('/') {
        return Some((f, false));
    }
    let found = if f.is_empty() { None } else { lookup(&f) };
    found.or_else(|| lookup("dialog-information")).map(|f| (f, false))
}

// ── The stack's maths (NotificationPopups.qml) ──────────────────────────────

/// Each banner's place (front first): (y, scale, opacity) and the stack's height
pub fn stack(heights: &[f32], expanded: bool, material: bool) -> (Vec<(f32, f32, f32)>, f32) {
    let gap = if material { 8.0 } else { 10.0 };
    let n = heights.len();
    if n == 0 {
        return (Vec::new(), 0.0);
    }
    let offset = |i: usize| heights[..i].iter().map(|h| h + gap).sum::<f32>();
    let places = (0..n)
        .map(|i| {
            let y = if expanded {
                offset(i)
            } else if i == 0 {
                0.0
            } else {
                // older cards peek 8 px below the front one
                heights[0] + 8.0 * i.min(2) as f32 - heights[i]
            };
            let front = i == 0;
            let scale = if expanded || front { 1.0 } else { 1.0 - 0.05 * i.min(2) as f32 };
            let shown = expanded || i < 3;
            let fade = if !shown { 0.0 } else if front || expanded { 1.0 } else { 0.9 - 0.25 * (i as f32 - 1.0) };
            (y, scale, fade)
        })
        .collect();
    let height = if expanded { offset(n) - gap } else { heights[0] + 8.0 * (n - 1).min(2) as f32 };
    (places, height)
}

/// The pills under a stack of more than one
fn pills(count: usize, expanded: bool) -> Vec<ui::StackPill> {
    let p = |t: String, i: &str, a: &str| ui::StackPill { text: t.into(), icon: i.into(), act: a.into() };
    match (count, expanded) {
        (0..=1, _) => Vec::new(),
        (_, true) => vec![p("Clear all".into(), "", "clear"), p("Show less".into(), "\u{e13c}", "collapse")],
        (n, false) => vec![p(format!("{} more", n - 1), "\u{e136}", "expand")],
    }
}

impl Notifications {
    /// One of its messages, a while from now
    fn send_later(&self, st: &State, ms: u64, m: M) {
        let tx = st.tx.clone();
        slint::Timer::single_shot(Duration::from_millis(ms), move || {
            let _ = tx.send(crate::Msg::Part(NAME, Box::new(m)));
        });
    }

    /// The exit's length (then the banner is removed)
    fn exit_ms(st: &State) -> u64 {
        if st.shell.look.material {
            170
        } else {
            210
        }
    }

    fn arm(&self, st: &State, timer: &slint::Timer, ms: u64, make: fn() -> M) {
        let tx = st.tx.clone();
        timer.start(slint::TimerMode::SingleShot, Duration::from_millis(ms), move || {
            let _ = tx.send(crate::Msg::Part(NAME, Box::new(make())));
        });
    }

    fn material(st: &State) -> bool {
        st.shell.look.material
    }

    // ── The list, and what the shell hears of it ────────────────────────────

    /// The list changed: tell the shell (soon) and keep it (a little later)
    fn changed(&mut self, st: &State) {
        if self.shell && !self.publish.running() {
            self.arm(st, &self.publish, 40, || M::Publish);
        }
        if !self.restored {
            return; // (not before the bus's id is known: the file is kept per bus)
        }
        let json = model::history_json(&self.bus_id(), &self.store);
        // (written off the UI thread, whole, then renamed into place)
        self.save.start(slint::TimerMode::SingleShot, Duration::from_millis(400), move || {
            let json = json.clone();
            std::thread::spawn(move || {
                let p = notify::history_path();
                if let Some(dir) = p.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                let tmp = p.with_extension("json.tmp");
                if std::fs::write(&tmp, json).is_ok() {
                    let _ = std::fs::rename(&tmp, &p);
                }
            });
        });
    }

    fn bus_id(&self) -> String {
        BUS_ID.with(|b| b.borrow().clone())
    }

    fn publish(&self) {
        if !self.shell {
            return;
        }
        crate::shell::tell("notifications", serde_json::json!({"list": self.store.list}));
    }

    fn arrived(&mut self, st: &mut State, mut n: Note) {
        if self.store.has(n.id) {
            // an update (same id): in place, where it is (a banner showing changes)
            self.store.add(n);
            self.changed(st);
            self.refresh(st);
            return;
        }
        let route = model::route(&n, &st.shell.cfg, &focus_json(), now_ms(), self.shell);
        n.route = route.name().into();
        let id = n.id;
        if let model::Added::New(gone) = self.store.add(n) {
            for g in gone {
                self.forget(g, notify::EXPIRED);
                self.send_later(st, Self::exit_ms(st), M::Gone(g));
            }
        }
        if route == Route::Banner {
            self.show_banner(st, id);
        }
        self.changed(st);
        self.refresh(st);
    }

    /// It's gone from the list (and the app hears why)
    fn forget(&mut self, id: u32, reason: u32) {
        if let Some(b) = notify::bus() {
            b.closed(id, reason);
        }
        self.drafts.remove(&id);
        for s in self.banners.iter_mut().filter(|s| s.id == id) {
            s.leaving = true;
        }
    }

    fn dismiss(&mut self, st: &mut State, id: u32, reason: u32) {
        if self.store.remove(id).is_some() {
            self.forget(id, reason);
            self.send_later(st, Self::exit_ms(st), M::Gone(id));
            self.changed(st);
            self.refresh(st);
        }
    }

    fn clear_all(&mut self, st: &mut State) {
        let ids: Vec<u32> = self.store.list.iter().map(|n| n.id).collect();
        for id in ids {
            self.store.remove(id);
            self.forget(id, notify::DISMISSED);
        }
        for s in &mut self.banners {
            s.leaving = true;
        }
        let gone: Vec<u32> = self.banners.iter().map(|s| s.id).collect();
        for id in gone {
            self.send_later(st, Self::exit_ms(st), M::Gone(id));
        }
        self.expanded = false;
        self.changed(st);
        self.refresh(st);
    }

    /// The app's action (the click is "default"); one that isn't resident is
    /// done with then, as Quickshell's invoke
    fn invoke(&mut self, st: &mut State, id: u32, key: &str) {
        let Some(n) = self.store.get(id).cloned() else { return };
        if let Some(b) = notify::bus() {
            b.action(id, key);
        }
        if !n.resident {
            self.dismiss(st, id, notify::DISMISSED);
        } else if model::is_call(&n) {
            // answered or declined: the call's banner goes (the app may keep it)
            self.leave(st, id);
        }
    }

    // ── Banners ─────────────────────────────────────────────────────────────

    fn show_banner(&mut self, st: &State, id: u32) {
        if self.banners.iter().any(|s| s.id == id && !s.leaving) {
            return;
        }
        self.banners.retain(|s| s.id != id);
        self.banners.insert(0, Shown { id, entered: false, leaving: false, deadline: None, replying: false, sent: false, error: String::new() });
        // the entrance starts once the card is there to animate
        self.arm(st, &self.enter, 40, || M::Enter);
    }

    fn leave(&mut self, st: &mut State, id: u32) {
        let mut any = false;
        for s in self.banners.iter_mut().filter(|s| s.id == id && !s.leaving) {
            s.leaving = true;
            s.deadline = None;
            any = true;
        }
        if any {
            self.send_later(st, Self::exit_ms(st), M::Gone(id));
            self.refresh(st);
        }
    }

    /// Timers: running for each banner that may leave on its own now, from
    /// its full time again (as the QML Timer, restarted when it may run)
    fn timers(&mut self, st: &State) {
        let held = self.hovered || self.expanded;
        let now = Instant::now();
        let mut next: Option<Instant> = None;
        for s in &mut self.banners {
            let Some(n) = self.store.get(s.id) else { continue };
            let ms = model::banner_ms(n);
            let runs = ms.is_some() && !held && !s.replying && !s.sent && !s.leaving;
            if !runs {
                s.deadline = None;
                continue;
            }
            let d = *s.deadline.get_or_insert(now + Duration::from_millis(ms.unwrap()));
            next = Some(next.map_or(d, |x: Instant| x.min(d)));
        }
        match next {
            Some(d) => {
                let wait = d.saturating_duration_since(now).max(Duration::from_millis(1));
                let tx = st.tx.clone();
                self.expire.start(slint::TimerMode::SingleShot, wait, move || {
                    let _ = tx.send(crate::Msg::Part(NAME, Box::new(M::Expire)));
                });
            }
            None => self.expire.stop(),
        }
    }

    fn expired(&mut self, st: &mut State) {
        let now = Instant::now();
        let due: Vec<u32> = self.banners.iter().filter(|s| s.deadline.is_some_and(|d| d <= now)).map(|s| s.id).collect();
        for id in due {
            self.leave(st, id);
        }
        self.timers(st);
    }

    fn image(&mut self, path: &str) -> Option<slint::Image> {
        if let Some(i) = self.images.get(path) {
            return Some(i.clone());
        }
        let img = slint::Image::load_from_path(std::path::Path::new(path)).ok()?;
        if self.images.len() > 64 {
            self.images.clear();
        }
        self.images.insert(path.to_string(), img.clone());
        Some(img)
    }

    fn row(&mut self, st: &State, s: &Shown, place: (f32, f32, f32), index: usize) -> ui::Banner {
        let n = self.store.get(s.id).cloned().unwrap_or_default();
        ui::Banner {
            index: index as i32,
            ty: place.0,
            tscale: place.1,
            tfade: place.2,
            front: index == 0,
            entered: s.entered,
            leaving: s.leaving,
            ..self.card(st, &n, Some(s))
        }
    }

    /// A notification's card (a banner's, or Notification Center's), with
    /// its composer's state
    fn card(&mut self, st: &State, n: &Note, s: Option<&Shown>) -> ui::Banner {
        let n = n.clone();
        let icons = &mut self.icons;
        let mut lookup = |name: &str| icons.entry(name.to_string()).or_insert_with(|| theme_icon(name)).clone();
        let src = icon_source(&n, &mut lookup);
        let (icon, picture) = match src {
            Some((f, pic)) => (self.image(&f), pic),
            None => (None, false),
        };
        let app = if n.app_name.is_empty() { "Notification".to_string() } else { n.app_name.clone() };
        let h24 = st.shell.cfg["appearance"]["clock24"].as_bool().unwrap_or(false);
        let plain = markup::plain(&n.body);
        ui::Banner {
            id: n.id as i32,
            app_upper: app.to_uppercase().into(),
            app: app.into(),
            time: model::time_label(n.at, now_ms(), |t| clock_at(t, h24)).into(),
            summary: n.summary.clone().into(),
            body: if plain.is_empty() { slint::StyledText::default() } else { markup::styled(&n.body) },
            body_plain: plain.into(),
            has_icon: icon.is_some(),
            icon: icon.unwrap_or_default(),
            picture,
            buttons: ModelRc::new(VecModel::from(n.buttons().iter().map(|a| ui::NoteButton { key: a.id.clone().into(), text: a.text.clone().into() }).collect::<Vec<_>>())),
            can_reply: n.has_inline_reply,
            placeholder: n.inline_reply_placeholder.clone().into(),
            replying: s.is_some_and(|s| s.replying),
            sent: s.is_some_and(|s| s.sent),
            error: s.map(|s| s.error.clone()).unwrap_or_default().into(),
            draft: self.drafts.get(&n.id).cloned().unwrap_or_default().into(),
            progress: if n.value >= 0 { n.value as f32 / 100.0 } else { -1.0 },
            ..Default::default()
        }
    }

    /// Everything on screen from the state: the stack's places, the cards,
    /// the window (made on the focused screen, sized, shown or hidden)
    fn refresh(&mut self, st: &mut State) {
        let material = Self::material(st);
        let count = self.banners.len();
        if count <= 1 {
            self.expanded = false;
        }
        let default_h = if material { 96.0 } else { 90.0 };
        let heights: Vec<f32> = self.banners.iter().map(|s| self.heights.get(&s.id).copied().unwrap_or(default_h)).collect();
        let (places, content) = stack(&heights, self.expanded, material);
        let want = count > 0 && !self.covered && !self.nc_open;
        if want && self.win.is_none() {
            self.make_window(st);
        }
        let banners = std::mem::take(&mut self.banners);
        let rows: Vec<ui::Banner> = banners.iter().enumerate().map(|(i, s)| self.row(st, s, places[i], i)).rev().collect();
        self.banners = banners;
        let Some(w) = self.win.as_mut() else { return };
        // the cards, back to front: kept, added and removed by id (a card
        // that stays keeps its animations)
        let ids: Vec<i32> = rows.iter().map(|r| r.id).collect();
        let m = &w.model;
        let mut i = 0;
        for r in rows {
            while i < m.row_count() && !ids.contains(&m.row_data(i).unwrap().id) {
                m.remove(i);
            }
            if i < m.row_count() && m.row_data(i).unwrap().id == r.id {
                if m.row_data(i).unwrap() != r {
                    m.set_row_data(i, r);
                }
            } else {
                m.insert(i, r);
            }
            i += 1;
        }
        while m.row_count() > i {
            m.remove(m.row_count() - 1);
        }
        w.ui.set_expanded(self.expanded);
        w.ui.set_content_height(content);
        w.ui.set_pills(ModelRc::new(VecModel::from(pills(count, self.expanded))));
        w.ui.set_glass_plugin(st.shell.plugin);
        // The window: the cards plus their room and the pills
        let pill_room = if count > 1 { if material { 48.0 } else { 36.0 } } else { 0.0 };
        let size = ((CARD_W + 2.0 * MARGIN) as u32, (content + 2.0 * MARGIN + pill_room).ceil().max(1.0) as u32);
        let top = 30 + if material { 8 } else { 6 } - MARGIN as i32 + if self.osd { 62 } else { 0 };
        w.surface.layer.set_margin(top, 8 - MARGIN as i32, 0, 0);
        // Input only where the cards and the pills are: the window's room
        // overlaps the menu bar (its clock, Control Center), whose clicks it
        // would take (from 8 px up and left in Glass: the close badge
        // sits over the corner)
        let reach = if material { 0.0 } else { 8.0 };
        let pills_h = if count > 1 { 8.0 + if material { 32.0 } else { 26.0 } } else { 0.0 };
        let region = (MARGIN - reach, MARGIN - reach, CARD_W + reach, content + reach + pills_h);
        let key = format!("{region:?}");
        if key != w.region {
            st.set_input_region(&w.surface, Some(&[(region.0 as i32, region.1 as i32, region.2.ceil() as i32, region.3.ceil() as i32)]));
            w.region = key;
        }
        // Banners never take the keyboard, except while a reply is written
        let kb = self.banners.iter().any(|s| s.replying);
        if kb != w.keyboard {
            w.surface.layer.set_keyboard_interactivity(if kb { KeyboardInteractivity::OnDemand } else { KeyboardInteractivity::None });
            w.keyboard = kb;
        }
        if want {
            if !w.shown || size != w.size {
                w.surface.show(size.0, size.1);
                w.size = size;
                w.shown = true;
            } else {
                w.surface.layer.commit();
            }
        } else if w.shown {
            w.shown = false;
            w.surface.hide();
            st.shell.glass.set(&w.output, NS, &[]);
            w.glass_sent.clear();
        }
        self.timers(st);
        if self.nc_open {
            self.nc_refresh(st);
        }
        if count > 0 && !self.clock.running() {
            self.arm(st, &self.clock, 30_000, || M::Clock);
        }
    }

    fn make_window(&mut self, st: &mut State) {
        let outputs = st.outputs_now();
        let Some((output, name)) = outputs.iter().find(|(_, n)| *n == self.focused).or(outputs.first()).cloned() else { return };
        let surface = st.prepare_surface(Spec {
            layer: Layer::Overlay,
            namespace: NS,
            output: Some((&output, &name)),
            anchor: Anchor::TOP | Anchor::RIGHT,
            size: ((CARD_W + 2.0 * MARGIN) as u32, 1),
            exclusive: -1,
            keyboard: KeyboardInteractivity::None,
            margin_left: 0,
        });
        let ui = ui::Banners::new().expect("Banners");
        st.shell.apply_theme(&ui.global::<ui::Theme>());
        let model = Rc::new(VecModel::<ui::Banner>::default());
        ui.set_banners(ModelRc::from(model.clone()));
        let to = st.shell.sender(NAME);
        let id = |i: i32| i as u32;
        let t = to.clone();
        ui.on_measured(move |i, h| t(Box::new(M::Measured(id(i), h))));
        let t = to.clone();
        ui.on_moved(move |i, x, y, w, h, o| t(Box::new(M::Moved(id(i), [x, y, w, h, o]))));
        let t = to.clone();
        ui.on_activated(move |i| t(Box::new(M::Activated(id(i)))));
        let t = to.clone();
        ui.on_dismiss_banner(move |i| t(Box::new(M::Close(id(i)))));
        let t = to.clone();
        ui.on_action(move |i, k| t(Box::new(M::Action(id(i), k.to_string()))));
        let t = to.clone();
        ui.on_reply_open(move |i| t(Box::new(M::ReplyOpen(id(i)))));
        let t = to.clone();
        ui.on_reply_cancel(move |i| t(Box::new(M::ReplyCancel(id(i)))));
        let t = to.clone();
        ui.on_reply_send(move |i, s| t(Box::new(M::ReplySend(id(i), s.to_string()))));
        let t = to.clone();
        ui.on_draft(move |i, s| t(Box::new(M::Draft(id(i), s.to_string()))));
        let t = to.clone();
        ui.on_link(move |u| t(Box::new(M::Link(u.to_string()))));
        let t = to;
        ui.on_pill(move |p| t(Box::new(M::Pill(p.to_string()))));
        ui.show().expect("show banners");
        let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
        self.moved.clear();
        self.win = Some(Win { output: name, surface, ui, model, a11y, size: (0, 0), keyboard: false, shown: false, glass_sent: String::new(), region: String::new() });
    }

    fn drop_window(&mut self, st: &mut State) {
        if let Some(w) = self.win.take() {
            st.shell.glass.set(&w.output, NS, &[]);
            st.drop_surface(&w.surface);
        }
    }

    fn command(&mut self, st: &mut State, c: &serde_json::Value) {
        let id = c["id"].as_u64().unwrap_or(0) as u32;
        match c["do"].as_str().unwrap_or("") {
            "hello" => {
                self.shell = true;
                self.publish();
            }
            "dismiss" => self.dismiss(st, id, notify::DISMISSED),
            "invoke" => self.invoke(st, id, c["action"].as_str().unwrap_or("default")),
            "reply" => {
                self.reply(st, id, c["text"].as_str().unwrap_or(""));
            }
            "clear" => self.clear_all(st),
            "banner" => {
                if self.store.has(id) {
                    self.show_banner(st, id);
                    self.refresh(st);
                }
            }
            // Notification Center's widgets and calendar (panel.rs)
            "widgets" => self.nc_shell(st, c),
            "calendar" => self.nc_shell(st, c),
            "shell" => {
                self.covered = c["covered"].as_bool().unwrap_or(false);
                self.osd = c["osd"].as_bool().unwrap_or(false);
                self.refresh(st);
            }
            _ => {}
        }
    }

    /// Sends a reply (Notifications.qml's reply): "" sent, else what went wrong
    fn reply(&mut self, st: &mut State, id: u32, text: &str) -> String {
        let t = text.trim();
        if t.is_empty() {
            return "Write a reply first.".into();
        }
        let Some(n) = self.store.get(id).cloned() else {
            return "The app closed this notification, so the reply couldn't be sent. Open the app to answer.".into();
        };
        if let Some(b) = notify::bus() {
            b.replied(id, t);
        }
        self.drafts.remove(&id);
        if !n.resident {
            // dismissed a moment after sending, so "Sent" shows first
            self.send_later(st, 1200, M::DismissAfterReply(id));
        }
        String::new()
    }
}

/// A link in a notification's body (http(s) and mailto only: markup.rs)
fn open_link(url: &str) {
    if markup::link_allowed(url) {
        let url = url.trim().to_string();
        std::thread::spawn(move || {
            let _ = std::process::Command::new("xdg-open").arg(url).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().and_then(|mut c| c.wait());
        });
    }
}

thread_local! {
    static BUS_ID: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

impl Part for Notifications {
    fn name(&self) -> &'static str {
        NAME
    }

    fn start(&mut self, st: &mut State) {
        // (said to be drawn only once the bus's name is ours: Event::Owner)
        st.shell.surfaces.hold(NAME);
        st.shell.want("hypr");
        st.shell.want("notify");
    }

    /// Notification Center: opened and closed through the shell's Panels
    fn command(&mut self, st: &mut State, cmd: &crate::shell::Command) {
        self.nc_command(st, cmd);
    }

    fn keyboard_left(&mut self, st: &mut State, s: &Rc<Surface>) {
        // (a banner writing a reply just lets go of the keyboard; the
        // composer and its draft stay)
        self.nc_keyboard_left(st, s);
    }

    fn output_removed(&mut self, st: &mut State, name: &str) {
        self.nc_output_removed(st, name);
        if self.win.as_ref().is_some_and(|w| w.output == name) {
            self.drop_window(st);
            self.refresh(st);
        }
    }

    fn surface_closed(&mut self, st: &mut State, s: &Rc<Surface>) {
        if self.nc_owns(s) {
            let name = s.output.clone();
            self.nc_output_removed(st, &name);
        }
        if self.win.as_ref().is_some_and(|w| Rc::ptr_eq(&w.surface, s)) {
            self.drop_window(st);
        }
    }

    fn config_changed(&mut self, st: &mut State, look: bool) {
        if look {
            if let Some(w) = &mut self.win {
                st.shell.apply_theme(&w.ui.global::<ui::Theme>());
                // (Material draws its own cards: no glass left under them)
                st.shell.glass.set(&w.output, NS, &[]);
                w.glass_sent.clear();
            }
            self.heights.clear();
            self.nc_theme(st);
        } else if self.nc_open {
            self.nc_refresh(st);
        }
        self.refresh(st);
    }

    fn plugin_changed(&mut self, st: &mut State) {
        if let Some(w) = &mut self.win {
            st.shell.apply_theme(&w.ui.global::<ui::Theme>());
            w.glass_sent.clear();
        }
        self.nc_theme(st);
        self.refresh(st);
    }

    fn data(&mut self, st: &mut State, what: TypeId) {
        if what == TypeId::of::<Snapshot>() {
            let f = st.shell.get::<Snapshot>().map(|s| s.focused.clone()).unwrap_or_default();
            if f != self.focused {
                self.focused = f;
                // (on the focused screen: one showing moves there)
                if self.win.as_ref().is_some_and(|w| w.output != self.focused) && st.outputs_now().iter().any(|(_, n)| *n == self.focused) {
                    self.drop_window(st);
                    self.refresh(st);
                }
            }
            return;
        }
        if what != TypeId::of::<Event>() {
            return;
        }
        let Some(ev) = st.shell.get::<Event>() else { return };
        match ev {
            Event::Restored { bus, list } => {
                BUS_ID.with(|b| *b.borrow_mut() = bus.clone());
                // (anything that came first stays, after the kept ones)
                let mut kept = Store { list: list.clone() };
                for n in std::mem::take(&mut self.store.list) {
                    kept.add(n);
                }
                self.store = kept;
                self.restored = true;
                self.changed(st);
            }
            Event::Owner(ours) => {
                let ours = *ours;
                if ours != self.owner {
                    self.owner = ours;
                    eprintln!("tessera-ui: notifications: {}", if ours { "serving org.freedesktop.Notifications" } else { "lost org.freedesktop.Notifications" });
                    if ours {
                        st.shell.surfaces.release(NAME);
                    } else {
                        st.shell.surfaces.hold(NAME);
                    }
                    crate::surfaces::announce(&st.shell.surfaces);
                }
            }
            Event::Notify(n) => {
                let n = n.clone();
                self.arrived(st, n);
            }
            Event::CloseRequest(id) => {
                let id = *id;
                self.dismiss(st, id, notify::CLOSED);
            }
        }
    }

    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        let msg = match msg.downcast::<serde_json::Value>() {
            Ok(c) => return self.command(st, &c),
            Err(m) => m,
        };
        let msg = match msg.downcast::<panel::C>() {
            Ok(c) => return self.nc_message(st, *c),
            Err(m) => m,
        };
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Expire => self.expired(st),
            M::Clock => {
                self.refresh(st);
                if !self.banners.is_empty() {
                    self.arm(st, &self.clock, 30_000, || M::Clock);
                }
            }
            M::Enter => {
                // (the window must be on screen first, or the entrance is lost;
                // nothing to wait for when it isn't meant to show: no ticking idle)
                if !self.banners.iter().any(|s| !s.entered) {
                    return;
                }
                let waiting = self.win.as_ref().is_some_and(|w| w.shown && !w.surface.mapped.get());
                if waiting && !self.covered {
                    self.arm(st, &self.enter, 16, || M::Enter);
                    return;
                }
                for s in &mut self.banners {
                    s.entered = true;
                }
                self.refresh(st);
            }
            M::Gone(id) => {
                let before = self.banners.len();
                self.banners.retain(|s| !(s.id == id && s.leaving));
                if self.banners.len() != before {
                    self.heights.remove(&id);
                    self.moved.remove(&id);
                    self.refresh(st);
                }
            }
            M::Publish => self.publish(),
            M::Measured(id, h) => {
                if self.heights.get(&id) != Some(&h) && h > 0.0 {
                    self.heights.insert(id, h);
                    self.refresh(st);
                }
            }
            M::Moved(id, g) => {
                self.moved.insert(id, g);
            }
            M::Activated(id) => {
                // the click: the app's default action, and it's done with
                let def = self.store.get(id).is_some_and(|n| n.default_action().is_some());
                if def {
                    if let Some(b) = notify::bus() {
                        b.action(id, "default");
                    }
                }
                self.dismiss(st, id, notify::DISMISSED);
            }
            M::Close(id) => self.leave(st, id),
            M::Action(id, k) => self.invoke(st, id, &k),
            M::ReplyOpen(id) => {
                if self.store.get(id).is_some_and(|n| n.has_inline_reply) {
                    for s in self.banners.iter_mut().filter(|s| s.id == id) {
                        s.replying = true;
                        s.sent = false;
                        s.error.clear();
                    }
                    self.refresh(st);
                }
            }
            M::ReplyCancel(id) => {
                for s in self.banners.iter_mut().filter(|s| s.id == id) {
                    s.replying = false;
                    s.error.clear();
                }
                self.refresh(st);
            }
            M::ReplySend(id, text) => {
                let err = self.reply(st, id, &text);
                for s in self.banners.iter_mut().filter(|s| s.id == id) {
                    if err.is_empty() {
                        s.replying = false;
                        s.sent = true;
                    }
                    s.error = err.clone();
                }
                if err.is_empty() {
                    self.send_later(st, 2500, M::SentDone(id));
                }
                self.refresh(st);
            }
            M::Draft(id, text) => {
                if text.is_empty() {
                    self.drafts.remove(&id);
                } else {
                    self.drafts.insert(id, text);
                }
            }
            M::SentDone(id) => {
                for s in self.banners.iter_mut().filter(|s| s.id == id) {
                    s.sent = false;
                }
                self.refresh(st);
            }
            M::DismissAfterReply(id) => self.dismiss(st, id, notify::DISMISSED),
            M::Link(url) => open_link(&url),
            M::Pill(p) => {
                match p.as_str() {
                    "expand" => self.expanded = self.banners.len() > 1,
                    "collapse" => self.expanded = false,
                    "clear" => return self.clear_all(st),
                    _ => {}
                }
                self.refresh(st);
            }
        }
    }

    /// Hover (timers hold), Glass's shapes as the cards move, accessibility
    fn after_frame(&mut self, st: &mut State) {
        self.nc_after_frame(st);
        let Some(w) = self.win.as_mut() else { return };
        let hovered = w.ui.get_hovered();
        let kbd = st.keyboard_surface();
        let f = kbd.as_ref().is_some_and(|k| Rc::ptr_eq(k, &w.surface));
        if let Some(a) = w.a11y.as_mut() {
            a.update(false, f);
        }
        if st.shell.plugin && !st.shell.look.material && w.surface.mapped.get() && w.shown {
            let look = &st.shell.look;
            let ids: Vec<i32> = (0..w.model.row_count()).filter_map(|i| w.model.row_data(i)).map(|r| r.id).collect();
            let mut shapes = Vec::new();
            for id in ids {
                let Some(&[x, y, cw, ch, op]) = self.moved.get(&(id as u32)) else { continue };
                if op <= 0.002 {
                    continue;
                }
                let s = w.model.iter().find(|r| r.id == id).map_or(1.0, |r| r.tscale);
                // (scaled about its bottom centre, as the card; `s` the target:
                // the scale only springs between 1, 0.95 and 0.9)
                let (sw, sh) = (cw * s, ch * s);
                shapes.push(theme::glass_shape(look, x + (cw - sw) / 2.0, y + (ch - sh), sw, sh, 22.0 * s, op, 1.0));
            }
            let key = format!("{shapes:?}");
            if key != w.glass_sent {
                st.shell.glass.set(&w.output, NS, &shapes);
                w.glass_sent = key;
            }
        }
        if hovered != self.hovered {
            self.hovered = hovered;
            self.timers(st);
        }
    }

    fn a11y(&mut self) -> Vec<&mut crate::a11y::A11y> {
        let mut out: Vec<&mut crate::a11y::A11y> = self.win.as_mut().and_then(|w| w.a11y.as_mut()).into_iter().collect();
        out.extend(self.nc.win_a11y());
        out
    }

    fn stats(&self) {
        if let Some(w) = &self.win {
            eprintln!("tessera-ui: stats {}", w.surface.stats.borrow_mut().report("notifications"));
        }
    }

    fn quit(&mut self, _: &mut State) {
        self.nc_quit();
        if let Some(w) = &self.win {
            Glass::clear_now(&w.output, NS);
        }
        // (the list as it is now, for the next run: the timer may not have fired)
        let json = model::history_json(&self.bus_id(), &self.store);
        if self.restored {
            let p = notify::history_path();
            if let Some(dir) = p.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let tmp = p.with_extension("json.tmp");
            if std::fs::write(&tmp, json).is_ok() {
                let _ = std::fs::rename(&tmp, &p);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_banner() {
        let (p, h) = stack(&[90.0], false, false);
        assert_eq!(p, vec![(0.0, 1.0, 1.0)]);
        assert_eq!(h, 90.0);
        assert!(pills(1, false).is_empty());
    }

    #[test]
    fn a_stack_peeks_and_fans_out() {
        let hs = [90.0, 120.0, 80.0, 100.0];
        let (p, h) = stack(&hs, false, false);
        // older cards peek 8 px under the front one (two at most), smaller and fainter
        assert_eq!(p[1], (90.0 + 8.0 - 120.0, 0.95, 0.9));
        assert_eq!(p[2], (90.0 + 16.0 - 80.0, 0.9, 0.65));
        assert_eq!(p[3].2, 0.0); // the fourth is hidden
        assert_eq!(h, 106.0);
        let (p, h) = stack(&hs, true, false);
        assert_eq!(p.iter().map(|x| x.0).collect::<Vec<_>>(), vec![0.0, 100.0, 230.0, 320.0]);
        assert!(p.iter().all(|x| x.1 == 1.0 && x.2 == 1.0));
        assert_eq!(h, 420.0);
        // Material's gap is 8
        assert_eq!(stack(&hs, true, true).1, 414.0);
        assert_eq!(pills(4, false)[0].text, "3 more");
        assert_eq!(pills(4, true).iter().map(|p| p.text.to_string()).collect::<Vec<_>>(), vec!["Clear all", "Show less"]);
    }

    #[test]
    fn the_picture_or_the_apps_icon() {
        let mut lookup = |n: &str| Some(format!("/icons/{n}.svg"));
        let n = Note { image: "file:///tmp/ann.png".into(), app_icon: "chat".into(), ..Default::default() };
        assert_eq!(icon_source(&n, &mut lookup), Some(("/tmp/ann.png".into(), true)));
        let n = Note { app_icon: "chat".into(), ..Default::default() };
        assert_eq!(icon_source(&n, &mut lookup), Some(("/icons/chat.svg".into(), false)));
        let n = Note { app_icon: "/opt/app/icon.png".into(), ..Default::default() };
        assert_eq!(icon_source(&n, &mut lookup), Some(("/opt/app/icon.png".into(), false)));
        // nothing named: the generic one; nothing found at all: none
        assert_eq!(icon_source(&Note::default(), &mut lookup), Some(("/icons/dialog-information.svg".into(), false)));
        assert_eq!(icon_source(&Note::default(), &mut |_| None), None);
    }
}

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
//!       {"to":"notifications","do":"widgets"|"calendar"|"calendarError",…}  Notification
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

pub mod ui {
    slint::include_modules!();
}
tessera_core::theme_global!(ui);

// tessera-core's modules (shell, wayland, theme, services, util…) under the
// names they had when everything was one crate: `crate::shell::Part`
#[allow(unused_imports)]
use tessera_core::*;

pub mod center;
pub mod stack;
/// (in tessera-core: the notification server, services::notify, keeps them)
pub use tessera_core::services::notify::{markup, model};
mod panel;
mod stable;

use tessera_core::services::hypr::{Glass, Snapshot};
use tessera_core::services::notify::{self, Event};
use tessera_core::shell::Part;
use tessera_core::theme;
use tessera_core::wayland::{Spec, State, Surface};
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
    /// an icon or a sender's picture, decoded on a thread (by its key)
    Image(String, Option<Decoded>),
}

/// A decoded icon (with its transparency) or picture (cut square)
enum Decoded {
    Rgba(slint::SharedPixelBuffer<slint::Rgba8Pixel>),
    Rgb(slint::SharedPixelBuffer<slint::Rgb8Pixel>),
}

/// How many decoded icons and pictures are kept (the oldest go first)
const IMAGES_KEPT: usize = 64;

/// The cache's key for a file as an icon or as a picture
fn image_key(path: &str, picture: bool) -> String {
    format!("{}{path}", if picture { "pic:" } else { "icon:" })
}

/// A card's icon or picture from its file, small (shown at 40 px; off the
/// UI thread: a full-size photo as a message's picture held the banner up)
fn decode_image(path: &str, picture: bool) -> Option<Decoded> {
    let p = std::path::Path::new(path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let svg = ext == "svg" || ext == "svgz";
    if picture && !svg {
        // (cut to the circle's square, as image-fit: cover did)
        return tessera_core::picture::decode(path, 96, 96).map(Decoded::Rgb);
    }
    let plain = tessera_core::appicon::Style { mode: 0, tiled: false, dark: false, contrast: 60, tint: [0; 3], tile: None, tile_from: "auto".into(), accent: [0; 3] };
    tessera_core::appicon::render(p, 96, &plain, None).map(Decoded::Rgba)
}

/// Room for one more in the cache: the oldest dropped while it's full
fn make_room<V>(images: &mut HashMap<String, V>, order: &mut std::collections::VecDeque<String>, keep: usize) {
    while images.len() >= keep {
        let Some(old) = order.pop_front() else { break };
        images.remove(&old);
    }
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
    /// where it was last laid out (a leaving card stays there as it fades)
    place: Option<Place>,
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
    /// decoded icons and pictures (None: the file wouldn't decode), by
    /// image_key; the order they came in, and those on their way
    images: HashMap<String, Option<slint::Image>>,
    image_order: std::collections::VecDeque<String>,
    image_wait: std::collections::HashSet<String>,
    /// Notification Center's rows are behind the list (rebuilt on the next
    /// refresh while it's open; a banner's layout alone doesn't)
    nc_dirty: bool,
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
    /// cards' buttons, the same model while the same (stable.rs)
    buttons: stable::Stable<ui::NoteButton>,
    /// live entries' histories (core notify/live.rs), the same way
    histories: stable::Stable<slint::SharedString>,
    /// the pills last set (count, expanded): set again only when they change
    pills_sent: Option<(usize, bool)>,
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
    // (core's lookup: the shell's own index of its theme and the themes it
    // inherits — Colloid's kind/size folders, symlinked ones too — as the
    // QML's IconLookup; the walk below only read size/kind folders and an
    // index file only the QML shell writes)
    if let Some(p) = crate::icon_theme::find(name, None) {
        return Some(p.to_string_lossy().into_owned());
    }
    theme_icon_walk(name)
}

fn theme_icon_walk(name: &str) -> Option<String> {
    let cache =std::env::var_os("XDG_CACHE_HOME").map(std::path::PathBuf::from).unwrap_or_else(|| std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".cache"));
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

/// Where a banner sits: its top, scale and opacity, and in a folded stack
/// the height it's drawn at (`plate`, 0 its own) and how far back it is
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Place {
    pub y: f32,
    pub scale: f32,
    pub fade: f32,
    /// a back card in a folded stack is drawn as a plate the front card's
    /// height, its content hidden, so only its lower edge shows under the
    /// front card: a taller one used to stick out above the front card and
    /// its text showed through it (two toasts drawn over each other)
    pub plate: f32,
    /// 0 the front card (or any, fanned out), 1 and 2 the cards peeking
    /// out under it (stack.rs: what shows of each is cut to below the card
    /// in front, 19 Nov)
    pub depth: usize,
}

impl Place {
    pub const WHOLE: Place = Place { y: 0.0, scale: 1.0, fade: 1.0, plate: 0.0, depth: 0 };
}

/// Each banner's place (front first) and the stack's height: folded, as
/// iOS (stack.rs): older cards peek out 8 then 6 px under the front one
/// (two at most), 8 px narrower a side a step, fainter
pub fn stack(heights: &[f32], expanded: bool, material: bool) -> (Vec<Place>, f32) {
    let gap = if material { 8.0 } else { 10.0 };
    let n = heights.len();
    if n == 0 {
        return (Vec::new(), 0.0);
    }
    let offset = |i: usize| heights[..i].iter().map(|h| h + gap).sum::<f32>();
    let places = (0..n)
        .map(|i| {
            if expanded || i == 0 {
                return Place { y: if expanded { offset(i) } else { 0.0 }, ..Place::WHOLE };
            }
            let d = i.min(stack::MAX_DEPTH);
            Place {
                y: stack::peek(d),
                scale: stack::scale(CARD_W, d),
                fade: if i <= stack::MAX_DEPTH { stack::fade(i) } else { 0.0 },
                plate: heights[0],
                depth: d,
            }
        })
        .collect();
    let height = if expanded { offset(n) - gap } else { stack::collapsed_height(heights[0], n) };
    (places, height)
}

/// The places of a stack whose leaving cards (`leaving[i]`) keep where they
/// were (`last`) while they fade out, the rest laid out without them: a card
/// on its way out no longer holds the front, and isn't counted
pub fn stack_with_leaving(heights: &[f32], leaving: &[bool], last: &[Option<Place>], expanded: bool, material: bool) -> (Vec<Place>, f32, usize) {
    let staying: Vec<f32> = heights.iter().zip(leaving).filter(|(_, l)| !**l).map(|(h, _)| *h).collect();
    let (places, height) = stack(&staying, expanded, material);
    let mut it = places.into_iter();
    let out = (0..heights.len())
        .map(|i| if leaving[i] { last[i].unwrap_or(Place::WHOLE) } else { it.next().unwrap() })
        .collect();
    (out, height, staying.len())
}

/// What shows of each card peeking out of a folded stack, for Slint's
/// Path (stack.rs; both windows)
pub(crate) fn stack_shapes(g: &ui::StackShape<'_>) {
    g.on_path(|a| {
        let f = if a.banner { stack::banner_path } else { stack::nc_path };
        f(a.w, a.h, a.r, a.depth.max(0) as usize, a.circular, a.edge).into()
    });
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
    /// The screen Notification Center is on (made)
    fn nc_output(&self) -> Option<String> {
        self.nc.output()
    }

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
        self.nc_dirty = true;
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
        if let Some(old) = self.store.get(n.id).cloned() {
            // the same entry again (its id, its event id, its thread: the
            // server joined them): in place, where it is, its history carried
            // on (a banner showing changes). It pops up again, its sound with
            // it (the shell plays it for a new `alerted`), only when it got
            // more urgent (live.rs)
            let (n, again) = model::merge(&old, n);
            let id = n.id;
            self.store.add(n);
            if again && old.route != Route::Quiet.name() {
                let route = model::route(self.store.get(id).unwrap(), &st.shell.cfg, &focus_json(), now_ms(), self.shell);
                if route == Route::Banner {
                    self.show_banner(st, id);
                }
            }
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
        if let Some(i) = self.banners.iter().position(|s| s.id == id && !s.leaving) {
            // (showing: to the front, its time from the start again)
            let mut s = self.banners.remove(i);
            s.deadline = None;
            self.banners.insert(0, s);
            return;
        }
        self.banners.retain(|s| s.id != id);
        self.banners.insert(0, Shown { id, entered: false, leaving: false, deadline: None, replying: false, sent: false, error: String::new(), place: None });
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

    /// A card's icon (`picture`: a sender's picture), once decoded on a
    /// thread; none until then (the card is made again when it's there)
    fn image(&mut self, st: &State, path: &str, picture: bool) -> Option<slint::Image> {
        let key = image_key(path, picture);
        if let Some(i) = self.images.get(&key) {
            return i.clone();
        }
        if self.image_wait.insert(key.clone()) {
            let to = st.shell.sender(NAME);
            let p = path.to_string();
            std::thread::spawn(move || {
                let px = decode_image(&p, picture);
                to(Box::new(M::Image(key, px)));
            });
        }
        None
    }

    fn row(&mut self, st: &State, s: &Shown, place: Place, index: usize) -> ui::Banner {
        let n = self.store.get(s.id).cloned().unwrap_or_default();
        ui::Banner {
            index: index as i32,
            ty: place.y,
            tscale: place.scale,
            tfade: place.fade,
            plate: place.plate,
            depth: place.depth as i32,
            // (drawn whole: the front card, or any while fanned out; a
            // plate is a back card of the folded stack)
            front: index == 0 || place.plate == 0.0,
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
        // (a miss is kept only once the shell's index is there: before, it's asked again)
        let indexed = tessera_core::services::icons::shared().is_some();
        let mut lookup = |name: &str| match icons.get(name) {
            Some(hit) => hit.clone(),
            None => {
                let f = theme_icon(name);
                if f.is_some() || indexed {
                    icons.insert(name.to_string(), f.clone());
                }
                f
            }
        };
        let src = icon_source(&n, &mut lookup);
        let (icon, picture) = match src {
            Some((f, pic)) => (self.image(st, &f, pic), pic),
            None => (None, false),
        };
        let app = if n.app_name.is_empty() { "Notification".to_string() } else { n.app_name.clone() };
        let h24 = st.shell.cfg["appearance"]["clock24"].as_bool().unwrap_or(false);
        let plain = markup::plain(&n.body);
        ui::Banner {
            id: n.id as i32,
            app_upper: app.to_uppercase().into(),
            app: app.into(),
            time: model::time_label(n.changes.last().map_or(n.at, |c| c.at), now_ms(), |t| clock_at(t, h24)).into(), // (a live entry: its latest change)
            summary: n.summary.clone().into(),
            body: if plain.is_empty() { slint::StyledText::default() } else { markup::styled(&n.body) },
            body_plain: plain.into(),
            has_icon: icon.is_some(),
            icon: icon.unwrap_or_default(),
            picture,
            buttons: self.buttons.get(&n.id.to_string(), n.buttons().iter().map(|a| ui::NoteButton { key: a.id.clone().into(), text: a.text.clone().into() }).collect::<Vec<_>>()),
            can_reply: n.has_inline_reply,
            placeholder: n.inline_reply_placeholder.clone().into(),
            replying: s.is_some_and(|s| s.replying),
            sent: s.is_some_and(|s| s.sent),
            error: s.map(|s| s.error.clone()).unwrap_or_default().into(),
            draft: self.drafts.get(&n.id).cloned().unwrap_or_default().into(),
            progress: if n.value >= 0 { n.value as f32 / 100.0 } else { -1.0 },
            history: self.history(&format!("n{}", n.id), &n.changes, h24),
            ..Default::default()
        }
    }

    /// A live entry's history for its card (more than one change: it folds out)
    fn history(&self, key: &str, changes: &[notify::live::Change], h24: bool) -> ModelRc<slint::SharedString> {
        let lines: Vec<slint::SharedString> = if changes.len() > 1 { notify::live::history_lines(changes, |t| clock_at(t, h24)).into_iter().map(Into::into).collect() } else { Vec::new() };
        self.histories.get(key, lines)
    }

    /// Everything on screen from the state: the stack's places, the cards,
    /// the window (made on the focused screen, sized, shown or hidden)
    fn refresh(&mut self, st: &mut State) {
        let material = Self::material(st);
        // (a leaving card isn't counted: "N more" and the fan-out are the others')
        let count = self.banners.iter().filter(|s| !s.leaving).count();
        let drawn = self.banners.len();
        if count <= 1 {
            self.expanded = false;
        }
        let default_h = if material { 96.0 } else { 90.0 };
        let heights: Vec<f32> = self.banners.iter().map(|s| self.heights.get(&s.id).copied().unwrap_or(default_h)).collect();
        let leaving: Vec<bool> = self.banners.iter().map(|s| s.leaving).collect();
        let last: Vec<Option<Place>> = self.banners.iter().map(|s| s.place).collect();
        let (places, content, _) = stack_with_leaving(&heights, &leaving, &last, self.expanded, material);
        for (s, p) in self.banners.iter_mut().zip(&places) {
            s.place = Some(*p);
        }
        // (the window still holds a leaving card while it fades)
        let content = places.iter().zip(&heights).zip(&leaving).filter(|(_, l)| **l).fold(content, |c, ((p, h), _)| c.max(p.y + if p.plate > 0.0 { p.plate } else { *h }));
        // (hidden while Notification Center or Control Center is open on its
        // screen, NotificationPopups' visible)
        let nc_here = self.nc_open && match (self.win.as_ref(), self.nc_output()) {
            (Some(w), Some(o)) => w.output == o,
            _ => true,
        };
        let want = drawn > 0 && !self.covered && !nc_here;
        // (hidden: the stack folds back, so its timers run again: QML's
        // onVisibleChanged collapse)
        if !want {
            self.expanded = false;
        }
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
        if self.pills_sent != Some((count, self.expanded)) {
            self.pills_sent = Some((count, self.expanded));
            w.ui.set_pills(ModelRc::new(VecModel::from(pills(count, self.expanded))));
        }
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
        let key = tessera_core::theme::glass_key(&region);
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
        // (only when what it lists changed: a banner measured or entering
        // rebuilt every row and the 42 days of the calendar)
        if self.nc_open && std::mem::take(&mut self.nc_dirty) {
            self.nc_refresh(st);
        }
        if drawn > 0 && !self.clock.running() {
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
        stack_shapes(&ui.global::<ui::StackShape>());
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
        self.pills_sent = None;
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
            "calendar" | "calendarError" => self.nc_shell(st, c),
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
    fn prewarm(&mut self, st: &mut State) {
        self.nc_prewarm(st);
    }

    fn name(&self) -> &'static str {
        NAME
    }

    fn start(&mut self, st: &mut State) {
        // (said to be drawn only once the bus's name is ours: Event::Owner)
        st.shell.surfaces.hold(NAME);
        st.shell.want("hypr");
        st.shell.want("notify");
        // (app icons: the shell's index of its icon theme, core/src/icon_theme.rs)
        st.shell.want("icons");
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
                // (Notification Center's composer too: Reply shows again)
                self.nc_sent_done(id);
                self.nc_dirty = true;
                self.refresh(st);
            }
            M::DismissAfterReply(id) => self.dismiss(st, id, notify::DISMISSED),
            M::Link(url) => open_link(&url),
            M::Image(key, px) => {
                self.image_wait.remove(&key);
                let img = px.map(|d| match d {
                    Decoded::Rgba(b) => slint::Image::from_rgba8(b),
                    Decoded::Rgb(b) => slint::Image::from_rgb8(b),
                });
                make_room(&mut self.images, &mut self.image_order, IMAGES_KEPT);
                self.images.insert(key.clone(), img);
                self.image_order.push_back(key);
                self.nc_dirty = true;
                self.refresh(st);
            }
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
                let row = w.model.iter().find(|r| r.id == id);
                let s = row.as_ref().map_or(1.0, |r| r.tscale);
                // (a card peeking out of the folded stack casts no shadow:
                // it fell over the front card's glass, a grey band across
                // its foot; its glass under the front card is drawn over by
                // the front's, so only what peeks out shows)
                let back = row.is_some_and(|r| !r.front && !self.expanded);
                // (scaled about its bottom centre, as the card; `s` the target:
                // the scale only springs between 1, 0.95 and 0.9)
                let (sw, sh) = (cw * s, ch * s);
                let g = theme::glass_shape(look, x + (cw - sw) / 2.0, y + (ch - sh), sw, sh, 22.0 * s, op, 1.0);
                shapes.push(if back { theme::shadow_margin(g, 0.0) } else { g });
            }
            let key = tessera_core::theme::glass_key(&shapes);
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
    fn image_cache_drops_the_oldest_and_keeps_misses() {
        let mut m: HashMap<String, Option<i32>> = HashMap::new();
        let mut order = std::collections::VecDeque::new();
        for i in 0..3 {
            make_room(&mut m, &mut order, 3);
            let k = image_key(&format!("/p{i}.png"), false);
            m.insert(k.clone(), if i == 1 { None } else { Some(i) });
            order.push_back(k);
        }
        assert_eq!(m.len(), 3);
        // (a file that wouldn't decode is remembered too: not tried on every card)
        assert_eq!(m.get(&image_key("/p1.png", false)), Some(&None));
        make_room(&mut m, &mut order, 3);
        assert_eq!(m.len(), 2);
        assert!(!m.contains_key(&image_key("/p0.png", false)));
        assert_ne!(image_key("/a", true), image_key("/a", false));
        assert!(decode_image("/nonexistent/x.png", false).is_none());
        assert!(decode_image("/nonexistent/x.jpg", true).is_none());
    }

    #[test]
    fn one_banner() {
        let (p, h) = stack(&[90.0], false, false);
        assert_eq!(p, vec![Place::WHOLE]);
        assert_eq!(h, 90.0);
        assert!(pills(1, false).is_empty());
    }

    #[test]
    fn a_stack_peeks_and_fans_out() {
        let hs = [90.0, 120.0, 80.0, 100.0];
        let (p, h) = stack(&hs, false, false);
        // older cards are plates the front's height, peeking 8 then 6 px
        // under it (two at most), 8 px narrower a side a step, fainter
        assert_eq!(p[1], Place { y: 8.0, scale: 1.0 - 16.0 / 360.0, fade: 0.8, plate: 90.0, depth: 1 });
        assert_eq!(p[2], Place { y: 14.0, scale: 1.0 - 32.0 / 360.0, fade: 0.6, plate: 90.0, depth: 2 });
        assert_eq!((p[3].fade, p[3].depth), (0.0, 2)); // the fourth is hidden
        assert_eq!(h, 104.0);
        for (i, c) in p.iter().enumerate().skip(1).take(2) {
            assert!((CARD_W * c.scale - (CARD_W - 16.0 * i as f32)).abs() < 1e-3);
        }
        let (p, h) = stack(&hs, true, false);
        assert_eq!(p.iter().map(|x| x.y).collect::<Vec<_>>(), vec![0.0, 100.0, 230.0, 320.0]);
        assert!(p.iter().all(|x| x.scale == 1.0 && x.fade == 1.0 && x.plate == 0.0 && x.depth == 0));
        assert_eq!(h, 420.0);
        // Material's gap is 8
        assert_eq!(stack(&hs, true, true).1, 414.0);
        assert_eq!(pills(4, false)[0].text, "3 more");
        assert_eq!(pills(4, true).iter().map(|p| p.text.to_string()).collect::<Vec<_>>(), vec!["Clear all", "Show less"]);
    }

    /// The bug: "Battery empty" (two lines) behind "Connection activated"
    /// (one) stuck out above it, and its text showed through the glass. In
    /// a folded stack nothing of a back card reaches above the front card,
    /// or below it by more than its peek
    #[test]
    fn back_cards_never_show_past_the_front_one() {
        for hs in [[70.0, 140.0, 200.0], [140.0, 70.0, 90.0], [90.0, 90.0, 90.0]] {
            let (p, h) = stack(&hs, false, false);
            for (i, c) in p.iter().enumerate().skip(1) {
                let drawn = if c.plate > 0.0 { c.plate } else { hs[i] };
                // scaled about its bottom centre: its top moves down
                let top = c.y + drawn * (1.0 - c.scale);
                assert!(top >= 0.0, "{hs:?} card {i} reaches above the front one");
                assert!(c.y + drawn <= hs[0] + 14.0 + 0.01, "{hs:?} card {i} hangs past the stack");
                assert!(c.y + drawn <= h + 0.01);
            }
        }
        // fanned out: one under the other with the gap, never overlapping
        let hs = [70.0, 140.0, 200.0];
        let (p, _) = stack(&hs, true, true);
        for i in 1..3 {
            assert!(p[i].y >= p[i - 1].y + hs[i - 1] + 8.0);
        }
    }

    /// Two at once, then the front one leaves (or is replaced): the leaving
    /// card stays where it was while it fades; the other takes the front and
    /// isn't drawn as a plate under nothing; "N more" counts only staying ones
    #[test]
    fn a_leaving_card_gives_up_the_front() {
        let hs = [90.0, 120.0];
        let (before, _) = stack(&hs, false, false);
        let (p, h, staying) = stack_with_leaving(&hs, &[true, false], &[Some(before[0]), Some(before[1])], false, false);
        assert_eq!(staying, 1);
        assert_eq!(p[0], before[0]); // where it was
        assert_eq!(p[1], Place::WHOLE); // the front now, whole
        assert_eq!(h, 120.0);
        assert!(pills(staying, false).is_empty());
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

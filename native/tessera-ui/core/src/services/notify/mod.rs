//! The notification server: org.freedesktop.Notifications on the session
//! bus (the desktop notifications spec, 1.2, with KDE's inline replies), as
//! the QML shell's Quickshell NotificationServer serves it: the same
//! capabilities, ids, hints and signals. Readings arrive in the part as
//! `Msg::Data(Box<Event>)`, one per event (read at once in `data`).
//!
//! The name: asked for without taking it from anyone (no ReplaceExisting) and
//! waiting in the bus's queue when it's held. The QML shell's server holds it
//! for good once made (Quickshell's server lives as long as the process), so
//! the QML shell doesn't make it while tessera-ui serves notifications (see
//! services/Notifications.qml); `Event::Owner` says when the name is ours,
//! and only then does the part say it draws notifications. When this process
//! ends the name is free again and Quickshell's server (which watches it)
//! takes it back.
//!
//! What it keeps itself: the ids (the next one and the live ones, for
//! replaces_id and CloseNotification) and pictures sent as pixels (image-data),
//! saved as PNG files so the shell's own cards and the lock screen can show
//! them too. The list and everything shown is the part's
//! (surfaces/notifications); it answers through `Bus`.

pub mod markup;
pub mod model;

use model::{Hints, Note};
use crate::Msg;
use smithay_client_toolkit::reexports::calloop::channel::Sender;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex, OnceLock};
use zbus::blocking::{Connection, MessageIterator};
use zbus::zvariant::{OwnedValue, Value};
use zbus::MatchRule;

pub const NAME: &str = "org.freedesktop.Notifications";
const PATH: &str = "/org/freedesktop/Notifications";

/// What the server tells the part
#[derive(Debug)]
pub enum Event {
    /// The list kept from this tessera-ui's last run on the same bus
    Restored { bus: String, list: Vec<Note> },
    /// The name is ours (true) or not (false: lost, or still queued)
    Owner(bool),
    /// An app sent one (new, or replacing one with the same id)
    Notify(Note),
    /// An app took one back (CloseNotification)
    CloseRequest(u32),
}

/// Why one closed (the spec's reasons)
pub const EXPIRED: u32 = 1;
pub const DISMISSED: u32 = 2;
pub const CLOSED: u32 = 3;

enum Out {
    Closed(u32, u32),
    Action(u32, String),
    Replied(u32, String),
}

/// The part's way back to the apps: signals, and forgetting ids
#[derive(Clone)]
pub struct Bus {
    out: mpsc::Sender<Out>,
    ids: Arc<Mutex<Ids>>,
}

impl Bus {
    /// It's gone (dismissed, expired, taken back): the app hears why
    pub fn closed(&self, id: u32, reason: u32) {
        if let Ok(mut ids) = self.ids.lock() {
            ids.live.remove(&id);
            if let Some(p) = ids.pictures.remove(&id) {
                let _ = std::fs::remove_file(p);
            }
        }
        let _ = self.out.send(Out::Closed(id, reason));
    }
    pub fn action(&self, id: u32, key: &str) {
        let _ = self.out.send(Out::Action(id, key.to_string()));
    }
    pub fn replied(&self, id: u32, text: &str) {
        let _ = self.out.send(Out::Replied(id, text.to_string()));
    }
}

static BUS: OnceLock<Bus> = OnceLock::new();

/// The server's handle, once it's started (shell.want("notify"))
pub fn bus() -> Option<&'static Bus> {
    BUS.get()
}

#[derive(Default)]
struct Ids {
    next: u32,
    live: HashSet<u32>,
    /// pictures saved from image-data, by id (removed with the notification)
    pictures: HashMap<u32, PathBuf>,
    saved: u64,
}

/// Tessera's state folder ($XDG_STATE_HOME/tessera, or irice while the
/// settings haven't moved: Paths.qml's `state`)
pub fn state_dir() -> PathBuf {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/state"));
    let name = crate::theme::config_path().parent().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "tessera".into());
    base.join(name)
}

pub fn history_path() -> PathBuf {
    state_dir().join("notifications.json")
}

fn pictures_dir() -> PathBuf {
    state_dir().join("notification-pictures")
}

// ── Hints ───────────────────────────────────────────────────────────────────

fn int(v: &Value) -> Option<i64> {
    Some(match v {
        Value::U8(x) => *x as i64,
        Value::I16(x) => *x as i64,
        Value::U16(x) => *x as i64,
        Value::I32(x) => *x as i64,
        Value::U32(x) => *x as i64,
        Value::I64(x) => *x,
        Value::U64(x) => *x as i64,
        Value::F64(x) => *x as i64,
        Value::Value(x) => return int(x),
        _ => return None,
    })
}

fn string(v: &Value) -> String {
    match v {
        Value::Str(s) => s.to_string(),
        Value::ObjectPath(p) => p.to_string(),
        Value::Value(x) => string(x),
        _ => String::new(),
    }
}

fn boolean(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Value(x) => boolean(x),
        other => int(other).is_some_and(|i| i != 0),
    }
}

/// The hints the shell uses (Quickshell's names, with their old spellings)
pub fn hints_from(h: &HashMap<String, OwnedValue>) -> Hints {
    let get = |k: &str| h.get(k).map(|v| &**v);
    let s = |k: &str| get(k).map(string).unwrap_or_default();
    let path = |p: String| p.strip_prefix("file://").map(str::to_string).unwrap_or(p);
    Hints {
        urgency: get("urgency").and_then(int).map(|u| u.clamp(0, 2) as u8),
        category: s("category"),
        desktop_entry: s("desktop-entry"),
        image_path: path(if h.contains_key("image-path") { s("image-path") } else { s("image_path") }),
        resident: get("resident").is_some_and(boolean),
        transient: get("transient").is_some_and(boolean),
        suppress_sound: get("suppress-sound").is_some_and(boolean),
        sound_file: s("sound-file"),
        value: get("value").and_then(int).map(|v| v as i32),
        reply_placeholder: s("x-kde-reply-placeholder-text"),
    }
}

/// The picture an app sent as pixels (image-data, or its older names): its
/// size and RGBA bytes
pub fn image_data(h: &HashMap<String, OwnedValue>) -> Option<(u32, u32, Vec<u8>)> {
    let v = ["image-data", "image_data", "icon_data"].iter().find_map(|k| h.get(*k))?;
    let Value::Structure(s) = &**v else { return None };
    let f = s.fields();
    if f.len() != 7 {
        return None;
    }
    let n = |i: usize| int(&f[i]);
    let data: Vec<u8> = Vec::<u8>::try_from(f[6].try_clone().ok()?).ok()?;
    let (w, h) = (n(0)?, n(1)?);
    rgba(w as i32, h as i32, n(2)? as i32, boolean(&f[3]), n(4)? as i32, n(5)? as i32, &data).map(|px| (w as u32, h as u32, px))
}

/// (iiibiiay) pixels as RGBA, 8 bits a sample, 3 or 4 channels, rows `stride` apart
pub fn rgba(w: i32, h: i32, stride: i32, alpha: bool, bits: i32, channels: i32, data: &[u8]) -> Option<Vec<u8>> {
    if w <= 0 || h <= 0 || w > 4096 || h > 4096 || bits != 8 || !(channels == 3 || channels == 4) || (alpha && channels != 4) {
        return None;
    }
    let (w, h, stride, ch) = (w as usize, h as usize, stride as usize, channels as usize);
    if stride < w * ch || data.len() < stride * (h - 1) + w * ch {
        return None;
    }
    let mut out = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        let row = &data[y * stride..y * stride + w * ch];
        for px in row.chunks(ch) {
            out.extend_from_slice(&[px[0], px[1], px[2], if ch == 4 { px[3] } else { 255 }]);
        }
    }
    Some(out)
}

// ── The interface ───────────────────────────────────────────────────────────

struct Server {
    tx: Sender<Msg>,
    ids: Arc<Mutex<Ids>>,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

#[zbus::interface(name = "org.freedesktop.Notifications")]
impl Server {
    /// What the QML server offers (Notifications.qml's NotificationServer)
    fn get_capabilities(&self) -> Vec<&'static str> {
        vec!["persistence", "body", "body-markup", "body-hyperlinks", "actions", "action-icons", "icon-static", "inline-reply"]
    }

    #[allow(clippy::too_many_arguments)]
    fn notify(&self, app_name: &str, replaces_id: u32, app_icon: &str, summary: &str, body: &str, actions: Vec<String>, hints: HashMap<String, OwnedValue>, expire_timeout: i32) -> u32 {
        let h = hints_from(&hints);
        let Ok(mut ids) = self.ids.lock() else { return 0 };
        let live = ids.live.clone();
        let mut next = ids.next;
        let (id, _) = model::assign_id(|i| live.contains(&i), &mut next, replaces_id);
        ids.next = next;
        ids.live.insert(id);
        // pixels: saved as a picture file (a new name each time: a picture
        // that changes under the same name would stay cached)
        let mut picture = String::new();
        if let Some((w, hh, px)) = image_data(&hints) {
            ids.saved += 1;
            let dir = pictures_dir();
            let file = dir.join(format!("{id}-{}-{}.png", std::process::id(), ids.saved));
            let saved = std::fs::create_dir_all(&dir).is_ok() && image::RgbaImage::from_raw(w, hh, px).is_some_and(|img| img.save(&file).is_ok());
            if saved {
                if let Some(old) = ids.pictures.insert(id, file.clone()) {
                    let _ = std::fs::remove_file(old);
                }
                picture = file.to_string_lossy().into_owned();
            }
        }
        drop(ids);
        let n = model::note_from(id, app_name, app_icon, summary, body, &actions, &h, expire_timeout, &picture, now_ms());
        let _ = self.tx.send(Msg::Data(Box::new(Event::Notify(n))));
        id
    }

    fn close_notification(&self, id: u32) {
        if self.ids.lock().is_ok_and(|ids| ids.live.contains(&id)) {
            let _ = self.tx.send(Msg::Data(Box::new(Event::CloseRequest(id))));
        }
    }

    #[zbus(out_args("name", "vendor", "version", "spec_version"))]
    fn get_server_information(&self) -> (String, String, String, String) {
        ("tessera".into(), "tessera".into(), env!("CARGO_PKG_VERSION").into(), "1.2".into())
    }
}

/// The bus's id (the same for every connection to one bus): the list's file
/// is kept for it
fn bus_id(conn: &Connection) -> String {
    conn.call_method(Some("org.freedesktop.DBus"), "/org/freedesktop/DBus", Some("org.freedesktop.DBus"), "GetId", &())
        .ok()
        .and_then(|m| m.body().deserialize::<String>().ok())
        .unwrap_or_default()
}

pub fn spawn(tx: Sender<Msg>) {
    let (out_tx, out_rx) = mpsc::channel::<Out>();
    let ids = Arc::new(Mutex::new(Ids { next: 1, ..Default::default() }));
    let _ = BUS.set(Bus { out: out_tx, ids: ids.clone() });
    std::thread::spawn(move || {
        let conn = loop {
            match Connection::session() {
                Ok(c) => break c,
                Err(e) => {
                    eprintln!("tessera-ui: notifications: no session bus ({e}); trying again");
                    std::thread::sleep(std::time::Duration::from_secs(5));
                }
            }
        };
        // The list kept from the last run on this bus, its ids still live
        let bus = bus_id(&conn);
        let kept = std::fs::read_to_string(history_path()).map(|t| model::history_read(&t, &bus)).unwrap_or_default();
        if let Ok(mut i) = ids.lock() {
            i.next = kept.next_id();
            i.live = kept.list.iter().map(|n| n.id).collect();
            let dir = pictures_dir();
            for n in &kept.list {
                if n.image.starts_with(dir.to_string_lossy().as_ref()) {
                    i.pictures.insert(n.id, PathBuf::from(&n.image));
                }
            }
        }
        if tx.send(Msg::Data(Box::new(Event::Restored { bus, list: kept.list }))).is_err() {
            return;
        }
        if let Err(e) = conn.object_server().at(PATH, Server { tx: tx.clone(), ids: ids.clone() }) {
            eprintln!("tessera-ui: notifications: can't serve {PATH}: {e}");
            return;
        }
        // Signals back to the apps, from the part
        let c2 = conn.clone();
        std::thread::spawn(move || {
            while let Ok(o) = out_rx.recv() {
                let r = match o {
                    Out::Closed(id, why) => c2.emit_signal(None::<&str>, PATH, NAME, "NotificationClosed", &(id, why)),
                    Out::Action(id, key) => c2.emit_signal(None::<&str>, PATH, NAME, "ActionInvoked", &(id, key.as_str())),
                    Out::Replied(id, text) => c2.emit_signal(None::<&str>, PATH, NAME, "NotificationReplied", &(id, text.as_str())),
                };
                if let Err(e) = r {
                    eprintln!("tessera-ui: notifications: signal: {e}");
                }
            }
        });
        // The name: watched before it's asked for (a NameAcquired can't slip
        // by), asked for with no flags: queued behind a holder, not taken
        let rule = |member: &'static str| MatchRule::builder().msg_type(zbus::message::Type::Signal).sender("org.freedesktop.DBus").and_then(|b| b.member(member)).and_then(|b| b.arg(0, NAME)).map(|b| b.build());
        let (Ok(acq), Ok(lost)) = (rule("NameAcquired"), rule("NameLost")) else { return };
        let (Ok(mut acquired), Ok(lost)) = (MessageIterator::for_match_rule(acq, &conn, Some(8)), MessageIterator::for_match_rule(lost, &conn, Some(8))) else { return };
        let tx2 = tx.clone();
        std::thread::spawn(move || {
            for m in lost.flatten() {
                if m.body().deserialize::<&str>().is_ok_and(|n| n == NAME) && tx2.send(Msg::Data(Box::new(Event::Owner(false)))).is_err() {
                    return;
                }
            }
        });
        match conn.request_name_with_flags(NAME, Default::default()) {
            Ok(zbus::fdo::RequestNameReply::PrimaryOwner) | Ok(zbus::fdo::RequestNameReply::AlreadyOwner) => {
                let _ = tx.send(Msg::Data(Box::new(Event::Owner(true))));
            }
            Ok(_) => eprintln!("tessera-ui: notifications: {NAME} is held (the QML shell's server?): waiting for it"),
            Err(e) => {
                eprintln!("tessera-ui: notifications: can't ask for {NAME}: {e}");
                return;
            }
        }
        // (queued: the bus hands it over when the holder leaves)
        for m in acquired.by_ref().flatten() {
            if m.body().deserialize::<&str>().is_ok_and(|n| n == NAME) && tx.send(Msg::Data(Box::new(Event::Owner(true)))).is_err() {
                return;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use zbus::zvariant::{Structure, StructureBuilder};

    fn ov(v: Value<'static>) -> OwnedValue {
        OwnedValue::try_from(v).unwrap()
    }

    #[test]
    fn hints() {
        let mut h: HashMap<String, OwnedValue> = HashMap::new();
        h.insert("urgency".into(), ov(Value::U8(2)));
        h.insert("category".into(), ov(Value::from("im.received")));
        h.insert("desktop-entry".into(), ov(Value::from("org.example.Chat")));
        h.insert("image-path".into(), ov(Value::from("file:///tmp/a.png")));
        h.insert("resident".into(), ov(Value::Bool(true)));
        h.insert("transient".into(), ov(Value::I32(1)));
        h.insert("value".into(), ov(Value::I32(40)));
        h.insert("x-kde-reply-placeholder-text".into(), ov(Value::from("Reply to Ann")));
        let x = hints_from(&h);
        assert_eq!(x.urgency, Some(2));
        assert_eq!(x.category, "im.received");
        assert_eq!(x.desktop_entry, "org.example.Chat");
        assert_eq!(x.image_path, "/tmp/a.png");
        assert!(x.resident && x.transient && !x.suppress_sound);
        assert_eq!(x.value, Some(40));
        assert_eq!(x.reply_placeholder, "Reply to Ann");
        // an int urgency (some apps), the old spelling of image-path, nothing
        let mut h: HashMap<String, OwnedValue> = HashMap::new();
        h.insert("urgency".into(), ov(Value::U32(7)));
        h.insert("image_path".into(), ov(Value::from("/p.png")));
        let x = hints_from(&h);
        assert_eq!((x.urgency, x.image_path.as_str()), (Some(2), "/p.png"));
        assert_eq!(hints_from(&HashMap::new()), Hints::default());
    }

    fn picture(w: i32, h: i32, stride: i32, alpha: bool, ch: i32, data: Vec<u8>) -> HashMap<String, OwnedValue> {
        let s: Structure = StructureBuilder::new().add_field(w).add_field(h).add_field(stride).add_field(alpha).add_field(8i32).add_field(ch).add_field(data).build().unwrap();
        let mut m = HashMap::new();
        m.insert("image-data".to_string(), ov(Value::Structure(s)));
        m
    }

    #[test]
    fn pixels() {
        // 2x1 RGB with a padded row (stride 8)
        let (w, h, px) = image_data(&picture(2, 1, 8, false, 3, vec![1, 2, 3, 4, 5, 6, 0, 0])).unwrap();
        assert_eq!((w, h), (2, 1));
        assert_eq!(px, vec![1, 2, 3, 255, 4, 5, 6, 255]);
        let (_, _, px) = image_data(&picture(1, 2, 4, true, 4, vec![9, 8, 7, 6, 5, 4, 3, 2])).unwrap();
        assert_eq!(px, vec![9, 8, 7, 6, 5, 4, 3, 2]);
        // too short, or nonsense: nothing
        assert!(image_data(&picture(4, 4, 16, true, 4, vec![0; 10])).is_none());
        assert!(rgba(1, 1, 4, true, 3, 4, &[0; 4]).is_none());
        assert!(rgba(0, 1, 4, true, 8, 4, &[0; 4]).is_none());
    }
}

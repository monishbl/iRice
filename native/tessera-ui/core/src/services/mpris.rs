//! Media players (MPRIS) on the session bus: every `org.mpris.MediaPlayer2.*`
//! name, its track and whether it plays (services::mpris::Players). Which
//! one the shell shows follows shell/services/Media.qml: the newest one
//! that's playing, else the newest. A thread watches the players' property
//! changes and names coming and going, and sends a fresh list after each
//! burst. `control(bus, "PlayPause")` and friends act on a player.

use crate::Msg;
use smithay_client_toolkit::reexports::calloop::channel::Sender;
use std::collections::HashMap;
use std::sync::mpsc;
use std::time::Duration;
use zbus::blocking::{proxy::Builder, Connection, MessageIterator, Proxy};
use zbus::proxy::CacheProperties;
use zbus::zvariant::{OwnedValue, Value};
use zbus::MatchRule;

const PREFIX: &str = "org.mpris.MediaPlayer2.";
const PATH: &str = "/org/mpris/MediaPlayer2";
const IFACE: &str = "org.mpris.MediaPlayer2.Player";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Track {
    pub title: String,
    pub artist: String,
    pub album: String,
    /// mpris:artUrl (file://, http(s)://, or "")
    pub art_url: String,
    /// xesam:url (a video's page)
    pub url: String,
    /// microseconds, 0 unknown
    pub length: i64,
    /// mpris:trackid (an object path): SetPosition needs it
    pub id: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Player {
    /// its bus name (org.mpris.MediaPlayer2.spotify)
    pub bus: String,
    /// what it calls itself (Identity: "Spotify")
    pub identity: String,
    pub track: Track,
    pub playing: bool,
    pub can_next: bool,
    pub can_prev: bool,
    pub can_play: bool,
}

/// Every player, oldest first (the order they appeared in)
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Players {
    pub list: Vec<Player>,
}

impl Players {
    /// The one worth showing (Media.qml): the newest that plays, else the newest
    pub fn chosen(&self) -> Option<&Player> {
        self.list.iter().rev().find(|p| p.playing).or_else(|| self.list.last())
    }
}

fn text(v: Option<&OwnedValue>) -> String {
    match v.map(|v| &**v) {
        Some(Value::Str(s)) => s.to_string(),
        Some(Value::ObjectPath(p)) => p.to_string(),
        Some(Value::Value(b)) => text_of(b),
        _ => String::new(),
    }
}
fn text_of(v: &Value) -> String {
    match v {
        Value::Str(s) => s.to_string(),
        Value::Value(b) => text_of(b),
        _ => String::new(),
    }
}

/// A track from a player's Metadata (a{sv}): xesam:artist is a list (joined
/// with ", "), though some players send a plain string
pub fn track_from(meta: &HashMap<String, OwnedValue>) -> Track {
    let artist = match meta.get("xesam:artist").map(|v| &**v) {
        Some(Value::Array(a)) => a.iter().map(text_of).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(", "),
        Some(v) => text_of(v),
        None => String::new(),
    };
    let length = match meta.get("mpris:length").map(|v| &**v) {
        Some(Value::I64(n)) => *n,
        Some(Value::U64(n)) => *n as i64,
        Some(Value::I32(n)) => *n as i64,
        Some(Value::U32(n)) => *n as i64,
        Some(Value::F64(n)) => *n as i64,
        _ => 0,
    };
    Track {
        title: text(meta.get("xesam:title")),
        artist,
        album: text(meta.get("xesam:album")),
        art_url: text(meta.get("mpris:artUrl")),
        url: text(meta.get("xesam:url")),
        length,
        id: match meta.get("mpris:trackid").map(|v| &**v) {
            Some(Value::ObjectPath(p)) => p.to_string(),
            Some(v) => text_of(v),
            None => String::new(),
        },
    }
}

fn proxy<'a>(conn: &Connection, dest: &'a str, iface: &'a str) -> zbus::Result<Proxy<'a>> {
    Builder::new(conn).destination(dest)?.path(PATH)?.interface(iface)?.cache_properties(CacheProperties::No).build()
}

fn read_player(conn: &Connection, bus: &str) -> zbus::Result<Player> {
    let p = proxy(conn, bus, IFACE)?;
    let meta: HashMap<String, OwnedValue> = p.get_property("Metadata").unwrap_or_default();
    let status: String = p.get_property("PlaybackStatus").unwrap_or_default();
    let identity: String = proxy(conn, bus, "org.mpris.MediaPlayer2").and_then(|r| r.get_property("Identity")).unwrap_or_default();
    Ok(Player {
        bus: bus.to_string(),
        identity,
        track: track_from(&meta),
        playing: status == "Playing",
        can_next: p.get_property("CanGoNext").unwrap_or(false),
        can_prev: p.get_property("CanGoPrevious").unwrap_or(false),
        can_play: p.get_property("CanPlay").unwrap_or(true),
    })
}

fn names(conn: &Connection) -> zbus::Result<Vec<String>> {
    let m = conn.call_method(Some("org.freedesktop.DBus"), "/org/freedesktop/DBus", Some("org.freedesktop.DBus"), "ListNames", &())?;
    let all: Vec<String> = m.body().deserialize()?;
    Ok(all.into_iter().filter(|n| n.starts_with(PREFIX)).collect())
}

/// Keep `order` (names in the order first seen) in step with what's there
pub fn keep_order(order: &mut Vec<String>, now: &[String]) {
    order.retain(|n| now.contains(n));
    for n in now {
        if !order.contains(n) {
            order.push(n.clone());
        }
    }
}

fn watch_rule(conn: &Connection, rule: MatchRule<'static>, dirty: mpsc::Sender<()>) -> zbus::Result<()> {
    let mut it = MessageIterator::for_match_rule(rule, conn, Some(64))?;
    std::thread::spawn(move || {
        while it.next().is_some() {
            if dirty.send(()).is_err() {
                return;
            }
        }
    });
    Ok(())
}

pub fn spawn(tx: Sender<Msg>) {
    // (the art cache pruned once a run, on the pool: audit #53)
    crate::work::spawn(|| prune_art(&art_dir(), ART_KEEP_FILES, ART_KEEP_BYTES));
    std::thread::spawn(move || loop {
        let run = || -> zbus::Result<()> {
            let conn = Connection::session()?;
            let (dirty, wake) = mpsc::channel::<()>();
            watch_rule(
                &conn,
                MatchRule::builder().msg_type(zbus::message::Type::Signal).interface("org.freedesktop.DBus.Properties")?.member("PropertiesChanged")?.path(PATH)?.build(),
                dirty.clone(),
            )?;
            watch_rule(
                &conn,
                MatchRule::builder().msg_type(zbus::message::Type::Signal).sender("org.freedesktop.DBus")?.interface("org.freedesktop.DBus")?.member("NameOwnerChanged")?.arg0ns(PREFIX.trim_end_matches('.'))?.build(),
                dirty,
            )?;
            let mut order: Vec<String> = Vec::new();
            let mut last: Option<Players> = None;
            loop {
                let now = names(&conn)?;
                keep_order(&mut order, &now);
                let list = order.iter().filter_map(|b| read_player(&conn, b).ok()).collect();
                let players = Players { list };
                if last.as_ref() != Some(&players) {
                    tx.send(Msg::Data(Box::new(players.clone()))).map_err(|_| zbus::Error::Failure("ui gone".into()))?;
                    last = Some(players);
                }
                // a burst of changes (a new track: title, art, length…) as one reading
                if wake.recv().is_err() {
                    return Ok(());
                }
                while wake.recv_timeout(Duration::from_millis(40)).is_ok() {}
            }
        };
        if let Err(zbus::Error::Failure(_)) = run() {
            return;
        }
        std::thread::sleep(Duration::from_secs(3));
    });
}

/// PlayPause, Next, Previous… on a player (a thread of its own)
pub fn control(bus: &str, method: &'static str) {
    let bus = bus.to_string();
    std::thread::spawn(move || {
        let Some(conn) = session() else { return };
        let _ = checked(conn.call_method(Some(bus.as_str()), PATH, Some(IFACE), method, &()));
    });
}

static CONN: std::sync::Mutex<Option<Connection>> = std::sync::Mutex::new(None);

/// One session bus connection for the calls below (made once, made again
/// after it fails: `checked`). Over D-Bus itself, not busctl: busctl is systemd's
/// (not on every system), and prints its numbers in your language ("0,5")
fn session() -> Option<Connection> {
    let mut c = CONN.lock().unwrap_or_else(|e| e.into_inner());
    if c.is_none() {
        *c = Connection::session().ok();
    }
    c.clone()
}

/// The connection is gone (its socket closed: an I/O error), not only
/// this call refused
fn is_dead(e: &zbus::Error) -> bool {
    matches!(e, zbus::Error::InputOutput(_))
}

/// A call's result: a dead connection is dropped, so the next call makes a
/// new one (media keys and the player's controls kept using the dead one
/// until the shell restarted, audit #113)
fn checked<T>(r: zbus::Result<T>) -> zbus::Result<T> {
    if let Err(e) = &r {
        if is_dead(e) {
            *CONN.lock().unwrap_or_else(|e| e.into_inner()) = None;
        }
    }
    r
}

/// A number property of the player (Position in µs, Volume 0..1);
/// blocking: call on a thread
pub fn number(bus: &str, prop: &str) -> Option<f64> {
    let conn = session()?;
    let v: OwnedValue = checked(checked(proxy(&conn, bus, IFACE)).ok()?.get_property(prop)).ok()?;
    number_of(&v)
}

/// An i64 / u64 / f64 / i32 property value as a number
pub fn number_of(v: &Value) -> Option<f64> {
    match v {
        Value::I64(n) => Some(*n as f64),
        Value::U64(n) => Some(*n as f64),
        Value::I32(n) => Some(*n as f64),
        Value::U32(n) => Some(*n as f64),
        Value::F64(n) => Some(*n),
        Value::Value(inner) => number_of(inner),
        _ => None,
    }
}

/// The player's own volume, 0..1 (a thread of its own)
pub fn set_volume(bus: &str, v: f64) {
    let bus = bus.to_string();
    std::thread::spawn(move || {
        let Some(conn) = session() else { return };
        let _ = checked(conn.call_method(Some(bus.as_str()), PATH, Some("org.freedesktop.DBus.Properties"), "Set", &(IFACE, "Volume", Value::F64(v.clamp(0.0, 1.0)))));
    });
}

/// Bring the player's window forward (org.mpris.MediaPlayer2.Raise)
pub fn raise(bus: &str) {
    let bus = bus.to_string();
    std::thread::spawn(move || {
        let Some(conn) = session() else { return };
        let _ = checked(conn.call_method(Some(bus.as_str()), PATH, Some("org.mpris.MediaPlayer2"), "Raise", &()));
    });
}

/// How to move the playhead: SetPosition (absolute) when the track has an
/// id, else Seek by the difference from where it is
#[derive(Debug, PartialEq)]
pub enum SeekTo {
    /// SetPosition(track id, µs)
    At(String, i64),
    /// Seek(offset µs)
    By(i64),
}

pub fn seek_plan(track: &Track, target: i64, now: i64) -> SeekTo {
    let target = target.clamp(0, track.length.max(0));
    if !track.id.is_empty() && track.id.starts_with('/') && !track.id.ends_with("/NoTrack") {
        SeekTo::At(track.id.clone(), target)
    } else {
        SeekTo::By(target - now)
    }
}

/// Seek `bus`'s player to `target` µs into `track` (a thread of its own);
/// `now`: where it is, µs. A missing `now` (< 0) is read first
pub fn seek(bus: &str, track: &Track, target: i64, now: i64) {
    let (bus, track) = (bus.to_string(), track.clone());
    std::thread::spawn(move || {
        let Some(conn) = session() else { return };
        let now = if now >= 0 { now } else { number(&bus, "Position").unwrap_or(0.0) as i64 };
        let _ = checked(match seek_plan(&track, target, now) {
            SeekTo::At(id, t) => match zbus::zvariant::ObjectPath::try_from(id.as_str()) {
                Ok(p) => conn.call_method(Some(bus.as_str()), PATH, Some(IFACE), "SetPosition", &(p, t)).map(|_| ()),
                Err(_) => Ok(()),
            },
            SeekTo::By(d) => conn.call_method(Some(bus.as_str()), PATH, Some(IFACE), "Seek", &(d,)).map(|_| ()),
        });
    });
}

/// A local picture for an art address: file:// as is; http(s) fetched once
/// into the cache (curl, as the shell's own scripts), then that file.
/// Blocking (a fetch takes up to 8 s): call on a thread; on the loop,
/// `art_now` says whether it's needed
pub fn art_file(url: &str) -> Option<std::path::PathBuf> {
    art_path(url).map(named_by_kind)
}

/// What `art_now` knows without waiting
#[derive(Debug, PartialEq)]
pub enum ArtNow {
    /// the answer, at once (a picture file named as one; or no art at all)
    Ready(Option<std::path::PathBuf>),
    /// `art_file` is needed, on a thread (web art, a browser's file without
    /// a picture's name)
    Fetch,
}

/// An art address as far as the loop can tell it without blocking (no
/// fetch, no reading of files): menubar's pictures. A fetch on the loop
/// froze the whole shell for up to 8 s on new web art (audit #12)
pub fn art_now(url: &str) -> ArtNow {
    if let Some(p) = url.strip_prefix("file://") {
        let p = std::path::PathBuf::from(percent_decode(p.split('?').next().unwrap_or(p)));
        return if picture_named(&p) { ArtNow::Ready(Some(p)) } else { ArtNow::Fetch };
    }
    if url.starts_with("http://") || url.starts_with("https://") {
        ArtNow::Fetch
    } else {
        ArtNow::Ready(None)
    }
}

/// Named as a picture (Slint loads by the extension)
fn picture_named(p: &std::path::Path) -> bool {
    p.extension().and_then(|e| e.to_str()).is_some_and(|e| ["png", "jpg", "jpeg", "webp", "gif", "bmp", "svg", "tif", "tiff", "ico"].contains(&e.to_ascii_lowercase().as_str()))
}

/// The art cache's size: the newest this many files, this many bytes in all
const ART_KEEP_FILES: usize = 300;
const ART_KEEP_BYTES: u64 = 64 << 20;

/// Old art goes: every new web picture and every browser's art file (a new
/// one per track) was kept for good. The newest stay, up to `files` and
/// `bytes`; half-fetched files (.part) of an earlier run go too
pub fn prune_art(dir: &std::path::Path, files: usize, bytes: u64) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut all: Vec<(std::time::SystemTime, u64, std::path::PathBuf)> = rd
        .flatten()
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            m.is_file().then(|| (m.modified().unwrap_or(std::time::UNIX_EPOCH), m.len(), e.path()))
        })
        .collect();
    // (newest first)
    all.sort_by(|a, b| b.0.cmp(&a.0));
    let (mut total, mut kept) = (0u64, 0usize);
    let hour_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
    for (t, len, p) in all {
        // (a .part an hour old is an earlier run's; a newer one may be
        // fetching now)
        if p.extension().is_some_and(|e| e == "part") {
            if t < hour_ago {
                let _ = std::fs::remove_file(&p);
            }
            continue;
        }
        total = total.saturating_add(len);
        kept += 1;
        if kept > files || total > bytes {
            let _ = std::fs::remove_file(&p);
        }
    }
}

/// Pictures load by their extension (Slint's image::open): the cache's files
/// had none, nor have browsers' art files (/tmp/.org.chromium…, firefox's),
/// so no art showed. One without a picture's extension gets a copy in the
/// cache named for what it is (read from its first bytes)
fn named_by_kind(p: std::path::PathBuf) -> std::path::PathBuf {
    if picture_named(&p) {
        return p;
    }
    let Ok(bytes) = std::fs::read(&p) else { return p };
    let Ok(kind) = image::guess_format(&bytes) else { return p };
    let Some(ext) = kind.extensions_str().first() else { return p };
    let stamp = std::fs::metadata(&p).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos());
    let out = art_dir().join(format!("{:016x}.{ext}", fnv(&format!("{}\n{}\n{stamp}", p.display(), bytes.len()))));
    if out.exists() || (std::fs::create_dir_all(art_dir()).is_ok() && std::fs::write(&out, &bytes).is_ok()) {
        return out;
    }
    p
}

fn art_dir() -> std::path::PathBuf {
    let base = crate::util::xdg("XDG_CACHE_HOME", ".cache");
    base.join("tessera/art")
}

fn art_path(url: &str) -> Option<std::path::PathBuf> {
    if let Some(p) = url.strip_prefix("file://") {
        let p = percent_decode(p.split('?').next().unwrap_or(p));
        return Some(std::path::PathBuf::from(p));
    }
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return None;
    }
    let dir = art_dir();
    let file = dir.join(format!("{:016x}", fnv(url)));
    if !file.exists() {
        if crate::work::on_ui_thread() {
            eprintln!("tessera-ui: art fetched on the UI thread (mpris::art_file): use art_now and a thread");
        }
        let _ = std::fs::create_dir_all(&dir);
        let tmp = file.with_extension("part");
        let ok = std::process::Command::new("curl").args(["-sfL", "--max-time", "8", "-o"]).arg(&tmp).arg(url).status().is_ok_and(|s| s.success());
        if !ok || std::fs::rename(&tmp, &file).is_err() {
            let _ = std::fs::remove_file(&tmp);
            return None;
        }
    }
    Some(file)
}

fn fnv(s: &str) -> u64 {
    s.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3))
}

/// %xx in a file address
pub fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Some(v) = std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    #[test]
    fn seeking_is_absolute_when_the_track_has_an_id() {
        let t = super::Track { length: 200_000_000, id: "/org/mpd/Tracks/7".into(), ..Default::default() };
        assert_eq!(super::seek_plan(&t, 30_000_000, 90_000_000), super::SeekTo::At("/org/mpd/Tracks/7".into(), 30_000_000));
        // (past the end: the end)
        assert_eq!(super::seek_plan(&t, 900_000_000, 0), super::SeekTo::At("/org/mpd/Tracks/7".into(), 200_000_000));
        // (no id, or NoTrack: a relative Seek, backwards too)
        let n = super::Track { length: 200_000_000, id: "/org/mpris/MediaPlayer2/TrackList/NoTrack".into(), ..Default::default() };
        assert_eq!(super::seek_plan(&n, 30_000_000, 90_000_000), super::SeekTo::By(-60_000_000));
        let none = super::Track { length: 200_000_000, ..Default::default() };
        assert_eq!(super::seek_plan(&none, 50_000_000, 10_000_000), super::SeekTo::By(40_000_000));
    }

    #[test]
    fn property_numbers_whatever_their_type() {
        use zbus::zvariant::Value;
        assert_eq!(super::number_of(&Value::I64(12_500_000)), Some(12_500_000.0));
        assert_eq!(super::number_of(&Value::F64(0.5)), Some(0.5));
        assert_eq!(super::number_of(&Value::Value(Box::new(Value::F64(0.25)))), Some(0.25));
        assert_eq!(super::number_of(&Value::from("0,5")), None);
    }

    use super::*;

    fn ov(v: Value<'static>) -> OwnedValue {
        OwnedValue::try_from(v).unwrap()
    }

    #[test]
    fn a_track_from_metadata() {
        let mut m = HashMap::new();
        m.insert("xesam:title".to_string(), ov(Value::from("Midnight City")));
        m.insert("xesam:artist".to_string(), ov(Value::from(vec!["M83", "Anthony Gonzalez"])));
        m.insert("xesam:album".to_string(), ov(Value::from("Hurry Up, We're Dreaming")));
        m.insert("mpris:artUrl".to_string(), ov(Value::from("file:///tmp/art%20one.png")));
        m.insert("mpris:length".to_string(), ov(Value::from(243_000_000i64)));
        let t = track_from(&m);
        assert_eq!(t.title, "Midnight City");
        assert_eq!(t.artist, "M83, Anthony Gonzalez");
        assert_eq!(t.length, 243_000_000);
        assert_eq!(art_file(&t.art_url), Some(std::path::PathBuf::from("/tmp/art one.png")));
        // a plain string for the artist, an unsigned length, nothing else
        let mut m = HashMap::new();
        m.insert("xesam:artist".to_string(), ov(Value::from("Solo")));
        m.insert("mpris:length".to_string(), ov(Value::from(5u64)));
        let t = track_from(&m);
        assert_eq!((t.artist.as_str(), t.length, t.title.as_str()), ("Solo", 5, ""));
        assert_eq!(track_from(&HashMap::new()), Track::default());
    }

    fn p(bus: &str, playing: bool) -> Player {
        Player { bus: bus.into(), playing, ..Default::default() }
    }

    #[test]
    fn the_newest_playing_one_is_shown() {
        // (a browser tab's old session left behind, still "playing", is older)
        let ps = Players { list: vec![p("ghost", true), p("spotify", true), p("vlc", false)] };
        assert_eq!(ps.chosen().unwrap().bus, "spotify");
        let ps = Players { list: vec![p("a", false), p("b", false)] };
        assert_eq!(ps.chosen().unwrap().bus, "b");
        assert!(Players::default().chosen().is_none());
    }

    #[test]
    fn players_keep_the_order_they_came_in() {
        let mut order = vec!["b".to_string(), "a".to_string()];
        keep_order(&mut order, &["a".into(), "c".into(), "b".into()]);
        assert_eq!(order, vec!["b", "a", "c"]);
        keep_order(&mut order, &["c".into()]);
        assert_eq!(order, vec!["c"]);
    }

    #[test]
    fn only_local_and_web_art() {
        assert_eq!(art_file(""), None);
        assert_eq!(art_file("data:image/png;base64,xx"), None);
        assert_eq!(art_file("file:///a/b.png?track=3"), Some(std::path::PathBuf::from("/a/b.png")));
        // (what the loop may know at once, without a fetch or a read)
        assert_eq!(art_now("file:///a/b%20c.JPG?x=1"), ArtNow::Ready(Some(std::path::PathBuf::from("/a/b c.JPG"))));
        assert_eq!(art_now("file:///tmp/.org.chromium.Chromium.abc"), ArtNow::Fetch);
        assert_eq!(art_now("https://i.scdn.co/image/ab67"), ArtNow::Fetch);
        assert_eq!(art_now(""), ArtNow::Ready(None));
        assert_eq!(art_now("data:image/png;base64,xx"), ArtNow::Ready(None));
    }

    #[test]
    fn the_art_cache_keeps_only_the_newest() {
        let dir = std::env::temp_dir().join(format!("tessera-art-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let at = |s: u64| std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000 + s);
        for (name, age, len) in [("old", 1, 10), ("mid", 2, 10), ("new", 3, 10), ("x.part", 4, 1)] {
            let f = std::fs::File::create(dir.join(name)).unwrap();
            f.set_len(len).unwrap();
            f.set_modified(at(age)).unwrap();
        }
        // (two files at most: the oldest goes, and the half-fetched one)
        prune_art(&dir, 2, 1 << 20);
        let left = |d: &std::path::Path| {
            let mut v: Vec<String> = std::fs::read_dir(d).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
            v.sort();
            v
        };
        assert_eq!(left(&dir), vec!["mid", "new"]);
        // (by size: 15 bytes keep only the newest)
        prune_art(&dir, 10, 15);
        assert_eq!(left(&dir), vec!["new"]);
        // (no folder: nothing to do)
        prune_art(&dir.join("none"), 1, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_dead_connection_is_made_again() {
        // (an I/O error: the socket is gone, the next call connects again;
        // a call the player refused keeps the connection)
        assert!(is_dead(&zbus::Error::InputOutput(std::sync::Arc::new(std::io::Error::other("closed")))));
        assert!(!is_dead(&zbus::Error::Failure("no".into())));
    }
}

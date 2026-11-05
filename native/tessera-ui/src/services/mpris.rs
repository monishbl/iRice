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
        let Ok(conn) = Connection::session() else { return };
        let _ = conn.call_method(Some(bus.as_str()), PATH, Some(IFACE), method, &());
    });
}

/// A local picture for an art address: file:// as is; http(s) fetched once
/// into the cache (curl, as the shell's own scripts), then that file
pub fn art_file(url: &str) -> Option<std::path::PathBuf> {
    if let Some(p) = url.strip_prefix("file://") {
        let p = percent_decode(p.split('?').next().unwrap_or(p));
        return Some(std::path::PathBuf::from(p));
    }
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return None;
    }
    let base = std::env::var_os("XDG_CACHE_HOME").map(std::path::PathBuf::from).unwrap_or_else(|| std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".cache"));
    let dir = base.join("tessera/art");
    let file = dir.join(format!("{:016x}", fnv(url)));
    if !file.exists() {
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
    }
}

//! Media.qml: the player worth showing (the one you picked, else the newest
//! that plays, else the most recent) and where its picture comes from: what
//! the player says (mpris:artUrl), what it said earlier for this track
//! (browsers drop the art now and then), a YouTube video's own thumbnail from
//! its address. core/src/services/mpris.rs reads the players; this keeps the
//! pick and the art memory, and says them as the state field `media`
//! ({ bus, title, artist, playing, art: [places in order] }). A place whose
//! picture fails to load is skipped by whoever draws it (Media's probe).
//!
//! Calls: media [previous | next | …] (the desktop widget's buttons: anything
//! else toggles play), media.pin [bus | ""].

use serde_json::{json, Value};
use std::collections::VecDeque;
use tessera_core::host::Host;
use tessera_core::services::mpris::{self, Player, Players};

#[derive(Default)]
pub struct Media {
    pub pinned: String,
    /// "title|artist" -> the address it last came with (the last 20 tracks)
    last_art: VecDeque<(String, String)>,
}

/// A YouTube video's id from its address ("" when it isn't one)
pub fn youtube_id(url: &str) -> String {
    let rest = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://")).unwrap_or("");
    let rest = ["www.", "m.", "music."].iter().find_map(|p| rest.strip_prefix(p)).unwrap_or(rest);
    let id = if let Some(r) = rest.strip_prefix("youtu.be/") {
        r
    } else if let Some(r) = rest.strip_prefix("youtube.com/") {
        if let Some(q) = r.strip_prefix("watch?") {
            q.split('&').find_map(|kv| kv.strip_prefix("v=")).unwrap_or("")
        } else {
            r.strip_prefix("shorts/").or_else(|| r.strip_prefix("live/")).unwrap_or("")
        }
    } else {
        ""
    };
    let id: String = id.chars().take(11).collect();
    if id.len() == 11 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') { id } else { String::new() }
}

fn track_key(p: &Player) -> String {
    if p.track.title.is_empty() { String::new() } else { format!("{}|{}", p.track.title, p.track.artist) }
}

impl Media {
    /// Media.player
    pub fn player<'a>(&self, ps: &'a Players) -> Option<&'a Player> {
        if !self.pinned.is_empty() {
            if let Some(p) = ps.list.iter().find(|p| p.bus == self.pinned) {
                return Some(p);
            }
        }
        ps.chosen()
    }

    /// Media.artSources
    pub fn art_sources(&self, p: &Player) -> Vec<String> {
        let mut out = vec![];
        if !p.track.art_url.is_empty() {
            out.push(p.track.art_url.clone());
        }
        let kept = self.last_art.iter().find(|(k, _)| *k == track_key(p)).map(|(_, u)| u.clone()).unwrap_or_default();
        if !kept.is_empty() && kept != p.track.art_url {
            out.push(kept);
        }
        let yt = youtube_id(&p.track.url);
        if !yt.is_empty() {
            out.push(format!("https://i.ytimg.com/vi/{yt}/maxresdefault.jpg"));
            out.push(format!("https://i.ytimg.com/vi/{yt}/hqdefault.jpg"));
        }
        out
    }

    fn remember(&mut self, p: &Player) {
        let (k, u) = (track_key(p), p.track.art_url.clone());
        if k.is_empty() || u.is_empty() || self.last_art.iter().any(|(kk, uu)| *kk == k && *uu == u) {
            return;
        }
        self.last_art.retain(|(kk, _)| *kk != k);
        self.last_art.push_back((k, u));
        while self.last_art.len() > 20 {
            self.last_art.pop_front();
        }
    }

    pub fn start(&mut self, h: &mut Host) {
        h.st.shell.want("mpris");
    }

    /// The tick: remember the art, say the player
    pub fn publish(&mut self, h: &mut Host) {
        let Some(ps) = h.st.shell.get::<Players>().cloned() else { return };
        let Some(p) = self.player(&ps).cloned() else {
            h.set_state("media", Value::Null);
            return;
        };
        self.remember(&p);
        h.set_state("media", json!({"bus": p.bus, "title": p.track.title, "artist": p.track.artist, "playing": p.playing, "art": self.art_sources(&p)}));
    }

    pub fn call(&mut self, h: &mut Host, call: &str, args: &Value) {
        let a = super::common::arg_str(args, 0);
        if call == "media.pin" {
            self.pinned = a;
            return;
        }
        let Some(ps) = h.st.shell.get::<Players>().cloned() else { return };
        let Some(p) = self.player(&ps) else { return };
        let method = match a.as_str() {
            "previous" => "Previous",
            "next" => "Next",
            _ => "PlayPause",
        };
        mpris::control(&p.bus, method);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn youtube_addresses() {
        assert_eq!(youtube_id("https://www.youtube.com/watch?v=dQw4w9WgXcQ"), "dQw4w9WgXcQ");
        assert_eq!(youtube_id("https://music.youtube.com/watch?feature=x&v=dQw4w9WgXcQ"), "dQw4w9WgXcQ");
        assert_eq!(youtube_id("https://youtu.be/dQw4w9WgXcQ?t=3"), "dQw4w9WgXcQ");
        assert_eq!(youtube_id("https://example.com/watch?v=dQw4w9WgXcQ"), "");
    }

    #[test]
    fn art_falls_back() {
        let mut m = Media::default();
        let mut p = Player::default();
        p.track.title = "T".into();
        p.track.artist = "A".into();
        p.track.art_url = "file:///a.png".into();
        m.remember(&p);
        p.track.art_url.clear();
        p.track.url = "https://youtu.be/dQw4w9WgXcQ".into();
        let s = m.art_sources(&p);
        assert_eq!(s.len(), 3);
        assert_eq!(s[0], "file:///a.png");
    }
}

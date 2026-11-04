//! `pactl subscribe`, spoken directly: PulseAudio's native protocol (which
//! PipeWire's pipewire-pulse speaks too) over its Unix socket, just enough
//! of it to authenticate, name ourselves and subscribe. Reported as
//!   {"src":"pulse","kind":"change","facility":"sink","index":42}
//! (pactl's "Event 'change' on sink #42"), and {"kind":"up"} / {"kind":"down"}
//! as the sound server comes and goes; it's looked for again after 5 s,
//! then 10, 20... up to 5 minutes (back to 5 s after a run of a minute).
//! A PULSE_SERVER that isn't a Unix socket: {"kind":"unsupported"}, and the
//! shell runs pactl instead.

use crate::{emit, stop::Stop};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const VERSION: u32 = 32;
const AUTH: u32 = 8;
const SET_CLIENT_NAME: u32 = 9;
const SUBSCRIBE: u32 = 35;
const SUBSCRIBE_EVENT: u32 = 66;
const REPLY: u32 = 2;
const MASK_ALL: u32 = 0x02ff;

pub fn run(stop: Stop, _args: Value) {
    let path = match server(std::env::var("PULSE_SERVER").ok().as_deref()) {
        Some(p) => p,
        None => {
            emit(json!({"src": "pulse", "kind": "unsupported", "error": "PULSE_SERVER isn't a Unix socket"}));
            return;
        }
    };
    let mut quick_ends = 0;
    loop {
        let started = Instant::now();
        let up = session(&stop, &path);
        if stop.is_set() {
            return;
        }
        if up {
            emit(json!({"src": "pulse", "kind": "down"}));
        }
        if started.elapsed() >= Duration::from_secs(60) {
            quick_ends = 0;
        }
        let wait = Duration::from_secs(5 * 2u64.pow(quick_ends.min(6))).min(Duration::from_secs(300));
        quick_ends += 1;
        if !stop.sleep(wait) {
            return;
        }
    }
}

/// The socket to use: PULSE_SERVER's first Unix one ("unix:/path", "/path",
/// either after a "{machine id}"), else $XDG_RUNTIME_DIR/pulse/native.
/// None when PULSE_SERVER names only other kinds (tcp:).
pub fn server(env: Option<&str>) -> Option<PathBuf> {
    if let Some(list) = env.filter(|s| !s.trim().is_empty()) {
        for s in list.split_whitespace() {
            let s = if s.starts_with('{') { s.split_once('}').map(|x| x.1).unwrap_or("") } else { s };
            let s = s.strip_prefix("unix:").unwrap_or(s);
            if s.starts_with('/') {
                return Some(PathBuf::from(s));
            }
        }
        return None;
    }
    let rt = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| format!("/run/user/{}", unsafe { libc::getuid() }));
    Some(PathBuf::from(rt).join("pulse/native"))
}

// One connection, for as long as it lasts: whether it got as far as up
fn session(stop: &Stop, path: &PathBuf) -> bool {
    let Ok(mut s) = UnixStream::connect(path) else { return false };
    let _ = s.set_read_timeout(Some(Duration::from_secs(5)));
    let mut tag = 0;
    let mut ask = |s: &mut UnixStream, cmd: u32, body: Tags| -> Option<Vec<u8>> {
        tag += 1;
        let p = Tags::new().u32(cmd).u32(tag).raw(&body.0);
        s.write_all(&packet(&p.0)).ok()?;
        // (the answer to this one; nothing else comes before we subscribe)
        let reply = read_packet(s)?;
        let mut r = Reader::new(&reply);
        (r.u32()? == REPLY && r.u32()? == tag).then_some(reply)
    };
    // The cookie, when there is one; the server also knows us by our uid
    // (SO_PASSCRED), which is what pipewire-pulse and a per-user PulseAudio use
    if ask(&mut s, AUTH, Tags::new().u32(VERSION).arbitrary(&cookie())).is_none() {
        return false;
    }
    let props = Tags::new().proplist(&[("application.name", "irice-events"), ("application.process.binary", "irice-events")]);
    if ask(&mut s, SET_CLIENT_NAME, props).is_none() {
        return false;
    }
    if ask(&mut s, SUBSCRIBE, Tags::new().u32(MASK_ALL)).is_none() {
        return false;
    }
    let _ = s.set_read_timeout(None);
    emit(json!({"src": "pulse", "kind": "up"}));
    loop {
        if stop.wait(&[s.as_raw_fd()], None).is_none() {
            return true;
        }
        let Some(p) = read_packet(&mut s) else { return true };
        if let Some(e) = event(&p) {
            emit(e);
        }
    }
}

fn cookie() -> Vec<u8> {
    let home = std::env::var("HOME").unwrap_or_default();
    let config = std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| format!("{home}/.config"));
    let paths = [std::env::var("PULSE_COOKIE").unwrap_or_default(), format!("{config}/pulse/cookie"), format!("{home}/.pulse-cookie")];
    for p in paths.iter().filter(|p| !p.is_empty()) {
        if let Ok(c) = std::fs::read(p) {
            if c.len() >= 256 {
                return c[..256].to_vec();
            }
        }
    }
    vec![0; 256]
}

/// A subscribe event as pactl prints it, or None for any other packet.
pub fn event(payload: &[u8]) -> Option<Value> {
    let mut r = Reader::new(payload);
    if r.u32()? != SUBSCRIBE_EVENT {
        return None;
    }
    r.u32()?; // (tag: none)
    let e = r.u32()?;
    let index = r.u32()?;
    const FACILITY: [&str; 10] = ["sink", "source", "sink-input", "source-output", "module", "client", "sample-cache", "server", "autoload", "card"];
    let facility = *FACILITY.get((e & 0x0f) as usize)?;
    let kind = match e & 0x30 {
        0x00 => "new",
        0x10 => "change",
        0x20 => "remove",
        _ => return None,
    };
    Some(json!({"src": "pulse", "kind": kind, "facility": facility, "index": index}))
}

/// A packet: its descriptor (payload length, channel -1 for commands,
/// offset, flags), then the payload.
pub fn packet(payload: &[u8]) -> Vec<u8> {
    let mut p = Vec::with_capacity(20 + payload.len());
    for w in [payload.len() as u32, u32::MAX, 0, 0, 0] {
        p.extend(w.to_be_bytes());
    }
    p.extend(payload);
    p
}

fn read_packet(s: &mut UnixStream) -> Option<Vec<u8>> {
    let mut d = [0u8; 20];
    s.read_exact(&mut d).ok()?;
    let len = u32::from_be_bytes(d[..4].try_into().unwrap()) as usize;
    if len > 1 << 20 {
        return None;
    }
    let mut p = vec![0u8; len];
    s.read_exact(&mut p).ok()?;
    Some(p)
}

/// A tagstruct being written: each value after its type's tag.
pub struct Tags(pub Vec<u8>);

impl Tags {
    pub fn new() -> Tags {
        Tags(vec![])
    }
    pub fn u32(mut self, v: u32) -> Tags {
        self.0.push(b'L');
        self.0.extend(v.to_be_bytes());
        self
    }
    pub fn string(mut self, s: &str) -> Tags {
        self.0.push(b't');
        self.0.extend(s.bytes().filter(|&b| b != 0));
        self.0.push(0);
        self
    }
    pub fn arbitrary(mut self, b: &[u8]) -> Tags {
        self.0.push(b'x');
        self.0.extend((b.len() as u32).to_be_bytes());
        self.0.extend(b);
        self
    }
    // key, length, value (with its NUL), ..., then a null string
    pub fn proplist(mut self, props: &[(&str, &str)]) -> Tags {
        self.0.push(b'P');
        for (k, v) in props {
            let mut val = v.as_bytes().to_vec();
            val.push(0);
            self = self.string(k).u32(val.len() as u32).arbitrary(&val);
        }
        self.0.push(b'N');
        self
    }
    fn raw(mut self, b: &[u8]) -> Tags {
        self.0.extend(b);
        self
    }
}

struct Reader<'a> {
    b: &'a [u8],
}

impl<'a> Reader<'a> {
    fn new(b: &'a [u8]) -> Reader<'a> {
        Reader { b }
    }
    fn u32(&mut self) -> Option<u32> {
        if self.b.len() < 5 || self.b[0] != b'L' {
            return None;
        }
        let v = u32::from_be_bytes(self.b[1..5].try_into().unwrap());
        self.b = &self.b[5..];
        Some(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_as_pactl_names_them() {
        let p = Tags::new().u32(SUBSCRIBE_EVENT).u32(u32::MAX).u32(0x10).u32(42).0;
        assert_eq!(event(&p), Some(json!({"src": "pulse", "kind": "change", "facility": "sink", "index": 42})));
        let p = Tags::new().u32(SUBSCRIBE_EVENT).u32(u32::MAX).u32(0x03).u32(7).0;
        assert_eq!(event(&p), Some(json!({"src": "pulse", "kind": "new", "facility": "source-output", "index": 7})));
        let p = Tags::new().u32(SUBSCRIBE_EVENT).u32(u32::MAX).u32(0x29).u32(1).0;
        assert_eq!(event(&p), Some(json!({"src": "pulse", "kind": "remove", "facility": "card", "index": 1})));
        // not an event; a cut-off one; an unknown facility
        assert_eq!(event(&Tags::new().u32(REPLY).u32(3).0), None);
        assert_eq!(event(&Tags::new().u32(SUBSCRIBE_EVENT).u32(u32::MAX).0), None);
        assert_eq!(event(&Tags::new().u32(SUBSCRIBE_EVENT).u32(u32::MAX).u32(0x0c).u32(1).0), None);
    }

    #[test]
    fn packets() {
        let auth = Tags::new().u32(AUTH).u32(1).u32(VERSION).arbitrary(&[7; 4]).0;
        assert_eq!(auth, [b'L', 0, 0, 0, 8, b'L', 0, 0, 0, 1, b'L', 0, 0, 0, 32, b'x', 0, 0, 0, 4, 7, 7, 7, 7]);
        let p = packet(&auth);
        assert_eq!(&p[..4], &(auth.len() as u32).to_be_bytes());
        assert_eq!(&p[4..8], &[0xff; 4]);
        assert_eq!(&p[20..], &auth[..]);
        let props = Tags::new().proplist(&[("a", "bc")]).0;
        assert_eq!(props, [b'P', b't', b'a', 0, b'L', 0, 0, 0, 3, b'x', 0, 0, 0, 3, b'b', b'c', 0, b'N']);
    }

    #[test]
    fn server_address() {
        assert_eq!(server(Some("unix:/run/user/1000/pulse/native")), Some("/run/user/1000/pulse/native".into()));
        assert_eq!(server(Some("{abc}unix:/tmp/p tcp:host")), Some("/tmp/p".into()));
        assert_eq!(server(Some("tcp:host:4713")), None);
        assert_eq!(server(Some("/tmp/x")), Some("/tmp/x".into()));
        assert!(server(None).unwrap().ends_with("pulse/native"));
        assert!(server(Some("  ")).unwrap().ends_with("pulse/native"));
    }
}

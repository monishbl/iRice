//! PulseAudio's native protocol, spoken directly (as tessera-events'
//! pulse.rs does): PipeWire's pipewire-pulse speaks it too, so this reaches
//! either sound server over its Unix socket with no C library. Enough of it
//! to authenticate, subscribe, read the server's defaults, its outputs,
//! inputs and playback streams, and set their volume, mute and the defaults.
//!
//! Why not libpulse or pipewire-rs: both link C libraries (pipewire-rs also
//! needs bindgen and clang to build); `pactl` per change would be a process
//! per event. The protocol is small, stable (version 32 is from 2016) and
//! already spoken in this repo.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

pub const VERSION: u32 = 32;
const ERROR: u32 = 0;
const REPLY: u32 = 2;
const AUTH: u32 = 8;
const SET_CLIENT_NAME: u32 = 9;
pub const GET_SERVER_INFO: u32 = 20;
pub const GET_SINK_INFO_LIST: u32 = 22;
pub const GET_SOURCE_INFO_LIST: u32 = 24;
pub const GET_SINK_INPUT_INFO_LIST: u32 = 30;
const SUBSCRIBE: u32 = 35;
pub const SET_SINK_VOLUME: u32 = 36;
pub const SET_SINK_INPUT_VOLUME: u32 = 37;
pub const SET_SOURCE_VOLUME: u32 = 38;
pub const SET_SINK_MUTE: u32 = 39;
pub const SET_SOURCE_MUTE: u32 = 40;
pub const SET_DEFAULT_SINK: u32 = 44;
pub const SET_DEFAULT_SOURCE: u32 = 45;
pub const SUBSCRIBE_EVENT: u32 = 66;
pub const SET_SINK_INPUT_MUTE: u32 = 69;
pub const INVALID: u32 = u32::MAX;
/// 100% (PA_VOLUME_NORM): volumes are on the cubic scale pactl, wpctl and
/// the shell's Quickshell show
pub const NORM: f64 = 65536.0;

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

fn bad(what: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, what.to_string())
}

/// One connection to the sound server
pub struct Conn {
    s: UnixStream,
    tag: u32,
    /// the protocol version both sides speak
    pub version: u32,
    /// subscription events that came while waiting for a reply
    pub events: Vec<Vec<u8>>,
}

impl Conn {
    /// Connect, authenticate (the cookie when there is one; the server also
    /// knows us by our uid) and say who we are
    pub fn open(name: &str) -> std::io::Result<Conn> {
        let path = server(std::env::var("PULSE_SERVER").ok().as_deref()).ok_or_else(|| bad("PULSE_SERVER isn't a Unix socket"))?;
        let s = UnixStream::connect(path)?;
        s.set_read_timeout(Some(Duration::from_secs(5)))?;
        let mut c = Conn { s, tag: 0, version: VERSION, events: Vec::new() };
        let reply = c.ask(AUTH, Tags::new().u32(VERSION).arbitrary(&cookie()))?;
        let server = Reader::new(&reply).u32().ok_or_else(|| bad("auth reply"))? & 0xffff;
        c.version = server.min(VERSION);
        c.ask(SET_CLIENT_NAME, Tags::new().proplist(&[("application.name", name), ("application.process.binary", "tessera-ui")]))?;
        Ok(c)
    }

    /// A command and its reply's data (after the command and tag); events
    /// arriving meanwhile are kept in `events`
    pub fn ask(&mut self, cmd: u32, body: Tags) -> std::io::Result<Vec<u8>> {
        self.tag = self.tag.wrapping_add(1) % INVALID;
        let tag = self.tag;
        let p = Tags::new().u32(cmd).u32(tag).raw(&body.0);
        self.s.write_all(&packet(&p.0))?;
        loop {
            let reply = self.read()?;
            let mut r = Reader::new(&reply);
            let (Some(c), Some(t)) = (r.u32(), r.u32()) else { return Err(bad("packet")) };
            if c == SUBSCRIBE_EVENT {
                self.events.push(reply);
                continue;
            }
            if t != tag {
                continue;
            }
            return match c {
                REPLY => Ok(r.rest().to_vec()),
                ERROR => Err(std::io::Error::other(format!("sound server error {}", r.u32().unwrap_or(0)))),
                _ => Err(bad("unexpected answer")),
            };
        }
    }

    /// Ask for events: `mask` of PA_SUBSCRIPTION_MASK_*
    pub fn subscribe(&mut self, mask: u32) -> std::io::Result<()> {
        self.ask(SUBSCRIBE, Tags::new().u32(mask)).map(|_| ())
    }

    /// The next packet (blocking up to `timeout`, None: forever)
    pub fn wait(&mut self, timeout: Option<Duration>) -> std::io::Result<Vec<u8>> {
        self.s.set_read_timeout(timeout)?;
        let r = self.read();
        let _ = self.s.set_read_timeout(Some(Duration::from_secs(5)));
        r
    }

    fn read(&mut self) -> std::io::Result<Vec<u8>> {
        let mut d = [0u8; 20];
        self.s.read_exact(&mut d)?;
        let len = u32::from_be_bytes(d[..4].try_into().unwrap()) as usize;
        if len > 1 << 22 {
            return Err(bad("packet too long"));
        }
        let mut p = vec![0u8; len];
        self.s.read_exact(&mut p)?;
        Ok(p)
    }
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

/// A subscribe event: (facility, kind, index), as pactl says it ("sink",
/// "change", 42); None for any other packet
pub fn event(payload: &[u8]) -> Option<(&'static str, &'static str, u32)> {
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
    Some((facility, kind, index))
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
    pub fn null(mut self) -> Tags {
        self.0.push(b'N');
        self
    }
    pub fn bool(mut self, v: bool) -> Tags {
        self.0.push(if v { b'1' } else { b'0' });
        self
    }
    /// The same volume on every channel (0..1 of 100%)
    pub fn cvolume(mut self, channels: u8, v: f64) -> Tags {
        let raw = (v.max(0.0) * NORM).round() as u32;
        self.0.push(b'v');
        self.0.push(channels.max(1));
        for _ in 0..channels.max(1) {
            self.0.extend(raw.to_be_bytes());
        }
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

/// A tagstruct being read: typed reads (None when the next value is another
/// type, or cut off) and `skip` for a value of any type
pub struct Reader<'a> {
    b: &'a [u8],
}

impl<'a> Reader<'a> {
    pub fn new(b: &'a [u8]) -> Reader<'a> {
        Reader { b }
    }
    pub fn done(&self) -> bool {
        self.b.is_empty()
    }
    fn rest(&self) -> &'a [u8] {
        self.b
    }
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        if self.b.len() < n {
            return None;
        }
        let (a, b) = self.b.split_at(n);
        self.b = b;
        Some(a)
    }
    fn tag(&mut self, want: u8) -> Option<()> {
        (self.b.first() == Some(&want)).then(|| self.b = &self.b[1..])
    }
    pub fn u32(&mut self) -> Option<u32> {
        if self.b.first() != Some(&b'L') && self.b.first() != Some(&b'V') {
            return None;
        }
        self.b = &self.b[1..];
        Some(u32::from_be_bytes(self.take(4)?.try_into().ok()?))
    }
    pub fn u8(&mut self) -> Option<u8> {
        self.tag(b'B')?;
        Some(self.take(1)?[0])
    }
    /// A string; "" for a null one
    pub fn string(&mut self) -> Option<String> {
        if self.tag(b'N').is_some() {
            return Some(String::new());
        }
        self.tag(b't')?;
        let n = self.b.iter().position(|&c| c == 0)?;
        let s = String::from_utf8_lossy(&self.b[..n]).into_owned();
        self.b = &self.b[n + 1..];
        Some(s)
    }
    pub fn bool(&mut self) -> Option<bool> {
        match self.b.first()? {
            b'1' => {
                self.b = &self.b[1..];
                Some(true)
            }
            b'0' => {
                self.b = &self.b[1..];
                Some(false)
            }
            _ => None,
        }
    }
    /// Each channel's volume (raw, NORM is 100%)
    pub fn cvolume(&mut self) -> Option<Vec<u32>> {
        self.tag(b'v')?;
        let n = self.take(1)?[0] as usize;
        let raw = self.take(4 * n)?;
        Some(raw.chunks(4).map(|c| u32::from_be_bytes(c.try_into().unwrap())).collect())
    }
    pub fn proplist(&mut self) -> Option<Vec<(String, String)>> {
        self.tag(b'P')?;
        let mut out = Vec::new();
        loop {
            if self.tag(b'N').is_some() {
                return Some(out);
            }
            let k = self.string()?;
            self.u32()?;
            self.tag(b'x')?;
            let n = u32::from_be_bytes(self.take(4)?.try_into().ok()?) as usize;
            let v = self.take(n)?;
            let v = v.strip_suffix(&[0]).unwrap_or(v);
            out.push((k, String::from_utf8_lossy(v).into_owned()));
        }
    }
    /// Skips one value, whatever its type
    pub fn skip(&mut self) -> Option<()> {
        let t = *self.b.first()?;
        match t {
            b't' | b'N' => {
                self.string()?;
            }
            b'P' => {
                self.proplist()?;
            }
            b'v' => {
                self.cvolume()?;
            }
            b'1' | b'0' => self.b = &self.b[1..],
            b'f' => {
                // a format: its encoding (u8) and proplist
                self.b = &self.b[1..];
                self.u8()?;
                self.proplist()?;
            }
            _ => {
                self.b = &self.b[1..];
                let n = match t {
                    b'L' | b'V' => 4,
                    b'B' => 1,
                    b'R' | b'r' | b'U' | b'T' => 8,
                    b'a' => 6, // sample spec: format, channels, rate
                    b'm' => 1 + *self.b.first()? as usize, // channel map
                    b'x' => 4 + u32::from_be_bytes(self.b.get(..4)?.try_into().ok()?) as usize,
                    _ => return None,
                };
                self.take(n)?;
            }
        }
        Some(())
    }
    pub fn skip_n(&mut self, n: usize) -> Option<()> {
        (0..n).try_for_each(|_| self.skip())
    }
}

/// The average of a volume's channels, 0..1 of 100% (pa_cvolume_avg)
pub fn average(v: &[u32]) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.iter().map(|&x| x as f64).sum::<f64>() / v.len() as f64 / NORM
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_as_pactl_names_them() {
        let p = Tags::new().u32(SUBSCRIBE_EVENT).u32(u32::MAX).u32(0x10).u32(42).0;
        assert_eq!(event(&p), Some(("sink", "change", 42)));
        let p = Tags::new().u32(SUBSCRIBE_EVENT).u32(u32::MAX).u32(0x07).u32(0).0;
        assert_eq!(event(&p), Some(("server", "new", 0)));
        assert_eq!(event(&Tags::new().u32(REPLY).u32(3).0), None);
    }

    #[test]
    fn values_read_back() {
        let t = Tags::new().u32(7).string("alsa_output").null().bool(true).cvolume(2, 0.5).proplist(&[("node.nick", "Speakers")]).0;
        let mut r = Reader::new(&t);
        assert_eq!(r.u32(), Some(7));
        assert_eq!(r.string().as_deref(), Some("alsa_output"));
        assert_eq!(r.string().as_deref(), Some(""));
        assert_eq!(r.bool(), Some(true));
        assert_eq!(r.cvolume(), Some(vec![32768, 32768]));
        assert_eq!(r.proplist(), Some(vec![("node.nick".into(), "Speakers".into())]));
        assert!(r.done());
        // the wrong type, or cut off: nothing (and nothing taken)
        let t = Tags::new().string("x").0;
        assert_eq!(Reader::new(&t).u32(), None);
        assert_eq!(Reader::new(&t[..1]).string(), None);
    }

    #[test]
    fn skips_every_kind_of_value() {
        let mut t = Tags::new().u32(1).string("a").null().bool(false).cvolume(1, 1.0).proplist(&[("k", "v")]).arbitrary(&[1, 2, 3]).0;
        t.extend([b'a', 3, 2, 0, 0, 0xbb, 0x80]); // sample spec
        t.extend([b'm', 2, 1, 2]); // channel map
        t.extend([b'U', 0, 0, 0, 0, 0, 0, 0, 9]); // usec
        t.extend([b'B', 4]);
        t.extend([b'f', b'B', 1, b'P', b'N']); // format: encoding, empty proplist
        t.extend(Tags::new().u32(99).0);
        let mut r = Reader::new(&t);
        r.skip_n(12).unwrap();
        assert_eq!(r.u32(), Some(99));
        assert!(r.done());
        assert_eq!(Reader::new(&[b'?']).skip(), None);
    }

    #[test]
    fn volumes_on_pulse_scale() {
        assert_eq!(average(&[65536, 65536]), 1.0);
        assert_eq!(average(&[32768, 65536]), 0.75);
        assert_eq!(average(&[]), 0.0);
        let t = Tags::new().cvolume(2, 0.42).0;
        assert_eq!(Reader::new(&t).cvolume().map(|v| average(&v)), Some((0.42f64 * NORM).round() / NORM));
    }

    #[test]
    fn server_address() {
        assert_eq!(server(Some("unix:/run/user/1000/pulse/native")), Some("/run/user/1000/pulse/native".into()));
        assert_eq!(server(Some("{abc}unix:/tmp/p tcp:host")), Some("/tmp/p".into()));
        assert_eq!(server(Some("tcp:host:4713")), None);
        assert!(server(None).unwrap().ends_with("pulse/native"));
    }

    #[test]
    fn packets_carry_their_length() {
        let p = packet(&[1, 2, 3]);
        assert_eq!(&p[..4], &3u32.to_be_bytes());
        assert_eq!(&p[4..8], &[0xff; 4]);
        assert_eq!(&p[20..], &[1, 2, 3]);
    }
}

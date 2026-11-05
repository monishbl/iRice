//! Sound (service "audio"): the default output and input with their volume
//! and mute, every output and input, and the apps playing (their streams),
//! read from the sound server over PulseAudio's protocol (pulse.rs; PipeWire
//! answers it through pipewire-pulse). A thread subscribes to the server's
//! events and sends a fresh `Sound` after each burst; another applies what
//! the surfaces ask (`send`), coalescing a drag's many volumes into the
//! newest. Shared by the OSD and, later, the Control Center.
//!
//! Volumes are 0..1 of 100% on the same cubic scale as the shell's Audio
//! (Quickshell's PipeWire), pactl and wpctl.

use super::pulse::{self, Conn, Reader, Tags};
use crate::Msg;
use smithay_client_toolkit::reexports::calloop::channel::Sender;
use std::sync::{mpsc, Mutex, OnceLock};
use std::time::Duration;

/// An output (sink) or input (source)
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Device {
    pub index: u32,
    /// the server's name for it (alsa_output.pci-…)
    pub name: String,
    pub description: String,
    /// node.nick: a short name, when the server has one
    pub nick: String,
    pub volume: f64,
    pub muted: bool,
    pub channels: u8,
    /// on Bluetooth (device.api bluez5)
    pub bluetooth: bool,
}

/// An app playing sound (a sink input)
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stream {
    pub index: u32,
    /// application.name, else media.name
    pub name: String,
    /// application.process.binary
    pub binary: String,
    /// application.icon-name, else application.id
    pub icon: String,
    pub volume: f64,
    pub muted: bool,
    pub channels: u8,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sound {
    /// the sound server answered
    pub up: bool,
    pub default_sink: String,
    pub default_source: String,
    pub sinks: Vec<Device>,
    /// inputs, without the outputs' monitors
    pub sources: Vec<Device>,
    pub streams: Vec<Stream>,
}

impl Sound {
    pub fn sink(&self) -> Option<&Device> {
        self.sinks.iter().find(|d| d.name == self.default_sink)
    }
    pub fn source(&self) -> Option<&Device> {
        self.sources.iter().find(|d| d.name == self.default_source)
    }
    pub fn volume(&self) -> f64 {
        self.sink().map_or(0.0, |d| d.volume)
    }
    pub fn muted(&self) -> bool {
        self.sink().is_some_and(|d| d.muted)
    }
    pub fn mic_volume(&self) -> f64 {
        self.source().map_or(0.0, |d| d.volume)
    }
    pub fn mic_muted(&self) -> bool {
        self.source().is_some_and(|d| d.muted)
    }
}

/// What a device is called, as Audio.nameOf: its nickname, description or name
pub fn name_of(d: &Device) -> String {
    [&d.nick, &d.description, &d.name].into_iter().find(|s| !s.is_empty()).cloned().unwrap_or_default()
}

/// What an output or input is called in a list, as AudioStreams.outputName:
/// its description first
pub fn list_name(d: &Device) -> String {
    [&d.description, &d.nick, &d.name].into_iter().find(|s| !s.is_empty()).cloned().unwrap_or_default()
}

/// Short event sounds (the shell's own clicks) come and go too fast to mix
/// (AudioStreams.eventPlayers)
pub const EVENT_PLAYERS: &[&str] = &["pw-play", "pw-cat", "paplay", "aplay", "canberra-gtk-play"];

// ── Reading the server's records ────────────────────────────────────────────

fn prop<'a>(props: &'a [(String, String)], k: &str) -> &'a str {
    props.iter().find(|(n, _)| n == k).map_or("", |(_, v)| v.as_str())
}

/// One sink or source record (they share their layout up to the formats);
/// with whether it's a monitor of an output (sources only)
pub fn device(r: &mut Reader, v: u32, sink: bool) -> Option<(Device, bool)> {
    let index = r.u32()?;
    let name = r.string()?;
    let description = r.string()?;
    r.skip_n(3)?; // sample spec, channel map, owner module
    let vol = r.cvolume()?;
    let muted = r.bool()?;
    let monitor_of = r.u32()?;
    r.skip_n(4)?; // monitor (of) name, latency, driver, flags
    let mut props = Vec::new();
    if v >= 13 {
        props = r.proplist()?;
        r.skip()?; // configured latency
    }
    if v >= 15 {
        r.skip_n(4)?; // base volume, state, volume steps, card
    }
    if v >= 16 {
        let ports = r.u32()?;
        let each = 3 + usize::from(v >= 24) + 2 * usize::from(v >= 34);
        r.skip_n(ports as usize * each)?;
        r.skip()?; // the active port
    }
    if v >= if sink { 21 } else { 22 } {
        let n = r.u8()?;
        r.skip_n(n as usize)?;
    }
    let d = Device {
        index,
        nick: prop(&props, "node.nick").to_string(),
        bluetooth: prop(&props, "device.api") == "bluez5",
        name,
        description,
        volume: pulse::average(&vol),
        muted,
        channels: vol.len() as u8,
    };
    Some((d, !sink && monitor_of != pulse::INVALID))
}

/// One sink input record
pub fn stream(r: &mut Reader, v: u32) -> Option<Stream> {
    let index = r.u32()?;
    let media = r.string()?;
    r.skip_n(5)?; // owner module, client, sink, sample spec, channel map
    let vol = r.cvolume()?;
    r.skip_n(4)?; // buffer and sink latency, resample method, driver
    let muted = if v >= 11 { r.bool()? } else { false };
    let props = if v >= 13 { r.proplist()? } else { Vec::new() };
    if v >= 19 {
        r.skip()?; // corked
    }
    if v >= 20 {
        r.skip_n(2)?; // has volume, volume writable
    }
    if v >= 21 {
        r.skip()?; // format
    }
    let first = |ks: &[&str]| ks.iter().map(|k| prop(&props, k)).find(|s| !s.is_empty()).unwrap_or("").to_string();
    Some(Stream {
        index,
        name: { let n = first(&["application.name", "media.name"]); if n.is_empty() { media } else { n } },
        binary: first(&["application.process.binary"]),
        icon: first(&["application.icon-name", "application.id"]),
        volume: pulse::average(&vol),
        muted,
        channels: vol.len() as u8,
    })
}

/// The server's defaults: (output, input) names
pub fn defaults(r: &mut Reader) -> Option<(String, String)> {
    r.skip_n(5)?; // package, version, user, host, sample spec
    Some((r.string()?, r.string()?))
}

fn read(c: &mut Conn) -> std::io::Result<Sound> {
    let bad = || std::io::Error::new(std::io::ErrorKind::InvalidData, "record");
    let v = c.version;
    let info = c.ask(pulse::GET_SERVER_INFO, Tags::new())?;
    let (default_sink, default_source) = defaults(&mut Reader::new(&info)).ok_or_else(bad)?;
    let mut s = Sound { up: true, default_sink, default_source, ..Default::default() };
    let list = c.ask(pulse::GET_SINK_INFO_LIST, Tags::new())?;
    let mut r = Reader::new(&list);
    while !r.done() {
        s.sinks.push(device(&mut r, v, true).ok_or_else(bad)?.0);
    }
    let list = c.ask(pulse::GET_SOURCE_INFO_LIST, Tags::new())?;
    let mut r = Reader::new(&list);
    while !r.done() {
        let (d, monitor) = device(&mut r, v, false).ok_or_else(bad)?;
        if !monitor && !d.name.ends_with(".monitor") {
            s.sources.push(d);
        }
    }
    let list = c.ask(pulse::GET_SINK_INPUT_INFO_LIST, Tags::new())?;
    let mut r = Reader::new(&list);
    while !r.done() {
        s.streams.push(stream(&mut r, v).ok_or_else(bad)?);
    }
    Ok(s)
}

// sink, source, sink input, server (PA_SUBSCRIPTION_MASK_*)
const MASK: u32 = 0x0001 | 0x0002 | 0x0004 | 0x0080;

pub fn spawn(tx: Sender<Msg>) {
    std::thread::spawn(move || {
        let mut wait = 1;
        loop {
            let run = || -> std::io::Result<()> {
                let mut c = Conn::open("Tessera")?;
                c.subscribe(MASK)?;
                let mut last = read(&mut c)?;
                tx.send(Msg::Data(Box::new(last.clone()))).map_err(|_| std::io::ErrorKind::BrokenPipe)?;
                loop {
                    // (events that came during the last reading count too;
                    // the next reading answers the whole burst)
                    if c.events.is_empty() {
                        let p = c.wait(None)?;
                        if pulse::event(&p).is_none() {
                            continue;
                        }
                    }
                    c.events.clear();
                    let now = read(&mut c)?;
                    if now != last {
                        last = now.clone();
                        tx.send(Msg::Data(Box::new(now))).map_err(|_| std::io::ErrorKind::BrokenPipe)?;
                    }
                }
            };
            let e = run().err();
            if e.as_ref().is_some_and(|e| e.kind() == std::io::ErrorKind::BrokenPipe) {
                return; // (the interface ended)
            }
            if crate::env::is_set("DEBUG") {
                eprintln!("debug: sound server: {e:?}");
            }
            if tx.send(Msg::Data(Box::new(Sound::default()))).is_err() {
                return;
            }
            // (the server restarting, or not up yet: look again, less often)
            std::thread::sleep(Duration::from_secs(wait));
            wait = (wait * 2).min(30);
        }
    });
}

// ── Changing things ─────────────────────────────────────────────────────────

/// What a surface asks the sound server to do (indexes and channel counts
/// from the newest `Sound`)
#[derive(Clone, Debug, PartialEq)]
pub enum Cmd {
    SinkVolume { index: u32, channels: u8, volume: f64 },
    SourceVolume { index: u32, channels: u8, volume: f64 },
    SinkMute { index: u32, muted: bool },
    SourceMute { index: u32, muted: bool },
    StreamVolume { index: u32, channels: u8, volume: f64 },
    StreamMute { index: u32, muted: bool },
    DefaultSink(String),
    DefaultSource(String),
}

impl Cmd {
    /// Which thing it sets: a newer command for the same one replaces it
    fn key(&self) -> (u8, u32) {
        match self {
            Cmd::SinkVolume { index, .. } => (0, *index),
            Cmd::SourceVolume { index, .. } => (1, *index),
            Cmd::SinkMute { index, .. } => (2, *index),
            Cmd::SourceMute { index, .. } => (3, *index),
            Cmd::StreamVolume { index, .. } => (4, *index),
            Cmd::StreamMute { index, .. } => (5, *index),
            Cmd::DefaultSink(_) => (6, 0),
            Cmd::DefaultSource(_) => (7, 0),
        }
    }
    fn request(&self) -> (u32, Tags) {
        let t = Tags::new();
        let clamp = |v: f64| v.clamp(0.0, 1.0);
        match self {
            Cmd::SinkVolume { index, channels, volume } => (pulse::SET_SINK_VOLUME, t.u32(*index).null().cvolume(*channels, clamp(*volume))),
            Cmd::SourceVolume { index, channels, volume } => (pulse::SET_SOURCE_VOLUME, t.u32(*index).null().cvolume(*channels, clamp(*volume))),
            Cmd::SinkMute { index, muted } => (pulse::SET_SINK_MUTE, t.u32(*index).null().bool(*muted)),
            Cmd::SourceMute { index, muted } => (pulse::SET_SOURCE_MUTE, t.u32(*index).null().bool(*muted)),
            Cmd::StreamVolume { index, channels, volume } => (pulse::SET_SINK_INPUT_VOLUME, t.u32(*index).cvolume(*channels, clamp(*volume))),
            Cmd::StreamMute { index, muted } => (pulse::SET_SINK_INPUT_MUTE, t.u32(*index).bool(*muted)),
            Cmd::DefaultSink(n) => (pulse::SET_DEFAULT_SINK, t.string(n)),
            Cmd::DefaultSource(n) => (pulse::SET_DEFAULT_SOURCE, t.string(n)),
        }
    }
}

/// The newest of each, in the order first asked
pub fn coalesce(cmds: Vec<Cmd>) -> Vec<Cmd> {
    let mut out: Vec<Cmd> = Vec::new();
    for c in cmds {
        match out.iter_mut().find(|o| o.key() == c.key()) {
            Some(o) => *o = c,
            None => out.push(c),
        }
    }
    out
}

static CMDS: OnceLock<Mutex<mpsc::Sender<Cmd>>> = OnceLock::new();

/// Ask the sound server (from any thread; never blocks)
pub fn send(cmd: Cmd) {
    let tx = CMDS.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<Cmd>();
        std::thread::spawn(move || {
            let mut conn: Option<Conn> = None;
            while let Ok(first) = rx.recv() {
                let mut cmds = vec![first];
                cmds.extend(rx.try_iter());
                for c in coalesce(cmds) {
                    // (once more on a fresh connection if the old one broke)
                    for _ in 0..2 {
                        if conn.is_none() {
                            conn = Conn::open("Tessera controls").ok();
                        }
                        let Some(k) = conn.as_mut() else { break };
                        let (cmd, body) = c.request();
                        match k.ask(cmd, body) {
                            Ok(_) => break,
                            Err(e) if e.kind() == std::io::ErrorKind::Other => break, // (the server said no)
                            Err(_) => conn = None,
                        }
                    }
                    if let Some(k) = conn.as_mut() {
                        k.events.clear();
                    }
                }
            }
        });
        Mutex::new(tx)
    });
    if let Ok(tx) = tx.lock() {
        let _ = tx.send(cmd);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A sink record as pipewire-pulse sends it at protocol 32
    pub fn sink_record(index: u32, name: &str, desc: &str, vol: f64, muted: bool, props: &[(&str, &str)]) -> Vec<u8> {
        let mut t = Tags::new().u32(index).string(name).string(desc).0;
        t.extend([b'a', 3, 2, 0, 0, 0xbb, 0x80, b'm', 2, 1, 2]);
        t.extend(Tags::new().u32(0).cvolume(2, vol).bool(muted).u32(5).string(&format!("{name}.monitor")).0);
        t.extend([b'U', 0, 0, 0, 0, 0, 0, 0, 0]);
        t.extend(Tags::new().string("PipeWire").u32(0).proplist(props).0);
        t.extend([b'U', 0, 0, 0, 0, 0, 0, 0, 0, b'V', 0, 1, 0, 0]);
        t.extend(Tags::new().u32(0).u32(65537).u32(1).u32(1).string("analog-output-speaker").string("Speakers").u32(100).u32(0).string("analog-output-speaker").0);
        t.extend([b'B', 1, b'f', b'B', 1, b'P', b'N']);
        t
    }

    #[test]
    fn reads_outputs_as_the_server_sends_them() {
        let mut list = sink_record(42, "alsa_output.pci", "Built-in Audio Analog Stereo", 0.5, false, &[("node.nick", "Speakers")]);
        list.extend(sink_record(43, "bluez_output.AA", "Buds", 0.25, true, &[("device.api", "bluez5")]));
        let mut r = Reader::new(&list);
        let (a, _) = device(&mut r, 32, true).unwrap();
        let (b, _) = device(&mut r, 32, true).unwrap();
        assert!(r.done());
        assert_eq!((a.index, a.volume, a.muted, a.channels, a.bluetooth), (42, 0.5, false, 2, false));
        assert_eq!(name_of(&a), "Speakers");
        assert_eq!(list_name(&a), "Built-in Audio Analog Stereo");
        assert_eq!((b.index, b.volume, b.muted, b.bluetooth), (43, 0.25, true, true));
        assert_eq!(name_of(&b), "Buds");
    }

    #[test]
    fn a_stream_and_the_defaults() {
        let mut t = Tags::new().u32(7).string("Playback").u32(1).u32(2).u32(42).0;
        t.extend([b'a', 3, 2, 0, 0, 0xbb, 0x80, b'm', 2, 1, 2]);
        t.extend(Tags::new().cvolume(2, 0.75).0);
        t.extend([b'U', 0, 0, 0, 0, 0, 0, 0, 0, b'U', 0, 0, 0, 0, 0, 0, 0, 0]);
        t.extend(Tags::new().null().string("PipeWire").bool(false).proplist(&[("application.name", "Firefox"), ("application.process.binary", "firefox")]).bool(false).bool(true).bool(true).0);
        t.extend([b'f', b'B', 1, b'P', b'N']);
        let s = stream(&mut Reader::new(&t), 32).unwrap();
        assert_eq!((s.index, s.name.as_str(), s.binary.as_str(), s.volume, s.muted), (7, "Firefox", "firefox", 0.75, false));

        let mut t = Tags::new().string("pulseaudio").string("15.0.0").string("me").string("host").0;
        t.extend([b'a', 3, 2, 0, 0, 0xbb, 0x80]);
        t.extend(Tags::new().string("alsa_output.pci").string("alsa_input.pci").u32(1).0);
        assert_eq!(defaults(&mut Reader::new(&t)), Some(("alsa_output.pci".into(), "alsa_input.pci".into())));
    }

    #[test]
    fn the_default_output_and_its_level() {
        let d = |n: &str, v: f64, m: bool| Device { name: n.into(), volume: v, muted: m, ..Default::default() };
        let s = Sound { up: true, default_sink: "b".into(), sinks: vec![d("a", 0.1, false), d("b", 0.6, true)], ..Default::default() };
        assert_eq!((s.volume(), s.muted()), (0.6, true));
        assert_eq!((s.mic_volume(), s.mic_muted()), (0.0, false));
    }

    #[test]
    fn a_drag_sends_only_its_newest_level() {
        let v = |x: f64| Cmd::SinkVolume { index: 1, channels: 2, volume: x };
        let m = Cmd::SinkMute { index: 1, muted: false };
        assert_eq!(coalesce(vec![m.clone(), v(0.1), v(0.2), v(0.3)]), vec![m, v(0.3)]);
        let other = Cmd::SinkVolume { index: 2, channels: 2, volume: 0.5 };
        assert_eq!(coalesce(vec![v(0.1), other.clone(), v(0.2)]), vec![v(0.2), other]);
    }
}

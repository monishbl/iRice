//! What the OSD shows and when, as plain functions (Osd.qml's logic): the
//! level, icon and label for each kind, which change shows it, the switch to
//! another output or microphone, where it goes, and the rows under its
//! button (IslandLevels.qml).

use crate::services::audio::{self, Sound};
use crate::services::brightness::Backlight;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Kind {
    #[default]
    Volume,
    Brightness,
    Mic,
}

// Phosphor glyphs (shell/common/phosphor.js)
pub const SPEAKER_X: &str = "\u{e45c}";
pub const SPEAKER_NONE: &str = "\u{e44e}";
pub const SPEAKER_LOW: &str = "\u{e44c}";
pub const SPEAKER_HIGH: &str = "\u{e44a}";
pub const SUN: &str = "\u{e472}";
pub const MICROPHONE: &str = "\u{e326}";
pub const MICROPHONE_SLASH: &str = "\u{e328}";
pub const SLIDERS: &str = "\u{e434}";
pub const CARET_LEFT: &str = "\u{e138}";
pub const GEAR: &str = "\u{e270}";
pub const LAPTOP: &str = "\u{e586}";
pub const HEADPHONES: &str = "\u{e2a6}";
pub const SPEAKER_HIFI: &str = "\u{ea08}";
pub const WEBCAM: &str = "\u{e9b2}";
pub const APP_WINDOW: &str = "\u{e5da}";

/// Audio.icon: muted or silent, then by thirds
pub fn volume_icon(volume: f64, muted: bool) -> &'static str {
    if muted || volume == 0.0 {
        SPEAKER_X
    } else if volume < 0.34 {
        SPEAKER_NONE
    } else if volume < 0.67 {
        SPEAKER_LOW
    } else {
        SPEAKER_HIGH
    }
}

/// The level shown (0..1; muted shows empty)
pub fn value(kind: Kind, s: &Sound, b: &Backlight) -> f64 {
    let v = match kind {
        Kind::Brightness => b.value,
        Kind::Mic if s.mic_muted() => 0.0,
        Kind::Mic => s.mic_volume(),
        Kind::Volume if s.muted() => 0.0,
        Kind::Volume => s.volume(),
    };
    v.clamp(0.0, 1.0)
}

pub fn icon(kind: Kind, s: &Sound) -> &'static str {
    match kind {
        Kind::Brightness => SUN,
        Kind::Mic if s.mic_muted() => MICROPHONE_SLASH,
        Kind::Mic => MICROPHONE,
        Kind::Volume => volume_icon(s.volume(), s.muted()),
    }
}

/// Its title: the device that was just switched to, else what it is
pub fn label(kind: Kind, device: &str, s: &Sound) -> String {
    if !device.is_empty() {
        return device.to_string();
    }
    match kind {
        Kind::Brightness => "Display",
        Kind::Mic if s.mic_muted() => "Microphone off",
        Kind::Mic => "Microphone on",
        Kind::Volume => "Sound",
    }
    .to_string()
}

/// The button under the level: what it opens
pub fn more_label(kind: Kind) -> &'static str {
    match kind {
        Kind::Brightness => "Displays",
        Kind::Mic => "Inputs",
        Kind::Volume => "Outputs",
    }
}

/// How long it stays after the last change (a device's name longer)
pub fn hide_after(device: &str) -> u64 {
    if device.is_empty() {
        1600
    } else {
        2500
    }
}

/// Where a change shows
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    /// not at all (the first seconds, or set with Tessera's own slider)
    Nowhere,
    /// a fullscreen app: the thin bar along the top
    Bar,
    /// the island (the QML menu bar's) shows it
    Island,
    /// the capsule (Glass) or the volume panel (Material)
    Hud,
}

/// Osd.qml's show(): `quiet` set with Tessera's own controls (but not for
/// a device that just arrived), `island`: the island's HUD takes it
pub fn place(armed: bool, quiet: bool, device: bool, fullscreen: bool, island: bool) -> Place {
    if !armed || (quiet && !device) {
        Place::Nowhere
    } else if fullscreen {
        Place::Bar
    } else if island {
        Place::Island
    } else {
        Place::Hud
    }
}

/// What a new sound reading asks for
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cue {
    Show(Kind),
    /// another output or microphone: show it with its name in a moment
    /// (once its level and mute have arrived)
    Switched(Kind),
}

/// Follows the sound readings as Osd.qml's Connections on Audio: a change of
/// volume or mute shows; a new default output or input shows its name
/// shortly after, and the changes that come with the switch (for 1.5 s)
/// don't show of their own
#[derive(Default)]
pub struct Watch {
    seen: Option<(String, String)>,
    switched_at: u64,
    last: Option<Sound>,
}

impl Watch {
    fn switched(&mut self, s: &Sound, now: u64) -> bool {
        let pair = (s.default_sink.clone(), s.default_source.clone());
        if self.seen.as_ref() != Some(&pair) {
            self.seen = Some(pair);
            self.switched_at = now;
        }
        now.saturating_sub(self.switched_at) < 1500
    }

    pub fn sound(&mut self, s: &Sound, now: u64) -> Vec<Cue> {
        let last = self.last.replace(s.clone()).unwrap_or_default();
        let mut out = Vec::new();
        let sink = last.default_sink != s.default_sink;
        let source = last.default_source != s.default_source;
        let switching = self.switched(s, now);
        if sink && switching && s.sink().is_some() {
            out.push(Cue::Switched(Kind::Volume));
        }
        if source && switching && s.source().is_some() && !out.contains(&Cue::Switched(Kind::Volume)) {
            out.push(Cue::Switched(Kind::Mic));
        }
        if !switching {
            if last.volume() != s.volume() || last.muted() != s.muted() {
                out.push(Cue::Show(Kind::Volume));
            }
            if last.mic_muted() != s.mic_muted() {
                out.push(Cue::Show(Kind::Mic));
            }
        }
        out
    }
}

// ── The rows under the button (IslandLevels.qml) ───────────────────────────

/// What a row does
#[derive(Clone, Debug, PartialEq)]
pub enum Act {
    Sink(String),
    Source(String),
    Backlight,
    Apps,
    Back,
    Stream(u32),
    Settings(&'static str),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub icon: &'static str,
    /// a device's picture (shell/assets/devices/<this>.svg) in place of the icon
    pub picture: Option<&'static str>,
    pub text: String,
    /// the one in use (ticked)
    pub current: bool,
    /// opens a page (an arrow)
    pub chevron: bool,
    /// a level row: its level, else -1
    pub value: f64,
    /// a level row's icon is dimmed (an app muted)
    pub dim: bool,
    pub act: Act,
}

impl Row {
    fn choice(icon: &'static str, text: impl Into<String>, act: Act) -> Row {
        Row { icon, picture: None, text: text.into(), current: false, chevron: false, value: -1.0, dim: false, act }
    }
}

/// AudioStreams.outputIcon / inputIcon
pub fn device_icon(d: &audio::Device, input: bool) -> &'static str {
    let name = audio::list_name(d).to_lowercase();
    if !input {
        return if name.contains("headphone") { HEADPHONES } else { SPEAKER_HIFI };
    }
    let t = format!("{name} {}", d.name.to_lowercase());
    if t.contains("webcam") || t.contains("camera") || t.contains("video") {
        WEBCAM
    } else if t.contains("bluez") || t.contains("headset") {
        HEADPHONES
    } else {
        MICROPHONE
    }
}

/// What follows each place `w` starts a word in `n` (a regex's \b before it)
fn after_word<'a>(n: &'a str, w: &'a str) -> impl Iterator<Item = &'a str> + 'a {
    n.match_indices(w).filter(|(i, _)| n[..*i].chars().last().is_none_or(|c| !c.is_alphanumeric())).map(move |(i, _)| &n[i + w.len()..])
}

/// Devices.pictureFor for a headset (Bluetooth audio): its picture by name,
/// shell/assets/devices/<kind>.svg (earbuds, a soundbar, else headphones)
pub fn headset_picture(name: &str) -> &'static str {
    let n = name.to_lowercase();
    let ends_word = |r: &str| r.chars().next().is_none_or(|c| !c.is_alphanumeric());
    if n.contains("soundbar") || n.contains("sound bar") {
        "soundbar"
    } else if ["buds", "pods", "earbud", "earphone", "liberty"].iter().any(|w| n.contains(w))
        || after_word(&n, "tws").any(ends_word)
        || after_word(&n, "wf-").next().is_some()
        || after_word(&n, "freeclip").next().is_some()
        || after_word(&n, "ear").any(|r| r.starts_with('(') || r.starts_with(" ("))
    {
        "earbuds"
    } else {
        "headphones"
    }
}

/// AudioStreams.picture: an output's or input's picture (Bluetooth ones
/// theirs, the computer's own the laptop's); None for HDMI, DisplayPort,
/// USB and cameras (they keep their icon)
pub fn device_picture(d: &audio::Device) -> Option<&'static str> {
    let name = audio::list_name(d);
    let t = format!("{name} {}", d.name).to_lowercase();
    if d.bluetooth || t.contains("bluez") {
        return Some(headset_picture(&name));
    }
    if ["hdmi", "displayport", "usb", "webcam", "camera"].iter().any(|w| t.contains(w)) {
        return None;
    }
    Some("laptop")
}

/// The apps playing (not the shell's own short sounds)
pub fn streams(s: &Sound) -> Vec<&audio::Stream> {
    s.streams.iter().filter(|x| !audio::EVENT_PLAYERS.contains(&x.binary.as_str())).collect()
}

/// The rows for a kind and page ("list" or "apps"); `app_name` names an
/// app's stream (its installed app's name when there is one)
pub fn rows(kind: Kind, page: &str, s: &Sound, b: &Backlight, app_name: &mut dyn FnMut(&audio::Stream) -> String) -> Vec<Row> {
    let mut out = Vec::new();
    if page == "apps" && kind == Kind::Volume {
        out.push(Row::choice(CARET_LEFT, "App volumes", Act::Back));
        for x in streams(s) {
            out.push(Row { value: if x.muted { 0.0 } else { x.volume.min(1.0) }, dim: x.muted, ..Row::choice(APP_WINDOW, app_name(x), Act::Stream(x.index)) });
        }
        return out;
    }
    match kind {
        Kind::Volume => {
            for d in &s.sinks {
                out.push(Row { current: d.name == s.default_sink, picture: device_picture(d), ..Row::choice(device_icon(d, false), audio::list_name(d), Act::Sink(d.name.clone())) });
            }
            if !streams(s).is_empty() {
                out.push(Row { chevron: true, ..Row::choice(SLIDERS, "App volumes", Act::Apps) });
            }
        }
        Kind::Mic => {
            for d in &s.sources {
                out.push(Row { current: d.name == s.default_source, picture: device_picture(d), ..Row::choice(device_icon(d, true), audio::list_name(d), Act::Source(d.name.clone())) });
            }
        }
        Kind::Brightness => {
            if b.present {
                out.push(Row { value: b.value, picture: Some("laptop"), ..Row::choice(LAPTOP, "Built-in display", Act::Backlight) });
            }
        }
    }
    out.push(Row::choice(GEAR, if kind == Kind::Brightness { "Display settings" } else { "Sound settings" }, Act::Settings(if kind == Kind::Brightness { "displays" } else { "sound" })));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::audio::{Device, Stream};

    fn dev(name: &str, v: f64, muted: bool) -> Device {
        Device { name: name.into(), description: format!("{name} desc"), volume: v, muted, channels: 2, ..Default::default() }
    }
    fn sound(sink: &str, v: f64, muted: bool) -> Sound {
        Sound { up: true, default_sink: sink.into(), default_source: "mic".into(), sinks: vec![dev("a", v, muted), dev("b", v, muted)], sources: vec![dev("mic", 0.5, false)], ..Default::default() }
    }

    #[test]
    fn icons_follow_the_level() {
        assert_eq!(volume_icon(0.0, false), SPEAKER_X);
        assert_eq!(volume_icon(0.8, true), SPEAKER_X);
        assert_eq!(volume_icon(0.2, false), SPEAKER_NONE);
        assert_eq!(volume_icon(0.34, false), SPEAKER_LOW);
        assert_eq!(volume_icon(0.67, false), SPEAKER_HIGH);
        let mut s = sound("a", 0.5, false);
        assert_eq!(icon(Kind::Mic, &s), MICROPHONE);
        s.sources[0].muted = true;
        assert_eq!(icon(Kind::Mic, &s), MICROPHONE_SLASH);
        assert_eq!(label(Kind::Mic, "", &s), "Microphone off");
        assert_eq!(icon(Kind::Brightness, &s), SUN);
    }

    #[test]
    fn muted_shows_empty_and_over_100_is_full() {
        let b = Backlight { present: true, device: "x".into(), value: 0.3 };
        assert_eq!(value(Kind::Volume, &sound("a", 0.5, true), &b), 0.0);
        assert_eq!(value(Kind::Volume, &sound("a", 1.4, false), &b), 1.0);
        assert_eq!(value(Kind::Brightness, &sound("a", 0.5, false), &b), 0.3);
        assert_eq!(value(Kind::Volume, &Sound::default(), &b), 0.0);
    }

    #[test]
    fn labels_and_how_long() {
        let s = sound("a", 0.5, false);
        assert_eq!(label(Kind::Volume, "", &s), "Sound");
        assert_eq!(label(Kind::Brightness, "", &s), "Display");
        assert_eq!(label(Kind::Volume, "Buds", &s), "Buds");
        assert_eq!((hide_after(""), hide_after("Buds")), (1600, 2500));
        assert_eq!(more_label(Kind::Volume), "Outputs");
    }

    #[test]
    fn where_it_shows() {
        assert_eq!(place(false, false, false, false, false), Place::Nowhere);
        assert_eq!(place(true, true, false, false, false), Place::Nowhere);
        assert_eq!(place(true, true, true, false, false), Place::Hud); // (a device's name still shows)
        assert_eq!(place(true, false, false, true, true), Place::Bar); // (fullscreen first)
        assert_eq!(place(true, false, false, false, true), Place::Island);
        assert_eq!(place(true, false, false, false, false), Place::Hud);
    }

    #[test]
    fn a_change_of_level_or_mute_shows() {
        let mut w = Watch::default();
        // (the first reading is the switch to the first output: its name, ignored until armed)
        assert_eq!(w.sound(&sound("a", 0.5, false), 0), vec![Cue::Switched(Kind::Volume)]);
        assert_eq!(w.sound(&sound("a", 0.55, false), 5000), vec![Cue::Show(Kind::Volume)]);
        assert_eq!(w.sound(&sound("a", 0.55, true), 6000), vec![Cue::Show(Kind::Volume)]);
        assert_eq!(w.sound(&sound("a", 0.55, true), 7000), vec![]);
        let mut s = sound("a", 0.55, true);
        s.sources[0].muted = true;
        assert_eq!(w.sound(&s, 8000), vec![Cue::Show(Kind::Mic)]);
        // (a microphone's level alone doesn't, as in the QML)
        s.sources[0].volume = 0.9;
        assert_eq!(w.sound(&s, 9000), vec![]);
    }

    #[test]
    fn a_new_output_shows_its_name_not_its_level() {
        let mut w = Watch::default();
        w.sound(&sound("a", 0.5, false), 0);
        assert_eq!(w.sound(&sound("b", 0.3, false), 10_000), vec![Cue::Switched(Kind::Volume)]);
        // its level arriving just after: not a change of its own
        assert_eq!(w.sound(&sound("b", 0.35, false), 10_400), vec![]);
        assert_eq!(w.sound(&sound("b", 0.4, false), 12_000), vec![Cue::Show(Kind::Volume)]);
        // both switching: the output's name
        let mut s = sound("a", 0.4, false);
        s.default_source = "other".into();
        s.sources.push(dev("other", 0.1, false));
        assert_eq!(w.sound(&s, 20_000), vec![Cue::Switched(Kind::Volume)]);
        // an output that's gone (none left): nothing to name
        let mut s2 = s.clone();
        s2.default_sink = "gone".into();
        assert_eq!(w.sound(&s2, 30_000), vec![]);
    }

    #[test]
    fn device_pictures_as_the_shell_picks_them() {
        assert_eq!(headset_picture("realme TechLife Buds T100"), "earbuds");
        assert_eq!(headset_picture("Sony WF-1000XM5"), "earbuds");
        assert_eq!(headset_picture("Nothing Ear (2)"), "earbuds");
        assert_eq!(headset_picture("QCY TWS"), "earbuds");
        assert_eq!(headset_picture("Sony WH-1000XM4"), "headphones");
        assert_eq!(headset_picture("Bose Soundbar 700"), "soundbar");
        assert_eq!(headset_picture("Gear tws2"), "headphones"); // (tws not a word of its own)
        let d = |name: &str, desc: &str, bt: bool| Device { name: name.into(), description: desc.into(), bluetooth: bt, ..Default::default() };
        assert_eq!(device_picture(&d("alsa_output.pci-0000_00_1f.3.analog-stereo", "Built-in Audio Analog Stereo", false)), Some("laptop"));
        assert_eq!(device_picture(&d("alsa_output.pci-0000_00_1f.3.hdmi-stereo", "HDMI / DisplayPort", false)), None);
        assert_eq!(device_picture(&d("alsa_input.usb-Logitech_Webcam", "Webcam C920", false)), None);
        assert_eq!(device_picture(&d("bluez_output.AA_BB.1", "Buds Pro", true)), Some("earbuds"));
    }

    #[test]
    fn rows_for_each_kind() {
        let mut s = sound("b", 0.5, false);
        s.sinks[0].description = "USB Headphones".into();
        s.streams = vec![Stream { index: 3, name: "Firefox".into(), binary: "firefox".into(), volume: 0.7, ..Default::default() }, Stream { index: 4, name: "sound".into(), binary: "pw-play".into(), ..Default::default() }];
        let b = Backlight { present: true, device: "intel".into(), value: 0.4 };
        let mut name = |x: &Stream| x.name.clone();
        let r = rows(Kind::Volume, "list", &s, &b, &mut name);
        let texts: Vec<&str> = r.iter().map(|r| r.text.as_str()).collect();
        assert_eq!(texts, vec!["USB Headphones", "b desc", "App volumes", "Sound settings"]);
        assert_eq!((r[0].icon, r[0].current, r[1].current, r[2].chevron), (HEADPHONES, false, true, true));
        assert_eq!(r[1].act, Act::Sink("b".into()));
        assert_eq!(r[3].act, Act::Settings("sound"));
        // the apps page: the shell's own short sounds aren't listed
        let r = rows(Kind::Volume, "apps", &s, &b, &mut name);
        assert_eq!(r.len(), 2);
        assert_eq!((r[1].act.clone(), r[1].value), (Act::Stream(3), 0.7));
        // no apps playing: no App volumes
        s.streams.clear();
        assert!(!rows(Kind::Volume, "list", &s, &b, &mut name).iter().any(|r| r.act == Act::Apps));
        let r = rows(Kind::Mic, "list", &s, &b, &mut name);
        assert_eq!((r[0].icon, r[0].current, r.len()), (MICROPHONE, true, 2));
        let r = rows(Kind::Brightness, "list", &s, &b, &mut name);
        assert_eq!((r[0].value, r[1].text.as_str(), r[1].act.clone()), (0.4, "Display settings", Act::Settings("displays")));
        assert_eq!(rows(Kind::Brightness, "list", &s, &Backlight::default(), &mut name).len(), 1);
    }
}

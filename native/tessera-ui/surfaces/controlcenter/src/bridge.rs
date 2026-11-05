//! What the QML shell tells the Control Center (stdin lines
//! `{"to":"controlcenter", …}` from shell/modules/controlcenter/RustBridge.qml),
//! and the pages built here from the sound server. Most modules' state lives
//! in the shell's own services (Focus, Sharing, Night Light, Caffeine,
//! Bluetooth, the Wi-Fi networks, the media player…) until those move to Rust:
//! the shell sends what each says (label, icon, on, status, usable) and the
//! open page's rows as plain blocks, and does what a click asks
//! (`shell::ask("cc.act" | "cc.block" | "cc.pick" …)`). The sliders, the Sound
//! and Microphone pages, the battery and Wi-Fi's own status are read here
//! (services::audio, brightness, status).
//!
//!   {"to":"controlcenter","state":{"modules":{id:{…}},"media":{…},"cast":…,"avatar":…}}
//!   {"to":"controlcenter","page":{"name":"wifi","title":"Wi-Fi","switch":true,…,"blocks":[…]}}
//!   {"to":"controlcenter","ccPage":"focus"}      the page Panels.ccPage asks for
//!   {"to":"controlcenter","editing":true}        Panels.ccEditing

use crate::services::audio::{self, Sound};
use serde_json::Value;
use std::collections::HashMap;

fn s(v: &Value, k: &str) -> String {
    v[k].as_str().unwrap_or("").to_string()
}
fn b(v: &Value, k: &str) -> bool {
    v[k].as_bool().unwrap_or(false)
}
fn f(v: &Value, k: &str) -> f64 {
    v[k].as_f64().unwrap_or(0.0)
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MenuItem {
    pub text: String,
    pub checked: bool,
    pub separator: bool,
    pub danger: bool,
}

pub fn menu(v: &Value) -> Vec<MenuItem> {
    v.as_array().map(|a| a.iter().map(|m| MenuItem { text: s(m, "text"), checked: b(m, "checked"), separator: b(m, "sep"), danger: b(m, "danger") }).collect()).unwrap_or_default()
}

/// One module as the shell has it now
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Module {
    pub label: String,
    pub icon: String,
    pub on: bool,
    pub status: String,
    pub usable: bool,
    /// why it can't be used here ("" when it can)
    pub why: String,
    /// info tiles: the reading and what it means (CPU & Memory, Weather)
    pub title: String,
    pub sub: String,
    pub menu: Vec<MenuItem>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Media {
    pub has: bool,
    pub identity: String,
    pub title: String,
    pub artist: String,
    pub playing: bool,
    /// a local picture of the album art ("" none)
    pub art: String,
    pub can_prev: bool,
    pub can_next: bool,
    pub can_toggle: bool,
    pub can_seek: bool,
    /// seconds
    pub position: f64,
    pub length: f64,
}

/// Everything the shell says about the panel's modules
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Said {
    pub modules: HashMap<String, Module>,
    pub media: Media,
    /// a TV to cast to (or casting)
    pub cast: bool,
    pub casting: bool,
    pub cast_tip: String,
    /// your picture (a file) and name
    pub avatar: String,
    pub user: String,
    /// Sound's outputs that switch the sound card over first (AudioProfiles:
    /// a monitor's speakers while the laptop's are on): name, kind, switching
    pub profiles: Vec<Profile>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Profile {
    pub name: String,
    /// display | headphones | (speakers)
    pub kind: String,
    pub busy: bool,
}

impl Said {
    pub fn from(v: &Value) -> Self {
        let modules = v["modules"]
            .as_object()
            .map(|o| {
                o.iter()
                    .map(|(k, m)| {
                        let usable = m["usable"].as_bool().unwrap_or(true);
                        (k.clone(), Module { label: s(m, "label"), icon: s(m, "icon"), on: b(m, "on"), status: s(m, "status"), usable, why: s(m, "why"), title: s(m, "title"), sub: s(m, "sub"), menu: menu(&m["menu"]) })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let m = &v["media"];
        let media = Media {
            has: b(m, "has"),
            identity: s(m, "identity"),
            title: s(m, "title"),
            artist: s(m, "artist"),
            playing: b(m, "playing"),
            art: s(m, "art").trim_start_matches("file://").to_string(),
            can_prev: b(m, "prev"),
            can_next: b(m, "next"),
            can_toggle: b(m, "toggle"),
            can_seek: b(m, "seek"),
            position: f(m, "position"),
            length: f(m, "length"),
        };
        let profiles = v["profiles"].as_array().map(|a| a.iter().map(|p| Profile { name: s(p, "name"), kind: s(p, "kind"), busy: b(p, "busy") }).collect()).unwrap_or_default();
        Self { modules, media, cast: b(v, "cast"), casting: b(v, "casting"), cast_tip: s(v, "castTip"), avatar: s(v, "avatar").trim_start_matches("file://").to_string(), user: s(v, "user"), profiles }
    }

    /// Whether this computer can use it (until the shell says: yes)
    pub fn usable(&self, id: &str) -> bool {
        self.modules.get(id).is_none_or(|m| m.usable)
    }
}

// ── Pages ────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BlockKind {
    /// a section heading (Output, Applications…)
    Head,
    /// a list row (ListRow.qml)
    #[default]
    Row,
    /// a line of text (a hint, why it's empty, an error)
    Note,
    /// a segmented control
    Seg,
    /// a slider (an app's volume, the microphone's)
    Slider,
    /// a password field with Join (a Wi-Fi network)
    Pass,
    /// why something doesn't work, and its fix (ServiceNotice.qml)
    Notice,
    /// an app playing sound: its icon (mutes), its name
    App,
    /// room
    Gap,
    /// a text field (Sharing's name): its text in `title`, placeholder in `status`
    Field,
}

impl BlockKind {
    pub fn code(self) -> i32 {
        self as i32
    }
    fn named(t: &str) -> Option<Self> {
        Some(match t {
            "head" => Self::Head,
            "row" => Self::Row,
            "note" => Self::Note,
            "seg" => Self::Seg,
            "slider" => Self::Slider,
            "pass" => Self::Pass,
            "notice" => Self::Notice,
            "app" => Self::App,
            "gap" => Self::Gap,
            "field" => Self::Field,
            _ => return None,
        })
    }
}

/// Notes' colours
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tone {
    #[default]
    Secondary,
    Tertiary,
    Danger,
}

impl Tone {
    pub fn code(self) -> i32 {
        self as i32
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Block {
    pub kind: BlockKind,
    pub id: String,
    pub icon: String,
    /// a device's picture (a file)
    pub picture: String,
    pub title: String,
    pub status: String,
    pub highlighted: bool,
    pub trailing: String,
    pub busy: bool,
    /// a small button of its own (Share this network)
    pub action: String,
    pub disabled: bool,
    pub value: f64,
    pub tone: Tone,
    pub center: bool,
    pub options: Vec<String>,
    pub current: i32,
    pub menu: Vec<MenuItem>,
    /// a muted app; a password long enough
    pub flag: bool,
    /// Gap: its height; Head: room above (1 small, 2 more)
    pub height: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Page {
    pub name: String,
    pub title: String,
    pub switch: bool,
    pub switch_on: bool,
    pub switch_label: String,
    pub refresh: bool,
    pub blocks: Vec<Block>,
}

impl Page {
    pub fn from(v: &Value) -> Self {
        let blocks = v["blocks"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|x| {
                        let kind = BlockKind::named(x["t"].as_str()?)?;
                        Some(Block {
                            kind,
                            id: s(x, "id"),
                            icon: s(x, "icon"),
                            picture: s(x, "pic").trim_start_matches("file://").to_string(),
                            title: if kind == BlockKind::Head || kind == BlockKind::Note { s(x, "text") } else { s(x, "title") },
                            status: s(x, "status"),
                            highlighted: b(x, "hl"),
                            trailing: s(x, "trail"),
                            busy: b(x, "busy"),
                            action: s(x, "act"),
                            disabled: b(x, "dis"),
                            value: f(x, "value"),
                            tone: match x["tone"].as_str() {
                                Some("tertiary") => Tone::Tertiary,
                                Some("danger") => Tone::Danger,
                                _ => Tone::Secondary,
                            },
                            center: b(x, "center"),
                            options: x["options"].as_array().map(|o| o.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()).unwrap_or_default(),
                            current: x["cur"].as_i64().unwrap_or(0) as i32,
                            menu: menu(&x["menu"]),
                            flag: b(x, "flag"),
                            height: x["h"].as_f64().unwrap_or(if kind == BlockKind::Gap { 8.0 } else { 0.0 }) as f32,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let title = s(v, "title");
        Self {
            name: s(v, "name"),
            switch_label: v["switchLabel"].as_str().map_or_else(|| format!("Use {title}"), str::to_string),
            title,
            switch: b(v, "switch"),
            switch_on: b(v, "switchOn"),
            refresh: b(v, "refresh"),
            blocks,
        }
    }
}

/// Material's segmented lists: rows that follow one another are one group,
/// its ends round (ListRow's roundTop / roundBottom): (first, last) per block
pub fn groups(blocks: &[Block]) -> Vec<(bool, bool)> {
    let row = |i: usize| blocks.get(i).is_some_and(|b| b.kind == BlockKind::Row);
    (0..blocks.len()).map(|i| (i == 0 || !row(i - 1), !row(i + 1))).collect()
}

fn head(text: &str, above: f32) -> Block {
    Block { kind: BlockKind::Head, title: text.into(), height: above, ..Default::default() }
}

/// AudioStreams.outputIcon / inputIcon, from what the device is called
pub fn device_icon(d: &audio::Device, input: bool) -> &'static str {
    let n = format!("{} {}", d.name, d.description).to_lowercase();
    if d.bluetooth || n.contains("headset") || n.contains("headphone") {
        "headphones"
    } else if n.contains("hdmi") || n.contains("displayport") {
        "monitor"
    } else if input && n.contains("webcam") {
        "webcam"
    } else if input {
        "microphone"
    } else if n.contains("usb") {
        "speaker-hifi"
    } else {
        "laptop"
    }
}

/// The drawing of a sound device (AudioStreams.picture: Bluetooth audio by
/// its name, the computer's own, none for screens and USB), a file in
/// shell/assets/devices
pub fn device_picture(d: &audio::Device) -> String {
    let n = format!("{} {}", audio::list_name(d), d.name).to_lowercase();
    let kind = if d.bluetooth || n.contains("bluez") {
        let name = audio::list_name(d).to_lowercase();
        if name.contains("soundbar") || name.contains("sound bar") {
            "soundbar"
        } else if ["buds", "pods", "earbud", "earphone", " tws", "wf-", "liberty", "freeclip"].iter().any(|k| name.contains(k)) {
            "earbuds"
        } else {
            "headphones"
        }
    } else if ["hdmi", "displayport", "usb", "webcam", "camera"].iter().any(|k| n.contains(k)) {
        return String::new();
    } else {
        "laptop"
    };
    crate::util::asset(&format!("shell/assets/devices/{kind}.svg")).map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()
}

/// How tall a wrapped line of text gets (Slint measures wrapped text in a
/// layout as one line): its lines at `font` px in `width`, about half an em
/// a character
pub fn text_height(text: &str, font: f32, width: f32) -> f32 {
    if text.is_empty() {
        return 0.0;
    }
    let per_line = (width / (font * 0.52)).max(1.0);
    let mut lines = 0.0f32;
    for para in text.split('\n') {
        lines += (para.chars().count() as f32 / per_line).ceil().max(1.0);
    }
    (lines * (font * 1.3).round()).ceil()
}

/// The microphone part (MicSection.qml): its level, then the inputs
fn mic_blocks(snd: &Sound, out: &mut Vec<Block>, title: bool) {
    if title {
        out.push(head("Microphone", 1.0));
    }
    if snd.sources.is_empty() {
        out.push(Block { kind: BlockKind::Note, title: "No microphone found. Connect one, or check that it's turned on.".into(), center: true, ..Default::default() });
        return;
    }
    let muted = snd.mic_muted();
    out.push(Block { kind: BlockKind::Slider, id: "mic".into(), icon: if muted { "microphone-slash" } else { "microphone" }.into(), value: if muted { 0.0 } else { snd.mic_volume() }, ..Default::default() });
    out.push(Block { kind: BlockKind::Gap, height: 4.0, ..Default::default() });
    for d in &snd.sources {
        let on = d.name == snd.default_source;
        out.push(Block { kind: BlockKind::Row, id: format!("in:{}", d.name), icon: device_icon(d, true).into(), picture: device_picture(d), title: audio::list_name(d), highlighted: on, trailing: if on { "check".into() } else { String::new() }, ..Default::default() });
    }
}

/// Control Center → Sound (SoundPage.qml): the outputs, the apps playing
/// (each: its icon mutes, its volume), and the microphone when Settings
/// says (`controlCenter.micInSound`)
pub fn sound_page(snd: &Sound, profiles: &[Profile], mic_in_sound: bool) -> Page {
    let mut blocks = vec![head("Output", 0.0)];
    for d in &snd.sinks {
        blocks.push(Block { kind: BlockKind::Row, id: format!("out:{}", d.name), icon: device_icon(d, false).into(), picture: device_picture(d), title: audio::list_name(d), highlighted: d.name == snd.default_sink, ..Default::default() });
    }
    for (i, p) in profiles.iter().enumerate() {
        let icon = match p.kind.as_str() {
            "display" => "monitor",
            "headphones" => "headphones",
            _ => "speaker-hifi",
        };
        blocks.push(Block { kind: BlockKind::Row, id: format!("prof:{i}"), icon: icon.into(), title: p.name.clone(), busy: p.busy, ..Default::default() });
    }
    let streams: Vec<&audio::Stream> = snd.streams.iter().filter(|s| !audio::EVENT_PLAYERS.contains(&s.binary.as_str())).collect();
    if !streams.is_empty() {
        blocks.push(head("Applications", 1.0));
    }
    for s in streams {
        blocks.push(Block { kind: BlockKind::App, id: format!("app:{}", s.index), icon: "app-window".into(), title: s.name.clone(), flag: s.muted, ..Default::default() });
        blocks.push(Block { kind: BlockKind::Slider, id: format!("app:{}", s.index), icon: if s.muted { "speaker-x" } else { "speaker-high" }.into(), value: if s.muted { 0.0 } else { s.volume.min(1.0) }, ..Default::default() });
    }
    if mic_in_sound {
        mic_blocks(snd, &mut blocks, true);
    }
    Page { name: "sound".into(), title: "Sound".into(), blocks, ..Default::default() }
}

/// Control Center → Microphone (MicPage.qml)
pub fn mic_page(snd: &Sound) -> Page {
    let mut blocks = Vec::new();
    mic_blocks(snd, &mut blocks, false);
    Page { name: "mic".into(), title: "Microphone".into(), blocks, ..Default::default() }
}

/// The pages drawn from the sound server here (the rest come from the shell)
pub fn local(page: &str) -> bool {
    page == "sound" || page == "mic"
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn modules_from_the_shell() {
        let v = json!({"modules": {"wifi": {"label": "Wi-Fi", "icon": "wifi-high", "on": true, "status": "Home"}, "warp": {"usable": false, "why": "Needs warp-cli"}, "sharing": {"menu": [{"text": "No one", "checked": true}, {"sep": true}]}},
            "media": {"has": true, "title": "Song", "art": "file:///tmp/a.png", "playing": true}, "avatar": "file:///home/u/.face"});
        let s = Said::from(&v);
        assert!(s.modules["wifi"].on && s.usable("wifi") && s.usable("never-said"));
        assert!(!s.usable("warp"));
        assert_eq!(s.modules["warp"].why, "Needs warp-cli");
        assert_eq!(s.modules["sharing"].menu.len(), 2);
        assert!(s.modules["sharing"].menu[0].checked && s.modules["sharing"].menu[1].separator);
        assert_eq!((s.media.art.as_str(), s.avatar.as_str()), ("/tmp/a.png", "/home/u/.face"));
        assert_eq!(Said::from(&json!(null)), Said::default());
        let s = Said::from(&json!({"profiles": [{"name": "HDMI", "kind": "display", "busy": false}]}));
        assert_eq!(s.profiles[0].name, "HDMI");
    }

    #[test]
    fn wrapped_text_grows() {
        assert_eq!(text_height("", 12.0, 300.0), 0.0);
        let one = text_height("short", 12.0, 300.0);
        assert_eq!(one, 16.0);
        assert_eq!(text_height(&"word ".repeat(28), 12.0, 300.0), 3.0 * 16.0); // (140 characters, 48 a line)
        let devs = audio::Device { name: "bluez_output.x".into(), description: "WH-1000XM5".into(), bluetooth: true, ..Default::default() };
        assert!(device_picture(&devs).ends_with("headphones.svg"));
        let buds = audio::Device { description: "realme Buds T100".into(), bluetooth: true, ..Default::default() };
        assert!(device_picture(&buds).ends_with("earbuds.svg"));
    }

    #[test]
    fn pages_from_the_shell() {
        let p = Page::from(&json!({"name": "wifi", "title": "Wi-Fi", "switch": true, "switchOn": true, "blocks": [
            {"t": "row", "id": "n0", "title": "Home", "hl": true, "trail": "lock-simple"},
            {"t": "row", "id": "n1", "title": "Cafe"},
            {"t": "pass", "id": "n1"},
            {"t": "note", "text": "Wrong password", "tone": "danger"},
            {"t": "row", "id": "join", "title": "Join other network…"},
            {"t": "mystery"}]}));
        assert_eq!(p.switch_label, "Use Wi-Fi");
        assert_eq!(p.blocks.len(), 5);
        assert_eq!(p.blocks[3].title, "Wrong password");
        assert_eq!(p.blocks[3].tone, Tone::Danger);
        assert_eq!(groups(&p.blocks), vec![(true, false), (false, true), (false, true), (true, false), (true, true)]);
        let c = Page::from(&json!({"title": "Caffeine", "switchLabel": "Keep the screen awake"}));
        assert_eq!(c.switch_label, "Keep the screen awake");
    }

    fn dev(name: &str, desc: &str) -> audio::Device {
        audio::Device { name: name.into(), description: desc.into(), volume: 0.5, ..Default::default() }
    }

    #[test]
    fn the_sound_page_from_the_server() {
        let snd = Sound {
            up: true,
            default_sink: "a".into(),
            default_source: "m".into(),
            sinks: vec![dev("a", "Speakers"), dev("hdmi-b", "Monitor")],
            sources: vec![dev("m", "Built-in mic")],
            streams: vec![audio::Stream { index: 7, name: "Music".into(), volume: 0.4, ..Default::default() }, audio::Stream { index: 8, name: "click".into(), binary: "pw-play".into(), ..Default::default() }],
        };
        let p = sound_page(&snd, &[], false);
        let kinds: Vec<BlockKind> = p.blocks.iter().map(|b| b.kind).collect();
        use BlockKind::*;
        assert_eq!(kinds, vec![Head, Row, Row, Head, App, Slider]);
        assert!(p.blocks[1].highlighted && !p.blocks[2].highlighted);
        assert_eq!(p.blocks[2].icon, "monitor");
        assert_eq!(p.blocks[5].value, 0.4);
        let p = sound_page(&snd, &[], true);
        assert_eq!(p.blocks.iter().filter(|b| b.kind == Slider).count(), 2);
        assert!(p.blocks.last().unwrap().highlighted);
        let prof = [Profile { name: "Monitor speakers".into(), kind: "display".into(), busy: true }];
        let p = sound_page(&snd, &prof, false);
        assert_eq!((p.blocks[3].id.as_str(), p.blocks[3].icon.as_str(), p.blocks[3].busy), ("prof:0", "monitor", true));
        assert!(p.blocks[1].picture.ends_with("laptop.svg"));
        assert_eq!(p.blocks[2].picture, "");
        let none = mic_page(&Sound::default());
        assert_eq!(none.blocks[0].kind, Note);
    }
}

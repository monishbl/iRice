//! Notifications & Island (pages/notifications.qml), its Island subpage
//! (notifications-island.qml) and Sound when you log in
//! (notifications-startup.qml, with services/StartupSounds.js's list).

use super::super::super::model::{btn, Ctx, Opt, Page, Row};
use super::{feed, idx, play_when_still, sw};
use serde_json::{json, Value};

// ── The sounds to log in to (StartupSounds.js) ──────────────────────────

pub const STYLES: [(&str, &str); 7] = [("soft", "Soft"), ("marimba", "Marimba"), ("keys", "Keys"), ("taps", "Taps"), ("minimal", "Minimal"), ("glass", "Glass"), ("material", "Material")];

const NAMES: [(&str, &str); 28] = [
    ("soft-rise", "Updraft"), ("soft-hello", "Good Morning"), ("soft-dawn", "Daybreak"), ("soft-bell", "Vesper"),
    ("marimba-rise", "Rainfall"), ("marimba-hello", "Greeting"), ("marimba-dawn", "Tropic"), ("marimba-bell", "Temple"),
    ("keys-rise", "Arpeggio"), ("keys-hello", "Lounge"), ("keys-dawn", "Late Night"), ("keys-bell", "Music Box"),
    ("taps-rise", "Footsteps"), ("taps-hello", "Knock"), ("taps-dawn", "Heartbeat"), ("taps-bell", "Drum Call"),
    ("minimal-rise", "Ladder"), ("minimal-hello", "Blink"), ("minimal-dawn", "Pulse"), ("minimal-bell", "Morse"),
    ("glass-rise", "Crystal"), ("glass-hello", "Prism"), ("glass-dawn", "Aurora"), ("glass-bell", "Wind Chime"),
    ("material-rise", "Bloom"), ("material-hello", "Wave"), ("material-dawn", "Halo"), ("material-bell", "Ripple"),
];

const MOTIFS: [(&str, &str); 4] = [
    ("rise", "A quick run up the chord that opens into it"),
    ("hello", "Two notes a fifth apart, blooming into a chord"),
    ("dawn", "A slow swell with notes sparkling over its peak"),
    ("bell", "Three chimes, down then a leap up"),
];

#[derive(Clone, Debug, PartialEq)]
pub struct Startup {
    pub id: String,
    pub name: String,
    pub style: String,
    pub about: String,
}

/// Every choice, the original first (StartupSounds.all)
pub fn startup_sounds() -> Vec<Startup> {
    let mut out = vec![Startup { id: String::new(), name: "Tessera".into(), style: "Original".into(), about: "The chime Tessera has always played".into() }];
    for (sid, sname) in STYLES {
        for (mid, about) in MOTIFS {
            let id = format!("{sid}-{mid}");
            let name = NAMES.iter().find(|n| n.0 == id).map_or("", |n| n.1).to_string();
            out.push(Startup { id, name, style: sname.into(), about: about.into() });
        }
    }
    out
}

/// StartupSounds.find: the choice with this id, else the original
pub fn find_startup(id: &str) -> Startup {
    let all = startup_sounds();
    all.iter().find(|s| s.id == id).cloned().unwrap_or_else(|| all[0].clone())
}

// ── Notifications & Island ──────────────────────────────────────────────

/// A sound event (SoundRow): Play, Choose… (your own file), Reset, its switch
fn sound_row(cx: &mut Ctx, key: &'static str, label: &str, custom: bool, always: bool, toggle: bool, detail: Option<&str>) -> Row {
    let file = if custom { cx.s(&format!("sounds.{key}File")) } else { String::new() };
    let busy = feed(cx)["pickerBusy"].as_bool().unwrap_or(false);
    let mut kinds: Vec<&'static str> = vec!["play"];
    let mut opts = vec![btn("Play").icon("play")];
    if custom {
        kinds.push("choose");
        opts.push(btn("Choose…").off(busy));
    }
    if !file.is_empty() {
        kinds.push("reset");
        opts.push(btn("Reset"));
    }
    let id = cx.on(move |cx, a| {
        let fkey = format!("sounds.{key}File");
        if !a.s.is_empty() {
            // (the file chooser's answer)
            cx.set(&fkey, a.s.clone());
            return;
        }
        if a.i < 0 {
            let path = format!("sounds.{key}");
            let v = cx.b(&path);
            cx.set(&path, !v);
            return;
        }
        match kinds.get(a.i as usize).copied() {
            Some("play") => cx.ask("notify", "play", json!([key])),
            Some("choose") => cx.choose_file(&a.id, false, "Choose a sound", "~/Music", "Sounds|*.ogg *.oga *.wav *.mp3 *.flac"),
            Some("reset") => cx.set(&fkey, ""),
            _ => {}
        }
    });
    let shown_detail = match detail {
        Some(d) => d.to_string(),
        None => file.rsplit('/').next().unwrap_or("").to_string(),
    };
    let mut r = Row::new(label).detail(shown_detail).visible(always || cx.b("sounds.enabled")).ctl("notify", "sound", id);
    r.opts = opts;
    r.step = if toggle { 1.0 } else { 0.0 };
    r.on = cx.b(&format!("sounds.{key}"));
    r
}

pub fn notifications(cx: &mut Ctx, p: &mut Page) {
    // Do Not Disturb (Focus.on: the shell's)
    let on = feed(cx)["focus"]["on"].as_bool().unwrap_or(false);
    let dnd = cx.on(move |cx, _| {
        if on {
            cx.ask("notify", "focusOff", Value::Null);
        } else {
            cx.ask("notify", "focusOn", json!(["dnd"]));
        }
    });
    p.group("")
        .footer("While on, banners are hidden (critical alerts still show). Everything still arrives in Notification Center.")
        .row(Row::new("Do Not Disturb").switch(on, dnd));

    let island = cx.b("island.enabled");
    let link = cx.link("notifications/island", "");
    p.group("").row(
        Row::link("Island", "notifications/island")
            .icon("pill")
            .detail("Live activities, alerts, the music player and what shows when you point at it")
            .value(if island { "On" } else { "Off" })
            .id(link),
    );

    // Sound effects: one choice, off or the style they play in (Material
    // also offers its own sounds: materialSounds wins over the pack)
    let enabled = cx.b("sounds.enabled");
    let material = cx.material;
    let now = if !enabled {
        "off".to_string()
    } else if material && cx.b("appearance.materialSounds") {
        "material".to_string()
    } else {
        cx.s("sounds.pack")
    };
    let all: [(&str, &str); 8] = [("off", "Off"), ("material", "Material"), ("marimba", "Marimba"), ("taps", "Taps"), ("soft", "Soft"), ("keys", "Keys"), ("minimal", "Minimal"), ("glass", "Glass")];
    let options: Vec<(&str, &str)> = all.iter().copied().filter(|o| o.0 != "material" || material).collect();
    let labels: Vec<String> = options.iter().map(|o| o.1.to_string()).collect();
    let values: Vec<&'static str> = options.iter().map(|o| o.0).collect();
    let current = values.iter().position(|v| *v == now).map_or(-1, |i| i as i32);
    let detail = match now.as_str() {
        "off" => "No sound effects. The log-in sound is set on its own.",
        "material" if material => "Short bright blips, made for the Material look",
        "soft" => "Low, rounded tones",
        "marimba" => "Warm wooden mallets",
        "keys" => "A soft electric piano",
        "taps" => "Short, low taps",
        "minimal" => "Quiet ticks",
        "glass" => "Bright glass and pops",
        _ => "",
    };
    let choose = cx.on(move |cx, a| {
        let Some(v) = values.get(a.i.max(0) as usize).copied() else { return };
        if v != "off" {
            if cx.material {
                cx.set("appearance.materialSounds", v == "material");
            }
            if v != "material" {
                cx.set("sounds.pack", v);
            }
        }
        cx.set("sounds.enabled", v != "off");
        if v != "off" {
            play_when_still("notifications"); // (hear it, once it's saved)
        }
    });
    let volume = cx.f("sounds.volume");
    let vol = cx.on(|cx, a| {
        cx.set_num("sounds.volume", a.f as f64);
        play_when_still("charging"); // (hear the level)
    });
    p.group("Sound effects")
        .footer("Choose… plays your own file instead of the style's sound.")
        .row(Row::new("Style").detail(detail).choice(&labels, current, choose).width(200.0))
        .row(Row::new("Volume").visible(enabled).detail(format!("{}%", (volume * 100.0).round())).slider(volume, 0.0, 1.0, 0.0, vol).width(220.0));

    let count = feed(cx)["notificationCount"].as_i64().unwrap_or(0);
    let clear = cx.on(|cx, _| cx.ask("notify", "clearAll", Value::Null));
    p.group("").row(Row::new("Notification history").detail(format!("{count} kept")).button("Clear all", clear).enabled(count > 0));

    // ── More options ──
    p.more(cx);
    {
        let rows = vec![
            sound_row(cx, "charging", "Plugging in the charger", true, false, true, None),
            sound_row(cx, "displays", "Connecting a display", true, false, true, None),
            sound_row(cx, "lock", "Locking and unlocking", false, false, true, None),
            sound_row(cx, "notifications", "Notifications", false, false, true, Some("Not in a Focus mode, and not for apps that ask for silence")),
            sound_row(cx, "battery", "Low battery", false, false, true, None),
            sound_row(cx, "volumeKeys", "Volume keys", false, false, true, None),
            sound_row(cx, "usb", "USB devices", false, false, true, None),
            sound_row(cx, "received", "Files received", false, false, true, None),
            sound_row(cx, "recording", "Screen recording", false, false, true, None),
            sound_row(cx, "error", "Wrong password", false, false, true, None),
            sound_row(cx, "screenshot", "Screenshots", false, false, true, None),
            sound_row(cx, "trash", "Emptying the Trash", false, false, true, None),
        ];
        let g = p.group("Sound effects");
        for r in rows {
            g.row(r);
        }

        // Logging in: its own sound, switch and volume, apart from the effects
        let file = cx.s("sounds.startupFile");
        let chosen = find_startup(&cx.s("sounds.startupSound"));
        let link = cx.link("notifications/startup", "");
        let row = sound_row(cx, "startup", "Play it, or use your own file", true, true, false, None);
        let on = cx.b("sounds.startup");
        let level = cx.f("sounds.startupVolume");
        // (one slider: its left end is off; the volume you had is kept)
        let vol = cx.on(|cx, a| {
            if a.f < 0.025 {
                cx.set("sounds.startup", false);
                return;
            }
            cx.set_num("sounds.startupVolume", (a.f as f64).max(0.05));
            cx.set("sounds.startup", true);
            play_when_still("startup"); // (hear the level)
        });
        p.group("Logging in")
            .footer("Plays once when you log in, at its own volume, even with sound effects off.")
            .row(
                Row::link("Sound when you log in", "notifications/startup")
                    .icon("music-notes")
                    .detail(if !file.is_empty() { "Your own file".to_string() } else { format!("{} · {}", chosen.name, chosen.style) })
                    .id(link),
            )
            .row(row)
            .row(
                Row::new("Volume")
                    .detail(if on { format!("{}%", (level * 100.0).round()) } else { "Off".into() })
                    .slider(if on { level } else { 0.0 }, 0.0, 1.0, 0.0, vol)
                    .words("Off", "Loud")
                    .width(220.0),
            );

        let panel = sw(cx, "notificationCenter.background", "Notification Center panel", "");
        p.group("")
            .footer("Without it, notifications and the calendar, to-do and focus widgets are each their own piece of glass, like the controls in Quick Settings.")
            .row(panel);
    }
    p.end_more();
}

/// Sound when you log in (notifications-startup.qml)
pub fn startup(cx: &mut Ctx, p: &mut Page) {
    p.title = "Sound when you log in".into();
    let current = if !cx.s("sounds.startupFile").is_empty() { "file".to_string() } else { cx.s("sounds.startupSound") };
    let all = startup_sounds();
    let choice = |cx: &mut Ctx, s: &Startup| -> Row {
        let in_use = current == s.id;
        let id = s.id.clone();
        let act = cx.on(move |cx, a| {
            if a.i == 0 {
                cx.ask("notify", "playStartup", json!([id]));
            } else {
                cx.set("sounds.startupSound", id.clone());
                cx.set("sounds.startupFile", "");
                play_when_still("startup");
            }
        });
        let use_btn = if in_use { btn("In use").icon("check").off(true) } else { btn("Use").primary() };
        Row::new(s.name.clone()).detail(s.about.clone()).buttons(vec![btn("Play").icon("play"), use_btn], act)
    };
    let first = choice(cx, &all[0]);
    p.group("Original").footer("Any of these can play when you log in, whichever sound style you use.").row(first);
    for (_, name) in STYLES {
        let rows: Vec<Row> = all.iter().filter(|s| s.style == name).map(|s| choice(cx, s)).collect();
        let g = p.group(name);
        for r in rows {
            g.row(r);
        }
    }
}

// ── The island ──────────────────────────────────────────────────────────

/// Kinds added after the setting existed count as on until touched
/// (Island.newKinds)
const NEW_KINDS: [&str; 8] = ["usb", "displays", "audio", "temperature", "network", "transfers", "calls", "sysupdate"];

/// Island.wants: is this kind of alert on?
pub fn wants(events: &[String], known: &[String], kind: &str) -> bool {
    events.iter().any(|e| e == kind) || (NEW_KINDS.contains(&kind) && !known.iter().any(|k| k == kind))
}

/// Island.liveDefaults: newer live activities' pieces until they're changed
fn live_defaults(kind: &str) -> Option<&'static [&'static str]> {
    Some(match kind {
        "transfer" => &["icon", "name", "time", "open", "cancel"],
        "call" => &["icon", "name", "time", "mute"],
        "screenShare" => &["icon", "name", "time", "stop"],
        "sysupdate" => &["icon", "name", "time"],
        "dictation" => &["icon", "name", "time", "stop"],
        "casting" => &["icon", "name", "time", "play", "stop"],
        "mirroring" => &["icon", "name", "stop"],
        "captions" => &["icon", "name", "stop"],
        _ => return None,
    })
}

fn customised(pieces: &[String], known: &[String], kind: &str) -> bool {
    live_defaults(kind).is_none() || known.iter().any(|k| *k == format!("live.{kind}")) || pieces.iter().any(|x| x.starts_with(&format!("{kind}.")))
}

/// Island.shows: is this piece of a closed live activity on?
pub fn shows(pieces: &[String], known: &[String], kind: &str, piece: &str) -> bool {
    if customised(pieces, known, kind) {
        pieces.iter().any(|x| *x == format!("{kind}.{piece}"))
    } else {
        live_defaults(kind).is_some_and(|d| d.contains(&piece))
    }
}

/// Island.setShows: the new livePieces and known lists
pub fn set_shows(pieces: &[String], known: &[String], kind: &str, piece: &str, on: bool) -> (Vec<String>, Vec<String>) {
    let mut list: Vec<String> = pieces.to_vec();
    if !customised(pieces, known, kind) {
        list.extend(live_defaults(kind).unwrap_or(&[]).iter().map(|x| format!("{kind}.{x}")));
    }
    let k = format!("{kind}.{piece}");
    list.retain(|x| *x != k);
    if on {
        list.push(k);
    }
    let mut known = known.to_vec();
    let lk = format!("live.{kind}");
    if live_defaults(kind).is_some() && !known.contains(&lk) {
        known.push(lk);
    }
    (list, known)
}

fn strings(v: Vec<String>) -> Value {
    Value::Array(v.into_iter().map(Value::from).collect())
}

pub fn island(cx: &mut Ctx, p: &mut Page) {
    p.title = "Island".into();
    let on = cx.b("island.enabled");
    let t = cx.toggle("island.enabled");
    let layouts = ["replace", "split", "beside"];
    let layout = cx.s("island.activityLayout");
    let lp = cx.pick("island.activityLayout", &layouts);
    let mut rows = vec![
        Row::new("Island").switch(on, t),
        Row::new("Live activities").visible(on).detail("Music, timers and recording when the island is closed").segmented(&["Replace spaces", "Around", "Beside"], idx(&layouts, &layout), lp).width(270.0),
    ];
    if on {
        let events = cx.list("island.events");
        let known = cx.list("island.known");
        let can_watch = feed(cx)["networkCanWatch"].as_bool().unwrap_or(true);
        let reason = feed(cx)["networkReason"].as_str().unwrap_or("").to_string();
        let kinds: [(&'static str, &str); 17] = [
            ("music", "New song"),
            ("charging", "Charger connected or removed"),
            ("battery", "Low battery"),
            ("timers", "Focus timer"),
            ("recording", "Screen recording"),
            ("focus", "Focus mode"),
            ("bluetooth", "Bluetooth devices connect or disconnect"),
            ("usb", "USB devices and drives"),
            ("displays", "Displays connect or disconnect"),
            ("audio", "Sound moves to another output"),
            ("network", "Wi-Fi, VPN, hotspot and Ethernet changes"),
            ("transfers", "Downloads and file copies (live)"),
            ("calls", "Incoming calls, and calls and meetings in progress (live)"),
            ("sysupdate", "System updates (live, and when they finish)"),
            ("resources", "CPU or memory stays high"),
            ("temperature", "CPU running hot (alert, and the island turns red)"),
            ("notifications", "Notifications as cards in the island (instead of banners)"),
        ];
        for (k, label) in kinds {
            // (network changes need something to watch them with)
            let unavailable = k == "network" && !can_watch;
            let id = cx.on(move |cx, _| {
                let events = cx.list("island.events");
                let known = cx.list("island.known");
                let had = wants(&events, &known, k);
                let mut l: Vec<String> = events.into_iter().filter(|x| x != k).collect();
                if !had {
                    l.push(k.to_string());
                }
                cx.set("island.events", strings(l));
                if !known.iter().any(|x| x == k) {
                    let mut kn = known;
                    kn.push(k.to_string());
                    cx.set("island.known", strings(kn));
                }
            });
            rows.push(
                Row::new(label)
                    .tone(if unavailable { 1 } else { 0 })
                    .detail(if unavailable { reason.clone() } else { String::new() })
                    .switch(wants(&events, &known, k) && !unavailable, id)
                    .enabled(!unavailable),
            );
        }
    }
    let g = p.group("Island");
    g.footer("The spaces at the top centre become an island: it widens for something ongoing (music, a timer, recording) and springs open for a moment when something happens. Point at it to see what's live.");
    for r in rows {
        g.row(r);
    }

    // ── More options ──
    p.more(cx);
    {
        let modes = ["one", "all"];
        let mode = cx.s("island.activityMode");
        let mp = cx.pick("island.activityMode", &modes);
        let player_layout = cx.s("island.playerLayout");
        let art = cx.s("island.playerArt");
        let arts = ["island", "faded", "small"];
        let player = cx.on(move |cx, a| {
            let v = arts.get(a.i.max(0) as usize).copied().unwrap_or("island");
            if v == "small" {
                cx.set("island.playerLayout", "side");
            } else {
                cx.set("island.playerArt", v);
                cx.set("island.playerLayout", "overlay");
            }
        });
        let mut rows = vec![
            Row::new("When several are live")
                .visible(on)
                .detail(if mode == "all" { "The open island lists them all" } else { "One at a time; scroll over the island to switch" })
                .segmented(&["One at a time", "All"], idx(&modes, &mode), mp)
                .width(200.0),
            Row::new("Music player")
                .visible(on)
                .detail(if player_layout == "side" {
                    "Small album on the left, the song and its progress beside it"
                } else if art == "faded" {
                    "The album art fades out behind the controls"
                } else {
                    "The album art fills the island and melts into its glass"
                })
                .segmented(&["Island", "Faded", "Small"], if player_layout == "side" { 2 } else { idx(&arts, &art) }, player)
                .width(240.0),
        ];
        if on {
            let pieces = cx.list("island.livePieces");
            let known = cx.list("island.known");
            let kinds: [(&'static str, &str, &[(&'static str, &str)]); 12] = [
                ("music", "Music shows", &[("art", "Album art"), ("title", "Title"), ("scroll", "Scroll long titles"), ("bars", "Playing bars"), ("prev", "Previous"), ("play", "Play/Pause"), ("next", "Next")]),
                ("timer", "Focus timer shows", &[("icon", "Icon"), ("label", "Focus/Break"), ("time", "Time left"), ("play", "Pause"), ("skip", "Skip")]),
                ("stopwatch", "Stopwatch shows", &[("icon", "Icon"), ("time", "Time"), ("play", "Pause"), ("reset", "Reset")]),
                ("recording", "Recording shows", &[("icon", "Red dot"), ("time", "Time"), ("stop", "Stop")]),
                ("transfer", "Transfers show", &[("icon", "Icon"), ("name", "File name"), ("time", "Progress"), ("open", "Open Downloads"), ("cancel", "Cancel copy")]),
                ("call", "Calls show", &[("icon", "Icon"), ("name", "App"), ("time", "Duration"), ("mute", "Mute"), ("open", "Open"), ("leave", "Leave")]),
                ("screenShare", "Screen sharing shows", &[("icon", "Icon"), ("name", "App"), ("time", "Duration"), ("stop", "Stop")]),
                ("sysupdate", "System updates show", &[("icon", "Icon"), ("name", "Updating system"), ("time", "Progress"), ("details", "Details"), ("cancel", "Cancel")]),
                ("mirroring", "Screen mirroring shows", &[("icon", "Icon"), ("name", "TV"), ("stop", "Stop")]),
                ("casting", "Casting shows", &[("icon", "Icon"), ("name", "Title"), ("time", "Time"), ("play", "Pause"), ("stop", "Stop")]),
                ("dictation", "Dictation shows", &[("icon", "Microphone"), ("name", "Listening"), ("time", "Time"), ("stop", "Done"), ("cancel", "Cancel")]),
                ("captions", "Live captions show", &[("icon", "Icon"), ("name", "Captions"), ("stop", "Turn off")]),
            ];
            for (kind, label, list) in kinds {
                let chips: Vec<Opt> = list.iter().map(|(k, t)| Opt::new(*t).on(shows(&pieces, &known, kind, k))).collect();
                let keys: Vec<&'static str> = list.iter().map(|x| x.0).collect();
                let id = cx.on(move |cx, a| {
                    let Some(piece) = keys.get(a.i.max(0) as usize).copied() else { return };
                    let pieces = cx.list("island.livePieces");
                    let known = cx.list("island.known");
                    let now = shows(&pieces, &known, kind, piece);
                    let (l, k) = set_shows(&pieces, &known, kind, piece, !now);
                    cx.set("island.livePieces", strings(l));
                    if k != known {
                        cx.set("island.known", strings(k));
                    }
                });
                rows.push(Row::new(label).chips(chips, id).width(320.0));
            }
        }
        let events = cx.list("island.events");
        let known = cx.list("island.known");
        let hot = cx.i("island.hotTemp");
        let temp = feed(cx)["temperature"].as_f64().unwrap_or(0.0);
        let hn = cx.num("island.hotTemp", 1.0);
        rows.push(
            Row::new("Running hot at")
                .visible(on && wants(&events, &known, "temperature"))
                .detail(format!("{hot} °C · the CPU package is {}", if temp > 0.0 { format!("{} °C now", temp.round()) } else { "not reporting".into() }))
                .slider(hot as f32, 70.0, 100.0, 1.0, hn)
                .width(220.0),
        );
        rows.push(sw(cx, "island.pinned", "Keep the island open", "Also with the pin in the island, or right-click it").visible(on));
        rows.push(sw(cx, "island.hud", "Volume and brightness in the island", "Off: a separate HUD below the menu bar. Over a full-screen app, either way, a thin bar along the top edge.").visible(on));
        let fs_values = ["show", "important", "compact"];
        let fs = cx.s("island.fullscreen");
        let fs = if fs_values.contains(&fs.as_str()) { fs } else { "show".to_string() };
        let fp = cx.pick("island.fullscreen", &fs_values);
        rows.push(
            Row::new("In full screen")
                .visible(on)
                .detail(match fs.as_str() {
                    "important" => "Only calls, low battery, running hot, timers and alarms show over a video or game. The rest waits quietly in Notification Center.",
                    "compact" => "Alerts show as a small pill at the top of a video or game. Click it to see the whole alert.",
                    _ => "Alerts show over a video or game, then slide away.",
                })
                .segmented(&["Show alerts", "Only important", "Compact pill"], idx(&fs_values, &fs), fp)
                .width(330.0),
        );
        let g = p.group("Island");
        for r in rows {
            g.row(r);
        }
    }
    p.end_more();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn the_same_sounds_as_startup_sounds_js() {
        let js = std::fs::read_to_string(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../shell/services/StartupSounds.js")).unwrap();
        let all = startup_sounds();
        assert_eq!(all.len(), 1 + 7 * 4);
        for x in &all[1..] {
            assert!(js.contains(&format!("\"{}\": \"{}\"", x.id, x.name)), "{}", x.id);
        }
        for (id, name) in STYLES {
            assert!(js.contains(&format!("{{ id: \"{id}\", name: \"{name}\" }}")));
        }
        assert_eq!(find_startup("glass-bell").name, "Wind Chime");
        assert_eq!(find_startup("nothing").name, "Tessera");
    }

    #[test]
    fn new_kinds_are_on_until_touched() {
        assert!(wants(&s(&["music"]), &[], "music"));
        assert!(!wants(&[], &[], "music"));
        assert!(wants(&[], &[], "usb"));
        assert!(!wants(&[], &s(&["usb"]), "usb"));
    }

    #[test]
    fn live_pieces_as_island_qml() {
        // a newer kind shows its defaults until changed
        assert!(shows(&[], &[], "call", "mute"));
        assert!(!shows(&[], &[], "call", "leave"));
        // music isn't a newer kind: only what's listed
        assert!(!shows(&[], &[], "music", "art"));
        assert!(shows(&s(&["music.art"]), &[], "music", "art"));
        // the first change writes the defaults out
        let (l, k) = set_shows(&[], &[], "call", "leave", true);
        assert_eq!(l, s(&["call.icon", "call.name", "call.time", "call.mute", "call.leave"]));
        assert_eq!(k, s(&["live.call"]));
        let (l, _) = set_shows(&l, &k, "call", "mute", false);
        assert!(!l.contains(&"call.mute".to_string()));
        assert!(!shows(&l, &k, "call", "mute"));
    }
}

//! Sound (pages/sound.qml): where sound plays and comes from with their
//! volumes (the sound server, services/audio.rs), AirPlay speakers (the
//! shell's AirPlay service: feed.airplay), and a way to the sound effects.

use super::super::super::model::{Ctx, Opt, Page, Row};
use super::{ask, feed, s};
use crate::services::audio::{self, Cmd, Device, Sound};
use serde_json::{json, Value};

/// An AirPlay speaker's output (AirPlay.isAirPlay)
pub fn is_airplay(d: &Device) -> bool {
    d.name.starts_with("raop_sink.")
}

/// What the AirPlay row says (sound.qml's detail), and whether it's an error
pub fn airplay_detail(a: &Value) -> (String, bool) {
    let b = |k: &str| a[k].as_bool().unwrap_or(false);
    let install_error = s(&a["installError"]);
    let error = s(&a["error"]);
    let n = a["speakers"].as_i64().unwrap_or(0);
    let text = if !b("checked") {
        String::new()
    } else if !install_error.is_empty() {
        install_error.clone()
    } else if !b("installed") {
        "Needs pipewire-zeroconf. Install it, then turn this on. It asks for your password once.".into()
    } else if !error.is_empty() {
        error.clone()
    } else if !b("wanted") {
        "Off".into()
    } else if n == 0 {
        "Looking for speakers on your network…".into()
    } else if n == 1 {
        "1 speaker found".into()
    } else {
        format!("{n} speakers found")
    };
    (text, !install_error.is_empty() || !error.is_empty())
}

/// The mute button and level of a device (sound.qml's Level)
fn level(cx: &mut Ctx, d: Option<&Device>, mic: bool, label: &str) -> Row {
    let muted = d.is_some_and(|d| d.muted);
    let icon = match (mic, muted) {
        (true, true) => "microphone-slash",
        (true, false) => "microphone",
        (false, true) => "speaker-x",
        (false, false) => "speaker-high",
    };
    let dev = d.cloned();
    let id = cx.on(move |_, a| {
        let Some(d) = &dev else { return };
        if a.s == "mute" {
            audio::send(if mic { Cmd::SourceMute { index: d.index, muted: !d.muted } } else { Cmd::SinkMute { index: d.index, muted: !d.muted } });
        } else if a.s == "vol" {
            crate::shell::ask("audio.hush", json!([]));
            let v = (a.f as f64).clamp(0.0, 1.0);
            if mic {
                audio::send(Cmd::SourceMute { index: d.index, muted: false });
                audio::send(Cmd::SourceVolume { index: d.index, channels: d.channels, volume: v });
            } else {
                audio::send(Cmd::SinkMute { index: d.index, muted: false });
                audio::send(Cmd::SinkVolume { index: d.index, channels: d.channels, volume: v });
            }
        }
    });
    let mut r = Row::new(label).ctl("devices", "level", id).num(d.map_or(0.0, |d| d.volume as f32)).enabled(d.is_some());
    r.on = muted;
    // (the glyph rides in the control's data: a row's own icon is drawn before its label)
    r.data = vec![Opt::new("").icon(icon)];
    r
}

pub fn sound(cx: &mut Ctx, p: &mut Page) {
    cx.want("audio");
    let shell = cx.shell;
    let snd: Sound = shell.get::<Sound>().cloned().unwrap_or_default();
    let f = feed(cx);
    let air = &f["airplay"];

    // ── Output ──
    let sink = snd.sink().cloned();
    let vol = level(cx, sink.as_ref(), false, "Volume").detail(sink.as_ref().map(audio::list_name).unwrap_or_default());
    let mut rows = vec![vol];
    for d in &snd.sinks {
        let used = d.name == snd.default_sink;
        let r = Row::new(audio::list_name(d)).detail(if is_airplay(d) { "AirPlay" } else { "" });
        rows.push(if used {
            r.ctl("devices", "check", String::new())
        } else {
            let name = d.name.clone();
            let use_it = cx.on(move |_, _| audio::send(Cmd::DefaultSink(name.clone())));
            r.button("Use", use_it)
        });
    }
    let speakers = air["speakers"].as_i64().unwrap_or(0) > 0;
    let g = p.group("Output");
    g.footer(if speakers { "AirPlay speakers play about 2 seconds behind: fine for music, not for videos or calls." } else { "" });
    for r in rows {
        g.row(r);
    }

    // ── Input ──
    let source = snd.source().cloned();
    let mut rows = Vec::new();
    if snd.sources.is_empty() {
        rows.push(Row::new("No microphone found").detail("Connect one, or check that it's turned on").tone(1));
    } else {
        rows.push(level(cx, source.as_ref(), true, "Input volume").detail(source.as_ref().map(|d| d.description.clone()).unwrap_or_default()));
    }
    if snd.sources.len() > 1 {
        for d in &snd.sources {
            let used = d.name == snd.default_source;
            let label = if d.description.is_empty() { d.name.clone() } else { d.description.clone() };
            let r = Row::new(label);
            rows.push(if used {
                r.ctl("devices", "check", String::new())
            } else {
                let name = d.name.clone();
                let use_it = cx.on(move |_, _| audio::send(Cmd::DefaultSource(name.clone())));
                r.button("Use", use_it)
            });
        }
    }
    let g = p.group("Input");
    for r in rows {
        g.row(r);
    }

    let on = cx.b("sounds.enabled");
    let link = cx.link("notifications", "");
    p.group("").row(Row::link("Sound effects", "notifications").icon("bell-simple-ringing").detail("Alerts, the startup sound, charging and more").value(if on { "On" } else { "Off" }).id(link));

    // ── More options ──
    p.more(cx);
    {
        let mic = cx.b("controlCenter.micInSound");
        let t = cx.toggle("controlCenter.micInSound");
        p.group("Input").row(Row::new("Microphone in the Sound menu").detail("Its level and the inputs, under the outputs in Quick Settings and the menu bar's Sound menu.").switch(mic, t));

        let needs = &f["needs"];
        let checked = needs["checked"].as_bool().unwrap_or(false);
        let pipewire = needs["pipewire"].as_bool().unwrap_or(true);
        let avahi = needs["avahi"].as_bool().unwrap_or(true);
        let fixing = needs["installing"].as_bool().unwrap_or(false) || s(&f["healthFixing"]) == "avahi";
        let avahi_on = cx.on(|cx, _| ask(cx, "avahiOn", Value::Null));
        let (detail, error) = airplay_detail(air);
        let b = |k: &str| air[k].as_bool().unwrap_or(false);
        let mut speakers_row = Row::new("AirPlay speakers").visible(!checked || pipewire).detail(detail).tone(if error { 2 } else { 0 });
        if b("installed") {
            let wanted = cx.b("audio.airplay");
            speakers_row = speakers_row.switch(wanted, cx.toggle("audio.airplay"));
        } else if b("checked") {
            let install = cx.on(|cx, _| ask(cx, "airplayInstall", Value::Null));
            let installing = b("installing");
            let mut o = Opt::new(if installing { "Installing…" } else { "Install" }).icon("download-simple").on(installing);
            o.off = f["updatesRunning"].as_bool().unwrap_or(false);
            speakers_row = speakers_row.buttons(vec![o], install);
        }
        p.group("AirPlay")
            .footer("HomePods, Apple TVs and AirPlay speakers on your network appear as outputs above. Some AirPlay 2-only speakers and ones with a password may not accept the sound.")
            .row(Row::new("AirPlay needs PipeWire").visible(checked && !pipewire).detail("Your sound isn't going through PipeWire (Arch's default sound server), which AirPlay speakers come in through."))
            .row(Row::new("Network discovery is off")
                .visible(checked && pipewire && !avahi)
                .detail("AirPlay speakers are found with avahi, which isn't running. It asks for your password once.")
                .button(if fixing { "Turning on…" } else { "Turn On" }, avahi_on)
                .busy(fixing))
            .row(speakers_row);
    }
    p.end_more();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn airplay_says_where_it_stands() {
        assert_eq!(airplay_detail(&json!({})).0, "");
        assert!(airplay_detail(&json!({"checked": true, "installed": false})).0.starts_with("Needs pipewire-zeroconf"));
        assert_eq!(airplay_detail(&json!({"checked": true, "installed": true, "wanted": true, "speakers": 2})).0, "2 speakers found");
        assert_eq!(airplay_detail(&json!({"checked": true, "installed": true, "wanted": false})).0, "Off");
        assert_eq!(airplay_detail(&json!({"checked": true, "installError": "x"})), ("x".to_string(), true));
        assert!(is_airplay(&Device { name: "raop_sink.HomePod".into(), ..Default::default() }));
    }
}

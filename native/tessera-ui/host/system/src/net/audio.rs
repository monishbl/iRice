//! Default output and input, with volume helpers (replaces
//! services/Audio.qml). The sound server itself is core's services::audio
//! (pipewire-pulse's native protocol); this is the QML singleton's face:
//! the default sink and source, `icon`, and the setters, whose `quiet` form
//! tells the volume bar (OSD) not to pop up for changes made with Tessera's
//! own sliders (Audio.hush: tessera-ui's osd part gets {"hush":"sound"}).
//!
//! Calls (prefix "audio"): setVolume(v, quiet), setMicVolume(v, quiet),
//! toggleMute(quiet), toggleMic(quiet). State on the bus as "audio":
//! {volume, muted, micMuted, micVolume, icon, sinkName, sourceName, quietUntil}.

use super::common::*;
use serde_json::{json, Value};
use std::any::Any;
use tessera_core::host::{Host, Service};
use tessera_core::services::audio::{self, Cmd, Sound};

const NAME: &str = "Audio";

/// Audio.icon
pub fn icon(muted: bool, volume: f64) -> &'static str {
    if muted || volume == 0.0 {
        "speaker-x"
    } else if volume < 0.34 {
        "speaker-none"
    } else if volume < 0.67 {
        "speaker-low"
    } else {
        "speaker-high"
    }
}

enum M {
    Tick,
}

#[derive(Default)]
pub struct Audio {
    quiet_until: f64,
    last: Value,
    tick: slint::Timer,
}

impl Audio {
    fn hush(&mut self, h: &mut Host, what: &str) {
        self.quiet_until = now_ms() + 1000.0;
        h.tell("osd", json!({"hush": what})); // (tessera-ui's OSD, when it draws it)
    }

    fn publish(&mut self, h: &Host) {
        let Some(s) = h.st.shell.get::<Sound>() else { return };
        let v = json!({
            "volume": s.volume(), "muted": s.muted(), "micMuted": s.mic_muted(), "micVolume": s.mic_volume(),
            "icon": icon(s.muted(), s.volume()), "sinkName": s.sink().map(audio::name_of).unwrap_or_default(),
            "sourceName": s.source().map(audio::name_of).unwrap_or_default(), "quietUntil": self.quiet_until,
        });
        if v != self.last {
            crate::bus::put(NAME, v.clone());
            self.last = v;
        }
    }
}

impl Service for Audio {
    fn name(&self) -> &'static str {
        NAME
    }
    fn calls(&self) -> &'static [&'static str] {
        &["audio"]
    }
    fn start(&mut self, h: &mut Host) {
        h.st.shell.want("audio");
        // (the sound server's readings arrive as data; this republishes them for the other services)
        let send = h.sender(NAME);
        self.tick.start(slint::TimerMode::Repeated, std::time::Duration::from_millis(400), move || send(Box::new(M::Tick)));
    }
    fn message(&mut self, h: &mut Host, msg: Box<dyn Any + Send>) {
        if msg.downcast::<M>().is_ok() {
            self.publish(h);
        }
    }
    fn call(&mut self, h: &mut Host, call: &str, args: &Value) {
        let a = args_list(args);
        let quiet = a.get(1).and_then(Value::as_bool).unwrap_or(false);
        let toggle_quiet = a.first().and_then(Value::as_bool).unwrap_or(false);
        // (a slider being dragged: its own sound/OSD stays quiet)
        if call == "audio.hush" {
            return self.hush(h, "sound");
        }
        let Some(snd) = h.st.shell.get::<Sound>().cloned() else { return };
        let v = a.first().and_then(Value::as_f64).unwrap_or(0.0).clamp(0.0, 1.0);
        match call.strip_prefix("audio.").unwrap_or("") {
            "setVolume" => {
                if quiet {
                    self.hush(h, "sound");
                }
                if let Some(d) = snd.sink() {
                    audio::send(Cmd::SinkMute { index: d.index, muted: false });
                    audio::send(Cmd::SinkVolume { index: d.index, channels: d.channels, volume: v });
                }
            }
            "setMicVolume" => {
                if quiet {
                    self.hush(h, "sound");
                }
                if let Some(d) = snd.source() {
                    audio::send(Cmd::SourceMute { index: d.index, muted: false });
                    audio::send(Cmd::SourceVolume { index: d.index, channels: d.channels, volume: v });
                }
            }
            "toggleMute" => {
                if toggle_quiet {
                    self.hush(h, "sound");
                }
                if let Some(d) = snd.sink() {
                    audio::send(Cmd::SinkMute { index: d.index, muted: !d.muted });
                }
            }
            "toggleMic" => {
                if toggle_quiet {
                    self.hush(h, "sound");
                }
                if let Some(d) = snd.source() {
                    audio::send(Cmd::SourceMute { index: d.index, muted: !d.muted });
                }
            }
            _ => {}
        }
    }
    fn quit(&mut self, _h: &mut Host) {
        self.tick.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_speaker_icon_follows_the_level() {
        assert_eq!(icon(true, 0.5), "speaker-x");
        assert_eq!(icon(false, 0.0), "speaker-x");
        assert_eq!(icon(false, 0.2), "speaker-none");
        assert_eq!(icon(false, 0.5), "speaker-low");
        assert_eq!(icon(false, 0.9), "speaker-high");
    }
}

//! Apps playing sound, and the outputs and inputs they can use (replaces
//! services/AudioStreams.qml). The streams, outputs and inputs are core's
//! services::audio::Sound; what the QML read from PipeWire's link graph
//! (which output each stream plays to) is read here with
//! `pactl list short sink-inputs` / `sinks`, and moving a stream is
//! `pactl move-sink-input <index> <sink>` (a sink input's index is the
//! stream's `index` in Sound, so the QML's object.id lookup isn't needed).
//!
//! Calls (prefix "audiostreams"): move(streamIndex, sinkName), refresh.
//! State on the bus as "audiostreams": {canMove, outputs:[{name,index,label,icon,bluetooth}],
//! inputs:[{name,index,label,icon,bluetooth}], streams:[{index,name,icon,binary,volume,muted,output}]}
//! ("output": the sink name it plays to, "" while unknown).

use super::common::*;
use serde_json::{json, Value};
use std::any::Any;
use std::collections::BTreeMap;
use tessera_core::host::{Host, Service};
use tessera_core::services::audio::{self, Sound, EVENT_PLAYERS};

const NAME: &str = "AudioStreams";

pub fn output_icon(label: &str) -> &'static str {
    if label.to_lowercase().contains("headphone") { "headphones" } else { "speaker-hifi" }
}

pub fn input_icon(label: &str, name: &str) -> &'static str {
    let t = format!("{label} {name}").to_lowercase();
    if t.contains("webcam") || t.contains("camera") || t.contains("video") {
        "webcam"
    } else if t.contains("bluez") || t.contains("headset") {
        "headphones"
    } else {
        "microphone"
    }
}

/// `pactl list short sink-inputs` (index, sink id, …) and `sinks` (id, name, …)
/// to stream index -> sink name
pub fn links(inputs: &str, sinks: &str) -> BTreeMap<u32, String> {
    let names: BTreeMap<&str, &str> = sinks.lines().filter_map(|l| {
        let f: Vec<&str> = l.split('\t').collect();
        Some((*f.first()?, *f.get(1)?))
    }).collect();
    inputs.lines().filter_map(|l| {
        let f: Vec<&str> = l.split('\t').collect();
        Some((f.first()?.parse().ok()?, names.get(f.get(1)?)?.to_string()))
    }).collect()
}

enum M {
    Links(BTreeMap<u32, String>),
}

#[derive(Default)]
pub struct AudioStreams {
    links: BTreeMap<u32, String>,
    tick: slint::Timer,
    last: Value,
    busy: bool,
}

impl AudioStreams {
    fn refresh(&mut self, h: &mut Host) {
        if self.busy || !has_tool("pactl") {
            return;
        }
        self.busy = true;
        let send = h.sender(NAME);
        std::thread::spawn(move || {
            let t = std::time::Duration::from_secs(5);
            let a = crate::util::capture(&["pactl", "list", "short", "sink-inputs"], t).map(|x| x.1).unwrap_or_default();
            let b = crate::util::capture(&["pactl", "list", "short", "sinks"], t).map(|x| x.1).unwrap_or_default();
            send(Box::new(M::Links(links(&a, &b))));
        });
    }

    fn publish(&mut self, h: &Host) {
        let Some(s) = h.st.shell.get::<Sound>() else { return };
        let dev = |d: &audio::Device, out: bool| {
            let label = audio::list_name(d);
            json!({"name": d.name, "index": d.index, "label": label, "icon": if out { output_icon(&label) } else { input_icon(&label, &d.name) }, "bluetooth": d.bluetooth})
        };
        let streams: Vec<Value> = s.streams.iter().filter(|x| !EVENT_PLAYERS.contains(&x.binary.as_str()))
            .map(|x| json!({"index": x.index, "name": x.name, "icon": x.icon, "binary": x.binary, "volume": x.volume, "muted": x.muted, "channels": x.channels, "output": self.links.get(&x.index).cloned().unwrap_or_default()}))
            .collect();
        let v = json!({"canMove": has_tool("pactl"), "outputs": s.sinks.iter().map(|d| dev(d, true)).collect::<Vec<_>>(),
                       "inputs": s.sources.iter().map(|d| dev(d, false)).collect::<Vec<_>>(), "streams": streams});
        if v != self.last {
            crate::bus::put(NAME, v.clone());
            self.last = v;
        }
    }
}

impl Service for AudioStreams {
    fn name(&self) -> &'static str {
        NAME
    }
    fn calls(&self) -> &'static [&'static str] {
        &["audiostreams"]
    }
    fn start(&mut self, h: &mut Host) {
        h.st.shell.want("audio");
        let send = h.sender(NAME);
        self.tick.start(slint::TimerMode::Repeated, std::time::Duration::from_millis(1000), move || send(Box::new(())));
    }
    fn message(&mut self, h: &mut Host, msg: Box<dyn Any + Send>) {
        match msg.downcast::<M>() {
            Ok(m) => {
                let M::Links(l) = *m;
                self.busy = false;
                self.links = l;
            }
            Err(_) => {
                // (a second's tick: the streams move rarely; links only while some play)
                if h.st.shell.get::<Sound>().is_some_and(|s| !s.streams.is_empty()) {
                    self.refresh(h);
                }
            }
        }
        self.publish(h);
    }
    fn call(&mut self, h: &mut Host, call: &str, args: &Value) {
        let a = args_list(args);
        match call.strip_prefix("audiostreams.").unwrap_or("") {
            // Fire and forget: the caller watches the stream's output change
            // and treats no change after a few seconds as failure
            "move" => {
                let (idx, sink) = (str_arg(&a, 0), str_arg(&a, 1));
                if !idx.is_empty() && !sink.is_empty() && has_tool("pactl") {
                    crate::util::run_async(&["pactl", "move-sink-input", &idx, &sink]);
                }
            }
            "refresh" => self.refresh(h),
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
    fn streams_and_their_sinks() {
        let l = links("12\t3\tmodule-native\ts16le\n15\t4\tx\n", "3\talsa_output.speakers\tm\n4\tbluez_output.buds\tm\n");
        assert_eq!(l[&12], "alsa_output.speakers");
        assert_eq!(l[&15], "bluez_output.buds");
        assert_eq!(output_icon("Sony Headphones"), "headphones");
        assert_eq!(input_icon("HD Webcam", "alsa_input.usb"), "webcam");
        assert_eq!(input_icon("Built-in", "alsa_input.pci"), "microphone");
    }
}

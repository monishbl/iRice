//! shell/services/Privacy.qml: who is using the microphone, the camera or
//! your location, and who is sharing the screen (the menu bar's privacy dots
//! and the island's "Sharing screen" activity).
//!   mic       PipeWire/PulseAudio recording streams (not monitors), looked at
//!             again 150 ms after `pactl subscribe` reports a change
//!   camera    assets/scripts/camera.sh (watch.sh), or `privacy.camera [apps]`
//!             from the events service
//!   location  assets/scripts/location.sh, or `privacy.location [apps]`
//!   screen    assets/scripts/screenshare.sh (PipeWire's graph), or
//!             `privacy.screenshare [graph]`; Hyprland's screencast event only
//!             hints at WHAT is shared (resolved with hyprctl on a thread)
//! Published on the bus as "privacy" (the QML property names, computed ones
//! too) and into the shell's state as privacy.{any,camera,mic,location,share}
//! (the menu bar's bridge says the same).
//! Calls: privacy.stopSharing, privacy.camera, privacy.location,
//! privacy.screenshare, privacy.pulseEvent [line].

use super::helpers::{self, Tick, Ticker, Tx};
use crate::util;
use serde_json::{json, Value};
use std::any::Any;
use std::io::{BufRead, BufReader};
use std::time::Duration;
use tessera_core::host::{Host, Service};

/// A name as the dots show it: "Firefox", "An app" when empty
pub fn nice(name: &str) -> String {
    let n = name.strip_suffix(".exe").or_else(|| name.strip_suffix(".bin")).unwrap_or(name);
    let mut c = n.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => "An app".into(),
    }
}

/// A location client as the dot names it ("?" is unknown)
pub fn location_name(a: &str) -> String {
    if a == "?" {
        "An app".into()
    } else {
        nice(a.strip_suffix(".desktop").unwrap_or(a).rsplit('.').next().unwrap_or(""))
    }
}

/// What a camera.sh / location.sh line says: "[a,b]" -> ["a","b"]
pub fn list_line(line: &str) -> Vec<String> {
    line.trim_start_matches('[').trim_end_matches(']').split(',').filter(|s| !s.is_empty()).map(str::to_string).collect()
}

/// The recording streams of `pactl -f json list source-outputs` + `list
/// sources short` output: (apps, streams [{name, bin, pid}])
pub fn parse_mic(text: &str) -> (Vec<String>, Vec<Value>) {
    let parts: Vec<&str> = text.split('\n').filter(|l| l.trim().starts_with('[')).collect();
    let outs: Vec<Value> = parts.first().and_then(|p| serde_json::from_str(p).ok()).unwrap_or_default();
    let sources: Vec<Value> = parts.get(1).and_then(|p| serde_json::from_str(p).ok()).unwrap_or_default();
    let monitors: Vec<&Value> = sources.iter().filter(|s| s["name"].as_str().unwrap_or("").ends_with(".monitor")).map(|s| &s["index"]).collect();
    let mut apps: Vec<String> = Vec::new();
    let mut streams = Vec::new();
    for o in &outs {
        let p = &o["properties"];
        let bin = p["application.process.binary"].as_str().unwrap_or("");
        if monitors.contains(&&o["source"]) || matches!(bin, "tessera-shell" | "irice-shell" | "qs" | "quickshell") || p["media.name"] == "Peak detect" {
            continue;
        }
        let aname = p["application.name"].as_str().unwrap_or("");
        let n = nice(if aname.is_empty() { bin } else { aname });
        if !apps.contains(&n) {
            apps.push(n);
        }
        let pid = p["application.process.id"].as_str().and_then(|s| s.parse::<i64>().ok()).or_else(|| p["application.process.id"].as_i64()).unwrap_or(0);
        streams.push(json!({"name": aname, "bin": bin, "pid": pid}));
    }
    (apps, streams)
}

fn props<'a>(o: &'a Value) -> &'a Value {
    &o["info"]["props"]
}

/// The shares in a PipeWire graph (Privacy.readGraph), given what was known
/// (to keep a share's start and target) and the hint for new ones
pub fn read_graph(graph: &[Value], known: &[Value], hint: (String, bool), now: f64) -> Vec<Value> {
    let null = Value::Null;
    let by_id = |id: &Value| graph.iter().find(|o| &o["id"] == id).unwrap_or(&null);
    let links: Vec<&Value> = graph.iter().filter(|o| o["type"].as_str().unwrap_or("").ends_with(":Link") && o["info"].is_object() && o["info"]["state"] != "error").collect();
    let mut out = Vec::new();
    for n in graph {
        if !n["type"].as_str().unwrap_or("").ends_with(":Node") || !props(n)["node.name"].as_str().unwrap_or("").starts_with("xdph-streaming-") {
            continue;
        }
        let s = |v: &Value, k: &str| v[k].as_str().unwrap_or("").to_string();
        // (Tessera's own screen recorder, through the portal, isn't a share)
        let mine: Vec<&&Value> = links
            .iter()
            .filter(|l| {
                if l["info"]["output-node-id"] != n["id"] {
                    return false;
                }
                let pp = props(by_id(&l["info"]["input-node-id"]));
                let cp = props(by_id(&pp["client.id"]));
                let t = format!("{} {} {} {}", s(pp, "application.process.binary"), s(cp, "application.process.binary"), s(pp, "application.name"), s(cp, "application.name")).to_lowercase();
                !(t.contains("gpu-screen-recorder") || t.contains("gsr"))
            })
            .collect();
        if mine.is_empty() {
            continue; // a stream nobody receives (yet): not shared
        }
        // The receiving app: its node's name, else its client's
        let names: Vec<String> = mine
            .iter()
            .map(|l| {
                let pp = props(by_id(&l["info"]["input-node-id"]));
                let cp = props(by_id(&pp["client.id"]));
                for (a, k) in [(pp, "application.name"), (cp, "application.name"), (pp, "application.process.binary"), (cp, "application.process.binary"), (pp, "node.name")] {
                    if !s(a, k).is_empty() {
                        return s(a, k);
                    }
                }
                String::new()
            })
            .filter(|x| !x.is_empty())
            .collect();
        let was = known.iter().find(|x| x["node"] == n["id"]);
        let (target, window) = match was {
            Some(w) => (w["target"].as_str().unwrap_or("").to_string(), w["window"].as_bool().unwrap_or(false)),
            None => hint.clone(),
        };
        out.push(json!({
            "node": n["id"],
            "app": names.first().map(|x| nice(x)).unwrap_or_else(|| "An app".into()),
            "target": target,
            "window": window,
            "links": mine.iter().map(|l| l["id"].clone()).collect::<Vec<_>>(),
            "started": was.map(|w| w["started"].clone()).unwrap_or(json!(now)),
        }));
    }
    out
}

enum Ev {
    Pulse(String),
    Mic(String),
    Camera(Vec<String>),
    Location(Vec<String>),
    Graph(Vec<Value>),
    ShareDead,
    CastRaw(String),
    Cast { at: f64, target: String, window: bool },
}

#[derive(Default)]
struct LastCast {
    owner: String,
    name: String,
    at: f64,
    target: String,
    window: bool,
}

pub struct Privacy {
    tx: Option<Tx>,
    mic_apps: Vec<String>,
    mic_streams: Vec<Value>,
    camera_apps: Vec<String>,
    location_apps: Vec<String>,
    shares: Vec<Value>,
    last_cast: Option<LastCast>,
    stopping: bool,
    stop_failed: bool,
    mic_refresh: Ticker,
    share_retry: Ticker,
    stop_check: Ticker,
    mic_busy: bool,
}

impl Default for Privacy {
    fn default() -> Self {
        Self {
            tx: None,
            mic_apps: vec![],
            mic_streams: vec![],
            camera_apps: vec![],
            location_apps: vec![],
            shares: vec![],
            last_cast: None,
            stopping: false,
            stop_failed: false,
            mic_refresh: Ticker::new("micRefresh"),
            share_retry: Ticker::new("shareRetry"),
            stop_check: Ticker::new("stopCheck"),
            mic_busy: false,
        }
    }
}

impl Privacy {
    fn indicator(h: &Host) -> String {
        h.cfg()["privacy"]["screenShareIndicator"].as_str().unwrap_or("island").to_string()
    }

    /// Publish the computed properties (bus + shell state)
    fn publish(&mut self, h: &mut Host) {
        let shared = !self.shares.is_empty();
        let ind = Self::indicator(h);
        let enabled = helpers::island_enabled(h);
        let share_in_island = shared && enabled && ind != "dot";
        let share_dot = shared && (!enabled || ind != "island");
        let (mic, camera, location) = (!self.mic_apps.is_empty(), !self.camera_apps.is_empty(), !self.location_apps.is_empty());
        let any = mic || camera || location || share_dot;
        if !shared {
            self.stop_failed = false;
        }
        let first = self.shares.first();
        crate::bus::put(
            "privacy",
            json!({
                "micApps": self.mic_apps, "micStreams": self.mic_streams, "cameraApps": self.camera_apps, "locationApps": self.location_apps,
                "shares": self.shares, "screenShared": shared,
                "screenApp": first.map(|s| s["app"].clone()).unwrap_or(json!("")),
                "screenTarget": first.map(|s| s["target"].clone()).unwrap_or(json!("")),
                "screenStarted": first.map(|s| s["started"].clone()).unwrap_or(json!(0)),
                "shareIndicator": ind, "shareInIsland": share_in_island, "shareDot": share_dot,
                "mic": mic, "camera": camera, "location": location, "any": any,
                "stopping": self.stopping, "stopFailed": self.stop_failed,
            }),
        );
        h.set_state("privacy.any", json!(any));
        h.set_state("privacy.camera", json!(camera));
        h.set_state("privacy.mic", json!(mic));
        h.set_state("privacy.location", json!(location));
        h.set_state("privacy.share", json!(share_dot));
    }

    fn mic_list(&mut self) {
        if self.mic_busy {
            return;
        }
        self.mic_busy = true;
        let tx = self.tx.clone().unwrap();
        std::thread::spawn(move || {
            let out = util::sh("pactl -f json list source-outputs; echo; pactl -f json list sources short", Duration::from_secs(10)).map(|(_, o)| o).unwrap_or_default();
            tx(Box::new(Ev::Mic(out)));
        });
    }

    fn watch_share(&self) {
        let tx = self.tx.clone().unwrap();
        let tx2 = tx.clone();
        helpers::watch(
            vec![util::script("screenshare.sh")],
            move |line| {
                if let Ok(Value::Array(g)) = serde_json::from_str::<Value>(&line) {
                    tx(Box::new(Ev::Graph(g)));
                }
            },
            move || tx2(Box::new(Ev::ShareDead)),
        );
    }

    fn cast_hint(&self) -> (String, bool) {
        match &self.last_cast {
            Some(c) if helpers::now_ms() - c.at <= 4000.0 => (c.target.clone(), c.window),
            _ => ("Your screen".into(), false),
        }
    }

    fn resolve_cast(&self, owner: String, name: String, at: f64) {
        let tx = self.tx.clone().unwrap();
        std::thread::spawn(move || {
            let (target, window) = if owner == "1" {
                let o = util::capture(&["hyprctl", "-j", "clients"], Duration::from_secs(3)).map(|(_, o)| o).and_then(|o| serde_json::from_str::<Value>(&o).ok()).unwrap_or(Value::Null);
                let c = o.as_array().and_then(|a| a.iter().find(|c| !name.is_empty() && (c["address"] == name.as_str() || format!("{}", c["address"].as_str().unwrap_or("")) == format!("0x{name}") || c["address"].as_str().map(|a| a.to_string()) == Some(format!("0x{name}")))).cloned());
                let t = c.as_ref().map(|c| if !c["title"].as_str().unwrap_or("").is_empty() { c["title"].as_str().unwrap().to_string() } else { c["class"].as_str().unwrap_or("").to_string() }).unwrap_or_default();
                (if !t.is_empty() { t } else if !name.is_empty() { name.clone() } else { "A window".into() }, true)
            } else {
                let o = util::capture(&["hyprctl", "-j", "monitors"], Duration::from_secs(3)).map(|(_, o)| o).and_then(|o| serde_json::from_str::<Value>(&o).ok()).unwrap_or(Value::Null);
                let m = o.as_array().and_then(|a| a.iter().find(|m| m["name"] == name.as_str()).cloned());
                let t = match m {
                    Some(m) => {
                        let d = m["description"].as_str().unwrap_or("");
                        if d.is_empty() { name.clone() } else { d.to_string() }
                    }
                    None => if name.is_empty() { "Your screen".into() } else { name.clone() },
                };
                (t, false)
            };
            tx(Box::new(Ev::Cast { at, target, window }));
        });
    }

    fn graph_from_args(args: &Value) -> Vec<Value> {
        match args {
            Value::Array(a) if a.first().is_some_and(Value::is_array) => a[0].as_array().cloned().unwrap_or_default(),
            Value::Array(a) => a.clone(),
            _ => vec![],
        }
    }

    fn apps_from_args(args: &Value) -> Vec<String> {
        let a = match args {
            Value::Array(a) if a.first().is_some_and(Value::is_array) => a[0].as_array().cloned().unwrap_or_default(),
            Value::Array(a) => a.clone(),
            _ => vec![],
        };
        a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()
    }

    fn set_shares(&mut self, h: &mut Host, out: Vec<Value>) {
        if out != self.shares {
            self.shares = out;
        }
        if self.shares.is_empty() {
            self.stopping = false;
        }
        self.publish(h);
    }

    /// Stop sharing: cut the stream's links, so the app gets no more frames
    /// (the menu says to stop it there if the app holds on)
    fn stop_sharing(&mut self, h: &mut Host) {
        let ids: Vec<String> = self.shares.iter().flat_map(|s| s["links"].as_array().cloned().unwrap_or_default()).map(|l| l.to_string().trim_matches('"').to_string()).collect();
        if ids.is_empty() {
            return;
        }
        self.stopping = true;
        self.stop_failed = false;
        let mut cmd = vec!["sh".to_string(), "-c".into(), "for l in \"$@\"; do pw-cli destroy \"$l\" >/dev/null 2>&1; done".into(), "sh".into()];
        cmd.extend(ids);
        let c: Vec<&str> = cmd.iter().map(String::as_str).collect();
        tessera_core::services::bridge::run(&c);
        self.stop_check.once(self.tx.as_ref().unwrap(), 2500);
        self.publish(h);
    }
}

impl Service for Privacy {
    fn name(&self) -> &'static str {
        "Privacy"
    }
    fn calls(&self) -> &'static [&'static str] {
        &["privacy"]
    }

    fn start(&mut self, h: &mut Host) {
        let tx = helpers::tx(h, "Privacy");
        self.tx = Some(tx.clone());
        // Microphone: the shell's pactl subscribe (services/PulseEvents),
        // here its own line reader so this crate stands alone
        let t = tx.clone();
        std::thread::spawn(move || loop {
            use std::process::{Command, Stdio};
            if let Ok(mut c) = Command::new("pactl").arg("subscribe").stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn() {
                if let Some(o) = c.stdout.take() {
                    for l in BufReader::new(o).lines().map_while(Result::ok) {
                        t(Box::new(Ev::Pulse(l)));
                    }
                }
                let _ = c.wait();
            }
            std::thread::sleep(Duration::from_secs(5));
        });
        self.mic_list();
        // Camera and location (their scripts; the events service may also say
        // them through the calls)
        let (t, t2) = (tx.clone(), tx.clone());
        let _ = t2;
        helpers::watch(vec![util::script("camera.sh")], move |l| t(Box::new(Ev::Camera(list_line(&l)))), || {});
        let t = tx.clone();
        helpers::watch(vec![util::script("location.sh")], move |l| t(Box::new(Ev::Location(list_line(&l).iter().map(|a| a.to_string()).collect()))), || {});
        self.watch_share();
        // Hyprland's screencast event
        let t = tx.clone();
        helpers::hypr_events(&["screencast", "screencastv2"], move |_, data| t(Box::new(Ev::CastRaw(data))));
        self.publish(h);
    }

    fn config_changed(&mut self, h: &mut Host) {
        self.publish(h);
    }

    fn call(&mut self, h: &mut Host, call: &str, args: &Value) {
        match call {
            "privacy.stopSharing" => self.stop_sharing(h),
            "privacy.camera" => {
                self.camera_apps = Self::apps_from_args(args).iter().map(|a| nice(a)).collect();
                self.publish(h);
            }
            "privacy.location" => {
                self.location_apps = Self::apps_from_args(args).iter().map(|a| location_name(a)).collect();
                self.publish(h);
            }
            "privacy.screenshare" => {
                let g = Self::graph_from_args(args);
                let hint = self.cast_hint();
                let out = read_graph(&g, &self.shares, hint, helpers::now_ms());
                self.set_shares(h, out);
            }
            "privacy.pulseEvent" => {
                if let Some(l) = args.as_array().and_then(|a| a.first()).and_then(Value::as_str).or(args.as_str()) {
                    if l.contains("source-output") || l.contains("'source'") {
                        self.mic_refresh.once(self.tx.as_ref().unwrap(), 150);
                    }
                }
            }
            _ => {}
        }
    }

    fn message(&mut self, h: &mut Host, msg: Box<dyn Any + Send>) {
        let msg = match msg.downcast::<Tick>() {
            Ok(t) => {
                match t.0 {
                    "micRefresh" => self.mic_list(),
                    "shareRetry" => {
                        self.shares.clear();
                        self.publish(h);
                        self.watch_share();
                    }
                    "stopCheck" => {
                        self.stop_failed = !self.shares.is_empty();
                        self.stopping = false;
                        self.publish(h);
                    }
                    _ => {}
                }
                return;
            }
            Err(m) => m,
        };
        let Ok(ev) = msg.downcast::<Ev>() else { return };
        match *ev {
            Ev::Pulse(l) => {
                // (a burst of events, one look: 150 ms)
                if l.contains("source-output") || l.contains("'source'") {
                    self.mic_refresh.once(self.tx.as_ref().unwrap(), 150);
                }
            }
            Ev::Mic(t) => {
                self.mic_busy = false;
                let (a, s) = parse_mic(&t);
                self.mic_apps = a;
                self.mic_streams = s;
                self.publish(h);
            }
            Ev::Camera(a) => {
                self.camera_apps = a.iter().map(|x| nice(x)).collect();
                self.publish(h);
            }
            Ev::Location(a) => {
                self.location_apps = a.iter().map(|x| location_name(x)).collect();
                self.publish(h);
            }
            Ev::Graph(g) => {
                let hint = self.cast_hint();
                let out = read_graph(&g, &self.shares, hint, helpers::now_ms());
                self.set_shares(h, out);
            }
            Ev::ShareDead => {
                // PipeWire restarted: forget the shares in 5 s, then watch again
                self.share_retry.once(self.tx.as_ref().unwrap(), 5000);
            }
            Ev::CastRaw(data) => {
                // screencast>>STATE,OWNER  (newer: screencastv2>>STATE,OWNER,NAME)
                let f: Vec<&str> = data.split(',').collect();
                if f.first() == Some(&"1") {
                    let owner = f.get(1).copied().unwrap_or("0").to_string();
                    let name = f.iter().skip(2).copied().collect::<Vec<_>>().join(",");
                    let at = helpers::now_ms();
                    self.last_cast = Some(LastCast { owner: owner.clone(), name: name.clone(), at, target: if owner == "1" { if name.is_empty() { "A window".into() } else { name.clone() } } else if name.is_empty() { "Your screen".into() } else { name.clone() }, window: owner == "1" });
                    self.resolve_cast(owner, name, at);
                }
            }
            Ev::Cast { at, target, window } => {
                if let Some(c) = self.last_cast.as_mut().filter(|c| c.at == at) {
                    c.target = target;
                    c.window = window;
                    let _ = (&c.owner, &c.name);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(nice("firefox"), "Firefox");
        assert_eq!(nice("obs.bin"), "Obs");
        assert_eq!(nice(""), "An app");
        assert_eq!(location_name("?"), "An app");
        assert_eq!(location_name("org.gnome.Maps.desktop"), "Maps");
        assert_eq!(list_line("[a,b]"), vec!["a", "b"]);
        assert!(list_line("[]").is_empty());
    }

    #[test]
    fn mic_streams_skip_monitors_and_the_shell() {
        let t = "[{\"source\":1,\"properties\":{\"application.name\":\"discord\",\"application.process.binary\":\"discord\",\"application.process.id\":\"42\"}},{\"source\":2,\"properties\":{\"application.name\":\"x\"}},{\"source\":1,\"properties\":{\"application.process.binary\":\"qs\"}}]\n\n[{\"index\":1,\"name\":\"mic\"},{\"index\":2,\"name\":\"out.monitor\"}]\n";
        let (a, s) = parse_mic(t);
        assert_eq!(a, vec!["Discord"]);
        assert_eq!(s[0]["pid"], 42);
    }

    #[test]
    fn a_stream_with_a_receiver_is_a_share() {
        let g: Vec<Value> = serde_json::from_str(
            r#"[{"id":1,"type":"PipeWire:Interface:Node","info":{"props":{"node.name":"xdph-streaming-DP-1"}}},
                {"id":2,"type":"PipeWire:Interface:Node","info":{"props":{"node.name":"in","application.name":"zoom"}}},
                {"id":3,"type":"PipeWire:Interface:Link","info":{"state":"active","output-node-id":1,"input-node-id":2}}]"#,
        )
        .unwrap();
        let out = read_graph(&g, &[], ("Your screen".into(), false), 5.0);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0]["app"], "Zoom");
        assert_eq!(out[0]["links"][0], 3);
    }
}

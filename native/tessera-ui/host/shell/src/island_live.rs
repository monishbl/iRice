//! The live activities whose owners are other host services (LiveInfo.qml
//! and Island.activities): mirroring, casting, dictation, live captions in
//! the island, a call in progress, screen sharing, a system update and file
//! transfers. Each is read from the state its owner publishes and given
//! LiveInfo's icon, colour, title, pill time, detail, progress and controls.
//! (Recording, the timer and the stopwatch are island.rs's `derive_live`;
//! music is the menu bar's own, from MPRIS.)
//!
//! A control's `act` is what island.rs's `run` takes: a host call
//! (`{"call", "args"}`, optionally `"then"` a second one) or an IPC line.

use crate::common::*;
use serde_json::{json, Value};
use tessera_core::host::Host;

/// "m:ss" (Timers.format)
pub fn fmt(secs: i64) -> String {
    let s = secs.max(0);
    format!("{}:{:02}", s / 60, s % 60)
}

/// Sizes in powers of a thousand (Transfers.size)
fn size(bytes: f64) -> String {
    let u = ["B", "KB", "MB", "GB", "TB"];
    let (mut v, mut i) = (bytes.max(0.0), 0);
    while v >= 1000.0 && i < u.len() - 1 {
        v /= 1000.0;
        i += 1;
    }
    if i == 0 {
        format!("{} {}", v.round(), u[i])
    } else {
        format!("{:.*} {}", if v < 10.0 { 1 } else { 0 }, v, u[i])
    }
}

fn ctl(piece: &str, icon: &str, fill: bool, on: bool, danger: bool, name: &str, act: Value) -> Value {
    json!({"piece": piece, "icon": icon, "fill": fill, "on": on, "danger": danger, "name": name, "act": act})
}

fn call(c: &str, args: Value) -> Value {
    json!({"call": c, "args": args})
}

fn str_of(v: &Value, k: &str) -> String {
    v[k].as_str().unwrap_or("").to_string()
}

/// Seconds since a start in ms (0: none yet)
fn since(start_ms: i64, now: i64) -> String {
    fmt(if start_ms > 0 { (now - start_ms) / 1000 } else { 0 })
}

/// Every other kind, in Island.activities' order (None: not happening)
pub fn derive(h: &Host, now: i64) -> Vec<(&'static str, Option<Value>)> {
    let dark = h.st.shell.look.dark;
    let accent = |n: &str| tessera_core::theme::accent_hex(n, dark);
    let mut out: Vec<(&'static str, Option<Value>)> = Vec::new();

    // ── Mirroring (sharing: Mirroring.active)
    let m = h.state("mirroring");
    out.push((
        "mirroring",
        m["active"].as_bool().unwrap_or(false).then(|| {
            let name = str_of(m, "targetName");
            json!({
                "icon": "screencast", "color": "",
                "title": format!("Mirroring to {name}"), "time": "",
                "detail": if str_of(m, "state") == "streaming" { format!("Your screen is showing on {name}") } else { format!("Connecting to {name}…") },
                "progress": -1.0,
                "controls": [ctl("stop", "stop", true, true, true, "Stop mirroring", call("share.mirrorStop", json!([])))],
            })
        }),
    ));

    // ── Casting (sharing: Cast.active)
    let c = h.state("cast");
    out.push((
        "casting",
        c["active"].as_bool().unwrap_or(false).then(|| {
            let state = str_of(c, "state");
            let (t, d) = (c["time"].as_f64().unwrap_or(0.0), c["duration"].as_f64().unwrap_or(0.0));
            let playing = state == "playing";
            let device = c["device"]["name"].as_str().filter(|s| !s.is_empty()).unwrap_or("the TV").to_string();
            let what = if playing {
                format!("{}{}", fmt(t as i64), if d > 0.0 { format!(" of {}", fmt(d as i64)) } else { String::new() })
            } else if state == "paused" {
                "Paused".into()
            } else {
                "Connecting…".into()
            };
            json!({
                "icon": "closed-captioning", "color": "",
                "title": c["title"].as_str().filter(|s| !s.is_empty()).unwrap_or("Casting"),
                "time": if playing || state == "paused" { fmt(t as i64) } else { String::new() },
                "detail": format!("On {device} · {what}"),
                "progress": if d > 0.0 { (t / d).min(1.0) } else { -1.0 },
                "controls": [
                    ctl("play", if playing { "pause" } else { "play" }, true, playing || state == "paused", false, if playing { "Pause" } else { "Play" }, call("share.castToggle", json!([]))),
                    ctl("stop", "stop", true, true, true, "Stop casting", call("share.castStop", json!([]))),
                ],
            })
        }),
    ));

    // ── Dictation (Dictation.active: listening or writing)
    let dc = h.state("dictation");
    let stage = str_of(dc, "stage");
    out.push((
        "dictation",
        (!stage.is_empty()).then(|| {
            let writing = stage == "writing";
            json!({
                "icon": if writing { "text-aa" } else { "microphone" }, "color": accent("red"),
                "title": if writing { "Writing…" } else { "Listening" },
                "time": if writing { String::new() } else { since(dc["startedAt"].as_f64().unwrap_or(0.0) as i64, now) },
                "detail": if writing { "Turning your words into text" } else { "Press Super+H again when you're done" },
                "progress": -1.0,
                "controls": [
                    ctl("cancel", "x", false, true, false, "Cancel dictation", call("dictation.cancel", json!([]))),
                    ctl("stop", "check", true, stage == "listening", false, "Stop and type it", call("dictation.stop", json!([]))),
                ],
            })
        }),
    ));

    // ── Live captions shown in the island (captions.placement "island")
    let cp = h.state("livecaptions");
    out.push((
        "captions",
        cp["on"].as_bool().unwrap_or(false).then(|| {
            let latest = str_of(cp, "latest");
            json!({
                "icon": "closed-captioning", "color": "",
                "title": if latest.is_empty() { "Live captions".to_string() } else { latest },
                "time": "",
                "detail": if !str_of(cp, "error").is_empty() { str_of(cp, "error") } else if !cp["ready"].as_bool().unwrap_or(false) { "Set them up in Settings → Accessibility".into() } else { str_of(cp, "previous") },
                "progress": -1.0,
                "lines": cp["lines"].clone(),
                "controls": [ctl("stop", "x", false, true, false, "Turn off live captions", call("captions", json!(["off"])))],
            })
        }),
    ));

    // ── A call in progress (Calls.active)
    let cl = h.state("calls");
    out.push((
        "call",
        cl["active"].as_bool().unwrap_or(false).then(|| {
            let muted = h.state("audio.micMuted").as_bool().unwrap_or(false);
            let camera = h.state("privacy.camera").as_bool().unwrap_or(false);
            let mut controls = vec![
                ctl("mute", if muted { "microphone-slash" } else { "microphone" }, true, true, false, if muted { "Unmute" } else { "Mute" }, call("audio.toggleMic", json!([true]))),
                ctl("open", "arrow-square-out", false, !cl["window"].is_null() && cl["window"] != "", false, "Open the call", call("calls.open", json!([]))),
            ];
            if cl["leave"].as_bool().unwrap_or(false) || cl["leave"].as_str().is_some_and(|s| !s.is_empty()) {
                controls.push(ctl("leave", "phone-x", true, true, true, "Leave the call", call("calls.leave", json!([]))));
            }
            let started = cl["started"].as_f64().unwrap_or(0.0) as i64;
            json!({
                "icon": if camera { "video-camera" } else { "phone" }, "color": accent("green"),
                "title": cl["name"].as_str().filter(|s| !s.is_empty()).unwrap_or("Call"),
                "time": since(started, now),
                "detail": format!("{}{}", since(started, now), if muted { " · Muted" } else { "" }),
                "appIcon": cl["appIcon"].clone(),
                "progress": -1.0,
                "controls": controls,
            })
        }),
    ));

    // ── Screen sharing shown in the island (Privacy.shareInIsland)
    let ps = h.state("privacy.screen");
    out.push((
        "screenShare",
        ps["inIsland"].as_bool().unwrap_or(false).then(|| {
            let app = str_of(ps, "app");
            let n = ps["count"].as_i64().unwrap_or(1);
            let started = ps["started"].as_f64().unwrap_or(0.0) as i64;
            json!({
                "icon": if ps["window"].as_bool().unwrap_or(false) { "app-window" } else { "screencast" }, "color": "#BF5AF2",
                "title": format!("Sharing screen · {app}{}", if n > 1 { format!(" and {} more", n - 1) } else { String::new() }),
                "time": since(started, now),
                "detail": if ps["stopFailed"].as_bool().unwrap_or(false) { format!("Still sharing: stop it in {app}") } else { format!("{} · {}", str_of(ps, "target"), since(started, now)) },
                "progress": -1.0,
                "controls": [ctl("stop", "stop", true, !ps["stopping"].as_bool().unwrap_or(false), true, "Stop sharing", call("privacy.stopSharing", json!([])))],
            })
        }),
    ));

    // ── A system update running (Updates.running and the sysupdate kind wanted)
    let up = h.state("sysupdate");
    out.push((
        "sysupdate",
        up["running"].as_bool().unwrap_or(false).then(|| {
            let job = &up["job"];
            let phase = str_of(job, "phase");
            let pkg = str_of(job, "package");
            let mut controls = vec![ctl("details", "terminal-window", false, true, false, "Show the update's details", json!({"call": "updates.showDetails", "args": [true], "then": {"call": "panels.openSettings", "args": ["update"]}}))];
            if up["cancellable"].as_bool().unwrap_or(false) {
                controls.push(ctl("cancel", "x", false, true, false, "Cancel the update", call("updates.cancel", json!([]))));
            }
            let p = up["progress"].as_f64().unwrap_or(0.0).max(0.0);
            json!({
                "icon": "arrow-circle-up", "color": accent("blue"),
                "title": format!("Updating · {}", up["step"].as_str().filter(|s| !s.is_empty()).unwrap_or("System")),
                "time": if phase == "auth" { "Password".to_string() } else { format!("{}%", (p * 100.0).round()) },
                "detail": format!("{}{}", str_of(job, "text"), if !pkg.is_empty() && (phase == "install" || phase == "download") { format!(" · {pkg}") } else { String::new() }),
                "progress": p,
                "controls": controls,
            })
        }),
    ));

    // ── File transfers (Transfers.active)
    let tr = h.state("transfers");
    out.push((
        "transfer",
        tr["active"].as_bool().unwrap_or(false).then(|| {
            let f = &tr["first"];
            let kind = str_of(f, "kind");
            let n = tr["items"].as_array().map_or(1, Vec::len);
            let peer = str_of(f, "peer");
            let what = match kind.as_str() {
                "copy" => if str_of(f, "cmd") == "mv" { "Moving".to_string() } else { "Copying".into() },
                "send" => if f["waiting"].as_bool().unwrap_or(false) { format!("Waiting for {peer}") } else { format!("Sending to {peer}") },
                "receive" => format!("Receiving from {peer}"),
                _ => "Downloading".into(),
            };
            let (done, total) = (f["done"].as_f64().unwrap_or(0.0), f["total"].as_f64().unwrap_or(0.0));
            let speed = tr["speed"].as_f64().unwrap_or(0.0);
            let detail = if f["waiting"].as_bool().unwrap_or(false) && !str_of(f, "pin").is_empty() {
                format!("{what} · PIN {}", str_of(f, "pin"))
            } else {
                let amount = if total > 0.0 { format!("{} of {}", size(done), size(total)) } else { size(done) };
                let note = str_of(f, "note");
                format!("{what} · {amount}{}{}", if speed > 0.0 { format!(" · {}/s", size(speed)) } else { String::new() }, if note.is_empty() { String::new() } else { format!(" · {note}") })
            };
            let progress = tr["progress"].as_f64().unwrap_or(-1.0);
            let controls = if kind == "copy" || kind == "send" {
                vec![ctl("cancel", "x", false, true, false, "Cancel", call("share.transferCancel", json!([f])))]
            } else {
                vec![ctl("open", "folder-open", false, true, false, "Open Downloads", call("share.openDownloads", json!([])))]
            };
            json!({
                "icon": match kind.as_str() { "copy" => "copy", "send" => "paper-plane-tilt", _ => "download-simple" }, "color": accent("blue"),
                "title": format!("{}{}", str_of(f, "name"), if n > 1 { format!(" and {} more", n - 1) } else { String::new() }),
                "time": if progress >= 0.0 { format!("{}%", (progress * 100.0).round()) } else if total > 0.0 { format!("{}%", (100.0 * done / total).round()) } else { size(done) },
                "detail": detail,
                "progress": progress,
                "controls": controls,
            })
        }),
    ));
    out
}

/// The second line of the open card for the host's own kinds: a timer
/// "Focus · paused", "Stopwatch · 2 laps", "Recording · mic off"
/// (IslandFocusCard's caption over the big number)
pub fn caption(kind: &str, a: &Value, h: &Host) -> String {
    match kind {
        "timer" => {
            let paused = !h.state("timers.running").as_bool().unwrap_or(false);
            format!("{}{}", a["title"].as_str().unwrap_or("Focus"), if paused { " · paused" } else { "" })
        }
        "stopwatch" => {
            let laps = h.state("timers.sw.laps").as_i64().unwrap_or(0);
            format!("Stopwatch{}", if laps > 0 { format!(" · {laps} {}", if laps == 1 { "lap" } else { "laps" }) } else { String::new() })
        }
        "recording" => {
            let mic = !cfg_str(h.cfg(), "recorder.mic", "").is_empty();
            let muted = h.state("recorder.micMuted").as_bool().unwrap_or(false);
            format!("Recording{}", if mic && muted { " · mic off" } else { "" })
        }
        _ => a["title"].as_str().unwrap_or("").to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_and_times() {
        assert_eq!(fmt(65), "1:05");
        assert_eq!(fmt(-3), "0:00");
        assert_eq!(size(999.0), "999 B");
        assert_eq!(size(1500.0), "1.5 KB");
        assert_eq!(size(12_400_000.0), "12 MB");
    }
}

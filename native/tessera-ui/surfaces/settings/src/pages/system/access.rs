//! Accessibility (pages/accessibility.qml): live captions (the shell's
//! Captions and Dictation services say what's installed and downloading:
//! feed.system.captions; installing and downloading are theirs: cx.ask) and
//! a link to Shake to find the pointer (its home is Mouse & Trackpad).

use super::super::super::model::{btn, Ctx, Page, Row};
use serde_json::Value;

fn s(v: &Value) -> String {
    v.as_str().unwrap_or("").to_string()
}

/// The captions row's detail (accessibility.qml's Live captions)
pub fn captions_detail(has_binary: bool, has_model: bool, error: &str, enabled: bool, placement: &str) -> String {
    if !has_binary {
        "Install speech recognition above first".into()
    } else if !has_model {
        "Download the Tiny model above first".into()
    } else if !error.is_empty() {
        error.into()
    } else if !enabled {
        "Off".into()
    } else {
        match placement {
            "island" => "In the island at the top: the last words, a few lines when it's open",
            "strip" => "Across the bottom of the screen; windows make room for them",
            "video" => "Over the bottom of the window that's playing, up to three lines, like a film's subtitles",
            _ => "In a bar above the Dock. Drag it anywhere.",
        }
        .into()
    }
}

pub fn accessibility(cx: &mut Ctx, p: &mut Page) {
    let feed = cx.feed;
    let c = &feed["system"]["captions"];
    let b = |k: &str| c[k].as_bool().unwrap_or(false);
    let (has_binary, has_model) = (b("hasBinary"), b("hasModel"));
    let enabled = cx.b("captions.enabled");
    let placement = {
        let p = cx.s("captions.placement");
        if p.is_empty() { "bar".to_string() } else { p }
    };
    let error = s(&c["error"]);
    let updates_running = feed["system"]["updatesRunning"].as_bool().unwrap_or(false);

    // ── Live captions ──
    let mut rows = Vec::new();
    if !has_binary {
        let ie = s(&c["installError"]);
        let installing = b("installing");
        let id = cx.on(|cx, _| cx.ask("system", "captionsInstall", Value::Null));
        rows.push(
            Row::new("Speech recognition")
                .detail(if ie.is_empty() { "Install whisper.cpp to turn speech into text on this computer. It asks for your password once.".to_string() } else { ie.clone() })
                .tone(if ie.is_empty() { 0 } else { 2 })
                .buttons(vec![btn(if installing { "Installing…" } else { "Install" }).icon("download-simple").primary().on(installing).off(updates_running)], id),
        );
    }
    if has_binary && !has_model {
        let downloading = b("downloading");
        let derr = s(&c["downloadError"]);
        let detail = if downloading {
            format!("Downloading… {}%", (c["downloadProgress"].as_f64().unwrap_or(0.0) * 100.0).round())
        } else if !derr.is_empty() && s(&c["lastDownload"]) == "tiny" {
            derr
        } else {
            "75 MB, downloaded once. Fast enough to keep up with speech.".into()
        };
        let other = !s(&c["dictDownloading"]).is_empty();
        let id = cx.on(move |cx, _| cx.ask("system", if downloading { "captionsCancel" } else { "captionsDownload" }, Value::Null));
        let mut button = btn(if downloading { "Cancel" } else { "Download" }).off(!downloading && other);
        if !downloading {
            button = button.icon("download-simple").primary();
        }
        rows.push(Row::new("Tiny speech model").detail(detail).buttons(vec![button], id));
    }
    // One choice: off, or where they show
    let values = ["off", "bar", "video", "island", "strip"];
    let current = if !enabled { 0 } else { values.iter().position(|v| *v == placement).unwrap_or(1).max(1) as i32 };
    let id = cx.on(move |cx, a| {
        if a.i > 0 {
            cx.set("captions.placement", values[a.i.clamp(0, 4) as usize]);
        }
        cx.set("captions.enabled", a.i > 0);
    });
    rows.push(
        Row::new("Live captions")
            .detail(captions_detail(has_binary, has_model, &error, enabled, &placement))
            .tone(if error.is_empty() { 0 } else { 2 })
            .segmented(&["Off", "Floating", "Over video", "Island", "Bottom"], current, id)
            .width(380.0)
            .enabled(b("ready") || enabled),
    );
    let g = p.group("Live captions");
    g.footer("Subtitles for what this computer plays: videos, calls, podcasts. Made on this computer with the Tiny speech model, about 2 seconds behind; nothing is saved or sent anywhere. The microphone is never captioned. Super+Ctrl+L turns them on and off.");
    for r in rows {
        g.row(r);
    }

    // ── The pointer: set in Mouse & Trackpad ──
    let style = cx.s("findPointer.style");
    let value = if !cx.b("findPointer.enabled") || style == "off" {
        "Off"
    } else if style == "grow" {
        "Bigger"
    } else {
        "Ring"
    };
    let link = cx.link("mouse", "Shake to find the pointer");
    p.group("Pointer").row(Row::link("Shake to find the pointer", "mouse").target("Shake to find the pointer").icon("cursor").detail("In Mouse & Trackpad").value(value).id(link));

    // ── More options ──
    p.more(cx);
    {
        let mut langs: Vec<(String, String)> = vec![("auto".into(), "Automatic".into())];
        if let Some(a) = c["languages"].as_array() {
            for l in a {
                let v = s(&l["value"]);
                if !v.is_empty() && v != "auto" {
                    langs.push((v, s(&l["label"])));
                }
            }
        }
        let cur = {
            let l = cx.s("captions.language");
            if l.is_empty() { "auto".to_string() } else { l }
        };
        let labels: Vec<String> = langs.iter().map(|l| l.1.clone()).collect();
        let index = langs.iter().position(|l| l.0 == cur).map_or(-1, |i| i as i32);
        let lid = cx.pick_values("captions.language", langs.iter().map(|l| Value::from(l.0.clone())).collect());
        let moved = placement == "bar" && (cx.f("captions.barX") >= 0.0 || cx.f("captions.barY") >= 0.0);
        let back = cx.on(|cx, _| {
            cx.set_num("captions.barX", -1.0);
            cx.set_num("captions.barY", -1.0);
        });
        let mut lang = Row::new("Language").detail("Automatic suits videos in different languages; choosing one is more accurate").choice(&labels, index, lid).width(220.0);
        if index < 0 {
            lang = lang.placeholder(cur);
        }
        p.group("Live captions").row(lang).row(Row::new("The bar's place").visible(moved).detail("Moved by you").button("Put It Back", back));
    }
    p.end_more();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captions_say_what_they_need() {
        assert_eq!(captions_detail(false, false, "", false, "bar"), "Install speech recognition above first");
        assert_eq!(captions_detail(true, false, "", false, "bar"), "Download the Tiny model above first");
        assert_eq!(captions_detail(true, true, "", false, "bar"), "Off");
        assert!(captions_detail(true, true, "", true, "strip").starts_with("Across the bottom"));
        assert_eq!(captions_detail(true, true, "oops", true, "bar"), "oops");
    }
}

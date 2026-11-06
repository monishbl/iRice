//! Dictation (pages/dictation.qml, a part of Keyboard): what it still
//! needs, the speech model, the language, the sounds. The shell's Dictation
//! service knows what's installed and downloads (feed `dictation`, `tools`,
//! `updatesRunning`); installs and downloads are its (`dictInstall`,
//! `dictDownload`, `dictCancel`, `dictRemove`, `dictRefresh`).

use super::super::super::model::{btn, Ctx, Opt, Page, Row};
use super::{feed, G};
use serde_json::json;

/// dictate.py's models: (name, label, MB, what it's like)
pub const MODELS: &[(&str, &str, u32, &str)] = &[
    ("tiny", "Tiny", 75, "Fastest, makes more mistakes"),
    ("base", "Base", 142, "Quick, good for most dictation"),
    ("small", "Small", 466, "More accurate, a little slower"),
    ("medium", "Medium", 1500, "Accurate; slow without a fast processor"),
    ("large-v3-turbo", "Large (turbo)", 1620, "Most accurate; best with a fast processor"),
];

pub const LANGUAGES: &[(&str, &str)] = &[
    ("", "Same as the system"),
    ("auto", "Automatic"),
    ("en", "English"),
    ("hi", "Hindi"),
    ("es", "Spanish"),
    ("fr", "French"),
    ("de", "German"),
    ("it", "Italian"),
    ("pt", "Portuguese"),
    ("nl", "Dutch"),
    ("ru", "Russian"),
    ("ar", "Arabic"),
    ("zh", "Chinese"),
    ("ja", "Japanese"),
    ("ko", "Korean"),
    ("ta", "Tamil"),
    ("te", "Telugu"),
    ("bn", "Bengali"),
    ("ur", "Urdu"),
    ("tr", "Turkish"),
    ("pl", "Polish"),
    ("uk", "Ukrainian"),
];

/// A model's size: "142 MB", "1.5 GB"
pub fn size(mb: u32) -> String {
    if mb >= 1000 {
        format!("{:.1} GB", mb as f64 / 1000.0)
    } else {
        format!("{mb} MB")
    }
}

pub fn dictation(cx: &mut Ctx, p: &mut Page) {
    // (Dictation.refresh() as the page opens: once while Settings is open)
    if !cx.local_b("dict.refreshed") {
        cx.local_set("dict.refreshed", true);
        cx.ask(G, "dictRefresh", json!([]));
    }
    let d = feed(cx)["dictation"].clone();
    let tools = feed(cx)["tools"].clone();
    let updating = feed(cx)["updatesRunning"].as_bool().unwrap_or(false);
    let has_binary = d["hasBinary"].as_bool().unwrap_or(false);
    let ready = d["ready"].as_bool().unwrap_or(false);
    let installing = d["installing"].as_bool().unwrap_or(false);
    let install_error = d["installError"].as_str().unwrap_or("").to_string();
    let install = cx.on(|cx, _| cx.ask(G, "dictInstall", json!([])));
    let install2 = cx.on(|cx, _| cx.ask(G, "dictInstall", json!([])));
    p.group("")
        .footer("Press Super+H, speak, then press it again. Or hold Super+H while you speak and let go when you're done. The words are typed where your cursor is. Everything stays on this computer.")
        .row(Row::new("Speech recognition")
            .visible(!has_binary)
            .detail(if install_error.is_empty() { "Install whisper.cpp to turn speech into text on this computer. It asks for your password once.".to_string() } else { install_error.clone() })
            .tone(if install_error.is_empty() { 0 } else { 2 })
            .buttons(vec![btn(if installing { "Installing…" } else { "Install" }).icon("download-simple").primary().on(installing).off(updating)], install))
        .row(Row::new("Typing into apps needs wtype")
            .visible(has_binary && tools["ready"] == true && tools["wtype"] != true)
            .detail("Without it, what you say goes on the clipboard to paste. It asks for your password once.")
            .buttons(vec![btn(if installing { "Installing…" } else { "Install" }).on(installing).off(updating)], install2))
        .row(Row::new(if ready { "Ready" } else { "Download a speech model to start" })
            .visible(has_binary)
            .detail(if ready { "Super+H to dictate" } else { "Speech recognition is installed" })
            .tone(if ready { 0 } else { 1 })
            .ctl(G, "icon", String::new())
            .data(vec![Opt::new("").icon(if ready { "check-circle" } else { "info" }).on(ready).style(if ready { 3 } else { 1 })]));

    // The speech model
    let have = d["models"].clone();
    let chosen_cfg = cx.s("dictation.model");
    let model = if have.get(&chosen_cfg).is_some() { chosen_cfg } else { "base".to_string() };
    let downloading = d["downloading"].as_str().unwrap_or("").to_string();
    let progress = d["downloadProgress"].as_f64().unwrap_or(0.0);
    let dl_error = d["downloadError"].as_str().unwrap_or("").to_string();
    let last = d["lastDownload"].as_str().unwrap_or("").to_string();
    let mut rows = Vec::new();
    for (value, label, mb, what) in MODELS {
        let chosen = model == *value;
        let has = have[*value].as_bool().unwrap_or(false);
        let loading = downloading == *value;
        let detail = if loading {
            format!("Downloading… {}%", (progress * 100.0).round())
        } else if !dl_error.is_empty() && downloading.is_empty() && last == *value && !has {
            dl_error.clone()
        } else {
            format!("{what} · {}{}", size(*mb), if has { " · downloaded" } else { "" })
        };
        // Use, Remove (downloaded), Download or Cancel (not yet)
        let mut b = Vec::new();
        let mut ops: Vec<&'static str> = Vec::new();
        if has && !chosen {
            b.push(btn("Use"));
            ops.push("use");
        }
        if has {
            b.push(btn("Remove").danger());
            ops.push("remove");
        } else {
            let mut x = btn(if loading { "Cancel" } else { "Download" });
            if !loading {
                x = x.icon("download-simple");
            }
            if !loading && chosen && has_binary {
                x = x.primary();
            }
            b.push(x.off(!(loading || downloading.is_empty())));
            ops.push(if loading { "cancel" } else { "download" });
        }
        let v = value.to_string();
        let id = cx.on(move |cx, a| match ops.get(a.i.max(0) as usize).copied() {
            Some("use") => cx.set("dictation.model", v.clone()),
            Some("remove") => cx.ask(G, "dictRemove", json!([v])),
            Some("cancel") => cx.ask(G, "dictCancel", json!([])),
            Some("download") => cx.ask(G, "dictDownload", json!([v])),
            _ => {}
        });
        rows.push(Row::new(format!("{label}{}", if chosen { " · in use" } else { "" })).detail(detail).buttons(b, id));
    }
    let g = p.group("Speech model");
    g.footer("Bigger models make fewer mistakes but take longer after you stop talking. Models are downloaded once and kept in ~/.local/share/tessera/whisper.");
    for r in rows {
        g.row(r);
    }

    let lang = cx.s("dictation.language");
    let labels: Vec<String> = LANGUAGES.iter().map(|l| l.1.to_string()).collect();
    let current = LANGUAGES.iter().position(|l| l.0 == lang).map_or(-1, |i| i as i32);
    let values: Vec<&str> = LANGUAGES.iter().map(|l| l.0).collect();
    let lp = cx.pick("dictation.language", &values);
    let sounds = cx.b("dictation.sounds");
    let st = cx.toggle("dictation.sounds");
    p.group("")
        .row(Row::new("Language")
            .detail("The language you speak. Automatic works for most, but may mistake a short sentence for another language.")
            .choice(&labels, current, lp)
            .width(220.0))
        .row(Row::new("Play sounds").detail("A sound as it starts and stops listening").switch(sounds, st));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lists_are_dictations() {
        let qml = std::fs::read_to_string(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../shell/services/Dictation.qml")).unwrap();
        for (v, l, mb, d) in MODELS {
            assert!(qml.contains(&format!("{{ value: \"{v}\", label: \"{l}\", mb: {mb}, detail: \"{d}\" }}")), "{v}");
        }
        for (v, l) in LANGUAGES {
            assert!(qml.contains(&format!("{{ value: \"{v}\", label: \"{l}\" }}")), "{v}");
        }
        assert_eq!(size(142), "142 MB");
        assert_eq!(size(1500), "1.5 GB");
    }
}

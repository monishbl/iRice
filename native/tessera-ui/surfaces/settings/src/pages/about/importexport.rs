//! Import and export for Keyboard Shortcuts and Gestures
//! (ImportExport.qml): one row per section (keybinds = changes to defaults,
//! shortcuts = your own, gestures), "Everything", and "Import from end4".
//! Imports show what will change first; Apply makes it so, with Undo after.
//! The shortcuts are the shell's (services/Keymap.qml writes them and
//! Hyprland's files): RustBridgeAbout.qml keeps this part's state per
//! section set (feed.about.ie[key]) and does each step (`ie.*`), the file
//! dialog (zenity or kdialog, else a path typed here) included.
//!   `super::super::about::importexport::group(cx, p, &["gestures"], "Import and export gestures", true)`
//! (Keyboard: all three sections, "Import and export")

use super::super::super::model::{btn, Ctx, Opt, Page, Row};
use super::{b, feed, n, s};
use serde_json::{json, Value};

fn name(sec: &str) -> &'static str {
    match sec {
        "keybinds" => "Changes to default shortcuts",
        "shortcuts" => "Your shortcuts",
        _ => "Gestures",
    }
}
fn detail(sec: &str) -> &'static str {
    match sec {
        "keybinds" => "Keys you changed or turned off",
        "shortcuts" => "Shortcuts you added",
        _ => "Trackpad gestures you changed or added",
    }
}

/// The state's key for a set of sections
pub fn key(sections: &[&str]) -> String {
    sections.join(",")
}

/// Shown only when the shortcuts are read (Keymap.ready), as the pages do
pub fn ready(cx: &Ctx) -> bool {
    b(&feed(cx)["keymap"]["ready"])
}

pub fn group(cx: &mut Ctx, p: &mut Page, sections: &[&str], title: &str, end4: bool) {
    let f = feed(cx);
    let k = key(sections);
    let st = &f["ie"][&k];
    let busy = b(&st["busy"]);
    let error = s(&st["error"]);
    let status = s(&st["status"]);
    let preview = &st["preview"];
    let path_mode = s(&st["pathMode"]);
    let dialog = b(&f["keymap"]["dialog"]);
    let all: Vec<String> = sections.iter().map(|x| x.to_string()).collect();

    // One row per section, Everything, end4
    let mut rows = Vec::new();
    for sec in sections {
        let (kk, one) = (k.clone(), vec![sec.to_string()]);
        let act = cx.on(move |cx, a| cx.ask("about", if a.i == 0 { "ie.import" } else { "ie.export" }, json!([kk, one])));
        rows.push(Row::new(name(sec)).detail(detail(sec)).buttons(vec![btn("Import…").off(busy), btn("Export…").off(busy)], act));
    }
    let (kk, secs) = (k.clone(), all.clone());
    let every = cx.on(move |cx, a| cx.ask("about", if a.i == 0 { "ie.import" } else { "ie.export" }, json!([kk, secs])));
    let (kk, secs) = (k.clone(), all.clone());
    let e4 = cx.on(move |cx, _| cx.ask("about", "ie.end4", json!([kk, secs])));
    let g = p.group(title);
    g.footer(if dialog { "" } else { "Install zenity or kdialog for a file dialog; until then, type the file's path." });
    for r in rows {
        g.row(r);
    }
    g.row(Row::new("Everything").visible(sections.len() > 1).detail("All of the above in one file").buttons(vec![btn("Import…").off(busy), btn("Export all").off(busy)], every));
    g.row(Row::new("end4 (illogical-impulse)")
        .visible(end4)
        .detail(if sections.contains(&"keybinds") {
            "Reads ~/.config/hypr/hyprland/keybinds.lua and custom/keybinds.lua, and the gestures in general.lua"
        } else {
            "Reads the gestures in ~/.config/hypr/hyprland/general.lua and custom/general.lua"
        })
        .buttons(vec![btn("Import from end4").on(busy).off(busy)], e4));

    // No file dialog: a path
    if !path_mode.is_empty() {
        let typed_key = format!("about.ie.path.{k}");
        let typed = cx.local.get(&typed_key).and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| s(&st["pathText"]));
        let (kk, tk) = (k.clone(), typed_key.clone());
        let field = cx.on(move |cx, a| {
            cx.local_set(&tk, a.s.clone());
            if a.i == 1 && !a.s.trim().is_empty() {
                cx.ask("about", "ie.path", json!([kk, Value::Null, a.s.trim()]));
            }
        });
        let (kk, tk) = (k.clone(), typed_key.clone());
        let go = cx.on(move |cx, a| {
            if a.i == 0 {
                let t = cx.local_s(&tk);
                cx.ask("about", "ie.path", json!([kk, Value::Null, t]));
            } else {
                cx.local.remove(&tk);
                cx.ask("about", "ie.pathCancel", json!([kk]));
            }
        });
        let mut r = Row::new(if path_mode == "export" { "Save as" } else { "Open file" }).ctl("about", "path", field);
        r.text = typed.clone();
        r.placeholder = "~/Documents/tessera-shortcuts.json".into();
        r.width = 260.0;
        r.value = go;
        r.opts = vec![btn(if path_mode == "export" { "Export" } else { "Import" }).primary().off(typed.trim().is_empty() || busy), btn("Cancel")];
        p.group("").row(r);
    }

    // Errors and results
    let kk = k.clone();
    let dismiss = cx.on(move |cx, _| cx.ask("about", "ie.dismiss", json!([kk])));
    p.group("").row(Row::new("Couldn't import or export").visible(!error.is_empty()).detail(error).buttons(vec![btn("").icon("x")], dismiss));
    let undo_snap = !s(&st["undoSnap"]).is_empty();
    let kk = k.clone();
    let undo = cx.on(move |cx, _| cx.ask("about", "ie.undo", json!([kk])));
    if !status.is_empty() && preview.is_null() {
        let r = Row::new(status);
        p.group("").row(if undo_snap { r.buttons(vec![btn("Undo").icon("arrow-counter-clockwise")], undo) } else { r });
    }

    // Preview
    if !preview.is_null() {
        let show_all = cx.local_b(&format!("about.ie.all.{k}"));
        let show_skipped = cx.local_b(&format!("about.ie.skipped.{k}"));
        let lines: Vec<Value> = preview["lines"].as_array().cloned().unwrap_or_default();
        let skipped: Vec<Value> = preview["skipped"].as_array().cloned().unwrap_or_default();
        let total = n(&preview["total"]);
        let kk = k.clone();
        let decide = cx.on(move |cx, a| {
            if total > 0 && a.i == 0 {
                cx.ask("about", "ie.apply", json!([kk]));
            } else {
                cx.ask("about", "ie.close", json!([kk]));
            }
            cx.local.remove(&format!("about.ie.all.{kk}"));
            cx.local.remove(&format!("about.ie.skipped.{kk}"));
        });
        let mut b1 = Vec::new();
        if total > 0 {
            b1.push(btn("Apply changes").primary());
        }
        b1.push(btn(if total > 0 { "Cancel" } else { "Close" }));
        let mut b2 = Vec::new();
        let mut which: Vec<&'static str> = Vec::new();
        if lines.len() > 8 {
            b2.push(btn(if show_all { "Show fewer".to_string() } else { format!("Show all {}", lines.len()) }));
            which.push("all");
        }
        if !skipped.is_empty() {
            b2.push(btn(if show_skipped { "Hide skipped".to_string() } else { format!("See skipped ({})", skipped.len()) }));
            which.push("skipped");
        }
        let kk = k.clone();
        let more = cx.on(move |cx, a| {
            let Some(w) = which.get(a.i as usize) else { return };
            let key = format!("about.ie.{w}.{kk}");
            let v = cx.local_b(&key);
            cx.local_set(&key, !v);
        });
        let shown: Vec<Opt> = lines
            .iter()
            .take(if show_all { usize::MAX } else { 8 })
            .map(|l| {
                Opt::new(s(&l["text"])).icon(match l["kind"].as_str().unwrap_or("") {
                    "added" => "plus",
                    "off" => "power",
                    _ => "pencil-simple",
                })
            })
            .collect();
        let skips: Vec<Opt> = if show_skipped { skipped.iter().map(|l| Opt::new(s(&l["text"]))).collect() } else { Vec::new() };
        let g = p.group(s(&preview["title"]));
        g.footer(s(&preview["note"]));
        g.row(Row::new(s(&preview["summary"])).detail(if total > 0 { "Nothing changes until you apply." } else { "" }).buttons(b1, decide));
        g.row(Row::custom("about", "lines").data(shown));
        if lines.len() > 8 || !skipped.is_empty() {
            g.row(Row::new("").buttons(b2, more));
        }
        if !skips.is_empty() {
            g.row(Row::custom("about", "skips").data(skips));
        }
    }
    super::finish(p);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sections_named_as_the_shell_does() {
        assert_eq!(key(&["keybinds", "shortcuts", "gestures"]), "keybinds,shortcuts,gestures");
        assert_eq!(name("shortcuts"), "Your shortcuts");
        assert_eq!(detail("gestures"), "Trackpad gestures you changed or added");
    }
}

//! Import and export for Keyboard Shortcuts and Gestures
//! (shell/modules/settings/ImportExport.qml): a row per section, Everything,
//! end4; the path field when there's no file dialog; errors, the result
//! with Undo, and an import's preview before it's applied. The work (file
//! dialogs, reading, the preview, applying) is the shell's Keymap, run by
//! RustBridgeInput.qml (`ie` acts); its state comes back in the feed
//! (`ie.<scope>`: busy, pathMode, pathText, pathSeq, preview, previewSeq,
//! error, status, canUndo, dialog).

use super::super::super::model::{btn, Ctx, Opt, Page, Row};
use super::{feed, field_gen, set_field, G};
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

/// The import and export blocks for `scope` ("keys" or "gestures")
pub fn block(cx: &mut Ctx, p: &mut Page, scope: &str, sections: &[&str], title: &str) {
    let st = feed(cx)["ie"][scope].clone();
    let busy = st["busy"].as_bool().unwrap_or(false);
    let dialog = st["dialog"].as_bool().unwrap_or(false);
    let secs: Vec<String> = sections.iter().map(|s| s.to_string()).collect();

    let mut rows = Vec::new();
    for sec in sections {
        let (sc, one) = (scope.to_string(), vec![sec.to_string()]);
        let id = cx.on(move |cx, a| cx.ask(G, "ie", json!([sc, if a.i == 0 { "import" } else { "export" }, one])));
        rows.push(Row::new(name(sec)).detail(detail(sec)).buttons(vec![btn("Import…").off(busy), btn("Export…").off(busy)], id));
    }
    if sections.len() > 1 {
        let (sc, all) = (scope.to_string(), secs.clone());
        let id = cx.on(move |cx, a| cx.ask(G, "ie", json!([sc, if a.i == 0 { "import" } else { "export" }, all])));
        rows.push(Row::new("Everything").detail("All of the above in one file").buttons(vec![btn("Import…").off(busy), btn("Export all").off(busy)], id));
    }
    let sc = scope.to_string();
    let end4 = cx.on(move |cx, _| cx.ask(G, "ie", json!([sc, "end4"])));
    rows.push(Row::new("end4 (illogical-impulse)")
        .detail(if sections.contains(&"keybinds") {
            "Reads ~/.config/hypr/hyprland/keybinds.lua and custom/keybinds.lua, and the gestures in general.lua"
        } else {
            "Reads the gestures in ~/.config/hypr/hyprland/general.lua and custom/general.lua"
        })
        .button("Import from end4", end4)
        .busy(busy)
        .enabled(!busy));
    let g = p.group(title);
    g.footer(if dialog { "" } else { "Install zenity or kdialog for a file dialog; until then, type the file's path." });
    for r in rows {
        g.row(r);
    }

    // ── No file dialog: a path ──
    let mode = st["pathMode"].as_str().unwrap_or("").to_string();
    if !mode.is_empty() {
        // (the shell suggests a path each time it asks: it lands in the field)
        let key = format!("ie.{scope}.path");
        let seq = st["pathSeq"].as_i64().unwrap_or(0);
        if cx.local.get(&format!("{key}.seq")).and_then(Value::as_i64) != Some(seq) {
            cx.local_set(&format!("{key}.seq"), seq);
            set_field(cx, &key, st["pathText"].as_str().unwrap_or(""));
        }
        let text = cx.local_s(&key);
        let (k2, sc) = (key.clone(), scope.to_string());
        // (the field: 0 typing, 1 Enter; its buttons: 10 Export or Import, 11 Cancel)
        let id = cx.on(move |cx, a| match a.i {
            0 => cx.local_set(&k2, a.s.clone()),
            1 | 10 => {
                let t = if a.i == 1 { a.s.clone() } else { cx.local_s(&k2) };
                if !t.trim().is_empty() {
                    cx.ask(G, "ie", json!([sc, "go", t.trim()]));
                }
            }
            11 => cx.ask(G, "ie", json!([sc, "cancelPath"])),
            _ => {}
        });
        let gen = field_gen(cx, &key);
        p.group("").row(Row::new(if mode == "export" { "Save as" } else { "Open file" })
            .ctl(G, "fieldbtns", id)
            .text_value(text.clone())
            .placeholder("~/Documents/tessera-shortcuts.json")
            .num(gen)
            .width(260.0)
            .data(vec![btn(if mode == "export" { "Export" } else { "Import" }).primary().off(text.trim().is_empty() || busy), btn("Cancel")]));
    }

    // ── Errors and results ──
    let error = st["error"].as_str().unwrap_or("").to_string();
    if !error.is_empty() {
        let sc = scope.to_string();
        let x = cx.on(move |cx, _| cx.ask(G, "ie", json!([sc, "dismiss"])));
        p.group("").row(Row::new("Couldn't import or export").detail(error).buttons(vec![btn("").icon("x")], x));
    }
    let status = st["status"].as_str().unwrap_or("").to_string();
    let preview = st["preview"].clone();
    if !status.is_empty() && preview.is_null() {
        let mut r = Row::new(status);
        if st["canUndo"].as_bool().unwrap_or(false) {
            let sc = scope.to_string();
            let u = cx.on(move |cx, _| cx.ask(G, "ie", json!([sc, "undo"])));
            r = r.buttons(vec![btn("Undo").icon("arrow-counter-clockwise")], u);
        }
        p.group("").row(r);
    }

    // ── The preview ──
    if preview.is_object() {
        // (a new preview: folded again)
        let sk = format!("ie.{scope}.seq");
        let seq = st["previewSeq"].as_i64().unwrap_or(0);
        if cx.local.get(&sk).and_then(Value::as_i64) != Some(seq) {
            cx.local_set(&sk, seq);
            cx.local_set(&format!("ie.{scope}.all"), false);
            cx.local_set(&format!("ie.{scope}.skipped"), false);
        }
        let show_all = cx.local_b(&format!("ie.{scope}.all"));
        let show_skipped = cx.local_b(&format!("ie.{scope}.skipped"));
        let total = preview["total"].as_i64().unwrap_or(0);
        let lines: Vec<Value> = preview["lines"].as_array().cloned().unwrap_or_default();
        let skipped: Vec<Value> = preview["skipped"].as_array().cloned().unwrap_or_default();
        let sc = scope.to_string();
        let act = cx.on(move |cx, a| {
            if a.i == 0 && total > 0 {
                cx.ask(G, "ie", json!([sc, "apply"]));
            } else {
                cx.ask(G, "ie", json!([sc, "cancelPreview"]));
            }
        });
        // (Flow: Apply, then Cancel; Close alone when nothing would change)
        let b = if total > 0 { vec![btn("Apply changes").primary(), btn("Cancel")] } else { vec![btn("Close")] };
        let shown_lines: Vec<&Value> = if show_all { lines.iter().collect() } else { lines.iter().take(8).collect() };
        let mut rows = vec![Row::new(preview["summary"].as_str().unwrap_or("")).detail(if total > 0 { "Nothing changes until you apply." } else { "" }).buttons(b, act)];
        for l in shown_lines {
            let icon = match l["kind"].as_str().unwrap_or("") {
                "added" => "plus",
                "off" => "power",
                _ => "pencil-simple",
            };
            rows.push(Row::custom(G, "line").label_text(l["text"].as_str().unwrap_or("")).data(vec![Opt::new("").icon(icon)]));
        }
        if lines.len() > 8 || !skipped.is_empty() {
            let mut b = Vec::new();
            let mut ops = Vec::new();
            if lines.len() > 8 {
                b.push(btn(if show_all { "Show fewer".to_string() } else { format!("Show all {}", lines.len()) }));
                ops.push("all");
            }
            if !skipped.is_empty() {
                b.push(btn(if show_skipped { "Hide skipped".to_string() } else { format!("See skipped ({})", skipped.len()) }));
                ops.push("skipped");
            }
            let sc = scope.to_string();
            let id = cx.on(move |cx, a| {
                if let Some(op) = ops.get(a.i.max(0) as usize) {
                    let k = format!("ie.{sc}.{op}");
                    let v = cx.local_b(&k);
                    cx.local_set(&k, !v);
                }
            });
            rows.push(Row::custom(G, "buttons").data(b).id(id));
        }
        if show_skipped {
            for s in &skipped {
                rows.push(Row::custom(G, "skip").label_text(s["text"].as_str().unwrap_or("")));
            }
        }
        let g = p.group(preview["title"].as_str().unwrap_or(""));
        g.footer(preview["note"].as_str().unwrap_or(""));
        for r in rows {
            g.row(r);
        }
    }
}

/// (a custom row's text: its label)
trait LabelText {
    fn label_text(self, t: &str) -> Self;
}
impl LabelText for Row {
    fn label_text(mut self, t: &str) -> Self {
        self.label = t.to_string();
        self
    }
}

//! The "input" page group: Mouse & Trackpad (mouse.rs, with the Trackpad
//! part), the trackpad's Gestures (gestures.rs), Keyboard (shortcuts.rs:
//! the shortcuts, their form and its key recorder, with the Dictation part,
//! dictation.rs) and the shortcuts' import and export (importexport.rs).
//! Keymap.qml's logic Settings needs is keymap.rs. What only the QML shell
//! knows comes from shell/modules/settings/RustBridgeInput.qml
//! (`cx.feed["input"]`: mice, keymap, dictation, tools, ocr, ie…); what only
//! it does goes there (`cx.ask("input", …)`). Its own views:
//! ui/settings-input.slint.

pub mod dictation;
pub mod gestures;
pub mod importexport;
pub mod keymap;
pub mod mouse;
pub mod shortcuts;

use super::super::model::{Block, Ctx, Kind, Page};
use serde_json::Value;

/// Build `page` if it's one of this group's; false when it isn't
pub fn build(page: &str, cx: &mut Ctx, p: &mut Page) -> bool {
    match page {
        "mouse" => mouse::mouse(cx, p),
        "mouse/gestures" => gestures::gestures(cx, p),
        "shortcuts" => shortcuts::shortcuts(cx, p),
        _ => return false,
    }
    true
}

pub const G: &str = "input";

/// The shell's side for this group (RustBridgeInput.qml's feed)
pub fn feed<'a>(cx: &'a Ctx) -> &'a Value {
    &cx.feed[G]
}

/// Where a value is in a list (0 when it isn't: Math.max(0, indexOf))
pub fn idx(values: &[&str], v: &str) -> i32 {
    values.iter().position(|x| *x == v).unwrap_or(0) as i32
}

/// A custom row's place in its group, for Material's tile corners: 0
/// between, 1 first, 2 last, 3 alone (settings-input.slint reads `r.num`)
pub fn places(b: &mut Block) {
    let n = b.rows.len();
    for (i, r) in b.rows.iter_mut().enumerate() {
        if r.kind == Kind::Custom {
            r.num = match (i == 0, i + 1 == n) {
                (true, true) => 3.0,
                (true, false) => 1.0,
                (false, true) => 2.0,
                _ => 0.0,
            };
        }
    }
}

/// Every group of the page: its custom rows' places
pub fn place_all(p: &mut Page) {
    for b in p.blocks.iter_mut() {
        places(b);
    }
}

// ── Undo for 8 s (the pages' `offerUndo`): text in local state, and a
// timer that ends it ──

pub fn offer_undo(cx: &mut Ctx, scope: &str, text: String) {
    let nk = format!("{scope}.undoN");
    let n = cx.local.get(&nk).and_then(Value::as_i64).unwrap_or(0) + 1;
    cx.local_set(&nk, n);
    cx.local_set(&format!("{scope}.undo"), text);
    cx.spawn(&format!("{scope}.undoT{n}"), || {
        std::thread::sleep(std::time::Duration::from_secs(8));
        Value::Bool(true)
    });
}

/// The undo on offer, while its 8 s last
pub fn undo_text(cx: &Ctx, scope: &str) -> Option<String> {
    let n = cx.local.get(&format!("{scope}.undoN")).and_then(Value::as_i64).unwrap_or(0);
    let t = cx.local_s(&format!("{scope}.undo"));
    (!t.is_empty() && !cx.local_b(&format!("{scope}.undoT{n}"))).then_some(t)
}

pub fn clear_undo(cx: &mut Ctx, scope: &str) {
    cx.local_set(&format!("{scope}.undo"), "");
}

/// A field whose text the page sets (an app's name, a cleared search):
/// its text, and a count that says "take this text now"
pub fn set_field(cx: &mut Ctx, key: &str, text: &str) {
    cx.local_set(key, text);
    let g = cx.local.get(&format!("{key}.gen")).and_then(Value::as_i64).unwrap_or(0) + 1;
    cx.local_set(&format!("{key}.gen"), g);
}
pub fn field_gen(cx: &Ctx, key: &str) -> f32 {
    cx.local.get(&format!("{key}.gen")).and_then(Value::as_i64).unwrap_or(0) as f32
}

/// A string list in the feed
pub fn strings(v: &Value) -> Vec<String> {
    v.as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::super::super::model::Row;
    use super::*;

    #[test]
    fn custom_rows_know_their_place() {
        let mut b = Block::default();
        b.row(Row::custom(G, "x")).row(Row::new("y")).row(Row::custom(G, "z"));
        places(&mut b);
        assert_eq!((b.rows[0].num, b.rows[2].num), (1.0, 2.0));
        let mut b = Block::default();
        b.row(Row::custom(G, "x"));
        places(&mut b);
        assert_eq!(b.rows[0].num, 3.0);
        assert_eq!(idx(&["a", "b"], "c"), 0);
    }
}

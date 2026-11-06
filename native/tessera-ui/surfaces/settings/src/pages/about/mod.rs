//! The "about" page group: Software Update (update.qml), About (about.qml)
//! and its subpages System health (about-health.qml), Report a Bug
//! (about-bugreport.qml) and Login Items (about-loginitems.qml); and two
//! parts other groups' pages show: a cloud drive's folder (DriveGroup.qml:
//! `drive::group`, for Google and Microsoft 365) and shortcut and gesture
//! import and export (ImportExport.qml: `importexport::group`, for Keyboard
//! and Gestures). Its own views: ui/settings-about.slint. What only the QML
//! shell knows and does goes through shell/modules/settings/RustBridgeAbout.qml
//! (`cx.feed["about"]`, `cx.ask("about", …)`): the updater, the root helper,
//! pacman, the health checks, the bug report service, rclone, Keymap.

pub mod about;
pub mod bugreport;
pub mod drive;
pub mod health;
pub mod importexport;
pub mod loginitems;
pub mod sysinfo;
pub mod update;

use super::super::model::{Block, Ctx, Kind, Page};
use serde_json::Value;

/// Build `page` if it's one of this group's; false when it isn't
pub fn build(page: &str, cx: &mut Ctx, p: &mut Page) -> bool {
    match page {
        "update" => update::page(cx, p),
        "about" => about::page(cx, p),
        "about/health" => health::page(cx, p),
        "about/bugreport" => bugreport::page(cx, p),
        "about/loginitems" => loginitems::page(cx, p),
        _ => return false,
    }
    finish(p);
    true
}

/// What the shell says for this group (RustBridgeAbout.qml's feed)
pub fn feed<'a>(cx: &Ctx<'a>) -> &'a Value {
    let f: &'a Value = cx.feed;
    &f["about"]
}

/// A string from the feed ("" when missing)
pub fn s(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        v => v.to_string(),
    }
}

pub fn b(v: &Value) -> bool {
    v.as_bool().unwrap_or(false)
}

pub fn n(v: &Value) -> i64 {
    v.as_f64().unwrap_or(0.0) as i64
}

pub fn strings(v: &Value) -> Vec<String> {
    v.as_array().map(|a| a.iter().map(s).collect()).unwrap_or_default()
}

/// What a toast's Undo does (an action made outside `cx.on`)
pub fn action(f: impl Fn(&mut Ctx, &super::super::model::Arg) + 'static) -> super::super::model::Action {
    std::rc::Rc::new(f)
}

/// "3 packages", "1 package"
pub fn plural(n: i64, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}

/// Material: a custom row draws its own tile, its corners round at the
/// group's ends (Group.qml's tiles): `from` 1 first, `to` 1 last
pub fn tile(b: &mut Block) {
    let n = b.rows.len();
    for (i, r) in b.rows.iter_mut().enumerate() {
        if r.kind == Kind::Custom && r.group == "about" {
            r.from = if i == 0 { 1.0 } else { 0.0 };
            r.to = if i + 1 == n { 1.0 } else { 0.0 };
        }
    }
}

/// Every group's custom rows told where they sit (call once a page is built)
pub fn finish(p: &mut Page) {
    for b in p.blocks.iter_mut() {
        tile(b);
    }
}

thread_local! {
    static ICONS: std::cell::RefCell<std::collections::HashMap<String, Option<slint::Image>>> = Default::default();
}

fn rgb(hex: &str) -> [u8; 3] {
    let h = hex.trim_start_matches('#');
    let n = u32::from_str_radix(h.get(..6).unwrap_or("808080"), 16).unwrap_or(0x808080);
    [(n >> 16) as u8, (n >> 8) as u8, n as u8]
}

/// An app's icon as AppIcon.qml draws it in this look (the icon style and
/// tile), `px` pixels; read once (the icon theme: the "icons" service)
pub fn app_icon(cx: &mut Ctx, names: &[&str], px: u32) -> Option<slint::Image> {
    cx.want("icons");
    let shell = cx.shell;
    let icons = shell.get::<crate::services::icons::Icons>()?;
    let path = icons.path(names, &[])?;
    let key = format!("{}@{px}:{}:{}", path.display(), cx.material, cx.dark);
    if let Some(i) = ICONS.with(|m| m.borrow().get(&key).cloned()) {
        return i;
    }
    let accent = rgb(&shell.look.accent);
    let themed = if cx.material {
        let roles = crate::theme::material_roles(&shell.look.seed, cx.dark, &shell.look.style);
        let r = |k: &str| rgb(roles.iter().find(|(n, _)| n == k).map_or("#6750A4", |(_, v)| v.as_str()));
        (r("primaryContainer"), r("onPrimaryContainer"))
    } else {
        ([0, 0, 0], [0, 0, 0])
    };
    let style = crate::appicon::Style::from_settings(&shell.cfg, cx.material, cx.dark, accent, themed);
    let img = crate::appicon::render(&path, px, &style, None).map(slint::Image::from_rgba8);
    ICONS.with(|m| m.borrow_mut().insert(key, img.clone()));
    img
}

/// The home folder written as ~
pub fn tilde(path: &str) -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    if !home.is_empty() && path.starts_with(&home) {
        format!("~{}", &path[home.len()..])
    } else {
        path.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Row;

    #[test]
    fn plurals_and_tiles() {
        assert_eq!(plural(1, "package"), "1 package");
        assert_eq!(plural(3, "update"), "3 updates");
        let mut b = Block::default();
        b.row(Row::new("a"));
        b.row(Row::custom("about", "notes"));
        tile(&mut b);
        assert_eq!((b.rows[1].from, b.rows[1].to), (0.0, 1.0));
        assert_eq!(rgb("#0A84FF"), [0x0a, 0x84, 0xff]);
    }
}

//! The "look" page group: Appearance (and its Glass subpage), Wallpaper,
//! Desktop & Widgets, Menu Bar & Dock (with the Dock and Quick Settings),
//! Lock & Login Screen. Its own views: ui/settings-look.slint. What only the
//! QML shell knows comes from shell/modules/settings/RustBridgeLook.qml
//! (`cx.feed["look"]`); what only it does goes there (`cx.ask("look", …)`).

pub mod appearance;
pub mod desktop;
pub mod lock;
pub mod menubar;
pub mod wallpaper;

use super::super::model::{color, Ctx, Opt, Page};
use serde_json::Value;

/// Build `page` if it's one of this group's; false when it isn't
pub fn build(page: &str, cx: &mut Ctx, p: &mut Page) -> bool {
    match page {
        "appearance" => appearance::appearance(cx, p),
        "appearance/glass" => appearance::glass(cx, p),
        "wallpaper" => wallpaper::wallpaper(cx, p),
        "desktop" => desktop::desktop(cx, p),
        "menubar" => menubar::menubar(cx, p),
        "lock" => lock::lock(cx, p),
        _ => return false,
    }
    true
}

/// The shell's side for this group (RustBridgeLook.qml's feed)
pub fn feed<'a>(cx: &'a Ctx) -> &'a Value {
    &cx.feed["look"]
}

/// Where a value is in a list (0 when it isn't: Math.max(0, indexOf))
pub fn idx(values: &[&str], v: &str) -> i32 {
    values.iter().position(|x| *x == v).unwrap_or(0) as i32
}

/// The time as the previews show it (TimeFormat.hmShort: "9:41" or "09:41")
pub fn time_short(h24: bool) -> String {
    crate::util::now(if h24 { "%H:%M" } else { "%-I:%M" })
}

/// The picture showing now (Wallpaper.path: the shell says; else the setting)
pub fn wallpaper_path(cx: &Ctx) -> String {
    feed(cx)["path"].as_str().filter(|s| !s.is_empty()).map(str::to_string).unwrap_or_else(|| cx.s("wallpaper.path"))
}

/// The glass material's amounts [lens, frost, solid, tint] (Theme.glass.amounts)
pub fn amounts(cx: &Ctx) -> [f32; 4] {
    match super::glass_material(cx).as_str() {
        "frosted" => [0.3, 1.0, 0.45, 0.5],
        "solid" => [0.0, 0.0, 1.0, 0.0],
        "veil" => [0.0, 1.0, 1.0 - 0.55 * cx.f("appearance.materialTransparency"), 0.5],
        "custom" => [cx.f("appearance.glassLens"), cx.f("appearance.glassFrost"), cx.f("appearance.glassSolid"), cx.f("appearance.glassTint")],
        _ => [1.0, 0.75, 0.0, 0.75],
    }
}

/// Material roles the previews draw with, in this order (settings-look.slint
/// reads them by index)
pub const ROLES: [&str; 11] = [
    "primaryContainer",
    "onPrimaryContainer",
    "surfaceContainer",
    "primary",
    "secondaryContainer",
    "surfaceContainerHighest",
    "surfaceContainerHigh",
    "tertiaryContainer",
    "tertiary",
    "onSurfaceVariant",
    "onSurface",
];

fn is_hex(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// The Material palette a look preview shows (LookPreview.qml's `m`): the
/// Material look's own, else from the chosen seed or the wallpaper's tone;
/// the first two carry the cookie's and the sunny shape's outlines
pub fn palette(cx: &Ctx) -> Vec<Opt> {
    let chosen = cx.s("appearance.materialSeed");
    let tone = feed(cx)["tone"].as_str().unwrap_or("").chars().take(7).collect::<String>();
    let seed = if cx.material {
        cx.shell.look.seed.clone()
    } else if is_hex(&chosen) {
        chosen
    } else if is_hex(&tone) {
        tone
    } else {
        "#6750A4".to_string()
    };
    let roles = crate::theme::material_roles(&seed, cx.dark, &cx.s("appearance.materialStyle"));
    let mut out: Vec<Opt> = ROLES
        .iter()
        .map(|r| {
            let hex = roles.iter().find(|(n, _)| n == r).map_or("#6750A4", |(_, v)| v.as_str());
            Opt::new(*r).color(color(hex))
        })
        .collect();
    out[0].detail = crate::surfaces::shapes::outline("cookie9");
    out[1].detail = crate::surfaces::shapes::outline("sunny");
    out
}

thread_local! {
    static ICONS: std::cell::RefCell<std::collections::HashMap<String, Option<slint::Image>>> = Default::default();
}

/// An icon theme's icon (a distribution's logo) as a picture, read once
pub fn theme_icon(names: &[&str]) -> Option<slint::Image> {
    let key = names.join("|");
    ICONS.with(|m| {
        m.borrow_mut()
            .entry(key)
            .or_insert_with(|| names.iter().filter(|n| !n.is_empty()).find_map(|n| crate::icon_theme::find(n, None)).and_then(|p| slint::Image::load_from_path(&p).ok()))
            .clone()
    })
}

/// A Material custom row's place in its group: 0 between, 1 first, 2 last,
/// 3 alone (its tile's corners)
pub fn place(first: bool, last: bool) -> f32 {
    match (first, last) {
        (true, true) => 3.0,
        (true, false) => 1.0,
        (false, true) => 2.0,
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexes_and_places() {
        assert_eq!(idx(&["a", "b"], "b"), 1);
        assert_eq!(idx(&["a", "b"], "z"), 0);
        assert_eq!(place(true, false), 1.0);
        assert_eq!(place(true, true), 3.0);
        assert!(is_hex("#12abEF") && !is_hex("wallpaper"));
    }
}

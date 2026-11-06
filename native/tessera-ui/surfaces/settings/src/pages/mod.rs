//! The pages, one module per page group (each a folder of its own: look,
//! notify, devices, input, accounts, system, about), as
//! shell/modules/settings/pages/*.qml. A page is `fn(cx, p)`: it reads the
//! settings and data through `cx` and adds blocks and rows to `p`
//! (model.rs). Shared here: how keys are written (keys.rs) and the glass
//! material Settings shows.

pub mod about;
pub mod accounts;
pub mod devices;
pub mod input;
pub mod keys;
pub mod look;
pub mod notify;
pub mod system;

use super::model::{Ctx, Page, Row};

/// The page on show (`cx.page`), built into `p`
pub fn build(cx: &mut Ctx, p: &mut Page) {
    let page = cx.page.to_string();
    let found = look::build(&page, cx, p)
        || notify::build(&page, cx, p)
        || devices::build(&page, cx, p)
        || input::build(&page, cx, p)
        || accounts::build(&page, cx, p)
        || system::build(&page, cx, p)
        || about::build(&page, cx, p);
    if !found {
        p.group("").row(Row::text(format!("There's no page called “{page}”.")));
    }
}

/// The glass material in use (Theme.glass.material): the chosen one, solid
/// with Reduce transparency, Cool-down's cheaper one while it's on (the
/// shell says: feed.look.cooldownGlass)
pub fn glass_material(cx: &Ctx) -> String {
    let chosen = if cx.b("appearance.reduceTransparency") {
        "solid".to_string()
    } else if cx.material {
        "veil".to_string()
    } else {
        let g = cx.s("appearance.glass");
        if g == "auto" {
            "frosted".into()
        } else {
            g
        }
    };
    let cool = cx.feed["look"]["cooldownGlass"].as_str().unwrap_or("");
    if cool == "solid" {
        "solid".into()
    } else if cool == "frosted" && chosen != "solid" && chosen != "veil" {
        "frosted".into()
    } else {
        chosen
    }
}

/// The nearest named material (Theme.glass.tier): solid | liquid | frosted
pub fn glass_tier(cx: &Ctx) -> String {
    let m = glass_material(cx);
    let [lens, frost, solid] = match m.as_str() {
        "liquid" => [1.0, 0.75, 0.0],
        "frosted" => [0.3, 1.0, 0.45],
        "solid" => [0.0, 0.0, 1.0],
        "veil" => [0.0, 1.0, 1.0 - 0.55 * cx.f("appearance.materialTransparency")],
        "custom" => [cx.f("appearance.glassLens"), cx.f("appearance.glassFrost"), cx.f("appearance.glassSolid")],
        _ => [1.0, 0.75, 0.0],
    };
    if solid >= 0.97 {
        "solid".into()
    } else if solid < 0.4 && (frost < 0.4 || lens >= 0.6) {
        "liquid".into()
    } else {
        "frosted".into()
    }
}

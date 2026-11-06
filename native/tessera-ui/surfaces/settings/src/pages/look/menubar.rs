//! Menu Bar & Dock (pages/menubar.qml, with dock.qml and controlcenter.qml
//! as its parts): the worked example of a page.

use super::super::super::model::{btn, Ctx, Opt, Page, Row};
use super::super::keys;
use serde_json::Value;

fn toggle_in(list: &[String], id: &str) -> Vec<String> {
    if list.iter().any(|x| x == id) {
        list.iter().filter(|x| *x != id).cloned().collect()
    } else {
        list.iter().cloned().chain(std::iter::once(id.to_string())).collect()
    }
}

fn index_of(values: &[&str], v: &str) -> i32 {
    values.iter().position(|x| *x == v).unwrap_or(0) as i32
}

/// The menu bar's switch rows (menubar.qml's Toggle)
fn toggle(cx: &mut Ctx, key: &str, text: &str, hint: &str) -> Row {
    let path = format!("menubar.{key}");
    let on = cx.b(&path);
    Row::new(text).detail(hint).switch(on, cx.toggle(&path))
}

pub fn menubar(cx: &mut Ctx, p: &mut Page) {
    let combo = |cx: &Ctx, k: &str| keys::combo(&*cx.cfg, k);

    let rows = vec![
        toggle(cx, "autoHide", "Automatically hide the menu bar", "Move the pointer to the top edge to show it."),
        toggle(cx, "background", "Show menu bar background", ""),
        toggle(cx, "scrollGestures", "Scroll to change brightness and volume", "Left half: brightness. Right half: volume."),
    ];
    let g = p.group("Behaviour");
    for r in rows {
        g.row(r);
    }

    let title = toggle(cx, "windowTitle", "Show window title", "");
    let spaces = ["off", "dots", "icons"];
    let ws = cx.s("menubar.workspaces");
    let pick = cx.pick("menubar.workspaces", &spaces);
    let g = p.group("Left and centre");
    g.row(title);
    g.row(Row::new("Spaces").segmented(&["Off", "Dots", "App icons"], index_of(&spaces, &ws), pick).width(240.0));

    // One choice: off, or the battery with its percentage where you want it
    // ("true"/"false" from before mean beside/none)
    let battery = cx.b("menubar.battery");
    let percent = match cx.s("menubar.batteryPercent").as_str() {
        "true" => "outside".to_string(),
        "false" => "off".to_string(),
        s => s.to_string(),
    };
    let values = ["off", "icon", "inside", "outside"];
    let current = if !battery { 0 } else { values.iter().position(|v| *v == percent).unwrap_or(0).max(1) as i32 };
    let pick = cx.on(move |cx, a| {
        if a.i > 0 {
            cx.set("menubar.batteryPercent", if a.i == 1 { "off" } else { values[a.i as usize] });
        }
        cx.set("menubar.battery", a.i > 0);
    });
    let mut rows = vec![Row::new("Battery")
        .detail(if battery { "The percentage inside is cut into the battery; with the icon alone, it shows beside it at 20% or less." } else { "" })
        .segmented(&["Off", "Icon", "% inside", "% beside"], current, pick)
        .width(300.0)];
    rows.push(toggle(cx, "wifi", "Wi-Fi", ""));
    rows.push(toggle(cx, "bluetooth", "Bluetooth", ""));
    rows.push(toggle(cx, "sound", "Sound", ""));
    rows.push(toggle(cx, "resources", "CPU and memory", ""));
    rows.push(toggle(cx, "weather", "Weather", ""));
    rows.push(toggle(cx, "keyboardLayout", "Keyboard layout", "Shown when you have more than one layout."));
    rows.push(toggle(cx, "collapseTray", "Tuck away app icons", "The icons apps put in the menu bar wait behind an arrow; click it to show them."));
    let h = format!("Search still opens with {}.", combo(&*cx, "SUPER Space"));
    rows.push(toggle(cx, "search", "Search", &h));
    let h = format!("The sidebar still opens with {}.", combo(&*cx, "SUPER A"));
    rows.push(toggle(cx, "ai", "AI", &h));
    let h = format!("Still opens with {}.", combo(&*cx, "SUPER C"));
    rows.push(toggle(cx, "controlCenter", "Quick Settings", &h));
    let h = format!("Notification Center still opens with {}.", combo(&*cx, "SUPER N"));
    rows.push(toggle(cx, "clock", "Clock and notifications", &h));
    rows.push(toggle(cx, "appMenu", "App name menu", "On the left, next to the system menu."));
    let h = format!("On the left; window actions are also in {} and friends.", combo(&*cx, "SUPER CTRL F"));
    rows.push(toggle(cx, "windowMenu", "Window menu", &h));
    let g = p.group("Status items");
    for r in rows {
        g.row(r);
    }

    let rows = vec![toggle(cx, "showDate", "Show date", ""), toggle(cx, "showSeconds", "Show seconds", "")];
    let g = p.group("Clock");
    for r in rows {
        g.row(r);
    }

    // ── More options: the fine-tuning, folded away ──
    p.more(cx);
    {
        // The screens it's on (every screen: an empty list)
        let all: Vec<String> = cx.screens.clone();
        let chosen = cx.list("menubar.screens");
        let chips: Vec<Opt> = all.iter().map(|n| Opt::new(n.clone()).on(chosen.is_empty() || chosen.contains(n))).collect();
        let names = all.clone();
        let pick = cx.on(move |cx, a| {
            let Some(name) = names.get(a.i as usize) else { return };
            let cur = cx.list("menubar.screens");
            let cur = if cur.is_empty() { names.clone() } else { cur };
            let next = toggle_in(&cur, name);
            let v: Vec<Value> = if next.len() == names.len() { Vec::new() } else { next.into_iter().map(Value::from).collect() };
            cx.set("menubar.screens", Value::Array(v));
        });
        p.group("Behaviour").row(Row::new("Screens").detail("Show the menu bar on these screens.").chips(chips, pick).width(260.0));

        let spaces_on = cx.s("menubar.workspaces") != "off";
        let count = cx.i("menubar.workspaceCount");
        let n = cx.num("menubar.workspaceCount", 1.0);
        let overflow = cx.s("menubar.workspaceOverflow");
        let ov = cx.pick("menubar.workspaceOverflow", &["arrows", "dynamic"]);
        let other = cx.b("menubar.otherSpaces");
        let ot = cx.toggle("menubar.otherSpaces");
        let g = p.group("Left and centre");
        g.row(Row::new("Spaces always shown")
            .visible(spaces_on)
            .detail(format!("{count} {}; more appear when used", if count == 1 { "space" } else { "spaces" }))
            .slider(count as f32, 1.0, 10.0, 1.0, n)
            .width(200.0));
        g.row(Row::new("When there are more spaces")
            .visible(spaces_on)
            .detail(if overflow == "dynamic" { "The next spaces shrink and fade at the ends, and slide along as you move" } else { "Arrows at the ends; a dot under one means a space past it has windows" })
            .segmented(&["Arrows", "Dynamic dots"], index_of(&["arrows", "dynamic"], &overflow), ov)
            .width(220.0));
        g.row(Row::new("Spaces on other displays")
            .visible(spaces_on)
            .detail("Listed after this display's own, under their display's name; right-click a space to move it to another display")
            .switch(other, ot));

        let values = ["bellDot", "dot", "count", "bellCount", "off"];
        let ind = cx.s("menubar.notificationIndicator");
        let pick = cx.pick("menubar.notificationIndicator", &values);
        p.group("Status items").row(Row::new("Unread notifications")
            .detail("Shown next to the clock; hidden while a Focus mode is on.")
            .segmented(&["Bell + dot", "Dot", "Count", "Bell + count", "Off"], index_of(&values, &ind), pick)
            .width(320.0));

        let buttons = [
            ("screenshot", "Screenshot tools"),
            ("colorPicker", "Colour picker"),
            ("mic", "Microphone mute"),
            ("keyboard", "On-screen keyboard"),
            ("darkMode", "Dark mode"),
            ("powerProfile", "Energy mode"),
            ("record", "Screen recording"),
        ];
        let util = cx.list("menubar.utilButtons");
        let mut rows = Vec::new();
        for (id, label) in buttons {
            let on = util.iter().any(|x| x == id);
            let t = cx.on(move |cx, _| {
                let next = toggle_in(&cx.list("menubar.utilButtons"), id);
                cx.set("menubar.utilButtons", Value::Array(next.into_iter().map(Value::from).collect()));
            });
            rows.push(Row::new(label).switch(on, t));
        }
        let g = p.group("Buttons");
        for r in rows {
            g.row(r);
        }
    }
    p.end_more();

    // ── Parts: other pages shown here ──
    p.part(cx, "Dock", dock);
    p.part(cx, "Quick Settings", controlcenter);
}

/// The dock (dock.qml)
pub fn dock(cx: &mut Ctx, p: &mut Page) {
    let on = cx.b("dock.enabled");
    let t = cx.toggle("dock.enabled");
    p.group("")
        .footer(if on { "" } else { "The App Drawer stays on Super+Ctrl+Space, and windows use the space at the bottom." })
        .row(Row::new("Show the dock").switch(on, t));

    // (Material's dock keeps its icons between 36 and 56)
    let size = cx.f("dock.iconSize");
    let (lo, hi) = if cx.material { (36.0, 56.0) } else { (32.0, 72.0) };
    let shown = if cx.material { size.clamp(36.0, 56.0) } else { size };
    let n = cx.num("dock.iconSize", 2.0);
    let apps = cx.b("dock.appsButton");
    let at = cx.toggle("dock.appsButton");
    let hide_values = ["off", "windows", "always"];
    let hide = cx.s("dock.autoHide");
    let hp = cx.pick("dock.autoHide", &hide_values);
    p.group("")
        .dim(!on)
        .row(Row::new("Size").detail(format!("{} px", shown.round())).slider(size, lo, hi, 2.0, n).width(220.0))
        .row(Row::new("Apps button").detail("Opens the App Drawer (also Super+Ctrl+Space)").switch(apps, at))
        .row(Row::new("Automatically hide")
            .detail(match hide.as_str() {
                "windows" => "Hides when a window overlaps it; point at the bottom edge to show it.",
                "always" => "Point at the bottom edge of the screen to show it.",
                _ => "",
            })
            .segmented(&["Never", "Over windows", "Always"], index_of(&hide_values, &hide), hp)
            .width(260.0));

    // Kept in Dock: each with its remove button
    let pinned = cx.list("dock.pinned");
    let shell = cx.shell;
    let apps_list = shell.get::<crate::services::apps::Apps>();
    let name_of = |id: &str| -> Option<String> { apps_list.and_then(|a| a.by_id(id)).map(|e| e.name.clone()) };
    let mut rows = Vec::new();
    for id in &pinned {
        let name = name_of(id);
        let idc = id.clone();
        let rm = cx.on(move |cx, _| {
            let next: Vec<Value> = cx.list("dock.pinned").into_iter().filter(|p| *p != idc).map(Value::from).collect();
            cx.set("dock.pinned", Value::Array(next));
        });
        rows.push(Row::new(name.clone().unwrap_or_else(|| id.clone())).detail(if name.is_some() { "" } else { "Not installed" }).buttons(vec![btn("").icon("minus")], rm));
    }
    cx.want("apps");
    let g = p.group("Kept in Dock");
    g.dim(!on).footer("Running apps that aren't kept appear after the divider.");
    for r in rows {
        g.row(r);
    }

    // Add to Dock: search the apps
    let filter = cx.local_s("dock.filter");
    let f = cx.on(|cx, a| cx.local_set("dock.filter", a.s.clone()));
    let mut rows = vec![Row::new("Search apps").field(filter.clone(), "App name", f).width(240.0)];
    let q = filter.trim().to_lowercase();
    if q.chars().count() >= 2 {
        if let Some(apps) = shell.get::<crate::services::apps::Apps>() {
            let found: Vec<(String, String)> = apps.entries.iter().filter(|a| !a.no_display && !pinned.contains(&a.id) && a.name.to_lowercase().contains(&q)).take(6).map(|a| (a.id.clone(), a.name.clone())).collect();
            for (id, name) in found {
                let add = cx.on(move |cx, _| {
                    let mut next: Vec<Value> = cx.list("dock.pinned").into_iter().map(Value::from).collect();
                    next.push(Value::from(id.clone()));
                    cx.set("dock.pinned", Value::Array(next));
                });
                rows.push(Row::new(name).button("Add", add).primary());
            }
        }
    }
    let g = p.group("Add to Dock");
    g.dim(!on);
    for r in rows {
        g.row(r);
    }

    // ── More options ──
    p.more(cx);
    {
        let gap = cx.f("dock.gap");
        let gn = cx.num("dock.gap", 0.01);
        let names = cx.b("dock.appNames");
        let nt = cx.toggle("dock.appNames");
        let recent = cx.b("dock.recentApps");
        let rt = cx.toggle("dock.recentApps");
        let dl = cx.b("dock.downloads");
        let dt = cx.toggle("dock.downloads");
        let tr = cx.b("dock.trash");
        let tt = cx.toggle("dock.trash");
        let mag = cx.b("dock.magnification");
        let scale = cx.f("dock.magnifiedScale");
        // One slider: its left end is off (the size you had is kept for
        // when it's on again)
        let mt = cx.on(|cx, a| {
            if a.f < 1.025 {
                cx.set("dock.magnification", false);
                return;
            }
            cx.set_num("dock.magnifiedScale", ((a.f as f64 * 100.0).round() / 100.0).max(1.1));
            cx.set("dock.magnification", true);
        });
        p.group("")
            .row(Row::new("Space between icons").detail(format!("{} px", (size * gap).round())).slider(gap, 0.05, 0.4, 0.0, gn).width(220.0))
            .row(Row::new("App names in the App Drawer").visible(cx.material).detail("Off shows only the icons").switch(names, nt))
            .row(Row::new("Recent apps").detail("Up to three apps you used lately, after a divider").switch(recent, rt))
            .row(Row::new("Downloads").switch(dl, dt))
            .row(Row::new("Trash").switch(tr, tt))
            .row(Row::new("Magnification")
                .visible(!cx.material)
                .detail(if mag { format!("{}% when you point at an icon", (scale * 100.0).round()) } else { "Off".into() })
                .slider(if mag { scale } else { 1.0 }, 1.0, 2.0, 0.05, mt)
                .words("Off", "Large")
                .width(220.0));
    }
    p.end_more();
}

/// Quick Settings (controlcenter.qml)
pub fn controlcenter(cx: &mut Ctx, p: &mut Page) {
    let n = cx.get("controlCenter.layout").as_array().map_or(0, Vec::len);
    let solid = super::super::glass_tier(cx) == "solid";
    let bg = cx.b("controlCenter.background");
    let edit = cx.on(|cx, _| cx.ask("look", "editControls", Value::Null));
    let bt = cx.toggle("controlCenter.background");
    let reset = cx.on(|cx, _| cx.set("controlCenter.layout", Value::Array(Vec::new())));
    p.group("Layout")
        .footer("Open Quick Settings and choose Edit Controls to add, remove, resize and move controls. Controls that need a missing tool stay hidden until it's installed. Game Mode turns off animations, blur, shadows and gaps; Anti-Flash softens very bright screens.")
        .row(Row::new("Edit Controls").detail(if n > 0 { format!("{n} controls, arranged by you") } else { "The default arrangement".into() }).button("Edit…", edit).primary())
        .row(Row::new("Panel background")
            .visible(!cx.material)
            .detail(if solid { "Always on with Solid glass" } else if bg { "Controls sit on one glass panel" } else { "Each control is its own glass chip" })
            .switch(bg || solid, bt)
            .enabled(!solid))
        .row(Row::new("Reset to the default layout").visible(n > 0).button("Reset", reset).minor());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_toggle_an_item() {
        let l = vec!["a".to_string(), "b".to_string()];
        assert_eq!(toggle_in(&l, "a"), vec!["b".to_string()]);
        assert_eq!(toggle_in(&l, "c"), vec!["a".to_string(), "b".to_string(), "c".to_string()]);
    }
}

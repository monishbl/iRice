//! Desktop & Widgets (pages/desktop.qml): Edit Widgets, the files app, the
//! weather's place and units, hot corners, and the drop shelf.

use super::super::super::model::{Ctx, Page, Row};
use super::feed;
use serde_json::Value;

/// Hot corners' actions: (value, label)
pub const CORNER_ACTIONS: [(&str, &str); 9] = [
    ("none", "None"),
    ("overview", "Overview"),
    ("missionControl", "All Windows"),
    ("desktop", "Show desktop"),
    ("notificationCenter", "Notification Center"),
    ("launchpad", "App Drawer"),
    ("lock", "Lock screen"),
    ("standby", "Start Bedside Mode"),
    ("quickNote", "Quick note"),
];

/// "3 widgets", "1 widget"
pub fn count(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

pub fn desktop(cx: &mut Ctx, p: &mut Page) {
    let widgets = cx.list("desktop.widgets").len();
    let edit = cx.on(|cx, _| cx.ask("look", "editWidgets", Value::Null));
    // Files app: Automatic (the first installed), or one of them (the shell says which are)
    let managers: Vec<(String, String)> = feed(cx)["files"]
        .as_array()
        .map(|a| a.iter().map(|m| (m["bin"].as_str().unwrap_or("").to_string(), m["name"].as_str().unwrap_or("").to_string())).collect())
        .unwrap_or_default();
    let mut opts = vec![format!("Automatic{}", managers.first().map(|m| format!(" ({})", m.1)).unwrap_or_default())];
    opts.extend(managers.iter().map(|m| m.1.clone()));
    let fm = cx.s("desktop.fileManager");
    let index = if fm.is_empty() { 0 } else { managers.iter().position(|m| m.0 == fm).map_or(-1, |i| i as i32 + 1) };
    let bins: Vec<String> = managers.iter().map(|m| m.0.clone()).collect();
    let fp = cx.on(move |cx, a| {
        let v = if a.i <= 0 { String::new() } else { bins.get(a.i as usize - 1).cloned().unwrap_or_default() };
        cx.set("desktop.fileManager", v);
    });
    p.group("Widgets")
        .footer("Drag a widget to move it; it snaps to the grid. Right-click a widget to change its size, set what it shows (Widget Options) or remove it, or right-click the desktop.")
        .row(Row::new("Edit Widgets").detail(format!("{} on the desktop", count(widgets, "widget", "widgets"))).button("Edit…", edit).primary())
        .row(Row::new("Files app")
            .detail(if managers.is_empty() { "No file manager installed (e.g. sudo pacman -S nautilus)" } else { "Opens folders, Downloads and the Trash" })
            .choice(&opts, index, fp)
            .width(220.0));

    // Weather
    let w = &feed(cx)["weather"];
    let footer = if w["ready"].as_bool().unwrap_or(false) {
        format!("Showing {}: {}°, {}.", w["place"].as_str().unwrap_or(""), w["temp"].as_i64().unwrap_or(0), w["condition"].as_str().unwrap_or("").to_lowercase())
    } else {
        w["error"].as_str().filter(|e| !e.is_empty()).unwrap_or("Loading…").to_string()
    };
    let loc = cx.on(|cx, a| {
        if a.i == 1 {
            let t = a.s.trim().to_string();
            if t != cx.s("weather.location") {
                cx.set("weather.location", t);
            }
        }
    });
    let units = cx.s("weather.units");
    let up = cx.on(|cx, a| cx.set("weather.units", if a.i == 1 { "imperial" } else { "metric" }));
    p.group("Weather")
        .footer(footer)
        .row(Row::new("Location").detail("Leave empty to detect it from your network.").field(cx.s("weather.location"), "City, e.g. Bengaluru", loc).width(220.0))
        .row(Row::new("Units").segmented(&["Celsius", "Fahrenheit"], if units == "imperial" { 1 } else { 0 }, up).width(200.0));

    // Hot corners
    let labels: Vec<String> = CORNER_ACTIONS.iter().map(|a| a.1.to_string()).collect();
    let mut rows = Vec::new();
    for (key, label) in [("topLeft", "Top left"), ("topRight", "Top right"), ("bottomLeft", "Bottom left"), ("bottomRight", "Bottom right")] {
        let path = format!("hotCorners.{key}");
        let cur = cx.s(&path);
        let i = CORNER_ACTIONS.iter().position(|a| a.0 == cur).map_or(-1, |i| i as i32);
        let values: Vec<&str> = CORNER_ACTIONS.iter().map(|a| a.0).collect();
        let pick = cx.pick(&path, &values);
        rows.push(Row::new(label).choice(&labels, i, pick).width(220.0));
    }
    let g = p.group("Hot corners");
    g.footer("Move the pointer into a corner and hold it there. Corners do nothing while a full-screen window is in front.");
    for r in rows {
        g.row(r);
    }

    // ── More options: the drop shelf ──
    p.more(cx);
    {
        let shake = cx.b("shelf.shake");
        let st = cx.toggle("shelf.shake");
        let edge = cx.b("shelf.edge");
        let et = cx.toggle("shelf.edge");
        let bar = cx.b("shelf.menubar");
        let bt = cx.toggle("shelf.menubar");
        let n = feed(cx)["shelfCount"].as_u64().unwrap_or(0) as usize;
        let opened = feed(cx)["shelfOpened"].as_bool().unwrap_or(false);
        let sh = cx.on(|cx, _| cx.ask("look", "shelf", Value::Null));
        p.group("Drop shelf")
            .footer("A place to park files while you get to where they're going: drop them on the shelf, then drag them out again, one at a time or all together. Super+Alt+D opens it too.")
            .row(Row::new("Shake while dragging").detail("Shake the pointer quickly from side to side with files held").switch(shake, st))
            .row(Row::new("Drag to the side of the screen").detail("Hold the files at the left or right edge for a moment").switch(edge, et))
            .row(Row::new("Menu bar icon").detail("While the shelf holds something, and while you drag: drop files on it").switch(bar, bt))
            .row(Row::new("Open the shelf")
                .detail(if n > 0 { format!("{} on it", count(n, "item", "items")) } else { "Empty".into() })
                .button(if opened { "Hide Shelf" } else { "Show Shelf" }, sh));
    }
    p.end_more();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_and_corners() {
        assert_eq!(count(1, "widget", "widgets"), "1 widget");
        assert_eq!(count(4, "widget", "widgets"), "4 widgets");
        assert_eq!(CORNER_ACTIONS.len(), 9);
    }
}

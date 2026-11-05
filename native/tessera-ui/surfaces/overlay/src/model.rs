//! The overlay's state as plain data (Overlay.qml, OverlayWidget.qml and
//! UiState.qml's `overlay`, `crosshair`): each widget's place, size, pin
//! and click-through, which ones exist (the gaming ones only with Settings'
//! gaming overlays), where the screen takes clicks, the crosshair, and the
//! Resources widget's graph. Pure, tested without a display.

use serde_json::{json, Map, Value};

/// Room round a widget for its shadow (OverlayWidget.m)
pub const M: f64 = 18.0;

/// The widgets, in the toolbar's order: (id, title, icon)
pub const WIDGETS: [(&str, &str, &str); 7] = [
    ("notes", "Notes", "note"),
    ("resources", "Resources", "cpu"),
    ("recorder", "Recorder", "record"),
    ("mixer", "Volume Mixer", "speaker-high"),
    ("image", "Floating Image", "image"),
    ("crosshair", "Crosshair", "crosshair"),
    ("fps", "FPS Limit", "gauge"),
];

/// The floating image, crosshair and FPS limit only with gaming overlays on
pub fn ids(gaming: bool) -> Vec<&'static str> {
    WIDGETS.iter().map(|w| w.0).filter(|id| gaming || !gaming_only(id)).collect()
}

pub fn gaming_only(id: &str) -> bool {
    matches!(id, "image" | "crosshair" | "fps")
}

/// A widget's smallest size (OverlayWidget's minWidth / minHeight)
pub fn min_size(id: &str) -> (f64, f64) {
    (220.0, if matches!(id, "recorder" | "fps") { 120.0 } else { 140.0 })
}

#[derive(Clone, Debug, PartialEq)]
pub struct Widget {
    pub open: bool,
    pub pinned: bool,
    pub clickthrough: bool,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// UiState's defaults
pub fn default_widget(id: &str) -> Widget {
    let (open, x, y, w, h) = match id {
        "notes" => (false, 80.0, 120.0, 340.0, 300.0),
        "resources" => (true, 440.0, 120.0, 320.0, 260.0),
        "recorder" => (false, 780.0, 120.0, 300.0, 170.0),
        "mixer" => (false, 1100.0, 120.0, 340.0, 300.0),
        "image" => (false, 80.0, 460.0, 360.0, 280.0),
        "crosshair" => (false, 460.0, 420.0, 300.0, 230.0),
        "fps" => (false, 780.0, 330.0, 300.0, 170.0),
        _ => (false, 100.0, 100.0, 320.0, 240.0),
    };
    Widget { open, pinned: false, clickthrough: false, x, y, w, h }
}

/// A widget from the shell's state (UiState.widget: its defaults where a key is missing)
pub fn widget(state: &Value, id: &str) -> Widget {
    let d = default_widget(id);
    let v = &state[id];
    let b = |k: &str, d: bool| v[k].as_bool().unwrap_or(d);
    let n = |k: &str, d: f64| v[k].as_f64().unwrap_or(d);
    Widget { open: b("open", d.open), pinned: b("pinned", d.pinned), clickthrough: b("clickthrough", d.clickthrough), x: n("x", d.x), y: n("y", d.y), w: n("w", d.w), h: n("h", d.h) }
}

impl Widget {
    /// On screen: open, and the overlay is open or it's pinned (and allowed)
    pub fn shown(&self, id: &str, overlay_open: bool, gaming: bool) -> bool {
        (gaming || !gaming_only(id)) && self.open && (overlay_open || self.pinned)
    }
    /// Its window-space rectangle with the shadow's room (the QML item's)
    pub fn item_rect(&self) -> (i32, i32, i32, i32) {
        ((self.x - M).round() as i32, (self.y - M).round() as i32, (self.w + 2.0 * M).round() as i32, (self.h + 2.0 * M).round() as i32)
    }
}

/// The crosshair (UiState.data.crosshair)
#[derive(Clone, Debug, PartialEq)]
pub struct Crosshair {
    pub enabled: bool,
    pub color: String,
    pub size: f64,
    pub thickness: f64,
    pub gap: f64,
    pub dot: bool,
}

pub fn crosshair(v: &Value) -> Crosshair {
    Crosshair {
        enabled: v["enabled"].as_bool().unwrap_or(false),
        color: v["color"].as_str().unwrap_or("#30D158").to_string(),
        size: v["size"].as_f64().unwrap_or(12.0),
        thickness: v["thickness"].as_f64().unwrap_or(2.0),
        gap: v["gap"].as_f64().unwrap_or(4.0),
        dot: v["dot"].as_bool().unwrap_or(true),
    }
}

/// The swatches the crosshair offers
pub const COLORS: [&str; 6] = ["#30D158", "#FF453A", "#0A84FF", "#FFD60A", "#FFFFFF", "#FF375F"];
/// The crosshair's sliders: (label, key, from, to)
pub const SLIDERS: [(&str, &str, f64, f64); 3] = [("Size", "size", 4.0, 40.0), ("Thickness", "thickness", 1.0, 8.0), ("Gap", "gap", 0.0, 20.0)];

/// The overlay's window exists: open, a widget pinned, or the crosshair on
pub fn active(open: bool, widgets: &Value, gaming: bool, cross: &Crosshair) -> bool {
    open || ids(gaming).iter().any(|id| {
        let w = widget(widgets, id);
        w.open && w.pinned
    }) || (gaming && cross.enabled)
}

/// Where the screen takes clicks: everywhere while open (None); closed,
/// only the pinned widgets that aren't click-through
pub fn input_rects(open: bool, widgets: &Value, gaming: bool) -> Option<Vec<(i32, i32, i32, i32)>> {
    if open {
        return None;
    }
    Some(ids(gaming).iter().map(|id| (id, widget(widgets, id))).filter(|(id, w)| w.shown(id, false, gaming) && !w.clickthrough).map(|(_, w)| w.item_rect()).collect())
}

/// A change to a widget, checked before it goes to the shell
/// (UiState.setWidget's patch: only its own keys, sizes whole and at least
/// the widget's smallest)
pub fn patch(id: &str, p: &Value) -> Option<Value> {
    if !WIDGETS.iter().any(|w| w.0 == id) {
        return None;
    }
    let mut out = Map::new();
    for k in ["open", "pinned", "clickthrough"] {
        if let Some(b) = p[k].as_bool() {
            out.insert(k.into(), json!(b));
        }
    }
    let (mw, mh) = min_size(id);
    for (k, min) in [("x", f64::MIN), ("y", f64::MIN), ("w", mw), ("h", mh)] {
        if let Some(n) = p[k].as_f64().filter(|n| n.is_finite()) {
            out.insert(k.into(), json!(n.round().max(min)));
        }
    }
    (!out.is_empty()).then(|| Value::Object(out))
}

/// A change to the crosshair, checked (a swatch, the sliders' ranges, the switch)
pub fn crosshair_patch(key: &str, v: &Value) -> Option<Value> {
    match key {
        "enabled" | "dot" => v.as_bool().map(|b| json!({ key: b })),
        "color" => v.as_str().filter(|c| COLORS.contains(c)).map(|c| json!({ "color": c })),
        _ => {
            let (_, _, from, to) = SLIDERS.iter().find(|s| s.1 == key)?;
            v.as_f64().map(|n| json!({ key: n.round().clamp(*from, *to) }))
        }
    }
}

/// The Resources widget's graph (its own Canvas: straight lines, the area
/// under them at a quarter alpha): the last minute right-aligned in a w × h
/// box, as SVG commands (area, line)
pub fn line_graph(values: &[f64], w: f64, h: f64) -> (String, String) {
    let n = 60usize;
    let vals = &values[values.len().saturating_sub(n)..];
    if vals.len() < 2 || w <= 0.0 || h <= 0.0 {
        return (String::new(), String::new());
    }
    let step = w / (n - 1) as f64;
    let off = (n - vals.len()) as f64 * step;
    let pt = |i: usize, v: f64| (off + i as f64 * step, h - v.clamp(0.0, 1.0) * (h - 2.0));
    let mut line = String::new();
    for (i, v) in vals.iter().enumerate() {
        let (x, y) = pt(i, *v);
        line += &format!("{}{x:.2} {y:.2} ", if i == 0 { "M " } else { "L " });
    }
    let last = off + (vals.len() - 1) as f64 * step;
    let mut area = format!("M {off:.2} {h:.2} ");
    for (i, v) in vals.iter().enumerate() {
        let (x, y) = pt(i, *v);
        area += &format!("L {x:.2} {y:.2} ");
    }
    area += &format!("L {last:.2} {h:.2} Z");
    (area, line.trim_end().to_string())
}

/// The FPS widget's line after a limit is set
pub fn fps_status(n: u32) -> String {
    if n == 0 { "Unlimited".into() } else { format!("Limited to {n} fps") }
}

/// The volume icon for a level (Audio.icon)
pub fn volume_icon(volume: f64, muted: bool) -> &'static str {
    if muted || volume <= 0.0 {
        "speaker-x"
    } else if volume < 0.34 {
        "speaker-none"
    } else if volume < 0.67 {
        "speaker-low"
    } else {
        "speaker-high"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widgets_from_the_shells_state() {
        assert_eq!(ids(false), vec!["notes", "resources", "recorder", "mixer"]);
        assert_eq!(ids(true).len(), 7);
        let s = json!({"notes": {"open": true, "x": 10, "pinned": true}});
        let n = widget(&s, "notes");
        assert_eq!((n.open, n.pinned, n.x, n.w), (true, true, 10.0, 340.0));
        assert!(widget(&s, "resources").open, "resources is open by default");
        assert!(n.shown("notes", false, false));
        assert!(!widget(&s, "resources").shown("resources", false, false));
        assert!(!widget(&json!({"image": {"open": true, "pinned": true}}), "image").shown("image", true, false));
        assert_eq!(n.item_rect(), (-8, 102, 376, 336));
    }

    #[test]
    fn where_clicks_land() {
        let s = json!({"notes": {"open": true, "pinned": true}, "mixer": {"open": true, "pinned": true, "clickthrough": true}});
        assert_eq!(input_rects(true, &s, false), None);
        assert_eq!(input_rects(false, &s, false), Some(vec![widget(&s, "notes").item_rect()]));
        let cross = crosshair(&json!({}));
        assert!(active(false, &s, false, &cross));
        assert!(!active(false, &json!({}), false, &cross));
        let on = crosshair(&json!({"enabled": true}));
        assert!(active(false, &json!({}), true, &on));
        assert!(!active(false, &json!({}), false, &on), "the crosshair is a gaming overlay");
    }

    #[test]
    fn changes_are_checked_before_the_shell_gets_them() {
        assert_eq!(patch("notes", &json!({"x": 10.4, "w": 50, "evil": 1})), Some(json!({"x": 10.0, "w": 220.0})));
        assert_eq!(patch("fps", &json!({"h": 10})), Some(json!({"h": 120.0})));
        assert_eq!(patch("nope", &json!({"open": true})), None);
        assert_eq!(patch("notes", &json!({})), None);
        assert_eq!(crosshair_patch("size", &json!(99)), Some(json!({"size": 40.0})));
        assert_eq!(crosshair_patch("color", &json!("#FF453A")), Some(json!({"color": "#FF453A"})));
        assert_eq!(crosshair_patch("color", &json!("red; rm -rf")), None);
        assert_eq!(crosshair_patch("enabled", &json!(true)), Some(json!({"enabled": true})));
        assert_eq!(crosshair_patch("speed", &json!(1)), None);
    }

    #[test]
    fn the_resources_graph_and_small_texts() {
        assert_eq!(line_graph(&[0.5], 59.0, 52.0), (String::new(), String::new()));
        let (area, line) = line_graph(&[0.0, 1.0], 59.0, 52.0);
        assert_eq!(line, "M 58.00 52.00 L 59.00 2.00");
        assert!(area.starts_with("M 58.00 52.00 L 58.00 52.00") && area.ends_with("L 59.00 52.00 Z"));
        assert_eq!(fps_status(0), "Unlimited");
        assert_eq!(fps_status(60), "Limited to 60 fps");
        assert_eq!(volume_icon(0.5, false), "speaker-low");
        assert_eq!(volume_icon(0.9, true), "speaker-x");
    }
}

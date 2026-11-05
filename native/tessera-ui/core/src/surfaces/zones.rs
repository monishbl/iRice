//! Where a snapped window lands (services/Tiling.qml's layouts, area and
//! rect): fractions of a monitor's usable area, with the gaps Settings sets.

use serde::Deserialize;
use serde_json::Value;

/// [x, y, w, h] as fractions of the usable area (Tiling.layouts)
pub fn layout(name: &str) -> Option<[f64; 4]> {
    Some(match name {
        "left" => [0.0, 0.0, 0.5, 1.0],
        "right" => [0.5, 0.0, 0.5, 1.0],
        "top" => [0.0, 0.0, 1.0, 0.5],
        "bottom" => [0.0, 0.5, 1.0, 0.5],
        "topLeft" => [0.0, 0.0, 0.5, 0.5],
        "topRight" => [0.5, 0.0, 0.5, 0.5],
        "bottomLeft" => [0.0, 0.5, 0.5, 0.5],
        "bottomRight" => [0.5, 0.5, 0.5, 0.5],
        "leftThird" => [0.0, 0.0, 1.0 / 3.0, 1.0],
        "centerThird" => [1.0 / 3.0, 0.0, 1.0 / 3.0, 1.0],
        "rightThird" => [2.0 / 3.0, 0.0, 1.0 / 3.0, 1.0],
        "leftTwoThirds" => [0.0, 0.0, 2.0 / 3.0, 1.0],
        "rightTwoThirds" => [1.0 / 3.0, 0.0, 2.0 / 3.0, 1.0],
        "fill" => [0.0, 0.0, 1.0, 1.0],
        "center" => [0.15, 0.1, 0.7, 0.8],
        _ => return None,
    })
}

/// [between windows, at the screen's edges] (System.gapLevels)
pub fn gaps(cfg: &Value) -> (f64, f64) {
    match cfg["appearance"]["gaps"].as_str().unwrap_or("compact") {
        "none" => (0.0, 0.0),
        "comfortable" => (5.0, 10.0),
        _ => (3.0, 4.0),
    }
}

/// A monitor as j/monitors gives it (with what bars reserve)
#[derive(Deserialize, Default, Clone, Debug, PartialEq)]
pub struct Monitor {
    pub name: String,
    #[serde(default)]
    pub x: i32,
    #[serde(default)]
    pub y: i32,
    #[serde(default)]
    pub width: i32,
    #[serde(default)]
    pub height: i32,
    #[serde(default = "one")]
    pub scale: f64,
    /// left, top, right, bottom
    #[serde(default)]
    pub reserved: [i32; 4],
}

fn one() -> f64 {
    1.0
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// The usable area of a monitor in global coordinates, inside the outer gap
/// (Tiling.area)
pub fn area(m: &Monitor, gap_out: f64) -> Rect {
    let r = m.reserved;
    let (x, y) = ((m.x + r[0]) as f64, (m.y + r[1]) as f64);
    let s = if m.scale > 0.0 { m.scale } else { 1.0 };
    let w = m.width as f64 / s - (r[0] + r[2]) as f64;
    let h = m.height as f64 / s - (r[1] + r[3]) as f64;
    Rect { x: x + gap_out, y: y + gap_out, w: w - 2.0 * gap_out, h: h - 2.0 * gap_out }
}

/// A layout's rectangle in an area: edges shared with another tile get half
/// the inner gap (Tiling.rect)
pub fn rect(name: &str, a: Rect, gap_in: f64) -> Option<Rect> {
    let f = layout(name)?;
    let round = |v: f64| v.round();
    if name == "center" {
        return Some(Rect { x: round(a.x + a.w * f[0]), y: round(a.y + a.h * f[1]), w: round(a.w * f[2]), h: round(a.h * f[3]) });
    }
    let l = if f[0] > 0.0 { gap_in } else { 0.0 };
    let t = if f[1] > 0.0 { gap_in } else { 0.0 };
    let r = if f[0] + f[2] < 0.999 { gap_in } else { 0.0 };
    let b = if f[1] + f[3] < 0.999 { gap_in } else { 0.0 };
    Some(Rect { x: round(a.x + a.w * f[0] + l), y: round(a.y + a.h * f[1] + t), w: round(a.w * f[2] - l - r), h: round(a.h * f[3] - t - b) })
}

/// The preview's target in the monitor's own coordinates (SnapPreview.place)
pub fn target(zone: &str, m: &Monitor, cfg: &Value) -> Option<Rect> {
    let (gin, gout) = gaps(cfg);
    let r = rect(zone, area(m, gout), gin)?;
    Some(Rect { x: r.x - m.x as f64, y: r.y - m.y as f64, ..r })
}

/// One of the glass plugin's drag lines: `tessera-drag>>zone,MON,ZONE,ADDR`
/// (or `drop`, or irice-drag from a plugin built before the rename):
/// (kind, monitor, zone)
pub fn drag_event(line: &str) -> Option<(String, String, String)> {
    let data = line.strip_prefix("tessera-drag>>").or_else(|| line.strip_prefix("irice-drag>>"))?;
    let mut f = data.trim_end().split(',');
    Some((f.next()?.to_string(), f.next().unwrap_or("").to_string(), f.next().unwrap_or("none").to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn mon() -> Monitor {
        Monitor { name: "DP-1".into(), x: 1920, y: 0, width: 3840, height: 2160, scale: 2.0, reserved: [0, 40, 0, 0] }
    }

    #[test]
    fn halves_and_thirds_as_tiling_qml() {
        let a = area(&mon(), 4.0);
        assert_eq!(a, Rect { x: 1924.0, y: 44.0, w: 1912.0, h: 1032.0 });
        assert_eq!(rect("left", a, 3.0), Some(Rect { x: 1924.0, y: 44.0, w: 953.0, h: 1032.0 }));
        assert_eq!(rect("right", a, 3.0), Some(Rect { x: 2883.0, y: 44.0, w: 953.0, h: 1032.0 }));
        assert_eq!(rect("fill", a, 3.0), Some(Rect { x: 1924.0, y: 44.0, w: 1912.0, h: 1032.0 }));
        assert_eq!(rect("center", a, 3.0), Some(Rect { x: 2211.0, y: 147.0, w: 1338.0, h: 826.0 }));
        let third = rect("centerThird", a, 3.0).unwrap();
        assert_eq!((third.x, third.w), (2564.0, 631.0));
        assert_eq!(rect("nowhere", a, 3.0), None);
    }

    #[test]
    fn the_target_is_on_the_monitor() {
        let t = target("topLeft", &mon(), &json!({"appearance": {"gaps": "none"}})).unwrap();
        assert_eq!(t, Rect { x: 0.0, y: 40.0, w: 960.0, h: 520.0 });
        assert_eq!(gaps(&json!({})), (3.0, 4.0));
        assert_eq!(gaps(&json!({"appearance": {"gaps": "comfortable"}})), (5.0, 10.0));
    }

    #[test]
    fn reads_the_plugins_lines() {
        assert_eq!(drag_event("tessera-drag>>zone,DP-1,left,0x55\n"), Some(("zone".into(), "DP-1".into(), "left".into())));
        assert_eq!(drag_event("irice-drag>>drop,DP-1,none,0x55"), Some(("drop".into(), "DP-1".into(), "none".into())));
        assert_eq!(drag_event("openwindow>>55,1,foot,foot"), None);
        let m: Vec<Monitor> = serde_json::from_str(r#"[{"name":"DP-1","x":0,"y":0,"width":1920,"height":1080,"scale":1.0,"reserved":[0,32,0,0],"focused":true}]"#).unwrap();
        assert_eq!(m[0].reserved, [0, 32, 0, 0]);
    }
}

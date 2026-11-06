//! The region selector's and the toolbar's pure parts (compare
//! shell/common/ScreenGeometry.js, RegionSelector.qml and ScreenToolbar.qml).

use serde_json::Value;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Rect { x, y, w, h }
    }
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x <= self.x + self.w && y >= self.y && y <= self.y + self.h
    }
}

/// Clipped to a `width` × `height` screen (windows partly off it included)
pub fn clip(r: Rect, width: f32, height: f32) -> Rect {
    if ![r.x, r.y, r.w, r.h, width, height].iter().all(|v| v.is_finite()) || width <= 0.0 || height <= 0.0 {
        return Rect::default();
    }
    let left = r.x.min(width).max(0.0);
    let top = r.y.min(height).max(0.0);
    let right = (r.x + r.w.max(0.0)).min(width).max(left);
    let bottom = (r.y + r.h.max(0.0)).min(height).max(top);
    Rect::new(left, top, right - left, bottom - top)
}

/// Whole pixels, for grim and the recorders
pub fn integer_clip(r: Rect, width: f32, height: f32) -> Rect {
    let c = clip(r, width, height);
    let (left, top) = (c.x.floor(), c.y.floor());
    let right = width.floor().min((c.x + c.w).ceil());
    let bottom = height.floor().min((c.y + c.h).ceil());
    Rect::new(left, top, (right - left).max(0.0), (bottom - top).max(0.0))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stroke {
    /// too small to be anything (a click)
    None,
    /// a roughly straight drag: its ends are opposite corners
    Box,
    /// anything else drawn round something: its bounding box counts
    Circle,
}

pub fn extent(points: &[(f32, f32)]) -> Rect {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
    for &(x, y) in points {
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    }
    Rect::new(x0, y0, x1 - x0, y1 - y0)
}

/// Circle to search: what a drawn stroke is (ScreenGeometry.strokeKind)
pub fn stroke_kind(points: &[(f32, f32)]) -> Stroke {
    if points.len() < 2 {
        return Stroke::None;
    }
    let e = extent(points);
    if e.w.max(e.h) < 8.0 {
        return Stroke::None;
    }
    let (a, b) = (points[0], points[points.len() - 1]);
    let chord = (b.0 - a.0).hypot(b.1 - a.1);
    if chord < 24.0 {
        return Stroke::Circle; // (it ends near where it began)
    }
    let mut length = 0.0f32;
    let mut stray = 0.0f32;
    for (i, p) in points.iter().enumerate() {
        if i > 0 {
            length += (p.0 - points[i - 1].0).hypot(p.1 - points[i - 1].1);
        }
        stray = stray.max(((b.0 - a.0) * (a.1 - p.1) - (a.0 - p.0) * (b.1 - a.1)).abs() / chord);
    }
    if length <= 1.2 * chord + 16.0 && stray <= 12f32.max(0.12 * chord) {
        Stroke::Box
    } else {
        Stroke::Circle
    }
}

/// The area a stroke selects, clipped: a box from end to end, or a shape's
/// bounding box padded by `pad`
pub fn stroke_box(points: &[(f32, f32)], pad: f32, width: f32, height: f32) -> Rect {
    let kind = stroke_kind(points);
    if kind == Stroke::None {
        return Rect::default();
    }
    let e = if kind == Stroke::Box { extent(&[points[0], points[points.len() - 1]]) } else { extent(points) };
    let p = if kind == Stroke::Box { 0.0 } else { pad };
    clip(Rect::new(e.x - p, e.y - p, e.w + 2.0 * p, e.h + 2.0 * p), width, height)
}

/// The box between two corners
pub fn box_between(a: (f32, f32), b: (f32, f32)) -> Rect {
    Rect::new(a.0.min(b.0), a.1.min(b.1), (b.0 - a.0).abs(), (b.1 - a.1).abs())
}

/// The windows on a monitor's showing workspace, in its own coordinates
/// (Hyprland's `j/monitors` and `j/clients`), front first
pub fn windows_on(monitors: &Value, clients: &Value, monitor: &str) -> Vec<Rect> {
    let Some(m) = monitors.as_array().and_then(|a| a.iter().find(|m| m["name"].as_str() == Some(monitor))) else { return Vec::new() };
    let (mx, my) = (m["x"].as_f64().unwrap_or(0.0) as f32, m["y"].as_f64().unwrap_or(0.0) as f32);
    let ws = m["activeWorkspace"]["id"].as_i64();
    let mut list: Vec<(i64, Rect)> = clients
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|c| c["workspace"]["id"].as_i64() == ws && ws.is_some() && c["mapped"].as_bool() != Some(false) && c["hidden"].as_bool() != Some(true))
                .filter_map(|c| {
                    let at = c["at"].as_array()?;
                    let size = c["size"].as_array()?;
                    let f = |v: &Value| v.as_f64().unwrap_or(0.0) as f32;
                    Some((c["focusHistoryID"].as_i64().unwrap_or(i64::MAX), Rect::new(f(&at[0]) - mx, f(&at[1]) - my, f(&size[0]), f(&size[1]))))
                })
                .collect()
        })
        .unwrap_or_default();
    // (the most recently focused first: on top where windows overlap)
    list.sort_by_key(|(f, _)| *f);
    list.into_iter().map(|(_, r)| r).collect()
}

/// What the selection is for, in the hint (RegionSelector's `verb`)
pub fn verb(mode: &str) -> &'static str {
    match mode {
        "screenshot" => "capture",
        "ocr" => "copy text",
        "translate" => "translate",
        "record" | "record-window" => "record",
        "ask" => "ask AI about",
        _ => "",
    }
}

pub fn hint(mode: &str) -> String {
    match mode {
        "record-window" => "Click a window to record it. Esc to cancel.".into(),
        "circle" => "Circle or scribble over anything to search it. Esc to cancel.".into(),
        m => format!("Drag to select an area to {}, or click a window. Esc to cancel.", verb(m)),
    }
}

// ── The toolbar (ScreenToolbar.qml's tools) ────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub enum Act {
    /// a separator
    Sep,
    /// Options, the menu above the bar
    Options,
    /// a screenshot of the whole screen
    Shot,
    /// pick a region for this ("screenshot", "ocr", "translate", "ask")
    Select(&'static str),
    /// record this area now ("screen", "window", "region"), or stop
    Record(&'static str),
    /// recording only: choose the area, Record starts it
    Area(&'static str),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tool {
    pub icon: &'static str,
    pub label: &'static str,
    pub act: Act,
}

pub fn tools(record_only: bool, recording: bool) -> Vec<Tool> {
    let t = |icon, label, act| Tool { icon, label, act };
    let sep = || Tool { icon: "", label: "", act: Act::Sep };
    if record_only {
        return vec![t("monitor", "Entire Screen", Act::Area("screen")), t("app-window", "Window", Act::Area("window")), t("selection", "Selected Region", Act::Area("region")), sep(), t("", "Options", Act::Options)];
    }
    let mut v = vec![
        t("monitor", "Capture Screen", Act::Shot),
        t("selection", "Capture Selection", Act::Select("screenshot")),
        sep(),
        t("text-t", "Copy Text", Act::Select("ocr")),
        t("translate", "Translate", Act::Select("translate")),
        t("sparkle", "Ask AI", Act::Select("ask")),
        sep(),
        t("record", if recording { "Stop Recording" } else { "Record Screen" }, Act::Record("screen")),
    ];
    if !recording {
        v.push(t("app-window", "Record Window", Act::Record("window")));
        v.push(t("selection-plus", "Record Selection", Act::Record("region")));
    }
    v.push(sep());
    v.push(t("", "Options", Act::Options));
    v
}

/// What the format's choice means (the note under it)
pub fn codec_note(codec: &str) -> &'static str {
    match codec {
        "h264" => "Plays everywhere",
        "hevc" => "Smaller files; some players and sites can't play it",
        _ => "Smallest files; needs a recent graphics card to record",
    }
}

/// The recording settings (Config.qml's recorder, its defaults)
#[derive(Clone, Debug, PartialEq)]
pub struct RecCfg {
    pub area: String,
    pub mic: String,
    pub system_audio: bool,
    pub cursor: bool,
    pub countdown: i64,
    pub fps: i64,
    pub codec: String,
    pub quality: String,
    pub folder: String,
}

impl RecCfg {
    pub fn from_cfg(cfg: &Value) -> Self {
        let r = &cfg["recorder"];
        let s = |k: &str, d: &str| r[k].as_str().unwrap_or(d).to_string();
        let b = |k: &str, d: bool| r[k].as_bool().unwrap_or(d);
        let i = |k: &str, d: i64| r[k].as_i64().unwrap_or(d);
        RecCfg {
            area: s("area", "screen"),
            mic: s("mic", ""),
            system_audio: b("systemAudio", true),
            cursor: b("cursor", true),
            countdown: i("countdown", 0),
            fps: i("fps", 60),
            codec: s("codec", "h264"),
            quality: s("quality", "high"),
            folder: s("folder", ""),
        }
    }
    /// One setting changed here (before the shell writes it back)
    pub fn set(&mut self, key: &str, v: &Value) {
        match key {
            "area" => self.area = v.as_str().unwrap_or("screen").into(),
            "mic" => self.mic = v.as_str().unwrap_or("").into(),
            "systemAudio" => self.system_audio = v.as_bool().unwrap_or(true),
            "cursor" => self.cursor = v.as_bool().unwrap_or(true),
            "countdown" => self.countdown = v.as_i64().unwrap_or(0),
            "fps" => self.fps = v.as_i64().unwrap_or(60),
            "codec" => self.codec = v.as_str().unwrap_or("h264").into(),
            "quality" => self.quality = v.as_str().unwrap_or("high").into(),
            "folder" => self.folder = v.as_str().unwrap_or("").into(),
            _ => {}
        }
    }
}

pub const CODECS: [&str; 3] = ["h264", "hevc", "av1"];
pub const QUALITIES: [&str; 4] = ["medium", "high", "very_high", "ultra"];
pub const COUNTDOWNS: [i64; 3] = [0, 3, 5];

/// The toolbar's bottom margin (above the dock, as ScreenToolbar.qml)
pub fn toolbar_bottom(cfg: &Value) -> f32 {
    let d = &cfg["dock"];
    if d["enabled"].as_bool().unwrap_or(true) {
        d["iconSize"].as_f64().unwrap_or(48.0) as f32 + 14.0 + 6.0 + 14.0
    } else {
        14.0
    }
}

/// "~/Videos/…" for a folder under the home folder
pub fn tilde(path: &str, home: &str) -> String {
    if !home.is_empty() && (path == home || path.starts_with(&format!("{home}/"))) {
        format!("~{}", &path[home.len()..])
    } else {
        path.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn clipping_to_the_screen() {
        assert_eq!(clip(Rect::new(-10.0, 5.0, 30.0, 10.0), 100.0, 50.0), Rect::new(0.0, 5.0, 20.0, 10.0));
        assert_eq!(clip(Rect::new(90.0, 40.0, 30.0, 30.0), 100.0, 50.0), Rect::new(90.0, 40.0, 10.0, 10.0));
        assert_eq!(clip(Rect::new(0.0, 0.0, 10.0, 10.0), 0.0, 50.0), Rect::default());
        assert_eq!(clip(Rect::new(f32::NAN, 0.0, 10.0, 10.0), 100.0, 50.0), Rect::default());
        assert_eq!(integer_clip(Rect::new(1.4, 2.6, 10.2, 3.1), 100.0, 50.0), Rect::new(1.0, 2.0, 11.0, 4.0));
    }

    #[test]
    fn a_straight_drag_is_a_box_a_round_one_a_circle() {
        assert_eq!(stroke_kind(&[(0.0, 0.0)]), Stroke::None);
        assert_eq!(stroke_kind(&[(0.0, 0.0), (3.0, 3.0)]), Stroke::None);
        let line: Vec<(f32, f32)> = (0..=50).map(|i| (i as f32 * 4.0, i as f32 * 2.0)).collect();
        assert_eq!(stroke_kind(&line), Stroke::Box);
        assert_eq!(stroke_box(&line, 12.0, 1000.0, 1000.0), Rect::new(0.0, 0.0, 200.0, 100.0));
        let circle: Vec<(f32, f32)> = (0..=60).map(|i| {
            let t = i as f32 / 60.0 * std::f32::consts::TAU;
            (300.0 + 100.0 * t.cos(), 300.0 + 80.0 * t.sin())
        }).collect();
        assert_eq!(stroke_kind(&circle), Stroke::Circle);
        let b = stroke_box(&circle, 12.0, 1000.0, 1000.0);
        assert!((b.x - 188.0).abs() < 0.5 && (b.w - 224.0).abs() < 0.5 && (b.h - 184.0).abs() < 1.0);
        // a hook: long way round from one end to the other
        let hook = vec![(0.0, 0.0), (200.0, 0.0), (200.0, 200.0), (100.0, 100.0)];
        assert_eq!(stroke_kind(&hook), Stroke::Circle);
    }

    #[test]
    fn windows_on_the_showing_workspace() {
        let mons = json!([{"name": "DP-1", "x": 1920, "y": 0, "activeWorkspace": {"id": 2}}]);
        let clients = json!([
            {"at": [1940, 50], "size": [800, 600], "workspace": {"id": 2}, "focusHistoryID": 1},
            {"at": [2000, 100], "size": [400, 300], "workspace": {"id": 2}, "focusHistoryID": 0},
            {"at": [0, 0], "size": [100, 100], "workspace": {"id": 1}, "focusHistoryID": 2}
        ]);
        let w = windows_on(&mons, &clients, "DP-1");
        assert_eq!(w, vec![Rect::new(80.0, 100.0, 400.0, 300.0), Rect::new(20.0, 50.0, 800.0, 600.0)]);
        assert!(windows_on(&mons, &clients, "HDMI-A-1").is_empty());
    }

    #[test]
    fn hints_say_what_the_drag_is_for() {
        assert_eq!(hint("ocr"), "Drag to select an area to copy text, or click a window. Esc to cancel.");
        assert_eq!(hint("record-window"), "Click a window to record it. Esc to cancel.");
        assert!(hint("circle").starts_with("Circle or scribble"));
    }

    #[test]
    fn the_toolbar_tools() {
        let all = tools(false, false);
        assert_eq!(all.len(), 12);
        assert_eq!(all[0].act, Act::Shot);
        assert_eq!(all.iter().filter(|t| t.act == Act::Sep).count(), 3);
        // recording: Stop, and no other ways to record
        let rec = tools(false, true);
        assert_eq!(rec.len(), 10);
        assert!(rec.iter().any(|t| t.label == "Stop Recording"));
        let only = tools(true, false);
        assert_eq!(only.iter().map(|t| t.label).collect::<Vec<_>>(), vec!["Entire Screen", "Window", "Selected Region", "", "Options"]);
    }

    #[test]
    fn recording_settings_and_their_place() {
        let c = RecCfg::from_cfg(&json!({}));
        assert_eq!((c.area.as_str(), c.fps, c.codec.as_str(), c.system_audio), ("screen", 60, "h264", true));
        let mut c = RecCfg::from_cfg(&json!({"recorder": {"fps": 30, "mic": "default"}}));
        assert_eq!((c.fps, c.mic.as_str()), (30, "default"));
        c.set("codec", &json!("av1"));
        assert_eq!(c.codec, "av1");
        assert_eq!(toolbar_bottom(&json!({})), 82.0);
        assert_eq!(toolbar_bottom(&json!({"dock": {"enabled": false}})), 14.0);
        assert_eq!(tilde("/home/a/Videos/Screen Recordings", "/home/a"), "~/Videos/Screen Recordings");
        assert_eq!(tilde("/mnt/x", "/home/a"), "/mnt/x");
    }
}

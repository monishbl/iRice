//! The shelf's pure logic: what the feed says, how a row reads, what a drag
//! out carries (shell/services/ShelfLogic.js's mimeFor and drop), where the
//! card goes, which drop zone target is under the drag, and Material's
//! dashed outline and clover8.

use serde_json::Value;

/// One thing on the shelf (Shelf.items): a file, folder, picture or text
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Item {
    pub uri: String,
    pub path: String,
    pub name: String,
    /// image | folder | file | text
    pub kind: String,
    pub text: String,
}

/// What the shell's feed says (ShelfFeed.qml: `{"to":"shelf","state":{…}}`)
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Feed {
    pub opened: bool,
    pub items: Vec<Item>,
    /// the screen it's on ("" the first)
    pub screen: String,
    /// at the left edge (else the right)
    pub left: bool,
    /// the pointer's height on that screen (-1: near the top)
    pub at_y: f32,
    pub zone: bool,
    pub zone_screen: String,
}

impl Feed {
    pub fn parse(v: &Value) -> Option<Feed> {
        let s = v.get("state")?;
        let st = |k: &str| s[k].as_str().unwrap_or_default().to_string();
        let items = s["items"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|i| {
                        let f = |k: &str| i[k].as_str().unwrap_or_default().to_string();
                        Item { uri: f("uri"), path: f("path"), name: f("name"), kind: if f("kind").is_empty() { "file".into() } else { f("kind") }, text: f("text") }
                    })
                    .collect()
            })
            .unwrap_or_default();
        Some(Feed {
            opened: s["opened"].as_bool().unwrap_or(false),
            items,
            screen: st("screen"),
            left: s["side"].as_str() == Some("left"),
            at_y: s["atY"].as_f64().unwrap_or(-1.0) as f32,
            zone: s["zone"].as_bool().unwrap_or(false),
            zone_screen: st("zoneScreen"),
        })
    }

    /// Shelf.paths: the items that are files
    pub fn paths(&self) -> Vec<String> {
        self.items.iter().filter(|i| !i.path.is_empty()).map(|i| i.path.clone()).collect()
    }
}

/// `name` ends with one of these extensions (any case)
fn ends(name: &str, exts: &[&str]) -> bool {
    let n = name.to_lowercase();
    exts.iter().any(|e| n.ends_with(&format!(".{e}")))
}

/// A row's glyph name (ShelfWindow.qml's delegate)
pub fn glyph_name(i: &Item) -> &'static str {
    let p = i.path.as_str();
    if i.kind == "text" {
        "text-t"
    } else if i.kind == "folder" {
        "folder"
    } else if ends(p, &["pdf"]) {
        "file-pdf"
    } else if ends(p, &["zip", "tar", "gz", "xz", "zst", "7z", "rar"]) {
        "file-zip"
    } else if ends(p, &["mp3", "flac", "ogg", "wav", "m4a", "opus"]) {
        "file-audio"
    } else if ends(p, &["mp4", "mkv", "webm", "mov", "avi"]) {
        "file-video"
    } else if ends(p, &["txt", "md", "csv", "log", "json"]) {
        "file-text"
    } else {
        "file"
    }
}

/// A row's second line: "Text", or the folder it's in with ~ for home
/// (`path.replace(/\/[^/]*$/, "").replace(HOME, "~")`)
pub fn detail(i: &Item, home: &str) -> String {
    if i.kind == "text" {
        return "Text".into();
    }
    let dir = match i.path.rfind('/') {
        Some(k) => &i.path[..k],
        None => i.path.as_str(),
    };
    if home.is_empty() {
        dir.to_string()
    } else {
        dir.replacen(home, "~", 1)
    }
}

/// Text elided in the middle (Text.ElideMiddle, which Slint hasn't): at most
/// `max` characters, the start and the end kept round "…"
pub fn elide_middle(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max || max < 3 {
        return s.to_string();
    }
    let keep = max - 1;
    let head = keep.div_ceil(2);
    let tail = keep - head;
    let mut out: String = chars[..head].iter().collect();
    out.push('…');
    out.extend(&chars[chars.len() - tail..]);
    out
}

/// About how many characters fit `width` px of text at `size` px (Inter or
/// Google Sans: about 0.55 em a character on average)
pub fn fits(width: f32, size: f32) -> usize {
    (width / (size * 0.55)).floor().max(3.0) as usize
}

/// What a drag out carries (ShelfLogic.mimeFor): the files' URIs, and text
/// (the text items, else the files' paths)
pub fn mime_for(list: &[Item]) -> Vec<(String, Vec<u8>)> {
    let uris: Vec<&str> = list.iter().filter(|i| !i.uri.is_empty()).map(|i| i.uri.as_str()).collect();
    let texts: Vec<&str> = list.iter().filter(|i| i.kind == "text").map(|i| i.text.as_str()).collect();
    let mut m = Vec::new();
    if !uris.is_empty() {
        m.push(("text/uri-list".to_string(), (uris.join("\r\n") + "\r\n").into_bytes()));
    }
    if !texts.is_empty() {
        m.push(("text/plain".to_string(), texts.join("\n").into_bytes()));
    } else if !uris.is_empty() {
        let paths: Vec<&str> = list.iter().filter(|i| !i.path.is_empty()).map(|i| i.path.as_str()).collect();
        m.push(("text/plain".to_string(), paths.join("\n").into_bytes()));
    }
    m
}

/// Whether a drop would add anything (ShelfLogic.drop's `added > 0`):
/// file URLs not on it yet, else text that isn't a file URL
pub fn would_add(items: &[Item], uris: &[String], text: &str) -> bool {
    if uris.iter().any(|u| u.starts_with("file://") && !items.iter().any(|i| &i.uri == u)) {
        return true;
    }
    let t = text.trim();
    !t.is_empty() && !t.starts_with("file://")
}

/// The shadow's room round the card (ShelfWindow.qml's `m`)
pub const M: f32 = 24.0;
pub const CARD_W: f32 = 240.0;

pub fn row_h(material: bool) -> f32 {
    if material {
        52.0
    } else {
        44.0
    }
}

/// The card's height: the title, then the empty drop area, or "All" (two
/// or more) and up to six rows; 12 round, 8 between
pub fn card_height(count: usize, material: bool) -> f32 {
    let title = 28.0;
    let body = if count == 0 {
        title + 8.0 + if material { 160.0 } else { 132.0 }
    } else if count == 1 {
        title + 8.0 + row_h(material)
    } else {
        title + 8.0 + row_h(material) + 8.0 + count.min(6) as f32 * row_h(material)
    };
    body + 24.0
}

/// Where the card's window starts (ShelfWindow.qml's margins.top: near the
/// top, or centred on where the drag was, kept on the screen), and so the
/// card's top (that plus the shadow's room)
pub fn card_top(at_y: f32, card_h: f32, screen_h: f32) -> f32 {
    let h = card_h + 2.0 * M;
    let margin = if at_y < 0.0 { 40.0 - M } else { (40.0 - M).max((at_y - h / 2.0).min(screen_h - h - 8.0)) };
    margin + M
}

/// The drop zone's window and its targets, in the window (560 wide, the zone
/// 520 at its top centre: 16 in, the header 22, 14 down, then two 236 × 116
/// 12 apart)
pub const ZONE_W: u32 = 560;
pub const ZONE_H: f32 = 16.0 + 22.0 + 14.0 + 116.0 + 16.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Keep,
    Send,
}

pub fn in_zone(x: f32, y: f32) -> bool {
    let x0 = (ZONE_W as f32 - 520.0) / 2.0;
    x >= x0 && x < x0 + 520.0 && y >= 0.0 && y < ZONE_H
}

pub fn target_at(x: f32, y: f32) -> Option<Target> {
    let x0 = (ZONE_W as f32 - 520.0) / 2.0 + 16.0;
    let y0 = 16.0 + 22.0 + 14.0;
    if !(y0..y0 + 116.0).contains(&y) {
        return None;
    }
    if (x0..x0 + 236.0).contains(&x) {
        Some(Target::Keep)
    } else if (x0 + 248.0..x0 + 248.0 + 236.0).contains(&x) {
        Some(Target::Send)
    } else {
        None
    }
}

/// Material's eight-leaf clover (MaterialShapes' clover8), in shapes.rs's
/// 0..100 box: a pinched polar wave as its clover4
pub fn clover8() -> String {
    let (n, a, r) = (8.0f64, 0.12, 44.0);
    let steps = 240;
    let mut d = String::new();
    for i in 0..=steps {
        let t = i as f64 / steps as f64 * std::f64::consts::TAU;
        let w = (n / 2.0 * t).cos().abs().powf(0.6) * 2.0 - 1.0;
        let rr = r * (1.0 + a * w);
        let (x, y) = (50.0 + rr * (t - std::f64::consts::FRAC_PI_2).cos(), 50.0 + rr * (t - std::f64::consts::FRAC_PI_2).sin());
        d += &format!("{}{x:.2} {y:.2} ", if i == 0 { "M " } else { "L " });
    }
    d + "Z"
}

/// A point `s` px along a rounded rectangle's outline (clockwise from the
/// top edge's start)
fn along(x: f64, y: f64, w: f64, h: f64, r: f64, s: f64) -> (f64, f64) {
    let (sw, sh) = (w - 2.0 * r, h - 2.0 * r);
    let arc = std::f64::consts::FRAC_PI_2 * r;
    let total = 2.0 * (sw + sh) + 4.0 * arc;
    let mut s = s.rem_euclid(total);
    // top, top-right arc, right, bottom-right arc, bottom, bottom-left, left, top-left
    let corner = |cx: f64, cy: f64, a0: f64, s: f64| {
        let a = a0 + if r > 0.0 { s / r } else { 0.0 };
        (cx + r * a.cos(), cy + r * a.sin())
    };
    use std::f64::consts::{FRAC_PI_2 as Q, PI};
    if s < sw {
        return (x + r + s, y);
    }
    s -= sw;
    if s < arc {
        return corner(x + w - r, y + r, -Q, s);
    }
    s -= arc;
    if s < sh {
        return (x + w, y + r + s);
    }
    s -= sh;
    if s < arc {
        return corner(x + w - r, y + h - r, 0.0, s);
    }
    s -= arc;
    if s < sw {
        return (x + w - r - s, y + h);
    }
    s -= sw;
    if s < arc {
        return corner(x + r, y + h - r, Q, s);
    }
    s -= arc;
    if s < sh {
        return (x, y + h - r - s);
    }
    s -= sh;
    corner(x + r, y + r, PI, s.min(arc))
}

/// Material's dashed outline (a Shape with DashLine, dashPattern [4, 3] in
/// stroke widths) as path commands in px: Slint's Path has no dashes, so
/// each dash is its own short polyline. The rectangle (1, 1, w-2, h-2),
/// corners r-1, as the QML's PathRectangle
pub fn dashes(w: f32, h: f32, r: f32, stroke: f32) -> String {
    let (x, y, w, h, r) = (1.0f64, 1.0f64, w as f64 - 2.0, h as f64 - 2.0, (r as f64 - 1.0).max(0.0));
    let r = r.min(w / 2.0).min(h / 2.0);
    let total = 2.0 * (w - 2.0 * r + h - 2.0 * r) + 2.0 * std::f64::consts::PI * r;
    let (dash, gap) = (4.0 * stroke as f64, 3.0 * stroke as f64);
    let mut d = String::new();
    let mut s = 0.0;
    while s < total - 0.5 {
        let end = (s + dash).min(total);
        let mut t = s;
        let (px, py) = along(x, y, w, h, r, t);
        d += &format!("M {px:.2} {py:.2} ");
        while t < end {
            t = (t + 2.0).min(end);
            let (px, py) = along(x, y, w, h, r, t);
            d += &format!("L {px:.2} {py:.2} ");
        }
        s += dash + gap;
    }
    d.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn file(path: &str) -> Item {
        Item { uri: format!("file://{path}"), path: path.into(), name: path.rsplit('/').next().unwrap_or_default().into(), kind: "file".into(), text: String::new() }
    }
    fn text(t: &str) -> Item {
        Item { name: t.into(), kind: "text".into(), text: t.into(), ..Default::default() }
    }

    #[test]
    fn the_feed_says_what_the_shelf_is() {
        let f = Feed::parse(&json!({"to": "shelf", "state": {"opened": true, "side": "left", "atY": 300, "zone": false, "screen": "DP-1",
            "items": [{"uri": "file:///home/ada/a.pdf", "path": "/home/ada/a.pdf", "name": "a.pdf", "kind": "file"}, {"text": "hello", "name": "hello", "kind": "text"}]}}))
        .unwrap();
        assert!(f.opened && f.left && !f.zone);
        assert_eq!((f.at_y, f.screen.as_str(), f.items.len()), (300.0, "DP-1", 2));
        assert_eq!(f.paths(), vec!["/home/ada/a.pdf".to_string()]);
        assert_eq!(Feed::parse(&json!({"to": "shelf"})), None);
        // (missing: closed, at the right, near the top)
        let f = Feed::parse(&json!({"state": {}})).unwrap();
        assert!(!f.opened && !f.left && f.at_y < 0.0);
    }

    #[test]
    fn rows_read_as_the_qml_ones() {
        assert_eq!(glyph_name(&file("/x/Report.PDF")), "file-pdf");
        assert_eq!(glyph_name(&file("/x/a.tar.gz")), "file-zip");
        assert_eq!(glyph_name(&file("/x/song.opus")), "file-audio");
        assert_eq!(glyph_name(&file("/x/clip.webm")), "file-video");
        assert_eq!(glyph_name(&file("/x/notes.md")), "file-text");
        assert_eq!(glyph_name(&file("/x/photo.png")), "file");
        assert_eq!(glyph_name(&text("hi")), "text-t");
        assert_eq!(glyph_name(&Item { kind: "folder".into(), path: "/x/dir/".into(), ..Default::default() }), "folder");
        assert_eq!(detail(&file("/home/ada/Documents/a.pdf"), "/home/ada"), "~/Documents");
        assert_eq!(detail(&file("/srv/a.pdf"), "/home/ada"), "/srv");
        assert_eq!(detail(&Item { path: "/home/ada/dir/".into(), kind: "folder".into(), ..Default::default() }, "/home/ada"), "~/dir");
        assert_eq!(detail(&text("hi"), "/home/ada"), "Text");
    }

    #[test]
    fn long_names_lose_their_middle() {
        assert_eq!(elide_middle("short.pdf", 20), "short.pdf");
        let e = elide_middle("a-very-long-file-name-indeed.pdf", 15);
        assert_eq!(e.chars().count(), 15);
        assert!(e.starts_with("a-very-") && e.ends_with("ed.pdf") && e.contains('…'));
        assert!(fits(162.0, 13.0) >= 20);
    }

    #[test]
    fn a_drag_out_carries_uris_and_text() {
        let m = mime_for(&[file("/a/x.pdf"), file("/a/y.png")]);
        assert_eq!(m[0], ("text/uri-list".into(), b"file:///a/x.pdf\r\nfile:///a/y.png\r\n".to_vec()));
        assert_eq!(m[1], ("text/plain".into(), b"/a/x.pdf\n/a/y.png".to_vec()));
        let m = mime_for(&[file("/a/x.pdf"), text("hello"), text("there")]);
        assert_eq!(m[1], ("text/plain".into(), b"hello\nthere".to_vec()));
        let m = mime_for(&[text("only")]);
        assert_eq!(m, vec![("text/plain".to_string(), b"only".to_vec())]);
        assert!(mime_for(&[]).is_empty());
    }

    #[test]
    fn drops_add_new_files_or_text() {
        let have = [file("/a/x.pdf")];
        assert!(!would_add(&have, &["file:///a/x.pdf".into()], ""));
        assert!(would_add(&have, &["file:///a/y.pdf".into()], ""));
        assert!(!would_add(&have, &["https://example.org".into()], ""));
        assert!(would_add(&have, &[], "some words"));
        assert!(!would_add(&have, &[], "  file:///a/z  "));
        assert!(!would_add(&have, &[], "   "));
    }

    #[test]
    fn the_card_sizes_and_places_as_the_qml() {
        assert_eq!(card_height(0, false), 28.0 + 8.0 + 132.0 + 24.0);
        assert_eq!(card_height(0, true), 28.0 + 8.0 + 160.0 + 24.0);
        assert_eq!(card_height(1, false), 28.0 + 8.0 + 44.0 + 24.0);
        assert_eq!(card_height(3, true), 28.0 + 8.0 + 52.0 + 8.0 + 3.0 * 52.0 + 24.0);
        assert_eq!(card_height(9, false), card_height(6, false));
        // near the top; centred on the drag; kept on the screen
        assert_eq!(card_top(-1.0, 192.0, 1080.0), 40.0);
        assert_eq!(card_top(500.0, 192.0, 1080.0), 500.0 - (192.0 + 48.0) / 2.0 + 24.0);
        assert_eq!(card_top(1070.0, 192.0, 1080.0), 1080.0 - 240.0 - 8.0 + 24.0);
        assert_eq!(card_top(10.0, 192.0, 1080.0), 40.0);
    }

    #[test]
    fn the_target_under_the_drag() {
        assert_eq!(target_at(100.0, 100.0), Some(Target::Keep));
        assert_eq!(target_at(400.0, 100.0), Some(Target::Send));
        assert_eq!(target_at(278.0, 100.0), None); // (between them)
        assert_eq!(target_at(100.0, 30.0), None); // (the header)
        assert!(in_zone(30.0, 10.0) && !in_zone(10.0, 10.0) && !in_zone(300.0, 190.0));
    }

    #[test]
    fn material_outlines() {
        let d = dashes(216.0, 160.0, 16.0, 1.5);
        assert!(d.starts_with("M "));
        // dashes 6 long, 4.5 apart, round a 214 × 158 outline
        let n = d.matches('M').count();
        assert!((60..75).contains(&n), "{n}");
        for v in d.split(' ').filter_map(|t| t.parse::<f64>().ok()) {
            assert!((0.0..=216.0).contains(&v));
        }
        let c = clover8();
        assert!(c.starts_with("M ") && c.ends_with('Z'));
    }
}

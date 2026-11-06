//! The drop shelf's pure logic (shell/services/ShelfLogic.js; the shelf
//! surface has its own copy of drop and mimeFor in surfaces/shelf/src/
//! logic.rs): what a drop adds, what a drag out carries, and whether the
//! pointer was shaken.

use serde_json::{json, Value};
use tessera_core::services::mpris::percent_decode;

/// A file's kind on the shelf, from its name
pub fn kind_of(path: &str) -> &'static str {
    let low = path.to_lowercase();
    let ext = low.rsplit_once('.').map(|(_, e)| e.to_string()).unwrap_or_default();
    if ["png", "jpg", "jpeg", "gif", "webp", "bmp", "svg", "avif", "heic", "tif", "tiff"].contains(&ext.as_str()) && !ext.contains('/') {
        return "image";
    }
    if path.ends_with('/') {
        "folder"
    } else {
        "file"
    }
}

/// The shelf after a drop: file URLs (once each), else the text. Returns the
/// items and how many came.
pub fn drop(items: &[Value], urls: &[String], text: &str) -> (Vec<Value>, usize) {
    let mut list = items.to_vec();
    let mut have: Vec<String> = list.iter().map(|i| i["uri"].as_str().unwrap_or("").to_string()).collect();
    let mut added = 0;
    for uri in urls {
        if !uri.starts_with("file://") || have.contains(uri) {
            continue;
        }
        let path = percent_decode(&uri[7..]);
        let name = path.trim_end_matches('/').rsplit('/').next().unwrap_or("").to_string();
        list.push(json!({"uri": uri, "path": path, "name": name, "kind": kind_of(&path)}));
        have.push(uri.clone());
        added += 1;
    }
    let t = text.trim();
    if added == 0 && !t.is_empty() && !t.starts_with("file://") {
        let name: String = t.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(80).collect();
        list.push(json!({"text": t, "name": name, "kind": "text"}));
        added += 1;
    }
    (list, added)
}

/// What a drag out of the shelf carries: the files' URIs, and text (the text
/// items, else the files' paths)
pub fn mime_for(list: &[Value]) -> Value {
    let uris: Vec<&str> = list.iter().filter_map(|i| i["uri"].as_str().filter(|u| !u.is_empty())).collect();
    let texts: Vec<&str> = list.iter().filter(|i| i["kind"] == "text").filter_map(|i| i["text"].as_str()).collect();
    let mut m = serde_json::Map::new();
    if !uris.is_empty() {
        m.insert("text/uri-list".into(), json!(format!("{}\r\n", uris.join("\r\n"))));
    }
    if !texts.is_empty() {
        m.insert("text/plain".into(), json!(texts.join("\n")));
    } else if !uris.is_empty() {
        let paths: Vec<&str> = list.iter().filter_map(|i| i["path"].as_str().filter(|p| !p.is_empty())).collect();
        m.insert("text/plain".into(), json!(paths.join("\n")));
    }
    Value::Object(m)
}

/// Shaken: four or more turns among the samples ((ms, x), the last 0.7 s),
/// each after at least `reach` px the other way
pub fn shaken(samples: &[(i64, f64)], reach: f64) -> bool {
    let mut turns = 0;
    let mut dir = 0;
    let mut from = samples.first().map(|s| s.1).unwrap_or(0.0);
    for i in 1..samples.len() {
        let dx = samples[i].1 - samples[i - 1].1;
        if dx == 0.0 {
            continue;
        }
        let d = if dx > 0.0 { 1 } else { -1 };
        if dir != 0 && d != dir {
            if (samples[i - 1].1 - from).abs() >= reach {
                turns += 1;
            }
            from = samples[i - 1].1;
        }
        dir = d;
    }
    turns >= 4
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_come_from_the_name() {
        assert_eq!(kind_of("/a/b/Photo.JPG"), "image");
        assert_eq!(kind_of("/a/b/"), "folder");
        assert_eq!(kind_of("/a/b/notes.txt"), "file");
    }

    #[test]
    fn a_drop_adds_each_file_once_else_the_text() {
        let (l, n) = drop(&[], &["file:///tmp/a%20b.png".into(), "file:///tmp/a%20b.png".into(), "https://x".into()], "ignored");
        assert_eq!(n, 1);
        assert_eq!(l[0]["path"], "/tmp/a b.png");
        assert_eq!(l[0]["name"], "a b.png");
        assert_eq!(l[0]["kind"], "image");
        let (l2, n2) = drop(&l, &["file:///tmp/a%20b.png".into()], "");
        assert_eq!((l2.len(), n2), (1, 0));
        let (t, n3) = drop(&[], &[], "  hello \n  world ");
        assert_eq!(n3, 1);
        assert_eq!(t[0]["kind"], "text");
        assert_eq!(t[0]["name"], "hello world");
        assert_eq!(drop(&[], &[], "file:///x").1, 0);
    }

    #[test]
    fn a_drag_out_carries_uris_and_text() {
        let (l, _) = drop(&[], &["file:///tmp/a.txt".into()], "");
        let m = mime_for(&l);
        assert_eq!(m["text/uri-list"], "file:///tmp/a.txt\r\n");
        assert_eq!(m["text/plain"], "/tmp/a.txt");
        let (t, _) = drop(&[], &[], "hi");
        assert_eq!(mime_for(&t)["text/plain"], "hi");
        assert!(mime_for(&t).get("text/uri-list").is_none());
    }

    #[test]
    fn shaking_needs_four_turns() {
        let zig: Vec<(i64, f64)> = [0.0, 60.0, 0.0, 60.0, 0.0, 60.0, 0.0].iter().enumerate().map(|(i, x)| (i as i64 * 50, *x)).collect();
        assert!(shaken(&zig, 40.0));
        let slow: Vec<(i64, f64)> = [0.0, 60.0, 0.0, 60.0].iter().enumerate().map(|(i, x)| (i as i64 * 50, *x)).collect();
        assert!(!shaken(&slow, 40.0));
        let tiny: Vec<(i64, f64)> = [0.0, 5.0, 0.0, 5.0, 0.0, 5.0, 0.0].iter().enumerate().map(|(i, x)| (i as i64 * 50, *x)).collect();
        assert!(!shaken(&tiny, 40.0));
    }
}

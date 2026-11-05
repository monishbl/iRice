//! The Desktop folder (DesktopIcons.qml): where it is, what's in it, each
//! file's icon name, and what the right-click menu does with a file.

use super::grid::Entry;
use std::path::PathBuf;

/// Your Desktop folder (xdg-user-dir DESKTOP, else ~/Desktop), made if it
/// doesn't exist yet
pub fn desktop_dir() -> PathBuf {
    let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
    let out = std::process::Command::new("xdg-user-dir").arg("DESKTOP").stderr(std::process::Stdio::null()).output().ok();
    let d = out.and_then(|o| String::from_utf8(o.stdout).ok()).map(|s| s.trim().trim_end_matches('/').to_string()).filter(|s| !s.is_empty() && PathBuf::from(s) != home);
    let d = d.map(PathBuf::from).unwrap_or_else(|| home.join("Desktop"));
    let _ = std::fs::create_dir_all(&d);
    d
}

/// What's in it (no hidden files), folders first, then by name
pub fn list(dir: &std::path::Path) -> Vec<Entry> {
    let mut v: Vec<Entry> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().into_owned();
                    if name.starts_with('.') {
                        return None;
                    }
                    let md = std::fs::metadata(e.path()).ok();
                    let modified = md.as_ref().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs() as i64);
                    Some(Entry { name, path: e.path().to_string_lossy().into_owned(), dir: md.is_some_and(|m| m.is_dir()), modified })
                })
                .collect()
        })
        .unwrap_or_default();
    super::grid::sort_entries(&mut v);
    v
}

/// A file's icon name (Search.iconForFile), or "folder"
pub fn icon_name(e: &Entry) -> &'static str {
    if e.dir {
        return "folder";
    }
    let x = e.name.rsplit('.').next().unwrap_or("").to_lowercase();
    match x.as_str() {
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "svg" => "image-x-generic",
        "mp4" | "mkv" | "webm" | "mov" => "video-x-generic",
        "mp3" | "flac" | "ogg" | "wav" | "m4a" => "audio-x-generic",
        "pdf" => "application-pdf",
        "zip" | "tar" | "gz" | "xz" | "7z" | "rar" => "package-x-generic",
        _ => "text-x-generic",
    }
}

/// An image the desktop shows as a thumbnail (not a stack, not a folder)
pub fn is_picture(e: &Entry) -> bool {
    !e.dir && super::grid::kind_of(e) == "Images" && !e.name.to_lowercase().ends_with(".svg")
}

/// Move to Trash, with the notification's Undo (as the QML: gio, then
/// notify-send with an action that restores it)
pub fn trash(path: &str) {
    let name = path.rsplit('/').next().unwrap_or(path).to_string();
    let script = "gio trash -- \"$1\" || exit; a=$(timeout 600 notify-send -a Files -i user-trash -t 8000 -A undo=Undo 'Moved to Trash' \"$2\"); [ \"$a\" = undo ] && gio trash --restore \"trash:///$(basename \"$1\")\"";
    let argv = vec!["sh".to_string(), "-c".into(), script.into(), "sh".into(), path.to_string(), name];
    std::thread::spawn(move || {
        let _ = std::process::Command::new("setsid").arg("-f").args(argv).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
    });
}

/// A thumbnail for a picture on the desktop, `px` device pixels at most
/// (decoded on the caller's thread)
pub fn thumbnail(path: &str, px: u32) -> Option<slint::SharedPixelBuffer<slint::Rgba8Pixel>> {
    let img = image::open(path).ok()?.thumbnail(px, px).to_rgba8();
    let (w, h) = img.dimensions();
    Some(slint::SharedPixelBuffer::clone_from_slice(img.as_raw(), w, h))
}

/// A picture filling `w` × `h` (cropped to cover, as PreserveAspectCrop),
/// for the Photos widget
pub fn cover(path: &str, w: u32, h: u32) -> Option<slint::SharedPixelBuffer<slint::Rgba8Pixel>> {
    let img = image::open(path).ok()?;
    let (iw, ih) = (img.width().max(1) as f32, img.height().max(1) as f32);
    let s = (w as f32 / iw).max(h as f32 / ih);
    let (cw, ch) = ((w as f32 / s).round().max(1.0) as u32, (h as f32 / s).round().max(1.0) as u32);
    let (x, y) = ((img.width().saturating_sub(cw)) / 2, (img.height().saturating_sub(ch)) / 2);
    let out = img.crop_imm(x, y, cw.min(img.width()), ch.min(img.height())).resize_exact(w, h, image::imageops::FilterType::Triangle).to_rgba8();
    Some(slint::SharedPixelBuffer::clone_from_slice(out.as_raw(), w, h))
}

/// Pictures in the Photos widget's folder
pub fn pictures(dir: &str) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.path().is_file())
                .map(|e| e.path().to_string_lossy().into_owned())
                .filter(|p| {
                    let l = p.to_lowercase();
                    [".jpg", ".jpeg", ".png", ".webp"].iter().any(|x| l.ends_with(x))
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(name: &str, dir: bool) -> Entry {
        Entry { name: name.into(), path: format!("/d/{name}"), dir, modified: 0 }
    }

    #[test]
    fn icons_for_files() {
        assert_eq!(icon_name(&e("x", true)), "folder");
        assert_eq!(icon_name(&e("a.JPG", false)), "image-x-generic");
        assert_eq!(icon_name(&e("a.pdf", false)), "application-pdf");
        assert_eq!(icon_name(&e("a.rs", false)), "text-x-generic");
        assert!(is_picture(&e("a.png", false)) && !is_picture(&e("a.svg", false)) && !is_picture(&e("a.png", true)));
    }

    #[test]
    fn lists_a_folder() {
        let d = std::env::temp_dir().join(format!("tessera-desk-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("zdir")).unwrap();
        std::fs::write(d.join("b.txt"), "").unwrap();
        std::fs::write(d.join("a.png"), "").unwrap();
        std::fs::write(d.join(".hidden"), "").unwrap();
        let l = list(&d);
        assert_eq!(l.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), vec!["zdir", "a.png", "b.txt"]);
        assert_eq!(pictures(&d.to_string_lossy()).len(), 1);
        let _ = std::fs::remove_dir_all(&d);
    }
}

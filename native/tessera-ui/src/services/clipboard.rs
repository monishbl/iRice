//! The clipboard history, as shell/services/Clipboard.qml: cliphist's list
//! (the session's `wl-paste --watch cliphist store` keeps it), the pins
//! (UiState's clipPins in state.json, the QML shell's file: read here, changed
//! through the shell, `shell::ask("clipboard.pin", …)`), when each entry was
//! copied (clip-times.json beside it, which the shell's Clipboard writes) and
//! previews of copied pictures (decoded into $XDG_RUNTIME_DIR/tessera-clip,
//! the same files the QML panel uses).
//!
//! The thread lists the history when asked (`refresh`) and when cliphist's
//! database is written (inotify), sends `History`, then decodes the pictures
//! it lists and sends `Previews` as they come.

use crate::Msg;
use slint::{Rgba8Pixel, SharedPixelBuffer};
use smithay_client_toolkit::reexports::calloop::channel::Sender;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

/// One history entry, as `cliphist list` shows it
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub id: String,
    /// cliphist's preview: whitespace collapsed, cut with "…"
    pub text: String,
    /// a copied picture: where its preview goes
    pub image: Option<PathBuf>,
}

/// A pinned item: text, or a picture kept in the state folder (`source`:
/// the history entry it came from)
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Pin {
    pub text: Option<String>,
    pub image: Option<String>,
    pub source: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct History {
    pub entries: Vec<Entry>,
    pub pins: Vec<Pin>,
    /// id -> when it was copied (ms; 0: from before the shell kept times)
    pub stamps: HashMap<String, i64>,
}

/// Decoded pictures, by path (the newest whole set)
#[derive(Clone, Default)]
pub struct Previews(pub HashMap<PathBuf, SharedPixelBuffer<Rgba8Pixel>>);

/// cliphist's database: CLIPHIST_DB_PATH, else its default
pub fn db_path() -> String {
    std::env::var("CLIPHIST_DB_PATH").unwrap_or_else(|_| format!("{}/.cache/cliphist/db", std::env::var("HOME").unwrap_or_default()))
}

pub fn preview_dir() -> PathBuf {
    PathBuf::from(std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into())).join("tessera-clip")
}

/// `cliphist list`'s lines: "id\ttext", newest first; a picture reads
/// "[[ binary data 12 KiB png 640x480 ]]"
pub fn parse_list(out: &str, dir: &Path) -> Vec<Entry> {
    out.lines()
        .filter_map(|l| {
            let (id, text) = l.split_once('\t')?;
            let image = text.strip_prefix("[[ binary data ").and_then(|rest| {
                rest.split_whitespace().find_map(|w| {
                    let w = w.to_lowercase();
                    let ext = match w.as_str() {
                        "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" => w,
                        _ => return None,
                    };
                    Some(dir.join(format!("{id}.{ext}")))
                })
            });
            Some(Entry { id: id.into(), text: text.into(), image })
        })
        .take(200)
        .collect()
}

/// UiState's clipPins, from state.json
pub fn parse_pins(state: &str) -> Vec<Pin> {
    let v: serde_json::Value = serde_json::from_str(state).unwrap_or_default();
    let s = |p: &serde_json::Value, k: &str| p.get(k).and_then(|x| x.as_str()).map(String::from);
    v["clipPins"].as_array().into_iter().flatten().map(|p| Pin { text: s(p, "text"), image: s(p, "image"), source: s(p, "source") }).filter(|p| p.text.is_some() || p.image.is_some()).collect()
}

/// cliphist's list shows a whitespace-collapsed preview
pub fn norm(t: &str) -> String {
    t.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Whether a history entry is pinned already (shown with the pins, not twice)
pub fn is_pinned(e: &Entry, pins: &[Pin]) -> bool {
    let t = norm(&e.text);
    let cut = t.ends_with('…');
    let head = t.trim_end_matches('…');
    pins.iter().any(|p| {
        if e.image.is_some() {
            p.source.as_deref() == Some(e.id.as_str())
        } else if let Some(pt) = &p.text {
            if cut {
                norm(pt).starts_with(head)
            } else {
                norm(pt) == t
            }
        } else {
            false
        }
    })
}

/// The heading an entry goes under: "Just now", "Earlier this hour",
/// "2 hours ago", "Yesterday", "Monday", "12 Oct" ("Earlier": no time)
pub fn section_for(ts: i64, now: i64) -> String {
    if ts <= 0 {
        return "Earlier".into();
    }
    let day = |ms: i64| crate::util::local_day(ms / 1000);
    let days = day(now) - day(ts);
    if days <= 0 {
        let mins = (now - ts) as f64 / 60000.0;
        if mins < 5.0 {
            return "Just now".into();
        }
        let hours = (mins / 60.0).floor() as i64;
        return match hours {
            0 => "Earlier this hour".into(),
            1 => "1 hour ago".into(),
            h => format!("{h} hours ago"),
        };
    }
    if days == 1 {
        return "Yesterday".into();
    }
    if days < 7 {
        return crate::util::strftime("%A", ts / 1000);
    }
    let same_year = crate::util::strftime("%Y", ts / 1000) == crate::util::strftime("%Y", now / 1000);
    let d = crate::util::strftime(if same_year { "%d %b" } else { "%d %b %Y" }, ts / 1000);
    d.strip_prefix('0').map(String::from).unwrap_or(d)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Filter {
    All,
    Text,
    Images,
}

/// A row of the panel: a pin or a history entry
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub pin: Option<Pin>,
    pub entry: Option<Entry>,
    pub text: String,
    /// the picture's file
    pub image: Option<PathBuf>,
    pub section: String,
}

impl Row {
    pub fn pinned(&self) -> bool {
        self.pin.is_some()
    }
}

/// The rows for a search and a filter, as Clipboard.filtered: the pins first
/// ("Pinned"), then the history by when it was copied. Pictures show only
/// without a search (there's no text to match)
pub fn filtered(h: &History, query: &str, kind: Filter, now: i64) -> Vec<Row> {
    let q = query.trim().to_lowercase();
    let matches = |t: &str| q.is_empty() || t.to_lowercase().contains(&q);
    let mut out: Vec<Row> = h
        .pins
        .iter()
        .filter(|p| match kind {
            Filter::Text => p.image.is_none(),
            Filter::Images => p.image.is_some(),
            Filter::All => true,
        })
        .filter(|p| if p.image.is_some() { q.is_empty() } else { matches(p.text.as_deref().unwrap_or("")) })
        .map(|p| Row { pin: Some(p.clone()), entry: None, text: p.text.clone().unwrap_or_default(), image: p.image.as_ref().map(PathBuf::from), section: "Pinned".into() })
        .collect();
    out.extend(
        h.entries
            .iter()
            .filter(|e| kind == Filter::All || (kind == Filter::Images) == e.image.is_some())
            .filter(|e| if e.image.is_some() { q.is_empty() } else { matches(&e.text) })
            .filter(|e| !is_pinned(e, &h.pins))
            .map(|e| Row { pin: None, entry: Some(e.clone()), text: e.text.clone(), image: e.image.clone(), section: section_for(h.stamps.get(&e.id).copied().unwrap_or(0), now) }),
    );
    out
}

/// "#a1b2c3": the row shows the colour
pub fn is_color(t: &str) -> bool {
    let t = t.trim();
    t.len() == 7 && t.starts_with('#') && t[1..].chars().all(|c| c.is_ascii_hexdigit())
}

pub fn is_link(t: &str) -> bool {
    t.starts_with("http://") || t.starts_with("https://")
}

// ── The thread ───────────────────────────────────────────────────────────

static ASK: std::sync::Mutex<Option<mpsc::Sender<()>>> = std::sync::Mutex::new(None);

/// List the history again (the panel opened; an entry was removed)
pub fn refresh() {
    if let Some(tx) = ASK.lock().ok().and_then(|g| g.clone()) {
        let _ = tx.send(());
    }
}

fn list(db: &str) -> Option<String> {
    let out = Command::new("cliphist").args(["-db-path", db, "list"]).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn read_stamps() -> HashMap<String, i64> {
    let text = std::fs::read_to_string(crate::util::state_dir().join("clip-times.json")).unwrap_or_default();
    let v: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
    v.as_object().into_iter().flatten().filter_map(|(k, t)| Some((k.clone(), t.as_f64()? as i64))).collect()
}

fn read_pins() -> Vec<Pin> {
    parse_pins(&std::fs::read_to_string(crate::util::state_dir().join("state.json")).unwrap_or_default())
}

/// A picture made small enough to show (the list's 152 px high, the grid's
/// tiles, at 2x)
pub fn thumbnail(bytes: &[u8]) -> Option<SharedPixelBuffer<Rgba8Pixel>> {
    let img = image::load_from_memory(bytes).ok()?;
    let img = if img.width() > 640 || img.height() > 400 { img.thumbnail(640, 400) } else { img };
    let rgba = img.to_rgba8();
    Some(SharedPixelBuffer::clone_from_slice(rgba.as_raw(), rgba.width(), rgba.height()))
}

/// The picture's file, decoded from cliphist when it isn't there yet (written
/// whole, then renamed: the QML panel shows only complete files)
fn decode(db: &str, id: &str, path: &Path) -> Option<Vec<u8>> {
    if let Ok(b) = std::fs::read(path) {
        if !b.is_empty() {
            return Some(b);
        }
    }
    if !id.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let out = Command::new("cliphist").args(["-db-path", db, "decode", id]).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    if !out.status.success() || out.stdout.is_empty() {
        return None;
    }
    if let Some(d) = path.parent() {
        let _ = std::fs::create_dir_all(d);
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(d, std::fs::Permissions::from_mode(0o700));
    }
    let _ = crate::util::write_atomic(path, &out.stdout);
    Some(out.stdout)
}

/// Wakes on writes to cliphist's database (its folder, for the file's name)
fn watch_db(db: &str, tx: mpsc::Sender<()>) {
    let path = PathBuf::from(db);
    let (Some(dir), Some(name)) = (path.parent().map(Path::to_owned), path.file_name().map(|n| n.as_encoded_bytes().to_vec())) else { return };
    std::thread::spawn(move || unsafe {
        let fd = libc::inotify_init1(libc::IN_CLOEXEC);
        let c = std::ffi::CString::new(dir.as_os_str().as_encoded_bytes()).unwrap();
        if fd < 0 || libc::inotify_add_watch(fd, c.as_ptr(), libc::IN_CLOSE_WRITE | libc::IN_MODIFY | libc::IN_MOVED_TO | libc::IN_CREATE) < 0 {
            return;
        }
        let mut buf = [0u8; 4096];
        loop {
            let n = libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len());
            if n <= 0 {
                return;
            }
            let mut off = 0usize;
            let mut hit = false;
            while off + std::mem::size_of::<libc::inotify_event>() <= n as usize {
                let ev = &*(buf.as_ptr().add(off) as *const libc::inotify_event);
                let raw = std::slice::from_raw_parts(buf.as_ptr().add(off + std::mem::size_of::<libc::inotify_event>()), ev.len as usize);
                let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
                hit |= raw[..end] == name[..];
                off += std::mem::size_of::<libc::inotify_event>() + ev.len as usize;
            }
            if hit && tx.send(()).is_err() {
                return;
            }
        }
    });
}

pub fn spawn(tx: Sender<Msg>) {
    let (ask, asked) = mpsc::channel::<()>();
    if let Ok(mut g) = ASK.lock() {
        *g = Some(ask.clone());
    }
    let db = db_path();
    watch_db(&db, ask);
    std::thread::spawn(move || {
        let dir = preview_dir();
        let mut seen: HashMap<String, i64> = HashMap::new(); // ids first seen here (no time from the shell yet)
        let mut thumbs: HashMap<PathBuf, SharedPixelBuffer<Rgba8Pixel>> = HashMap::new();
        let mut last: Option<Vec<Entry>> = None;
        let mut last_pins: Vec<Pin> = Vec::new();
        loop {
            let entries = parse_list(&list(&db).unwrap_or_default(), &dir);
            let pins = read_pins();
            let mut stamps = read_stamps();
            let now = crate::util::now_ms();
            for e in &entries {
                let t = *seen.entry(e.id.clone()).or_insert(if stamps.is_empty() { 0 } else { now });
                stamps.entry(e.id.clone()).or_insert(t);
            }
            let changed = last.as_ref() != Some(&entries) || last_pins != pins;
            last = Some(entries.clone());
            last_pins = pins.clone();
            let h = History { entries: entries.clone(), pins: pins.clone(), stamps };
            if tx.send(Msg::Data(Box::new(h))).is_err() {
                return;
            }
            if changed {
                // the pictures listed (and pinned), newest first; the rest forgotten
                let mut want: Vec<(Option<String>, PathBuf)> = entries.iter().filter_map(|e| Some((Some(e.id.clone()), e.image.clone()?))).collect();
                want.extend(pins.iter().filter_map(|p| Some((None, PathBuf::from(p.image.as_ref()?)))));
                thumbs.retain(|p, _| want.iter().any(|w| &w.1 == p));
                let mut fresh = 0;
                for (id, path) in want.iter().take(60) {
                    if thumbs.contains_key(path) {
                        continue;
                    }
                    let bytes = match id {
                        Some(id) => decode(&db, id, path),
                        None => std::fs::read(path).ok(),
                    };
                    if let Some(t) = bytes.as_deref().and_then(thumbnail) {
                        thumbs.insert(path.clone(), t);
                        fresh += 1;
                        if fresh % 4 == 0 && tx.send(Msg::Data(Box::new(Previews(thumbs.clone())))).is_err() {
                            return;
                        }
                    }
                }
                if tx.send(Msg::Data(Box::new(Previews(thumbs.clone())))).is_err() {
                    return;
                }
            }
            // the next ask (a copy lands as a burst of writes: settle first)
            if asked.recv().is_err() {
                return;
            }
            std::thread::sleep(Duration::from_millis(300));
            while asked.try_recv().is_ok() {}
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIST: &str = "12\thello   world\n11\t[[ binary data 31 KiB png 640x480 ]]\n10\thttps://example.org/a\n9\t[[ binary data 2 MiB JPEG 4000x3000 ]]\n8\t[[ binary data 1 KiB application/x-thing ]]\nbroken line\n7\t#1e6fd9\n";

    #[test]
    fn reads_cliphists_list() {
        let e = parse_list(LIST, Path::new("/run/clip"));
        assert_eq!(e.len(), 6);
        assert_eq!(e[0], Entry { id: "12".into(), text: "hello   world".into(), image: None });
        assert_eq!(e[1].image.as_deref(), Some(Path::new("/run/clip/11.png")));
        assert_eq!(e[3].image.as_deref(), Some(Path::new("/run/clip/9.jpeg")));
        assert_eq!(e[4].image, None); // (not a picture we show)
        assert_eq!(e[5].text, "#1e6fd9");
        let many: String = (0..300).map(|i| format!("{i}\tx\n")).collect();
        assert_eq!(parse_list(&many, Path::new("/")).len(), 200);
    }

    #[test]
    fn reads_the_pins_from_the_shells_state() {
        let p = parse_pins(r#"{"todos":[],"clipPins":[{"text":"a b"},{"image":"/s/clip-pins/1.png","source":"11"},{"nothing":1}]}"#);
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].text.as_deref(), Some("a b"));
        assert_eq!(p[1].source.as_deref(), Some("11"));
        assert!(parse_pins("").is_empty() && parse_pins(r#"{"clipPins":null}"#).is_empty());
    }

    #[test]
    fn pinned_entries_show_once() {
        let pins = vec![Pin { text: Some("hello\nworld".into()), ..Default::default() }, Pin { image: Some("/p.png".into()), source: Some("11".into()), text: None }, Pin { text: Some("a long text that cliphist cut".into()), ..Default::default() }];
        let e = parse_list(LIST, Path::new("/r"));
        assert!(is_pinned(&e[0], &pins)); // (whitespace doesn't count)
        assert!(is_pinned(&e[1], &pins)); // the picture it came from
        assert!(!is_pinned(&e[2], &pins));
        let cut = Entry { id: "1".into(), text: "a long text…".into(), image: None };
        assert!(is_pinned(&cut, &pins));
    }

    #[test]
    fn rows_for_a_search_and_a_filter() {
        let mut h = History { entries: parse_list(LIST, Path::new("/r")), ..Default::default() };
        h.pins = vec![Pin { text: Some("pinned words".into()), ..Default::default() }, Pin { image: Some("/p.png".into()), source: Some("99".into()), text: None }];
        let now = 1_700_000_000_000;
        h.stamps.insert("12".into(), now - 60_000);
        let all = filtered(&h, "", Filter::All, now);
        assert_eq!(all.len(), 8);
        assert_eq!((all[0].section.as_str(), all[1].section.as_str()), ("Pinned", "Pinned"));
        assert!(all[0].pinned() && !all[2].pinned());
        assert_eq!(all[2].section, "Just now");
        assert_eq!(all[3].section, "Earlier"); // (no time)
        let text = filtered(&h, "", Filter::Text, now);
        assert!(text.iter().all(|r| r.image.is_none()));
        assert_eq!(text.len(), 5);
        let images = filtered(&h, "", Filter::Images, now);
        assert_eq!(images.len(), 3);
        // a search: text only, any case
        let found = filtered(&h, " WORLD ", Filter::All, now);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].entry.as_ref().unwrap().id, "12");
        assert_eq!(filtered(&h, "words", Filter::All, now).len(), 1);
        assert!(filtered(&h, "zzz", Filter::Images, now).is_empty());
    }

    #[test]
    fn headings_by_when_it_was_copied() {
        let now = crate::util::local_noon_ms(2026, 11, 4); // a Wednesday, noon
        assert_eq!(section_for(0, now), "Earlier");
        assert_eq!(section_for(now - 2 * 60_000, now), "Just now");
        assert_eq!(section_for(now - 20 * 60_000, now), "Earlier this hour");
        assert_eq!(section_for(now - 70 * 60_000, now), "1 hour ago");
        assert_eq!(section_for(now - 3 * 3_600_000 - 1, now), "3 hours ago");
        assert_eq!(section_for(now - 86_400_000, now), "Yesterday");
        assert_eq!(section_for(now - 3 * 86_400_000, now), "Sunday");
        assert_eq!(section_for(crate::util::local_noon_ms(2026, 10, 12), now), "12 Oct");
        assert_eq!(section_for(crate::util::local_noon_ms(2025, 3, 2), now), "2 Mar 2025");
    }

    #[test]
    fn colours_and_links() {
        assert!(is_color(" #1E6fd9 ") && !is_color("#1e6fd") && !is_color("1e6fd9a"));
        assert!(is_link("https://a.b") && !is_link("see https://a.b"));
        assert_eq!(norm(" a \n\t b "), "a b");
    }

    #[test]
    fn pictures_become_small_previews() {
        let mut png = Vec::new();
        let img = image::RgbaImage::from_pixel(1600, 800, image::Rgba([10, 20, 30, 255]));
        image::DynamicImage::ImageRgba8(img).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        let t = thumbnail(&png).unwrap();
        assert_eq!((t.width(), t.height()), (640, 320));
        assert!(thumbnail(b"not a picture").is_none());
    }
}

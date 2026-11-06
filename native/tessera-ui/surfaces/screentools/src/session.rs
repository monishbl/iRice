//! The screenshot's editing session (ScreenshotEditor.qml): what the
//! thumbnail shows, whether the editor is open, the markup and its history,
//! the tool settings kept between screenshots, Delete with its Undo, and a
//! newer screenshot waiting for the unsaved-changes question. Pure: the part
//! acts on what these return (a file to put in the Trash, the editor to
//! open). Markup is kept in the image's native pixels.

use super::geometry::Rect;

/// The chart series from the design rules: one fixed set, painted into the
/// image whatever the look (ScreenshotEditor.qml's palette)
pub const PALETTE: [&str; 8] = ["#6D7CDF", "#D05F43", "#409D48", "#BB5FAA", "#BE7100", "#1D9999", "#9B6BCE", "#CE597A"];
pub const PALETTE_NAMES: [&str; 8] = ["Indigo", "Vermilion", "Green", "Orchid", "Amber", "Teal", "Violet", "Rose"];

#[derive(Clone, Debug, PartialEq)]
pub enum Ann {
    /// pen and highlighter
    Stroke { points: Vec<(f32, f32)>, color: String, width: f32, highlighter: bool },
    Arrow { x1: f32, y1: f32, x2: f32, y2: f32, color: String, width: f32 },
    Rect { r: Rect, color: String, width: f32 },
    Ellipse { r: Rect, color: String, width: f32 },
    Text { x: f32, y: f32, text: String, color: String, size: f32 },
    /// redaction: the screenshot under it in blocks `block` px wide
    Pixelate { r: Rect, block: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Thumb {
    #[default]
    Hidden,
    Shot,
    /// the Undo toast after Delete
    Deleted,
}

/// What opening the editor did
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Opened {
    /// open it now (on the session's path)
    Now,
    /// the open one has changes: it asks about them first
    AskFirst,
    /// nothing to open
    No,
}

pub const TOOLS: [(&str, &str, &str, &str); 9] = [
    ("crop", "crop", "Crop", "C"),
    ("pen", "pencil-simple", "Pen", "P"),
    ("highlighter", "highlighter", "Highlighter", "H"),
    ("arrow", "arrow-up-right", "Arrow", "A"),
    ("rect", "square", "Rectangle", "R"),
    ("ellipse", "circle", "Ellipse", "O"),
    ("text", "text-t", "Text", "T"),
    ("pixelate", "checkerboard", "Redact", "B"),
    ("livetext", "text-aa", "Select text", "S"),
];

#[derive(Clone, Debug)]
pub struct Session {
    pub path: String,
    pub screen: String,
    /// bumped when a file changes on disk (pictures load again)
    pub serial: u32,
    pub thumb: Thumb,
    pub thumb_file: String,
    pub editor_open: bool,
    pub items: Vec<Ann>,
    /// 0 × 0: the whole image
    pub crop: Rect,
    pub undo: Vec<(Vec<Ann>, Rect)>,
    pub redo: Vec<(Vec<Ann>, Rect)>,
    pub tool: &'static str,
    pub color: usize,
    /// 0 thin, 1 medium, 2 thick
    pub width: usize,
    /// deleted when the Undo toast times out
    pub pending_delete: String,
    /// opens once the unsaved-changes question is answered
    pub pending_open: String,
}

impl Default for Session {
    fn default() -> Self {
        Session {
            path: String::new(),
            screen: String::new(),
            serial: 0,
            thumb: Thumb::Hidden,
            thumb_file: String::new(),
            editor_open: false,
            items: Vec::new(),
            crop: Rect::default(),
            undo: Vec::new(),
            redo: Vec::new(),
            tool: "pen",
            color: 1,
            width: 1,
            pending_delete: String::new(),
            pending_open: String::new(),
        }
    }
}

impl Session {
    pub fn dirty(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn reset_edits(&mut self) {
        self.items.clear();
        self.crop = Rect::default();
        self.undo.clear();
        self.redo.clear();
    }

    // ── History ──────────────────────────────────────────────────────
    pub fn commit(&mut self, items: Vec<Ann>, crop: Option<Rect>) {
        self.undo.push((self.items.clone(), self.crop));
        self.redo.clear();
        self.items = items;
        if let Some(c) = crop {
            self.crop = c;
        }
    }
    pub fn add(&mut self, a: Ann) {
        let mut next = self.items.clone();
        next.push(a);
        self.commit(next, None);
    }
    pub fn set_crop(&mut self, r: Rect) {
        let items = self.items.clone();
        self.commit(items, Some(r));
    }
    pub fn undo(&mut self) -> bool {
        let Some((items, crop)) = self.undo.pop() else { return false };
        self.redo.push((std::mem::replace(&mut self.items, items), std::mem::replace(&mut self.crop, crop)));
        true
    }
    pub fn redo(&mut self) -> bool {
        let Some((items, crop)) = self.redo.pop() else { return false };
        self.undo.push((std::mem::replace(&mut self.items, items), std::mem::replace(&mut self.crop, crop)));
        true
    }

    // ── Flow ─────────────────────────────────────────────────────────
    /// A new screenshot was saved (and copied): its thumbnail floats in.
    /// Returns a file to put in the Trash (a deletion it confirmed)
    pub fn shot(&mut self, file: &str, screen: &str) -> Option<String> {
        let gone = self.finish_delete();
        if !self.editor_open {
            self.path = file.to_string();
            self.reset_edits();
        }
        self.screen = screen.to_string();
        self.thumb_file = file.to_string();
        self.serial += 1;
        self.thumb = Thumb::Shot;
        gone
    }

    /// Open the editor on `file` (or the thumbnail's, or the session's)
    pub fn open_editor(&mut self, file: &str) -> (Opened, Option<String>) {
        let f = [file, &self.thumb_file, &self.path].into_iter().find(|s| !s.is_empty()).map(str::to_string);
        let Some(f) = f else { return (Opened::No, None) };
        let gone = self.finish_delete();
        self.thumb = Thumb::Hidden;
        if self.editor_open && f != self.path && self.dirty() {
            self.pending_open = f;
            return (Opened::AskFirst, gone);
        }
        if f != self.path {
            self.path = f;
            self.reset_edits();
        }
        self.editor_open = true;
        (Opened::Now, gone)
    }

    /// Closed, changes discarded; true when a waiting screenshot opens next
    pub fn close_editor(&mut self) -> bool {
        self.editor_open = false;
        self.reset_edits();
        self.open_pending()
    }

    fn open_pending(&mut self) -> bool {
        let next = std::mem::take(&mut self.pending_open);
        if next.is_empty() {
            return false;
        }
        self.path = next;
        self.reset_edits();
        self.editor_open = true;
        true
    }

    /// Saved: the result floats in as a thumbnail (`thumbnails`: the
    /// setting); true when a waiting screenshot opens next instead
    pub fn saved(&mut self, file: &str, thumbnails: bool) -> bool {
        self.editor_open = false;
        self.reset_edits();
        self.path = file.to_string();
        self.serial += 1;
        if self.open_pending() {
            return true;
        }
        self.thumb_file = file.to_string();
        self.thumb = if thumbnails { Thumb::Shot } else { Thumb::Hidden };
        false
    }

    /// Delete acts at once; the file goes when the Undo toast times out
    pub fn delete_shot(&mut self) -> Option<String> {
        let gone = self.finish_delete();
        self.pending_delete = self.path.clone();
        self.thumb_file = self.path.clone();
        self.editor_open = false;
        self.pending_open.clear();
        self.thumb = Thumb::Deleted;
        gone
    }

    pub fn undo_delete(&mut self) {
        self.pending_delete.clear();
        self.thumb = Thumb::Hidden;
        self.editor_open = true; // (the session, edits included, is still here)
    }

    /// A deletion is final now: the file to put in the Trash
    pub fn finish_delete(&mut self) -> Option<String> {
        if self.pending_delete.is_empty() {
            return None;
        }
        let f = std::mem::take(&mut self.pending_delete);
        if self.path == f {
            self.path.clear();
            self.reset_edits();
        }
        if self.thumb == Thumb::Deleted {
            self.thumb = Thumb::Hidden;
        }
        Some(f)
    }

    /// The thumbnail finished leaving (timed out, swiped)
    pub fn thumb_gone(&mut self) -> Option<String> {
        let gone = if self.thumb == Thumb::Deleted { self.finish_delete() } else { None };
        self.thumb = Thumb::Hidden;
        gone
    }
}

/// Next to the original: "<name> (edited).png", then "(edited 2)", …
pub fn copy_name(path: &str, exists: impl Fn(&str) -> bool) -> String {
    let stem = match path.rfind('.') {
        Some(i) if !path[i..].contains('/') => &path[..i],
        _ => path,
    };
    let mut n = format!("{stem} (edited).png");
    let mut i = 2;
    while exists(&n) {
        n = format!("{stem} (edited {i}).png");
        i += 1;
    }
    n
}

pub fn base_name(p: &str) -> &str {
    p.rsplit('/').next().unwrap_or(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dot(x: f32) -> Ann {
        Ann::Stroke { points: vec![(x, 0.0)], color: "#000".into(), width: 1.0, highlighter: false }
    }

    #[test]
    fn history_undoes_and_redoes_marks_and_crops() {
        let mut s = Session::default();
        assert!(!s.dirty() && !s.undo());
        s.add(dot(1.0));
        s.add(dot(2.0));
        s.set_crop(Rect::new(1.0, 1.0, 5.0, 5.0));
        assert_eq!((s.items.len(), s.crop.w), (2, 5.0));
        assert!(s.undo());
        assert_eq!((s.items.len(), s.crop.w), (2, 0.0));
        assert!(s.undo());
        assert_eq!(s.items.len(), 1);
        assert!(s.redo());
        assert_eq!(s.items.len(), 2);
        // something new drops what could be redone
        s.add(dot(3.0));
        assert!(!s.redo());
        assert!(s.dirty());
    }

    #[test]
    fn a_screenshot_floats_in_and_opens_for_markup() {
        let mut s = Session::default();
        assert_eq!(s.open_editor("").0, Opened::No);
        s.shot("/a.png", "DP-1");
        assert_eq!((s.thumb, s.path.as_str(), s.serial), (Thumb::Shot, "/a.png", 1));
        assert_eq!(s.open_editor("").0, Opened::Now);
        assert!(s.editor_open && s.thumb == Thumb::Hidden);
        // a newer one while editing keeps the session; with changes, it asks first
        s.add(dot(1.0));
        s.shot("/b.png", "DP-1");
        assert_eq!((s.path.as_str(), s.thumb_file.as_str(), s.items.len()), ("/a.png", "/b.png", 1));
        assert_eq!(s.open_editor("/b.png").0, Opened::AskFirst);
        assert_eq!(s.pending_open, "/b.png");
        // discarding opens the waiting one
        assert!(s.close_editor());
        assert_eq!((s.path.as_str(), s.editor_open, s.items.len()), ("/b.png", true, 0));
    }

    #[test]
    fn delete_waits_for_the_undo_toast() {
        let mut s = Session::default();
        s.shot("/a.png", "");
        s.open_editor("");
        s.add(dot(1.0));
        assert_eq!(s.delete_shot(), None);
        assert_eq!((s.thumb, s.editor_open, s.pending_delete.as_str()), (Thumb::Deleted, false, "/a.png"));
        // Undo: back in the editor, the edits still there
        s.undo_delete();
        assert!(s.editor_open && s.items.len() == 1 && s.pending_delete.is_empty());
        s.delete_shot();
        // the toast times out: now it goes
        assert_eq!(s.thumb_gone(), Some("/a.png".into()));
        assert!(s.path.is_empty() && s.items.is_empty() && s.thumb == Thumb::Hidden);
        // a new screenshot makes a waiting deletion final
        s.shot("/b.png", "");
        s.open_editor("");
        s.delete_shot();
        assert_eq!(s.shot("/c.png", ""), Some("/b.png".into()));
    }

    #[test]
    fn saved_floats_the_result_in() {
        let mut s = Session::default();
        s.shot("/a.png", "");
        s.open_editor("");
        s.add(dot(1.0));
        assert!(!s.saved("/a (edited).png", true));
        assert_eq!((s.thumb, s.thumb_file.as_str(), s.dirty(), s.serial), (Thumb::Shot, "/a (edited).png", false, 2));
        assert!(!s.saved("/a.png", false));
        assert_eq!(s.thumb, Thumb::Hidden);
    }

    #[test]
    fn copies_get_free_names() {
        let taken = ["/s/Shot (edited).png", "/s/Shot (edited 2).png"];
        assert_eq!(copy_name("/s/Shot.png", |p| taken.contains(&p)), "/s/Shot (edited 3).png");
        assert_eq!(copy_name("/s.d/Shot", |_| false), "/s.d/Shot (edited).png");
        assert_eq!(base_name("/s/Shot.png"), "Shot.png");
    }
}

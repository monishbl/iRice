//! The desktop's grid: the widget catalogue (WidgetDefs.qml), where widgets
//! sit and snap (DesktopWidgets.qml), and where the files on the desktop sit
//! around them (DesktopIcons.qml). Pure functions: the part calls them with
//! the screen's size and config.json's lists.

use std::collections::{HashMap, HashSet};

/// A small widget is CELL × CELL, GAP apart
pub const CELL: f32 = 164.0;
pub const GAP: f32 = 16.0;
/// The grid's top left on screen (below the menu bar)
pub const ORIGIN: (f32, f32) = (32.0, 54.0);
/// Room left for the dock at the bottom
const DOCK_ROOM: f32 = 110.0;

pub struct KindDef {
    pub id: &'static str,
    pub label: &'static str,
    // (the gallery's list)
    pub icon: &'static str,
    pub sizes: &'static [&'static str],
    /// drawn edge to edge (the frame adds no padding)
    pub bleed: bool,
    /// each one keeps its own content (a note's text, a stack's widgets)
    pub own_id: bool,
}

const fn k(id: &'static str, label: &'static str, icon: &'static str, sizes: &'static [&'static str], bleed: bool, own_id: bool) -> KindDef {
    KindDef { id, label, icon, sizes, bleed, own_id }
}

/// WidgetDefs.qml's kinds, in its order
pub const KINDS: &[KindDef] = &[
    k("clock", "Clock", "clock", &["s", "m"], false, false),
    k("worldClock", "World Clock", "globe-hemisphere-west", &["s", "m"], false, false),
    k("weather", "Weather", "cloud-sun", &["s", "m"], false, false),
    k("today", "Today", "sun-horizon", &["m", "l"], false, false),
    k("calendar", "Calendar", "calendar-blank", &["s", "m", "l"], false, false),
    k("reminders", "Reminders", "list-checks", &["s", "m", "l"], false, false),
    k("batteries", "Batteries", "battery-high", &["s", "m"], false, false),
    k("nowPlaying", "Now Playing", "music-notes", &["s", "m"], false, false),
    k("system", "System", "cpu", &["s", "m", "l"], false, false),
    k("photos", "Photos", "image", &["s", "m", "l"], true, false),
    k("notes", "Note", "note-pencil", &["s", "m", "l"], false, true),
    k("stack", "Widget Stack", "stack", &["s", "m", "l"], false, true),
    k("shortcuts", "Shortcuts", "lightning", &["s", "m"], true, true),
    k("screenTime", "Screen Time", "hourglass-medium", &["s", "m"], false, false),
    k("batteryHistory", "Battery History", "battery-charging", &["s", "m"], false, false),
    k("stocks", "Stocks", "chart-line-up", &["s", "m", "l"], false, false),
];

pub fn kind(id: &str) -> Option<&'static KindDef> {
    KINDS.iter().find(|k| k.id == id)
}

pub fn size_name(size: &str) -> &'static str {
    match size {
        "m" => "Medium",
        "l" => "Large",
        _ => "Small",
    }
}

/// Grid cells a size takes (columns, rows)
pub fn units(size: &str) -> (i32, i32) {
    match size {
        "m" => (2, 1),
        "l" => (2, 2),
        _ => (1, 1),
    }
}

pub fn pixel_size(size: &str) -> (f32, f32) {
    let (c, r) = units(size);
    (c as f32 * CELL + (c - 1) as f32 * GAP, r as f32 * CELL + (r - 1) as f32 * GAP)
}

/// The Material look's card for a kind (WidgetDefs.materialCard): a Theme.m3
/// role ("" draws no card: the clock face, the weather circle and a small
/// photo are shapes of their own), fully round (`pill`), and its padding
#[derive(Clone, Debug, PartialEq)]
pub struct Card {
    pub bg: &'static str,
    pub pill: bool,
    pub pad: f32,
    pub pad_x: f32,
}

pub fn material_card(kind: &str, size: &str) -> Card {
    if size == "s" && matches!(kind, "clock" | "weather" | "photos") {
        return Card { bg: "", pill: false, pad: 2.0, pad_x: 2.0 };
    }
    if kind == "weather" {
        return Card { bg: "secondaryContainer", pill: true, pad: 16.0, pad_x: 36.0 };
    }
    let bg = match kind {
        "clock" | "today" => "primaryContainer",
        "notes" => "tertiaryContainer",
        "nowPlaying" => "secondaryContainer",
        "screenTime" => "surfaceContainerHigh",
        _ => "surfaceContainer",
    };
    Card { bg, pill: false, pad: 16.0, pad_x: 16.0 }
}

/// One widget as config.json keeps it: "kind:size:col:row[:id]"
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct W {
    pub kind: String,
    pub size: String,
    pub col: i32,
    pub row: i32,
    pub id: String,
}

pub fn parse(entry: &str) -> W {
    let mut f = entry.split(':');
    let kind = f.next().unwrap_or("").to_string();
    let size = f.next().unwrap_or("");
    let num = |s: Option<&str>| s.and_then(|s| s.trim().parse::<f64>().ok()).filter(|v| v.is_finite()).map_or(0, |v| v as i32);
    let col = num(f.next());
    let row = num(f.next());
    let id = f.next().unwrap_or("").to_string();
    W { kind, size: if matches!(size, "s" | "m" | "l") { size.to_string() } else { "s".into() }, col, row, id }
}

pub fn format(w: &W) -> String {
    let mut s = format!("{}:{}:{}:{}", w.kind, w.size, w.col, w.row);
    if !w.id.is_empty() {
        s += ":";
        s += &w.id;
    }
    s
}

/// config.json's `desktop.widgets`: the ones this build knows, and the
/// entries it doesn't (a newer build's kind), kept as they are when saving
pub fn read(entries: &[String]) -> (Vec<W>, Vec<String>) {
    let mut known = Vec::new();
    let mut unknown = Vec::new();
    for e in entries {
        let w = parse(e);
        if kind(&w.kind).is_some() {
            known.push(w);
        } else {
            unknown.push(e.clone());
        }
    }
    (known, unknown)
}

pub fn entries(list: &[W], unknown: &[String]) -> Vec<String> {
    list.iter().map(format).chain(unknown.iter().cloned()).collect()
}

/// The grid on a screen of this logical size
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grid {
    pub cols: i32,
    pub rows: i32,
}

impl Grid {
    pub fn new(width: f32, height: f32) -> Self {
        let step = CELL + GAP;
        Grid {
            cols: (((width - 2.0 * ORIGIN.0 + GAP) / step).floor() as i32).max(1),
            rows: (((height - ORIGIN.1 - DOCK_ROOM + GAP) / step).floor() as i32).max(1),
        }
    }

    pub fn fits(&self, list: &[W], c: i32, r: i32, size: &str, skip: Option<usize>) -> bool {
        let (uc, ur) = units(size);
        if c < 0 || r < 0 || c + uc > self.cols || r + ur > self.rows {
            return false;
        }
        list.iter().enumerate().all(|(i, o)| {
            if Some(i) == skip {
                return true;
            }
            let (oc, or) = units(&o.size);
            !(c < o.col + oc && o.col < c + uc && r < o.row + or && o.row < r + ur)
        })
    }

    /// The free cell closest to (c, r), or None if the grid is full (rows
    /// count a little more than columns, as the QML)
    pub fn nearest_free(&self, list: &[W], c: i32, r: i32, size: &str, skip: Option<usize>) -> Option<(i32, i32)> {
        let mut best = None;
        let mut best_d = 1e9f64;
        for cc in 0..self.cols {
            for rr in 0..self.rows {
                if self.fits(list, cc, rr, size, skip) {
                    let d = ((cc - c) * (cc - c)) as f64 + ((rr - r) * (rr - r)) as f64 * 1.1;
                    if d < best_d {
                        best_d = d;
                        best = Some((cc, rr));
                    }
                }
            }
        }
        best
    }

    /// Where each widget shows: its saved cell, or the nearest free one if
    /// that doesn't fit this screen (the saved layout isn't rewritten)
    pub fn placed(&self, list: &[W]) -> Vec<W> {
        let mut out: Vec<W> = Vec::with_capacity(list.len());
        for w in list {
            if self.fits(&out, w.col, w.row, &w.size, None) {
                out.push(w.clone());
                continue;
            }
            let mut w = w.clone();
            if let Some((c, r)) = self.nearest_free(&out, w.col, w.row, &w.size, None) {
                w.col = c;
                w.row = r;
            }
            out.push(w);
        }
        out
    }

    /// A widget's top left on screen
    pub fn home(col: i32, row: i32) -> (f32, f32) {
        (ORIGIN.0 + col as f32 * (CELL + GAP), ORIGIN.1 + row as f32 * (CELL + GAP))
    }

    /// The cell a dropped widget's top left is nearest
    pub fn cell_at(x: f32, y: f32) -> (i32, i32) {
        (((x - ORIGIN.0) / (CELL + GAP)).round() as i32, ((y - ORIGIN.1) / (CELL + GAP)).round() as i32)
    }

    /// The widget under a point, or None for the bare desktop
    #[allow(dead_code)] // (Slint's own hit testing does it on screen)
    pub fn widget_at(placed: &[W], x: f32, y: f32) -> Option<usize> {
        placed.iter().position(|w| {
            let (wx, wy) = Self::home(w.col, w.row);
            let (pw, ph) = pixel_size(&w.size);
            x >= wx && x < wx + pw && y >= wy && y < wy + ph
        })
    }

    /// Add a widget in the first free cell; None when there's no room
    pub fn add(&self, list: &[W], kind_id: &str, size: &str, id: &str) -> Option<Vec<W>> {
        let (c, r) = self.nearest_free(list, 0, 0, size, None)?;
        let mut l = list.to_vec();
        l.push(W { kind: kind_id.into(), size: size.into(), col: c, row: r, id: id.into() });
        Some(l)
    }

    pub fn resize(&self, list: &[W], i: usize, size: &str) -> Option<Vec<W>> {
        let w = list.get(i)?;
        let spot = if self.fits(list, w.col, w.row, size, Some(i)) { (w.col, w.row) } else { self.nearest_free(list, w.col, w.row, size, Some(i))? };
        let mut l = list.to_vec();
        l[i] = W { size: size.into(), col: spot.0, row: spot.1, ..w.clone() };
        Some(l)
    }

    /// Dropped with its top left at (x, y): the nearest free cell there
    /// (None: it snaps back)
    pub fn move_to(&self, list: &[W], i: usize, x: f32, y: f32) -> Option<Vec<W>> {
        let (c, r) = Self::cell_at(x, y);
        let w = list.get(i)?;
        let (nc, nr) = self.nearest_free(list, c, r, &w.size, Some(i))?;
        let mut l = list.to_vec();
        l[i].col = nc;
        l[i].row = nr;
        Some(l)
    }
}

pub fn remove(list: &[W], i: usize) -> Vec<W> {
    let mut l = list.to_vec();
    if i < l.len() {
        l.remove(i);
    }
    l
}

/// A new widget's own id (a note's, a stack's): six base-36 characters
pub fn new_id(seed: u64) -> String {
    let mut x = seed ^ 0x9E37_79B9_7F4A_7C15;
    let mut s = String::new();
    for _ in 0..6 {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        s.push(char::from_digit((x % 36) as u32, 36).unwrap_or('0'));
    }
    s
}

// ── Files on the desktop (DesktopIcons.qml) ─────────────────────────────────

/// An icon spot: half a widget cell
pub const SPOT: f32 = (CELL + GAP) / 2.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub name: String,
    pub path: String,
    pub dir: bool,
    /// seconds since the epoch
    pub modified: i64,
}

/// What a file is, for stacks
pub fn kind_of(e: &Entry) -> &'static str {
    if e.dir {
        return "Folders";
    }
    if e.name.len() >= 10 && e.name[..10].eq_ignore_ascii_case("screenshot") {
        return "Screenshots";
    }
    let x = e.name.rsplit('.').next().unwrap_or("").to_lowercase();
    match x.as_str() {
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "svg" | "heic" | "avif" | "bmp" => "Images",
        "mp4" | "mkv" | "webm" | "mov" | "avi" => "Movies",
        "mp3" | "flac" | "ogg" | "wav" | "m4a" | "opus" => "Music",
        "pdf" | "doc" | "docx" | "odt" | "txt" | "md" | "rtf" | "xls" | "xlsx" | "ods" | "csv" | "ppt" | "pptx" | "odp" | "epub" => "Documents",
        "zip" | "tar" | "gz" | "xz" | "7z" | "rar" | "zst" | "bz2" => "Archives",
        _ => "Other",
    }
}

const STACK_ORDER: &[&str] = &["Folders", "Documents", "Images", "Screenshots", "Movies", "Music", "Archives", "Other"];

/// One thing on the desktop: a file, or (Use Stacks) a stack of one kind
#[derive(Clone, Debug, PartialEq)]
pub struct IconItem {
    pub key: String,
    pub name: String,
    pub path: String,
    pub dir: bool,
    /// a stack's files, newest first (empty for a file)
    pub stack: Vec<Entry>,
}

impl IconItem {
    pub fn is_stack(&self) -> bool {
        self.key.starts_with("stack:")
    }
}

/// The folder's entries (folders first, then by name, as FolderListModel)
pub fn sort_entries(list: &mut [Entry]) {
    list.sort_by(|a, b| b.dir.cmp(&a.dir).then_with(|| a.name.cmp(&b.name)));
}

pub fn items(entries: &[Entry], use_stacks: bool) -> Vec<IconItem> {
    if !use_stacks {
        return entries.iter().map(|e| IconItem { key: e.path.clone(), name: e.name.clone(), path: e.path.clone(), dir: e.dir, stack: Vec::new() }).collect();
    }
    let mut groups: HashMap<&str, Vec<Entry>> = HashMap::new();
    for e in entries {
        groups.entry(kind_of(e)).or_default().push(e.clone());
    }
    STACK_ORDER
        .iter()
        .filter_map(|k| {
            let mut g = groups.remove(k)?;
            g.sort_by(|a, b| b.modified.cmp(&a.modified));
            Some(IconItem { key: format!("stack:{k}"), name: k.to_string(), path: String::new(), dir: false, stack: g })
        })
        .collect()
}

/// `desktop.iconPositions`: "key\tcol\trow" in icon spots
pub fn saved_positions(lines: &[String]) -> HashMap<String, (i32, i32)> {
    let mut m = HashMap::new();
    for l in lines {
        let mut f = l.split('\t');
        let (Some(k), Some(c), Some(r)) = (f.next(), f.next(), f.next()) else { continue };
        if let (Ok(c), Ok(r)) = (c.trim().parse::<f64>(), r.trim().parse::<f64>()) {
            m.insert(k.to_string(), (c as i32, r as i32));
        }
    }
    m
}

/// Icon spots under widgets
pub fn blocked(placed: &[W]) -> HashSet<(i32, i32)> {
    let mut b = HashSet::new();
    for w in placed {
        let (uc, ur) = units(&w.size);
        for c in w.col * 2..(w.col + uc) * 2 {
            for r in w.row * 2..(w.row + ur) * 2 {
                b.insert((c, r));
            }
        }
    }
    b
}

/// Where each icon sits: its saved spot if it's free, the rest from the
/// top right, down each column, then the next column left (as macOS)
pub fn place_icons(items: &[IconItem], saved: &HashMap<String, (i32, i32)>, blocked: &HashSet<(i32, i32)>, cols: i32, rows: i32) -> HashMap<String, (i32, i32)> {
    let mut taken = blocked.clone();
    let mut out = HashMap::new();
    for it in items {
        if let Some(&s) = saved.get(&it.key) {
            if s.0 >= 0 && s.1 >= 0 && s.0 < cols && s.1 < rows && !taken.contains(&s) {
                out.insert(it.key.clone(), s);
                taken.insert(s);
            }
        }
    }
    for it in items {
        if out.contains_key(&it.key) {
            continue;
        }
        'search: for c in (0..cols).rev() {
            for r in 0..rows {
                if !taken.contains(&(c, r)) {
                    out.insert(it.key.clone(), (c, r));
                    taken.insert((c, r));
                    break 'search;
                }
            }
        }
    }
    out
}

/// An icon dropped with its top left at (x, y): the new `iconPositions`
/// (None: nowhere free). Entries for files no longer there are dropped.
pub fn move_icon(key: &str, x: f32, y: f32, placed: &HashMap<String, (i32, i32)>, blocked: &HashSet<(i32, i32)>, cols: i32, rows: i32, lines: &[String], items: &[IconItem]) -> Option<Vec<String>> {
    let col = ((x - ORIGIN.0) / SPOT).round() as i32;
    let row = ((y - ORIGIN.1) / SPOT).round() as i32;
    let mut taken = blocked.clone();
    for (k, s) in placed {
        if k != key {
            taken.insert(*s);
        }
    }
    let mut best = None;
    let mut best_d = i64::MAX;
    for c in 0..cols {
        for r in 0..rows {
            if !taken.contains(&(c, r)) {
                let d = ((c - col) as i64).pow(2) + ((r - row) as i64).pow(2);
                if d < best_d {
                    best_d = d;
                    best = Some((c, r));
                }
            }
        }
    }
    let (c, r) = best?;
    let prefix = format!("{key}\t");
    let mut keep: Vec<String> = lines.iter().filter(|l| !l.starts_with(&prefix) && items.iter().any(|i| l.starts_with(&format!("{}\t", i.key)))).cloned().collect();
    keep.push(format!("{key}\t{c}\t{r}"));
    Some(keep)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ws(e: &[&str]) -> Vec<W> {
        e.iter().map(|s| parse(s)).collect()
    }

    #[test]
    fn entries_read_and_written_as_the_shell_does() {
        let w = parse("notes:l:2:1:ab12cd");
        assert_eq!(w, W { kind: "notes".into(), size: "l".into(), col: 2, row: 1, id: "ab12cd".into() });
        assert_eq!(format(&w), "notes:l:2:1:ab12cd");
        assert_eq!(format(&parse("clock:m:0:0")), "clock:m:0:0");
        // a bad size is small, bad numbers 0
        assert_eq!(parse("clock:x:a:"), W { kind: "clock".into(), size: "s".into(), col: 0, row: 0, id: String::new() });
        // kinds this build doesn't know are kept, after the known ones
        let (known, unknown) = read(&["clock:m:0:0".into(), "future:s:3:3".into(), "weather:s:1:1".into()]);
        assert_eq!(known.len(), 2);
        assert_eq!(entries(&known, &unknown), vec!["clock:m:0:0", "weather:s:1:1", "future:s:3:3"]);
    }

    #[test]
    fn the_grid_fits_the_screen() {
        // 1920 × 1080: (1920 - 64 + 16) / 180 = 10.4; (1080 - 54 - 110 + 16) / 180 = 5.18
        assert_eq!(Grid::new(1920.0, 1080.0), Grid { cols: 10, rows: 5 });
        assert_eq!(Grid::new(10.0, 10.0), Grid { cols: 1, rows: 1 });
        assert_eq!(pixel_size("s"), (164.0, 164.0));
        assert_eq!(pixel_size("m"), (344.0, 164.0));
        assert_eq!(pixel_size("l"), (344.0, 344.0));
        assert_eq!(Grid::home(1, 2), (32.0 + 180.0, 54.0 + 360.0));
    }

    #[test]
    fn widgets_snap_to_free_cells() {
        let g = Grid { cols: 4, rows: 3 };
        let list = ws(&["clock:m:0:0", "weather:s:0:1"]);
        assert!(!g.fits(&list, 1, 0, "s", None));
        assert!(g.fits(&list, 2, 0, "m", None));
        assert!(!g.fits(&list, 3, 0, "m", None)); // off the edge
        // (the moved one doesn't block itself)
        assert!(g.fits(&list, 1, 0, "s", Some(0)));
        // the free cell nearest the top left (rows count a little more)
        assert_eq!(g.nearest_free(&list, 0, 0, "s", None), Some((1, 1)));
        let added = g.add(&list, "notes", "s", "x").unwrap();
        assert_eq!(format(&added[2]), "notes:s:1:1:x");
        // dropped half a cell right of column 2 lands in column 2 (rounding)
        let (x, _) = Grid::home(2, 1);
        let moved = g.move_to(&list, 1, x + 60.0, Grid::home(2, 1).1).unwrap();
        assert_eq!((moved[1].col, moved[1].row), (2, 1));
        // full: nothing
        let full = ws(&["stack:l:0:0", "stack:l:2:0"]);
        assert_eq!(Grid { cols: 4, rows: 2 }.add(&full, "clock", "s", ""), None);
    }

    #[test]
    fn resize_keeps_the_cell_or_moves_aside() {
        let g = Grid { cols: 4, rows: 3 };
        let list = ws(&["clock:s:0:0", "weather:s:1:0"]);
        // medium from (0,0) would cover the weather: the nearest that fits
        let r = g.resize(&list, 0, "m").unwrap();
        assert_eq!(format(&r[0]), "clock:m:0:1");
        let r = g.resize(&list, 1, "m").unwrap();
        assert_eq!(format(&r[1]), "weather:m:1:0");
        assert_eq!(remove(&list, 0), ws(&["weather:s:1:0"]));
    }

    #[test]
    fn a_layout_from_a_bigger_screen_is_shown_inside_this_one() {
        let g = Grid { cols: 3, rows: 2 };
        let p = g.placed(&ws(&["clock:m:0:0", "weather:m:5:4"]));
        assert_eq!((p[1].col, p[1].row), (1, 1));
        assert_eq!(Grid::widget_at(&p, 40.0, 60.0), Some(0));
        assert_eq!(Grid::widget_at(&p, 40.0 + 345.0, 60.0), None);
    }

    #[test]
    fn material_cards_per_kind() {
        assert_eq!(material_card("clock", "s").bg, "");
        assert_eq!(material_card("clock", "m").bg, "primaryContainer");
        assert_eq!(material_card("weather", "m"), Card { bg: "secondaryContainer", pill: true, pad: 16.0, pad_x: 36.0 });
        assert_eq!(material_card("stocks", "l").bg, "surfaceContainer");
        assert_eq!(material_card("notes", "s").bg, "tertiaryContainer");
    }

    #[test]
    fn ids_are_six_characters() {
        let a = new_id(1);
        assert_eq!(a.len(), 6);
        assert_ne!(a, new_id(2));
        assert!(a.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    fn e(name: &str, dir: bool, modified: i64) -> Entry {
        Entry { name: name.into(), path: format!("/d/{name}"), dir, modified }
    }

    #[test]
    fn stacks_group_files_by_kind() {
        assert_eq!(kind_of(&e("Screenshot from 2026.png", false, 0)), "Screenshots");
        assert_eq!(kind_of(&e("a.PNG", false, 0)), "Images");
        assert_eq!(kind_of(&e("notes.md", false, 0)), "Documents");
        assert_eq!(kind_of(&e("x", true, 0)), "Folders");
        assert_eq!(kind_of(&e("run.sh", false, 0)), "Other");
        let list = vec![e("b.pdf", false, 1), e("a.png", false, 5), e("c.txt", false, 9), e("dir", true, 0)];
        let s = items(&list, true);
        assert_eq!(s.iter().map(|i| i.key.as_str()).collect::<Vec<_>>(), vec!["stack:Folders", "stack:Documents", "stack:Images"]);
        // newest first
        assert_eq!(s[1].stack[0].name, "c.txt");
        assert!(s[1].is_stack());
        let f = items(&list, false);
        assert_eq!((f.len(), f[0].key.as_str(), f[0].is_stack()), (4, "/d/b.pdf", false));
        let mut l = list.clone();
        sort_entries(&mut l);
        assert_eq!(l.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), vec!["dir", "a.png", "b.pdf", "c.txt"]);
    }

    #[test]
    fn icons_fill_from_the_top_right_around_widgets() {
        let widgets = ws(&["clock:m:0:0"]);
        let b = blocked(&widgets);
        assert_eq!(b.len(), 8); // 2 × 1 cells = 4 × 2 spots
        assert!(b.contains(&(3, 1)) && !b.contains(&(4, 0)));
        let list = vec![e("a", false, 0), e("b", false, 0), e("c", false, 0)];
        let it = items(&list, false);
        let saved = saved_positions(&["/d/c\t5\t0".into(), "bad".into()]);
        let p = place_icons(&it, &saved, &b, 6, 2);
        assert_eq!(p["/d/c"], (5, 0));
        assert_eq!(p["/d/a"], (5, 1));
        assert_eq!(p["/d/b"], (4, 0));
        // a saved spot under a widget isn't kept
        let p = place_icons(&it, &saved_positions(&["/d/a\t0\t0".into()]), &b, 6, 2);
        assert_eq!(p["/d/a"], (5, 0));
        // dropped near (4, 1): it goes there; positions for gone files go
        let (x, y) = (ORIGIN.0 + 4.0 * SPOT + 10.0, ORIGIN.1 + SPOT);
        let lines = move_icon("/d/a", x, y, &p, &b, 6, 2, &["/gone\t1\t1".into(), "/d/b\t4\t0".into()], &it).unwrap();
        assert_eq!(lines, vec!["/d/b\t4\t0".to_string(), "/d/a\t4\t1".to_string()]);
    }
}

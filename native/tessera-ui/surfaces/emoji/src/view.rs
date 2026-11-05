//! What the picker's grid shows, as PickerWindow.qml's `view`: sections (a
//! tab's list, or search results) laid out as rows (a header, a hint for an
//! empty section, or up to `cols` items), every item in order in `flat`, and
//! the keyboard's moves over them. Pure: tested without a display.

use super::data::{Data, Remembered};

pub const COLS: usize = 9;
pub const CELL: f32 = 44.0;
pub const GRID_H: f32 = 276.0;

/// Tabs: 0 frequently used, 1-8 emoji groups, 9 kaomoji, 10 symbols, 11 GIFs
/// (icon glyph, or text; its name)
pub const TABS: [(&str, &str, &str); 12] = [
    ("\u{e1a0}", "", "Frequently used"),
    ("\u{e436}", "", "Smileys & people"),
    ("\u{e648}", "", "Animals & nature"),
    ("\u{e790}", "", "Food & drink"),
    ("\u{e716}", "", "Activities"),
    ("\u{e112}", "", "Travel & places"),
    ("\u{e2dc}", "", "Objects"),
    ("\u{e2a8}", "", "Symbols"),
    ("\u{e244}", "", "Flags"),
    ("", "^_^", "Kaomoji"),
    ("\u{eab8}", "", "Text symbols"),
    ("\u{e274}", "", "GIFs"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Emoji,
    Kaomoji,
    Symbols,
    Gif,
}

pub fn mode(tab: usize) -> Mode {
    match tab {
        11 => Mode::Gif,
        10 => Mode::Symbols,
        9 => Mode::Kaomoji,
        _ => Mode::Emoji,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Emoji,
    Kaomoji,
    Symbol,
}

impl Kind {
    /// as recorded in emoji.json's usage
    pub fn word(self) -> &'static str {
        match self {
            Kind::Emoji => "emoji",
            Kind::Kaomoji => "kaomoji",
            Kind::Symbol => "symbol",
        }
    }
}

/// One item: an emoji (index into Data::emoji) or a kaomoji / symbol (into
/// its flat list)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub kind: Kind,
    pub i: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RowKind {
    Header(String),
    Hint(String),
    /// its items, from flat[start..]
    Items { cols: usize, start: usize, cells: Vec<Cell> },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub kind: RowKind,
    /// the tab its section belongs to (-1: none)
    pub tab: i32,
    pub y: f32,
    pub h: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct View {
    pub rows: Vec<Row>,
    pub flat: Vec<Cell>,
    /// each item's (row, column)
    pub pos: Vec<(usize, usize)>,
    /// each section's header row
    pub headers: Vec<usize>,
    pub height: f32,
}

struct Section {
    title: String,
    kind: Kind,
    cols: usize,
    tab: i32,
    items: Vec<Cell>,
    hint: Option<&'static str>,
}

impl View {
    /// The grid for a tab (and a search, when `query` isn't blank)
    pub fn build(d: &Data, r: &Remembered, tab: usize, query: &str, now: i64) -> Self {
        let m = mode(tab);
        if m == Mode::Gif {
            return Self::default();
        }
        let cells = |kind: Kind, list: &[usize]| list.iter().map(|&i| Cell { kind, i }).collect::<Vec<_>>();
        let mut sections = Vec::new();
        if !query.trim().is_empty() {
            let f = d.search(query, 120);
            let take = |v: &[usize], n: usize| v[..v.len().min(n)].to_vec();
            let mut parts = vec![
                ("emoji", Section { title: "Emoji".into(), kind: Kind::Emoji, cols: COLS, tab: -1, items: cells(Kind::Emoji, &take(&f.emoji, 90)), hint: None }),
                ("kaomoji", Section { title: "Kaomoji".into(), kind: Kind::Kaomoji, cols: 3, tab: -1, items: cells(Kind::Kaomoji, &take(&f.kaomoji, 30)), hint: None }),
                ("symbols", Section { title: "Text symbols".into(), kind: Kind::Symbol, cols: COLS, tab: -1, items: cells(Kind::Symbol, &take(&f.symbols, 45)), hint: None }),
            ];
            // the tab you're on first
            let order: [&str; 3] = match m {
                Mode::Kaomoji => ["kaomoji", "emoji", "symbols"],
                Mode::Symbols => ["symbols", "emoji", "kaomoji"],
                _ => ["emoji", "kaomoji", "symbols"],
            };
            for k in order {
                let i = parts.iter().position(|p| p.0 == k).unwrap();
                let (_, s) = parts.remove(i);
                if !s.items.is_empty() {
                    sections.push(s);
                }
            }
        } else if m == Mode::Emoji {
            let freq: Vec<usize> = r.frequent("emoji", 2 * COLS, now).iter().filter_map(|c| d.by_char.get(c).copied()).collect();
            sections.push(Section { title: "Frequently used".into(), kind: Kind::Emoji, cols: COLS, tab: 0, items: cells(Kind::Emoji, &freq), hint: Some("Emoji you use will show up here.") });
            for (gi, g) in d.groups.iter().enumerate() {
                sections.push(Section { title: g.1.clone(), kind: Kind::Emoji, cols: COLS, tab: gi as i32 + 1, items: cells(Kind::Emoji, &d.by_group[gi]), hint: None });
            }
        } else {
            let (kind, src, flat, cols, recent_n) = if m == Mode::Kaomoji { (Kind::Kaomoji, &d.kaomoji, &d.kaomoji_flat, 3, 6) } else { (Kind::Symbol, &d.symbols, &d.symbols_flat, COLS, COLS) };
            let recent: Vec<usize> = r.frequent(kind.word(), recent_n, now).iter().filter_map(|c| flat.iter().position(|x| &x.c == c)).collect();
            if !recent.is_empty() {
                sections.push(Section { title: "Recently used".into(), kind, cols, tab: -1, items: cells(kind, &recent), hint: None });
            }
            for (title, range) in src {
                sections.push(Section { title: title.clone(), kind, cols, tab: -1, items: cells(kind, &range.clone().collect::<Vec<_>>()), hint: None });
            }
        }
        let mut v = View::default();
        let mut y = 0.0;
        let mut push = |v: &mut View, kind: RowKind, tab: i32, h: f32| {
            v.rows.push(Row { kind, tab, y, h });
            y += h;
        };
        for s in sections {
            v.headers.push(v.rows.len());
            push(&mut v, RowKind::Header(s.title), s.tab, 28.0);
            if s.items.is_empty() {
                if let Some(h) = s.hint {
                    push(&mut v, RowKind::Hint(h.into()), s.tab, 40.0);
                }
            }
            for chunk in s.items.chunks(s.cols) {
                let r = v.rows.len();
                v.pos.extend((0..chunk.len()).map(|c| (r, c)));
                let start = v.flat.len();
                v.flat.extend_from_slice(chunk);
                push(&mut v, RowKind::Items { cols: s.cols, start, cells: chunk.to_vec() }, s.tab, if s.kind == Kind::Kaomoji { 40.0 } else { CELL });
            }
        }
        v.height = y;
        v
    }

    fn items_row(&self, r: usize) -> Option<(usize, usize, usize)> {
        match &self.rows.get(r)?.kind {
            RowKind::Items { cols, start, cells } => Some((*cols, *start, cells.len())),
            _ => None,
        }
    }

    /// Left / right: along the items in order
    pub fn move_h(&self, sel: usize, d: i32) -> usize {
        if self.flat.is_empty() {
            return 0;
        }
        (sel as i64 + d as i64).clamp(0, self.flat.len() as i64 - 1) as usize
    }

    /// Up / down: to the row above or below (past headers), the same column
    /// or the nearest one (kaomoji rows have 3); past the end, the first or
    /// last item
    pub fn move_v(&self, sel: usize, d: i32) -> usize {
        if self.flat.is_empty() {
            return 0;
        }
        let sel = sel.min(self.flat.len() - 1);
        let (row, col) = self.pos[sel];
        let from_cols = self.items_row(row).map_or(COLS, |x| x.0);
        let mut r = row as i64 + d as i64;
        while r >= 0 && (r as usize) < self.rows.len() && self.items_row(r as usize).is_none() {
            r += d as i64;
        }
        if r < 0 || r as usize >= self.rows.len() {
            return if d < 0 { 0 } else { self.flat.len() - 1 };
        }
        let (cols, start, n) = self.items_row(r as usize).unwrap();
        let col = if cols == from_cols { col } else { ((col * cols) as f32 / from_cols as f32).round() as usize };
        start + col.min(n - 1)
    }

    /// The scroll position that shows the selected item's row (and its
    /// section's title, on a section's first row): as ListView.Contain
    pub fn reveal(&self, sel: usize, scroll: f32, height: f32) -> f32 {
        let Some(&(r, _)) = self.pos.get(sel) else { return scroll };
        let contain = |s: f32, row: &Row| {
            if row.y < s {
                row.y
            } else if row.y + row.h > s + height {
                row.y + row.h - height
            } else {
                s
            }
        };
        let mut s = scroll;
        if r > 0 && matches!(self.rows[r - 1].kind, RowKind::Header(_)) {
            s = contain(s, &self.rows[r - 1]);
        }
        contain(s, &self.rows[r]).clamp(0.0, (self.height - height).max(0.0))
    }

    /// The tab of the section at the top of the list (scrolled to `scroll`)
    pub fn tab_at(&self, scroll: f32) -> i32 {
        let y = scroll + 8.0;
        self.rows.iter().rev().find(|r| r.y <= y).map_or(-1, |r| r.tab)
    }

    /// Where a tab's section starts (its header), and its first item
    pub fn section_of_tab(&self, tab: usize) -> Option<(f32, usize)> {
        let h = *self.headers.get(tab)?;
        let first = self.rows.iter().skip(h).find_map(|r| match &r.kind {
            RowKind::Items { start, .. } => Some(*start),
            _ => None,
        });
        Some((self.rows[h].y.min((self.height - GRID_H).max(0.0)), first.unwrap_or(0)))
    }
}

#[cfg(test)]
mod tests {
    use super::super::data::tests::sample;
    use super::*;

    fn now() -> i64 {
        1_000_000_000_000
    }

    #[test]
    fn the_emoji_tab_lists_every_group_after_frequently_used() {
        let d = sample();
        let mut r = Remembered::default();
        let v = View::build(&d, &r, 0, "", now());
        // nothing used yet: a hint under Frequently used
        assert_eq!(v.rows[0].kind, RowKind::Header("Frequently used".into()));
        assert_eq!(v.rows[1].kind, RowKind::Hint("Emoji you use will show up here.".into()));
        assert_eq!(v.headers.len(), 3);
        assert_eq!(v.flat.len(), d.emoji.len());
        assert_eq!(v.rows[v.headers[2]].tab, 2);
        r.record("😺", "emoji", now());
        let v = View::build(&d, &r, 0, "", now());
        assert_eq!(v.flat[0], Cell { kind: Kind::Emoji, i: 1 });
        assert_eq!(v.flat.len(), d.emoji.len() + 1);
        // heights add up (28 a header, 44 a row of emoji)
        assert_eq!(v.height, v.rows.iter().map(|r| r.h).sum::<f32>());
        assert_eq!(v.rows[1].y, 28.0);
    }

    #[test]
    fn kaomoji_go_three_a_row_with_recently_used_first() {
        let d = sample();
        let mut r = Remembered::default();
        r.record("(=^･ω･^=)", "kaomoji", now());
        let v = View::build(&d, &r, 9, "", now());
        assert_eq!(v.rows[0].kind, RowKind::Header("Recently used".into()));
        assert_eq!(v.flat[0], Cell { kind: Kind::Kaomoji, i: 2 });
        assert!(v.rows.iter().all(|r| match &r.kind {
            RowKind::Items { cols, .. } => *cols == 3 && r.h == 40.0,
            _ => true,
        }));
        assert!(View::build(&d, &r, 11, "", now()).rows.is_empty()); // (GIFs have their own grid)
    }

    #[test]
    fn search_results_put_the_current_tab_first() {
        let d = sample();
        let r = Remembered::default();
        let heads = |v: &View| v.headers.iter().map(|&h| v.rows[h].kind.clone()).collect::<Vec<_>>();
        let h = |s: &str| RowKind::Header(s.into());
        assert_eq!(heads(&View::build(&d, &r, 0, "cat", now())), vec![h("Emoji"), h("Kaomoji")]);
        assert_eq!(heads(&View::build(&d, &r, 9, "cat", now())), vec![h("Kaomoji"), h("Emoji")]);
        assert_eq!(heads(&View::build(&d, &r, 10, "heart", now())), vec![h("Text symbols"), h("Emoji")]);
        assert!(View::build(&d, &r, 0, "zzz", now()).flat.is_empty());
    }

    /// A view of n emoji in one section, then 4 kaomoji
    fn grid(n: usize) -> View {
        let mut s = String::from(r#"{"groups":[["a","A"]],"emoji":["#);
        s += &(0..n).map(|i| format!(r#"["e{i}","e {i}","","",0,0]"#)).collect::<Vec<_>>().join(",");
        s += r#"],"kaomoji":[["K",[["k0","x"],["k1","x"],["k2","x"],["k3","x"]]]],"symbols":[]}"#;
        let d = Data::parse(&s).unwrap();
        let mut r = Remembered::default();
        r.record("k0", "kaomoji", 0);
        // (search "e" for emoji first, then the kaomoji named x… use the tabs instead)
        let mut v = View::build(&d, &r, 1, "", 0);
        // drop Frequently used's header and hint, so the grid starts at the group
        v.rows.drain(..2);
        let shift = 68.0;
        for row in &mut v.rows {
            row.y -= shift;
        }
        v.pos.iter_mut().for_each(|p| p.0 -= 2);
        v.headers = vec![0];
        v.height -= shift;
        v
    }

    #[test]
    fn arrows_move_through_the_grid() {
        let v = grid(20); // rows of 9, 9, 2
        assert_eq!(v.move_h(0, -1), 0);
        assert_eq!(v.move_h(8, 1), 9); // (left and right wrap into the next row)
        assert_eq!(v.move_h(19, 1), 19);
        assert_eq!(v.move_v(3, 1), 12);
        assert_eq!(v.move_v(12, 1), 19); // the short last row: its last item
        assert_eq!(v.move_v(19, 1), 19); // past the end: the last
        assert_eq!(v.move_v(4, -1), 0); // past the start: the first
        assert_eq!(v.move_v(1, -1), 0);
    }

    #[test]
    fn up_and_down_between_nine_and_three_columns() {
        let d = sample();
        let r = Remembered::default();
        // search "happy": emoji (grinning face) then kaomoji (two, 3 a row)
        let v = View::build(&d, &r, 0, "happy", now());
        assert_eq!(v.flat.len(), 3);
        assert_eq!(v.flat[1].kind, Kind::Kaomoji);
        assert_eq!(v.move_v(0, 1), 1);
        assert_eq!(v.move_v(2, -1), 0);
    }

    #[test]
    fn reveal_scrolls_just_enough() {
        let v = grid(90); // 10 rows of 9 under a 28 px header: 468 px
        assert_eq!(v.height, 28.0 + 10.0 * 44.0);
        // in view: unchanged
        assert_eq!(v.reveal(9, 0.0, GRID_H), 0.0);
        // below: its bottom at the bottom
        assert_eq!(v.reveal(89, 0.0, GRID_H), 28.0 + 440.0 - GRID_H);
        // above: its top at the top, with the section's title on its first row
        assert_eq!(v.reveal(9, 100.0, GRID_H), 72.0);
        assert_eq!(v.reveal(0, 100.0, GRID_H), 0.0);
    }

    #[test]
    fn the_tab_follows_the_section_at_the_top() {
        let d = sample();
        let v = View::build(&d, &Remembered::default(), 0, "", now());
        assert_eq!(v.tab_at(0.0), 0);
        let (y, first) = v.section_of_tab(2).unwrap();
        assert_eq!(v.flat[first], Cell { kind: Kind::Emoji, i: 1 });
        assert_eq!(v.tab_at(y.max(v.rows[v.headers[2]].y)), 2);
        assert!(v.section_of_tab(9).is_none());
    }
}

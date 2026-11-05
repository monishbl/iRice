//! Where each result goes in the list, in each look (the QML lists'
//! delegates: SpotlightGlass.qml's Column of heading, AnswerCard and
//! ResultRow; SearchView.qml's groups of SearchRow under a SearchHero), and
//! how far the list scrolls to show the selection.

use super::rank::Row;

/// Glass: a first result of these kinds is a big card (AnswerCard)
pub const GLASS_BIG: [&str; 5] = ["Calculator", "Answer", "Conversion", "Dictionary", "Do"];
/// Material: a first result of these kinds is a hero card (SearchHero)
pub const HERO: [&str; 6] = ["Top Hit", "Calculator", "Answer", "Conversion", "Dictionary", "Do"];

/// A heading's line (Glass: Inter 11 px; Material: label large, 14 px)
pub const GLASS_HEAD_LINE: f32 = 14.0;
pub const MATERIAL_HEAD_LINE: f32 = 18.0;
/// The lists' most (then they scroll)
pub const GLASS_MAX: f32 = 440.0;
pub const MATERIAL_MAX: f32 = 480.0;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Entry {
    /// its top in the list, and its whole height (heading and gaps too)
    pub y: f32,
    pub h: f32,
    /// a heading above it ("" none), at `head_y` within the entry
    pub head: String,
    pub head_y: f32,
    /// the row (or card) within the entry
    pub row_y: f32,
    pub row_h: f32,
    /// a big card (Glass's AnswerCard, Material's hero)
    pub card: bool,
    /// Material's segmented groups: its corners
    pub first: bool,
    pub last: bool,
}

pub fn glass(rows: &[Row]) -> (Vec<Entry>, f32) {
    let mut out = Vec::new();
    let mut y = 0.0;
    for (i, r) in rows.iter().enumerate() {
        let mut e = Entry { y, ..Default::default() };
        if i == 0 && GLASS_BIG.contains(&r.section.as_str()) {
            e.card = true;
            e.row_h = 84.0;
            e.h = 84.0 + 6.0;
        } else {
            let mut top = 0.0;
            if i == 0 || rows[i - 1].section != r.section {
                let pad = if i == 0 { 2.0 } else { 10.0 };
                e.head = r.section.clone();
                e.head_y = pad;
                top = pad + GLASS_HEAD_LINE + 4.0;
            }
            e.row_y = top;
            e.row_h = 40.0;
            e.h = top + 40.0;
        }
        y += e.h;
        out.push(e);
    }
    (out, y)
}

pub fn material(rows: &[Row]) -> (Vec<Entry>, f32) {
    let mut out = Vec::new();
    let mut y = 0.0;
    for (i, r) in rows.iter().enumerate() {
        let prev = i.checked_sub(1).map(|p| &rows[p]);
        let next = rows.get(i + 1);
        let hero = i == 0 && HERO.contains(&r.section.as_str());
        let start = prev.is_none_or(|p| p.section != r.section) || (i == 1 && HERO.contains(&prev.unwrap().section.as_str()));
        let end = next.is_none_or(|n| n.section != r.section);
        let labelled = !hero && prev.is_none_or(|p| p.section != r.section);
        let mut e = Entry { y, card: hero, first: start, last: end, ..Default::default() };
        let mut top = 0.0;
        if start && i > 0 {
            top += 12.0;
        }
        if labelled {
            let pad = if i == 0 { 4.0 } else { 0.0 };
            e.head = if r.section == "Top Hit" { "Top hit".into() } else { r.section.clone() };
            e.head_y = top + pad;
            top += pad + MATERIAL_HEAD_LINE + 8.0;
        }
        e.row_y = top;
        e.row_h = if hero { 96.0 } else { 56.0 };
        e.h = top + e.row_h + if next.is_some() { 2.0 } else { 0.0 };
        y += e.h;
        out.push(e);
    }
    (out, y)
}

/// The scroll that shows entry `sel` whole, moving as little as it can
/// (ListView following its currentIndex)
pub fn reveal(entries: &[Entry], sel: usize, scroll: f32, view: f32) -> f32 {
    let Some(e) = entries.get(sel) else { return scroll };
    if e.y < scroll {
        e.y
    } else if e.y + e.h > scroll + view {
        (e.y + e.h - view).max(0.0)
    } else {
        scroll
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(s: &[&str]) -> Vec<Row> {
        s.iter().map(|s| Row { section: s.to_string(), title: "t".into(), ..Default::default() }).collect()
    }

    #[test]
    fn glass_headings_and_the_answer_card() {
        let (e, h) = glass(&rows(&["Calculator", "Applications", "Applications", "Web"]));
        assert!(e[0].card && e[0].head.is_empty());
        assert_eq!((e[0].h, e[1].y), (90.0, 90.0));
        // a heading over each section (10 above it after the first)
        assert_eq!((e[1].head.as_str(), e[1].head_y, e[1].row_y), ("Applications", 10.0, 28.0));
        assert_eq!((e[2].head.as_str(), e[2].h), ("", 40.0));
        assert_eq!(h, 90.0 + 68.0 + 40.0 + 68.0);
        // the top hit is a row in Glass, its heading 2 from the top
        let (e, _) = glass(&rows(&["Top Hit", "Applications"]));
        assert!(!e[0].card);
        assert_eq!((e[0].head.as_str(), e[0].head_y, e[0].row_y), ("Top Hit", 2.0, 20.0));
    }

    #[test]
    fn material_groups() {
        let (e, h) = material(&rows(&["Top Hit", "Applications", "Applications", "Actions", "Web"]));
        assert!(e[0].card && e[0].head.is_empty());
        assert_eq!(e[0].h, 98.0);
        // after the hero, a group of its own, labelled, 12 apart
        assert!(e[1].first && !e[1].last && e[2].last && !e[2].first);
        assert_eq!((e[1].head.as_str(), e[1].head_y, e[1].row_y), ("Applications", 12.0, 38.0));
        assert_eq!(e[2].h, 58.0);
        assert!(e[3].first && e[3].last);
        assert_eq!(e[4].h, 12.0 + 26.0 + 56.0);
        assert_eq!(h, e[4].y + e[4].h);
        let (e, _) = material(&rows(&["Top Hit", "Top Hit"]));
        // (a row after the hero starts its own group even in the same section)
        assert!(e[1].first);
        let (e, _) = material(&rows(&["Applications"]));
        assert_eq!((e[0].head.as_str(), e[0].head_y, e[0].row_y, e[0].h), ("Applications", 4.0, 30.0, 86.0));
        let (e, _) = material(&rows(&["Top Hit", "Web"]));
        assert_eq!(material(&rows(&["Top Hit"])).0[0].head, "");
        assert_eq!(e[1].head, "Web");
    }

    #[test]
    fn the_selection_is_scrolled_into_view() {
        let (e, _) = glass(&rows(&["Applications"; 20]));
        assert_eq!(reveal(&e, 0, 0.0, 440.0), 0.0);
        let s = reveal(&e, 15, 0.0, 440.0);
        assert_eq!(s, e[15].y + 40.0 - 440.0);
        assert_eq!(reveal(&e, 2, s, 440.0), e[2].y);
        assert_eq!(reveal(&e, 99, 12.0, 440.0), 12.0);
    }
}

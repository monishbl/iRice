//! Lists that change in place.
//!
//! A Slint `for` over a model makes one element per row and keeps it while
//! the model keeps the row. Handing a component a new model
//! (`set_x(ModelRc::new(VecModel::from(v)))`) drops every element and makes
//! them all again: pictures load again, animations start over, hovers and
//! presses are lost, and the whole list is drawn anew. The QML's Repeaters
//! over a ScriptModel keep their delegates. So a list a part refreshes is one
//! `Rows`, handed to the component once; each refresh only changes the rows
//! that differ, adds the new ones at the end and removes the extra ones.
//!
//!   let rows = Rows::new();              // kept in the part, beside its window
//!   ui.set_items(rows.model());          // once, when the window is made
//!   rows.set(new_items);                 // every refresh after
//!
//! `set_keyed` matches rows by a key instead of by position (a list whose
//! rows move: notifications arriving at the top, windows reordering), so a
//! row that only moved is moved, not rebuilt.

use slint::{Model, ModelRc, VecModel};
use std::rc::Rc;

pub struct Rows<T: Clone + PartialEq + 'static> {
    model: Rc<VecModel<T>>,
}

impl<T: Clone + PartialEq + 'static> Default for Rows<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone + PartialEq + 'static> Clone for Rows<T> {
    fn clone(&self) -> Self {
        Self { model: self.model.clone() }
    }
}

impl<T: Clone + PartialEq + 'static> Rows<T> {
    pub fn new() -> Self {
        Self { model: Rc::new(VecModel::default()) }
    }

    /// The model to hand the component (once)
    pub fn model(&self) -> ModelRc<T> {
        ModelRc::from(self.model.clone())
    }

    pub fn len(&self) -> usize {
        self.model.row_count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn get(&self, i: usize) -> Option<T> {
        self.model.row_data(i)
    }

    /// The rows now, by position: only those that differ change
    pub fn set(&self, new: Vec<T>) -> bool {
        let m = &self.model;
        let mut changed = false;
        let old = m.row_count();
        for (i, v) in new.iter().enumerate() {
            if i < old {
                if m.row_data(i).as_ref() != Some(v) {
                    m.set_row_data(i, v.clone());
                    changed = true;
                }
            } else {
                m.push(v.clone());
                changed = true;
            }
        }
        while m.row_count() > new.len() {
            m.remove(m.row_count() - 1);
            changed = true;
        }
        changed
    }

    /// The rows now, matched by `key`: a row whose key is still there keeps its
    /// element (moved if it moved, its data updated if it changed); rows whose
    /// key went are removed, new keys inserted where they belong
    pub fn set_keyed<K: PartialEq>(&self, new: Vec<T>, key: impl Fn(&T) -> K) -> bool {
        let m = &self.model;
        let mut changed = false;
        // rows whose key isn't wanted any more
        let mut i = 0;
        while i < m.row_count() {
            let k = key(&m.row_data(i).unwrap());
            if new.iter().any(|v| key(v) == k) {
                i += 1;
            } else {
                m.remove(i);
                changed = true;
            }
        }
        for (want, v) in new.iter().enumerate() {
            let k = key(v);
            let at = (want..m.row_count()).find(|&j| key(&m.row_data(j).unwrap()) == k);
            match at {
                Some(j) if j == want => {
                    if m.row_data(j).as_ref() != Some(v) {
                        m.set_row_data(j, v.clone());
                        changed = true;
                    }
                }
                Some(j) => {
                    // moved up to here: taken out and put back (Slint has no move)
                    m.remove(j);
                    m.insert(want, v.clone());
                    changed = true;
                }
                None => {
                    m.insert(want, v.clone());
                    changed = true;
                }
            }
        }
        while m.row_count() > new.len() {
            m.remove(m.row_count() - 1);
            changed = true;
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all(r: &Rows<(u32, &'static str)>) -> Vec<(u32, &'static str)> {
        (0..r.len()).map(|i| r.get(i).unwrap()).collect()
    }

    #[test]
    fn by_position_changes_only_what_differs() {
        let r = Rows::new();
        assert!(r.set(vec![(1, "a"), (2, "b")]));
        assert!(!r.set(vec![(1, "a"), (2, "b")]));
        assert!(r.set(vec![(1, "a"), (2, "B"), (3, "c")]));
        assert_eq!(all(&r), vec![(1, "a"), (2, "B"), (3, "c")]);
        assert!(r.set(vec![(1, "a")]));
        assert_eq!(all(&r), vec![(1, "a")]);
    }

    #[test]
    fn by_key_keeps_moves_and_updates() {
        let r = Rows::new();
        let k = |v: &(u32, &'static str)| v.0;
        r.set_keyed(vec![(1, "a"), (2, "b"), (3, "c")], k);
        // a new one on top, one gone, one changed
        assert!(r.set_keyed(vec![(4, "d"), (1, "a"), (3, "C")], k));
        assert_eq!(all(&r), vec![(4, "d"), (1, "a"), (3, "C")]);
        // reordered
        r.set_keyed(vec![(3, "C"), (4, "d"), (1, "a")], k);
        assert_eq!(all(&r), vec![(3, "C"), (4, "d"), (1, "a")]);
        assert!(!r.set_keyed(vec![(3, "C"), (4, "d"), (1, "a")], k));
    }
}

//! What keeps the sliders and pages live while you use them (shared with
//! the status menus, which draw the Control Center's blocks):
//!
//! - `Held`: a level you just set is shown at once and kept for a moment,
//!   so the sound server's answers (each a step behind the pointer during a
//!   drag) don't pull the slider back under it. QML's Audio and Brightness
//!   take the new level at once the same way (Quickshell's PipeWire node,
//!   Brightness.backlightValue).
//! - `set_blocks`: a page's rows changed in place, matched by their id, so a
//!   row being dragged keeps its slider (a model handed over anew made every
//!   row again, and the drag ended after one step).
//! - pictures decoded once (a device's drawing is an SVG: rendered again on
//!   every sound reading before).

use crate::ui;
use slint::{Model, ModelRc};
use std::cell::RefCell;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tessera_core::rows::Rows;

/// How long a level set here wins over the readings that follow
pub const HOLD: Duration = Duration::from_millis(600);

#[derive(Default)]
pub struct Held(HashMap<String, (f32, Instant)>);

impl Held {
    /// You set `id` to `v` now
    pub fn set(&mut self, id: &str, v: f32) {
        self.0.insert(id.to_string(), (v.clamp(0.0, 1.0), Instant::now()));
    }
    /// The level to show for `id` while it's held
    pub fn get(&self, id: &str) -> Option<f32> {
        self.0.get(id).filter(|(_, t)| t.elapsed() < HOLD).map(|(v, _)| *v)
    }
    /// `reading`, unless `id` was just set here
    pub fn or(&self, id: &str, reading: f32) -> f32 {
        self.get(id).unwrap_or(reading)
    }
    /// Forget the held levels that are over
    pub fn sweep(&mut self) {
        self.0.retain(|_, (_, t)| t.elapsed() < HOLD);
    }
}

/// A block's identity in its page: its id, else its kind and title (heads, notes)
pub fn block_key(b: &ui::CcBlock) -> String {
    key_of(b.kind, &b.id, &b.title)
}

/// (the same for any component's copy of the block struct: the status menus')
pub fn key_of(kind: i32, id: &str, title: &str) -> String {
    if id.is_empty() {
        format!("{kind}:{title}")
    } else {
        format!("{kind}:{id}")
    }
}

/// A page's blocks into `rows`, in place: rows matched by `block_key`; a
/// row's own choices (a segmented control) keep their model when their text
/// is the same, so an unchanged row compares equal and isn't touched
pub fn set_blocks(rows: &Rows<ui::CcBlock>, new: Vec<ui::CcBlock>) {
    set_blocks_of(rows, new, block_key, |b| b.options.clone(), |b, o| b.options = o);
}

/// `set_blocks` for any copy of the block struct: its key, and its choices' model
pub fn set_blocks_of<T: Clone + PartialEq + 'static>(
    rows: &Rows<T>,
    mut new: Vec<T>,
    key: impl Fn(&T) -> String,
    options: impl Fn(&T) -> ModelRc<slint::SharedString>,
    set_options: impl Fn(&mut T, ModelRc<slint::SharedString>),
) {
    let old: HashMap<String, ModelRc<slint::SharedString>> = (0..rows.len()).filter_map(|i| rows.get(i)).map(|b| (key(&b), options(&b))).collect();
    for b in &mut new {
        if let Some(o) = old.get(&key(b)) {
            let n = options(b);
            let same = o.row_count() == n.row_count() && (0..o.row_count()).all(|i| o.row_data(i) == n.row_data(i));
            if same {
                set_options(b, o.clone());
            }
        }
    }
    rows.set_keyed(new, key);
}

thread_local! {
    static PICTURES: RefCell<HashMap<String, Option<slint::Image>>> = RefCell::new(HashMap::new());
}

/// A picture decoded already (None: not yet; Some(None): no picture there)
pub fn cached_hit(path: &str) -> Option<Option<slint::Image>> {
    if path.is_empty() {
        return Some(None);
    }
    PICTURES.with(|p| p.borrow().get(path).cloned())
}

/// A picture file as an image, decoded once per path (device drawings,
/// ~/.face); `decode` makes it the first time
pub fn cached(path: &str, decode: impl FnOnce() -> Option<slint::Image>) -> Option<slint::Image> {
    if path.is_empty() {
        return None;
    }
    if let Some(hit) = PICTURES.with(|p| p.borrow().get(path).cloned()) {
        return hit;
    }
    let img = decode();
    PICTURES.with(|p| {
        let mut p = p.borrow_mut();
        // (a handful of drawings and a face; a runaway list of album art isn't kept)
        if p.len() > 64 {
            p.clear();
        }
        p.insert(path.to_string(), img.clone());
    });
    img
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_set_level_wins_for_a_moment() {
        let mut h = Held::default();
        assert_eq!(h.or("sound", 0.3), 0.3);
        h.set("sound", 0.7);
        assert_eq!(h.or("sound", 0.3), 0.7);
        assert_eq!(h.or("display", 0.4), 0.4);
        h.0.insert("display".into(), (0.9, Instant::now() - HOLD - Duration::from_millis(1)));
        assert_eq!(h.or("display", 0.4), 0.4);
        h.sweep();
        assert!(h.get("display").is_none() && h.get("sound").is_some());
    }

    #[test]
    fn blocks_keep_their_rows() {
        let b = |id: &str, v: f32| ui::CcBlock { kind: 4, id: id.into(), value: v, ..Default::default() };
        let rows = Rows::new();
        set_blocks(&rows, vec![b("mic", 0.2), b("app:3", 0.5)]);
        assert_eq!(rows.len(), 2);
        // an app starts playing above: the others are moved, not remade
        set_blocks(&rows, vec![b("app:9", 0.1), b("mic", 0.25), b("app:3", 0.5)]);
        let ids: Vec<String> = (0..rows.len()).map(|i| rows.get(i).unwrap().id.to_string()).collect();
        assert_eq!(ids, vec!["app:9", "mic", "app:3"]);
        assert_eq!(rows.get(1).unwrap().value, 0.25);
    }
}

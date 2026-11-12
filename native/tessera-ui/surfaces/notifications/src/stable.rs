//! Small lists inside a card or a day (a card's buttons, a day's colours and
//! lines) kept as the same model while their content is the same: a new
//! `ModelRc` makes the row it sits in compare different on every refresh,
//! so every card was set again (its buttons made anew, their hover and
//! press lost) whenever anything ticked.

use slint::{ModelRc, VecModel};
use std::cell::RefCell;
use std::collections::HashMap;

pub struct Stable<T: Clone + PartialEq + 'static>(RefCell<HashMap<String, (Vec<T>, ModelRc<T>)>>);

impl<T: Clone + PartialEq + 'static> Default for Stable<T> {
    fn default() -> Self {
        Self(RefCell::new(HashMap::new()))
    }
}

impl<T: Clone + PartialEq + 'static> Stable<T> {
    /// The model for `key` holding `v`: the one handed out last time when
    /// `v` is the same
    pub fn get(&self, key: &str, v: Vec<T>) -> ModelRc<T> {
        let mut m = self.0.borrow_mut();
        if let Some((old, model)) = m.get(key) {
            if *old == v {
                return model.clone();
            }
        }
        // (a long-running shell: forget them all now and then)
        if m.len() > 512 {
            m.clear();
        }
        let model = ModelRc::new(VecModel::from(v.clone()));
        m.insert(key.to_string(), (v, model.clone()));
        model
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_content_is_the_same_model() {
        let s = Stable::<i32>::default();
        let a = s.get("x", vec![1, 2]);
        assert!(a == s.get("x", vec![1, 2]));
        assert!(a != s.get("x", vec![1, 3]));
    }
}

//! Phosphor icon names to their glyphs (shell/common/phosphor.js, read at
//! build time): the widgets show icons the shell names (the weather's,
//! Spotlight's actions', a device's).

use std::collections::HashMap;
use std::sync::OnceLock;

const JS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../../shell/common/phosphor.js"));

fn map() -> &'static HashMap<&'static str, String> {
    static M: OnceLock<HashMap<&'static str, String>> = OnceLock::new();
    M.get_or_init(|| {
        let mut m = HashMap::new();
        for l in JS.lines() {
            // "    \"acorn\": \"\","
            let l = l.trim();
            let Some(rest) = l.strip_prefix('"') else { continue };
            let Some((name, v)) = rest.split_once("\": \"") else { continue };
            let Some(hex) = v.strip_prefix("\\u").map(|h| h.trim_end_matches(['"', ','])) else { continue };
            if let Some(c) = u32::from_str_radix(hex, 16).ok().and_then(char::from_u32) {
                m.insert(name, c.to_string());
            }
        }
        m
    })
}

/// The glyph for a name ("" for none; an unknown name: a question mark)
pub fn glyph(name: &str) -> String {
    if name.is_empty() {
        return String::new();
    }
    map().get(name).cloned().or_else(|| map().get("question").cloned()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_to_glyphs() {
        assert_eq!(glyph("acorn"), "\u{eb9a}");
        assert_eq!(glyph("clock"), "\u{e19a}");
        assert_eq!(glyph(""), "");
        assert!(!glyph("no-such-icon").is_empty());
        assert!(map().len() > 1000);
    }
}

//! Phosphor's glyphs by name, as the QML shell names them (shell/common/
//! phosphor.js, compiled in): the island's alerts, Focus modes, the sound
//! item and the tray's fallbacks arrive as names ("music-notes", "moon").

use std::collections::HashMap;
use std::sync::OnceLock;

const MAP: &str = include_str!("../../../shell/common/phosphor.js");

fn map() -> &'static HashMap<&'static str, String> {
    static M: OnceLock<HashMap<&'static str, String>> = OnceLock::new();
    M.get_or_init(|| {
        let mut m = HashMap::new();
        // lines: `    "acorn": "",`
        for l in MAP.lines() {
            let l = l.trim();
            let Some(rest) = l.strip_prefix('"') else { continue };
            let Some((name, rest)) = rest.split_once('"') else { continue };
            let Some(hex) = rest.split("\\u").nth(1) else { continue };
            let hex: String = hex.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
            if let Some(c) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                m.insert(name, c.to_string());
            }
        }
        m
    })
}

/// The glyph for a Phosphor name ("" when there's none)
pub fn glyph(name: &str) -> &'static str {
    map().get(name).map_or("", String::as_str)
}

/// The glyph, or `fallback`'s when the name is unknown or empty
pub fn glyph_or(name: &str, fallback: &str) -> &'static str {
    let g = glyph(name);
    if g.is_empty() {
        glyph(fallback)
    } else {
        g
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_the_shell_uses() {
        assert_eq!(glyph("lightning"), "\u{e2de}");
        assert_eq!(glyph("circle-wavy"), "\u{e604}");
        assert_eq!(glyph("nothing-like-this"), "");
        assert_eq!(glyph_or("", "bell"), glyph("bell"));
        assert!(map().len() > 1400);
    }
}

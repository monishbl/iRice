//! Phosphor's glyphs by name, as the QML shell names them (shell/common/
//! phosphor.js, compiled in): the island's alerts, Focus modes, the sound
//! item and the tray's fallbacks arrive as names ("music-notes", "moon").
//! The Material look draws Material Symbols in their place where the shell
//! has one (shell/common/material-symbols.js, Icon.qml): `symbol` maps a
//! Phosphor glyph to its symbol.

use std::collections::HashMap;
use std::sync::OnceLock;

const MAP: &str = include_str!("../../../shell/common/phosphor.js");
const SYMBOLS: &str = include_str!("../../../shell/common/material-symbols.js");

/// name -> glyph, from one of the shell's generated .js maps
/// (lines: `    "acorn": "\ue900",`)
fn parse(js: &'static str) -> HashMap<&'static str, String> {
    let mut m = HashMap::new();
    for l in js.lines() {
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
}

fn map() -> &'static HashMap<&'static str, String> {
    static M: OnceLock<HashMap<&'static str, String>> = OnceLock::new();
    M.get_or_init(|| parse(MAP))
}

/// Phosphor glyph -> its Material Symbol (Phosphor's regular, fill and bold
/// fonts share their code points, so the glyph names the icon; where two
/// names share a glyph and only one has a symbol, as seal and circle-wavy,
/// the glyph keeps Phosphor's: the shell's name may be the other)
fn symbols() -> &'static HashMap<String, String> {
    static M: OnceLock<HashMap<String, String>> = OnceLock::new();
    M.get_or_init(|| {
        let sym = parse(SYMBOLS);
        let mut by: HashMap<String, Vec<Option<&String>>> = HashMap::new();
        for (name, g) in map() {
            by.entry(g.clone()).or_default().push(sym.get(name));
        }
        by.into_iter()
            .filter_map(|(g, s)| match s[0] {
                Some(first) if s.iter().all(|x| *x == Some(first)) => Some((g, first.clone())),
                _ => None,
            })
            .collect()
    })
}

/// The Material Symbol drawn for a Phosphor glyph ("" when the shell has
/// none: the Phosphor glyph stays)
pub fn symbol(phosphor: &str) -> &'static str {
    symbols().get(phosphor).map_or("", String::as_str)
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

    #[test]
    fn material_draws_symbols() {
        assert_eq!(symbol(glyph("wifi-high")), "\u{e63e}");
        assert_eq!(symbol(glyph("lightning")), "\u{ea0b}");
        // (none for the badge: Phosphor's)
        assert_eq!(symbol(glyph("circle-wavy")), "");
        assert_eq!(symbol("x"), "");
    }
}

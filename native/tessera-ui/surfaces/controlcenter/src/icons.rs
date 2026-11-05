//! Icons by Phosphor name, as shell/common/Icon.qml draws them: the Phosphor
//! glyph (shell/common/phosphor.js), and in the Material look the matching
//! Material Symbols Rounded one where there is one (material-symbols.js).
//! The names come from the shell at run time (Focus modes, devices), so the
//! maps are read from the repo once, not compiled in.
//!
//! (Slint can't set the Symbols font's FILL axis: Material's filled icons
//! draw outlined.)

use std::collections::HashMap;
use std::sync::OnceLock;

/// `"name": "\uXXXX",` lines of a generated map
pub fn parse_map(src: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for line in src.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix('"') else { continue };
        let Some((name, rest)) = rest.split_once('"') else { continue };
        let Some(i) = rest.find("\\u") else { continue };
        let hex: String = rest[i + 2..].chars().take_while(|c| c.is_ascii_hexdigit()).collect();
        if let Some(c) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
            out.insert(name.to_string(), c.to_string());
        }
    }
    out
}

fn load(rel: &str) -> HashMap<String, String> {
    crate::util::asset(rel).and_then(|p| std::fs::read_to_string(p).ok()).map(|s| parse_map(&s)).unwrap_or_default()
}

fn phosphor() -> &'static HashMap<String, String> {
    static M: OnceLock<HashMap<String, String>> = OnceLock::new();
    M.get_or_init(|| load("shell/common/phosphor.js"))
}

fn symbols() -> &'static HashMap<String, String> {
    static M: OnceLock<HashMap<String, String>> = OnceLock::new();
    M.get_or_init(|| load("shell/common/material-symbols.js"))
}

/// (glyph, drawn with the Symbols font)
pub fn glyph(name: &str, material: bool) -> (String, bool) {
    if material {
        if let Some(g) = symbols().get(name) {
            return (g.clone(), true);
        }
    }
    (phosphor().get(name).cloned().unwrap_or_default(), false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_read_from_the_generated_files() {
        let m = parse_map("var glyphs = {\n    \"wifi-high\": \"\\ue4ea\",\n    \"x\": \"\\uE000\"\n};");
        assert_eq!(m["wifi-high"], "\u{e4ea}");
        assert_eq!(m["x"], "\u{e000}");
        // the repo's own
        assert_eq!(glyph("caret-left", false), ("\u{e138}".to_string(), false));
        let (g, sym) = glyph("arrow-clockwise", true);
        assert!(sym && !g.is_empty());
        assert_eq!(glyph("nothing-called-this", true), (String::new(), false));
    }
}

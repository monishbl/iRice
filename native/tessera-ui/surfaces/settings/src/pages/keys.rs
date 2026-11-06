//! How keys are written (shell/common/KeyNames.qml): ⌘ ⌃ ⌥ ⇧ for the
//! macOS style, Super+Ctrl+… for the others, as Settings' hints show them.

use super::super::config::Cfg;

/// mac | windows | linux (auto follows the menu icon: Command is mac)
pub fn style(cfg: &Cfg) -> String {
    let s = cfg.s("appearance.keySymbols");
    if s != "auto" && !s.is_empty() {
        return s;
    }
    if cfg.s("appearance.menuIcon") == "command" { "mac".into() } else { "linux".into() }
}

fn table(style: &str) -> &'static [(&'static str, &'static str)] {
    match style {
        "mac" => &[("SUPER", "⌘"), ("CTRL", "⌃"), ("ALT", "⌥"), ("SHIFT", "⇧"), ("Escape", "⎋"), ("Return", "↩"), ("BackSpace", "⌫"), ("Tab", "⇥"), ("Space", "Space"), ("Left", "←"), ("Right", "→"), ("Up", "↑"), ("Down", "↓")],
        "windows" => &[("SUPER", "Win"), ("CTRL", "Ctrl"), ("ALT", "Alt"), ("SHIFT", "Shift"), ("Escape", "Esc"), ("Return", "Enter"), ("BackSpace", "Backspace"), ("Tab", "Tab"), ("Space", "Space"), ("Left", "←"), ("Right", "→"), ("Up", "↑"), ("Down", "↓")],
        _ => &[("SUPER", "Super"), ("CTRL", "Ctrl"), ("ALT", "Alt"), ("SHIFT", "Shift"), ("Escape", "Esc"), ("Return", "Enter"), ("BackSpace", "Backspace"), ("Tab", "Tab"), ("Space", "Space"), ("Left", "←"), ("Right", "→"), ("Up", "↑"), ("Down", "↓")],
    }
}

fn strip_side(k: &str) -> String {
    let up = k.to_uppercase();
    up.strip_suffix("_L").or_else(|| up.strip_suffix("_R")).unwrap_or(&up).to_string()
}

/// One key's name in a style
pub fn name(style: &str, k: &str) -> String {
    let t = table(style);
    let up = strip_side(k);
    let key = match up.as_str() {
        "CONTROL" => "CTRL".to_string(),
        "META" | "MOD4" | "WIN" => "SUPER".to_string(),
        _ => up.clone(),
    };
    if let Some((_, v)) = t.iter().find(|(n, _)| *n == key) {
        return v.to_string();
    }
    if let Some((_, v)) = t.iter().find(|(n, _)| n.to_uppercase() == up) {
        return v.to_string();
    }
    if k.chars().count() == 1 { k.to_uppercase() } else { k.to_string() }
}

/// Keys in order: modifiers first (macOS's order in mac style)
pub fn keys(style: &str, list: &[&str]) -> Vec<String> {
    let mods = ["CTRL", "ALT", "SHIFT", "SUPER"];
    let up: Vec<String> = list.iter().map(|k| strip_side(k).replacen("CONTROL", "CTRL", 1)).collect();
    let m: Vec<&str> = mods.iter().copied().filter(|x| up.iter().any(|u| u == x)).collect();
    let rest: Vec<&str> = list.iter().enumerate().filter(|(i, _)| !mods.contains(&up[*i].as_str())).map(|(_, k)| *k).collect();
    let ordered: Vec<&str> = if style == "mac" { m } else { ["SUPER", "CTRL", "ALT", "SHIFT"].into_iter().filter(|x| m.contains(x)).collect() };
    ordered.into_iter().chain(rest).map(|k| name(style, k)).collect()
}

/// "SUPER Space" -> "⌘Space" (mac) or "Super+Space"
pub fn combo_in(style: &str, spec: &str) -> String {
    let parts: Vec<&str> = spec.split(|c: char| c.is_whitespace() || c == '+').filter(|x| !x.is_empty()).collect();
    let k = keys(style, &parts);
    if style == "mac" { k.join("") } else { k.join("+") }
}

pub fn combo(cfg: &Cfg, spec: &str) -> String {
    combo_in(&style(cfg), spec)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn written_as_key_names_qml() {
        assert_eq!(combo_in("linux", "SUPER Space"), "Super+Space");
        assert_eq!(combo_in("mac", "SUPER CTRL F"), "⌃⌘F");
        assert_eq!(combo_in("linux", "SUPER CTRL F"), "Super+Ctrl+F");
        assert_eq!(combo_in("windows", "ctrl+shift+Escape"), "Ctrl+Shift+Esc");
        let c = Cfg::detached(serde_json::json!({"appearance": {"menuIcon": "command"}}));
        assert_eq!(combo(&c, "SUPER A"), "⌘A");
    }
}

//! How shortcuts are read and written (shell/services/Keymap.qml: its fixed
//! lists, parseKeys, formatKeys, canonical, keyLabel, keyList, keyText,
//! describe, actionKey; and shell/common/KeyNames.qml). The same logic the
//! Settings surface keeps for its UI (surfaces/settings/src/pages/input/
//! keymap.rs), copied here so the host does not depend on a surface.

use serde_json::Value;

pub mod keys {
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

}

pub const SHELL_ACTIONS: &[(&str, &str)] = &[
    ("spotlight", "Search"),
    ("overview", "Overview"),
    ("superTap", "Tap Super (as set above)"),
    ("missionControl", "All Windows"),
    ("appExpose", "App Windows"),
    ("launchpad", "App Drawer"),
    ("showDesktop", "Show desktop"),
    ("switcher", "Switch windows"),
    ("switcherBack", "Switch windows backwards"),
    ("controlCenter", "Quick Settings"),
    ("notificationCenter", "Notification Center"),
    ("sidebar", "AI and tools sidebar"),
    ("clipboard", "Clipboard history"),
    ("shareSelection", "Share the selection with a nearby device"),
    ("replyLatest", "Reply to the newest message"),
    ("emoji", "Emoji and symbols"),
    ("settings", "Settings"),
    ("cheatsheet", "Keyboard shortcuts cheatsheet"),
    ("keyboard", "On-screen keyboard"),
    ("overlay", "Overlay widgets"),
    ("wallpaper", "Change wallpaper"),
    ("darkMode", "Light or dark appearance"),
    ("caffeine", "Caffeine on or off"),
    ("quickNote", "Quick note"),
    ("session", "Power options"),
    ("lock", "Lock the screen"),
    ("reset", "Close everything (if something is stuck)"),
    ("reportBug", "Report a bug"),
    ("screenshotRegion", "Screenshot of an area"),
    ("screenshotScreen", "Screenshot of the screen"),
    ("screenTools", "Screenshot and recording tools"),
    ("record", "Record an area (again to stop)"),
    ("ocr", "Copy text from the screen"),
    ("screenTranslate", "Translate text on the screen"),
    ("lens", "Search an area with Google Lens"),
    ("askScreen", "Ask AI about part of the screen"),
    ("circleSearch", "Circle to search"),
    ("volumeUp", "Volume up"),
    ("volumeDown", "Volume down"),
    ("volumeMute", "Mute sound"),
    ("micMute", "Mute microphone"),
    ("brightnessUp", "Brightness up"),
    ("brightnessDown", "Brightness down"),
    ("snap-left", "Tile window left half"),
    ("snap-right", "Tile window right half"),
    ("snap-top", "Tile window top half"),
    ("snap-bottom", "Tile window bottom half"),
    ("snap-fill", "Window fills the screen"),
    ("snap-center", "Centre window"),
    ("snap-restore", "Window back to its previous size"),
    ("tabs", "Merge windows into tabs"),
    ("pip", "Picture in Picture"),
];

pub const ROLES: &[(&str, &str)] = &[
    ("terminal", "Terminal"),
    ("files", "Files"),
    ("browser", "Browser"),
    ("code", "Code editor"),
    ("text", "Text editor"),
    ("office", "Office"),
    ("mixer", "Volume mixer"),
    ("tasks", "Task manager"),
];

/// Hyprland's own gesture actions
pub const GESTURE_ACTIONS: &[(&str, &str)] = &[
    ("workspace", "Switch workspace"),
    ("move", "Move window"),
    ("fullscreen", "Full screen"),
    ("float", "Float or tile"),
    ("close", "Close window"),
];

pub const DIRECTIONS: &[(&str, &str)] = &[
    ("horizontal", "Left or right"),
    ("vertical", "Up or down"),
    ("left", "Left"),
    ("right", "Right"),
    ("up", "Up"),
    ("down", "Down"),
    ("swipe", "Any direction"),
    ("pinch", "Pinch or spread"),
    ("pinchin", "Pinch"),
    ("pinchout", "Spread"),
    ("hold", "Long press"),
];

fn label_in(list: &[(&str, &str)], name: &str) -> String {
    list.iter().find(|x| x.0 == name).map_or(name.to_string(), |x| x.1.to_string())
}
pub fn shell_label(name: &str) -> String {
    label_in(SHELL_ACTIONS, name)
}
pub fn role_label(name: &str) -> String {
    label_in(ROLES, name)
}
pub fn direction_label(name: &str) -> String {
    label_in(DIRECTIONS, name)
}

/// A readable description of an action (Keymap.describe); `app_name` finds
/// a desktop entry's name
pub fn describe(a: &Value, app_name: &dyn Fn(&str) -> Option<String>) -> String {
    if !a.is_object() {
        return "Does nothing".into();
    }
    if let Some(s) = a.get("shell") {
        return shell_label(s.as_str().unwrap_or(""));
    }
    if let Some(s) = a.get("app") {
        return format!("Opens your {}", role_label(s.as_str().unwrap_or("")).to_lowercase());
    }
    if let Some(d) = a.get("desktop").and_then(Value::as_str).filter(|d| !d.is_empty()) {
        return format!("Opens {}", app_name(d).unwrap_or_else(|| d.to_string()));
    }
    if let Some(e) = a.get("exec") {
        let e = e.as_str().map(str::to_string).unwrap_or_else(|| e.to_string());
        return format!("Runs {}", e.replace("$TESSERA_SHELL/assets/scripts/", "").replace("$IRICE_SHELL/assets/scripts/", ""));
    }
    if let Some(z) = a.get("zoom") {
        return if z.as_f64().unwrap_or(0.0) > 0.0 { "Zooms in".into() } else { "Zooms out".into() };
    }
    if let Some(sp) = a.get("space") {
        let sp = sp.as_str().unwrap_or("");
        let n = a.get("n").filter(|n| !n.is_null());
        let wh = match n {
            Some(n) => format!("space {}", n.as_i64().map(|x| x.to_string()).unwrap_or_else(|| n.to_string())),
            None => if a["step"].as_f64().unwrap_or(0.0) > 0.0 { "the next space".into() } else { "the previous space".into() },
        };
        let verb = match sp {
            "focus" => "Goes to ",
            "send" => "Sends the window to ",
            _ => "Moves the window to ",
        };
        return format!("{verb}{wh} on this display");
    }
    if let Some(d) = a.get("dsp") {
        return format!("Window manager: {}", d.as_str().unwrap_or("").trim_start_matches("hl.dsp."));
    }
    "Unknown action".into()
}

/// Keymap.describeGesture: Hyprland's own by name, else an action
pub fn describe_gesture(a: &Value, app_name: &dyn Fn(&str) -> Option<String>) -> String {
    match a.as_str() {
        Some(s) => label_in(GESTURE_ACTIONS, s),
        None => describe(a, app_name),
    }
}

/// One string per distinct action (whitespace in commands doesn't count)
pub fn action_key(a: &Value) -> String {
    if let Some(s) = a.as_str() {
        return format!("g:{s}");
    }
    if !a.is_object() {
        return String::new();
    }
    let s = |k: &str| a[k].as_str().map(str::to_string).unwrap_or_else(|| a[k].to_string());
    if a.get("shell").is_some() {
        return format!("shell:{}", s("shell"));
    }
    if a.get("app").is_some() {
        return format!("app:{}", s("app"));
    }
    if a.get("exec").is_some() {
        return format!("exec:{}", s("exec").split_whitespace().collect::<Vec<_>>().join(" "));
    }
    if let Some(z) = a.get("zoom") {
        return format!("zoom:{:.2}", z.as_f64().unwrap_or(0.0));
    }
    if a.get("space").is_some() {
        let n = a.get("n").filter(|n| !n.is_null());
        let tail = match n {
            Some(n) => n.as_i64().map(|x| x.to_string()).unwrap_or_else(|| n.to_string()),
            None => format!("step{}", a["step"].as_i64().map(|x| x.to_string()).unwrap_or_else(|| a["step"].to_string())),
        };
        return format!("space:{}:{tail}", s("space"));
    }
    if a.get("dsp").is_some() {
        return format!("dsp:{}", s("dsp").split_whitespace().collect::<String>());
    }
    a.to_string()
}

// ── Keys ────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Parsed {
    pub mods: Vec<String>,
    pub key: String,
    pub tap: bool,
}

/// "$MOD + SHIFT + S" -> mods [SUPER, SHIFT], key "S"
pub fn parse_keys(keys: &str) -> Parsed {
    let mut p = Parsed::default();
    let mut rest: Vec<String> = Vec::new();
    for part in keys.split('+').map(str::trim).filter(|s| !s.is_empty()) {
        let u = part.to_uppercase();
        let m = match u.as_str() {
            "$MOD" | "SUPER" | "MOD4" | "WIN" | "META" => Some("SUPER"),
            "CTRL" | "CONTROL" => Some("CTRL"),
            "ALT" | "MOD1" => Some("ALT"),
            "SHIFT" => Some("SHIFT"),
            _ => None,
        };
        if let Some(m) = m {
            if !p.mods.iter().any(|x| x == m) {
                p.mods.push(m.into());
            }
        } else if u == "$MODKEY" || u == "SUPER_L" {
            p.tap = true;
            rest.push("$MODKEY".into());
        } else {
            rest.push(part.to_string());
        }
    }
    p.key = rest.join(" + ");
    p
}

/// Back to Hyprland's syntax, in a fixed order
pub fn format_keys(mods: &[String], key: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    for m in ["SUPER", "CTRL", "ALT", "SHIFT"] {
        if mods.iter().any(|x| x == m) {
            out.push(if m == "SUPER" { "$MOD".into() } else { m.into() });
        }
    }
    if !key.is_empty() {
        out.push(key.into());
    }
    out.join(" + ")
}

/// The same combo whatever the order or case
pub fn canonical(keys: &str) -> String {
    let p = parse_keys(keys);
    let mut m = p.mods.clone();
    m.sort();
    format!("{}|{}", m.join("+"), p.key.to_lowercase())
}

pub fn key_label(k: &str) -> String {
    if k == "$MODKEY" {
        return String::new();
    }
    let named: &[(&str, &str)] = &[
        ("Period", "."), ("period", "."), ("Comma", ","), ("comma", ","), ("Slash", "/"), ("slash", "/"), ("Semicolon", ";"), ("semicolon", ";"),
        ("Apostrophe", "'"), ("apostrophe", "'"), ("Minus", "-"), ("minus", "-"), ("Equal", "="), ("equal", "="),
        ("bracketleft", "["), ("BracketLeft", "["), ("bracketright", "]"), ("BracketRight", "]"), ("grave", "`"), ("Grave", "`"),
        ("backslash", "\\"), ("Backslash", "\\"), ("Print", "Print Screen"), ("Page_Up", "Page Up"), ("Page_Down", "Page Down"),
        ("Delete", "Delete"), ("Home", "Home"), ("End", "End"), ("Insert", "Insert"),
        ("mouse:272", "Left click"), ("mouse:273", "Right click"), ("mouse:274", "Middle click"), ("mouse:275", "Back button"), ("mouse:276", "Forward button"),
        ("SUPER_R", "Right Super"), ("Super_R", "Right Super"),
        ("mouse_down", "Scroll down"), ("mouse_up", "Scroll up"), ("mouse_left", "Scroll left"), ("mouse_right", "Scroll right"),
        ("code:86", "Keypad +"), ("code:82", "Keypad −"), ("code:63", "Keypad ×"), ("code:106", "Keypad ÷"), ("code:104", "Keypad Enter"),
    ];
    if let Some((_, v)) = named.iter().find(|(n, _)| *n == k) {
        return v.to_string();
    }
    let digits = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
    if let Some(n) = k.strip_prefix("code:").filter(|n| digits(n)) {
        return format!("Key {n}");
    }
    if let Some(n) = k.strip_prefix("mouse:").filter(|n| digits(n)) {
        return format!("Mouse button {n}");
    }
    if let Some(rest) = k.strip_prefix("XF86") {
        let mut out = String::new();
        let mut prev_lower = false;
        for c in rest.chars() {
            if c.is_ascii_uppercase() && prev_lower {
                out.push(' ');
            }
            prev_lower = c.is_ascii_lowercase();
            out.push(c);
        }
        return out;
    }
    if k.chars().count() == 1 {
        k.to_uppercase()
    } else {
        k.to_string()
    }
}

/// Keycaps (Keymap.keyList), e.g. [SUPER, SHIFT, S]; `mod_key` is what
/// Super is here (ALT in the sandbox)
pub fn key_list(keys: &str, mod_key: &str) -> Vec<String> {
    let p = parse_keys(keys);
    let right_tap = p.key.eq_ignore_ascii_case("super_r");
    let mut mods: Vec<String> = p.mods.iter().filter(|m| !(right_tap && *m == "SUPER")).map(|m| if m == "SUPER" { mod_key.to_string() } else { m.clone() }).collect();
    if p.tap && !mods.iter().any(|m| m == mod_key) {
        mods.push(mod_key.into());
    }
    mods.extend(p.key.split(" + ").map(key_label).filter(|x| !x.is_empty()));
    mods
}

/// "Super Shift S" (or ⇧⌘S with Mac symbols)
pub fn key_text(keys: &str, mod_key: &str, style: &str) -> String {
    let list = key_list(keys, mod_key);
    let refs: Vec<&str> = list.iter().map(String::as_str).collect();
    let l = keys::keys(style, &refs);
    if style == "mac" {
        l.join("")
    } else {
        l.join(" ")
    }
}


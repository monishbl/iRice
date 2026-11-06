//! What Settings needs of shell/services/Keymap.qml in Rust: its fixed lists
//! (Tessera actions, default-app roles, gesture actions and directions),
//! how keys are read and written (parseKeys, formatKeys, canonical,
//! keyLabel, keyList, keyText), the shortcut search (matches) and the clash
//! checks (conflicts, gestureConflicts). The binds and gestures themselves
//! come from the shell (RustBridgeInput.qml's feed: `keymap`), which keeps
//! keybinds.json; changes go back to it (`cx.ask("input", …)`).

use super::super::keys;
use serde_json::Value;

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

/// One string per distinct action (Keymap.actionKey)
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

// ── Search ──────────────────────────────────────────────────────────────

#[derive(Debug, PartialEq)]
pub struct Query {
    pub mods: Vec<String>,
    pub key: String,
}

/// Whether a query reads as keys ("super s", "ctrl print", "⇧⌘S")
pub fn parse_query(q: &str) -> Option<Query> {
    let mut s = String::new();
    for c in q.to_lowercase().trim().chars() {
        if "⌘⌃⌥⇧".contains(c) {
            s.push(' ');
            s.push(c);
            s.push(' ');
        } else if c == '+' {
            s.push(' ');
        } else {
            s.push(c);
        }
    }
    let words: Vec<&str> = s.split_whitespace().collect();
    if words.is_empty() {
        return None;
    }
    let mod_of = |w: &str| -> Option<&'static str> {
        Some(match w {
            "super" | "win" | "windows" | "cmd" | "command" | "meta" | "mod" | "⌘" => "SUPER",
            "ctrl" | "control" | "ctl" | "⌃" => "CTRL",
            "alt" | "option" | "opt" | "⌥" => "ALT",
            "shift" | "⇧" => "SHIFT",
            _ => return None,
        })
    };
    let key_of = |w: &str| -> String {
        match w {
            "enter" => "return",
            "esc" => "escape",
            "del" => "delete",
            "pgup" | "pageup" => "page_up",
            "pgdn" | "pagedown" => "page_down",
            "prtsc" | "printscreen" => "print",
            "." => "period",
            "," => "comma",
            "/" => "slash",
            ";" => "semicolon",
            "'" => "apostrophe",
            "-" => "minus",
            "=" => "equal",
            "[" => "bracketleft",
            "]" => "bracketright",
            "`" => "grave",
            "\\" => "backslash",
            "←" => "left",
            "→" => "right",
            "↑" => "up",
            "↓" => "down",
            w => w,
        }
        .to_string()
    };
    let mut mods: Vec<String> = Vec::new();
    let mut ks: Vec<String> = Vec::new();
    for w in words {
        match mod_of(w) {
            Some(m) => {
                if !mods.iter().any(|x| x == m) {
                    mods.push(m.into());
                }
            }
            None => ks.push(key_of(w)),
        }
    }
    const NAMES: &[&str] = &["print", "escape", "return", "tab", "space", "backspace", "delete", "home", "end", "page_up", "page_down", "insert"];
    if mods.is_empty() && !(ks.len() == 1 && ks[0].chars().count() > 1 && NAMES.contains(&ks[0].as_str())) {
        return None;
    }
    if ks.len() > 1 {
        return None;
    }
    Some(Query { mods, key: ks.into_iter().next().unwrap_or_default() })
}

fn key_matches(keys_of_bind: &str, k: &Query) -> bool {
    let p = parse_keys(keys_of_bind);
    if k.mods.iter().any(|m| !p.mods.contains(m)) {
        return false;
    }
    if k.key.is_empty() {
        return true;
    }
    let key = p.key.to_lowercase();
    let label = key_label(&p.key).to_lowercase();
    key == k.key || label == k.key
}

/// Keymap.matches: keys ("super s") match keys only; words match names
pub fn matches(b: &Value, query: &str) -> bool {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return true;
    }
    let keys_of_bind = b["keys"].as_str().unwrap_or("");
    let k = parse_query(&q);
    if let Some(k) = &k {
        if !k.mods.is_empty() {
            return key_matches(keys_of_bind, k);
        }
        if key_matches(keys_of_bind, k) {
            return true;
        }
    }
    let s = |f: &str| b[f].as_str().unwrap_or("").to_lowercase();
    s("label").contains(&q) || s("group").contains(&q) || s("actionText").contains(&q)
}

/// Binds (turned on) that use these keys, except `except`
pub fn conflicts<'a>(binds: &'a [Value], keys: &str, except: &str) -> Vec<&'a Value> {
    if keys.is_empty() {
        return Vec::new();
    }
    let c = canonical(keys);
    binds.iter().filter(|b| b["disabled"] != true && b["id"].as_str() != Some(except) && canonical(b["keys"].as_str().unwrap_or("")) == c).collect()
}

/// Directions that overlap: "swipe" covers every swipe, "horizontal" left
/// and right, "pinch" both pinches
pub fn overlaps(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    let cover = |x: &str| -> &'static [&'static str] {
        match x {
            "swipe" => &["horizontal", "vertical", "left", "right", "up", "down"],
            "horizontal" => &["left", "right"],
            "vertical" => &["up", "down"],
            "pinch" => &["pinchin", "pinchout"],
            _ => &[],
        }
    };
    cover(a).contains(&b) || cover(b).contains(&a)
}

pub fn gesture_conflicts<'a>(gestures: &'a [Value], fingers: i64, direction: &str, except: &str) -> Vec<&'a Value> {
    gestures
        .iter()
        .filter(|g| g["disabled"] != true && g["id"].as_str() != Some(except) && g["fingers"].as_i64() == Some(fingers) && overlaps(g["direction"].as_str().unwrap_or(""), direction))
        .collect()
}

// ── ShortcutRecorder.qml ────────────────────────────────────────────────

/// Keys the key box can catch (others open as text)
pub fn catchable(k: &str) -> bool {
    if k == "$MODKEY" {
        return true;
    }
    let one = k.chars().count() == 1 && k.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
    let f = k.strip_prefix('F').is_some_and(|n| (1..=2).contains(&n.len()) && n.chars().all(|c| c.is_ascii_digit()));
    one || f
        || [
            "Left", "Right", "Up", "Down", "Space", "Return", "Tab", "Escape", "BackSpace", "Delete", "Home", "End", "Page_Up", "Page_Down", "Print", "Insert", "Period", "Comma", "Slash", "Semicolon", "Apostrophe", "Minus", "Equal", "bracketleft",
            "bracketright", "grave", "backslash",
        ]
        .contains(&k)
}

/// Hyprland's keys as typed text ($MOD reads SUPER)
pub fn to_text(keys: &str) -> String {
    keys.replace("$MODKEY", "SUPER_L").replace("$MOD", "SUPER")
}

/// Typed text back to Hyprland's keys
pub fn from_text(t: &str) -> String {
    t.split('+')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| {
            let u = s.to_uppercase();
            match u.as_str() {
                "SUPER" | "WIN" | "META" | "MOD4" => "$MOD".to_string(),
                "SUPER_L" => "$MODKEY".to_string(),
                "CTRL" | "CONTROL" => "CTRL".to_string(),
                "ALT" | "SHIFT" => u,
                _ => s.to_string(),
            }
        })
        .collect::<Vec<_>>()
        .join(" + ")
}

/// A key Slint reported (its text) as Hyprland's name, and whether it was a
/// shifted symbol (Shift turns on); None for a modifier alone or a key it
/// can't name. Slint's special keys are private-use characters.
pub fn key_name(text: &str) -> Option<(String, bool)> {
    let mut chars = text.chars();
    let c = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    let name = |s: &str| Some((s.to_string(), false));
    let shifted = |s: &str| Some((s.to_string(), true));
    match c {
        'a'..='z' | 'A'..='Z' | '0'..='9' => Some((c.to_ascii_uppercase().to_string(), false)),
        ' ' => name("Space"),
        '\n' | '\r' => name("Return"),
        '\t' | '\u{19}' => name("Tab"),
        '\u{1b}' => name("Escape"),
        '\u{8}' => name("BackSpace"),
        '\u{7f}' => name("Delete"),
        '\u{F700}' => name("Up"),
        '\u{F701}' => name("Down"),
        '\u{F702}' => name("Left"),
        '\u{F703}' => name("Right"),
        '\u{F704}'..='\u{F71B}' => Some((format!("F{}", c as u32 - 0xF704 + 1), false)),
        '\u{F727}' => name("Insert"),
        '\u{F729}' => name("Home"),
        '\u{F72B}' => name("End"),
        '\u{F72C}' => name("Page_Up"),
        '\u{F72D}' => name("Page_Down"),
        '.' => name("Period"),
        ',' => name("Comma"),
        '/' => name("Slash"),
        ';' => name("Semicolon"),
        '\'' => name("Apostrophe"),
        '-' => name("Minus"),
        '=' => name("Equal"),
        '[' => name("bracketleft"),
        ']' => name("bracketright"),
        '`' => name("grave"),
        '\\' => name("backslash"),
        '?' => shifted("Slash"),
        ':' => shifted("Semicolon"),
        '"' => shifted("Apostrophe"),
        '<' => shifted("Comma"),
        '>' => shifted("Period"),
        '_' => shifted("Minus"),
        '+' => shifted("Equal"),
        '{' => shifted("bracketleft"),
        '}' => shifted("bracketright"),
        '~' => shifted("grave"),
        '|' => shifted("backslash"),
        '!' => shifted("1"),
        '@' => shifted("2"),
        '#' => shifted("3"),
        '$' => shifted("4"),
        '%' => shifted("5"),
        '^' => shifted("6"),
        '&' => shifted("7"),
        '*' => shifted("8"),
        '(' => shifted("9"),
        ')' => shifted("0"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn keys_read_and_written_as_keymap_qml() {
        let p = parse_keys("$MOD + SHIFT + S");
        assert_eq!(p.mods, ["SUPER", "SHIFT"]);
        assert_eq!(p.key, "S");
        assert_eq!(format_keys(&p.mods, &p.key), "$MOD + SHIFT + S");
        assert_eq!(format_keys(&["SHIFT".into(), "SUPER".into()], "S"), "$MOD + SHIFT + S");
        assert_eq!(canonical("SHIFT + SUPER + s"), canonical("$MOD + SHIFT + S"));
        assert!(parse_keys("$MODKEY").tap);
        assert_eq!(key_label("Print"), "Print Screen");
        assert_eq!(key_label("code:12"), "Key 12");
        assert_eq!(key_label("XF86AudioPlay"), "Audio Play");
        assert_eq!(key_list("$MOD + SHIFT + S", "SUPER"), ["SUPER", "SHIFT", "S"]);
        assert_eq!(key_list("$MODKEY", "SUPER"), ["SUPER"]);
        assert_eq!(key_text("$MOD + SHIFT + S", "SUPER", "linux"), "Super Shift S");
        assert_eq!(key_text("$MOD + SHIFT + S", "SUPER", "mac"), "⇧⌘S");
    }

    #[test]
    fn search_by_name_or_keys() {
        let b = json!({"keys": "$MOD + SHIFT + S", "label": "Screenshot", "group": "Tools", "actionText": "Screenshot of an area"});
        assert!(matches(&b, "screen"));
        assert!(matches(&b, "super shift s"));
        assert!(matches(&b, "⇧⌘S"));
        assert!(!matches(&b, "ctrl s"));
        assert!(matches(&b, ""));
        assert_eq!(parse_query("print"), Some(Query { mods: vec![], key: "print".into() }));
        assert_eq!(parse_query("screenshot"), None);
    }

    #[test]
    fn clashes() {
        let binds = vec![json!({"id": "a", "keys": "$MOD + Q"}), json!({"id": "b", "keys": "SUPER + q", "disabled": true})];
        assert_eq!(conflicts(&binds, "$MOD + Q", "x").len(), 1);
        assert!(conflicts(&binds, "$MOD + Q", "a").is_empty());
        let g = vec![json!({"id": "w", "fingers": 3, "direction": "horizontal"})];
        assert_eq!(gesture_conflicts(&g, 3, "left", "").len(), 1);
        assert!(gesture_conflicts(&g, 3, "up", "").is_empty());
        assert!(overlaps("swipe", "down") && overlaps("pinchin", "pinch") && !overlaps("left", "right"));
    }

    #[test]
    fn the_recorder_names_keys() {
        assert_eq!(key_name("a"), Some(("A".into(), false)));
        assert_eq!(key_name("?"), Some(("Slash".into(), true)));
        assert_eq!(key_name("\u{F704}"), Some(("F1".into(), false)));
        assert_eq!(key_name("\u{10}"), None);
        assert!(catchable("F12") && catchable("Q") && !catchable("mouse:272") && !catchable("XF86AudioPlay"));
        assert_eq!(from_text("super + shift + s"), "$MOD + SHIFT + s");
        assert_eq!(to_text("$MOD + $MODKEY"), "SUPER + SUPER_L");
    }

    #[test]
    fn the_lists_are_keymap_qmls() {
        let qml = std::fs::read_to_string(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../shell/services/Keymap.qml")).unwrap();
        for (n, l) in SHELL_ACTIONS.iter().chain(ROLES).chain(GESTURE_ACTIONS).chain(DIRECTIONS) {
            assert!(qml.contains(&format!("{{ name: \"{n}\", label: \"{l}\" }}")), "{n}");
        }
        assert_eq!(describe(&json!({"app": "terminal"}), &|_| None), "Opens your terminal");
        assert_eq!(describe(&json!({"desktop": "x", "exec": "y"}), &|_| Some("Firefox".into())), "Opens Firefox");
        assert_eq!(describe_gesture(&json!("move"), &|_| None), "Move window");
    }
}

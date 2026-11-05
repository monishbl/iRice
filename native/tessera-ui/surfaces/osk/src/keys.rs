//! The on-screen keyboard's keys (shell/modules/osk/Keyboard.qml's `rows`)
//! and what a press sends: a key of its own keymap (one key per keysym, as
//! wtype makes it), with the modifiers held for it. Pure, tested without a display.

/// One key: [label, shift label, keysym or @modifier, width in units] as
/// the QML's rows say it
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Key {
    pub label: &'static str,
    /// the label with Shift ("" for a letter: its capital, by Shift or Caps)
    pub shifted: &'static str,
    /// the keysym it types, or "@shift", "@ctrl", "@alt", "@logo", "@caps"
    pub sym: &'static str,
    pub units: f32,
}

const fn l(label: &'static str) -> Key {
    Key { label, shifted: "", sym: label, units: 1.0 }
}
const fn p(label: &'static str, shifted: &'static str) -> Key {
    Key { label, shifted, sym: label, units: 1.0 }
}
const fn n(label: &'static str, shifted: &'static str, sym: &'static str) -> Key {
    Key { label, shifted, sym, units: 1.0 }
}
const fn w(label: &'static str, sym: &'static str, units: f32) -> Key {
    Key { label, shifted: label, sym, units }
}

pub const ROWS: [&[Key]; 5] = [
    &[n("`", "~", "grave"), p("1", "!"), p("2", "@"), p("3", "#"), p("4", "$"), p("5", "%"), p("6", "^"), p("7", "&"), p("8", "*"), p("9", "("), p("0", ")"), n("-", "_", "minus"), n("=", "+", "equal"), w("⌫", "BackSpace", 1.6)],
    &[w("Tab", "Tab", 1.5), l("q"), l("w"), l("e"), l("r"), l("t"), l("y"), l("u"), l("i"), l("o"), l("p"), n("[", "{", "bracketleft"), n("]", "}", "bracketright"), Key { label: "\\", shifted: "|", sym: "backslash", units: 1.1 }],
    &[w("Caps", "@caps", 1.8), l("a"), l("s"), l("d"), l("f"), l("g"), l("h"), l("j"), l("k"), l("l"), n(";", ":", "semicolon"), n("'", "\"", "apostrophe"), w("Return", "Return", 2.0)],
    &[w("Shift", "@shift", 2.3), l("z"), l("x"), l("c"), l("v"), l("b"), l("n"), l("m"), n(",", "<", "comma"), n(".", ">", "period"), n("/", "?", "slash"), w("Shift", "@shift", 2.5)],
    &[w("Ctrl", "@ctrl", 1.4), w("Super", "@logo", 1.4), w("Alt", "@alt", 1.4), w("", "space", 6.2), w("←", "Left", 1.0), w("↓", "Down", 1.0), w("↑", "Up", 1.0), w("→", "Right", 1.0), w("Esc", "Escape", 1.4)],
];

/// The sticky modifiers (tap Shift, then a key)
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Mods {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub logo: bool,
}

impl Mods {
    pub fn any(&self) -> bool {
        self.shift || self.ctrl || self.alt || self.logo
    }
    /// One toggled ("shift", "ctrl", "alt", "logo")
    pub fn toggled(mut self, m: &str) -> Self {
        match m {
            "shift" => self.shift = !self.shift,
            "ctrl" => self.ctrl = !self.ctrl,
            "alt" => self.alt = !self.alt,
            "logo" => self.logo = !self.logo,
            _ => {}
        }
        self
    }
    pub fn has(&self, m: &str) -> bool {
        match m {
            "shift" => self.shift,
            "ctrl" => self.ctrl,
            "alt" => self.alt,
            "logo" => self.logo,
            _ => false,
        }
    }
}

impl Key {
    pub fn is_mod(&self) -> bool {
        self.sym.starts_with('@')
    }
    /// A letter: Shift or Caps picks its case (the QML's one-entry keys)
    pub fn letter(&self) -> bool {
        self.shifted.is_empty() && !self.label.is_empty() // (the space bar has no label at all)
    }
    /// Return: Material's action key (primary container)
    pub fn action(&self) -> bool {
        self.sym == "Return"
    }
    /// Wider than one unit, or a modifier: the tonal keys
    pub fn wide(&self) -> bool {
        self.is_mod() || self.units > 1.0
    }
    /// What it shows now (Keyboard.qml: a letter follows Shift and Caps, the
    /// others Shift alone)
    pub fn text(&self, mods: Mods, caps: bool) -> String {
        if self.letter() {
            if mods.shift != caps { self.label.to_uppercase() } else { self.label.to_string() }
        } else if mods.shift {
            self.shifted.to_string()
        } else {
            self.label.to_string()
        }
    }
    pub fn latched(&self, mods: Mods, caps: bool) -> bool {
        match self.sym.strip_prefix('@') {
            Some("caps") => caps,
            Some(m) => mods.has(m),
            None => false,
        }
    }
}


/// xkb's real modifier bits (wtype's -M)
pub const SHIFT: u32 = 1;
pub const CTRL: u32 = 4;
pub const ALT: u32 = 8; // Mod1
pub const LOGO: u32 = 64; // Mod4

/// A key to tap: its evdev code in our keymap and the modifiers held for it
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stroke {
    pub code: u32,
    pub depressed: u32,
}

/// The keysym's name for a character a key types ("!" exclam, "A" A)
pub fn keysym(ch: &str) -> String {
    let named = match ch {
        "!" => "exclam",
        "@" => "at",
        "#" => "numbersign",
        "$" => "dollar",
        "%" => "percent",
        "^" => "asciicircum",
        "&" => "ampersand",
        "*" => "asterisk",
        "(" => "parenleft",
        ")" => "parenright",
        "_" => "underscore",
        "+" => "plus",
        "~" => "asciitilde",
        "{" => "braceleft",
        "}" => "braceright",
        "|" => "bar",
        ":" => "colon",
        "\"" => "quotedbl",
        "<" => "less",
        ">" => "greater",
        "?" => "question",
        _ => return ch.to_string(),
    };
    named.to_string()
}

/// What a key types now: a letter's capital with Shift or Caps, a shifted
/// character with Shift, else its own keysym
pub fn typed(key: &Key, mods: Mods, caps: bool) -> String {
    if key.letter() {
        if mods.shift != caps { key.label.to_uppercase() } else { key.label.to_string() }
    } else if mods.shift && key.shifted != key.label && key.shifted.chars().count() == 1 {
        keysym(key.shifted)
    } else {
        key.sym.to_string()
    }
}

/// Every keysym the keyboard types, then the modifier keys, in keycode
/// order (the keymap below gives each its own key, as wtype does)
pub fn keysyms() -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut add = |s: String| {
        if !out.contains(&s) {
            out.push(s);
        }
    };
    for k in ROWS.iter().flat_map(|r| r.iter()).filter(|k| !k.is_mod()) {
        add(k.sym.to_string());
        if k.letter() {
            add(k.label.to_uppercase());
        } else if k.shifted != k.label && k.shifted.chars().count() == 1 {
            add(keysym(k.shifted));
        }
    }
    for m in ["Shift_L", "Control_L", "Alt_L", "Super_L"] {
        add(m.to_string());
    }
    out
}

/// A keysym's evdev code in our keymap (keycode 9 is evdev 1)
pub fn code(sym: &str) -> Option<u32> {
    keysyms().iter().position(|s| s == sym).map(|i| i as u32 + 1)
}

/// The keymap the virtual keyboard sends (xkb text, the compositor's xkb
/// resolves the includes, as wtype's): one one-level key per keysym, so
/// what it types never depends on how the compositor passes modifiers on;
/// the modifiers mapped as a standard keyboard has them
pub fn keymap() -> String {
    let syms = keysyms();
    let mut k = format!("xkb_keymap {{\nxkb_keycodes \"(unnamed)\" {{\nminimum = 8;\nmaximum = {};\n", syms.len() + 9);
    for i in 0..syms.len() {
        k += &format!("<K{}> = {};\n", i + 1, i + 9);
    }
    k += "};\nxkb_types \"(unnamed)\" { include \"complete\" };\nxkb_compatibility \"(unnamed)\" { include \"complete\" };\nxkb_symbols \"(unnamed)\" {\n";
    for (i, s) in syms.iter().enumerate() {
        k += &format!("key <K{}> {{[ {s} ]}};\n", i + 1);
    }
    for (m, s) in [("Shift", "Shift_L"), ("Control", "Control_L"), ("Mod1", "Alt_L"), ("Mod4", "Super_L")] {
        if let Some(i) = syms.iter().position(|x| x == s) {
            k += &format!("modifier_map {m} {{ <K{}> }};\n", i + 1);
        }
    }
    k + "};\n};\n"
}

/// What a press does: a modifier or Caps toggles (no stroke); any other key
/// is typed with the latched modifiers held (wtype's -M), which then let go
/// (Caps stays). Returns the stroke and the modifiers and Caps after it.
pub fn press(key: &Key, mods: Mods, caps: bool) -> (Option<Stroke>, Mods, bool) {
    if let Some(m) = key.sym.strip_prefix('@') {
        return if m == "caps" { (None, mods, !caps) } else { (None, mods.toggled(m), caps) };
    }
    let Some(code) = code(&typed(key, mods, caps)) else { return (None, mods, caps) };
    let mut depressed = 0;
    for (on, bit) in [(mods.shift, SHIFT), (mods.ctrl, CTRL), (mods.alt, ALT), (mods.logo, LOGO)] {
        if on {
            depressed |= bit;
        }
    }
    (Some(Stroke { code, depressed }), if mods.any() { Mods::default() } else { mods }, caps)
}

/// The keyboard's size for a screen width (logical px): the key unit, then
/// the window's width and height with the shadow's room `m` (Keyboard.qml)
pub fn unit(screen_w: f32) -> f32 {
    58f32.min((screen_w - 120.0) / 15.6)
}
pub fn window_size(screen_w: f32, m: f32) -> (u32, u32) {
    let u = unit(screen_w);
    ((u * 15.6 + 24.0 + 2.0 * m).round() as u32, (u * 5.0 + 4.0 * 6.0 + 24.0 + 2.0 * m).round() as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find(label: &str) -> Key {
        *ROWS.iter().flat_map(|r| r.iter()).find(|k| k.label == label).unwrap()
    }

    #[test]
    fn every_key_types_something_or_latches() {
        let shift = Mods { shift: true, ..Default::default() };
        for k in ROWS.iter().flat_map(|r| r.iter()) {
            assert!(k.is_mod() || code(&typed(k, Mods::default(), false)).is_some(), "{k:?}");
            assert!(k.is_mod() || code(&typed(k, shift, false)).is_some(), "{k:?} shifted");
        }
        assert_eq!(ROWS.iter().map(|r| r.len()).collect::<Vec<_>>(), vec![14, 14, 13, 12, 9]);
    }

    #[test]
    fn labels_follow_shift_and_caps() {
        let (none, shift) = (Mods::default(), Mods { shift: true, ..Default::default() });
        assert_eq!(find("q").text(none, false), "q");
        assert_eq!(find("q").text(shift, false), "Q");
        assert_eq!(find("q").text(none, true), "Q");
        assert_eq!(find("q").text(shift, true), "q");
        // (caps leaves the others alone)
        assert_eq!(find("1").text(none, true), "1");
        assert_eq!(find("1").text(shift, false), "!");
        assert_eq!(find("Tab").text(shift, false), "Tab");
    }

    #[test]
    fn it_types_what_the_cap_shows() {
        let (none, shift) = (Mods::default(), Mods { shift: true, ..Default::default() });
        assert_eq!(typed(&find("1"), shift, false), "exclam");
        assert_eq!(typed(&find("`"), shift, false), "asciitilde");
        assert_eq!(typed(&find("a"), none, true), "A");
        assert_eq!(typed(&find("a"), shift, true), "a");
        assert_eq!(typed(&find("Tab"), shift, false), "Tab");
        assert_eq!(typed(&find(""), none, false), "space");
    }

    #[test]
    fn modifiers_latch_then_let_go() {
        let (s, m, c) = press(&find("Shift"), Mods::default(), false);
        assert!(s.is_none() && m.shift && !c);
        let (s, m2, _) = press(&find("a"), m, false);
        assert_eq!(s, Some(Stroke { code: code("A").unwrap(), depressed: SHIFT }));
        assert_eq!(m2, Mods::default());
        // ctrl + alt then a named key: both held, both let go
        let m = Mods { ctrl: true, alt: true, ..Default::default() };
        let (s, m2, _) = press(&find("←"), m, false);
        assert_eq!(s, Some(Stroke { code: code("Left").unwrap(), depressed: CTRL | ALT }));
        assert!(!m2.any());
        // caps toggles and stays
        let (s, _, c) = press(&find("Caps"), Mods::default(), false);
        assert!(s.is_none() && c);
        let (s, _, c) = press(&find("z"), Mods::default(), true);
        assert_eq!(s, Some(Stroke { code: code("Z").unwrap(), depressed: 0 }));
        assert!(c);
        // Super latched
        let (_, m, _) = press(&find("Super"), Mods::default(), false);
        assert!(m.logo && find("Super").latched(m, false));
        assert_eq!(press(&find("Esc"), m, false).0.unwrap().depressed, LOGO);
    }

    #[test]
    fn a_key_per_keysym() {
        let syms = keysyms();
        assert_eq!(syms[0], "grave");
        assert_eq!(code("grave"), Some(1));
        assert!(code("exclam").is_some() && code("Shift_L").is_some() && code("F13").is_none());
        let mut sorted = syms.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), syms.len(), "each keysym once");
        let k = keymap();
        assert!(k.contains("<K1> = 9;") && k.contains("key <K1> {[ grave ]};"));
        assert!(k.contains(&format!("modifier_map Shift {{ <K{}> }};", code("Shift_L").unwrap())));
        assert!(k.contains(&format!("maximum = {};", syms.len() + 9)));
    }

    #[test]
    fn sized_as_the_qml() {
        // 1920 wide: the unit is 58 (its maximum)
        assert_eq!(unit(1920.0), 58.0);
        assert_eq!(window_size(1920.0, 20.0), ((58.0f32 * 15.6 + 64.0).round() as u32, 58 * 5 + 24 + 24 + 40));
        // a narrow screen: the keys shrink
        assert!((unit(800.0) - 680.0 / 15.6).abs() < 1e-4);
    }
}

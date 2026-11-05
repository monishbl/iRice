//! Search's results for what's typed, in the order shell/services/Search.qml's
//! query() gives them (the same score, the same sections, the same top hit),
//! as pure functions: what's known goes in (`Known`), the rows come out, with
//! what still has to be asked for (`Wants`: an answer, a conversion, a
//! definition, files, the shell's own rows). Prefixes: ">" a command, ";"
//! the clipboard, ":" emoji, "/" files, "?" ask AI.
//!
//! Rows only the QML shell knows (plain-language actions, the calendar,
//! contacts, notes, screenshots, the App Store, the clipboard, Chromecasts)
//! come from it as `Extras` (Search.qml's rustSend) and run there by key.

/// What a row does when it's run
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Run {
    #[default]
    Nothing,
    /// copy this text (wl-copy)
    Copy(String),
    /// start the app with this desktop id
    App(String),
    /// open a web link
    Url(String),
    /// run this in a terminal (kept open)
    Terminal(String),
    /// open a file or folder (Files.open)
    File(String),
    /// one of Search.qml's actions, by title
    Action(String),
    /// a row the shell sent, by its key
    Shell(String),
    /// ask the AI this, answered in place
    Ask(String),
    /// AI isn't set up: its settings
    SetupAi,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Row {
    pub section: String,
    pub title: String,
    pub subtitle: String,
    /// a Phosphor name (shell/common/phosphor.js)
    pub glyph: String,
    /// an icon name or file path (looked up in the icon theme), "" none
    pub icon: String,
    pub icon_fallback: &'static str,
    pub emoji: String,
    /// a picture of its own (a screenshot found by its words)
    pub picture: String,
    /// a file it is (Quick Look)
    pub path: String,
    pub loading: bool,
    /// answered in place (Ask AI)
    pub ai: bool,
    /// an answer's kind (math, date, time, unit, currency)
    pub kind: String,
    pub score: f64,
    pub run: Run,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct App {
    pub id: String,
    pub name: String,
    pub generic: String,
    pub comment: String,
    pub keywords: Vec<String>,
    pub icon: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Action {
    pub title: String,
    pub glyph: String,
    pub keywords: String,
}

/// answers.py's reply (an empty title: no answer)
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Answer {
    pub kind: String,
    pub title: String,
    pub subtitle: String,
    pub copy: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Definition {
    pub word: String,
    pub phonetic: String,
    pub part: String,
    pub text: String,
    pub missing: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AiMeta {
    pub configured: bool,
    /// Ai.model.name
    pub name: String,
}

/// The shell's rows for one query (Search.qml's rustSend)
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Extras {
    pub q: String,
    pub intents: Vec<Row>,
    pub reminder: Vec<Row>,
    pub store: Vec<Row>,
    pub calendar: Vec<Row>,
    pub contacts: Vec<Row>,
    pub notes: Vec<Row>,
    pub shots: Vec<Row>,
    pub clipboard: Vec<Row>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Known<'a> {
    pub apps: &'a [App],
    /// Search.qml's actions, then the Chromecasts' (castActions)
    pub actions: &'a [Action],
    /// answers.py's reply for this very text (None: not yet)
    pub answer: Option<&'a Answer>,
    /// qalc is installed
    pub qalc: bool,
    /// qalc's conversion of this very text ("" none)
    pub conversion: Option<&'a str>,
    /// the definition of the word asked about (for word_to_define(text))
    pub definition: Option<&'a Definition>,
    /// the shell's rows for this very text
    pub extras: Option<&'a Extras>,
    pub ai: Option<&'a AiMeta>,
    /// emoji.js's list: (emoji, name)
    pub emoji: &'a [(String, String)],
    /// the last file search's names (is a folder, path) and documents with it inside
    pub files: &'a [(bool, String)],
    pub contents: &'a [String],
    pub home: &'a str,
}

/// What the rows depend on that isn't known yet
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Wants {
    pub answer: Option<String>,
    pub conversion: Option<String>,
    pub define: Option<String>,
    pub files: Option<String>,
    /// the shell's rows for this text, and whether to look in the App Store
    pub extras: Option<(String, bool)>,
}

/// Sections whose first row is an answer
pub const ANSWERS: [&str; 4] = ["Calculator", "Answer", "Conversion", "Dictionary"];

/// JavaScript's String.length (UTF-16 units)
pub fn js_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// Search.score: fuzzy match, 0 = none. Prefers a whole-string prefix, then a
/// word's prefix, then a substring, then letters in order
pub fn score(text: &str, q: &str) -> f64 {
    if text.is_empty() {
        return 0.0;
    }
    let t = text.to_lowercase();
    if t.starts_with(q) {
        return 100.0 - (js_len(&t) as f64 - js_len(q) as f64).min(20.0) * 0.5;
    }
    if t.split(|c: char| c.is_whitespace() || c == '-' || c == '_' || c == '.').any(|w| w.starts_with(q)) {
        return 80.0;
    }
    if t.contains(q) {
        return 60.0;
    }
    let qc: Vec<char> = q.chars().collect();
    let (mut i, mut gaps) = (0usize, 0i32);
    for c in t.chars() {
        if i < qc.len() && c == qc[i] {
            i += 1;
        } else if i > 0 {
            gaps += 1;
        }
        if i == qc.len() {
            return (40 - gaps).max(1) as f64;
        }
    }
    0.0
}

// ── The calculator: Search.calculate, as JavaScript would evaluate it ──────

/// JavaScript's Math.round (halves up)
fn js_round(x: f64) -> f64 {
    let f = x.floor();
    if x - f >= 0.5 {
        f + 1.0
    } else {
        f
    }
}

/// JavaScript's `**`
fn js_pow(b: f64, e: f64) -> f64 {
    if e.is_nan() {
        return f64::NAN;
    }
    if e == 0.0 {
        return 1.0;
    }
    if b.is_nan() || (b.abs() == 1.0 && e.is_infinite()) {
        return f64::NAN;
    }
    b.powf(e)
}

/// JavaScript's String(number)
pub fn js_str(v: f64) -> String {
    if v.is_nan() {
        return "NaN".into();
    }
    if v == 0.0 {
        return "0".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "Infinity" } else { "-Infinity" }.into();
    }
    if v < 0.0 {
        return format!("-{}", js_str(-v));
    }
    let s = format!("{v:e}");
    let (mant, exp) = s.split_once('e').unwrap_or((&s, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let digits: String = mant.chars().filter(|c| *c != '.').collect();
    let k = digits.len() as i32;
    let n = exp + 1;
    if k <= n && n <= 21 {
        digits + &"0".repeat((n - k) as usize)
    } else if 0 < n && n <= 21 {
        format!("{}.{}", &digits[..n as usize], &digits[n as usize..])
    } else if -6 < n && n <= 0 {
        format!("0.{}{}", "0".repeat((-n) as usize), digits)
    } else {
        let e = n - 1;
        let sign = if e >= 0 { "+" } else { "-" };
        if k == 1 {
            format!("{digits}e{sign}{}", e.abs())
        } else {
            format!("{}.{}e{sign}{}", &digits[..1], &digits[1..], e.abs())
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Num(f64),
    /// Math.<name> (or an unknown name: NaN, as `undefined`)
    Name(String),
    Op(&'static str),
}

#[derive(Clone, Copy, Debug)]
enum Val {
    Num(f64),
    Func(fn(f64) -> f64),
}

fn tokens(s: &str) -> Option<Vec<Tok>> {
    let c: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        if ch.is_ascii_digit() || (ch == '.' && c.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
            // (strict mode: no legacy octal "01")
            if ch == '0' && c.get(i + 1).is_some_and(|d| d.is_ascii_digit()) {
                return None;
            }
            let st = i;
            while i < c.len() && c[i].is_ascii_digit() {
                i += 1;
            }
            if i < c.len() && c[i] == '.' {
                i += 1;
                while i < c.len() && c[i].is_ascii_digit() {
                    i += 1;
                }
            }
            if i < c.len() && (c[i] == 'e' || c[i] == 'E') {
                let mut j = i + 1;
                if j < c.len() && (c[j] == '+' || c[j] == '-') {
                    j += 1;
                }
                if j < c.len() && c[j].is_ascii_digit() {
                    while j < c.len() && c[j].is_ascii_digit() {
                        j += 1;
                    }
                    i = j;
                } else {
                    return None;
                }
            }
            let text: String = c[st..i].iter().collect();
            // (a name or digit right after a number is a syntax error)
            if i < c.len() && (c[i].is_ascii_alphanumeric() || c[i] == '_' || c[i] == '$') {
                return None;
            }
            out.push(Tok::Num(text.parse().ok()?));
        } else if ch.is_ascii_alphabetic() {
            let st = i;
            while i < c.len() && (c[i].is_ascii_alphanumeric() || c[i] == '.') {
                i += 1;
            }
            let text: String = c[st..i].iter().collect();
            let parts: Vec<&str> = text.split('.').collect();
            if parts[0] != "Math" || parts.iter().any(|p| p.is_empty()) {
                return None; // (a ReferenceError, or a syntax error)
            }
            if parts.len() > 2 {
                return None; // (a member of undefined: TypeError)
            }
            out.push(Tok::Name(parts.get(1).unwrap_or(&"").to_string()));
        } else if ch == '/' && c.get(i + 1) == Some(&'*') {
            // a block comment
            let mut j = i + 2;
            loop {
                if j + 1 >= c.len() {
                    return None;
                }
                if c[j] == '*' && c[j + 1] == '/' {
                    break;
                }
                j += 1;
            }
            i = j + 2;
        } else if ch == '/' && c.get(i + 1) == Some(&'/') {
            return None; // (a line comment eats the closing parenthesis)
        } else {
            let two: String = c[i..(i + 2).min(c.len())].iter().collect();
            if two == "++" || two == "--" {
                return None;
            }
            if two == "**" {
                out.push(Tok::Op("**"));
                i += 2;
                continue;
            }
            let op = match ch {
                '+' => "+",
                '-' => "-",
                '*' => "*",
                '/' => "/",
                '%' => "%",
                '(' => "(",
                ')' => ")",
                _ => return None,
            };
            out.push(Tok::Op(op));
            i += 1;
        }
    }
    Some(out)
}

struct Parser {
    t: Vec<Tok>,
    i: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.t.get(self.i)
    }
    fn op(&self, o: &str) -> bool {
        matches!(self.peek(), Some(Tok::Op(x)) if *x == o)
    }
    fn num(v: Val) -> Option<f64> {
        match v {
            Val::Num(n) => Some(n),
            Val::Func(_) => None, // (a function as a number: not a number result)
        }
    }
    fn additive(&mut self) -> Option<Val> {
        let mut a = self.multiplicative()?;
        loop {
            if self.op("+") || self.op("-") {
                let plus = self.op("+");
                self.i += 1;
                let b = Self::num(self.multiplicative()?)?;
                let x = Self::num(a)?;
                a = Val::Num(if plus { x + b } else { x - b });
            } else {
                return Some(a);
            }
        }
    }
    fn multiplicative(&mut self) -> Option<Val> {
        let mut a = self.exponent()?;
        loop {
            let o = match self.peek() {
                Some(Tok::Op(o)) if ["*", "/", "%"].contains(o) => *o,
                _ => return Some(a),
            };
            self.i += 1;
            let b = Self::num(self.exponent()?)?;
            let x = Self::num(a)?;
            a = Val::Num(match o {
                "*" => x * b,
                "/" => x / b,
                _ => x % b,
            });
        }
    }
    fn exponent(&mut self) -> Option<Val> {
        if self.op("+") || self.op("-") {
            let v = self.unary()?;
            // (-2 ** 2 is a syntax error)
            if self.op("**") {
                return None;
            }
            return Some(v);
        }
        let base = self.call()?;
        if self.op("**") {
            self.i += 1;
            let e = Self::num(self.exponent()?)?;
            return Some(Val::Num(js_pow(Self::num(base)?, e)));
        }
        Some(base)
    }
    fn unary(&mut self) -> Option<Val> {
        if self.op("+") || self.op("-") {
            let neg = self.op("-");
            self.i += 1;
            let v = Self::num(self.unary()?)?;
            return Some(Val::Num(if neg { -v } else { v }));
        }
        self.call()
    }
    fn call(&mut self) -> Option<Val> {
        let mut v = self.primary()?;
        while self.op("(") {
            let Val::Func(f) = v else { return None }; // (TypeError: not a function)
            self.i += 1;
            if self.op(")") {
                self.i += 1;
                v = Val::Num(f(f64::NAN));
                continue;
            }
            let a = Self::num(self.additive()?)?;
            if !self.op(")") {
                return None;
            }
            self.i += 1;
            v = Val::Num(f(a));
        }
        Some(v)
    }
    fn primary(&mut self) -> Option<Val> {
        let t = self.peek()?.clone();
        self.i += 1;
        match t {
            Tok::Num(n) => Some(Val::Num(n)),
            Tok::Name(n) => Some(match n.as_str() {
                "PI" => Val::Num(std::f64::consts::PI),
                "E" => Val::Num(std::f64::consts::E),
                "sqrt" => Val::Func(f64::sqrt),
                "sin" => Val::Func(f64::sin),
                "cos" => Val::Func(f64::cos),
                "tan" => Val::Func(f64::tan),
                "log10" => Val::Func(f64::log10),
                "log" => Val::Func(f64::ln),
                _ => Val::Num(f64::NAN),
            }),
            Tok::Op("(") => {
                let v = self.additive()?;
                if !self.op(")") {
                    return None;
                }
                self.i += 1;
                Some(v)
            }
            Tok::Op(_) => None,
        }
    }
}

/// Search.calculate: arithmetic only (digits, operators, parentheses, a few
/// functions), None when it isn't a sum or doesn't give a finite number
pub fn calculate(q: &str) -> Option<f64> {
    let expr: String = q.chars().filter(|c| !c.is_whitespace()).collect::<String>().replace('×', "*").replace('÷', "/").replace(',', "");
    if !expr.chars().any(|c| c.is_ascii_digit()) {
        return None;
    }
    if !(expr.chars().any(|c| "-+*/^%()".contains(c)) || expr.contains("sqrt") || expr.contains("pi")) {
        return None;
    }
    // (?:[0-9.+\-*/%^()]|sqrt|sin|cos|tan|log|ln|pi|e)+ : no word is another's prefix
    let mut rest = expr.as_str();
    while !rest.is_empty() {
        let c = rest.chars().next().unwrap();
        if c.is_ascii_digit() || ".+-*/%^()".contains(c) {
            rest = &rest[1..];
            continue;
        }
        match ["sqrt", "sin", "cos", "tan", "log", "ln", "pi", "e"].iter().find(|w| rest.starts_with(**w)) {
            Some(w) => rest = &rest[w.len()..],
            None => return None,
        }
    }
    let js = expr
        .replace('^', "**")
        .replace("sqrt", "Math.sqrt")
        .replace("sin", "Math.sin")
        .replace("cos", "Math.cos")
        .replace("tan", "Math.tan")
        .replace("log", "Math.log10")
        .replace("ln", "Math.log")
        .replace("pi", "Math.PI");
    // (^|[^a-zA-Z.])e(?![a-zA-Z]) -> Math.E
    let c: Vec<char> = js.chars().collect();
    let mut s = String::new();
    for (i, &ch) in c.iter().enumerate() {
        let before_ok = i == 0 || !(c[i - 1].is_ascii_alphabetic() || c[i - 1] == '.');
        let after_ok = !c.get(i + 1).is_some_and(|n| n.is_ascii_alphabetic());
        if ch == 'e' && before_ok && after_ok {
            s.push_str("Math.E");
        } else {
            s.push(ch);
        }
    }
    let mut p = Parser { t: tokens(&s)?, i: 0 };
    let v = Parser::num(p.additive()?)?;
    if p.i != p.t.len() || !v.is_finite() {
        return None;
    }
    let r = js_round(v * 1e10) / 1e10;
    r.is_finite().then_some(r)
}

// ── Words: conversions and definitions ─────────────────────────────────────

/// Search.looksLikeConversion: "10 km to mi", "$20 in eur", "72 f to c"
/// (/^[-$€£¥₹]?\s*\d[\d.,]*\s*\S.*\s(to|in|as|into)\s+\S+$/i)
pub fn looks_like_conversion(q: &str) -> bool {
    // the last word, the keyword before it, and what's before that
    let Some(last_ws) = q.char_indices().filter(|(_, c)| c.is_whitespace()).map(|(i, c)| i + c.len_utf8()).last() else { return false };
    if last_ws >= q.len() {
        return false;
    }
    let s1 = q[..last_ws].trim_end();
    let Some((k_at, k_ch)) = s1.char_indices().filter(|(_, c)| c.is_whitespace()).last() else { return false };
    let keyword = s1[k_at + k_ch.len_utf8()..].to_lowercase();
    if !["to", "in", "as", "into"].contains(&keyword.as_str()) {
        return false;
    }
    let head = &s1[..k_at];
    let mut it = head.chars().peekable();
    if it.peek().is_some_and(|c| "-$€£¥₹".contains(*c)) {
        it.next();
    }
    while it.peek().is_some_and(|c| c.is_whitespace()) {
        it.next();
    }
    if !it.next().is_some_and(|c| c.is_ascii_digit()) {
        return false;
    }
    it.any(|c| !c.is_whitespace())
}

fn is_word_char(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '\'' || c == '-'
}

/// A word that starts here ([a-z][a-z'-]*), and the rest
fn word_at(s: &str) -> Option<(&str, &str)> {
    if !s.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    let end = s.find(|c: char| !is_word_char(c)).unwrap_or(s.len());
    Some((&s[..end], &s[end..]))
}

/// Strip a case-insensitive ASCII prefix
fn strip_ci<'a>(s: &'a str, p: &str) -> Option<&'a str> {
    (s.len() >= p.len() && s.is_char_boundary(p.len()) && s[..p.len()].eq_ignore_ascii_case(p)).then(|| &s[p.len()..])
}

/// Search.wordToDefine: "define word", "what does word mean", "word meaning"
pub fn word_to_define(q: &str) -> String {
    for p in ["define", "definition of", "meaning of", "what does"] {
        let Some(rest) = strip_ci(q, p) else { continue };
        if !rest.starts_with(char::is_whitespace) {
            continue;
        }
        let Some((w, tail)) = word_at(rest.trim_start()) else { continue };
        let tail = tail.strip_suffix('?').unwrap_or(tail);
        let ok = tail.is_empty() || (tail.starts_with(char::is_whitespace) && tail.trim_start().eq_ignore_ascii_case("mean"));
        if ok {
            return w.to_lowercase();
        }
    }
    if let Some((w, tail)) = word_at(q) {
        if tail.starts_with(char::is_whitespace) && ["meaning", "definition", "define"].iter().any(|k| tail.trim_start().eq_ignore_ascii_case(k)) {
            return w.to_lowercase();
        }
    }
    String::new()
}

// ── Actions ────────────────────────────────────────────────────────────────

/// Search.qml's actions (title, glyph, keywords): they run in the shell
pub const ACTIONS: &[(&str, &str, &str)] = &[
    ("Lock Screen", "lock", "lock"),
    ("Sleep…", "moon", "suspend sleep"),
    ("Restart…", "arrow-clockwise", "reboot restart"),
    ("Shut Down…", "power", "shutdown poweroff power off"),
    ("Log Out…", "sign-out", "logout exit"),
    ("Send Files…", "paper-plane-tilt", "share send files localsend nearby quickshare quick share android phone"),
    ("Receive with Quick Share", "share-network", "quickshare quick share nearby share android phone receive visible"),
    ("Toggle Dark Mode", "circle-half", "dark light appearance theme"),
    ("Quick Settings", "sliders-horizontal", "control center quick settings toggles"),
    ("Notification Center", "bell", "notifications calendar"),
    ("Ask AI", "sparkle", "ai chat claude assistant gpt gemini"),
    ("Ask AI About the Screen", "sparkle", "ai ask screen screenshot explain what is this error"),
    ("Translate", "translate", "translate translator language"),
    ("System Settings", "gear-six", "settings preferences config options"),
    ("Take Screenshot", "camera", "screenshot capture screen grab"),
    ("Copy Text from Screen", "text-t", "ocr text recognize copy screen"),
    ("Record Screen", "record", "record recording video screencast"),
    ("Keyboard Shortcuts", "keyboard", "shortcuts keybinds cheatsheet keys"),
    ("Periodic Table", "atom", "periodic table elements chemistry"),
    ("On-Screen Keyboard", "keyboard", "osk keyboard virtual touch"),
    ("Clipboard History", "clipboard-text", "clipboard history paste copied pins"),
    ("Overlay Widgets", "squares-four", "overlay widgets notes crosshair resources game"),
    ("Identify Song", "music-notes", "song recognize shazam songrec music what playing"),
    ("Run Health Check", "heartbeat", "health check status services broken diagnose troubleshoot doctor"),
    ("Report a Bug…", "bug", "bug report issue problem crash broken feedback"),
    ("Check for Updates", "arrow-circle-up", "update upgrade pacman packages"),
    ("Change Wallpaper", "image", "wallpaper background desktop picture"),
    ("Show Today", "sun-horizon", "today morning greeting day events to-dos weather options"),
    ("Screen Recording Options", "record", "record recording screen video frame rate fps format quality codec microphone sound countdown folder options"),
    ("Stages", "squares-four", "stage manager stages windows group strip"),
    ("Software", "storefront", "app store install apps software get download flathub aur packages"),
    ("App Drawer", "squares-four", "apps launchpad app library app drawer all applications grid"),
    ("All Windows", "squares-four", "overview workspaces windows expose mission control all windows"),
];

pub fn actions() -> Vec<Action> {
    ACTIONS.iter().map(|(t, g, k)| Action { title: (*t).into(), glyph: (*g).into(), keywords: (*k).into() }).collect()
}

/// Search.aiRow
pub fn ai_row(question: &str, ai: Option<&AiMeta>) -> Row {
    let configured = ai.is_some_and(|a| a.configured);
    let name = ai.map(|a| a.name.as_str()).unwrap_or("");
    // (the model's name without its "(…)" note)
    let short = match name.find(" (") {
        Some(i) if name.ends_with(')') => &name[..i],
        _ => name,
    };
    Row {
        section: "Ask AI".into(),
        title: if configured { format!("Ask {short}") } else { "Set up AI…".into() },
        subtitle: format!("“{question}”"),
        glyph: "sparkle".into(),
        ai: configured,
        run: if configured { Run::Ask(question.into()) } else { Run::SetupAi },
        ..Default::default()
    }
}

/// Search.iconForFile
pub fn icon_for_file(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "svg" => "image-x-generic",
        "mp4" | "mkv" | "webm" | "mov" => "video-x-generic",
        "mp3" | "flac" | "ogg" | "wav" | "m4a" => "audio-x-generic",
        "pdf" => "application-pdf",
        "zip" | "tar" | "gz" | "xz" | "7z" | "rar" => "package-x-generic",
        _ => "text-x-generic",
    }
}

fn file_row(section: &str, path: &str, dir: bool, home: &str) -> Row {
    let (folder, name) = match path.rfind('/') {
        Some(i) => (&path[..i], &path[i + 1..]),
        None => ("", path),
    };
    let sub = if !home.is_empty() { folder.replacen(home, "~", 1) } else { folder.to_string() };
    Row {
        section: section.into(),
        title: name.into(),
        subtitle: sub,
        icon: if dir { "folder".into() } else { icon_for_file(path).into() },
        icon_fallback: "text-x-generic",
        path: path.into(),
        run: Run::File(path.into()),
        ..Default::default()
    }
}

/// The first letter in upper case
fn cap(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

/// Search.query: the rows for what's typed, and what's still wanted
pub fn query(text: &str, k: &Known) -> (Vec<Row>, Wants) {
    let mut w = Wants::default();
    let raw = text.trim();
    let q = raw.to_lowercase();
    if q.is_empty() {
        return (Vec::new(), w);
    }

    if let Some(aq) = raw.strip_prefix('?') {
        let aq = aq.trim();
        return (if aq.is_empty() { Vec::new() } else { vec![ai_row(aq, k.ai)] }, w);
    }

    if let Some(cmd) = raw.strip_prefix('>') {
        let cmd = cmd.trim();
        if cmd.is_empty() {
            return (Vec::new(), w);
        }
        return (vec![Row { section: "Command".into(), title: cmd.into(), subtitle: "Run in terminal".into(), glyph: "terminal-window".into(), run: Run::Terminal(cmd.into()), ..Default::default() }], w);
    }

    if let Some(eq) = raw.strip_prefix(':') {
        let eq = eq.trim().to_lowercase();
        if eq.is_empty() {
            return (Vec::new(), w);
        }
        let skin = eq.contains("skin");
        let mut list: Vec<&(String, String)> = k.emoji.iter().filter(|e| (skin || !e.1.contains("skin tone")) && e.1.contains(&eq)).collect();
        let whole = |n: &str| n.split(' ').any(|x| x == eq) as i32;
        list.sort_by(|a, b| (whole(&b.1) - whole(&a.1)).cmp(&0).then(js_len(&a.1).cmp(&js_len(&b.1))));
        let rows = list
            .into_iter()
            .take(40)
            .map(|e| Row { section: "Emoji".into(), title: cap(&e.1), emoji: e.0.clone(), run: Run::Copy(e.0.clone()), ..Default::default() })
            .collect();
        return (rows, w);
    }

    if let Some(fq) = raw.strip_prefix('/') {
        let fq = fq.trim();
        if js_len(fq) < 2 {
            return (Vec::new(), w);
        }
        w.files = Some(fq.into());
        let mut rows: Vec<Row> = k.files.iter().map(|(d, p)| file_row("Files", p, *d, k.home)).collect();
        let named: std::collections::HashSet<&str> = k.files.iter().map(|f| f.1.as_str()).collect();
        rows.extend(k.contents.iter().filter(|p| !named.contains(p.as_str())).map(|p| file_row("File contents", p, false, k.home)));
        return (rows, w);
    }

    if raw.starts_with(';') {
        w.extras = Some((raw.into(), false));
        return (k.extras.map(|e| e.clipboard.clone()).unwrap_or_default(), w);
    }

    let empty = Extras::default();
    let x = k.extras.unwrap_or(&empty);
    let mut out: Vec<Row> = Vec::new();

    let value = calculate(raw);
    if let Some(v) = value {
        let s = js_str(v);
        out.push(Row { section: "Calculator".into(), title: s.clone(), subtitle: format!("{raw} ="), glyph: "calculator".into(), run: Run::Copy(s), ..Default::default() });
    }

    // (plain arithmetic is answered above, at once)
    if value.is_none() && js_len(raw) >= 3 {
        w.answer = Some(raw.into());
        if let Some(a) = k.answer.filter(|a| !a.title.is_empty()) {
            let glyph = match a.kind.as_str() {
                "math" => "calculator",
                "date" => "calendar-blank",
                "time" => "globe-hemisphere-west",
                "unit" => "ruler",
                "currency" => "currency-circle-dollar",
                _ => "sparkle",
            };
            out.push(Row { section: "Answer".into(), kind: a.kind.clone(), title: a.title.clone(), subtitle: a.subtitle.clone(), glyph: glyph.into(), run: Run::Copy(a.copy.clone()), ..Default::default() });
        }
    }
    let answered = k.answer.is_some_and(|a| !a.title.is_empty());

    // (units and currency it didn't know: qalc, when installed)
    if value.is_none() && k.answer.is_some_and(|a| a.title.is_empty()) && k.qalc && looks_like_conversion(raw) {
        w.conversion = Some(raw.into());
        if let Some(r) = k.conversion.filter(|r| !r.is_empty()) {
            // (the number of the answer: what's after any leading words, up to a space)
            let n = r.trim_start_matches(|c: char| !(c.is_ascii_digit() || c == '−' || c == '-'));
            let n = n.split(' ').next().unwrap_or("");
            out.push(Row { section: "Conversion".into(), title: r.into(), subtitle: raw.into(), glyph: "arrows-left-right".into(), run: Run::Copy(n.into()), ..Default::default() });
        }
    }

    let word = word_to_define(raw);
    if !word.is_empty() {
        w.define = Some(word.clone());
        // (the part passes the definition of this very word)
        if let Some(d) = k.definition.filter(|d| !d.missing) {
            let title = if d.part.is_empty() { d.word.clone() } else { format!("{} · {}", d.word, d.part) };
            out.push(Row { section: "Dictionary".into(), title, subtitle: d.text.clone(), glyph: "book-open".into(), run: Run::Url(format!("https://en.wiktionary.org/wiki/{}", crate::surfaces::emoji::klipy::encode(&d.word))), ..Default::default() });
        }
    }

    // reminders in plain words (Calendar.parseQuick, the shell's)
    out.extend(x.reminder.iter().cloned());

    let mut apps: Vec<(&App, f64)> = k
        .apps
        .iter()
        .map(|a| (a, score(&a.name, &q).max(score(&a.generic, &q) * 0.8).max(score(&a.keywords.join(" "), &q) * 0.6).max(score(&a.id, &q) * 0.7)))
        .filter(|x| x.1 >= 30.0)
        .collect();
    apps.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    apps.truncate(8);
    for (a, s) in &apps {
        let sub = if !a.generic.is_empty() { a.generic.clone() } else { a.comment.clone() };
        out.push(Row { section: "Applications".into(), title: a.name.clone(), subtitle: sub, icon: a.icon.clone(), icon_fallback: "application-x-executable", run: Run::App(a.id.clone()), score: *s, ..Default::default() });
    }

    let mut acts: Vec<(&Action, f64)> = k.actions.iter().map(|a| (a, score(&a.title, &q).max(score(&a.keywords, &q) * 0.9))).filter(|x| x.1 > 0.0).collect();
    acts.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    acts.truncate(4);
    for (a, s) in &acts {
        out.push(Row { section: "Actions".into(), title: a.title.clone(), glyph: a.glyph.clone(), run: Run::Action(a.title.clone()), score: *s, ..Default::default() });
    }

    // An app you don't have, from the App Store: only when nothing here
    // answers it (the shell adds: the App Store is there, no plain-language request)
    let aur = strip_ci(raw, "aur:").is_some();
    let store = js_len(&q) >= 3
        && raw.split_whitespace().count() <= 4
        && value.is_none()
        && !answered
        && (aur || (apps.first().map_or(0.0, |a| a.1) < 80.0 && acts.first().map_or(0.0, |a| a.1) < 80.0));
    w.extras = Some((raw.into(), store));
    if store {
        out.extend(x.store.iter().cloned());
    }
    out.extend(x.calendar.iter().cloned());

    if js_len(raw) >= 3 {
        out.push(ai_row(raw, k.ai));
    }
    out.push(Row {
        section: "Web".into(),
        title: format!("Search the web for “{raw}”"),
        glyph: "globe".into(),
        run: Run::Url(format!("https://duckduckgo.com/?q={}", crate::surfaces::emoji::klipy::encode(raw))),
        ..Default::default()
    });
    out.extend(x.contacts.iter().cloned());
    out.extend(x.notes.iter().cloned());
    out.extend(x.shots.iter().cloned());

    // Plain-language actions come first; otherwise the best app or action
    // becomes the top hit (answers stay on top)
    if !x.intents.is_empty() {
        let mut all = x.intents.clone();
        all.extend(out);
        return (all, w);
    }
    (top_hit(out), w)
}

/// The best app or action becomes the top hit; answers stay on top; with
/// nothing here, an app from the App Store
pub fn top_hit(mut out: Vec<Row>) -> Vec<Row> {
    let is_answer = |r: &Row| ANSWERS.contains(&r.section.as_str());
    let mut top = out.iter().position(is_answer);
    if top.is_none() {
        // (letters merely in order don't make a top hit: it has to contain what was typed)
        for (i, r) in out.iter().enumerate() {
            if (r.section == "Applications" || r.section == "Actions") && r.score >= 50.0 && top.is_none_or(|t| r.score > out[t].score) {
                top = Some(i);
            }
        }
    }
    if top.is_none() {
        top = out.iter().position(|r| r.section == "Software" && !r.loading);
    }
    if let Some(t) = top {
        if t > 0 || !is_answer(&out[0]) {
            let mut hit = out.remove(t);
            hit.section = "Top Hit".into();
            out.insert(0, hit);
        }
    }
    out
}

/// Keep the row the selection was on (by title) when the rows change; else
/// the first (SearchModel's revisions)
pub fn keep_selection(old: &[Row], sel: usize, new: &[Row]) -> usize {
    if sel == 0 {
        return 0;
    }
    let Some(t) = old.get(sel).map(|r| &r.title) else { return 0 };
    new.iter().position(|r| &r.title == t).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(id: &str, name: &str, generic: &str, kw: &[&str]) -> App {
        App { id: id.into(), name: name.into(), generic: generic.into(), keywords: kw.iter().map(|s| s.to_string()).collect(), ..Default::default() }
    }

    fn titles(r: &[Row]) -> Vec<(String, String)> {
        r.iter().map(|r| (r.section.clone(), r.title.clone())).collect()
    }

    #[test]
    fn prefixes() {
        let k = Known::default();
        let (r, _) = query(">  ls -la ", &k);
        assert_eq!(r[0].run, Run::Terminal("ls -la".into()));
        assert_eq!(r[0].subtitle, "Run in terminal");
        assert!(query(">", &k).0.is_empty());
        let ai = AiMeta { configured: true, name: "Claude Sonnet (Anthropic)".into() };
        let k = Known { ai: Some(&ai), ..Default::default() };
        let (r, _) = query("? why is the sky blue", &k);
        assert_eq!((r[0].title.as_str(), r[0].subtitle.as_str(), r[0].ai), ("Ask Claude Sonnet", "“why is the sky blue”", true));
        let (r, _) = query("?x", &Known::default());
        assert_eq!((r[0].title.as_str(), &r[0].run), ("Set up AI…", &Run::SetupAi));
        let (r, w) = query("/ a", &Known::default());
        assert!(r.is_empty() && w.files.is_none());
        let files = vec![(false, "/home/u/Documents/notes.md".to_string()), (true, "/home/u/Notes".to_string())];
        let contents = vec!["/home/u/Documents/notes.md".to_string(), "/home/u/Desktop/plan.txt".to_string()];
        let k = Known { files: &files, contents: &contents, home: "/home/u", ..Default::default() };
        let (r, w) = query("/notes", &k);
        assert_eq!(w.files.as_deref(), Some("notes"));
        assert_eq!(titles(&r), vec![("Files".into(), "notes.md".into()), ("Files".into(), "Notes".into()), ("File contents".into(), "plan.txt".into())]);
        assert_eq!((r[0].subtitle.as_str(), r[1].icon.as_str(), r[2].icon.as_str()), ("~/Documents", "folder", "text-x-generic"));
    }

    #[test]
    fn emoji_by_name() {
        let e: Vec<(String, String)> = [("🐱", "cat face"), ("🐈", "cat"), ("😺", "grinning cat"), ("👍🏽", "thumbs up: medium skin tone"), ("🎉", "party popper"), ("🙀", "weary cat"), ("😸", "grinning cat with smiling eyes"), ("🛒", "shopping cart")]
            .iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect();
        let k = Known { emoji: &e, ..Default::default() };
        let (r, _) = query(":cat", &k);
        // whole word first, then shorter names
        assert_eq!(r.iter().map(|r| r.title.as_str()).collect::<Vec<_>>(), vec!["Cat", "Cat face", "Weary cat", "Grinning cat", "Grinning cat with smiling eyes"]);
        assert_eq!(r[0].run, Run::Copy("🐈".into()));
        assert!(query(":thumbs", &k).0.is_empty());
        assert_eq!(query(":up: medium skin", &k).0.len(), 1);
    }

    #[test]
    fn apps_actions_and_the_top_hit() {
        let apps = vec![
            app("firefox", "Firefox", "Web Browser", &["internet", "www"]),
            app("org.kde.dolphin", "Dolphin", "File Manager", &[]),
            app("org.kde.krita", "Krita", "Digital Painting", &["paint"]),
            app("chromium", "Chromium", "Web Browser", &[]),
        ];
        let acts = actions();
        let ai = AiMeta { configured: false, name: "Add a model".into() };
        let k = Known { apps: &apps, actions: &acts, ai: Some(&ai), ..Default::default() };
        let (r, w) = query("fire", &k);
        assert_eq!(titles(&r[..1]), vec![("Top Hit".into(), "Firefox".into())]);
        assert_eq!(r[0].run, Run::App("firefox".into()));
        assert_eq!(r[0].subtitle, "Web Browser");
        assert_eq!(w.answer.as_deref(), Some("fire"));
        // apps found by their generic name (a tie: the first listed leads), the AI row and the web search after
        let (r, _) = query("browser", &k);
        let t = titles(&r);
        assert_eq!(t[0], ("Top Hit".into(), "Firefox".into()));
        assert_eq!(t[1], ("Applications".into(), "Chromium".into()));
        assert_eq!(t[t.len() - 2], ("Ask AI".into(), "Set up AI…".into()));
        assert_eq!(t[t.len() - 1], ("Web".into(), "Search the web for “browser”".into()));
        // an action named for it beats an app found only by keywords
        let (r, _) = query("lock", &k);
        assert_eq!(titles(&r[..1]), vec![("Top Hit".into(), "Lock Screen".into())]);
        // letters merely in order: no top hit
        let (r, _) = query("krt", &k);
        assert!(r.iter().all(|r| r.section != "Top Hit"));
        // a sum stays on top
        let (r, w) = query("2*3", &k);
        assert_eq!(titles(&r[..1]), vec![("Calculator".into(), "6".into())]);
        assert_eq!(r[0].subtitle, "2*3 =");
        assert_eq!(w.answer, None);
        assert_eq!(w.extras, Some(("2*3".into(), false)));
    }

    #[test]
    fn slow_answers_join_in_their_places() {
        let apps = vec![app("org.gnome.Calendar", "Calendar", "", &[])];
        let acts = actions();
        let answer = Answer { kind: "date".into(), title: "51 days".into(), subtitle: "Until Christmas".into(), copy: "51".into() };
        let none = Answer::default();
        let def = Definition { word: "serendipity".into(), part: "noun".into(), text: "Luck".into(), ..Default::default() };
        let x = Extras {
            q: "days until christmas".into(),
            intents: vec![],
            calendar: vec![Row { section: "Calendar".into(), title: "Christmas party".into(), run: Run::Shell("r0".into()), ..Default::default() }],
            notes: vec![Row { section: "Notes".into(), title: "Christmas list".into(), ..Default::default() }],
            ..Default::default()
        };
        let k = Known { apps: &apps, actions: &acts, answer: Some(&answer), extras: Some(&x), ..Default::default() };
        let (r, w) = query("days until christmas", &k);
        let t: Vec<&str> = r.iter().map(|r| r.section.as_str()).collect();
        assert_eq!(t, vec!["Answer", "Calendar", "Ask AI", "Web", "Notes"]);
        assert_eq!(r[0].run, Run::Copy("51".into()));
        assert_eq!(r[0].glyph, "calendar-blank");
        // (an answer: no App Store)
        assert_eq!(w.extras, Some(("days until christmas".into(), false)));
        // no answer: a conversion with qalc
        let k2 = Known { answer: Some(&none), qalc: true, conversion: Some("6.2137119 mi"), ..Default::default() };
        let (r, w) = query("10 km to mi", &k2);
        assert_eq!(w.conversion.as_deref(), Some("10 km to mi"));
        assert_eq!((r[0].section.as_str(), &r[0].run), ("Conversion", &Run::Copy("6.2137119".into())));
        let (_, w) = query("10 km to mi", &Known { qalc: true, ..Default::default() });
        assert_eq!(w.conversion, None); // (asks answers.py first)
        // a definition
        let k3 = Known { definition: Some(&def), ..Default::default() };
        let (r, w) = query("define serendipity", &k3);
        assert_eq!(w.define.as_deref(), Some("serendipity"));
        assert_eq!((r[0].section.as_str(), r[0].title.as_str()), ("Dictionary", "serendipity · noun"));
        assert_eq!(r[0].run, Run::Url("https://en.wiktionary.org/wiki/serendipity".into()));
    }

    #[test]
    fn plain_language_requests_come_first_and_the_store_when_nothing_matches() {
        let x = Extras {
            q: "timer 5 min".into(),
            intents: vec![Row { section: "Do".into(), title: "Start a 5 min timer".into(), run: Run::Shell("r0".into()), ..Default::default() }],
            ..Default::default()
        };
        let k = Known { extras: Some(&x), ..Default::default() };
        let (r, _) = query("timer 5 min", &k);
        assert_eq!(r[0].section, "Do");
        assert!(r.iter().all(|r| r.section != "Top Hit"));
        // the App Store: asked when no app or action starts with it
        let acts = actions();
        let store = Extras {
            q: "gimp".into(),
            store: vec![
                Row { section: "Software".into(), title: "GIMP".into(), subtitle: "Get · Flathub".into(), ..Default::default() },
                Row { section: "Software".into(), title: "Looking on Flathub…".into(), loading: true, ..Default::default() },
            ],
            ..Default::default()
        };
        let k = Known { actions: &acts, extras: Some(&store), ..Default::default() };
        let (r, w) = query("gimp", &k);
        assert_eq!(w.extras, Some(("gimp".into(), true)));
        assert_eq!(titles(&r[..1]), vec![("Top Hit".into(), "GIMP".into())]);
        assert!(r.iter().any(|r| r.loading && r.section == "Software"));
        // more than four words, or an action starting with it: no store
        assert_eq!(query("a b c d e", &k).1.extras, Some(("a b c d e".into(), false)));
        assert_eq!(query("lock", &k).1.extras, Some(("lock".into(), false)));
        assert_eq!(query("aur: lock", &k).1.extras, Some(("aur: lock".into(), true)));
    }

    #[test]
    fn the_selection_stays_on_its_row() {
        let r = |t: &[&str]| t.iter().map(|t| Row { title: t.to_string(), ..Default::default() }).collect::<Vec<_>>();
        assert_eq!(keep_selection(&r(&["a", "b", "c"]), 2, &r(&["x", "a", "c"])), 2);
        assert_eq!(keep_selection(&r(&["a", "b", "c"]), 1, &r(&["x", "a", "c"])), 0);
        assert_eq!(keep_selection(&r(&["a", "b"]), 0, &r(&["b", "a"])), 0);
    }

    /// Search.qml's own functions under node (golden.js) gave these
    #[test]
    fn ranks_as_search_qml_does() {
        let g: serde_json::Value = serde_json::from_str(include_str!("golden.json")).unwrap();
        let s = |v: &serde_json::Value| v.as_str().unwrap_or("").to_string();
        for c in g["score"].as_array().unwrap() {
            assert_eq!(score(&s(&c[0]), &s(&c[1])), c[2].as_f64().unwrap(), "score({}, {})", c[0], c[1]);
        }
        for c in g["calc"].as_array().unwrap() {
            assert_eq!(calculate(&s(&c[0])).map(js_str), c[1].as_str().map(String::from), "calculate({})", c[0]);
        }
        for c in g["conversion"].as_array().unwrap() {
            assert_eq!(looks_like_conversion(&s(&c[0])), c[1].as_bool().unwrap(), "{}", c[0]);
        }
        for c in g["define"].as_array().unwrap() {
            assert_eq!(word_to_define(&s(&c[0])), s(&c[1]), "{}", c[0]);
        }
        let acts: Vec<Vec<String>> = actions().iter().map(|a| vec![a.title.clone(), a.glyph.clone(), a.keywords.clone()]).collect();
        let want: Vec<Vec<String>> = g["actions"].as_array().unwrap().iter().map(|a| a.as_array().unwrap().iter().map(s).collect()).collect();
        assert_eq!(acts, want);
        let apps: Vec<App> = g["apps"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|a| !a["noDisplay"].as_bool().unwrap_or(false))
            .map(|a| App { id: s(&a["id"]), name: s(&a["name"]), generic: s(&a["generic"]), comment: s(&a["comment"]), keywords: a["keywords"].as_array().unwrap().iter().map(s).collect(), icon: String::new() })
            .collect();
        let acts = actions();
        let ai = AiMeta { configured: false, name: "Add a model".into() };
        let k = Known { apps: &apps, actions: &acts, ai: Some(&ai), emoji: super::super::providers::emoji(), home: "/home/u", ..Default::default() };
        for c in g["cases"].as_array().unwrap() {
            let (rows, _) = query(&s(&c["q"]), &k);
            let got: Vec<Vec<String>> = rows.iter().map(|r| vec![r.section.clone(), r.title.clone(), r.subtitle.clone()]).collect();
            let want: Vec<Vec<String>> = c["rows"].as_array().unwrap().iter().map(|r| r.as_array().unwrap().iter().map(s).collect()).collect();
            assert_eq!(got, want, "query({})", c["q"]);
        }
    }
}

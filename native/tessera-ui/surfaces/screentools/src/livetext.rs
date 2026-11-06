//! Live Text over a picture, shared by the screenshot editor and Quick Look
//! (compare shell/modules/screentools/editor/LiveTextLayer.qml and
//! LiveTextBar.qml, shell/common/LiveTextLogic.js): the words tesseract found
//! (read by the shell's LiveText service, `livetext.read`; its answer comes
//! back on stdin as `{"to": …, "livetext": key, "result": …}`) as boxes you
//! select like text on a phone: drag across words, click one, double-click
//! for its line, Shift+click to extend. The bar under the picture says what
//! it's doing and offers the links, addresses, numbers and Wi-Fi codes in it
//! as chips (LiveTextLogic.actions, ported below without a regex engine).

use serde_json::{json, Value};

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Word {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub text: String,
    pub block: String,
    pub line: String,
    /// the line's top and height (a selection reads as one band)
    pub ly: f32,
    pub lh: f32,
}

impl Word {
    pub fn from_json(v: &Value) -> Option<Self> {
        let f = |k: &str| v[k].as_f64().unwrap_or(0.0) as f32;
        let s = |k: &str| match &v[k] {
            Value::String(s) => s.clone(),
            Value::Null => String::new(),
            o => o.to_string(),
        };
        let text = s("text");
        if text.is_empty() {
            return None;
        }
        let (y, h) = (f("y"), f("h"));
        let ly = v["ly"].as_f64().map_or(y, |x| x as f32);
        let lh = v["lh"].as_f64().map_or(h, |x| x as f32);
        Some(Word { x: f("x"), y, w: f("w"), h, text, block: s("block"), line: s("line"), ly, lh })
    }
}

/// Words a..b (either order) as text: a space within a line, a line break
/// between lines, a blank line between blocks
pub fn text_of(words: &[Word], a: i32, b: i32) -> String {
    if words.is_empty() || a < 0 || b < 0 {
        return String::new();
    }
    let from = a.min(b).max(0) as usize;
    let to = (a.max(b) as usize).min(words.len() - 1);
    let mut s = String::new();
    for i in from..=to {
        let w = &words[i];
        if i > from {
            let p = &words[i - 1];
            s += if w.block != p.block {
                "\n\n"
            } else if w.line != p.line {
                "\n"
            } else {
                " "
            };
        }
        s += &w.text;
    }
    s
}

pub fn all_text(words: &[Word]) -> String {
    text_of(words, 0, words.len() as i32 - 1)
}

/// The word at (x, y) in picture pixels (`slack` round each box), else -1
pub fn word_at(words: &[Word], x: f32, y: f32, slack: f32) -> i32 {
    words.iter().position(|w| x >= w.x - slack && x <= w.x + w.w + slack && y >= w.y - slack && y <= w.y + w.h + slack).map_or(-1, |i| i as i32)
}

/// The word a drag reaches: on the line nearest to y, the word nearest to x
pub fn nearest_word(words: &[Word], x: f32, y: f32) -> i32 {
    let mut best = -1i32;
    let mut best_dy = f32::INFINITY;
    for (i, w) in words.iter().enumerate() {
        let dy = if y < w.y { w.y - y } else if y > w.y + w.h { y - w.y - w.h } else { 0.0 };
        if dy < best_dy {
            best_dy = dy;
            best = i as i32;
        }
    }
    if best < 0 {
        return -1;
    }
    let line = &words[best as usize].line;
    let mut pick = best;
    let mut best_dx = f32::INFINITY;
    for (i, w) in words.iter().enumerate() {
        if &w.line != line {
            continue;
        }
        let dx = if x < w.x { w.x - x } else if x > w.x + w.w { x - w.x - w.w } else { 0.0 };
        if dx < best_dx {
            best_dx = dx;
            pick = i as i32;
        }
    }
    pick
}

/// The first and last word of word i's line
pub fn line_of(words: &[Word], i: i32) -> (i32, i32) {
    if i < 0 || i as usize >= words.len() {
        return (-1, -1);
    }
    let i = i as usize;
    let (mut a, mut b) = (i, i);
    while a > 0 && words[a - 1].line == words[i].line {
        a -= 1;
    }
    while b + 1 < words.len() && words[b + 1].line == words[i].line {
        b += 1;
    }
    (a as i32, b as i32)
}

/// Words containing any word of `query` (what Search found a picture by)
pub fn matching(words: &[Word], query: &str) -> Vec<usize> {
    let terms: Vec<String> = query.to_lowercase().split_whitespace().map(str::to_string).collect();
    words.iter().enumerate().filter(|(_, w)| {
        let t = w.text.to_lowercase();
        terms.iter().any(|q| t.contains(q.as_str()))
    }).map(|(i, _)| i).collect()
}

// ── Quick actions (LiveTextLogic.actions) ───────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub struct Action {
    /// link | email | phone | wifi
    pub kind: String,
    pub value: String,
    pub label: String,
    /// a Wi-Fi code's own object, as the shell's WifiShare gave it
    pub wifi: Value,
}

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn starts_ci(s: &[char], at: usize, pat: &str) -> bool {
    let p: Vec<char> = pat.chars().collect();
    at + p.len() <= s.len() && s[at..at + p.len()].iter().zip(&p).all(|(a, b)| a.to_ascii_lowercase() == *b)
}

/// `\b(?:https?://|www\.)[^\s<>"'`]+` (any case): (start, end) in chars
fn find_links(s: &[char]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < s.len() {
        let boundary = i == 0 || !is_word(s[i - 1]);
        let head = if !boundary {
            0
        } else if starts_ci(s, i, "https://") {
            8
        } else if starts_ci(s, i, "http://") {
            7
        } else if starts_ci(s, i, "www.") {
            4
        } else {
            0
        };
        if head > 0 {
            let mut j = i + head;
            while j < s.len() && !s[j].is_whitespace() && !"<>\"'`".contains(s[j]) {
                j += 1;
            }
            if j > i + head {
                out.push((i, j));
                i = j;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// A link's host has a dot inside it and no `@` before its path
fn plain_web_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    let rest = if lower.starts_with("https://") {
        &url[8..]
    } else if lower.starts_with("http://") {
        &url[7..]
    } else {
        return false;
    };
    let end = rest.find(|c: char| c.is_whitespace() || "/?#@".contains(c)).unwrap_or(rest.len());
    if rest[end..].starts_with('@') {
        return false;
    }
    let host: Vec<char> = rest[..end].chars().collect();
    host.len() >= 3 && host[1..host.len() - 1].contains(&'.')
}

/// The emails (`[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}`)
fn find_emails(s: &[char]) -> Vec<(usize, usize)> {
    let local = |c: char| c.is_ascii_alphanumeric() || "._%+-".contains(c);
    let dom = |c: char| c.is_ascii_alphanumeric() || c == '-';
    let mut out = Vec::new();
    let mut from = 0;
    for at in 0..s.len() {
        if s[at] != '@' || at < from {
            continue;
        }
        let mut a = at;
        while a > from && local(s[a - 1]) {
            a -= 1;
        }
        if a == at {
            continue;
        }
        // the labels after it, dot-separated
        let mut labels: Vec<(usize, usize)> = Vec::new();
        let mut j = at + 1;
        loop {
            let st = j;
            while j < s.len() && dom(s[j]) {
                j += 1;
            }
            if j == st {
                break;
            }
            labels.push((st, j));
            if j < s.len() && s[j] == '.' && j + 1 < s.len() && dom(s[j + 1]) {
                j += 1;
            } else {
                break;
            }
        }
        // the last label (after at least one dot) that starts with two letters
        let end = (1..labels.len()).rev().find_map(|k| {
            let (st, en) = labels[k];
            let n = s[st..en].iter().take_while(|c| c.is_ascii_alphabetic()).count();
            (n >= 2).then_some(st + n)
        });
        if let Some(end) = end {
            out.push((a, end));
            from = end;
        }
    }
    out
}

/// Phone-like runs: `(?:\+|\(|\b)\d[\d ().\-‐-‒]{5,18}\d\b`
fn find_phones(s: &[char]) -> Vec<(usize, usize)> {
    let class = |c: char| c.is_ascii_digit() || " ().-".contains(c) || ('\u{2010}'..='\u{2012}').contains(&c);
    let mut out = Vec::new();
    let mut i = 0;
    'scan: while i < s.len() {
        let mut starts = Vec::new();
        if s[i] == '+' || s[i] == '(' {
            starts.push(i + 1);
        }
        if (i == 0 || !is_word(s[i - 1])) && s[i].is_ascii_digit() {
            starts.push(i);
        }
        for p in starts {
            if p >= s.len() || !s[p].is_ascii_digit() {
                continue;
            }
            let mut run = 0;
            while run < 18 && p + 1 + run < s.len() && class(s[p + 1 + run]) {
                run += 1;
            }
            for m in (5..=run).rev() {
                let d = p + 1 + m;
                if d < s.len() && s[d].is_ascii_digit() && (d + 1 == s.len() || !is_word(s[d + 1])) {
                    out.push((i, d + 1));
                    i = d + 1;
                    continue 'scan;
                }
            }
        }
        i += 1;
    }
    out
}

/// A run of digits that reads as a phone number (not a date, a version, an
/// address on the network, or one bare run)
pub fn looks_like_phone(raw: &str) -> bool {
    let digits = raw.chars().filter(|c| c.is_ascii_digit()).count();
    if !(7..=15).contains(&digits) {
        return false;
    }
    // a date: three groups of digits with -./ between them
    let groups: Vec<&str> = raw.split(|c| "-./".contains(c)).collect();
    if groups.len() == 3 && groups.iter().all(|g| !g.is_empty() && g.chars().all(|c| c.is_ascii_digit())) {
        let l: Vec<usize> = groups.iter().map(|g| g.len()).collect();
        if (l[0] == 4 && (1..=2).contains(&l[1]) && (1..=2).contains(&l[2])) || ((1..=2).contains(&l[0]) && (1..=2).contains(&l[1]) && (2..=4).contains(&l[2])) {
            return false;
        }
    }
    let seps = |c: char| " ()-".contains(c) || ('\u{2010}'..='\u{2012}').contains(&c);
    // a version or an IP address: digit.digits.digit and no other separators
    let c: Vec<char> = raw.chars().collect();
    let dotted = (0..c.len()).any(|i| {
        if !(c[i].is_ascii_digit() && c.get(i + 1) == Some(&'.')) {
            return false;
        }
        let mut j = i + 2;
        let st = j;
        while j < c.len() && c[j].is_ascii_digit() {
            j += 1;
        }
        j > st && c.get(j) == Some(&'.') && c.get(j + 1).is_some_and(|d| d.is_ascii_digit())
    });
    if dotted && !raw.chars().any(|x| " ()-".contains(x)) {
        return false;
    }
    raw.starts_with('+') || raw.chars().any(seps)
}

/// Links, addresses and numbers in found text, as one-tap actions
pub fn actions(text: &str, max: usize) -> Vec<Action> {
    let s: Vec<char> = text.chars().collect();
    let mut out: Vec<Action> = Vec::new();
    let mut taken: Vec<(usize, usize)> = Vec::new();
    let add = |out: &mut Vec<Action>, taken: &mut Vec<(usize, usize)>, kind: &str, value: String, label: String, at: (usize, usize)| {
        taken.push(at);
        if out.iter().any(|a| a.kind == kind && a.value == value) {
            return;
        }
        out.push(Action { kind: kind.into(), value, label, wifi: Value::Null });
    };
    for (a, b) in find_links(&s) {
        let m: String = s[a..b].iter().collect();
        let raw = m.trim_end_matches(|c: char| ".,;:!?)]}>'\"".contains(c)).to_string();
        let url = if raw.to_ascii_lowercase().starts_with("www.") { format!("https://{raw}") } else { raw.clone() };
        if plain_web_url(&url) {
            let lower = raw.to_ascii_lowercase();
            let label = if lower.starts_with("https://") { &raw[8..] } else if lower.starts_with("http://") { &raw[7..] } else { &raw[..] };
            add(&mut out, &mut taken, "link", url.clone(), label.trim_end_matches('/').to_string(), (a, b));
        } else {
            taken.push((a, b));
        }
    }
    let in_taken = |taken: &Vec<(usize, usize)>, a: usize, b: usize| taken.iter().any(|r| a < r.1 && b > r.0);
    for (a, b) in find_emails(&s) {
        if !in_taken(&taken, a, b) {
            let m: String = s[a..b].iter().collect();
            add(&mut out, &mut taken, "email", m.clone(), m, (a, b));
        }
    }
    for (a, b) in find_phones(&s) {
        let m: String = s[a..b].iter().collect();
        let raw = m.trim().to_string();
        if in_taken(&taken, a, b) || !looks_like_phone(&raw) {
            continue;
        }
        let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
        let value = if raw.starts_with('+') { format!("+{digits}") } else { digits };
        add(&mut out, &mut taken, "phone", value, raw, (a, b));
    }
    out.truncate(if max == 0 { 6 } else { max });
    out
}

/// Labels kept short on the chips
pub fn short(s: &str) -> String {
    if s.chars().count() > 22 {
        s.chars().take(21).collect::<String>() + "…"
    } else {
        s.to_string()
    }
}

// ── One picture's Live Text (LiveTextLayer + LiveTextBar's state) ────────

/// The shell's Live Text state it hears about (LiveText, ScreenTools,
/// Updates, WifiShare), sent with the screen tools' state
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ocr {
    pub available: bool,
    pub installing: bool,
    pub updates_running: bool,
    pub install_error: String,
    pub tel_handler: bool,
}

impl Ocr {
    pub fn from_json(v: &Value) -> Self {
        Ocr {
            available: v["ocrAvailable"].as_bool().unwrap_or(false),
            installing: v["ocrInstalling"].as_bool().unwrap_or(false),
            updates_running: v["updatesRunning"].as_bool().unwrap_or(false),
            install_error: v["ocrInstallError"].as_str().unwrap_or("").to_string(),
            tel_handler: v["telHandler"].as_bool().unwrap_or(false),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Live {
    pub active: bool,
    pub path: String,
    pub rev: String,
    /// "" | reading | ready | none | missing | error
    pub status: String,
    pub words: Vec<Word>,
    pub page: (f32, f32),
    pub wifi: Vec<Value>,
    pub anchor: i32,
    pub focus: i32,
    /// what Search found it by (selected once read)
    pub highlight: String,
    /// what was just copied (said for a moment)
    pub said: String,
    /// the found words' glow, 0..1 (set by the owner's animation)
    pub glow: f32,
    press: (f32, f32),
    dragged: bool,
    /// it just read them: glow once
    pub found: bool,
}

pub fn key_of(path: &str, rev: &str) -> String {
    format!("{path}#{rev}")
}

impl Live {
    pub fn new() -> Self {
        Live { anchor: -1, focus: -1, ..Default::default() }
    }
    pub fn key(&self) -> String {
        key_of(&self.path, &self.rev)
    }
    pub fn has_selection(&self) -> bool {
        self.anchor >= 0 && self.focus >= 0
    }
    pub fn sel(&self) -> (i32, i32) {
        (self.anchor.min(self.focus), self.anchor.max(self.focus))
    }
    pub fn clear(&mut self) {
        self.anchor = -1;
        self.focus = -1;
    }
    pub fn select_all(&mut self) {
        if !self.words.is_empty() {
            self.anchor = 0;
            self.focus = self.words.len() as i32 - 1;
        }
    }
    pub fn selected_text(&self) -> String {
        if self.has_selection() {
            text_of(&self.words, self.anchor, self.focus)
        } else {
            String::new()
        }
    }
    pub fn all_text(&self) -> String {
        all_text(&self.words)
    }

    /// Turned on (or its picture changed): read it. Returns the shell call
    /// to make (`livetext.read` [path, rev]), if any
    pub fn start(&mut self) -> Option<Value> {
        self.clear();
        self.words.clear();
        self.wifi.clear();
        self.said.clear();
        if !self.active || self.path.is_empty() {
            self.status.clear();
            return None;
        }
        self.status = "reading".into();
        Some(json!([self.path, self.rev]))
    }

    /// The shell's answer for a key; true when it was this one's
    pub fn done(&mut self, key: &str, r: &Value) -> bool {
        if key != self.key() || !self.active {
            return false;
        }
        self.page = (r["width"].as_f64().unwrap_or(0.0) as f32, r["height"].as_f64().unwrap_or(0.0) as f32);
        self.words = r["words"].as_array().map(|a| a.iter().filter_map(Word::from_json).collect()).unwrap_or_default();
        self.status = r["status"].as_str().unwrap_or("error").to_string();
        self.select_found();
        self.found = self.status == "ready";
        true
    }

    fn select_found(&mut self) {
        if self.highlight.trim().is_empty() {
            return;
        }
        let m = matching(&self.words, &self.highlight);
        let Some(&first) = m.first() else { return };
        let mut to = first;
        while m.contains(&(to + 1)) {
            to += 1;
        }
        self.anchor = first as i32;
        self.focus = to as i32;
    }

    /// The chips: links, addresses, numbers, then Wi-Fi codes
    pub fn actions(&self) -> Vec<Action> {
        let mut a = actions(&self.all_text(), 4);
        for w in &self.wifi {
            let ssid = w["ssid"].as_str().unwrap_or("").to_string();
            a.push(Action { kind: "wifi".into(), value: ssid.clone(), label: ssid, wifi: w.clone() });
        }
        a
    }

    // ── Selecting, in picture pixels (`sx`: display px per picture px) ──
    pub fn press(&mut self, x: f32, y: f32, shift: bool, sx: f32) {
        self.press = (x, y);
        self.dragged = false;
        let i = word_at(&self.words, x, y, 4.0 / sx.max(0.0001));
        if shift && self.has_selection() {
            self.focus = nearest_word(&self.words, x, y);
        } else if i >= 0 {
            self.anchor = i;
            self.focus = i;
        } else {
            self.clear();
        }
    }
    pub fn drag(&mut self, x: f32, y: f32, sx: f32, sy: f32) {
        if !self.dragged && ((x - self.press.0) * sx).hypot((y - self.press.1) * sy) < 4.0 {
            return;
        }
        self.dragged = true;
        if self.anchor < 0 {
            self.anchor = nearest_word(&self.words, self.press.0, self.press.1);
        }
        self.focus = nearest_word(&self.words, x, y);
    }
    pub fn double(&mut self, x: f32, y: f32, sx: f32) {
        let i = word_at(&self.words, x, y, 4.0 / sx.max(0.0001));
        if i >= 0 {
            let (a, b) = line_of(&self.words, i);
            self.anchor = a;
            self.focus = b;
        }
    }
    pub fn over_word(&self, x: f32, y: f32, sx: f32) -> bool {
        word_at(&self.words, x, y, 4.0 / sx.max(0.0001)) >= 0
    }
}

/// The part of the picture shown (`region`, picture px; empty: all of it),
/// drawn at `dw` × `dh`: (shown rect, sx, sy)
pub fn mapping(live: &Live, region: (f32, f32, f32, f32), dw: f32, dh: f32) -> ((f32, f32, f32, f32), f32, f32) {
    let shown = if region.2 > 0.0 { region } else { (0.0, 0.0, live.page.0, live.page.1) };
    let sx = if shown.2 > 0.0 { dw / shown.2 } else { 1.0 };
    let sy = if shown.3 > 0.0 { dh / shown.3 } else { 1.0 };
    (shown, sx, sy)
}

/// A word's box as drawn: (x, y, w, h, selected, joined to the one before,
/// joined to the next)
pub fn boxes(live: &Live, region: (f32, f32, f32, f32), dw: f32, dh: f32) -> Vec<(f32, f32, f32, f32, bool, bool, bool)> {
    if live.status != "ready" {
        return Vec::new();
    }
    let (shown, sx, sy) = mapping(live, region, dw, dh);
    let (from, to) = live.sel();
    let has = live.has_selection();
    let w = &live.words;
    (0..w.len())
        .map(|i| {
            let m = &w[i];
            let ii = i as i32;
            let selected = has && ii >= from && ii <= to;
            let joined = selected && ii < to && w.get(i + 1).is_some_and(|n| n.line == m.line);
            let before = selected && ii > from && i > 0 && w[i - 1].line == m.line;
            let right = if joined { w[i + 1].x } else { m.x + m.w };
            ((m.x - shown.0) * sx - 2.0, (m.ly - shown.1) * sy - 1.0, (right - m.x) * sx + if joined { 0.0 } else { 4.0 }, m.lh * sy + 2.0, selected, before, joined)
        })
        .collect()
}

/// What the bar says (LiveTextBar.message) and its glyph's name
pub fn bar_message(live: &Live, ocr: &Ocr) -> (String, &'static str) {
    let st = live.status.as_str();
    let msg = if !live.said.is_empty() {
        live.said.clone()
    } else {
        match st {
            "reading" => "Finding text…".into(),
            "none" => "No text found in this picture".into(),
            "missing" if !ocr.install_error.is_empty() => format!("Couldn't install it: {}", ocr.install_error),
            "missing" => "Text recognition needs Tesseract".into(),
            "error" => "Couldn't read the text in this picture".into(),
            _ if live.has_selection() => {
                let (a, b) = live.sel();
                format!("{} {}", b - a + 1, if a == b { "word selected" } else { "words selected" })
            }
            _ => "Drag across text to select it".into(),
        }
    };
    let glyph = if !live.said.is_empty() {
        "check"
    } else {
        match st {
            "reading" => "circle-notch",
            "ready" | "none" => "text-aa",
            "" => "",
            _ => "warning-circle",
        }
    };
    (msg, glyph)
}

/// A chip's label (LiveTextBar.chipLabel)
pub fn chip_label(a: &Action, ocr: &Ocr) -> String {
    match a.kind.as_str() {
        "wifi" if a.wifi["joinable"].as_bool().unwrap_or(false) => format!("Join {}", short(&a.label)),
        "wifi" => "Copy Wi-Fi password".into(),
        "phone" if !ocr.tel_handler => format!("Copy {}", short(&a.label)),
        _ => short(&a.label),
    }
}

pub fn chip_tooltip(a: &Action, ocr: &Ocr) -> &'static str {
    match a.kind.as_str() {
        "link" => "Open in your browser",
        "email" => "Write an email",
        "phone" if ocr.tel_handler => "Call",
        "phone" => "Copy the number",
        _ if a.wifi["joinable"].as_bool().unwrap_or(false) => "Join this network",
        _ => "Copy the password",
    }
}

pub fn chip_icon(kind: &str) -> &'static str {
    match kind {
        "email" => "envelope",
        "phone" => "phone",
        "wifi" => "wifi-high",
        _ => "link",
    }
}

/// What a chip's tap says when it copied instead of opening ("" when it
/// opened something: LiveText.act's answer)
pub fn act_says(a: &Action, ocr: &Ocr) -> &'static str {
    match a.kind.as_str() {
        "phone" if !ocr.tel_handler => "Number copied",
        "wifi" if !a.wifi["joinable"].as_bool().unwrap_or(false) => "Password copied",
        _ => "",
    }
}

/// The action as the shell's LiveText.act takes it
pub fn act_json(a: &Action) -> Value {
    json!({"kind": a.kind, "value": a.value, "label": a.label, "wifi": a.wifi})
}

/// How many chips fit in `room` px beside the rest (each about as wide as
/// an EditorButton with an icon and its label; the label's width estimated
/// from its characters, Slint gives Rust no text metrics)
pub fn chips_that_fit(labels: &[String], room: f32, material: bool) -> usize {
    let per_char = if material { 7.4 } else { 7.0 };
    let mut used = 0.0;
    let mut n = 0;
    for l in labels {
        used += l.chars().count() as f32 * per_char + 16.0 + 6.0 + if material { 32.0 } else { 28.0 } + 8.0;
        if used > room {
            break;
        }
        n += 1;
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(x: f32, y: f32, text: &str, block: &str, line: &str) -> Word {
        Word { x, y, w: 40.0, h: 12.0, text: text.into(), block: block.into(), line: line.into(), ly: y, lh: 12.0 }
    }

    fn page() -> Vec<Word> {
        vec![w(0.0, 0.0, "Invoice", "1", "1.1.1"), w(50.0, 0.0, "total", "1", "1.1.1"), w(0.0, 20.0, "Due", "1", "1.1.2"), w(0.0, 60.0, "Thanks", "2", "2.1.1")]
    }

    #[test]
    fn text_of_a_selection() {
        let p = page();
        assert_eq!(text_of(&p, 0, 1), "Invoice total");
        assert_eq!(text_of(&p, 1, 0), "Invoice total");
        assert_eq!(all_text(&p), "Invoice total\nDue\n\nThanks");
        assert_eq!(text_of(&p, -1, 2), "");
        assert_eq!(text_of(&[], 0, 0), "");
    }

    #[test]
    fn words_under_the_pointer() {
        let p = page();
        assert_eq!(word_at(&p, 10.0, 5.0, 0.0), 0);
        assert_eq!(word_at(&p, 45.0, 5.0, 0.0), -1);
        assert_eq!(word_at(&p, 45.0, 5.0, 6.0), 0);
        // past the end of a line: its last word; between lines: the nearer
        assert_eq!(nearest_word(&p, 200.0, 4.0), 1);
        assert_eq!(nearest_word(&p, 0.0, 40.0), 2);
        assert_eq!(line_of(&p, 1), (0, 1));
        assert_eq!(line_of(&p, 3), (3, 3));
        assert_eq!(line_of(&p, 9), (-1, -1));
        assert_eq!(matching(&p, "TOTAL thanks"), vec![1, 3]);
    }

    #[test]
    fn selecting_like_on_a_phone() {
        let mut l = Live::new();
        l.words = page();
        l.status = "ready".into();
        l.press(10.0, 5.0, false, 1.0);
        assert_eq!(l.sel(), (0, 0));
        l.drag(12.0, 5.0, 1.0, 1.0); // (under 4 px: not a drag yet)
        assert_eq!(l.sel(), (0, 0));
        l.drag(10.0, 25.0, 1.0, 1.0);
        assert_eq!(l.sel(), (0, 2));
        assert_eq!(l.selected_text(), "Invoice total\nDue");
        l.press(10.0, 65.0, true, 1.0);
        assert_eq!(l.sel(), (0, 3));
        l.press(300.0, 300.0, false, 1.0);
        assert!(!l.has_selection());
        l.double(60.0, 5.0, 1.0);
        assert_eq!(l.sel(), (0, 1));
        // a selected band reaches the next word on its line
        let b = boxes(&l, (0.0, 0.0, 0.0, 0.0), 100.0, 100.0);
        assert!(b.is_empty() == false && b[0].4 && b[0].6 && !b[0].5 && b[1].5 && !b[1].6);
        assert_eq!(b[0].2, 50.0);
    }

    #[test]
    fn found_words_get_selected() {
        let mut l = Live::new();
        l.active = true;
        l.path = "/p.png".into();
        l.highlight = "invoice total".into();
        assert_eq!(l.start(), Some(json!(["/p.png", ""])));
        let r = json!({"status": "ready", "width": 100, "height": 80, "words": [
            {"x": 0, "y": 0, "w": 40, "h": 12, "text": "Invoice", "block": "1", "line": "1.1.1", "ly": 0, "lh": 12},
            {"x": 50, "y": 0, "w": 40, "h": 12, "text": "total", "block": "1", "line": "1.1.1", "ly": 0, "lh": 12}]});
        assert!(!l.done("/other#", &r));
        assert!(l.done("/p.png#", &r));
        assert_eq!((l.sel(), l.page), ((0, 1), (100.0, 80.0)));
        assert!(l.found);
    }

    #[test]
    fn links_addresses_and_numbers() {
        let a = actions("See https://example.com/pay, or www.tessera.dev. Mail ann@mail.example.org or call +44 20 7946 0958.", 6);
        let k: Vec<(&str, &str, &str)> = a.iter().map(|a| (a.kind.as_str(), a.value.as_str(), a.label.as_str())).collect();
        assert_eq!(k, vec![
            ("link", "https://example.com/pay", "example.com/pay"),
            ("link", "https://www.tessera.dev", "www.tessera.dev"),
            ("email", "ann@mail.example.org", "ann@mail.example.org"),
            ("phone", "+442079460958", "+44 20 7946 0958"),
        ]);
        // a name before an @ in a link disguises where it goes: no chip, and
        // its address isn't offered as an email either
        assert!(actions("http://bank.com@evil.io/x", 6).is_empty());
        // dates, versions and bare runs aren't numbers to call
        assert!(actions("on 2024-03-05, v1.2.3, order 12345678", 6).is_empty());
        assert_eq!(actions("(555) 123-4567", 6)[0].value, "5551234567");
        // duplicates once, and at most `max`
        assert_eq!(actions("a@b.co a@b.co c@d.co", 1).len(), 1);
    }

    #[test]
    fn phones_and_dates() {
        assert!(looks_like_phone("+31612345678"));
        assert!(looks_like_phone("020 794 6095"));
        assert!(!looks_like_phone("12345678"));
        assert!(!looks_like_phone("2024-03-05"));
        assert!(!looks_like_phone("05.03.2024"));
        assert!(!looks_like_phone("192.168.100.200"));
        assert!(!looks_like_phone("+1 23"));
    }

    #[test]
    fn what_the_bar_says() {
        let mut l = Live::new();
        let ocr = Ocr::default();
        l.status = "reading".into();
        assert_eq!(bar_message(&l, &ocr), ("Finding text…".into(), "circle-notch"));
        l.status = "missing".into();
        assert_eq!(bar_message(&l, &Ocr { install_error: "no network".into(), ..Default::default() }).0, "Couldn't install it: no network");
        l.status = "ready".into();
        l.words = page();
        assert_eq!(bar_message(&l, &ocr).0, "Drag across text to select it");
        l.anchor = 0;
        l.focus = 0;
        assert_eq!(bar_message(&l, &ocr).0, "1 word selected");
        l.focus = 2;
        assert_eq!(bar_message(&l, &ocr).0, "3 words selected");
        l.said = "Text copied".into();
        assert_eq!(bar_message(&l, &ocr), ("Text copied".into(), "check"));
        let phone = Action { kind: "phone".into(), value: "+1".into(), label: "+1 555 0100 2000 3000 4000".into(), wifi: Value::Null };
        assert_eq!(chip_label(&phone, &ocr), "Copy +1 555 0100 2000 3000…");
        assert_eq!(act_says(&phone, &ocr), "Number copied");
        assert_eq!(act_says(&phone, &Ocr { tel_handler: true, ..Default::default() }), "");
        assert_eq!(chips_that_fit(&["abc".into(), "def".into()], 80.0, false), 1);
    }
}

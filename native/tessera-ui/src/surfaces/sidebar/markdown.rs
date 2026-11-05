//! Markdown as models write it, into blocks the sidebar draws with Slint
//! elements: shell/common/Markdown.js's reading (headings, paragraphs,
//! emphasis, inline code, fenced code, lists (nested, numbered, tasks),
//! quotes, tables, rules and links), its blocks laid out in sidebar.slint.
//!
//! Inline text goes to Slint's StyledText, whose markup is a CommonMark
//! subset (bold, italic, strikethrough, code, links). The reply's text is
//! read here, by the shell's rules, and written out again as that markup with
//! every other character escaped: a reply can't bring HTML, images, or
//! markup Slint would refuse. Links open only for http(s) and mailto.
//!   split(md)   the reply in parts: prose, and each fenced code block
//!   blocks(md)  the parts as blocks to draw
//!   inline(s)   one line or paragraph as StyledText markup

/// A part of the reply (Markdown.js's split)
#[derive(Clone, Debug, PartialEq)]
pub enum Part {
    Text(String),
    /// language, code, still streaming (no closing fence yet)
    Code(String, String, bool),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Paragraph,
    Heading,
    List,
    Quote,
    Code,
    Table,
    Rule,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub depth: usize,
    /// "•", "3." or "" (a task's box is part of its text)
    pub marker: String,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub kind: Kind,
    /// heading level 1..6
    pub level: usize,
    /// StyledText markup (paragraphs, headings, quotes)
    pub text: String,
    pub code: String,
    pub lang: String,
    pub open: bool,
    pub items: Vec<Item>,
    /// table rows: (header row, its cells' markup)
    pub rows: Vec<(bool, Vec<String>)>,
    /// table columns' alignment: 0 left, 1 center, 2 right
    pub align: Vec<u8>,
}

impl Block {
    fn new(kind: Kind) -> Self {
        Block { kind, level: 0, text: String::new(), code: String::new(), lang: String::new(), open: false, items: Vec::new(), rows: Vec::new(), align: Vec::new() }
    }
}

/// Heading sizes in px (Markdown.js's HEAD: product UI stays at or under 24)
pub const HEAD: [f32; 7] = [0.0, 20.0, 18.0, 16.0, 15.0, 14.0, 14.0];

/// The room above each block: Markdown.js's margins (a paragraph 0 above
/// and 8 below; a heading 10 above, 8 from h3 on, 4 below; a list 2 and 2;
/// a table 4 and 4; a quote and a rule 8 and 8) where two meet the larger
/// one, as Qt's rich text lays them; a code block 8 from what's around it
/// (ChatMessage.qml's column); the first block none
pub fn gaps(blocks: &[Block]) -> Vec<f32> {
    let m = |b: &Block| match b.kind {
        Kind::Paragraph => (0.0, 8.0),
        Kind::Heading => (if b.level <= 2 { 10.0 } else { 8.0 }, 4.0),
        Kind::List => (2.0, 2.0),
        Kind::Table => (4.0, 4.0),
        Kind::Quote => (12.0, 8.0),
        Kind::Rule | Kind::Code => (8.0, 8.0),
    };
    blocks.iter().enumerate().map(|(i, b)| if i == 0 { 0.0 } else { f32::max(m(&blocks[i - 1]).1, m(b).0) }).collect()
}

/// A table's columns as wide as their widest cell (Qt's rich text tables),
/// from the text's length at 13 px (no text measuring here: Inter's average
/// advance, a little more in bold), 6 px padding a side; squeezed to `max`
/// when wider (the cells wrap)
pub fn column_widths(rows: &[(bool, Vec<String>)], max: f32) -> Vec<f32> {
    let n = rows.iter().map(|r| r.1.len()).max().unwrap_or(0);
    let mut w = vec![0f32; n];
    for (head, cells) in rows {
        for (i, c) in cells.iter().enumerate() {
            let len = plain(c).chars().count() as f32;
            w[i] = w[i].max(len * if *head { 7.8 } else { 6.8 } + 13.0);
        }
    }
    let total: f32 = w.iter().sum();
    if total > max && total > 0.0 {
        w.iter_mut().for_each(|x| *x *= max / total);
    }
    w.into_iter().map(|x| x.round()).collect()
}

/// StyledText markup's text alone (for its length)
fn plain(markup: &str) -> String {
    let mut out = String::new();
    let mut it = markup.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '*' | '_' | '`' | '\\' => {}
            // [text](url): the text
            ']' if it.peek() == Some(&'(') => {
                for c in it.by_ref() {
                    if c == ')' {
                        break;
                    }
                }
            }
            '[' => {}
            _ => out.push(c),
        }
    }
    out
}

// ── Lines ────────────────────────────────────────────────────────────────

fn indent(l: &str) -> usize {
    l.len() - l.trim_start_matches(' ').len()
}

/// ^ {0,3}(`{3,}|~{3,})\s*([\w+#.-]*)[^`]*$ : the fence and its language
fn fence(l: &str) -> Option<(String, String)> {
    if indent(l) > 3 {
        return None;
    }
    let t = &l[indent(l)..];
    let ch = t.chars().next().filter(|c| *c == '`' || *c == '~')?;
    let run = t.chars().take_while(|c| *c == ch).count();
    if run < 3 {
        return None;
    }
    let rest = &t[run..];
    if rest.contains('`') {
        return None;
    }
    let lang: String = rest.trim_start().chars().take_while(|c| c.is_alphanumeric() || "_+#.-".contains(*c)).collect();
    Some((ch.to_string().repeat(run), lang))
}

/// ^(\s*)([-*+]|\d{1,9}[.)])\s+(.*)$ : (indent, ordered, start, text)
fn list_item(l: &str) -> Option<(usize, bool, u64, String)> {
    let lead: String = l.chars().take_while(|c| c.is_whitespace()).collect();
    let rest = &l[lead.len()..];
    let (ordered, start, after) = if let Some(r) = rest.strip_prefix(['-', '*', '+']) {
        (false, 1, r)
    } else {
        let digits = rest.chars().take_while(|c| c.is_ascii_digit()).count();
        if !(1..=9).contains(&digits) {
            return None;
        }
        let r = &rest[digits..];
        let r = r.strip_prefix(['.', ')'])?;
        (true, rest[..digits].parse().unwrap_or(1), r)
    };
    if !after.starts_with(char::is_whitespace) {
        return None;
    }
    let text = after.trim_start();
    if text.is_empty() && after.chars().all(char::is_whitespace) {
        // ("- " alone: \s+(.*) still matches with an empty item)
        return Some((lead.replace('\t', "    ").len(), ordered, start, String::new()));
    }
    Some((lead.replace('\t', "    ").len(), ordered, start, text.to_string()))
}

/// ^ {0,3}(#{1,6})\s+(.*?)\s*#*\s*$
fn heading(l: &str) -> Option<(usize, String)> {
    if indent(l) > 3 {
        return None;
    }
    let t = &l[indent(l)..];
    let n = t.chars().take_while(|c| *c == '#').count();
    if !(1..=6).contains(&n) || !t[n..].starts_with(char::is_whitespace) {
        return None;
    }
    let body = t[n..].trim();
    let body = body.trim_end_matches('#').trim_end();
    Some((n, body.to_string()))
}

/// ^ {0,3}([-*_])(\s*\1){2,}\s*$
fn rule(l: &str) -> bool {
    if indent(l) > 3 {
        return false;
    }
    let t: String = l.chars().filter(|c| !c.is_whitespace()).collect();
    let Some(c) = t.chars().next() else { return false };
    "-*_".contains(c) && t.len() >= 3 && t.chars().all(|x| x == c) && l.trim_start().starts_with(c)
}

fn quote(l: &str) -> bool {
    indent(l) <= 3 && l[indent(l)..].starts_with('>')
}

fn table_sep(l: &str) -> bool {
    // ^\s*\|?\s*:?-+:?\s*(\|\s*:?-+:?\s*)*\|?\s*$ with a dash
    let t = l.trim();
    let t = t.strip_prefix('|').unwrap_or(t);
    let t = t.strip_suffix('|').unwrap_or(t);
    l.contains('-') && t.split('|').all(|c| {
        let c = c.trim();
        let c = c.strip_prefix(':').unwrap_or(c);
        let c = c.strip_suffix(':').unwrap_or(c);
        !c.is_empty() && c.chars().all(|x| x == '-')
    })
}

fn cells(line: &str) -> Vec<String> {
    let t = line.trim();
    let t = t.strip_prefix('|').unwrap_or(t);
    let t = t.strip_suffix('|').unwrap_or(t);
    // (an escaped \| stays in its cell)
    t.replace("\\|", "\u{1}").split('|').map(|c| c.trim().replace('\u{1}', "|")).collect()
}

/// The reply in parts: prose, and each fenced code block on its own
pub fn split(md: &str) -> Vec<Part> {
    let md = md.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = md.split('\n').collect();
    let mut parts = Vec::new();
    let mut prose: Vec<&str> = Vec::new();
    let flush = |prose: &mut Vec<&str>, parts: &mut Vec<Part>| {
        let text = prose.join("\n");
        if !text.trim().is_empty() {
            parts.push(Part::Text(text));
        }
        prose.clear();
    };
    let mut i = 0;
    while i < lines.len() {
        let Some((f, lang)) = fence(lines[i]) else {
            prose.push(lines[i]);
            i += 1;
            continue;
        };
        flush(&mut prose, &mut parts);
        let mut code = Vec::new();
        i += 1;
        let mut closed = false;
        let fc = f.chars().next().unwrap();
        while i < lines.len() {
            let l = lines[i];
            i += 1;
            let t = l.trim();
            if t.starts_with(&f) && t.chars().all(|c| c == fc) {
                closed = true;
                break;
            }
            code.push(l);
        }
        parts.push(Part::Code(lang, code.join("\n"), !closed));
    }
    flush(&mut prose, &mut parts);
    parts
}

/// The reply as blocks to draw
pub fn blocks(md: &str) -> Vec<Block> {
    let mut out = Vec::new();
    for p in split(md) {
        match p {
            Part::Text(t) => prose(&t, &mut out),
            Part::Code(lang, code, open) => out.push(Block { lang, code, open, ..Block::new(Kind::Code) }),
        }
    }
    out
}

fn prose(md: &str, out: &mut Vec<Block>) {
    let lines: Vec<&str> = md.split('\n').collect();
    let mut i = 0;
    while i < lines.len() {
        let l = lines[i];
        if l.trim().is_empty() {
            i += 1;
            continue;
        }
        if let Some((n, text)) = heading(l) {
            out.push(Block { level: n, text: inline(&text), ..Block::new(Kind::Heading) });
            i += 1;
            continue;
        }
        if rule(l) {
            out.push(Block::new(Kind::Rule));
            i += 1;
            continue;
        }
        if quote(l) {
            let mut q = Vec::new();
            while i < lines.len() && quote(lines[i]) {
                let t = &lines[i][indent(lines[i]) + 1..];
                q.push(t.strip_prefix(' ').unwrap_or(t));
                i += 1;
            }
            // (a quote's own blocks, flattened to lines of text)
            let mut inner = Vec::new();
            prose(&q.join("\n"), &mut inner);
            let text = inner.iter().map(flat).filter(|t| !t.is_empty()).collect::<Vec<_>>().join("\n");
            out.push(Block { text, ..Block::new(Kind::Quote) });
            continue;
        }
        if l.contains('|') && i + 1 < lines.len() && table_sep(lines[i + 1]) {
            let head = cells(l);
            let align = cells(lines[i + 1])
                .iter()
                .map(|c| {
                    if c.starts_with(':') && c.ends_with(':') && c.len() > 1 {
                        1
                    } else if c.ends_with(':') {
                        2
                    } else {
                        0
                    }
                })
                .collect();
            let mut b = Block { align, ..Block::new(Kind::Table) };
            b.rows.push((true, head.iter().map(|c| inline(c)).collect()));
            i += 2;
            while i < lines.len() && lines[i].contains('|') && !lines[i].trim().is_empty() {
                b.rows.push((false, cells(lines[i]).iter().map(|c| inline(c)).collect()));
                i += 1;
            }
            out.push(b);
            continue;
        }
        if list_item(l).is_some() {
            let mut items = Vec::new();
            while i < lines.len() && !lines[i].trim().is_empty() && (list_item(lines[i]).is_some() || continuation(lines[i])) {
                items.push(lines[i]);
                i += 1;
            }
            out.push(Block { items: list(&items), ..Block::new(Kind::List) });
            continue;
        }
        // A paragraph: lines joined (a line ending in two spaces or "\" breaks)
        let mut para: Vec<&str> = Vec::new();
        while i < lines.len() {
            let x = lines[i];
            if x.trim().is_empty() || fence(x).is_some() || heading(x).is_some() || quote(x) || (!para.is_empty() && list_item(x).is_some()) {
                break;
            }
            para.push(x);
            i += 1;
        }
        let mut text = String::new();
        for (k, p) in para.iter().enumerate() {
            let br = k + 1 < para.len() && (p.ends_with("  ") || p.ends_with('\\'));
            text += &inline(p.strip_suffix('\\').unwrap_or(p).trim());
            if k + 1 < para.len() {
                text.push(if br { '\n' } else { ' ' });
            }
        }
        out.push(Block { text, ..Block::new(Kind::Paragraph) });
    }
}

/// ^\s{2,}\S
fn continuation(l: &str) -> bool {
    let lead = l.chars().take_while(|c| c.is_whitespace()).count();
    lead >= 2 && l.chars().nth(lead).is_some()
}

/// Markdown as lines of markup, for one StyledText (a reasoning summary)
pub fn flatten(md: &str) -> String {
    blocks(md).iter().map(flat).filter(|t| !t.is_empty()).collect::<Vec<_>>().join("\n")
}

/// A block as one line of markup (inside a quote)
fn flat(b: &Block) -> String {
    match b.kind {
        Kind::List => b.items.iter().map(|it| format!("{}{} {}", "  ".repeat(it.depth), escape(&it.marker), it.text)).collect::<Vec<_>>().join("\n"),
        Kind::Code => escape(&b.code),
        Kind::Table => b.rows.iter().map(|r| r.1.join(" · ")).collect::<Vec<_>>().join("\n"),
        _ => b.text.clone(),
    }
}

/// List items with their depth: deeper ones nest under the item before
fn list(lines: &[&str]) -> Vec<Item> {
    let mut out: Vec<Item> = Vec::new();
    // (indent, ordered, next number) per open list
    let mut stack: Vec<(usize, bool, u64)> = Vec::new();
    for l in lines {
        let Some((ind, ordered, start, text)) = list_item(l) else {
            // a continuation line: part of the item before
            if let Some(last) = out.last_mut() {
                last.text.push(' ');
                last.text += &inline(l.trim());
            }
            continue;
        };
        while stack.last().is_some_and(|s| ind < s.0) {
            stack.pop();
        }
        if stack.last().is_none_or(|s| ind > s.0) {
            stack.push((ind, ordered, start));
        }
        let depth = stack.len() - 1;
        let top = stack.last_mut().unwrap();
        let marker = if top.1 {
            let n = top.2;
            top.2 += 1;
            format!("{n}.")
        } else {
            "•".to_string()
        };
        let (text, task) = match text.strip_prefix('[').and_then(|t| t.get(..2).map(|m| (m, &t[2..]))) {
            Some((m, rest)) if (m == " ]" || m == "x]" || m == "X]") && rest.starts_with(char::is_whitespace) => (rest.trim_start().to_string(), Some(m != " ]")),
            _ => (text, None),
        };
        let text = match task {
            Some(done) => format!("{} {}", if done { "☑" } else { "☐" }, inline(&text)),
            None => inline(&text),
        };
        out.push(Item { depth, marker, text });
    }
    out
}

// ── Inline ───────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum Tok {
    C(char),
    /// a finished piece of markup (code span, link)
    Slot(usize),
    Mark(&'static str),
}

/// Every ASCII punctuation character escaped: nothing the reply wrote is
/// read as markup by Slint
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii_punctuation() {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

pub fn safe_url(url: &str) -> Option<String> {
    let u = url.trim();
    let lower = u.to_ascii_lowercase();
    let scheme = ["http://", "https://", "mailto:"].iter().find(|s| lower.starts_with(**s))?;
    (u.len() > scheme.len() && !u.chars().any(|c| c.is_whitespace() || "\"'<>".contains(c))).then(|| u.to_string())
}

fn link_markup(label: &str, url: &str) -> String {
    // (<…>: a destination with any characters but <, > and line breaks)
    format!("[{}](<{}>)", escape(label), url)
}

fn code_markup(code: &str) -> String {
    let longest = code.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let ticks = "`".repeat(longest + 1);
    format!("{ticks} {code} {ticks}")
}

fn is_ws(t: Option<&Tok>) -> bool {
    matches!(t, Some(Tok::C(c)) if c.is_whitespace())
}
fn is_word(t: Option<&Tok>) -> bool {
    matches!(t, Some(Tok::C(c)) if c.is_alphanumeric() || *c == '_')
}
fn is_char(t: Option<&Tok>, ch: char) -> bool {
    matches!(t, Some(Tok::C(c)) if *c == ch)
}

fn text_of(t: &[Tok]) -> String {
    t.iter().filter_map(|x| if let Tok::C(c) = x { Some(*c) } else { None }).collect()
}

/// One line or paragraph, as StyledText markup (Markdown.js's inline, in
/// the same order: code spans, images and links, web addresses, then bold,
/// italic and strikethrough)
pub fn inline(text: &str) -> String {
    let mut slots: Vec<String> = Vec::new();
    let mut t: Vec<Tok> = text.chars().map(Tok::C).collect();

    // Code spans first: nothing inside them is formatted
    let mut i = 0;
    while i < t.len() {
        if !is_char(t.get(i), '`') {
            i += 1;
            continue;
        }
        let n = (i..t.len()).take_while(|&k| is_char(t.get(k), '`')).count();
        let mut j = i + n;
        let mut found = None;
        while j < t.len() {
            if is_char(t.get(j), '`') {
                let m = (j..t.len()).take_while(|&k| is_char(t.get(k), '`')).count();
                if m == n {
                    found = Some(j);
                    break;
                }
                j += m;
            } else {
                j += 1;
            }
        }
        match found {
            Some(j) if j > i + n => {
                let code = text_of(&t[i + n..j]).trim().to_string();
                slots.push(code_markup(&code));
                t.splice(i..j + n, [Tok::Slot(slots.len() - 1)]);
                i += 1;
            }
            _ => i += n,
        }
    }

    // Images (their text as a link: no image is fetched) and links
    let mut i = 0;
    while i < t.len() {
        let image = is_char(t.get(i), '!') && is_char(t.get(i + 1), '[');
        let start = if image { i + 1 } else { i };
        if !is_char(t.get(start), '[') {
            i += 1;
            continue;
        }
        let close = (start + 1..t.len()).find(|&k| is_char(t.get(k), ']'));
        let Some(close) = close.filter(|&c| is_char(t.get(c + 1), '(')) else {
            i += 1;
            continue;
        };
        if close == start + 1 && !image {
            i += 1;
            continue;
        }
        let label = text_of(&t[start + 1..close]);
        // ([^)\s]+) then an optional "title"
        let mut k = close + 2;
        let url_start = k;
        while k < t.len() && !is_char(t.get(k), ')') && !is_ws(t.get(k)) {
            k += 1;
        }
        let url = text_of(&t[url_start..k]);
        if url.is_empty() {
            i += 1;
            continue;
        }
        if is_ws(t.get(k)) {
            let q = (k..t.len()).find(|&x| !is_ws(t.get(x)));
            let Some(q) = q.filter(|&q| is_char(t.get(q), '"')) else {
                i += 1;
                continue;
            };
            let Some(e) = (q + 1..t.len()).find(|&x| is_char(t.get(x), '"')) else {
                i += 1;
                continue;
            };
            if !is_char(t.get(e + 1), ')') {
                i += 1;
                continue;
            }
            k = e + 1;
        }
        if !is_char(t.get(k), ')') {
            i += 1;
            continue;
        }
        let shown = if image && label.is_empty() { "image".to_string() } else { label };
        let piece = match safe_url(&url) {
            Some(u) => link_markup(&shown, &u),
            None => escape(&shown),
        };
        slots.push(piece);
        t.splice(i..=k, [Tok::Slot(slots.len() - 1)]);
        i += 1;
    }

    // Bare and <angle> web addresses
    let mut i = 0;
    while i < t.len() {
        let angled = is_char(t.get(i), '<');
        let s = if angled { i + 1 } else { i };
        let starts = |p: &str| p.chars().enumerate().all(|(n, c)| is_char(t.get(s + n), c));
        if !(starts("http://") || starts("https://")) || (!angled && i > 0 && !(is_ws(t.get(i - 1)) || is_char(t.get(i - 1), '('))) {
            i += 1;
            continue;
        }
        let mut e = s;
        while e < t.len() && matches!(t[e], Tok::C(c) if !c.is_whitespace() && c != '<' && !(angled && c == '>')) {
            e += 1;
        }
        if angled {
            if !is_char(t.get(e), '>') {
                i += 1;
                continue;
            }
        } else {
            // (not ending in punctuation that's the sentence's)
            while e > s && matches!(t[e - 1], Tok::C(c) if ".,;:!?)'&".contains(c)) {
                e -= 1;
            }
        }
        let url = text_of(&t[s..e]);
        let Some(u) = safe_url(&url) else {
            i += 1;
            continue;
        };
        slots.push(link_markup(&u, &u));
        let end = if angled { e + 1 } else { e };
        t.splice(i..end, [Tok::Slot(slots.len() - 1)]);
        i += 1;
    }

    // Bold: (\*\*|__)(?=\S)([\s\S]*?\S)\1
    for d in ['*', '_'] {
        let mut i = 0;
        while i + 1 < t.len() {
            if !(is_char(t.get(i), d) && is_char(t.get(i + 1), d)) || t.get(i + 2).is_none() || is_ws(t.get(i + 2)) {
                i += 1;
                continue;
            }
            let close = (i + 3..t.len().saturating_sub(1)).find(|&j| is_char(t.get(j), d) && is_char(t.get(j + 1), d) && !is_ws(t.get(j - 1)));
            match close {
                Some(j) => {
                    t.splice(j..j + 2, [Tok::Mark("**")]);
                    t.splice(i..i + 2, [Tok::Mark("**")]);
                    i = j;
                }
                None => i += 2,
            }
        }
    }
    // Italic: (^|[^*\w])\*(?=\S)([^*]*?\S)\*(?!\*), and the same with _ (not inside words)
    for d in ['*', '_'] {
        let mut i = 0;
        while i < t.len() {
            let before = if i == 0 { None } else { t.get(i - 1) };
            if !is_char(t.get(i), d) || is_char(before, '*') || is_word(before) || t.get(i + 1).is_none() || is_ws(t.get(i + 1)) {
                i += 1;
                continue;
            }
            let mut close = None;
            for j in i + 2..t.len() {
                if is_char(t.get(j), d) {
                    if !is_ws(t.get(j - 1)) && !is_char(t.get(j + 1), '*') && (d == '*' || !is_word(t.get(j + 1))) {
                        close = Some(j);
                    }
                    break;
                }
                if d == '*' && is_char(t.get(j), '*') {
                    break;
                }
            }
            match close {
                Some(j) => {
                    t[j] = Tok::Mark("*");
                    t[i] = Tok::Mark("*");
                    i = j + 1;
                }
                None => i += 1,
            }
        }
    }
    // Strikethrough: ~~(?=\S)([\s\S]*?\S)~~
    let mut i = 0;
    while i + 1 < t.len() {
        if !(is_char(t.get(i), '~') && is_char(t.get(i + 1), '~')) || t.get(i + 2).is_none() || is_ws(t.get(i + 2)) {
            i += 1;
            continue;
        }
        let close = (i + 3..t.len().saturating_sub(1)).find(|&j| is_char(t.get(j), '~') && is_char(t.get(j + 1), '~') && !is_ws(t.get(j - 1)));
        match close {
            Some(j) => {
                t.splice(j..j + 2, [Tok::Mark("~~")]);
                t.splice(i..i + 2, [Tok::Mark("~~")]);
                i = j;
            }
            None => i += 2,
        }
    }

    let mut out = String::new();
    for x in &t {
        match x {
            Tok::C(c) => {
                if c.is_ascii_punctuation() {
                    out.push('\\');
                }
                // (a line break inside a paragraph's markup would start a new
                // one: the caller decides where lines break)
                out.push(if *c == '\n' { ' ' } else { *c });
            }
            Tok::Slot(n) => out += &slots[*n],
            Tok::Mark(m) => out += m,
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_as_wide_as_their_content() {
        let rows = vec![(true, vec!["**Day**".to_string(), "**Morning**".to_string()]), (false, vec!["Sat".to_string(), "[Alfama](https://x.y)".to_string()])];
        let w = column_widths(&rows, 396.0);
        assert_eq!(w, vec![(3.0f32 * 7.8 + 13.0).round(), (7.0f32 * 7.8 + 13.0).round()]);
        let long = vec![(false, vec!["x".repeat(80), "y".repeat(80)])];
        let w = column_widths(&long, 396.0);
        assert!((w.iter().sum::<f32>() - 396.0).abs() <= 1.0 && w[0] == w[1]);
        assert_eq!(gaps(&blocks("# A\n\n- b\n\ntext\n\n```\nx\n```")), vec![0.0, 4.0, 2.0, 8.0]);
    }

    /// Slint reads the markup without an error
    fn slint_ok(markup: &str) {
        if let Err(e) = slint::StyledText::from_markdown(markup) {
            panic!("Slint refused {markup:?}: {e}");
        }
    }

    #[test]
    fn the_lisbon_answer() {
        let md = "**A weekend in Lisbon**\n\n1. **Saturday:** Tram 28 through Alfama, lunch at a *tasca*, sunset at Senhora do Monte\n2. **Sunday:** pastries in Belém, the riverside walk, LX Factory\n\n```text\npack: walking shoes · light jacket · camera\n```";
        let b = blocks(md);
        assert_eq!(b.iter().map(|b| b.kind).collect::<Vec<_>>(), [Kind::Paragraph, Kind::List, Kind::Code]);
        assert_eq!(b[0].text, "**A weekend in Lisbon**");
        assert_eq!(b[1].items[0].marker, "1.");
        assert_eq!(b[1].items[1].marker, "2.");
        assert_eq!(b[1].items[0].text, "**Saturday\\:** Tram 28 through Alfama\\, lunch at a *tasca*\\, sunset at Senhora do Monte");
        assert_eq!((b[2].lang.as_str(), b[2].code.as_str(), b[2].open), ("text", "pack: walking shoes · light jacket · camera", false));
        for x in &b {
            slint_ok(&x.text);
            x.items.iter().for_each(|i| slint_ok(&i.text));
        }
    }

    #[test]
    fn split_as_the_shell() {
        assert_eq!(split("Hi\n\n```py\nx = 1\n```\nBye"), vec![Part::Text("Hi\n".into()), Part::Code("py".into(), "x = 1".into(), false), Part::Text("Bye".into())]);
        // still streaming: the fence isn't closed yet
        assert_eq!(split("Run:\n~~~bash\nls -la"), vec![Part::Text("Run:".into()), Part::Code("bash".into(), "ls -la".into(), true)]);
        // a longer closing fence of the same kind closes it; another kind doesn't
        assert_eq!(split("````\na\n~~~\n`````"), vec![Part::Code("".into(), "a\n~~~".into(), false)]);
    }

    #[test]
    fn headings_rules_quotes_tables() {
        let b = blocks("# Title #\n### Smaller\n---\n> quoted *text*\n> more\n\n| A | B |\n|:--|--:|\n| `x` | [l](https://e.x) |\nafter");
        assert_eq!((b[0].kind, b[0].level, b[0].text.as_str()), (Kind::Heading, 1, "Title"));
        assert_eq!(b[1].level, 3);
        assert_eq!(b[2].kind, Kind::Rule);
        assert_eq!((b[3].kind, b[3].text.as_str()), (Kind::Quote, "quoted *text* more"));
        assert_eq!(b[4].kind, Kind::Table);
        assert_eq!(b[4].align, vec![0, 2]);
        assert_eq!(b[4].rows.len(), 2);
        assert!(b[4].rows[0].0 && !b[4].rows[1].0);
        assert_eq!(b[4].rows[1].1[1], "[l](<https://e.x>)");
        assert_eq!(b[5].text, "after");
        for x in &b {
            slint_ok(&x.text);
            x.rows.iter().flat_map(|r| &r.1).for_each(|c| slint_ok(c));
        }
    }

    #[test]
    fn nested_lists_and_tasks() {
        let b = blocks("- one\n  - two\n    continued\n- [x] done\n- [ ] todo\n3) three");
        let items = &b[0].items;
        assert_eq!(items.iter().map(|i| i.depth).collect::<Vec<_>>(), [0, 1, 0, 0, 0]);
        assert_eq!(items[1].text, "two continued");
        assert_eq!(items[2].text, "☑ done");
        assert_eq!(items[3].text, "☐ todo");
        // (a number in a bulleted list stays in it, as the shell's)
        assert_eq!(items[4].marker, "•");
        assert_eq!(blocks("3) three\n4) four")[0].items[1].marker, "4.");
    }

    #[test]
    fn emphasis_code_and_links() {
        assert_eq!(inline("**bold** and *it* and _it_ and ~~gone~~"), "**bold** and *it* and *it* and ~~gone~~");
        assert_eq!(inline("snake_case_name stays"), "snake\\_case\\_name stays");
        assert_eq!(inline("a * b * c"), "a \\* b \\* c");
        assert_eq!(inline("run `ls -la *.txt`"), "run ` ls -la *.txt `");
        assert_eq!(inline("``a ` b``"), "`` a ` b ``");
        assert_eq!(inline("see https://example.com/a?b=1."), "see [https\\:\\/\\/example\\.com\\/a\\?b\\=1](<https://example.com/a?b=1>)\\.");
        assert_eq!(inline("<https://x.y/z>"), "[https\\:\\/\\/x\\.y\\/z](<https://x.y/z>)");
        assert_eq!(inline("[docs](https://d.io \"Title\")"), "[docs](<https://d.io>)");
        assert_eq!(inline("![cat](https://c.at/c.png)"), "[cat](<https://c.at/c.png>)");
        // anything else isn't a link, and no HTML gets through
        assert_eq!(inline("[run](javascript:alert(1))"), "run\\)");
        assert_eq!(inline("<b>hi</b> & <img src=x>"), "\\<b\\>hi\\<\\/b\\> \\& \\<img src\\=x\\>");
        assert_eq!(inline("# not a heading inside"), "\\# not a heading inside");
        for s in ["**bold *nested* bold**", "x**y**z", "1. not a list", "> not a quote", "| a | b |", "trailing \\", "`unclosed", "**unclosed", "<u>under</u>", "<font color=red>x</font>", "~~~", "* * *", "[a]", "[](https://e.x)"] {
            slint_ok(&inline(s));
        }
    }

    #[test]
    fn hard_breaks_keep_their_line() {
        let b = blocks("one  \ntwo\\\nthree\nfour");
        assert_eq!(b[0].text, "one\ntwo\nthree four");
        slint_ok(&b[0].text);
    }
}

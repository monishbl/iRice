//! What Settings' search finds, as Settings.qml builds it: section names,
//! and every row and group caption in the page files (label, title, detail,
//! pageTitle), read from the QML pages themselves so nothing has to load
//! (SettingsIndex.js's `captions`: a part shown in one look only is left
//! out of the other's, a `Theme.material ? "a" : "b"` caption gives the
//! look's text).

use super::map;

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub page: String,
    pub text: String,
    pub detail: String,
    /// a section's name (go to the page, not a row on it)
    pub section: bool,
    line: usize,
}

#[derive(Debug)]
enum Ev {
    Open,
    Close,
    Look(bool), // material
    Caption { key: String, material: Option<String>, glass: String },
}

/// Strings and comments blanked to the same length (their braces don't count)
fn code_of(text: &str) -> Vec<u8> {
    let b = text.as_bytes();
    let mut out = b.to_vec();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'"' {
            let start = i;
            i += 1;
            while i < b.len() && b[i] != b'"' {
                if b[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
            for c in out.iter_mut().take(i.min(b.len())).skip(start + 1) {
                *c = b' ';
            }
            i += 1;
        } else if b[i] == b'/' && b.get(i + 1) == Some(&b'/') {
            for c in out.iter_mut().skip(i) {
                *c = b' ';
            }
            break;
        } else {
            i += 1;
        }
    }
    out
}

/// A line's braces, captions and look conditions, in the order they come
fn events(text: &str) -> Vec<(usize, Ev)> {
    let code = code_of(text);
    let code_s = String::from_utf8_lossy(&code).into_owned();
    let mut out: Vec<(usize, Ev)> = Vec::new();
    for (k, c) in code.iter().enumerate() {
        if *c == b'{' {
            out.push((k, Ev::Open));
        } else if *c == b'}' {
            out.push((k, Ev::Close));
        }
    }
    // `(visible|active): *!?Theme.material *($|;|&&)`
    for key in ["visible:", "active:"] {
        let mut from = 0;
        while let Some(p) = code_s[from..].find(key) {
            let at = from + p;
            from = at + key.len();
            if at > 0 && code_s.as_bytes()[at - 1].is_ascii_alphanumeric() {
                continue;
            }
            let rest = code_s[from..].trim_start();
            let (neg, rest) = match rest.strip_prefix('!') {
                Some(r) => (true, r),
                None => (false, rest),
            };
            if let Some(after) = rest.strip_prefix("Theme.material") {
                let after = after.trim_start();
                if after.is_empty() || after.starts_with(';') || after.starts_with("&&") {
                    out.push((at, Ev::Look(!neg)));
                }
            }
        }
    }
    // `(label|title|detail|pageTitle): (?:Theme.material ? "a" : )?"b"`
    for key in ["label", "title", "detail", "pageTitle"] {
        let pat = format!("{key}: ");
        let mut from = 0;
        while let Some(p) = text[from..].find(&pat) {
            let at = from + p;
            from = at + pat.len();
            // (a whole word: not "sublabel: ")
            if at > 0 && (text.as_bytes()[at - 1].is_ascii_alphanumeric() || text.as_bytes()[at - 1] == b'_') {
                continue;
            }
            if code.get(at) == Some(&b' ') {
                continue; // (inside a string or comment)
            }
            let rest = &text[from..];
            let quoted = |s: &str| -> Option<(String, usize)> {
                let s2 = s.strip_prefix('"')?;
                let end = s2.find('"')?;
                Some((s2[..end].to_string(), end + 2))
            };
            if let Some(r) = rest.strip_prefix("Theme.material ? ") {
                if let Some((m, n)) = quoted(r) {
                    if let Some(r2) = r[n..].strip_prefix(" : ") {
                        if let Some((g, _)) = quoted(r2) {
                            out.push((at, Ev::Caption { key: key.into(), material: Some(m), glass: g }));
                        }
                    }
                }
            } else if let Some((g, _)) = quoted(rest) {
                out.push((at, Ev::Caption { key: key.into(), material: None, glass: g }));
            }
        }
    }
    out.sort_by_key(|e| e.0);
    out
}

/// One page file's captions for a look: (line, key, text)
pub fn captions(text: &str, material: bool) -> Vec<(usize, String, String)> {
    let lines: Vec<(usize, Vec<(usize, Ev)>)> = text.lines().enumerate().map(|(i, l)| (i + 1, events(l))).collect();
    // the blocks (numbered by their opening brace) and the look each is for
    let mut look_of: std::collections::HashMap<usize, bool> = Default::default();
    let mut next = 0;
    let mut open: Vec<usize> = Vec::new();
    for (_, evs) in &lines {
        for (_, e) in evs {
            match e {
                Ev::Open => {
                    open.push(next);
                    next += 1;
                }
                Ev::Close => {
                    open.pop();
                }
                Ev::Look(m) => {
                    if let Some(b) = open.last() {
                        look_of.insert(*b, *m);
                    }
                }
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    let mut next = 0;
    let mut open: Vec<usize> = Vec::new();
    for (n, evs) in &lines {
        for (_, e) in evs {
            match e {
                Ev::Open => {
                    open.push(next);
                    next += 1;
                }
                Ev::Close => {
                    open.pop();
                }
                Ev::Caption { key, material: m, glass } => {
                    if open.iter().any(|b| look_of.get(b) == Some(&!material)) {
                        continue;
                    }
                    let said = if material { m.clone().unwrap_or_else(|| glass.clone()) } else { glass.clone() };
                    if !said.is_empty() {
                        out.push((*n, key.clone(), said));
                    }
                }
                _ => {}
            }
        }
    }
    out
}

/// The index from the page files (name without .qml, text), in file order
pub fn build(files: &[(String, String)], material: bool) -> Vec<Entry> {
    let mut out: Vec<Entry> = Vec::new();
    for (file, text) in files {
        let mut last: Option<usize> = None;
        for (line, key, said) in captions(text, material) {
            if said.contains('+') || said.chars().count() < 3 {
                continue;
            }
            if key == "detail" {
                if let Some(i) = last {
                    let page = map::page_of_file(file).0;
                    if out[i].page == page && line - out[i].line <= 3 {
                        out[i].detail = said;
                    }
                }
                continue;
            }
            let (page, _) = map::page_of_file(file);
            if out.iter().any(|e| e.page == page && e.text == said) {
                // (QML still moves `last` to it)
                last = None;
                continue;
            }
            out.push(Entry { page, text: said, detail: String::new(), section: false, line });
            last = Some(out.len() - 1);
        }
    }
    // (rows the Rust pages leave out: no hit that opens a page without them)
    out.retain(|e| !LEFT_OUT.contains(&e.text.as_str()));
    // The Rust pages' own rows: the QML page files are frozen, so a row
    // added to a Rust page since is found only here
    for (page, text, detail) in rust_entries() {
        if !out.iter().any(|e| e.page == page && e.text == text) {
            out.push(Entry { page, text, detail, section: false, line: 0 });
        }
    }
    out
}

/// Captions in the QML page files whose rows the Rust Settings doesn't show
/// (their settings do nothing in the Rust shell: phone.dock has no Dock item,
/// performance.keepPanelsLoaded is the QML shell's PanelLoader)
pub const LEFT_OUT: &[&str] = &["Show the phone in the Dock", "Keep panels loaded"];

/// The Rust pages with rows the QML page files don't have, compiled in:
/// (the file's page, its source, functions in it that build another page)
const RUST_PAGES: &[(&str, &str, &[(&str, &str)])] = &[
    ("update", include_str!("pages/about/update.rs"), &[("options", "update/options"), ("back", "update/back")]),
    ("health", include_str!("pages/system/health.rs"), &[]),
];

/// Every Rust page's captions (RUST_PAGES), as (page, text, detail)
fn rust_entries() -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    for (page, src, fns) in RUST_PAGES {
        out.extend(rust_captions(src, page, fns));
    }
    out
}

/// A string literal's text from just after its opening quote (None: no
/// closing quote on the line)
fn literal_at(s: &str) -> Option<String> {
    let mut out = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(out),
            '\\' => match chars.next() {
                Some('n') => out.push(' '),
                Some(e) => out.push(e),
                None => return None,
            },
            c => out.push(c),
        }
    }
    None
}

/// A Rust page's captions: `Row::new("…")` and `group("…")`, each with the
/// `.detail("…")` that follows it (within 3 lines), as (page, text,
/// detail). The page is `page`, or another for the functions named in
/// `fns`; a caption built at run time (format!) isn't seen.
fn rust_captions(src: &str, page: &str, fns: &[(&str, &str)]) -> Vec<(String, String, String)> {
    let mut out: Vec<(String, String, String)> = Vec::new();
    let mut here = page.to_string();
    let mut last: Option<(usize, usize)> = None; // (entry, line)
    for (n, line) in src.lines().enumerate() {
        if line.trim_start().starts_with("#[cfg(test)]") {
            break;
        }
        if let Some(rest) = line.strip_prefix("pub fn ").or_else(|| line.strip_prefix("fn ")) {
            let name = rest.split(|c: char| c == '(' || c == '<').next().unwrap_or("");
            here = fns.iter().find(|f| f.0 == name).map_or(page, |f| f.1).to_string();
            last = None;
        }
        let mut found: Vec<(usize, bool, String)> = Vec::new(); // (where, a detail, text)
        for (pat, detail) in [("Row::new(\"", false), ("group(\"", false), (".detail(\"", true)] {
            let mut from = 0;
            while let Some(p) = line[from..].find(pat) {
                let at = from + p;
                from = at + pat.len();
                if let Some(text) = literal_at(&line[from..]) {
                    found.push((at, detail, text));
                }
            }
        }
        found.sort_by_key(|f| f.0);
        for (_, detail, text) in found {
            if detail {
                if let Some((i, l)) = last {
                    if n - l <= 3 && out[i].2.is_empty() {
                        out[i].2 = text;
                    }
                }
                continue;
            }
            if text.chars().count() < 3 || text.ends_with('…') || out.iter().any(|e| e.0 == here && e.1 == text) {
                last = None;
                continue;
            }
            out.push((here.clone(), text, String::new()));
            last = Some((out.len() - 1, n));
        }
    }
    out
}

/// The page files beside the shell (shell/modules/settings/pages/*.qml)
pub fn read_pages() -> Vec<(String, String)> {
    let Some(dir) = crate::util::asset("shell/modules/settings/pages") else { return Vec::new() };
    let mut files: Vec<(String, String)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            let name = p.file_name()?.to_str()?.strip_suffix(".qml")?.to_string();
            Some((name, std::fs::read_to_string(&p).ok()?))
        })
        .collect();
    // (grep reads them in the shell's glob order)
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}

/// The results for a search: sections whose name has every word, then the
/// rows (with their page's name) that do; 60 at most
pub fn search(index: &[Entry], query: &str) -> Vec<Entry> {
    let q = map::search_query(&query.trim().to_lowercase());
    if q.is_empty() {
        return Vec::new();
    }
    let words: Vec<&str> = q.split_whitespace().collect();
    let hit = |s: &str| {
        let s = s.to_lowercase();
        words.iter().all(|w| s.contains(w))
    };
    let mut out = Vec::new();
    for s in map::SECTIONS {
        if hit(s.name) {
            out.push(Entry { page: s.id.into(), text: s.name.into(), detail: String::new(), section: true, line: 0 });
        }
    }
    for e in index {
        let name = map::section_of(&e.page).map_or("", |s| s.name);
        if hit(&format!("{} {} {}", e.text, e.detail, name)) {
            out.push(e.clone());
        }
    }
    out.truncate(60);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = r#"Column {
    Group {
        title: "Behaviour"
        Row {
            label: "Show the dock"
            detail: "Hides when you want"
            Switch {}
        }
        Row {
            visible: Theme.material // (Material only)
            label: "App names"
        }
        Row {
            visible: !Theme.material
            label: "Magnification"
        }
        Row { label: Theme.material ? "Colours" : "Accent"; detail: "x" + y }
        Text { text: "label: \"not this\"" } // label: "nor this"
    }
}
"#;

    #[test]
    fn captions_per_look() {
        let g: Vec<String> = captions(PAGE, false).into_iter().map(|c| c.2).collect();
        // ("x" too, as SettingsIndex.js gives it: Settings.qml drops captions under 3 characters)
        assert_eq!(g, ["Behaviour", "Show the dock", "Hides when you want", "Magnification", "Accent", "x"]);
        let m: Vec<String> = captions(PAGE, true).into_iter().map(|c| c.2).collect();
        assert_eq!(m, ["Behaviour", "Show the dock", "Hides when you want", "App names", "Colours", "x"]);
    }

    #[test]
    fn rows_with_their_details_and_pages() {
        let idx = build(&[("dock".into(), PAGE.into())], false);
        assert_eq!(idx[1].text, "Show the dock");
        assert_eq!(idx[1].detail, "Hides when you want");
        assert_eq!(idx[1].page, "menubar");
        let r = search(&idx, "show dock");
        assert_eq!(r[0].text, "Show the dock");
        // a section by name, and the rows on its page
        let r = search(&idx, "menu bar");
        assert!(r[0].section && r[0].page == "menubar");
        assert!(search(&idx, "  ").is_empty());
    }

    #[test]
    fn rust_page_captions_with_their_details_and_pages() {
        let src = "pub fn page(cx: &mut Ctx, p: &mut Page) {\n    p.group(\"Tessera\")\n        .row(Row::new(\"Follow \\\"beta\\\"\").detail(\"Which one\").switch(a, b))\n        .row(Row::new(name).detail(\"not this\"));\n    p.group(\"\").row(Row::new(\"Loading…\"));\n}\npub fn options(cx: &mut Ctx, p: &mut Page) {\n    p.group(\"\").row(Row::new(\"Snapshots\"));\n}\n#[cfg(test)]\nmod tests { fn t() { Row::new(\"Not a row\"); } }\n";
        let got = rust_captions(src, "update", &[("options", "update/options")]);
        assert_eq!(
            got,
            vec![
                ("update".to_string(), "Tessera".to_string(), String::new()),
                ("update".to_string(), "Follow \"beta\"".to_string(), "Which one".to_string()),
                ("update/options".to_string(), "Snapshots".to_string(), String::new()),
            ]
        );
    }

    #[test]
    fn rows_only_the_rust_pages_have_are_found_and_left_out_ones_arent() {
        let idx = build(&read_pages(), false);
        assert!(idx.iter().any(|e| e.page == "update" && e.text == "Set up the rest of Tessera"));
        assert!(idx.iter().any(|e| e.page == "update/options" && e.text == "Snapshots in the boot menu"));
        for gone in LEFT_OUT {
            assert!(!idx.iter().any(|e| e.text == *gone), "{gone}");
        }
        let r = search(&idx, "rest of tessera");
        assert!(r.iter().any(|e| e.page == "update"));
    }

    #[test]
    fn the_real_pages_are_found() {
        let files = read_pages();
        assert!(files.len() > 40);
        let idx = build(&files, false);
        assert!(idx.iter().any(|e| e.page == "menubar" && e.text == "Show the dock"));
        assert!(idx.iter().any(|e| e.page == "appearance" && e.text == "Reduce transparency"));
        let m = build(&files, true);
        assert!(!m.iter().any(|e| e.page == "appearance" && e.text == "Reduce transparency"));
    }
}

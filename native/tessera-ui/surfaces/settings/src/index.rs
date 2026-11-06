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

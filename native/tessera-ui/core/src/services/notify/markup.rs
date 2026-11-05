//! A notification body made safe to show, as Notifications.qml's safeMarkup
//! and plainBody: the spec's subset stays (bold, italic, underline, line
//! breaks, links to web and mail), everything else goes, above all <img>
//! (fetching it would tell whoever wrote the message that it was read).
//!
//! Slint's StyledText reads CommonMark with <u> as its only HTML, so the
//! subset is written as Markdown: text escaped (every ASCII punctuation mark
//! gets its backslash, so "1. done" or "*hi*" stay what they are), <b> as
//! **, <i> as *, <a href> as [text](href), each line its own paragraph with
//! its styles closed at the line's end and opened again on the next (Slint
//! starts a new paragraph at a line break).

/// One piece of a parsed body
#[derive(Clone, Debug, PartialEq)]
enum Piece {
    Text(String),
    Open(Tag),
    Close(Tag),
    Break,
}

#[derive(Clone, Debug, PartialEq)]
enum Tag {
    B,
    I,
    U,
    /// a link: its target, when it's one we open (web or mail)
    A(Option<String>),
}

/// Links a card may open: web and mail (Launch.qml's linkAllowed)
pub fn link_allowed(url: &str) -> bool {
    let u = url.trim().to_ascii_lowercase();
    u.starts_with("http://") || u.starts_with("https://") || u.starts_with("mailto:")
}

/// The entities a body may carry, decoded (&amp; last, as plainBody)
pub fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let end = tail[1..].find(|c: char| c == ';' || c == '&' || c.is_whitespace()).map(|e| e + 1);
        let decoded = end.filter(|&e| tail.as_bytes().get(e) == Some(&b';')).and_then(|e| {
            let name = &tail[1..e];
            let c = match name {
                "lt" => Some('<'),
                "gt" => Some('>'),
                "amp" => Some('&'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some('\u{a0}'),
                _ if name.starts_with("#x") || name.starts_with("#X") => u32::from_str_radix(&name[2..], 16).ok().and_then(char::from_u32),
                _ if name.starts_with('#') => name[1..].parse::<u32>().ok().and_then(char::from_u32),
                _ => None,
            };
            c.map(|c| (c, e + 1))
        });
        match decoded {
            Some((c, len)) => {
                out.push(c);
                rest = &tail[len..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn parse(body: &str) -> Vec<Piece> {
    let mut out = Vec::new();
    let mut text = String::new();
    let flush = |text: &mut String, out: &mut Vec<Piece>| {
        if !text.is_empty() {
            for (i, line) in std::mem::take(text).split('\n').enumerate() {
                if i > 0 {
                    out.push(Piece::Break);
                }
                if !line.is_empty() {
                    out.push(Piece::Text(decode_entities(line)));
                }
            }
        }
    };
    let mut rest = body;
    while let Some(lt) = rest.find('<') {
        text.push_str(&rest[..lt]);
        let tail = &rest[lt..];
        // a tag: <name …> or </name>, no '<' or '>' inside (safeMarkup's pattern)
        let close = tail[1..].find(['<', '>']).map(|i| i + 1).filter(|&i| tail.as_bytes()[i] == b'>');
        let Some(end) = close else {
            text.push('<');
            rest = &tail[1..];
            continue;
        };
        let inner = &tail[1..end];
        let (closing, inner) = match inner.strip_prefix('/') {
            Some(i) => (true, i),
            None => (false, inner),
        };
        let name: String = inner.chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
        if name.is_empty() || !name.chars().next().unwrap().is_ascii_alphabetic() {
            // not a tag ("a < b > c"): the text as written
            text.push_str(&tail[..=end]);
            rest = &tail[end + 1..];
            continue;
        }
        let attrs = &inner[name.len()..];
        flush(&mut text, &mut out);
        let tag = match name.to_ascii_lowercase().as_str() {
            "b" => Some(Tag::B),
            "i" => Some(Tag::I),
            "u" => Some(Tag::U),
            "a" => Some(Tag::A(if closing { None } else { href(attrs).filter(|h| link_allowed(h)) })),
            "br" => {
                if !closing {
                    out.push(Piece::Break);
                }
                None
            }
            _ => None, // gone (img and anything else)
        };
        if let Some(t) = tag {
            out.push(if closing { Piece::Close(t) } else { Piece::Open(t) });
        }
        rest = &tail[end + 1..];
    }
    text.push_str(rest);
    flush(&mut text, &mut out);
    out
}

/// href="…" or href='…' among a tag's attributes
fn href(attrs: &str) -> Option<String> {
    let lower = attrs.to_ascii_lowercase();
    let mut from = 0;
    while let Some(i) = lower[from..].find("href") {
        let at = from + i;
        from = at + 4;
        // (a whole word: not data-href)
        if at > 0 && !attrs.as_bytes()[at - 1].is_ascii_whitespace() {
            continue;
        }
        let after = attrs[at + 4..].trim_start();
        let Some(after) = after.strip_prefix('=') else { continue };
        let after = after.trim_start();
        let q = after.chars().next()?;
        if q != '"' && q != '\'' {
            continue;
        }
        let v = &after[1..];
        return v.find(q).map(|e| decode_entities(&v[..e]));
    }
    None
}

/// The body as plain text: markup and entities gone, lines kept
pub fn plain(body: &str) -> String {
    let mut out = String::new();
    for p in parse(body) {
        match p {
            Piece::Text(t) => out.push_str(&t),
            Piece::Break => out.push('\n'),
            _ => {}
        }
    }
    out.trim().to_string()
}

/// Every ASCII punctuation mark escaped, and a line's leading spaces dropped
/// (four would make a code block)
fn escape(t: &str, line_start: bool) -> String {
    let t = if line_start { t.trim_start() } else { t };
    let mut out = String::with_capacity(t.len() + 8);
    for c in t.chars() {
        if c.is_ascii_punctuation() {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

fn open_mark(t: &Tag) -> &'static str {
    match t {
        Tag::B => "**",
        Tag::I => "*",
        Tag::U => "<u>",
        Tag::A(Some(_)) => "[",
        Tag::A(None) => "",
    }
}

fn close_mark(t: &Tag) -> String {
    match t {
        Tag::B => "**".into(),
        Tag::I => "*".into(),
        Tag::U => "</u>".into(),
        Tag::A(Some(h)) => format!("](<{}>)", h.replace(['<', '>', '\n'], "")),
        Tag::A(None) => String::new(),
    }
}

/// The body as Markdown for Slint's StyledText (see the top). Empty styles
/// are left out: CommonMark reads "****" as a rule, not as nothing
pub fn markdown(body: &str) -> String {
    let pieces = parse(body);
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    let mut open: Vec<Tag> = Vec::new();
    // the styles written so far on this line (some may wait for text)
    let mut written: Vec<Tag> = Vec::new();
    let end_line = |line: &mut String, written: &mut Vec<Tag>, lines: &mut Vec<String>| {
        for t in written.drain(..).rev() {
            line.push_str(&close_mark(&t));
        }
        lines.push(std::mem::take(line));
    };
    for p in pieces {
        match p {
            Piece::Text(t) => {
                // the styles open now, written before their first text
                while written.len() < open.len() {
                    let t = open[written.len()].clone();
                    line.push_str(open_mark(&t));
                    written.push(t);
                }
                let at_start = line.chars().all(|c| c == '*' || c == '[') && !line.contains("<u>");
                line.push_str(&escape(&t, at_start));
            }
            Piece::Open(t) => open.push(t),
            Piece::Close(t) => {
                // close the innermost of its kind (and what's inside it)
                let same = |a: &Tag| std::mem::discriminant(a) == std::mem::discriminant(&t);
                if let Some(i) = open.iter().rposition(same) {
                    open.truncate(i);
                    while written.len() > open.len() {
                        let w = written.pop().unwrap();
                        line.push_str(&close_mark(&w));
                    }
                }
            }
            Piece::Break => end_line(&mut line, &mut written, &mut lines),
        }
    }
    end_line(&mut line, &mut written, &mut lines);
    // (blank lines would end a paragraph twice: one break between lines is
    // enough, and nothing at either end)
    let lines: Vec<String> = lines.into_iter().map(|l| l.trim_end().to_string()).collect();
    let first = lines.iter().position(|l| !l.is_empty()).unwrap_or(lines.len());
    let last = lines.iter().rposition(|l| !l.is_empty()).map_or(first, |i| i + 1);
    lines[first..last].iter().filter(|l| !l.is_empty()).cloned().collect::<Vec<_>>().join("\n")
}

/// The body as Slint shows it: Markdown when Slint reads it cleanly, plain
/// text otherwise
pub fn styled(body: &str) -> slint::StyledText {
    let md = markdown(body);
    slint::StyledText::from_markdown(&md).unwrap_or_else(|_| slint::StyledText::from_plain_text(&plain(body)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_stays() {
        assert_eq!(markdown("Hello there"), "Hello there");
        assert_eq!(plain("Hello there"), "Hello there");
    }

    #[test]
    fn markdown_characters_are_text() {
        assert_eq!(markdown("1. done *really* #1"), "1\\. done \\*really\\* \\#1");
        assert_eq!(markdown("    indented"), "indented");
        assert_eq!(markdown("a_b [x](y)"), "a\\_b \\[x\\]\\(y\\)");
        assert!(slint::StyledText::from_markdown(&markdown("- [ ] `code` > quote ~~x~~ | t |")).is_ok());
    }

    #[test]
    fn the_subset_stays() {
        assert_eq!(markdown("<b>Bold</b> and <i>it</i> <u>under</u>"), "**Bold** and *it* <u>under</u>");
        assert_eq!(markdown("<B>caps</B>"), "**caps**");
        assert_eq!(markdown("<a href=\"https://example.org/x?a=1\">site</a>"), "[site](<https://example.org/x?a=1>)");
        assert_eq!(markdown("<a href='mailto:me@example.org'>mail</a>"), "[mail](<mailto:me@example.org>)");
    }

    #[test]
    fn what_isnt_allowed_goes() {
        // <img> never stays (a read receipt), nor its alt text
        assert_eq!(markdown("Hi <img src=\"https://tracker.example/p.png\" alt=\"x\"/> there"), "Hi  there".trim_end());
        assert_eq!(plain("<span style=\"x\">Hi</span> <script>x</script>"), "Hi x");
        // links to anything but web and mail keep their text, not the link
        assert_eq!(markdown("<a href=\"file:///etc/passwd\">open</a>"), "open");
        assert_eq!(markdown("<a href=\"javascript:alert(1)\">x</a>"), "x");
        assert!(!link_allowed(" ftp://x") && link_allowed(" HTTPS://x") && link_allowed("mailto:a@b"));
    }

    #[test]
    fn line_breaks_and_open_styles() {
        assert_eq!(markdown("one<br>two<br/>three\nfour"), "one\ntwo\nthree\nfour");
        // a style across a break is closed and opened again
        assert_eq!(markdown("<b>one\ntwo</b> three"), "**one**\n**two** three");
        // unclosed and stray tags
        assert_eq!(markdown("<b>bold"), "**bold**");
        assert_eq!(markdown("x</b>y"), "xy");
        assert_eq!(markdown("<b></b>x"), "x");
        assert_eq!(markdown("\n\nx\n\n\ny\n"), "x\ny");
    }

    #[test]
    fn entities_decode_once() {
        assert_eq!(plain("a &lt;b&gt; &amp;lt; &quot;q&quot; &#39;s&#x21; & done"), "a <b> &lt; \"q\" 's! & done");
        assert_eq!(markdown("5 &gt; 3"), "5 \\> 3");
        assert_eq!(decode_entities("&unknown; &amp"), "&unknown; &amp");
    }

    #[test]
    fn not_tags_stay_text() {
        assert_eq!(plain("a < b and c > d"), "a < b and c > d");
        assert_eq!(plain("x <3 y"), "x <3 y");
    }

    #[test]
    fn whatever_comes_slint_reads_it() {
        for body in ["<b><i>x</b></i>", "<a href=\"https://a\"><b>x</a></b>", "<u>a\nb</u>", "*<b>*</b>*", "<a>no href</a>", "](<x>)", "&lt;u&gt;hi&lt;/u&gt;", "a\\b"] {
            let md = markdown(body);
            assert!(slint::StyledText::from_markdown(&md).is_ok(), "{body:?} -> {md:?}");
        }
    }
}

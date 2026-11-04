//! GIFs from KLIPY (docs.klipy.com), as shell/modules/emoji/Klipy.qml:
//! trending and search, 24 a page.
//!   GET https://api.klipy.com/api/v1/{app_key}/gifs/trending?page&per_page&customer_id&locale&content_filter&format_filter
//!   GET https://api.klipy.com/api/v1/{app_key}/gifs/search?q&…
//! The app key is the user's own: KLIPY_API_KEY, else Settings' emoji.klipyKey.
//! Nothing is fetched without one. The key is part of the URL, so curl gets
//! the URL on its stdin (`--config -`), never on its command line (which any
//! program can read), and nothing here prints a URL.

use std::process::{Command, Stdio};

#[derive(Clone, Debug, PartialEq)]
pub struct Gif {
    pub id: String,
    pub title: String,
    /// a still to show in the grid
    pub preview: String,
    /// the GIF to share
    pub url: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Idle,
    Loading,
    Ready,
    /// an HTTP status (or -1: an answer we couldn't read)
    Error(i32),
    Offline,
    BadKey,
}

/// The key: the environment's, else Settings'
pub fn key(cfg: &serde_json::Value) -> String {
    std::env::var("KLIPY_API_KEY").ok().filter(|k| !k.trim().is_empty()).unwrap_or_else(|| cfg["emoji"]["klipyKey"].as_str().unwrap_or("").to_string()).trim().to_string()
}

/// encodeURIComponent
pub fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The country part of the locale ("en_GB.UTF-8" → "gb"), as Qt.locale()
pub fn country() -> String {
    let lang = std::env::var("LC_ALL").ok().filter(|v| !v.is_empty()).or_else(|| std::env::var("LANG").ok()).unwrap_or_default();
    lang.split(['.', '@']).next().and_then(|l| l.split('_').nth(1)).map(str::to_lowercase).filter(|c| !c.is_empty()).unwrap_or_else(|| "us".into())
}

pub fn url(key: &str, query: &str, page: u32, customer: &str, filter: &str) -> String {
    let mut params = vec![
        format!("page={page}"),
        "per_page=24".into(),
        format!("customer_id={}", encode(if customer.is_empty() { "tessera" } else { customer })),
        format!("locale={}", country()),
        format!("content_filter={}", encode(filter)),
        "format_filter=gif,jpg".into(),
    ];
    if !query.is_empty() {
        params.insert(0, format!("q={}", encode(query)));
    }
    format!("https://api.klipy.com/api/v1/{}/gifs/{}?{}", encode(key), if query.is_empty() { "trending" } else { "search" }, params.join("&"))
}

/// A page of the answer: its GIFs, and whether there's another
pub fn parse(body: &str) -> Result<(Vec<Gif>, bool), Status> {
    let j: serde_json::Value = serde_json::from_str(body).map_err(|_| Status::Error(-1))?;
    if j["result"] == serde_json::Value::Bool(false) {
        return Err(Status::BadKey);
    }
    let d = &j["data"];
    let pick = |f: &serde_json::Value, sizes: &[&str], fmt: &str| sizes.iter().find_map(|s| f[s][fmt]["url"].as_str().filter(|u| !u.is_empty()).map(String::from));
    let gifs: Vec<Gif> = d["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|g| g["type"].is_null() || g["type"] == "gif")
        .filter_map(|g| {
            let f = &g["file"];
            let still = pick(f, &["sm", "xs", "md"], "jpg");
            let small = pick(f, &["sm", "xs", "md"], "gif");
            let share = pick(f, &["hd", "md", "sm"], "gif").or_else(|| small.clone())?;
            let id = g["id"].as_i64().map(|i| i.to_string()).or_else(|| g["id"].as_str().map(String::from)).or_else(|| g["slug"].as_str().map(String::from)).unwrap_or_default();
            Some(Gif { id, title: g["title"].as_str().unwrap_or("").into(), preview: still.or(small).unwrap_or_default(), url: share })
        })
        .collect();
    let next = d["has_next"].as_bool().unwrap_or(false) && !gifs.is_empty();
    Ok((gifs, next))
}

/// GET a URL with curl, the URL on its stdin; (HTTP status, body). Ok(0, _):
/// no answer (offline, or 12 s went by)
pub fn get(url: &str) -> std::io::Result<(u16, Vec<u8>)> {
    use std::io::Write;
    let mut child = Command::new("curl")
        .args(["--silent", "--location", "--max-time", "12", "--config", "-", "--write-out", "\n%{http_code}"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    // (curl's config: a quoted string; the URL is percent-encoded, no quotes or backslashes)
    let line = format!("url = \"{}\"\n", url.replace(['"', '\\', '\n'], ""));
    child.stdin.take().unwrap().write_all(line.as_bytes())?;
    let out = child.wait_with_output()?;
    let mut body = out.stdout;
    let at = body.iter().rposition(|&b| b == b'\n').unwrap_or(0);
    let code = String::from_utf8_lossy(&body[at..]).trim().parse::<u16>().unwrap_or(0);
    body.truncate(at);
    Ok((code, body))
}

/// One page, as the panel wants it
pub fn fetch(url: &str) -> Result<(Vec<Gif>, bool), Status> {
    match get(url) {
        Err(_) | Ok((0, _)) => Err(Status::Offline),
        Ok((401 | 403, _)) => Err(Status::BadKey),
        Ok((200, body)) => parse(&String::from_utf8_lossy(&body)),
        Ok((code, _)) => Err(Status::Error(code as i32)),
    }
}

/// A still for the grid (fetched, made cell-sized)
pub fn preview(url: &str) -> Option<slint::SharedPixelBuffer<slint::Rgba8Pixel>> {
    let (code, body) = get(url).ok()?;
    if code != 200 {
        return None;
    }
    let img = image::load_from_memory(&body).ok()?.thumbnail(260, 200).to_rgba8();
    Some(slint::SharedPixelBuffer::clone_from_slice(img.as_raw(), img.width(), img.height()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn urls_carry_the_query_and_paging() {
        let u = url("k/y", "cat & dog", 2, "abc", "medium");
        assert!(u.starts_with("https://api.klipy.com/api/v1/k%2Fy/gifs/search?q=cat%20%26%20dog&page=2&per_page=24&customer_id=abc&locale="));
        assert!(u.ends_with("&content_filter=medium&format_filter=gif,jpg"));
        assert!(url("k", "", 1, "", "off").contains("/gifs/trending?page=1&per_page=24&customer_id=tessera&"));
        assert_eq!(encode("é~"), "%C3%A9~");
    }

    #[test]
    fn the_key_comes_from_settings() {
        // (KLIPY_API_KEY wins when set; tests don't set it)
        if std::env::var("KLIPY_API_KEY").is_err() {
            assert_eq!(key(&json!({"emoji": {"klipyKey": "  abc "}})), "abc");
            assert_eq!(key(&json!({})), "");
        }
    }

    #[test]
    fn reads_a_page() {
        let body = json!({"result": true, "data": {"has_next": true, "data": [
            {"id": 7, "title": "Cat", "type": "gif", "file": {"sm": {"jpg": {"url": "s.jpg"}, "gif": {"url": "s.gif"}}, "hd": {"gif": {"url": "h.gif"}}}},
            {"slug": "dog", "title": "Dog", "file": {"xs": {"gif": {"url": "x.gif"}}}},
            {"id": 9, "type": "sticker", "file": {"hd": {"gif": {"url": "no.gif"}}}},
            {"id": 10, "file": {}}
        ]}})
        .to_string();
        let (g, next) = parse(&body).unwrap();
        assert!(next);
        assert_eq!(g.len(), 2);
        assert_eq!(g[0], Gif { id: "7".into(), title: "Cat".into(), preview: "s.jpg".into(), url: "h.gif".into() });
        assert_eq!((g[1].id.as_str(), g[1].preview.as_str(), g[1].url.as_str()), ("dog", "x.gif", "x.gif"));
        assert_eq!(parse(r#"{"result": false}"#), Err(Status::BadKey));
        assert_eq!(parse("<html>"), Err(Status::Error(-1)));
        assert_eq!(parse(r#"{"data": {"has_next": true, "data": []}}"#), Ok((vec![], false)));
    }
}

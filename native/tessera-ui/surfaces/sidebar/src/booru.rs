//! The sidebar's Anime tab (Booru.qml): yande.re, Konachan and Danbooru,
//! safe-rated results unless "Explicit" is on (sidebar.booruNsfw), twenty a
//! page, the next page when the grid reaches its end. Pure parts here (the
//! request's URL, the posts from its answer, the error to show); the
//! fetching runs on a thread (`fetch`, `preview`).

use serde_json::Value;
use std::time::Duration;

/// [id, name], as Booru.qml's providers
pub const PROVIDERS: &[(&str, &str)] = &[("yandere", "yande.re"), ("konachan", "Konachan"), ("danbooru", "Danbooru")];

pub fn provider_index(id: &str) -> usize {
    PROVIDERS.iter().position(|p| p.0 == id).unwrap_or(0)
}

pub fn provider_name(id: &str) -> &'static str {
    PROVIDERS[provider_index(id)].1
}

/// sidebar.booruProvider (yandere when unknown, as the QML's list falls back)
pub fn provider(cfg: &Value) -> String {
    let id = cfg["sidebar"]["booruProvider"].as_str().unwrap_or("yandere");
    PROVIDERS[provider_index(id)].0.to_string()
}

pub fn nsfw(cfg: &Value) -> bool {
    cfg["sidebar"]["booruNsfw"].as_bool().unwrap_or(false)
}

/// The tab is there (sidebar.booru, off by default)
pub fn enabled(cfg: &Value) -> bool {
    cfg["sidebar"]["booru"].as_bool().unwrap_or(false)
}

/// JavaScript's encodeURIComponent
pub fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(b as char);
        } else {
            out += &format!("%{b:02X}");
        }
    }
    out
}

/// One page's request (Booru.qml's url())
pub fn url(provider: &str, tags: &str, page: u32, nsfw: bool) -> String {
    let t = tags.split_whitespace().collect::<Vec<_>>().join(" ");
    if provider == "danbooru" {
        let q = format!("{t}{}", if nsfw { "" } else { " rating:g" });
        return format!("https://danbooru.donmai.us/posts.json?limit=20&page={page}&tags={}", encode(q.trim()));
    }
    let host = match (provider, nsfw) {
        ("konachan", true) => "https://konachan.com",
        ("konachan", false) => "https://konachan.net",
        _ => "https://yande.re",
    };
    let q = format!("{t}{}", if nsfw { "" } else { " rating:s" });
    format!("{host}/post.json?limit=20&page={page}&tags={}", encode(q.trim()))
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Post {
    /// the small picture for the grid
    pub preview: String,
    /// the post's page, opened on a click
    pub page: String,
}

/// The posts in an answer (Booru.qml's normalize(), those with a preview);
/// None when it isn't a list of posts
pub fn parse(provider: &str, body: &str) -> Option<Vec<Post>> {
    let v: Value = serde_json::from_str(body).ok()?;
    let list = v.as_array()?;
    let s = |p: &Value, k: &str| p[k].as_str().unwrap_or("").to_string();
    let id = |p: &Value| match &p["id"] {
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        _ => String::new(),
    };
    Some(
        list.iter()
            .map(|p| {
                if provider == "danbooru" {
                    Post { preview: s(p, "preview_file_url"), page: format!("https://danbooru.donmai.us/posts/{}", id(p)) }
                } else {
                    let host = if provider == "konachan" { "https://konachan.net" } else { "https://yande.re" };
                    Post { preview: s(p, "preview_url"), page: format!("{host}/post/show/{}", id(p)) }
                }
            })
            .filter(|p| !p.preview.is_empty())
            .collect(),
    )
}

/// What a failed search says (status 0: no answer at all)
pub fn error_text(status: u16, provider: &str) -> String {
    if status == 0 {
        format!("Couldn't reach {}. Check your internet connection.", provider_name(provider))
    } else {
        format!("Search failed ({status}). Try fewer tags.")
    }
}

fn agent() -> ureq::Agent {
    use ureq::tls::{RootCerts, TlsConfig};
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(20)))
        .user_agent("Tessera")
        .tls_config(TlsConfig::builder().root_certs(RootCerts::PlatformVerifier).build())
        .build()
        .into()
}

/// (the review's mock server in place of the sites, when it's set:
/// TESSERA_BOORU_MOCK=http://127.0.0.1:PORT, tools/sidebar-shots.sh)
fn routed(url: &str) -> String {
    if let Ok(base) = std::env::var("TESSERA_BOORU_MOCK") {
        if let Some(i) = url.find("://").and_then(|i| url[i + 3..].find('/').map(|j| i + 3 + j)) {
            return format!("{}{}", base.trim_end_matches('/'), &url[i..]);
        }
    }
    url.to_string()
}

/// GET: (status, body); status 0 when nothing answered
pub fn get(url: &str, limit: u64) -> (u16, Vec<u8>) {
    match agent().get(&routed(url)).call() {
        Ok(mut r) => {
            let status = r.status().as_u16();
            let body = r.body_mut().with_config().limit(limit).read_to_vec().unwrap_or_default();
            (status, body)
        }
        Err(_) => (0, Vec::new()),
    }
}

/// One page: the posts, or the error to show
pub fn fetch(provider: &str, tags: &str, page: u32, nsfw: bool) -> Result<Vec<Post>, String> {
    let (status, body) = get(&url(provider, tags, page, nsfw), 8 << 20);
    if status != 200 {
        return Err(error_text(status, provider));
    }
    parse(provider, &String::from_utf8_lossy(&body)).ok_or_else(|| "Got an unexpected response. Try again.".to_string())
}

/// A grid tile's picture: fetched, cropped square (the QML's
/// PreserveAspectCrop), at most `px` a side
pub fn preview(url: &str, px: u32) -> Option<slint::SharedPixelBuffer<slint::Rgba8Pixel>> {
    let (status, body) = get(url, 16 << 20);
    if status != 200 {
        return None;
    }
    let img = image::load_from_memory(&body).ok()?;
    let side = img.width().min(img.height());
    let sq = img.crop_imm((img.width() - side) / 2, (img.height() - side) / 2, side, side);
    let sq = if side > px { sq.thumbnail(px, px) } else { sq };
    let rgba = sq.to_rgba8();
    Some(slint::SharedPixelBuffer::clone_from_slice(rgba.as_raw(), rgba.width(), rgba.height()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn urls_as_the_shells() {
        assert_eq!(url("yandere", "", 1, false), "https://yande.re/post.json?limit=20&page=1&tags=rating%3As");
        assert_eq!(url("yandere", "  scenery   sky ", 2, false), "https://yande.re/post.json?limit=20&page=2&tags=scenery%20sky%20rating%3As");
        assert_eq!(url("konachan", "sky", 1, false), "https://konachan.net/post.json?limit=20&page=1&tags=sky%20rating%3As");
        assert_eq!(url("konachan", "sky", 1, true), "https://konachan.com/post.json?limit=20&page=1&tags=sky");
        assert_eq!(url("danbooru", "", 3, false), "https://danbooru.donmai.us/posts.json?limit=20&page=3&tags=rating%3Ag");
        assert_eq!(url("danbooru", "", 1, true), "https://danbooru.donmai.us/posts.json?limit=20&page=1&tags=");
        assert_eq!(encode("a&b=c/é"), "a%26b%3Dc%2F%C3%A9");
    }

    #[test]
    fn posts_from_each_site() {
        let y = json!([{"id": 7, "preview_url": "https://y/p.jpg"}, {"id": 8, "preview_url": ""}]).to_string();
        assert_eq!(parse("yandere", &y).unwrap(), vec![Post { preview: "https://y/p.jpg".into(), page: "https://yande.re/post/show/7".into() }]);
        assert_eq!(parse("konachan", &y).unwrap()[0].page, "https://konachan.net/post/show/7");
        let d = json!([{"id": 9, "preview_file_url": "https://d/p.jpg"}, {"id": 10}]).to_string();
        assert_eq!(parse("danbooru", &d).unwrap(), vec![Post { preview: "https://d/p.jpg".into(), page: "https://danbooru.donmai.us/posts/9".into() }]);
        assert_eq!(parse("yandere", "[]").unwrap(), vec![]);
        assert!(parse("yandere", "{\"error\":1}").is_none());
        assert!(parse("yandere", "<html>").is_none());
    }

    #[test]
    fn settings_and_errors() {
        assert_eq!(provider(&json!({})), "yandere");
        assert_eq!(provider(&json!({"sidebar": {"booruProvider": "danbooru"}})), "danbooru");
        assert_eq!(provider(&json!({"sidebar": {"booruProvider": "nope"}})), "yandere");
        assert!(!enabled(&json!({})) && enabled(&json!({"sidebar": {"booru": true}})));
        assert!(!nsfw(&json!({})));
        assert_eq!(error_text(0, "konachan"), "Couldn't reach Konachan. Check your internet connection.");
        assert_eq!(error_text(500, "yandere"), "Search failed (500). Try fewer tags.");
    }
}

//! The App Store's words and numbers, as AppStoreWindow.qml has them: the
//! places, the grid, the headings and the empty states per view and filter,
//! the buttons' labels per state, the "aur:" prefix, the AUR warning.

use serde_json::Value;

/// The store's places: (id, name, Phosphor icon, a category)
pub const PLACES: [(&str, &str, &str, bool); 11] = [
    ("discover", "Discover", "compass", false),
    ("Productivity", "Productivity", "briefcase", true),
    ("Development", "Development", "code", true),
    ("Graphics & Design", "Graphics & Design", "paint-brush", true),
    ("Music & Video", "Music & Video", "film-strip", true),
    ("Games", "Games", "game-controller", true),
    ("Internet", "Internet", "globe", true),
    ("Education & Science", "Education & Science", "graduation-cap", true),
    ("Utilities", "Utilities", "wrench", true),
    ("System", "System", "gear", true),
    ("installed", "Installed", "download-simple", false),
];

/// Search results from: all (repos, Flathub) | repo | flathub | aur
pub const FILTERS: [&str; 4] = ["all", "repo", "flathub", "aur"];

/// Cards a row and a card's width for the content area's width
/// (24 px margins; 300 px cards at least, 12 / 16 apart)
pub fn columns(content_w: f32, material: bool) -> (usize, f32) {
    let gap = if material { 12.0 } else { 16.0 };
    let cols = (((content_w - 48.0 + gap) / (300.0 + gap)).floor() as i64).max(1) as usize;
    let w = (content_w - 48.0 - (cols as f32 - 1.0) * gap) / cols as f32;
    (cols, w)
}

/// The place on show: a category's id, else the view
pub fn place_id<'a>(view: &'a str, category: &'a str) -> &'a str {
    if view == "category" {
        category
    } else {
        view
    }
}

/// Whether a place is the one on show (none while searching)
pub fn is_on(view: &str, category: &str, id: &str) -> bool {
    view != "search" && place_id(view, category) == id
}

/// The view and category a place opens
pub fn view_for(place: usize) -> (&'static str, &'static str) {
    let (id, _, _, cat) = PLACES[place.min(PLACES.len() - 1)];
    if cat {
        ("category", id)
    } else {
        (id, "")
    }
}

pub fn heading(view: &str, category: &str, query: &str) -> String {
    if view == "search" {
        return format!("Results for “{query}”");
    }
    let id = place_id(view, category);
    PLACES.iter().find(|p| p.0 == id).map_or(String::new(), |p| p.1.to_string())
}

pub fn subtitle(view: &str, filter: &str) -> &'static str {
    if view == "installed" {
        "Apps on this computer from the Arch repos, the AUR and Flathub"
    } else if view == "search" && filter == "aur" {
        "Community packages from the AUR"
    } else {
        "From the Arch repos and Flathub"
    }
}

pub fn loading_text(view: &str, filter: &str) -> &'static str {
    if view != "search" {
        "Loading…"
    } else if filter == "aur" {
        "Looking in the AUR…"
    } else {
        "Looking in the Arch repos and on Flathub…"
    }
}

pub fn failed_text(error: &str) -> String {
    format!("Couldn't load the apps ({error}). Check your connection, then choose the place again.")
}

pub fn empty_title(view: &str, query: &str) -> String {
    match view {
        "search" => format!("No apps found for “{query}”"),
        "installed" => "No apps installed from here yet".into(),
        _ => "Nothing here right now".into(),
    }
}

pub fn empty_help(view: &str, filter: &str) -> &'static str {
    match view {
        "search" if filter == "aur" => "Check the spelling, or try a shorter name.",
        "search" => "Check the spelling, or try a shorter name. Community packages are under AUR.",
        "installed" => "Apps you install from Software show here.",
        _ => "The store couldn't reach Flathub. Check your connection.",
    }
}

/// Typed "aur: name" (any case): the AUR's filter
pub fn is_aur_query(q: &str) -> bool {
    q.get(..4).is_some_and(|p| p.eq_ignore_ascii_case("aur:"))
}

/// The query without its "aur:" prefix (and the spaces after it)
pub fn strip_aur(q: &str) -> String {
    if is_aur_query(q) {
        q[4..].trim_start().to_string()
    } else {
        q.to_string()
    }
}

/// A card's button: "…" while busy, Open when installed, else Get (Glass) / Install (Material)
pub fn card_label(busy: &str, installed: bool, material: bool) -> &'static str {
    if !busy.is_empty() {
        "…"
    } else if installed {
        "Open"
    } else if material {
        "Install"
    } else {
        "Get"
    }
}

/// The page's main button: Glass's Get, Material's split button
pub fn main_label(busy: &str, installed: bool, material: bool) -> &'static str {
    match (busy, material) {
        ("installing", _) => "Installing…",
        ("removing", true) => "Removing…",
        _ if installed => "Open",
        (_, true) => "Install",
        _ => "Get",
    }
}

pub fn remove_label(busy: &str) -> &'static str {
    if busy == "removing" {
        "Removing…"
    } else {
        "Remove"
    }
}

/// Hardly anyone uses it: popularity under 0.1 and fewer than 10 votes
pub fn little_used(popularity: f64, votes: f64) -> bool {
    popularity < 0.1 && votes < 10.0
}

/// A number as JavaScript writes it (12, 0.5, 3.25)
pub fn js_number(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

/// The AUR card's text, as Markdown (its first sentence bold)
pub fn aur_text(app: &Value) -> String {
    let pop = app["popularity"].as_f64().unwrap_or(0.0);
    let votes = app["votes"].as_f64().unwrap_or(0.0);
    let mut s = String::from("**A community package from the AUR.** It's built on this computer from a script anyone can upload, not checked by Arch. ");
    if little_used(pop, votes) {
        s += "Hardly anyone uses this one: install it only if you know and trust it. ";
    }
    s += &format!("{} votes · popularity {}", js_number(votes), js_number(pop));
    if truthy(&app["outOfDate"]) {
        s += " · flagged out of date";
    }
    s
}

/// "Package pkg · version · repo"
pub fn package_line(app: &Value) -> String {
    let mut s = format!("Package {}", text(app, "pkg"));
    for k in ["version", "repo"] {
        let v = text(app, k);
        if !v.is_empty() {
            s += " · ";
            s += &v;
        }
    }
    s
}

/// "by …" ("packaged by …" for the AUR), "" without a developer
pub fn developer_line(app: &Value) -> String {
    let d = text(app, "developer");
    if d.is_empty() {
        String::new()
    } else if text(app, "source") == "aur" {
        format!("packaged by {d}")
    } else {
        format!("by {d}")
    }
}

/// AppStore.sourceName, as the feed sent it (or worked out here)
pub fn source_name(app: &Value) -> String {
    let s = text(app, "_source");
    if !s.is_empty() {
        return s;
    }
    match app["source"].as_str() {
        Some("repo") => "Arch repos",
        Some("aur") => "AUR · community",
        Some("flathub") => "Flathub",
        _ => "",
    }
    .into()
}

/// The source chip: where from, and "verified" when Flathub says so
pub fn chip_text(app: &Value) -> String {
    let mut s = source_name(app);
    if truthy(&app["verified"]) {
        s += " · verified";
    }
    s
}

/// "From" in Material's list: where from, and the repo
pub fn from_text(app: &Value) -> String {
    let mut s = source_name(app);
    let r = text(app, "repo");
    if !r.is_empty() {
        s += " · ";
        s += &r;
    }
    s
}

/// The description, when it says more than the summary
pub fn description(app: &Value) -> String {
    let d = text(app, "description");
    if d.is_empty() || d == text(app, "summary") {
        String::new()
    } else {
        d
    }
}

/// A tile's letter when there's no logo
pub fn initial(name: &str) -> String {
    let n = if name.is_empty() { "?" } else { name };
    n.chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_default()
}

/// A field as text ("" when missing; numbers as JS writes them)
pub fn text(app: &Value, key: &str) -> String {
    match &app[key] {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.as_f64().map(js_number).unwrap_or_default(),
        _ => String::new(),
    }
}

/// JavaScript's truthiness, for the app's flags
pub fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|x| x != 0.0 && !x.is_nan()),
        Value::String(s) => !s.is_empty(),
        _ => true,
    }
}

/// Roughly whether a summary needs two lines at `width` (font `size`)
pub fn two_lines(s: &str, width: f32, size: f32) -> bool {
    s.chars().count() as f32 * size * 0.52 > width.max(1.0)
}

/// A card's Get: an AUR package goes to its page first (the warning), an
/// installed app opens, the rest install
#[derive(Debug, PartialEq, Eq)]
pub enum CardAct {
    Open,
    Page,
    Install,
}

pub fn card_act(app: &Value) -> CardAct {
    if truthy(&app["installed"]) {
        CardAct::Open
    } else if app["source"].as_str() == Some("aur") {
        CardAct::Page
    } else {
        CardAct::Install
    }
}

/// Where a logo fetched by URL is kept ($XDG_CACHE_HOME/tessera/appstore-icons,
/// named by the URL's hash, its extension kept for the decoder)
pub fn cached_icon(url: &str) -> std::path::PathBuf {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    url.hash(&mut h);
    let ext = url.rsplit('/').next().and_then(|f| f.rsplit_once('.')).map(|(_, e)| e.to_ascii_lowercase()).filter(|e| ["png", "svg", "jpg", "jpeg", "webp"].contains(&e.as_str())).unwrap_or_else(|| "png".into());
    let base = std::env::var("XDG_CACHE_HOME").ok().filter(|s| !s.is_empty()).map(std::path::PathBuf::from).unwrap_or_else(|| std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".cache"));
    base.join("tessera/appstore-icons").join(format!("{:016x}.{ext}", h.finish()))
}

/// Fetch a logo once (curl, 15 s, 2 MB at most), into `to` (whole or not at all)
pub fn fetch(url: &str, to: &std::path::Path) {
    if to.exists() || !url.starts_with("https://") {
        return;
    }
    let Some(dir) = to.parent() else { return };
    let _ = std::fs::create_dir_all(dir);
    let part = to.with_extension("part");
    let ok = std::process::Command::new("curl")
        .args(["-fsSL", "--proto", "=https", "--max-time", "15", "--max-filesize", "2000000", "-o"])
        .arg(&part)
        .arg(url)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    if ok {
        let _ = std::fs::rename(&part, to);
    } else {
        let _ = std::fs::remove_file(&part);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn cards_fill_the_width_three_hundred_at_least() {
        // 980 wide, a 230 sidebar: 750 of content, 702 for cards: two
        assert_eq!(columns(750.0, false).0, 2);
        assert_eq!(columns(750.0, false).1, (750.0 - 48.0 - 16.0) / 2.0);
        // Material: 1040 - 272
        let (c, w) = columns(768.0, true);
        assert_eq!((c, w), (2, (768.0 - 48.0 - 12.0) / 2.0));
        // narrow: one, wide: four
        assert_eq!(columns(300.0, false).0, 1);
        assert_eq!(columns(1300.0, false).0, 4);
        assert_eq!(columns(10.0, true).0, 1);
    }

    #[test]
    fn headings_and_subtitles_per_view() {
        assert_eq!(heading("discover", "", ""), "Discover");
        assert_eq!(heading("category", "Music & Video", ""), "Music & Video");
        assert_eq!(heading("installed", "", ""), "Installed");
        assert_eq!(heading("search", "", "paint"), "Results for “paint”");
        assert_eq!(subtitle("installed", "all"), "Apps on this computer from the Arch repos, the AUR and Flathub");
        assert_eq!(subtitle("search", "aur"), "Community packages from the AUR");
        assert_eq!(subtitle("search", "repo"), "From the Arch repos and Flathub");
        assert_eq!(subtitle("discover", "aur"), "From the Arch repos and Flathub");
        assert_eq!(loading_text("category", "aur"), "Loading…");
        assert_eq!(loading_text("search", "aur"), "Looking in the AUR…");
        assert_eq!(loading_text("search", "all"), "Looking in the Arch repos and on Flathub…");
    }

    #[test]
    fn places_open_their_views() {
        assert_eq!(view_for(0), ("discover", ""));
        assert_eq!(view_for(3), ("category", "Graphics & Design"));
        assert_eq!(view_for(10), ("installed", ""));
        assert!(is_on("category", "Games", "Games"));
        assert!(is_on("discover", "", "discover"));
        assert!(!is_on("search", "", "discover"));
        assert!(!is_on("category", "Games", "System"));
    }

    #[test]
    fn empty_states_say_what_to_do() {
        assert_eq!(empty_title("search", "zzq"), "No apps found for “zzq”");
        assert_eq!(empty_title("installed", ""), "No apps installed from here yet");
        assert_eq!(empty_title("discover", ""), "Nothing here right now");
        assert!(empty_help("search", "all").ends_with("Community packages are under AUR."));
        assert_eq!(empty_help("search", "aur"), "Check the spelling, or try a shorter name.");
        assert_eq!(empty_help("installed", "all"), "Apps you install from Software show here.");
        assert_eq!(empty_help("category", "all"), "The store couldn't reach Flathub. Check your connection.");
        assert_eq!(failed_text("timeout"), "Couldn't load the apps (timeout). Check your connection, then choose the place again.");
    }

    #[test]
    fn aur_prefix() {
        assert!(is_aur_query("aur: paru"));
        assert!(is_aur_query("AUR:paru"));
        assert!(!is_aur_query("au"));
        assert!(!is_aur_query("paru aur:"));
        assert_eq!(strip_aur("aur:   paru"), "paru");
        assert_eq!(strip_aur("Aur:x"), "x");
        assert_eq!(strip_aur("gimp"), "gimp");
    }

    #[test]
    fn buttons_per_state() {
        assert_eq!(card_label("", false, false), "Get");
        assert_eq!(card_label("", false, true), "Install");
        assert_eq!(card_label("", true, true), "Open");
        assert_eq!(card_label("installing", false, false), "…");
        assert_eq!(main_label("installing", false, false), "Installing…");
        assert_eq!(main_label("removing", true, false), "Open"); // (Glass: Remove says it)
        assert_eq!(main_label("removing", true, true), "Removing…");
        assert_eq!(main_label("", true, true), "Open");
        assert_eq!(main_label("", false, true), "Install");
        assert_eq!(main_label("", false, false), "Get");
        assert_eq!(remove_label("removing"), "Removing…");
        assert_eq!(remove_label(""), "Remove");
        assert_eq!(card_act(&json!({"installed": true, "source": "aur"})), CardAct::Open);
        assert_eq!(card_act(&json!({"source": "aur"})), CardAct::Page);
        assert_eq!(card_act(&json!({"source": "flathub"})), CardAct::Install);
    }

    #[test]
    fn the_aur_warning() {
        assert!(little_used(0.05, 3.0));
        assert!(!little_used(0.05, 10.0));
        assert!(!little_used(0.1, 0.0));
        let t = aur_text(&json!({"votes": 2, "popularity": 0.01, "outOfDate": 1700000000}));
        assert!(t.starts_with("**A community package from the AUR.**"));
        assert!(t.contains("Hardly anyone uses this one"));
        assert!(t.ends_with("2 votes · popularity 0.01 · flagged out of date"));
        let t = aur_text(&json!({"votes": 812, "popularity": 14.5}));
        assert!(!t.contains("Hardly"));
        assert!(t.ends_with("812 votes · popularity 14.5"));
    }

    #[test]
    fn the_pages_lines() {
        let app = json!({"name": "Inkpad", "pkg": "inkpad", "version": "2.1-1", "repo": "extra", "source": "repo", "developer": "Inkpad Team", "summary": "Draw", "description": "Draw"});
        assert_eq!(package_line(&app), "Package inkpad · 2.1-1 · extra");
        assert_eq!(package_line(&json!({"pkg": "x"})), "Package x");
        assert_eq!(developer_line(&app), "by Inkpad Team");
        assert_eq!(developer_line(&json!({"developer": "sam", "source": "aur"})), "packaged by sam");
        assert_eq!(chip_text(&json!({"source": "flathub", "verified": true})), "Flathub · verified");
        assert_eq!(from_text(&app), "Arch repos · extra");
        assert_eq!(description(&app), ""); // (the same as the summary)
        assert_eq!(initial("inkpad"), "I");
        assert_eq!(initial(""), "?");
    }

    #[test]
    fn fetched_logos_are_kept_by_url() {
        let a = cached_icon("https://dl.example.org/media/a/icon.png");
        assert_eq!(a, cached_icon("https://dl.example.org/media/a/icon.png"));
        assert_ne!(a, cached_icon("https://dl.example.org/media/b/icon.png"));
        assert!(a.to_string_lossy().ends_with(".png") && a.to_string_lossy().contains("tessera/appstore-icons/"));
        assert!(cached_icon("https://dl.example.org/x/icon.svg").to_string_lossy().ends_with(".svg"));
        assert!(cached_icon("https://dl.example.org/x/icon").to_string_lossy().ends_with(".png"));
    }
}

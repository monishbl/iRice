//! Icons by freedesktop name: the tray's IconName, an app's Icon= from its
//! desktop entry (the spaces' app icons). A small lookup, not the whole spec:
//! the user's GTK icon theme, then Papirus, breeze, Adwaita and hicolor,
//! trying the sizes a 16-32 px icon looks best at (scalable first), then
//! /usr/share/pixmaps. Answers are remembered (names don't move while it runs).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

fn data_dirs() -> Vec<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    let user = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".local/share"));
    let sys = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    let mut v = vec![home.join(".icons"), user.join("icons")];
    v.extend(sys.split(':').filter(|s| !s.is_empty()).map(|d| PathBuf::from(d).join("icons")));
    v
}

/// The GTK icon theme the user chose (gtk-3.0/settings.ini), if any
fn user_theme() -> Option<String> {
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let cfg = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".config"));
    let text = std::fs::read_to_string(cfg.join("gtk-3.0/settings.ini")).ok()?;
    text.lines().find_map(|l| l.trim().strip_prefix("gtk-icon-theme-name").map(|v| v.trim_start_matches([' ', '=']).trim().trim_matches('"').to_string()))
}

const SIZES: &[&str] = &["scalable", "32x32", "32", "24x24", "24", "22x22", "22", "48x48", "48", "16x16", "16", "64x64", "64", "128x128", "256x256", "symbolic"];
const KINDS: &[&str] = &["apps", "status", "devices", "panel", "actions", "categories", "places", "mimetypes", "emblems", "legacy"];

/// Where a name's file is in one theme folder (both layouts: size/kind and kind/size)
pub fn in_theme(dir: &Path, name: &str) -> Option<PathBuf> {
    for size in SIZES {
        for kind in KINDS {
            for (a, b) in [(size, kind), (kind, size)] {
                for ext in ["svg", "png"] {
                    let p = dir.join(a).join(b).join(format!("{name}.{ext}"));
                    if p.is_file() {
                        return Some(p);
                    }
                }
            }
        }
    }
    None
}

fn lookup(name: &str, extra: Option<&Path>) -> Option<PathBuf> {
    if name.is_empty() {
        return None;
    }
    if name.starts_with('/') {
        return Path::new(name).is_file().then(|| PathBuf::from(name));
    }
    // an app's own folder (the tray's IconThemePath): flat, or a theme
    if let Some(e) = extra {
        for ext in ["svg", "png"] {
            let p = e.join(format!("{name}.{ext}"));
            if p.is_file() {
                return Some(p);
            }
        }
        if let Some(p) = in_theme(e, name).or_else(|| in_theme(&e.join("hicolor"), name)) {
            return Some(p);
        }
    }
    // The shell's own index (services/icons.rs: the theme Settings chose,
    // e.g. Colloid, the themes it inherits, hicolor, the pixmaps; the QML's
    // IconLookup), with its name variants (case, a reverse-DNS id's last part)
    if let Some(icons) = crate::services::icons::shared() {
        if let Some(p) = icons.exact(name) {
            return Some(p);
        }
    }
    // (before the index is built, or a name it hasn't: the themes walked
    // here, the shell's theme and what it inherits first, as the QML's
    // Quickshell.iconPath fallback does)
    // (read once: the shell's theme is chosen at start, as Quickshell's)
    static SHELL_CHAIN: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    let mut themes: Vec<String> = SHELL_CHAIN
        .get_or_init(|| {
            let roots = crate::services::icons::roots();
            crate::services::icons::chain_of(&roots, &crate::services::icons::theme_name(&crate::theme::read_json()))
        })
        .clone();
    themes.extend(user_theme());
    for t in ["Papirus", "breeze", "Adwaita", "hicolor"] {
        if !themes.iter().any(|x| x == t) {
            themes.push(t.into());
        }
    }
    let dirs = data_dirs();
    for t in &themes {
        for d in &dirs {
            if let Some(p) = in_theme(&d.join(t), name) {
                return Some(p);
            }
        }
    }
    for ext in ["svg", "png", "xpm"] {
        let p = PathBuf::from(format!("/usr/share/pixmaps/{name}.{ext}"));
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

static CACHE: Mutex<Option<HashMap<String, Option<PathBuf>>>> = Mutex::new(None);

/// The file for an icon name (`extra`: a folder of the app's own icons)
pub fn find(name: &str, extra: Option<&Path>) -> Option<PathBuf> {
    let key = format!("{name}\n{}", extra.map(|p| p.to_string_lossy().into_owned()).unwrap_or_default());
    if let Some(hit) = CACHE.lock().ok().and_then(|c| c.as_ref().and_then(|m| m.get(&key).cloned())) {
        return hit;
    }
    let found = lookup(name, extra);
    // (a miss before the shell's index is built isn't kept: asked again once it is)
    if found.is_some() || crate::services::icons::shared().is_some() {
        if let Ok(mut c) = CACHE.lock() {
            c.get_or_insert_with(HashMap::new).insert(key, found.clone());
        }
    }
    found
}

/// The shell's index was (re)built: answers found without it are dropped
pub fn forget() {
    if let Ok(mut c) = CACHE.lock() {
        *c = None;
    }
}

/// An app's icon by its window class (as Apps.iconFor: its desktop entry's
/// Icon=, else the class itself as an icon name)
pub fn for_class(class: &str) -> Option<PathBuf> {
    let icon = crate::util::desktop_field(class, "Icon").unwrap_or_else(|| class.to_lowercase());
    find(&icon, None).or_else(|| find(class, None))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_theme_layouts() {
        let d = std::env::temp_dir().join(format!("tessera-icons-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("t1/32x32/apps")).unwrap();
        std::fs::write(d.join("t1/32x32/apps/foo.png"), b"x").unwrap();
        std::fs::create_dir_all(d.join("t2/apps/scalable")).unwrap();
        std::fs::write(d.join("t2/apps/scalable/bar.svg"), b"x").unwrap();
        assert_eq!(in_theme(&d.join("t1"), "foo"), Some(d.join("t1/32x32/apps/foo.png")));
        assert_eq!(in_theme(&d.join("t2"), "bar"), Some(d.join("t2/apps/scalable/bar.svg")));
        assert_eq!(in_theme(&d.join("t2"), "foo"), None);
        // an app's own flat folder wins
        std::fs::write(d.join("own.png"), b"x").unwrap();
        assert_eq!(lookup("own", Some(&d)), Some(d.join("own.png")));
        assert_eq!(lookup("", None), None);
        let _ = std::fs::remove_dir_all(&d);
    }
}

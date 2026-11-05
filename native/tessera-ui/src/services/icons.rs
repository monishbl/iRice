//! Icon names to files, as the QML shell's IconLookup.qml: the icon theme
//! the shell started with ($QS_ICON_THEME, else launch-shell.sh's choice),
//! indexed once (name -> the best file: the theme, the themes it inherits,
//! hicolor, then /usr/share/pixmaps; scalable and large over small,
//! full-colour over symbolic: assets/scripts/icon-index.py's rules). The
//! shell's own cached index is read when it's there; otherwise the themes
//! are walked here. The service sends `Icons` once it's built.

use crate::Msg;
use smithay_client_toolkit::reexports::calloop::channel::Sender;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Debug, Default)]
pub struct Icons {
    pub theme: String,
    pub index: Arc<HashMap<String, PathBuf>>,
}

/// Folders icon themes live in (icon-index.py's theme_dirs)
pub fn roots() -> Vec<PathBuf> {
    let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
    let data_home = std::env::var("XDG_DATA_HOME").ok().filter(|s| !s.is_empty()).map(PathBuf::from).unwrap_or_else(|| home.join(".local/share"));
    let mut dirs = vec![data_home.join("icons"), home.join(".local/share/icons"), home.join(".icons")];
    let sys = std::env::var("XDG_DATA_DIRS").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    dirs.extend(sys.split(':').filter(|d| !d.is_empty()).map(|d| PathBuf::from(d).join("icons")));
    let mut out: Vec<PathBuf> = Vec::new();
    for d in dirs {
        if d.is_dir() && !out.contains(&d) {
            out.push(d);
        }
    }
    out
}

fn theme_paths(roots: &[PathBuf], name: &str) -> Vec<PathBuf> {
    roots.iter().map(|r| r.join(name)).filter(|p| p.join("index.theme").is_file()).collect()
}

/// A usable theme: has an index.theme that lists folders (launch-shell.sh's installed())
fn installed(roots: &[PathBuf], name: &str) -> bool {
    !name.is_empty() && theme_paths(roots, name).iter().any(|p| std::fs::read_to_string(p.join("index.theme")).is_ok_and(|t| t.lines().any(|l| l.starts_with("Directories="))))
}

/// The theme the shell uses: $QS_ICON_THEME (the shell sets it before it
/// starts, and this process inherits it), else what launch-shell.sh would
/// pick: Settings' iconTheme, Adwaita, KDE's, GTK's, then the first good one
pub fn theme_name(cfg: &serde_json::Value) -> String {
    if let Ok(t) = std::env::var("QS_ICON_THEME") {
        if !t.is_empty() {
            return t;
        }
    }
    let roots = roots();
    let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
    let config = std::env::var("XDG_CONFIG_HOME").ok().filter(|s| !s.is_empty()).map(PathBuf::from).unwrap_or_else(|| home.join(".config"));
    let mut pick = cfg["appearance"]["iconTheme"].as_str().unwrap_or("").to_string();
    if pick.is_empty() && installed(&roots, "Adwaita") {
        pick = "Adwaita".into();
    }
    let ini = |file: PathBuf, section: Option<&str>, key: &str| -> String {
        let text = std::fs::read_to_string(file).unwrap_or_default();
        let mut on = section.is_none();
        for l in text.lines() {
            if l.starts_with('[') {
                on = section.is_none_or(|s| l == s);
            } else if on {
                if let Some(v) = l.strip_prefix(key).and_then(|r| r.trim_start().strip_prefix('=')) {
                    return v.trim().trim_matches('"').to_string();
                }
            }
        }
        String::new()
    };
    if pick.is_empty() {
        pick = ini(config.join("kdeglobals"), Some("[Icons]"), "Theme");
    }
    if pick.is_empty() {
        pick = ini(config.join("gtk-4.0/settings.ini"), None, "gtk-icon-theme-name");
    }
    if pick.is_empty() {
        pick = ini(config.join("gtk-3.0/settings.ini"), None, "gtk-icon-theme-name");
    }
    if !installed(&roots, &pick) {
        pick = ["Papirus-Dark", "Papirus", "breeze-plus-dark", "breeze-dark", "breeze", "Adwaita"].iter().find(|t| installed(&roots, t)).map(|t| t.to_string()).unwrap_or("hicolor".into());
    }
    pick
}

/// The theme, the ones it inherits (breadth first), then hicolor
fn chain(roots: &[PathBuf], name: &str) -> Vec<String> {
    let mut order: Vec<String> = Vec::new();
    let mut todo = vec![name.to_string()];
    while !todo.is_empty() {
        let t = todo.remove(0);
        if order.contains(&t) || theme_paths(roots, &t).is_empty() {
            continue;
        }
        for p in theme_paths(roots, &t) {
            if let Ok(text) = std::fs::read_to_string(p.join("index.theme")) {
                if let Some(l) = text.lines().find(|l| l.starts_with("Inherits=")) {
                    todo.extend(l[9..].split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()));
                    break;
                }
            }
        }
        order.push(t);
    }
    if !order.iter().any(|t| t == "hicolor") {
        order.push("hicolor".into());
    }
    order
}

/// Higher is better: scalable > big > small; symbolic last (icon-index.py)
pub fn score(path: &str) -> i32 {
    let low = path.to_lowercase();
    let mut s = 0;
    if low.contains("symbolic") {
        s -= 1000;
    }
    if low.contains("scalable") {
        s += 500;
    }
    for part in low.split('/') {
        let digits = part.split('x').next().unwrap_or("").split('@').next().unwrap_or("");
        if !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()) {
            s += digits.parse::<i32>().unwrap_or(0).min(256);
            break;
        }
    }
    if low.contains("@2x") {
        s += 50;
    }
    if low.ends_with(".svg") {
        s += 10;
    }
    s
}

fn walk(dir: &Path, seen: &mut std::collections::HashSet<(u64, u64)>, f: &mut dyn FnMut(&Path)) {
    use std::os::unix::fs::MetadataExt;
    let Ok(md) = std::fs::metadata(dir) else { return };
    if !seen.insert((md.dev(), md.ino())) {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, seen, f);
        } else {
            f(&p);
        }
    }
}

/// Every icon in a theme's chain, mapped to its best file
pub fn build(roots: &[PathBuf], theme: &str) -> HashMap<String, PathBuf> {
    let mut best: HashMap<String, ((usize, i32), PathBuf)> = HashMap::new();
    for (rank, t) in chain(roots, theme).iter().enumerate() {
        for base in theme_paths(roots, t) {
            let mut seen = std::collections::HashSet::new();
            walk(&base, &mut seen, &mut |p| {
                let Some(f) = p.file_name().and_then(|f| f.to_str()) else { return };
                let Some(name) = f.strip_suffix(".svg").or_else(|| f.strip_suffix(".png")) else { return };
                let key = (rank, -score(&p.to_string_lossy()));
                match best.get(name) {
                    Some((k, _)) if *k <= key => {}
                    _ => {
                        best.insert(name.to_string(), (key, p.to_path_buf()));
                    }
                }
            });
        }
    }
    if let Ok(rd) = std::fs::read_dir("/usr/share/pixmaps") {
        for e in rd.flatten() {
            let f = e.file_name().to_string_lossy().into_owned();
            if let Some(name) = f.strip_suffix(".svg").or_else(|| f.strip_suffix(".png")) {
                best.entry(name.to_string()).or_insert(((99, 0), e.path()));
            }
        }
    }
    best.into_iter().map(|(k, (_, p))| (k, p)).collect()
}

/// The shell's cached index for a theme ($XDG_CACHE_HOME/tessera/icon-index-<theme>.tsv)
fn cached(theme: &str) -> Option<HashMap<String, PathBuf>> {
    let base = std::env::var("XDG_CACHE_HOME").ok().filter(|s| !s.is_empty()).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".cache"));
    let text = ["tessera", "irice"].iter().find_map(|d| std::fs::read_to_string(base.join(d).join(format!("icon-index-{theme}.tsv"))).ok())?;
    let map: HashMap<String, PathBuf> = text.lines().filter_map(|l| l.split_once('\t')).map(|(n, p)| (n.to_string(), PathBuf::from(p))).collect();
    (!map.is_empty()).then_some(map)
}

pub fn spawn(tx: Sender<Msg>) {
    std::thread::spawn(move || {
        let theme = theme_name(&crate::theme::read_json());
        let index = cached(&theme).unwrap_or_else(|| build(&roots(), &theme));
        let _ = tx.send(Msg::Data(Box::new(Icons { theme, index: Arc::new(index) })));
    });
}

impl Icons {
    fn find(&self, n: &str) -> Option<PathBuf> {
        self.index.get(n).filter(|p| p.exists()).cloned()
    }

    /// The first of `names` the theme has (with the usual variants: lower
    /// case, a reverse-DNS id's last part, dashes for spaces), then the
    /// fallbacks, then their symbolic versions, then a generic app icon
    /// (IconLookup.path). A name that's a file path is that file.
    pub fn path(&self, names: &[&str], fallbacks: &[&str]) -> Option<PathBuf> {
        let names: Vec<&str> = names.iter().copied().filter(|n| !n.is_empty()).collect();
        for n in &names {
            if n.starts_with('/') {
                return Some(PathBuf::from(n));
            }
            if let Some(p) = n.strip_prefix("file://") {
                return Some(PathBuf::from(p));
            }
        }
        for n in &names {
            if let Some(p) = self.exact(n) {
                return Some(p);
            }
        }
        for f in fallbacks {
            if let Some(p) = self.find(f) {
                return Some(p);
            }
        }
        for n in names.iter().chain(fallbacks) {
            if let Some(p) = self.find(&format!("{n}-symbolic")) {
                return Some(p);
            }
        }
        self.find("application-x-executable").or_else(|| self.find("application-default-icon"))
    }

    /// One name and its variants, without any fallback (IconLookup.exact)
    pub fn exact(&self, n: &str) -> Option<PathBuf> {
        let last = n.rsplit('.').next().unwrap_or(n);
        let spaced = n.split_whitespace().collect::<Vec<_>>().join("-").to_lowercase();
        [n.to_string(), n.to_lowercase(), last.to_string(), last.to_lowercase(), spaced].iter().find_map(|v| self.find(v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("tessera-icons-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }
    fn put(p: PathBuf) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, "<svg/>").unwrap();
    }

    #[test]
    fn scalable_beats_small_and_symbolic_comes_last() {
        assert!(score("/i/Papirus/scalable/apps/foot.svg") > score("/i/Papirus/64x64/apps/foot.svg"));
        assert!(score("/i/Papirus/64x64/apps/foot.svg") > score("/i/Papirus/16x16/apps/foot.png"));
        assert!(score("/i/Adwaita/symbolic/places/user-trash-symbolic.svg") < score("/i/Adwaita/16x16/places/user-trash.png"));
    }

    #[test]
    fn finds_icons_through_the_theme_chain() {
        let d = tmp("chain");
        let icons = d.join("icons");
        std::fs::create_dir_all(icons.join("Papirus-Dark")).unwrap();
        std::fs::write(icons.join("Papirus-Dark/index.theme"), "[Icon Theme]\nName=Papirus-Dark\nInherits=Papirus,hicolor\nDirectories=64x64/apps\n").unwrap();
        std::fs::create_dir_all(icons.join("Papirus")).unwrap();
        std::fs::write(icons.join("Papirus/index.theme"), "[Icon Theme]\nName=Papirus\nDirectories=64x64/apps\n").unwrap();
        std::fs::create_dir_all(icons.join("hicolor")).unwrap();
        std::fs::write(icons.join("hicolor/index.theme"), "[Icon Theme]\nName=hicolor\nDirectories=48x48/apps\n").unwrap();
        put(icons.join("Papirus-Dark/16x16/apps/firefox.svg"));
        put(icons.join("Papirus-Dark/64x64/apps/firefox.svg"));
        put(icons.join("Papirus/64x64/apps/foot.svg")); // inherited
        put(icons.join("Papirus-Dark/64x64/places/user-trash.svg"));
        put(icons.join("hicolor/48x48/apps/firefox.png")); // the theme's own wins
        put(icons.join("hicolor/48x48/apps/org.gnome.Calculator.png"));
        put(icons.join("hicolor/48x48/apps/application-x-executable.png"));
        let roots = vec![icons.clone()];
        assert!(installed(&roots, "Papirus-Dark") && !installed(&roots, "Breeze"));
        let i = Icons { theme: "Papirus-Dark".into(), index: Arc::new(build(&roots, "Papirus-Dark")) };
        assert_eq!(i.path(&["firefox"], &[]), Some(icons.join("Papirus-Dark/64x64/apps/firefox.svg")));
        assert_eq!(i.path(&["foot"], &[]), Some(icons.join("Papirus/64x64/apps/foot.svg")));
        // variants: case, and a reverse-DNS id's last part
        assert_eq!(i.path(&["org.gnome.Calculator"], &[]), Some(icons.join("hicolor/48x48/apps/org.gnome.Calculator.png")));
        assert_eq!(i.path(&["Foot"], &[]), Some(icons.join("Papirus/64x64/apps/foot.svg")));
        assert_eq!(i.path(&["org.mozilla.Firefox"], &[]), Some(icons.join("Papirus-Dark/64x64/apps/firefox.svg")));
        // the first name that exists, then fallbacks, then the generic icon
        assert_eq!(i.path(&["user-trash-full", "trash-full", "user-trash"], &["edit-delete"]), Some(icons.join("Papirus-Dark/64x64/places/user-trash.svg")));
        assert_eq!(i.path(&["nothing"], &["user-trash"]), Some(icons.join("Papirus-Dark/64x64/places/user-trash.svg")));
        assert_eq!(i.path(&["nothing"], &[]), Some(icons.join("hicolor/48x48/apps/application-x-executable.png")));
        assert_eq!(i.exact("nothing"), None);
        // a path is itself
        assert_eq!(i.path(&["/opt/app/icon.png"], &[]), Some(PathBuf::from("/opt/app/icon.png")));
        let _ = std::fs::remove_dir_all(&d);
    }
}

//! What both looks' app lists show and do (shell/modules/launchpad/
//! AppLibraryModel.qml): every app, grouped into category folders or listed
//! A–Z, the search over them, the selection, the letter index, and an app's
//! menu (its desktop file's own actions, Open, Keep in Dock).

use crate::services::apps::Entry;
use crate::rank;

/// [label, freedesktop categories]; apps go in the first match
pub const GROUPS: &[(&str, &[&str])] = &[
    ("Productivity", &["Office", "Calendar", "ContactManagement", "Finance", "WordProcessor", "Spreadsheet"]),
    ("Development", &["Development", "IDE", "TextEditor"]),
    ("Internet", &["Network", "WebBrowser", "Email", "Chat", "InstantMessaging"]),
    ("Creativity", &["Graphics", "Photography", "2DGraphics", "3DGraphics", "VectorGraphics", "RasterGraphics"]),
    ("Entertainment", &["AudioVideo", "Audio", "Video", "Player", "Music"]),
    ("Games", &["Game"]),
    ("Education", &["Education", "Science", "Math"]),
    ("Utilities", &["Utility", "Accessories", "FileManager", "TerminalEmulator", "Archiving"]),
    ("System", &["System", "Settings", "Monitor", "PackageManager", "HardwareSettings"]),
];

#[derive(Clone, Debug, PartialEq)]
pub struct App {
    pub id: String,
    pub name: String,
    pub generic: String,
    pub keywords: Vec<String>,
    pub icon: String,
    pub categories: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Folder {
    pub name: String,
    /// indexes into the apps
    pub apps: Vec<usize>,
}

/// Every app that shows, A–Z (case aside: "mpv" goes with the M's)
pub fn sorted(mut apps: Vec<App>) -> Vec<App> {
    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.name.cmp(&b.name)));
    apps
}

pub fn folders(apps: &[App]) -> Vec<Folder> {
    let mut out: Vec<Folder> = GROUPS.iter().map(|g| Folder { name: g.0.into(), apps: Vec::new() }).chain(std::iter::once(Folder { name: "Other".into(), apps: Vec::new() })).collect();
    for (i, a) in apps.iter().enumerate() {
        let g = GROUPS.iter().position(|g| g.1.iter().any(|c| a.categories.iter().any(|x| x == c))).unwrap_or(GROUPS.len());
        out[g].apps.push(i);
    }
    out.retain(|f| !f.apps.is_empty());
    out
}

/// Search: Search.score of the name, the generic name (×0.8) and the
/// keywords (×0.6), at least 30, best first
pub fn search(apps: &[App], query: &str) -> Vec<usize> {
    let mut scored: Vec<(usize, f64)> = apps
        .iter()
        .enumerate()
        .map(|(i, a)| (i, rank::score(&a.name, query).max(rank::score(&a.generic, query) * 0.8).max(rank::score(&a.keywords.join(" "), query) * 0.6)))
        .filter(|(_, s)| *s >= 30.0)
        .collect();
    scored.sort_by(|a, b| b.1.total_cmp(&a.1));
    scored.into_iter().map(|(i, _)| i).collect()
}

/// A–Z's letter index: each letter's first app (by place in the list), "#"
/// for names that don't start with A–Z
pub fn letters(apps: &[App]) -> Vec<(String, usize)> {
    let mut out: Vec<(String, usize)> = Vec::new();
    for (i, a) in apps.iter().enumerate() {
        let c = a.name.chars().next().map(|c| c.to_uppercase().collect::<String>()).unwrap_or_default();
        let l = if c.len() == 1 && c.chars().all(|c| c.is_ascii_uppercase()) { c } else { "#".into() };
        if out.last().map(|x| x.0 != l).unwrap_or(true) {
            out.push((l, i));
        }
    }
    out
}

/// The letter of the app at place i
pub fn letter_at(letters: &[(String, usize)], i: usize) -> String {
    letters.iter().filter(|x| x.1 <= i).last().map(|x| x.0.clone()).unwrap_or_default()
}

/// What the list shows and how it's chosen
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Lib {
    pub apps: Vec<App>,
    pub folders: Vec<Folder>,
    pub letters: Vec<(String, usize)>,
    /// 0 folders, 1 all apps
    pub view: i32,
    pub folder: Option<usize>,
    pub query: String,
    pub selected: usize,
    /// a letter picked on the index: its apps lit, the rest dimmed
    pub picked: String,
}

pub enum Activated {
    Nothing,
    Launch(usize),
    Opened,
}

impl Lib {
    pub fn set_apps(&mut self, apps: Vec<App>) {
        self.apps = sorted(apps);
        self.folders = folders(&self.apps);
        self.letters = letters(&self.apps);
        if self.folder.is_some_and(|f| f >= self.folders.len()) {
            self.folder = None;
        }
        self.clamp();
    }

    pub fn showing_apps(&self) -> bool {
        !self.query.is_empty() || self.view == 1 || self.folder.is_some()
    }

    pub fn alphabetical(&self) -> bool {
        self.query.is_empty() && self.view == 1 && self.folder.is_none()
    }

    /// The apps in view (indexes), when it shows apps
    pub fn visible(&self) -> Vec<usize> {
        if !self.query.is_empty() {
            search(&self.apps, &self.query)
        } else if let Some(f) = self.folder.and_then(|f| self.folders.get(f)) {
            f.apps.clone()
        } else {
            (0..self.apps.len()).collect()
        }
    }

    pub fn count(&self) -> usize {
        if self.showing_apps() {
            self.visible().len()
        } else {
            self.folders.len()
        }
    }

    fn clamp(&mut self) {
        self.selected = self.selected.min(self.count().saturating_sub(1));
    }

    /// Back to the start: no search, no folder, the first one chosen
    pub fn reset(&mut self) {
        self.query.clear();
        self.folder = None;
        self.selected = 0;
        self.picked.clear();
    }

    pub fn set_query(&mut self, q: &str) {
        self.query = q.trim().to_lowercase();
        self.selected = 0;
        self.picked.clear();
    }

    pub fn toggle_view(&mut self) {
        self.view = 1 - self.view;
        self.folder = None;
        self.selected = 0;
        self.picked.clear();
    }

    pub fn set_view(&mut self, v: i32) {
        self.view = v;
        self.folder = None;
        self.selected = 0;
        self.picked.clear();
    }

    pub fn activate(&mut self, i: usize) -> Activated {
        if self.showing_apps() {
            match self.visible().get(i) {
                Some(&a) => Activated::Launch(a),
                None => Activated::Nothing,
            }
        } else if i < self.folders.len() {
            self.folder = Some(i);
            self.selected = 0;
            Activated::Opened
        } else {
            Activated::Nothing
        }
    }

    /// Escape: the search, then the folder; true when there's nothing left (close)
    pub fn back(&mut self, text_empty: bool) -> bool {
        if !text_empty {
            false
        } else if let Some(f) = self.folder.take() {
            self.selected = f;
            false
        } else {
            true
        }
    }

    pub fn select(&mut self, i: usize) {
        self.selected = i.min(self.count().saturating_sub(1));
        if !self.picked.is_empty() && self.alphabetical() && letter_at(&self.letters, self.selected) != self.picked {
            self.picked.clear();
        }
    }

    pub fn step(&mut self, d: i32) {
        let n = self.count() as i32;
        self.select((self.selected as i32 + d).clamp(0, (n - 1).max(0)) as usize);
    }

    /// A letter picked on the index: its first app selected, its apps lit
    pub fn pick(&mut self, k: usize) -> Option<usize> {
        let (l, i) = self.letters.get(k)?.clone();
        self.picked = l;
        self.selected = i;
        Some(i)
    }

    pub fn lit(&self, i: usize) -> bool {
        !self.picked.is_empty() && self.alphabetical() && letter_at(&self.letters, i) == self.picked
    }

    /// The page shown changed (cells come in again): what it shows
    pub fn page(&self) -> String {
        format!("{}/{}/{}", if self.showing_apps() { "apps" } else { "folders" }, self.folder.and_then(|f| self.folders.get(f)).map(|f| f.name.as_str()).unwrap_or(""), if self.query.is_empty() { self.view.to_string() } else { "search".into() })
    }
}

/// A desktop file's own actions ([Desktop Action …]: New Window…): (name, Exec)
pub fn actions(text: &str) -> Vec<(String, String)> {
    let mut listed: Vec<String> = Vec::new();
    let mut out: Vec<(String, String, String)> = Vec::new(); // id, name, exec
    let mut group = String::new();
    for line in text.lines() {
        let l = line.trim();
        if l.starts_with('[') && l.ends_with(']') {
            group = l[1..l.len() - 1].to_string();
            if let Some(id) = group.strip_prefix("Desktop Action ") {
                out.push((id.trim().to_string(), String::new(), String::new()));
            }
            continue;
        }
        let Some((k, v)) = l.split_once('=') else { continue };
        let (k, v) = (k.trim(), v.trim());
        if group == "Desktop Entry" && k == "Actions" {
            listed = v.split(';').map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect();
        } else if group.starts_with("Desktop Action ") {
            if let Some(a) = out.last_mut() {
                match k {
                    "Name" => a.1 = v.to_string(),
                    "Exec" => a.2 = v.to_string(),
                    _ => {}
                }
            }
        }
    }
    listed.iter().filter_map(|id| out.iter().find(|a| &a.0 == id)).filter(|a| !a.1.is_empty() && !a.2.is_empty()).map(|a| (a.1.clone(), a.2.clone())).collect()
}

/// An entry as the list keeps it
pub fn app_of(e: &Entry, categories: Vec<String>) -> App {
    App { id: e.id.clone(), name: e.name.clone(), generic: e.generic_name.clone(), keywords: e.keywords.clone(), icon: e.icon.clone(), categories }
}

/// The desktop file for an id, in the data dirs' applications folders
pub fn desktop_file(id: &str) -> Option<std::path::PathBuf> {
    for d in crate::services::apps::data_dirs() {
        let base = d.join("applications");
        let direct = base.join(format!("{id}.desktop"));
        if direct.is_file() {
            return Some(direct);
        }
        // (ids made from subfolders: kde-org.foo → kde/org.foo)
        let mut parts: Vec<&str> = id.split('-').collect();
        while parts.len() > 1 {
            let head = parts.remove(0);
            let p = base.join(head).join(format!("{}.desktop", parts.join("-")));
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

/// Its Categories= list
pub fn categories(text: &str) -> Vec<String> {
    let mut in_entry = false;
    for line in text.lines() {
        let l = line.trim();
        if l.starts_with('[') {
            in_entry = l == "[Desktop Entry]";
            continue;
        }
        if in_entry {
            if let Some(v) = l.strip_prefix("Categories=") {
                return v.split(';').map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect();
            }
        }
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(name: &str, cats: &[&str]) -> App {
        App { id: name.to_lowercase(), name: name.into(), generic: String::new(), keywords: Vec::new(), icon: String::new(), categories: cats.iter().map(|c| c.to_string()).collect() }
    }

    fn lib() -> Lib {
        let mut l = Lib::default();
        l.set_apps(vec![app("mpv", &["AudioVideo", "Player"]), app("Firefox", &["Network", "WebBrowser"]), app("foot", &["System", "TerminalEmulator"]), app("Zed", &["Development"]), app("2048", &["Game"]), app("Ark", &[])]);
        l
    }

    #[test]
    fn a_to_z_whatever_the_case() {
        let l = lib();
        let names: Vec<&str> = l.apps.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, vec!["2048", "Ark", "Firefox", "foot", "mpv", "Zed"]);
        let lt: Vec<(&str, usize)> = l.letters.iter().map(|(a, b)| (a.as_str(), *b)).collect();
        assert_eq!(lt, vec![("#", 0), ("A", 1), ("F", 2), ("M", 4), ("Z", 5)]);
        assert_eq!(letter_at(&l.letters, 3), "F");
    }

    #[test]
    fn folders_by_the_first_matching_group() {
        let l = lib();
        let f: Vec<(&str, usize)> = l.folders.iter().map(|f| (f.name.as_str(), f.apps.len())).collect();
        // (foot is System and a terminal: Utilities comes first)
        assert_eq!(f, vec![("Development", 1), ("Internet", 1), ("Entertainment", 1), ("Games", 1), ("Utilities", 1), ("Other", 1)]);
    }

    #[test]
    fn keys_and_views() {
        let mut l = lib();
        assert!(!l.showing_apps());
        assert!(matches!(l.activate(1), Activated::Opened));
        assert_eq!(l.folder, Some(1));
        assert!(l.showing_apps());
        // Escape steps out of the folder (onto it), then closes
        assert!(!l.back(true));
        assert_eq!((l.folder, l.selected), (None, 1));
        assert!(l.back(true));
        l.toggle_view();
        assert!(l.alphabetical());
        l.step(99);
        assert_eq!(l.selected, 5);
        // a letter picked: its apps lit, until the selection leaves them
        assert_eq!(l.pick(2), Some(2));
        assert!(l.lit(3) && !l.lit(4));
        l.select(4);
        assert!(l.picked.is_empty());
        l.set_query("fire");
        assert_eq!(l.visible().first().map(|&i| l.apps[i].name.as_str()), Some("Firefox"));
        assert!(matches!(l.activate(0), Activated::Launch(_)));
        assert_ne!(l.page(), "apps//1");
    }

    #[test]
    fn a_desktop_files_actions() {
        let t = "[Desktop Entry]\nName=Firefox\nCategories=Network;WebBrowser;\nActions=new-window;new-private-window;\n\n[Desktop Action new-window]\nName=New Window\nExec=firefox --new-window %u\n\n[Desktop Action new-private-window]\nName=New Private Window\nExec=firefox --private-window %u\n\n[Desktop Action unlisted]\nName=X\nExec=x\n";
        let a = actions(t);
        assert_eq!(a, vec![("New Window".into(), "firefox --new-window %u".into()), ("New Private Window".into(), "firefox --private-window %u".into())]);
        assert_eq!(categories(t), vec!["Network", "WebBrowser"]);
    }
}

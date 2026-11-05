//! What the dock shows, in what order (DockLogic.qml's items, Apps.qml's
//! dockApps and recentFor): the Apps button, pinned apps in Settings' order
//! then running ones in the order they opened | up to 3 recent apps | the
//! minimized windows, Downloads and Trash; each section after the first
//! starts with a divider. Plus each item's right-click menu (DockMenu.qml).

use super::settings::Settings;
use crate::services::apps::{Apps, Entry};
use crate::services::clients::{Client, Windows};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Launcher,
    App,
    Recent,
    Minimized,
    Downloads,
    Trash,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Launcher => "launcher",
            Kind::App => "app",
            Kind::Recent => "recent",
            Kind::Minimized => "minimized",
            Kind::Downloads => "downloads",
            Kind::Trash => "trash",
        }
    }
    pub fn is_app(self) -> bool {
        matches!(self, Kind::App | Kind::Recent)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub kind: Kind,
    /// the app's entry id (or its window class when it has none); the
    /// window's address for a minimized one
    pub id: String,
    pub name: String,
    pub entry: Option<Entry>,
    pub pinned: bool,
    pub divider: bool,
    pub running: bool,
    /// the app in front (Material's longer, primary dash)
    pub focused: bool,
    /// its windows' addresses, oldest first
    pub windows: Vec<String>,
}

impl Item {
    fn new(kind: Kind, id: &str, name: &str) -> Self {
        Item { kind, id: id.into(), name: name.into(), entry: None, pinned: false, divider: false, running: false, focused: false, windows: Vec::new() }
    }

    /// What identifies it across updates (the shell's `kind:id|`)
    pub fn key(&self) -> String {
        format!("{}:{}{}", self.kind.name(), self.id, if self.divider { "|" } else { "" })
    }

    /// Icon names to look up, and fallbacks (Apps.iconFor, DockItem's choices)
    pub fn icon_names(&self, trash_full: bool) -> (Vec<String>, Vec<&'static str>) {
        let own = |e: &Option<Entry>, id: &str| {
            if id == "org.quickshell" {
                return vec!["preferences-system".to_string()];
            }
            match e.as_ref().map(|e| e.icon.clone()).filter(|i| !i.is_empty()) {
                Some(i) => vec![i],
                None => vec![id.to_string()],
            }
        };
        match self.kind {
            Kind::Launcher => (["view-app-grid", "applications-all", "launchpad", "start-here-kde", "start-here", "view-grid"].map(String::from).to_vec(), vec![]),
            Kind::Downloads => (["folder-download", "folder-downloads", "folder_download", "folder-download-open"].map(String::from).to_vec(), vec!["folder", "inode-directory"]),
            Kind::Trash => {
                let n: &[&str] = if trash_full { &["user-trash-full", "trash-full", "user-trash"] } else { &["user-trash", "trash-empty", "user-trash-empty"] };
                (n.iter().map(|s| s.to_string()).collect(), vec!["edit-delete"])
            }
            Kind::Minimized => (own(&self.entry, self.entry.as_ref().map_or("", |e| e.id.as_str())), vec!["application-x-executable"]),
            Kind::App | Kind::Recent => (own(&self.entry, &self.id), vec!["application-x-executable"]),
        }
    }
}

/// The app id a window belongs to: its entry's, else its class
fn app_of(apps: &Apps, c: &Client) -> String {
    apps.entry_for(&c.class).map(|e| e.id.clone()).unwrap_or_else(|| c.class.clone())
}

/// An app's windows (Apps.windowsOf): those whose entry is it, or whose class is its id
pub fn windows_of(apps: &Apps, w: &Windows, id: &str) -> Vec<String> {
    w.clients.iter().filter(|c| c.mapped && (c.class == id || app_of(apps, c) == id)).map(|c| c.address.clone()).collect()
}

/// The dock's items, flat, with section breaks marked
pub fn build(s: &Settings, apps: &Apps, w: &Windows, recent: &[String]) -> Vec<Item> {
    let active = w.active_client().map(|c| app_of(apps, c));
    let fill = |mut it: Item| {
        it.windows = windows_of(apps, w, &it.id);
        it.running = !it.windows.is_empty();
        it.focused = it.running && active.as_deref() == Some(it.id.as_str());
        it
    };

    // Apps button, pinned, then running unpinned in the order they opened
    let mut main: Vec<Item> = Vec::new();
    if s.apps_button {
        main.push(Item::new(Kind::Launcher, "Apps", "Apps"));
    }
    let mut seen = std::collections::HashSet::new();
    for id in &s.pinned {
        let Some(e) = apps.by_id(id) else { continue };
        if !seen.insert(e.id.clone()) {
            continue;
        }
        let mut it = Item::new(Kind::App, &e.id, &e.name);
        it.entry = Some(e.clone());
        it.pinned = true;
        main.push(fill(it));
    }
    for c in w.clients.iter().filter(|c| c.mapped) {
        let e = apps.entry_for(&c.class);
        let id = e.map(|e| e.id.clone()).unwrap_or_else(|| c.class.clone());
        if id.is_empty() || !seen.insert(id.clone()) {
            continue;
        }
        let mut it = Item::new(Kind::App, &id, e.map(|e| e.name.as_str()).unwrap_or(&id));
        it.entry = e.cloned();
        main.push(fill(it));
    }

    // Recent apps: not pinned, not running, still installed; up to 3
    let mut recents: Vec<Item> = Vec::new();
    if s.recent_apps {
        let running: std::collections::HashSet<String> = w.clients.iter().map(|c| app_of(apps, c)).collect();
        for id in recent {
            if recents.len() >= 3 {
                break;
            }
            if s.pinned.contains(id) || running.contains(id) {
                continue;
            }
            let Some(e) = apps.by_id(id) else { continue };
            let mut it = Item::new(Kind::Recent, &e.id, &e.name);
            it.entry = Some(e.clone());
            recents.push(it);
        }
    }

    // Minimized windows, Downloads, Trash
    let mut end: Vec<Item> = Vec::new();
    for c in w.clients.iter().filter(|c| c.minimized()) {
        let e = apps.entry_for(&c.class);
        let name = if !c.title.is_empty() { c.title.clone() } else { e.map(|e| e.name.clone()).unwrap_or_else(|| "Window".into()) };
        let mut it = Item::new(Kind::Minimized, &format!("minimized:{}", c.address), &name);
        // (no entry: its class names the icon)
        it.entry = Some(e.cloned().unwrap_or_else(|| Entry { id: c.class.clone(), icon: c.class.clone(), ..Default::default() }));
        it.windows = vec![c.address.clone()];
        end.push(it);
    }
    if s.downloads {
        end.push(Item::new(Kind::Downloads, "Downloads", "Downloads"));
    }
    if s.trash {
        end.push(Item::new(Kind::Trash, "Trash", "Trash"));
    }

    let mut out = main;
    for mut section in [recents, end] {
        if let Some(first) = section.first_mut() {
            first.divider = true;
        }
        out.append(&mut section);
    }
    out
}

/// The window in front changed to this app: it goes first in the recent
/// list (Apps.qml: the shell's own windows don't count; at most 12 kept)
pub fn note_recent(recent: &mut Vec<String>, id: &str) -> bool {
    if id.is_empty() || id == "org.quickshell" || recent.first().map(String::as_str) == Some(id) {
        return false;
    }
    recent.retain(|x| x != id);
    recent.insert(0, id.to_string());
    recent.truncate(12);
    true
}

/// The shell's remembered recent apps ($XDG_STATE_HOME/tessera/state.json)
pub fn read_recent() -> Vec<String> {
    let base = std::env::var("XDG_STATE_HOME").ok().filter(|s| !s.is_empty()).map(std::path::PathBuf::from).unwrap_or_else(|| std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".local/state"));
    ["tessera", "irice"]
        .iter()
        .find_map(|d| std::fs::read_to_string(base.join(d).join("state.json")).ok())
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v["recentApps"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()))
        .unwrap_or_default()
}

// ── The right-click menu ────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    OpenDrawer,
    RemoveLauncher,
    Restore,
    CloseWindow,
    OpenDownloads,
    ShowDownloads,
    RemoveDownloads,
    OpenTrash,
    EmptyTrash,
    Launch,
    Pin,
    Unpin,
    Quit,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub text: String,
    pub action: Option<Action>,
    pub enabled: bool,
    pub danger: bool,
    /// asks once more, in red, before it acts (only for what can't be undone)
    pub confirm: String,
}

fn row(text: &str, a: Action) -> Row {
    Row { text: text.into(), action: Some(a), enabled: true, danger: false, confirm: String::new() }
}
fn separator() -> Row {
    Row { text: String::new(), action: None, enabled: false, danger: false, confirm: String::new() }
}

/// DockMenu.qml's rows for an item (`trash`: how many items the Trash holds)
pub fn menu(it: &Item, trash: usize) -> Vec<Row> {
    match it.kind {
        Kind::Launcher => vec![row("Open App Drawer", Action::OpenDrawer), separator(), row("Remove from Dock", Action::RemoveLauncher)],
        Kind::Minimized => vec![row("Restore", Action::Restore), separator(), row("Close Window", Action::CloseWindow)],
        Kind::Downloads => vec![row("Open Downloads", Action::OpenDownloads), row("Show Recent Files", Action::ShowDownloads), separator(), row("Remove from Dock", Action::RemoveDownloads)],
        Kind::Trash => vec![
            row("Open Trash", Action::OpenTrash),
            separator(),
            Row {
                text: "Empty Trash…".into(),
                action: Some(Action::EmptyTrash),
                enabled: trash > 0,
                danger: true,
                confirm: format!("Delete {trash} {} for good", if trash == 1 { "item" } else { "items" }),
            },
        ],
        Kind::App | Kind::Recent => {
            let mut v = vec![row(if it.running { "New Window" } else { "Open" }, Action::Launch), separator(), if it.pinned { row("Remove from Dock", Action::Unpin) } else { row("Keep in Dock", Action::Pin) }];
            if it.running {
                v.push(separator());
                v.push(row("Quit", Action::Quit));
            }
            v
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::clients::{Monitor, WsRef};
    use serde_json::json;

    fn entry(id: &str, name: &str) -> Entry {
        Entry { id: id.into(), name: name.into(), icon: id.into(), exec: id.into(), ..Default::default() }
    }
    fn apps() -> Apps {
        Apps::from_entries(vec![entry("org.kde.dolphin", "Dolphin"), entry("firefox", "Firefox"), entry("foot", "Foot"), entry("code", "Code"), entry("org.gnome.Calculator", "Calculator"), entry("gimp", "GIMP"), entry("vlc", "VLC")])
    }
    fn client(addr: &str, class: &str, ws: (i32, &str)) -> Client {
        Client { address: addr.into(), class: class.into(), title: format!("{class} window"), mapped: true, workspace: WsRef { id: ws.0, name: ws.1.into() }, ..Default::default() }
    }
    fn windows(clients: Vec<Client>, active: &str) -> Windows {
        Windows { clients, monitors: vec![Monitor { name: "DP-1".into(), ..Default::default() }], active: active.into(), ..Default::default() }
    }
    fn kinds(v: &[Item]) -> Vec<String> {
        v.iter().map(|i| i.key()).collect()
    }

    #[test]
    fn apps_button_pinned_running_recent_end() {
        let s = Settings::read(&json!({"dock": {"pinned": ["firefox", "missing-app", "foot"]}}));
        let w = windows(vec![client("0x1", "code", (1, "1")), client("0x2", "firefox", (1, "1")), client("0x3", "Unknown-Tool", (2, "2")), client("0x4", "code", (2, "2"))], "0x1");
        let recent = vec!["code".to_string(), "gimp".into(), "firefox".into(), "vlc".into(), "org.kde.dolphin".into(), "org.gnome.Calculator".into()];
        let items = build(&s, &apps(), &w, &recent);
        assert_eq!(
            kinds(&items),
            vec![
                "launcher:Apps",
                "app:firefox",
                "app:foot", // pinned, in Settings' order (a pin with no app is skipped)
                "app:code",
                "app:Unknown-Tool", // running, in the order they opened (once each)
                "recent:gimp|",
                "recent:vlc",
                "recent:org.kde.dolphin", // recent: not pinned, not running, at most 3
                "downloads:Downloads|",
                "trash:Trash",
            ]
        );
        let firefox = &items[1];
        assert!(firefox.pinned && firefox.running && !firefox.focused);
        assert!(!items[2].running);
        let code = &items[3];
        assert!(code.running && code.focused && code.windows == vec!["0x1", "0x4"]);
        assert_eq!(items[4].name, "Unknown-Tool");
    }

    #[test]
    fn settings_turn_sections_off() {
        let s = Settings::read(&json!({"dock": {"pinned": ["foot"], "appsButton": false, "recentApps": false, "downloads": false, "trash": false}}));
        let items = build(&s, &apps(), &windows(vec![], ""), &["vlc".to_string()]);
        assert_eq!(kinds(&items), vec!["app:foot"]);
    }

    #[test]
    fn minimized_windows_end_the_dock() {
        let s = Settings::read(&json!({"dock": {"pinned": [], "downloads": false}}));
        let w = windows(vec![client("0xa", "firefox", (-99, "special:minimized"))], "");
        let items = build(&s, &apps(), &w, &[]);
        assert_eq!(kinds(&items), vec!["launcher:Apps", "app:firefox", "minimized:minimized:0xa|", "trash:Trash"]);
        // (its app still counts as running: the window is only out of sight)
        assert!(items[1].running);
        assert_eq!(items[2].name, "firefox window");
    }

    #[test]
    fn recent_apps_follow_focus() {
        let mut r = vec!["a".to_string(), "b".into()];
        assert!(note_recent(&mut r, "b"));
        assert_eq!(r, vec!["b", "a"]);
        assert!(!note_recent(&mut r, "b")); // already first
        assert!(!note_recent(&mut r, "org.quickshell")); // the shell's own windows
        assert!(!note_recent(&mut r, ""));
        for i in 0..20 {
            note_recent(&mut r, &format!("app{i}"));
        }
        assert_eq!(r.len(), 12);
        assert_eq!(r[0], "app19");
    }

    #[test]
    fn menus_per_item() {
        let s = Settings::read(&json!({"dock": {"pinned": ["foot"]}}));
        let w = windows(vec![client("0x1", "foot", (1, "1")), client("0x2", "vlc", (1, "1"))], "0x1");
        let items = build(&s, &apps(), &w, &[]);
        let texts = |it: &Item, n| menu(it, n).iter().map(|r| r.text.clone()).collect::<Vec<_>>();
        assert_eq!(texts(&items[1], 0), vec!["New Window", "", "Remove from Dock", "", "Quit"]);
        assert_eq!(texts(&items[2], 0), vec!["New Window", "", "Keep in Dock", "", "Quit"]);
        let trash = items.iter().find(|i| i.kind == Kind::Trash).unwrap();
        let rows = menu(trash, 0);
        assert!(!rows[2].enabled && rows[2].danger);
        assert_eq!(menu(trash, 1)[2].confirm, "Delete 1 item for good");
        assert_eq!(menu(trash, 12)[2].confirm, "Delete 12 items for good");
        assert_eq!(texts(&items[0], 0), vec!["Open App Drawer", "", "Remove from Dock"]);
    }

    #[test]
    fn icons_by_kind() {
        let mut t = Item::new(Kind::Trash, "Trash", "Trash");
        assert_eq!(t.icon_names(true).0[0], "user-trash-full");
        assert_eq!(t.icon_names(false).0[0], "user-trash");
        t.kind = Kind::App;
        t.id = "org.quickshell".into();
        assert_eq!(t.icon_names(false).0, vec!["preferences-system"]);
        let mut a = Item::new(Kind::App, "Unknown-Tool", "Unknown-Tool");
        assert_eq!(a.icon_names(false).0, vec!["Unknown-Tool"]);
        a.entry = Some(Entry { icon: "/opt/tool/icon.png".into(), ..Default::default() });
        assert_eq!(a.icon_names(false).0, vec!["/opt/tool/icon.png"]);
    }
}

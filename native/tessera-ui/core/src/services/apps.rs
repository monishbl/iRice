//! Desktop entries (the installed apps), as the QML shell's Apps.qml and
//! Launch.qml see them: every `*.desktop` under $XDG_DATA_HOME and
//! $XDG_DATA_DIRS' `applications` (the first of an id wins), looked up by id
//! or by a window's class, and started detached in a scope of their own.
//!
//! The service scans on a thread, sends `Apps` once, then again whenever an
//! applications folder changes (an app installed or removed; checked every
//! few seconds by the folders' times, which is cheap).

use crate::Msg;
use smithay_client_toolkit::reexports::calloop::channel::Sender;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Entry {
    /// The desktop file id ("org.kde.dolphin": its path under applications/,
    /// slashes as dashes, without ".desktop")
    pub id: String,
    pub name: String,
    /// An icon name, or a file path
    pub icon: String,
    pub exec: String,
    pub terminal: bool,
    pub wm_class: String,
    /// Path=: the folder it runs in
    pub dir: String,
    pub no_display: bool,
    /// GenericName=, Comment= and Keywords= (Search ranks by them)
    pub generic_name: String,
    pub comment: String,
    pub keywords: Vec<String>,
}

/// One desktop file's [Desktop Entry] group (None: not an application, or
/// Hidden, which means deleted)
pub fn parse(id: &str, text: &str) -> Option<Entry> {
    let mut e = Entry { id: id.to_string(), ..Default::default() };
    let mut in_entry = false;
    let mut kind = String::new();
    let mut hidden = false;
    for l in text.lines() {
        let l = l.trim_end();
        if l.starts_with('[') {
            in_entry = l == "[Desktop Entry]";
            continue;
        }
        if !in_entry {
            continue;
        }
        let Some((k, v)) = l.split_once('=') else { continue };
        let v = v.trim().to_string();
        match k.trim() {
            "Type" => kind = v,
            "Name" => e.name = v,
            "Icon" => e.icon = v,
            "Exec" => e.exec = v,
            "Terminal" => e.terminal = v == "true",
            "StartupWMClass" => e.wm_class = v,
            "Path" => e.dir = v,
            "NoDisplay" => e.no_display = v == "true",
            "Hidden" => hidden = v == "true",
            "GenericName" => e.generic_name = v,
            "Comment" => e.comment = v,
            "Keywords" => e.keywords = v.split(';').map(str::trim).filter(|k| !k.is_empty()).map(String::from).collect(),
            _ => {}
        }
    }
    (kind == "Application" && !hidden && !e.name.is_empty()).then_some(e)
}

/// Exec= as a command line: quoted arguments unquoted (the spec's escapes),
/// field codes (%f %U %i %c %k…) dropped, "%%" kept as "%"
pub fn exec_argv(exec: &str) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut has = false; // (an empty quoted argument still counts)
    let mut quoted = false;
    let mut it = exec.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '"' => {
                quoted = !quoted;
                has = true;
            }
            '\\' if quoted => {
                if let Some(n) = it.next() {
                    cur.push(n);
                }
            }
            c if c.is_whitespace() && !quoted => {
                if has || !cur.is_empty() {
                    args.push(std::mem::take(&mut cur));
                }
                has = false;
            }
            '%' => match it.next() {
                Some('%') => cur.push('%'),
                // a field code: nothing to pass (the dock opens apps, not files)
                Some(_) | None => has |= !cur.is_empty(),
            },
            c => cur.push(c),
        }
    }
    if has || !cur.is_empty() {
        args.push(cur);
    }
    args.retain(|a| !a.is_empty());
    args
}

#[derive(Clone, Debug, Default)]
pub struct Apps {
    pub entries: Vec<Entry>,
    by_id: HashMap<String, usize>,
    by_lower: HashMap<String, usize>,
}

/// AppQuirks.qml's aliases: a window class with no desktop file of its own
fn alias(class: &str) -> Option<&'static str> {
    match class {
        "CiscoCollabHost" | "ciscocollabhost" | "Webex" => Some("webex"),
        _ => None,
    }
}

impl Apps {
    pub fn from_entries(entries: Vec<Entry>) -> Self {
        let mut a = Apps { entries, ..Default::default() };
        for (i, e) in a.entries.iter().enumerate() {
            a.by_id.entry(e.id.clone()).or_insert(i);
            a.by_lower.entry(e.id.to_lowercase()).or_insert(i);
        }
        a
    }

    /// Every application in these data folders (the first one of an id wins)
    pub fn scan_dirs(dirs: &[PathBuf]) -> Self {
        let mut seen = std::collections::HashSet::new();
        let mut entries = Vec::new();
        for d in dirs {
            let base = d.join("applications");
            let mut files = Vec::new();
            walk(&base, &base, &mut files, 0);
            files.sort();
            for (id, path) in files {
                if !seen.insert(id.clone()) {
                    continue;
                }
                if let Some(e) = std::fs::read_to_string(&path).ok().and_then(|t| parse(&id, &t)) {
                    entries.push(e);
                }
            }
        }
        Self::from_entries(entries)
    }

    /// An entry by its id (Quickshell's DesktopEntries.byId: exact, then ignoring case)
    pub fn by_id(&self, id: &str) -> Option<&Entry> {
        if id.is_empty() {
            return None;
        }
        self.by_id.get(id).or_else(|| self.by_lower.get(&id.to_lowercase())).map(|&i| &self.entries[i])
    }

    /// The entry a window's class belongs to (Apps.entryFor): its id, an
    /// alias, then a guess: the last part of a reverse-DNS id, StartupWMClass,
    /// or the app's name
    pub fn entry_for(&self, class: &str) -> Option<&Entry> {
        if class.is_empty() {
            return None;
        }
        if let Some(e) = self.by_id(class).or_else(|| alias(class).and_then(|a| self.by_id(a))) {
            return Some(e);
        }
        let lc = class.to_lowercase();
        self.entries
            .iter()
            .find(|e| e.wm_class.eq_ignore_ascii_case(class))
            .or_else(|| self.entries.iter().find(|e| e.id.rsplit('.').next().is_some_and(|l| l.eq_ignore_ascii_case(&lc))))
            .or_else(|| self.entries.iter().find(|e| e.name.eq_ignore_ascii_case(class)))
    }
}

fn walk(base: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>, depth: u32) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() && depth < 4 {
            walk(base, &p, out, depth + 1);
        } else if let Some(rel) = p.strip_prefix(base).ok().and_then(|r| r.to_str()).and_then(|r| r.strip_suffix(".desktop")) {
            out.push((rel.replace('/', "-"), p));
        }
    }
}

/// $XDG_DATA_HOME then $XDG_DATA_DIRS (the spec's defaults)
pub fn data_dirs() -> Vec<PathBuf> {
    let home = std::env::var("XDG_DATA_HOME").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| format!("{}/.local/share", std::env::var("HOME").unwrap_or_default()));
    let dirs = std::env::var("XDG_DATA_DIRS").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    let mut out = vec![PathBuf::from(home)];
    out.extend(dirs.split(':').filter(|d| !d.is_empty()).map(PathBuf::from));
    out.dedup();
    out
}

/// The newest change time among the applications folders (and their subfolders)
fn stamp(dirs: &[PathBuf]) -> Vec<Option<SystemTime>> {
    let mut out = Vec::new();
    for d in dirs {
        let a = d.join("applications");
        out.push(std::fs::metadata(&a).and_then(|m| m.modified()).ok());
        if let Ok(rd) = std::fs::read_dir(&a) {
            for e in rd.flatten() {
                if e.file_type().is_ok_and(|t| t.is_dir()) {
                    out.push(e.metadata().and_then(|m| m.modified()).ok());
                }
            }
        }
    }
    out
}

pub fn spawn(tx: Sender<Msg>) {
    std::thread::spawn(move || {
        let dirs = data_dirs();
        let mut last = Vec::new();
        loop {
            let now = stamp(&dirs);
            if now != last {
                last = now;
                if tx.send(Msg::Data(Box::new(Apps::scan_dirs(&dirs)))).is_err() {
                    return;
                }
            }
            std::thread::sleep(Duration::from_secs(4));
        }
    });
}

// ── Starting things ─────────────────────────────────────────────────────────

/// A file of the shell's (scripts): $TESSERA_ROOT's, else this checkout's
pub fn shell_file(rel: &str) -> Option<PathBuf> {
    let roots = [crate::env::var("ROOT").map(PathBuf::from), Some(PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../..")))];
    roots.into_iter().flatten().map(|r| r.join(rel)).find(|p| p.exists())
}

/// In the tty test session the shell runs with sandbox folders; apps get the
/// user's real ones (Launch.qml's scrub)
fn scrub(cmd: &mut std::process::Command) {
    if crate::env::var("TTY").as_deref() == Some("1") {
        for v in ["XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_STATE_HOME", "XDG_CACHE_HOME", "TESSERA_SANDBOX", "TESSERA_ROOT", "TESSERA_TTY", "IRICE_SANDBOX", "IRICE_ROOT", "IRICE_TTY", "QML_IMPORT_PATH", "QML2_IMPORT_PATH"] {
            cmd.env_remove(v);
        }
    }
}

/// Run a command detached (its own session, no stdio), in a scope of its
/// own under app.slice when the shell's app-scope.sh is there; on a thread,
/// so the UI never waits for it
pub fn run(argv: Vec<String>, id: &str, dir: Option<String>) {
    if argv.is_empty() {
        return;
    }
    let id = id.to_string();
    std::thread::spawn(move || {
        let mut full: Vec<String> = vec!["-f".into()];
        if let Some(scope) = shell_file("shell/assets/scripts/app-scope.sh") {
            full.extend([scope.to_string_lossy().into_owned(), "--id".into(), id, "--".into()]);
        }
        full.extend(argv);
        let mut cmd = std::process::Command::new("setsid");
        cmd.args(&full).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
        if let Some(d) = dir.filter(|d| Path::new(d).is_dir()) {
            cmd.current_dir(d);
        }
        scrub(&mut cmd);
        // (setsid -f forks the app away and returns at once: reaped here)
        if let Ok(mut c) = cmd.spawn() {
            let _ = c.wait();
        }
    });
}

/// AppQuirks.qml's env: apps that need a nudge
fn quirk_env(e: &Entry) -> Vec<String> {
    let low = format!("{} {}", e.id, e.exec).to_lowercase();
    if e.id.eq_ignore_ascii_case("webex") || low.contains("/opt/webex/") {
        return vec!["QT_QPA_PLATFORM=xcb".into()];
    }
    Vec::new()
}

/// The command line that starts an entry (Launch.qml's app())
pub fn command_for(e: &Entry) -> Vec<String> {
    let mut cmd = exec_argv(&e.exec);
    if e.terminal {
        if let Some(open_app) = shell_file("shell/assets/scripts/open-app.sh") {
            let user_dir = crate::theme::config_path().parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
            let mut t = vec!["env".into(), format!("TESSERA_USER_DIR={user_dir}"), "bash".into(), open_app.to_string_lossy().into_owned(), "terminal".into(), "--".into()];
            t.append(&mut cmd);
            cmd = t;
        }
    }
    let env = quirk_env(e);
    if !env.is_empty() {
        let mut c = vec!["env".to_string()];
        c.extend(env);
        c.append(&mut cmd);
        cmd = c;
    }
    cmd
}

pub fn launch(e: &Entry) {
    run(command_for(e), &e.id, (!e.dir.is_empty()).then(|| e.dir.clone()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exec_field_codes_are_dropped() {
        assert_eq!(exec_argv("firefox %u"), vec!["firefox"]);
        assert_eq!(exec_argv("/usr/bin/code --unity-launch %F"), vec!["/usr/bin/code", "--unity-launch"]);
        assert_eq!(exec_argv("foot"), vec!["foot"]);
        assert_eq!(exec_argv("app --icon %i --name %c %k"), vec!["app", "--icon", "--name"]);
        // quoted arguments, escapes inside quotes, and a literal percent
        assert_eq!(exec_argv(r#""/opt/My App/run" --title "a \"b\"" 100%%"#), vec!["/opt/My App/run", "--title", r#"a "b""#, "100%"]);
        assert_eq!(exec_argv("env FOO=1 bar %U"), vec!["env", "FOO=1", "bar"]);
        // a code glued to text leaves the text
        assert_eq!(exec_argv("app --file=%f"), vec!["app", "--file="]);
        assert!(exec_argv("").is_empty());
    }

    #[test]
    fn parses_the_desktop_entry_group_only() {
        let t = "[Desktop Entry]\nType=Application\nName=Files\nName[de]=Dateien\nIcon=system-file-manager\nExec=dolphin %u\nStartupWMClass=dolphin\n\n[Desktop Action new]\nName=New Window\nExec=dolphin --new\n";
        let e = parse("org.kde.dolphin", t).unwrap();
        assert_eq!((e.name.as_str(), e.icon.as_str(), e.exec.as_str(), e.wm_class.as_str()), ("Files", "system-file-manager", "dolphin %u", "dolphin"));
        assert!(parse("x", "[Desktop Entry]\nType=Link\nName=Site\n").is_none());
        assert!(parse("x", "[Desktop Entry]\nType=Application\nName=Gone\nHidden=true\n").is_none());
        assert!(parse("x", "[Desktop Entry]\nType=Application\nName=Quiet\nNoDisplay=true\n").unwrap().no_display);
    }

    fn app(id: &str, name: &str, wm: &str) -> Entry {
        Entry { id: id.into(), name: name.into(), wm_class: wm.into(), exec: id.into(), ..Default::default() }
    }

    #[test]
    fn a_window_finds_its_app() {
        let a = Apps::from_entries(vec![app("org.kde.dolphin", "Dolphin", ""), app("code", "Code", "Code"), app("org.gnome.Calculator", "Calculator", ""), app("webex", "Webex", ""), app("chromium", "Chromium", "")]);
        assert_eq!(a.entry_for("org.kde.dolphin").unwrap().id, "org.kde.dolphin");
        assert_eq!(a.entry_for("Code").unwrap().id, "code"); // (id, ignoring case)
        assert_eq!(a.entry_for("dolphin").unwrap().id, "org.kde.dolphin"); // reverse-DNS's last part
        assert_eq!(a.entry_for("calculator").unwrap().id, "org.gnome.Calculator");
        assert_eq!(a.entry_for("CiscoCollabHost").unwrap().id, "webex"); // AppQuirks
        assert!(a.entry_for("nothing-like-it").is_none());
        assert!(a.entry_for("").is_none());
    }

    #[test]
    fn the_first_folder_wins_and_subfolders_make_ids() {
        let d = std::env::temp_dir().join(format!("tessera-apps-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let (home, sys) = (d.join("home"), d.join("sys"));
        std::fs::create_dir_all(home.join("applications")).unwrap();
        std::fs::create_dir_all(sys.join("applications/kde")).unwrap();
        let entry = |n: &str| format!("[Desktop Entry]\nType=Application\nName={n}\nExec=x\n");
        std::fs::write(home.join("applications/foot.desktop"), entry("Mine")).unwrap();
        std::fs::write(sys.join("applications/foot.desktop"), entry("System")).unwrap();
        std::fs::write(sys.join("applications/kde/kate.desktop"), entry("Kate")).unwrap();
        let a = Apps::scan_dirs(&[home, sys]);
        assert_eq!(a.by_id("foot").unwrap().name, "Mine");
        assert_eq!(a.by_id("kde-kate").unwrap().name, "Kate");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn webex_gets_its_nudge() {
        let e = Entry { id: "webex".into(), exec: "/opt/Webex/bin/CiscoCollabHost %U".into(), ..Default::default() };
        assert_eq!(command_for(&e), vec!["env", "QT_QPA_PLATFORM=xcb", "/opt/Webex/bin/CiscoCollabHost"]);
        let e = Entry { id: "foot".into(), exec: "foot".into(), ..Default::default() };
        assert_eq!(command_for(&e), vec!["foot"]);
    }
}

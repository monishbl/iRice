//! Small helpers every surface may want: local time, desktop entries, the
//! shell's folders, files written whole, the repo's assets.

use std::path::PathBuf;

/// Milliseconds since the epoch (JavaScript's Date.now())
pub fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

/// An XDG base folder: `$var` when it's set to an absolute path (the spec:
/// an empty or relative value is ignored), else `~/<home_rel>`
/// (xdg("XDG_STATE_HOME", ".local/state"))
pub fn xdg(var: &str, home_rel: &str) -> PathBuf {
    xdg_from(std::env::var_os(var), std::env::var_os("HOME"), home_rel)
}

fn xdg_from(val: Option<std::ffi::OsString>, home: Option<std::ffi::OsString>, home_rel: &str) -> PathBuf {
    val.map(PathBuf::from).filter(|p| p.is_absolute()).unwrap_or_else(|| PathBuf::from(home.unwrap_or_default()).join(home_rel))
}

/// One of your folders (DESKTOP, DOWNLOAD, DOCUMENTS, PICTURES, VIDEOS,
/// MUSIC…): $XDG_<KIND>_DIR, else its line in $XDG_CONFIG_HOME/user-dirs.dirs
/// (what xdg-user-dirs wrote, in your language: ~/Téléchargements,
/// ~/Bilder…), else ~/<fallback>
pub fn user_dir(kind: &str, fallback: &str) -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    if let Some(v) = std::env::var(format!("XDG_{kind}_DIR")).ok().filter(|v| v.starts_with('/')) {
        return PathBuf::from(v);
    }
    let dirs = std::fs::read_to_string(xdg("XDG_CONFIG_HOME", ".config").join("user-dirs.dirs")).unwrap_or_default();
    user_dir_in(&dirs, kind, &home).unwrap_or_else(|| PathBuf::from(&home).join(fallback))
}

/// `XDG_<KIND>_DIR="$HOME/…"` of a user-dirs.dirs text (None when it's
/// missing, or set to $HOME itself: "no such folder" to xdg-user-dirs)
pub fn user_dir_in(text: &str, kind: &str, home: &str) -> Option<PathBuf> {
    let key = format!("XDG_{kind}_DIR=");
    let line = text.lines().map(str::trim).filter(|l| !l.starts_with('#')).find_map(|l| l.strip_prefix(&key))?;
    let v = line.trim().trim_matches('"');
    let path = if let Some(rest) = v.strip_prefix("$HOME") {
        format!("{home}{rest}")
    } else if v.starts_with('/') {
        v.to_string()
    } else {
        return None;
    };
    let path = path.trim_end_matches('/').to_string();
    (!path.is_empty() && path != home.trim_end_matches('/')).then(|| PathBuf::from(path))
}

/// Which package manager this system has (/etc/os-release ID and ID_LIKE):
/// "arch", "debian", "fedora", "suse", or "" when it's none of those
pub fn distro_family() -> &'static str {
    static F: std::sync::OnceLock<&'static str> = std::sync::OnceLock::new();
    F.get_or_init(|| {
        let t = std::fs::read_to_string("/etc/os-release").or_else(|_| std::fs::read_to_string("/usr/lib/os-release")).unwrap_or_default();
        family_of(&t)
    })
}

pub fn family_of(os_release: &str) -> &'static str {
    let field = |k: &str| {
        os_release.lines().find_map(|l| l.strip_prefix(k).and_then(|v| v.strip_prefix('='))).map(|v| v.trim().trim_matches('"').to_lowercase()).unwrap_or_default()
    };
    let ids = format!("{} {}", field("ID"), field("ID_LIKE"));
    let has = |n: &str| ids.split_whitespace().any(|w| w == n);
    if has("arch") || has("archlinux") {
        "arch"
    } else if has("debian") || has("ubuntu") {
        "debian"
    } else if has("fedora") || has("rhel") || has("centos") {
        "fedora"
    } else if has("suse") || has("opensuse") {
        "suse"
    } else {
        ""
    }
}

/// The command that installs a package here: `arch` names it on Arch
/// (pacman), `debian` on Debian and Ubuntu (apt), `fedora` on Fedora (dnf)
/// and openSUSE (zypper). An empty name: Arch's. Elsewhere "install <arch>"
pub fn install_hint(arch: &str, debian: &str, fedora: &str) -> String {
    install_hint_for(distro_family(), arch, debian, fedora)
}

pub fn install_hint_for(family: &str, arch: &str, debian: &str, fedora: &str) -> String {
    let pick = |n: &str| if n.is_empty() { arch.to_string() } else { n.to_string() };
    match family {
        "arch" => format!("sudo pacman -S {arch}"),
        "debian" => format!("sudo apt install {}", pick(debian)),
        "fedora" => format!("sudo dnf install {}", pick(fedora)),
        "suse" => format!("sudo zypper install {}", pick(fedora)),
        _ => format!("install {arch}"),
    }
}

/// Make a command's output the same in every language, for reading it:
/// untranslated words, "." decimals, still UTF-8 (file and Wi-Fi names).
/// Not for what the user sees run (apps, terminals)
pub fn plain_locale(c: &mut std::process::Command) -> &mut std::process::Command {
    c.env("LC_ALL", "C.UTF-8").env_remove("LANGUAGE")
}

/// A notification from the shell itself: notify-send when it's installed,
/// else straight to the notification server over D-Bus (notify-send is
/// libnotify-bin on Debian, not always there). On a thread
pub fn notify(app: &str, icon: &str, urgency: &str, title: &str, body: &str) {
    notify_live(app, icon, urgency, &Live::default(), title, body);
}

/// What makes one of the shell's notifications a live entry (services/
/// notify/live.rs): its event id (the same id again changes the one entry,
/// quietly unless it's more urgent), how urgent it is (0–3), and whether it's
/// over ("done": it stays as this last state; "remove": it goes)
#[derive(Clone, Debug, Default)]
pub struct Live {
    pub key: String,
    pub severity: Option<u8>,
    pub resolved: String,
}

impl Live {
    pub fn key(key: &str, severity: u8) -> Self {
        Self { key: key.to_string(), severity: Some(severity), resolved: String::new() }
    }
    pub fn resolved(key: &str, how: &str) -> Self {
        Self { key: key.to_string(), severity: None, resolved: how.to_string() }
    }
}

/// notify, as one live entry (`live`: its event id, severity, resolution)
pub fn notify_live(app: &str, icon: &str, urgency: &str, live: &Live, title: &str, body: &str) {
    let (app, icon, urgency, title, body, live) = (app.to_string(), icon.to_string(), urgency.to_string(), title.to_string(), body.to_string(), live.clone());
    std::thread::spawn(move || {
        if on_path("notify-send") {
            let mut c = std::process::Command::new("notify-send");
            c.args(["-a", &app]);
            if !icon.is_empty() {
                c.args(["-i", &icon]);
            }
            if !urgency.is_empty() {
                c.args(["-u", &urgency]);
            }
            if !live.key.is_empty() {
                c.args(["-h", &format!("string:x-tessera-key:{}", live.key)]);
            }
            if let Some(v) = live.severity {
                c.args(["-h", &format!("int:x-tessera-severity:{v}")]);
            }
            if !live.resolved.is_empty() {
                c.args(["-h", &format!("string:x-tessera-resolved:{}", live.resolved)]);
            }
            let _ = c.arg("--").arg(&title).arg(&body).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
            return;
        }
        let Ok(conn) = zbus::blocking::Connection::session() else { return };
        let level: u8 = match urgency.as_str() {
            "low" => 0,
            "critical" => 2,
            _ => 1,
        };
        let mut hints: std::collections::HashMap<&str, zbus::zvariant::Value> = std::collections::HashMap::new();
        hints.insert("urgency", zbus::zvariant::Value::U8(level));
        if !live.key.is_empty() {
            hints.insert("x-tessera-key", zbus::zvariant::Value::from(live.key.as_str()));
        }
        if let Some(v) = live.severity {
            hints.insert("x-tessera-severity", zbus::zvariant::Value::I32(v as i32));
        }
        if !live.resolved.is_empty() {
            hints.insert("x-tessera-resolved", zbus::zvariant::Value::from(live.resolved.as_str()));
        }
        let _ = conn.call_method(
            Some("org.freedesktop.Notifications"),
            "/org/freedesktop/Notifications",
            Some("org.freedesktop.Notifications"),
            "Notify",
            &(app.as_str(), 0u32, icon.as_str(), title.as_str(), body.as_str(), Vec::<&str>::new(), hints, -1i32),
        );
    });
}

/// $XDG_STATE_HOME/tessera, as shell/common/Paths.qml's `state` (iRice's
/// folder while only that one has settings: the shell hasn't moved them yet)
pub fn state_dir() -> PathBuf {
    let base = xdg("XDG_STATE_HOME", ".local/state");
    let old = crate::theme::config_path().parent().and_then(|p| p.file_name()).is_some_and(|n| n == "irice");
    base.join(if old { "irice" } else { "tessera" })
}

/// Write a file whole (a temporary file renamed over it), so a reader never
/// sees half of it
/// (written to the file a link points at, so a dotfiles link stays a link;
/// the file's permissions kept; on disk before the rename, so a crash or a
/// power cut leaves the old file or the new, never an empty one; a
/// temporary name of its own per write, so two threads never share one)
pub fn write_atomic(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let path = link_target(path);
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = path.with_file_name(format!(".{name}.tmp-{}-{n}", std::process::id()));
    let done = (|| {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        if let Ok(m) = std::fs::metadata(&path) {
            let _ = f.set_permissions(m.permissions());
        }
        f.sync_all()?;
        std::fs::rename(&tmp, &path)
    })();
    if done.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    done
}

/// The file a path names: the one a symbolic link points at (followed to
/// the end), else the path itself
pub fn link_target(path: &std::path::Path) -> PathBuf {
    if std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
        if let Ok(t) = std::fs::canonicalize(path) {
            return t;
        }
    }
    path.to_path_buf()
}

/// A file of the repo's (shell/assets/…): from the checkout the shell runs
/// from (crate::env::root), else the source tree (17 Nov: the source tree
/// came first, and it's the build machine's path in a ready-made binary)
pub fn asset(rel: &str) -> Option<PathBuf> {
    [crate::env::root(), PathBuf::from(crate::env::BUILT_ROOT)]
        .into_iter()
        .map(|r| r.join(rel))
        .find(|p| p.exists())
}

/// Run a program with `input` on its stdin, in the background (a thread
/// waits for it). Text that shouldn't be on a command line (any program can
/// read those) goes this way
pub fn pipe_to(cmd: &[&str], input: Vec<u8>) {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let Some((prog, args)) = cmd.split_first() else { return };
    let child = Command::new(prog).args(args).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
    let Ok(mut child) = child else {
        eprintln!("tessera-ui: can't run {prog}");
        return;
    };
    std::thread::spawn(move || {
        if let Some(mut i) = child.stdin.take() {
            let _ = i.write_all(&input);
        }
        let _ = child.wait();
    });
}

/// strftime for a time (seconds since the epoch) in the local time zone
pub fn strftime(fmt: &str, secs: i64) -> String {
    let fmt = std::ffi::CString::new(fmt).unwrap_or_default();
    let mut buf = [0u8; 128];
    unsafe {
        let t = secs as libc::time_t;
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        let n = libc::strftime(buf.as_mut_ptr() as *mut libc::c_char, buf.len(), fmt.as_ptr(), &tm);
        String::from_utf8_lossy(&buf[..n]).into_owned()
    }
}

/// The local calendar day of a time, as a day number (consecutive days
/// differ by one, whatever the clocks did in between)
pub fn local_day(secs: i64) -> i64 {
    let (y, m, d) = unsafe {
        let t = secs as libc::time_t;
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        (tm.tm_year as i64 + 1900, tm.tm_mon as i64 + 1, tm.tm_mday as i64)
    };
    // (days from civil, proleptic Gregorian)
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    era * 146097 + yoe * 365 + yoe / 4 - yoe / 100 + doy
}

/// Noon on a local date, in ms
#[cfg(test)]
pub fn local_noon_ms(y: i32, m: i32, d: i32) -> i64 {
    unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        tm.tm_year = y - 1900;
        tm.tm_mon = m - 1;
        tm.tm_mday = d;
        tm.tm_hour = 12;
        tm.tm_isdst = -1;
        libc::mktime(&mut tm) as i64 * 1000
    }
}

/// strftime in the local time zone
pub fn now(fmt: &str) -> String {
    let fmt = std::ffi::CString::new(fmt).unwrap();
    let mut buf = [0u8; 128];
    unsafe {
        let t = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        let n = libc::strftime(buf.as_mut_ptr() as *mut libc::c_char, buf.len(), fmt.as_ptr(), &tm);
        String::from_utf8_lossy(&buf[..n]).into_owned()
    }
}

/// An app's name from its desktop entry (by file name or StartupWMClass)
pub fn desktop_name(class: &str) -> Option<String> {
    desktop_field(class, "Name")
}

/// A key of an app's desktop entry's [Desktop Entry] group ("Name", "Icon")
pub fn desktop_field(class: &str, key: &str) -> Option<String> {
    let text = std::fs::read_to_string(desktop_entry(class)?).ok()?;
    entry_field(&text, key)
}

/// `key=` in a desktop entry's [Desktop Entry] group
pub fn entry_field(text: &str, key: &str) -> Option<String> {
    let mut in_entry = false;
    for l in text.lines() {
        if l.starts_with('[') {
            in_entry = l == "[Desktop Entry]";
        } else if in_entry {
            if let Some(v) = l.strip_prefix(key).and_then(|r| r.strip_prefix('=')) {
                return Some(v.to_string());
            }
        }
    }
    None
}

/// An app's desktop entry, by file name (or its last dotted part) or
/// StartupWMClass, remembered per class
pub fn desktop_entry(class: &str) -> Option<std::path::PathBuf> {
    use std::collections::HashMap;
    use std::sync::Mutex;
    static CACHE: Mutex<Option<HashMap<String, Option<std::path::PathBuf>>>> = Mutex::new(None);
    if let Some(hit) = CACHE.lock().ok().and_then(|c| c.as_ref().and_then(|m| m.get(class).cloned())) {
        return hit;
    }
    let found = find_entry(class);
    if let Ok(mut c) = CACHE.lock() {
        c.get_or_insert_with(HashMap::new).insert(class.to_string(), found.clone());
    }
    found
}

fn find_entry(class: &str) -> Option<std::path::PathBuf> {
    if class.is_empty() {
        return None;
    }
    let home = xdg("XDG_DATA_HOME", ".local/share").to_string_lossy().into_owned();
    let dirs = std::env::var("XDG_DATA_DIRS").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    let lc = class.to_lowercase();
    let mut by_wmclass = None;
    for d in std::iter::once(home.as_str()).chain(dirs.split(':')) {
        let Ok(rd) = std::fs::read_dir(format!("{d}/applications")) else { continue };
        for e in rd.flatten() {
            let f = e.file_name().to_string_lossy().to_lowercase();
            let Some(stem) = f.strip_suffix(".desktop") else { continue };
            if stem == lc || stem.rsplit('.').next() == Some(lc.as_str()) {
                return Some(e.path());
            }
            if by_wmclass.is_some() {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(e.path()) else { continue };
            if text.lines().any(|l| l.strip_prefix("StartupWMClass=").is_some_and(|c| c.eq_ignore_ascii_case(class))) {
                by_wmclass = Some(e.path());
            }
        }
    }
    by_wmclass
}

/// An executable of that name on $PATH
pub fn on_path(bin: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(bin).is_file()))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xdg_folders_ignore_empty_and_relative_values() {
        let h = Some(std::ffi::OsString::from("/home/zoë m"));
        assert_eq!(xdg_from(Some("/x/state".into()), h.clone(), ".local/state"), PathBuf::from("/x/state"));
        assert_eq!(xdg_from(Some("".into()), h.clone(), ".local/state"), PathBuf::from("/home/zoë m/.local/state"));
        assert_eq!(xdg_from(Some("rel/dir".into()), h.clone(), ".cache"), PathBuf::from("/home/zoë m/.cache"));
        assert_eq!(xdg_from(None, h, ".config"), PathBuf::from("/home/zoë m/.config"));
    }

    #[test]
    fn user_dirs_in_your_language() {
        let t = "# written by xdg-user-dirs-update\nXDG_DESKTOP_DIR=\"$HOME/Bureau\"\nXDG_DOWNLOAD_DIR=\"$HOME/Téléchargements\"\nXDG_MUSIC_DIR=\"$HOME/\"\nXDG_PICTURES_DIR=\"/data/Fotos/\"\n";
        assert_eq!(user_dir_in(t, "DOWNLOAD", "/home/zoë m"), Some(PathBuf::from("/home/zoë m/Téléchargements")));
        assert_eq!(user_dir_in(t, "DESKTOP", "/home/a"), Some(PathBuf::from("/home/a/Bureau")));
        assert_eq!(user_dir_in(t, "PICTURES", "/home/a"), Some(PathBuf::from("/data/Fotos")));
        // ($HOME itself: no such folder; missing: the fallback)
        assert_eq!(user_dir_in(t, "MUSIC", "/home/a"), None);
        assert_eq!(user_dir_in(t, "VIDEOS", "/home/a"), None);
        assert_eq!(user_dir_in("", "DOWNLOAD", "/home/a"), None);
    }

    #[test]
    fn install_hints_per_distro() {
        assert_eq!(family_of("NAME=\"CachyOS Linux\"\nID=cachyos\nID_LIKE=arch\n"), "arch");
        assert_eq!(family_of("ID=endeavouros\nID_LIKE=arch\n"), "arch");
        assert_eq!(family_of("ID=manjaro\nID_LIKE=arch\n"), "arch");
        assert_eq!(family_of("ID=arch\n"), "arch");
        assert_eq!(family_of("ID=ubuntu\nID_LIKE=debian\n"), "debian");
        assert_eq!(family_of("ID=linuxmint\nID_LIKE=\"ubuntu debian\"\n"), "debian");
        assert_eq!(family_of("ID=fedora\n"), "fedora");
        assert_eq!(family_of("ID=\"opensuse-tumbleweed\"\nID_LIKE=\"opensuse suse\"\n"), "suse");
        assert_eq!(family_of("ID=void\n"), "");
        assert_eq!(install_hint_for("arch", "bluez bluez-utils", "bluez", ""), "sudo pacman -S bluez bluez-utils");
        assert_eq!(install_hint_for("debian", "bluez bluez-utils", "bluez", ""), "sudo apt install bluez");
        assert_eq!(install_hint_for("fedora", "curl", "", ""), "sudo dnf install curl");
        assert_eq!(install_hint_for("", "ddcutil", "", ""), "install ddcutil");
    }

    #[test]
    fn plain_locale_for_parsed_output() {
        let mut c = std::process::Command::new("true");
        plain_locale(&mut c);
        let envs: Vec<_> = c.get_envs().map(|(k, v)| (k.to_string_lossy().into_owned(), v.map(|v| v.to_string_lossy().into_owned()))).collect();
        assert!(envs.contains(&("LC_ALL".into(), Some("C.UTF-8".into()))));
        assert!(envs.contains(&("LANGUAGE".into(), None)));
    }

    #[test]
    fn fields_of_the_main_group_only() {
        let t = "[Desktop Entry]\nName=Files\nIcon=org.gnome.Nautilus\n[Desktop Action new]\nName=New Window\n";
        assert_eq!(entry_field(t, "Name").as_deref(), Some("Files"));
        assert_eq!(entry_field(t, "Icon").as_deref(), Some("org.gnome.Nautilus"));
        assert_eq!(entry_field(t, "Exec"), None);
        // (NameX= isn't Name=)
        assert_eq!(entry_field("[Desktop Entry]\nNameX=a\n", "Name"), None);
    }
}

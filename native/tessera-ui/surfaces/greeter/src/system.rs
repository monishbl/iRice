//! What the login screen reads from the system, as GreeterAuth.qml does:
//! the people (/etc/passwd: a login shell and a normal uid), the sessions
//! (/usr/share/wayland-sessions: Name and Exec, Tessera's first), this
//! computer's name (/etc/hostname), the pictures AccountsService keeps, and
//! the last person and session chosen ($XDG_CACHE_HOME/tessera-greeter.json).
//! A session's Exec line becomes its command as session-exec.js does (never
//! through a shell).
//!
//! Every path can be pointed elsewhere for the sandbox and the tests
//! (TESSERA_GREETER_PASSWD, _SESSIONS, _HOSTNAME, _ICONS); `--demo` brings
//! its own made-up people and sessions.

use super::model::{Session, User};
use std::path::{Path, PathBuf};

fn env_or(name: &str, default: &str) -> PathBuf {
    PathBuf::from(crate::env::var(name).filter(|v| !v.is_empty()).unwrap_or_else(|| default.to_string()))
}

pub fn passwd_path() -> PathBuf {
    env_or("GREETER_PASSWD", "/etc/passwd")
}
pub fn sessions_dir() -> PathBuf {
    env_or("GREETER_SESSIONS", "/usr/share/wayland-sessions")
}
pub fn hostname_path() -> PathBuf {
    env_or("GREETER_HOSTNAME", "/etc/hostname")
}
pub fn icons_dir() -> PathBuf {
    env_or("GREETER_ICONS", "/var/lib/AccountsService/icons")
}

/// People with a login shell and a normal uid (1000 to 59999), their full
/// name the GECOS field's first part (else the username)
pub fn users_from_passwd(text: &str) -> Vec<User> {
    text.split('\n')
        .map(|l| l.split(':').collect::<Vec<_>>())
        .filter(|f| f.len() >= 7)
        .filter(|f| f[2].trim().parse::<f64>().is_ok_and(|uid| (1000.0..60000.0).contains(&uid)))
        .filter(|f| !(f[6].ends_with("nologin") || f[6].ends_with("false")))
        .map(|f| {
            let full = f[4].split(',').next().unwrap_or("");
            User { name: f[0].to_string(), full: if full.is_empty() { f[0].to_string() } else { full.to_string() } }
        })
        .collect()
}

pub fn users() -> Vec<User> {
    std::fs::read_to_string(passwd_path()).map(|t| users_from_passwd(&t)).unwrap_or_default()
}

/// The first `key=` line's value (grep -m1 '^key=')
fn field(text: &str, key: &str) -> String {
    text.lines().find_map(|l| l.strip_prefix(key).and_then(|r| r.strip_prefix('='))).unwrap_or("").to_string()
}

/// Tessera's session first, then Hyprland's (what Tessera runs on), then
/// the rest by name; a list never empty (Hyprland, when there's none)
pub fn sort_sessions(mut list: Vec<Session>) -> Vec<Session> {
    let rank = |s: &Session| {
        let n = s.name.to_lowercase();
        if n.contains("tessera") {
            0
        } else if n.contains("hyprland") {
            1
        } else {
            2
        }
    };
    list.sort_by(|a, b| rank(a).cmp(&rank(b)).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    if list.is_empty() {
        list.push(Session { name: "Hyprland".into(), exec: "Hyprland".into() });
    }
    list
}

pub fn sessions_in(dir: &Path) -> Vec<Session> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|d| d.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "desktop") && p.is_file()).collect())
        .unwrap_or_default();
    files.sort();
    let list = files
        .iter()
        .filter_map(|p| std::fs::read_to_string(p).ok())
        .map(|t| Session { name: field(&t, "Name"), exec: field(&t, "Exec") })
        .collect();
    sort_sessions(list)
}

pub fn sessions() -> Vec<Session> {
    sessions_in(&sessions_dir())
}

pub fn hostname() -> String {
    std::fs::read_to_string(hostname_path()).map(|t| t.trim().to_string()).unwrap_or_default()
}

/// A person's picture the greeter may read (AccountsService's: your ~/.face
/// it can't)
pub fn avatar(name: &str) -> Option<PathBuf> {
    if name.is_empty() || name.contains('/') || name.starts_with('.') {
        return None;
    }
    let p = icons_dir().join(name);
    p.is_file().then_some(p)
}

pub fn cache_file() -> PathBuf {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".cache"));
    base.join("tessera-greeter.json")
}

/// The last person and session, as GreeterAuth.qml writes them
pub fn remembered() -> Option<serde_json::Value> {
    std::fs::read_to_string(cache_file()).ok().and_then(|t| serde_json::from_str(&t).ok())
}

pub fn remember(user: &str, session: &str) {
    let v = serde_json::json!({"user": user, "session": session});
    let _ = crate::util::write_atomic(&cache_file(), v.to_string().as_bytes());
}

/// A desktop entry's Exec as a command (session-exec.js): words split on
/// white space outside quotes, `\` escapes the next character, `"` quotes;
/// field codes dropped (%f %u %i…, but %c is the name), %% a percent sign;
/// any other field is refused, and nothing is ever given to a shell
pub fn parse_exec(exec: &str, name: &str) -> Result<Vec<String>, String> {
    let mut args = Vec::new();
    let (mut word, mut quoted, mut escaped, mut started) = (String::new(), false, false, false);
    for c in exec.chars() {
        if escaped {
            word.push(c);
            escaped = false;
            started = true;
        } else if c == '\\' {
            escaped = true;
            started = true;
        } else if c == '"' {
            quoted = !quoted;
            started = true;
        } else if c.is_whitespace() && !quoted {
            if started {
                args.push(std::mem::take(&mut word));
            }
            word.clear();
            started = false;
        } else {
            word.push(c);
            started = true;
        }
    }
    if quoted || escaped {
        return Err("Invalid session command quoting".into());
    }
    if started {
        args.push(word);
    }
    let mut out = Vec::new();
    for arg in args {
        let b = arg.as_bytes();
        if b.len() == 2 && b[0] == b'%' && b"fFuUdDnNickvm".contains(&b[1]) && arg != "%c" {
            continue;
        }
        let mut v = String::new();
        let mut it = arg.chars().peekable();
        while let Some(c) = it.next() {
            if c != '%' {
                v.push(c);
                continue;
            }
            match it.peek().copied() {
                Some('%') => {
                    it.next();
                    v.push('%');
                }
                Some('c') => {
                    it.next();
                    v.push_str(name);
                }
                // (a lone % at the end, or before a line break, stays: the
                // JavaScript regex's `.` needs a character that isn't one)
                None | Some('\n' | '\r' | '\u{2028}' | '\u{2029}') => v.push('%'),
                Some(other) => return Err(format!("Unsupported session command field: %{other}")),
            }
        }
        out.push(v);
    }
    if out.first().map_or(true, |a| a.is_empty()) {
        return Err("Empty session command".into());
    }
    Ok(out)
}

/// `--demo`'s made-up people and sessions (never anyone real)
pub fn demo_users() -> Vec<User> {
    [("ada", "Ada Lovelace"), ("grace", "Grace Hopper"), ("katherine", "Katherine Johnson")]
        .iter()
        .map(|(n, f)| User { name: n.to_string(), full: f.to_string() })
        .collect()
}

pub fn demo_sessions() -> Vec<Session> {
    sort_sessions(vec![
        Session { name: "Hyprland".into(), exec: "Hyprland".into() },
        Session { name: "Tessera".into(), exec: "tessera-session".into() },
        Session { name: "Sway".into(), exec: "sway".into() },
    ])
}

/// `--demo`'s picture: one of the bundled wallpapers, for Ada
pub fn demo_avatar(name: &str) -> Option<PathBuf> {
    (name == "ada").then(|| crate::util::asset("shell/assets/wallpapers/tahoe-dawn.jpg")).flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn people_with_a_login_shell_and_a_normal_uid() {
        let passwd = "root:x:0:0::/root:/usr/bin/bash\n\
            ada:x:1000:1000:Ada Lovelace,,,:/home/ada:/usr/bin/zsh\n\
            grace:x:1001:1001::/home/grace:/bin/bash\n\
            svc:x:1002:1002:A service:/var/svc:/usr/bin/nologin\n\
            off:x:1003:1003::/home/off:/bin/false\n\
            nobody:x:65534:65534:Nobody:/:/usr/bin/nologin\n\
            big:x:60000:60000::/home/big:/bin/bash\n\
            short:x:1004\n\
            odd:x:abc:1:Odd:/:/bin/sh\n";
        let u = users_from_passwd(passwd);
        assert_eq!(u, vec![User { name: "ada".into(), full: "Ada Lovelace".into() }, User { name: "grace".into(), full: "grace".into() }]);
        assert!(users_from_passwd("").is_empty());
    }

    #[test]
    fn sessions_tessera_first_then_hyprland_then_by_name() {
        let s = |n: &str| Session { name: n.into(), exec: n.to_lowercase() };
        let sorted = sort_sessions(vec![s("Sway"), s("Hyprland (uwsm)"), s("niri"), s("Tessera"), s("Hyprland")]);
        let names: Vec<&str> = sorted.iter().map(|x| x.name.as_str()).collect();
        assert_eq!(names, ["Tessera", "Hyprland", "Hyprland (uwsm)", "niri", "Sway"]);
        assert_eq!(sort_sessions(vec![]), vec![Session { name: "Hyprland".into(), exec: "Hyprland".into() }]);
    }

    #[test]
    fn sessions_from_desktop_files() {
        let dir = super::super::greetd::tests::temp_dir("sessions");
        std::fs::write(dir.0.join("hyprland.desktop"), "[Desktop Entry]\nName=Hyprland\nComment=x\nExec=Hyprland\nName=Second\n").unwrap();
        std::fs::write(dir.0.join("tessera.desktop"), "[Desktop Entry]\nName[de]=Tessera DE\nName=Tessera\nExec=\"/usr/bin/tessera session\" --x\n").unwrap();
        std::fs::write(dir.0.join("notes.txt"), "Name=Not a session\n").unwrap();
        let s = sessions_in(&dir.0);
        assert_eq!(s.len(), 2);
        assert_eq!((s[0].name.as_str(), s[0].exec.as_str()), ("Tessera", "\"/usr/bin/tessera session\" --x"));
        assert_eq!((s[1].name.as_str(), s[1].exec.as_str()), ("Hyprland", "Hyprland"));
        // none: Hyprland
        assert_eq!(sessions_in(&dir.0.join("missing"))[0].name, "Hyprland");
    }

    #[test]
    fn session_commands_as_session_exec_js() {
        // (tests/audit_logic.test.cjs's case for session-exec.js, the same here)
        assert_eq!(
            parse_exec("\"/some path/session\" \"two words\" literal$HOME %% %c %f", "Session").unwrap(),
            vec!["/some path/session", "two words", "literal$HOME", "%", "Session"]
        );
        assert!(parse_exec("\"unfinished", "Session").is_err());
        assert!(parse_exec("trailing\\", "Session").is_err());
        assert_eq!(parse_exec("  Hyprland  ", "H").unwrap(), vec!["Hyprland"]);
        assert_eq!(parse_exec("a\\ b \"\"", "x").unwrap(), vec!["a b", ""]);
        assert_eq!(parse_exec("run --name=%c-%% %U %i", "Tessera").unwrap(), vec!["run", "--name=Tessera-%"]);
        assert!(parse_exec("run %x", "S").unwrap_err().contains("%x"));
        assert!(parse_exec("run --file=%f", "S").is_err());
        assert!(parse_exec("", "S").is_err());
        assert!(parse_exec("%f", "S").is_err());
        assert!(parse_exec("\"\" x", "S").is_err());
        assert_eq!(parse_exec("hundred%", "S").unwrap(), vec!["hundred%"]);
    }

    #[test]
    fn pictures_only_by_a_plain_name() {
        assert_eq!(avatar(""), None);
        assert_eq!(avatar("../etc/passwd"), None);
        assert_eq!(avatar(".hidden"), None);
    }

    #[test]
    fn demo_people_are_made_up() {
        let u = demo_users();
        assert!(u.len() >= 2 && u.iter().all(|x| !x.full.is_empty()));
        assert_eq!(demo_sessions()[0].name, "Tessera");
    }
}

//! Crash reports: one text file per panic, caught in a part (contain.rs) or
//! fatal, in $XDG_STATE_HOME/tessera/crashes/<time>-<part>.txt; the newest
//! 10 are kept. A report says the version and commit, the part, the panic's
//! message and backtrace, the last 200 lines of tessera-ui.log, Hyprland's
//! version and the GPU, and nothing else: the home folder is written as ~,
//! the user, computer, e-mail, IP and MAC addresses are taken out.
//! Settings → About → Report a Bug lists them, saves them as one file to
//! Downloads and deletes them (with Undo).

use std::path::{Path, PathBuf};

/// How many reports are kept
pub const KEEP: usize = 10;

/// Where the reports are
pub fn dir() -> PathBuf {
    crate::util::xdg("XDG_STATE_HOME", ".local/state").join("tessera/crashes")
}

/// What one panic said
#[derive(Clone, Debug, Default)]
pub struct Panic {
    /// the part it happened in ("dock", a host service's name, "shell" for
    /// the loop itself, "thread-<name>" for another thread)
    pub part: String,
    pub message: String,
    /// file:line:column
    pub location: String,
    pub backtrace: String,
    /// the shell ended with it
    pub fatal: bool,
}

/// A part's name as a file name's end (letters, digits, - and _)
fn file_part(part: &str) -> String {
    let p: String = part.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' }).collect();
    if p.is_empty() { "unknown".into() } else { p }
}

/// Run a command for at most `secs`: its first line of output ("" when it
/// isn't there, fails or hangs)
fn first_line(cmd: &[&str], secs: u64) -> String {
    use std::process::{Command, Stdio};
    use std::io::Read;
    let Ok(mut child) = Command::new(cmd[0]).args(&cmd[1..]).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn() else { return String::new() };
    let t = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if t.elapsed().as_secs() < secs => std::thread::sleep(std::time::Duration::from_millis(10)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return String::new();
            }
        }
    }
    let mut out = String::new();
    if let Some(mut o) = child.stdout.take() {
        let _ = o.read_to_string(&mut out);
    }
    out.lines().next().unwrap_or("").trim().to_string()
}

/// This build's commit: TESSERA_COMMIT at build time, else the checkout's
/// (the shell's folder, or where it was built)
fn commit() -> String {
    if let Some(c) = option_env!("TESSERA_COMMIT") {
        return c.to_string();
    }
    let dirs = [crate::env::var("ROOT"), std::env::var("TESSERA_QML_FALLBACK").ok(), Some(env!("CARGO_MANIFEST_DIR").to_string())];
    for d in dirs.into_iter().flatten().filter(|d| !d.is_empty() && Path::new(d).is_dir()) {
        let c = first_line(&["git", "-C", d.as_str(), "rev-parse", "--short=10", "HEAD"], 2);
        if !c.is_empty() {
            return c;
        }
    }
    "unknown".into()
}

/// Hyprland's version (its first line), once
fn hyprland() -> String {
    static V: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    V.get_or_init(|| {
        let v = first_line(&["hyprctl", "version"], 2);
        if v.is_empty() { "unknown".into() } else { v }
    })
    .clone()
}

/// The last `n` lines of tessera-ui.log
fn log_tail(n: usize) -> String {
    let path = crate::util::xdg("XDG_STATE_HOME", ".local/state").join("tessera/tessera-ui.log");
    let Ok(bytes) = std::fs::read(&path) else { return "(no log)".into() };
    // (the end of a big log only: a megabyte holds far more than 200 lines)
    let from = bytes.len().saturating_sub(1 << 20);
    let text = String::from_utf8_lossy(&bytes[from..]);
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(n)..].join("\n")
}

/// A backtrace from the frame that panicked: the hook's and std's own
/// frames above it (the panic machinery) left out
pub fn trim_backtrace(bt: &str) -> String {
    let lines: Vec<&str> = bt.lines().collect();
    let is_frame = |l: &str| l.trim_start().split_once(':').is_some_and(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
    // (only the leading run: catch_unwind further down goes through std::panicking too)
    let machinery = |l: &str| ["contain::install", "::panicking::", "rust_begin_unwind", "alloc::boxed::Box<dyn", "std::backtrace", "__rustc::"].iter().any(|m| l.contains(m));
    let frames: Vec<usize> = (0..lines.len()).filter(|&i| is_frame(lines[i])).collect();
    match frames.iter().find(|&&i| !machinery(lines[i])) {
        Some(&first) if first > 0 => lines[first..].join("\n"),
        _ => bt.to_string(),
    }
}

/// The report's text, scrubbed
pub fn compose(p: &Panic, now_secs: i64) -> String {
    let mut s = String::new();
    s.push_str(&format!("Tessera crash report\n\nVersion: tessera-ui {} ({})\n", env!("CARGO_PKG_VERSION"), commit()));
    s.push_str(&format!("Time: {}\n", crate::util::strftime("%Y-%m-%d %H:%M:%S %z", now_secs)));
    s.push_str(&format!("Part: {}\n", p.part));
    s.push_str(&format!("Outcome: {}\n", if p.fatal { "the shell ended (launch-shell.sh starts it again)" } else { "the part was stopped and started again" }));
    s.push_str(&format!("Hyprland: {}\n", hyprland()));
    let gpu = crate::egl::renderer();
    s.push_str(&format!("GPU: {}\n", if gpu.is_empty() { "unknown" } else { gpu }));
    s.push_str(&format!("\nPanic: {}\nAt: {}\n\nBacktrace:\n{}\n", p.message, p.location, p.backtrace.trim_end()));
    s.push_str(&format!("\nLast 200 lines of tessera-ui.log:\n{}\n", log_tail(200)));
    scrub(&s)
}

/// Write a report now; the oldest beyond KEEP go. The file, if written
pub fn write(p: &Panic) -> Option<PathBuf> {
    let dir = dir();
    std::fs::create_dir_all(&dir).ok()?;
    let now = crate::util::now_ms() / 1000;
    let stem = format!("{}-{}", crate::util::strftime("%Y%m%d-%H%M%S", now), file_part(&p.part));
    let mut path = dir.join(format!("{stem}.txt"));
    let mut i = 2;
    while path.exists() {
        path = dir.join(format!("{stem}-{i}.txt"));
        i += 1;
    }
    std::fs::write(&path, compose(p, now)).ok()?;
    prune(&dir, KEEP);
    Some(path)
}

/// Write it on a thread of its own (Hyprland's version and the commit are
/// read from commands: not on the UI thread)
pub fn write_later(p: Panic) -> std::thread::JoinHandle<Option<PathBuf>> {
    std::thread::spawn(move || write(&p))
}

/// The reports, newest first
pub fn list() -> Vec<PathBuf> {
    list_in(&dir())
}

fn list_in(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|r| r.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "txt") && p.is_file()).collect())
        .unwrap_or_default();
    // (the names begin with the time: they sort as the reports came)
    v.sort();
    v.reverse();
    v
}

fn prune(dir: &Path, keep: usize) {
    for old in list_in(dir).into_iter().skip(keep) {
        let _ = std::fs::remove_file(old);
    }
}

/// A report's part and time, from its name ("20261116-140233-dock.txt")
pub fn describe(path: &Path) -> (String, String) {
    let name = path.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut it = name.splitn(3, '-');
    let (d, t, part) = (it.next().unwrap_or(""), it.next().unwrap_or(""), it.next().unwrap_or(""));
    // (a second report in the same second: "-2" after the part)
    let part = match part.rsplit_once('-') {
        Some((p, n)) if n.chars().all(|c| c.is_ascii_digit()) && !p.is_empty() => p.to_string(),
        _ => part.to_string(),
    };
    let when = if d.len() == 8 && t.len() == 6 { format!("{}-{}-{} {}:{}", &d[..4], &d[4..6], &d[6..], &t[..2], &t[2..4]) } else { String::new() };
    (part, when)
}

/// Every report as one text (newest first)
pub fn bundle(paths: &[PathBuf]) -> String {
    let mut s = format!("Tessera crash reports ({})\n", paths.len());
    for p in paths {
        let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        s.push_str(&format!("\n===== {name} =====\n\n"));
        s.push_str(&std::fs::read_to_string(p).unwrap_or_default());
    }
    s
}

/// Save every report as one file in Downloads: the file
pub fn save_bundle() -> std::io::Result<PathBuf> {
    let paths = list();
    if paths.is_empty() {
        return Err(std::io::Error::new(std::io::ErrorKind::NotFound, "no crash reports"));
    }
    let dl = crate::util::user_dir("DOWNLOAD", "Downloads");
    std::fs::create_dir_all(&dl)?;
    let stem = format!("tessera-crash-reports-{}", crate::util::strftime("%Y%m%d-%H%M%S", crate::util::now_ms() / 1000));
    let mut path = dl.join(format!("{stem}.txt"));
    let mut i = 2;
    while path.exists() {
        path = dl.join(format!("{stem}-{i}.txt"));
        i += 1;
    }
    std::fs::write(&path, bundle(&paths))?;
    Ok(path)
}

/// Is a report (by its file name, "20261116-140233-dock.txt") still kept?
pub fn exists(name: &str) -> bool {
    !name.is_empty() && !name.contains('/') && dir().join(name).is_file()
}

/// Save one report (by its file name) to Downloads: the file
pub fn save_one(name: &str) -> std::io::Result<PathBuf> {
    if !exists(name) {
        return Err(std::io::Error::new(std::io::ErrorKind::NotFound, "the report was deleted"));
    }
    let dl = crate::util::user_dir("DOWNLOAD", "Downloads");
    std::fs::create_dir_all(&dl)?;
    let stem = format!("tessera-crash-{}", name.trim_end_matches(".txt"));
    let mut path = dl.join(format!("{stem}.txt"));
    let mut i = 2;
    while path.exists() {
        path = dl.join(format!("{stem}-{i}.txt"));
        i += 1;
    }
    std::fs::copy(dir().join(name), &path)?;
    Ok(path)
}

/// Where deleted reports wait for an Undo (emptied at the next delete)
fn trash() -> PathBuf {
    dir().join(".deleted")
}

/// Delete every report (kept aside until the next delete, for Undo): how many
pub fn delete_all() -> usize {
    let t = trash();
    let _ = std::fs::remove_dir_all(&t);
    let paths = list();
    if paths.is_empty() || std::fs::create_dir_all(&t).is_err() {
        return 0;
    }
    paths.iter().filter(|p| p.file_name().is_some_and(|n| std::fs::rename(p, t.join(n)).is_ok())).count()
}

/// Undo the last delete
pub fn restore_deleted() {
    let t = trash();
    if let Ok(r) = std::fs::read_dir(&t) {
        for e in r.flatten() {
            let _ = std::fs::rename(e.path(), dir().join(e.file_name()));
        }
    }
    let _ = std::fs::remove_dir_all(&t);
    prune(&dir(), KEEP);
}

/// Take out what could say who you are: the home folder (~), the user's and
/// the computer's names, e-mail, IPv4 and MAC addresses
pub fn scrub(text: &str) -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    let user = std::env::var("USER").unwrap_or_default();
    let host = std::fs::read_to_string("/etc/hostname").or_else(|_| std::fs::read_to_string("/proc/sys/kernel/hostname")).unwrap_or_default();
    scrub_with(text, &home, &user, host.trim())
}

pub fn scrub_with(text: &str, home: &str, user: &str, host: &str) -> String {
    let mut s = text.to_string();
    if home.len() > 1 {
        s = s.replace(home.trim_end_matches('/'), "~");
    }
    // (names too short to be told from ordinary words are left)
    if user.len() >= 4 {
        s = s.replace(user, "<user>");
    }
    if host.len() >= 4 && host != "localhost" {
        s = s.replace(host, "<host>");
    }
    s.lines().map(scrub_line).collect::<Vec<_>>().join("\n") + if s.ends_with('\n') { "\n" } else { "" }
}

/// One line's words: an e-mail, IPv4 or MAC address becomes a placeholder
fn scrub_line(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut word = String::new();
    let flush = |w: &mut String, out: &mut String| {
        out.push_str(scrub_word(w));
        w.clear();
    };
    for c in line.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '.' | ':' | '@' | '_' | '-' | '+') {
            word.push(c);
        } else {
            flush(&mut word, &mut out);
            out.push(c);
        }
    }
    flush(&mut word, &mut out);
    out
}

fn scrub_word(w: &str) -> &str {
    let t = w.trim_end_matches(['.', ':']);
    if is_mac(t) {
        return "<mac>";
    }
    // (an address with its port too)
    if is_ipv4(t.split(':').next().unwrap_or("")) {
        return "<ip>";
    }
    if let Some((a, b)) = t.split_once('@') {
        if a.chars().any(|c| c.is_ascii_alphabetic()) && b.contains('.') && !b.starts_with('.') && b.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-') {
            return "<email>";
        }
    }
    w
}

fn is_mac(w: &str) -> bool {
    let p: Vec<&str> = w.split(':').collect();
    p.len() == 6 && p.iter().all(|x| x.len() == 2 && x.chars().all(|c| c.is_ascii_hexdigit()))
}

fn is_ipv4(w: &str) -> bool {
    let p: Vec<&str> = w.split('.').collect();
    // (0.0.0.0 and 127.x say nothing about anyone)
    p.len() == 4 && p.iter().all(|x| !x.is_empty() && x.len() <= 3 && x.parse::<u8>().is_ok()) && p[0] != "127" && w != "0.0.0.0"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_personal_stays() {
        let t = "at /home/alice/x.rs:3 alice@laptop-7 mail bob@example.com ip 192.168.1.20:8080 mac aa:bb:cc:dd:ee:0f v1.2.3 local 127.0.0.1 laptop-7\n";
        let s = scrub_with(t, "/home/alice", "alice", "laptop-7");
        assert_eq!(s, "at ~/x.rs:3 <user>@<host> mail <email> ip <ip> mac <mac> v1.2.3 local 127.0.0.1 <host>\n");
        // (short names aren't replaced inside other words)
        assert_eq!(scrub_with("bo bob", "/home/bo", "bo", "pc"), "bo bob");
        // (versions and times aren't addresses)
        assert_eq!(scrub_with("0.57.1 12:30:45 2026-11-16", "", "", ""), "0.57.1 12:30:45 2026-11-16");
    }

    #[test]
    fn backtraces_start_where_it_panicked() {
        let bt = "   0: tessera_core::contain::install::{closure#1}\n   1: std::panicking::panic_with_hook\n             at x.rs:1\n   2: core::panicking::panic_fmt\n   3: tessera_dock::Dock::message\n             at dock.rs:9\n   4: std::panicking::catch_unwind\n   5: main";
        assert_eq!(trim_backtrace(bt), "   3: tessera_dock::Dock::message\n             at dock.rs:9\n   4: std::panicking::catch_unwind\n   5: main");
        assert_eq!(trim_backtrace("disabled backtrace"), "disabled backtrace");
    }

    #[test]
    fn names_say_part_and_time() {
        assert_eq!(describe(Path::new("/x/20261116-140233-dock.txt")), ("dock".into(), "2026-11-16 14:02".into()));
        assert_eq!(describe(Path::new("/x/20261116-140233-dock-2.txt")).0, "dock");
        assert_eq!(describe(Path::new("/x/20261116-140233-thread-glass.txt")).0, "thread-glass");
        assert_eq!(file_part("host/Net work"), "host-Net-work");
    }

    #[test]
    fn keeps_the_newest_ten() {
        let d = std::env::temp_dir().join(format!("tessera-crash-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        for i in 0..13 {
            std::fs::write(d.join(format!("20261116-1400{i:02}-dock.txt")), "x").unwrap();
        }
        prune(&d, KEEP);
        let left = list_in(&d);
        assert_eq!(left.len(), 10);
        assert!(left[0].ends_with("20261116-140012-dock.txt"));
        assert!(left[9].ends_with("20261116-140003-dock.txt"));
        let _ = std::fs::remove_dir_all(&d);
    }
}

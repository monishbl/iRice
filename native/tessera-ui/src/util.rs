//! Small helpers every surface may want: local time, desktop entries, the
//! shell's folders, files written whole, the repo's assets.

use std::path::PathBuf;

/// Milliseconds since the epoch (JavaScript's Date.now())
pub fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

/// $XDG_STATE_HOME/tessera, as shell/common/Paths.qml's `state` (iRice's
/// folder while only that one has settings: the shell hasn't moved them yet)
pub fn state_dir() -> PathBuf {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/state"));
    let old = crate::theme::config_path().parent().and_then(|p| p.file_name()).is_some_and(|n| n == "irice");
    base.join(if old { "irice" } else { "tessera" })
}

/// Write a file whole (a temporary file renamed over it), so a reader never
/// sees half of it
pub fn write_atomic(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

/// A file of the repo's (shell/assets/…): from the source tree, else from
/// $TESSERA_ROOT (where the shell runs from)
pub fn asset(rel: &str) -> Option<PathBuf> {
    [PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../..")), PathBuf::from(crate::env::var("ROOT").unwrap_or_default())]
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
    let home = std::env::var("XDG_DATA_HOME").unwrap_or_else(|_| format!("{}/.local/share", std::env::var("HOME").unwrap_or_default()));
    let dirs = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    let lc = class.to_lowercase();
    let name_of = |text: &str| {
        let mut in_entry = false;
        for l in text.lines() {
            if l.starts_with('[') {
                in_entry = l == "[Desktop Entry]";
            } else if in_entry {
                if let Some(n) = l.strip_prefix("Name=") {
                    return Some(n.to_string());
                }
            }
        }
        None
    };
    let mut by_wmclass = None;
    for d in std::iter::once(home.as_str()).chain(dirs.split(':')) {
        let Ok(rd) = std::fs::read_dir(format!("{d}/applications")) else { continue };
        for e in rd.flatten() {
            let f = e.file_name().to_string_lossy().to_lowercase();
            let Some(stem) = f.strip_suffix(".desktop") else { continue };
            let wanted = stem == lc || stem.rsplit('.').next() == Some(lc.as_str());
            if !wanted && by_wmclass.is_some() {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(e.path()) else { continue };
            if wanted {
                return name_of(&text);
            }
            if text.lines().any(|l| l.strip_prefix("StartupWMClass=").is_some_and(|c| c.eq_ignore_ascii_case(class))) {
                by_wmclass = name_of(&text);
            }
        }
    }
    by_wmclass
}

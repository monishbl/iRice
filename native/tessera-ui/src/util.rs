//! Small helpers every surface may want: local time, desktop entries.

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

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
    let home = std::env::var("XDG_DATA_HOME").unwrap_or_else(|_| format!("{}/.local/share", std::env::var("HOME").unwrap_or_default()));
    let dirs = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
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

#[cfg(test)]
mod tests {
    use super::*;

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

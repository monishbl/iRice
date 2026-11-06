//! Settings' map (shell/common/SettingsMap.js): the sidebar's pages under
//! their groups, the pages shown as a part of another, where older page ids
//! go, and the former names search still finds. Tests keep it equal to the
//! JS file.

pub struct Section {
    pub id: &'static str,
    pub name: &'static str,
    pub icon: &'static str,
    pub color: &'static str,
    pub group: &'static str,
}

macro_rules! s {
    ($id:literal, $name:literal, $icon:literal, $color:literal, $group:literal) => {
        Section { id: $id, name: $name, icon: $icon, color: $color, group: $group }
    };
}

pub const SECTIONS: &[Section] = &[
    s!("appearance", "Appearance", "circle-half", "#5E5CE6", "Look & Feel"),
    s!("wallpaper", "Wallpaper", "image", "#32ADE6", "Look & Feel"),
    s!("desktop", "Desktop & Widgets", "squares-four", "#FF9F0A", "Look & Feel"),
    s!("menubar", "Menu Bar & Dock", "list", "#8E8E93", "Look & Feel"),
    s!("lock", "Lock & Login Screen", "lock", "#636366", "Look & Feel"),
    s!("notifications", "Notifications & Island", "bell", "#FF453A", "Notifications"),
    s!("focus", "Focus", "moon", "#5E5CE6", "Notifications"),
    s!("windowrules", "Windows & Spaces", "app-window", "#FF375F", "Windows"),
    s!("displays", "Displays", "monitor", "#0A84FF", "Devices"),
    s!("sound", "Sound", "speaker-high", "#FF375F", "Devices"),
    s!("network", "Wi-Fi & Bluetooth", "wifi-high", "#0A84FF", "Devices"),
    s!("mouse", "Mouse & Trackpad", "mouse", "#636366", "Devices"),
    s!("shortcuts", "Keyboard", "keyboard", "#636366", "Devices"),
    s!("phone", "Phone & Sharing", "device-mobile", "#30D158", "Devices"),
    s!("accounts", "Accounts", "user-circle", "#4285F4", "Accounts & Apps"),
    s!("calendar", "Calendar", "calendar-blank", "#FF453A", "Accounts & Apps"),
    s!("ai", "AI & Sidebar", "sparkle", "#BF5AF2", "Accounts & Apps"),
    s!("defaultapps", "Default Apps", "browsers", "#0A84FF", "Accounts & Apps"),
    s!("security", "Privacy & Security", "shield-check", "#0A84FF", "System"),
    s!("power", "Battery & Power", "battery-high", "#30D158", "System"),
    s!("accessibility", "Accessibility", "person-arms-spread", "#0A84FF", "System"),
    s!("update", "Software Update", "arrow-circle-up", "#8E8E93", "System"),
    s!("about", "About", "info", "#30D158", "System"),
];

/// Page files shown as a part of another page: (file, page, part heading)
pub const PARTS: &[(&str, &str, &str)] = &[
    ("dock", "menubar", "Dock"),
    ("controlcenter", "menubar", "Quick Settings"),
    ("appearance-windows", "windowrules", "Windows, Stages and Overview"),
    ("bluetooth", "network", "Bluetooth"),
    ("trackpad", "mouse", "Trackpad"),
    ("dictation", "shortcuts", "Dictation"),
    ("sharing", "phone", "Sharing"),
    ("google", "accounts", "Google"),
    ("icloud", "accounts", "iCloud"),
    ("microsoft", "accounts", "Microsoft 365"),
    ("sidebar", "ai", "Sidebar"),
    ("biometrics", "security", ""),
];

/// Older page ids -> (page, part to show)
pub const ALIASES: &[(&str, &str, &str)] = &[
    ("dock", "menubar", "Dock"),
    ("controlcenter", "menubar", "Quick Settings"),
    ("menubar/island", "notifications/island", ""),
    ("appearance/windows", "windowrules", "Windows, Stages and Overview"),
    ("appearance/cooldown", "power/cooldown", ""),
    ("bluetooth", "network", "Bluetooth"),
    ("trackpad", "mouse", "Trackpad"),
    ("trackpad/gestures", "mouse/gestures", ""),
    ("dictation", "shortcuts", "Dictation"),
    ("sharing", "phone", "Sharing"),
    ("google", "accounts", "Google"),
    ("icloud", "accounts", "iCloud"),
    ("microsoft", "accounts", "Microsoft 365"),
    ("sidebar", "ai", "Sidebar"),
    ("loginitems", "about/loginitems", ""),
    ("bugreport", "about/bugreport", ""),
];

/// Former names (lower case) -> today's
pub const FORMER: &[(&str, &str)] = &[
    ("spotlight", "search"),
    ("mission control", "all windows"),
    ("app exposé", "app windows"),
    ("exposé", "app windows"),
    ("app library", "app drawer"),
    ("launchpad", "app drawer"),
    ("stage manager", "stages"),
    ("standby", "bedside mode"),
    ("smart stack", "widget stack"),
    ("quick look", "quick preview"),
    ("app store", "software"),
    ("control center", "quick settings"),
    ("control centre", "quick settings"),
    ("liquid glass", "glass"),
    ("dynamic island", "island"),
];

pub fn section(id: &str) -> Option<&'static Section> {
    SECTIONS.iter().find(|s| s.id == id)
}

/// The section a page id is in ("about/health" -> about)
pub fn section_of(page: &str) -> Option<&'static Section> {
    section(page.split('/').next().unwrap_or(""))
}

/// Where an older id goes now: (page, part) or None
pub fn alias(page: &str) -> Option<(&'static str, &'static str)> {
    ALIASES.iter().find(|a| a.0 == page).map(|a| (a.1, a.2))
}

/// A search with any former name swapped for today's (longest first)
pub fn search_query(q: &str) -> String {
    let mut keys: Vec<&(&str, &str)> = FORMER.iter().collect();
    keys.sort_by_key(|k| std::cmp::Reverse(k.0.chars().count()));
    let mut q = q.to_string();
    for (old, new) in keys {
        q = q.split(old).collect::<Vec<_>>().join(new);
    }
    q
}

/// The page a search hit in page file `file` (no .qml) belongs to, and the
/// part heading it's under ("" when none)
pub fn page_of_file(file: &str) -> (String, String) {
    if let Some(p) = PARTS.iter().find(|p| p.0 == file) {
        return (p.1.to_string(), p.2.to_string());
    }
    (file.replacen('-', "/", 1), String::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn js() -> String {
        std::fs::read_to_string(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../shell/common/SettingsMap.js")).unwrap()
    }

    #[test]
    fn the_same_as_settings_map_js() {
        let js = js();
        for s in SECTIONS {
            let line = format!("{{ id: \"{}\", name: \"{}\", icon: \"{}\", color: \"{}\", group: \"{}\" }}", s.id, s.name, s.icon, s.color, s.group);
            assert!(js.contains(&line), "{line}");
        }
        assert_eq!(js.matches("{ id: \"").count(), SECTIONS.len());
        for p in PARTS {
            assert!(js.contains(&format!("\"{}\": {{ page: \"{}\", part: \"{}\" }}", p.0, p.1, p.2)), "{}", p.0);
        }
        for a in ALIASES {
            let line = if a.2.is_empty() { format!("\"{}\": {{ page: \"{}\" }}", a.0, a.1) } else { format!("\"{}\": {{ page: \"{}\", part: \"{}\" }}", a.0, a.1, a.2) };
            assert!(js.contains(&line), "{line}");
        }
        for f in FORMER {
            assert!(js.contains(&format!("\"{}\": \"{}\"", f.0, f.1)), "{}", f.0);
        }
    }

    #[test]
    fn former_names_find_todays() {
        assert_eq!(search_query("spotlight"), "search");
        assert_eq!(search_query("app exposé windows"), "app windows windows");
        assert_eq!(search_query("dock"), "dock");
    }

    #[test]
    fn pages_of_files() {
        assert_eq!(page_of_file("dock"), ("menubar".into(), "Dock".into()));
        assert_eq!(page_of_file("about-health"), ("about/health".into(), "".into()));
        assert_eq!(page_of_file("appearance"), ("appearance".into(), "".into()));
        assert_eq!(section_of("about/health").unwrap().name, "About");
        assert_eq!(alias("trackpad"), Some(("mouse", "Trackpad")));
    }
}

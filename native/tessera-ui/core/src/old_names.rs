//! Tessera's old name in Rust: the one place that still knows it.
//!
//! Tessera was called iRice until 4 November 2026. What an installed machine
//! kept under that name moves over once, as the shell starts
//! (shell/assets/scripts/migrate-names.py: folders, the checkout, settings);
//! what can't move by itself is read here until it has: the system pieces
//! until the root setup (tessera-root.py setup) replaces them, keyring entries
//! and files you made (settings backups, exported shortcuts) under the old
//! name, and the old environment names for one release.
//!
//! REMOVE AFTER 2027-02-01, with old-names.sh, migrate-names.py,
//! tessera-root.py's own section and common/OldNames.qml
//! (docs/rename-to-tessera.md), and the lines that use these.

/// The date after which this module goes (the tests say so then)
pub const REMOVE_AFTER: &str = "2027-02-01";

/// `IRICE_<name>`: an environment name from before (the new one wins)
pub fn env_var(name: &str) -> Option<String> {
    std::env::var(format!("IRICE_{name}")).ok()
}

/// The keyring's `application` for AI keys saved under the old name (looked
/// up after Tessera's, cleared with it)
pub const KEYRING_APP: &str = "irice";

/// A settings backup made under the old name: its extension
pub const BACKUP_EXT: &str = "irice";

/// Shortcuts exported under the old name: their file's top key
pub const KEYBINDS_KEY: &str = "irice-keybinds";

/// Settings values saved under the old name (the menu icon, the screenshot
/// editor), which mean Tessera's
pub const SETTING_VALUE: &str = "irice";

/// The root helper until the root setup installs Tessera's, its polkit
/// actions and the prefix on its messages
pub const ROOT_HELPER: &str = "/usr/lib/irice/irice-root";
pub const ROOT_POLICY: &str = "/usr/share/polkit-1/actions/org.irice.root.policy";
pub const ROOT_PREFIX: &str = "irice-root: ";

/// The helper's folders until the root setup moves them
pub const ETC: &str = "/etc/irice";
pub const PLYMOUTH_BREATHE: &str = "/usr/lib/irice/plymouth/irice-breathe";
/// Boot animations installed under the old name: `irice-<id>`
pub const PLYMOUTH_PREFIX: &str = "irice-";
/// The shell's priority rule until the root setup replaces it
pub const ANANICY_RULE: &str = "/etc/ananicy.d/irice.rules";
/// The login screen's copy until `./install.sh --only=login` replaces it
pub const LOGIN_SHARE: &str = "/usr/share/irice";

/// sh: the helper that's there as `$h` and its polkit actions file as
/// `$policy`: Tessera's (`path`, `policy`), else the old one while only it is
/// installed. Decided when the command runs, so a setup in between never
/// leaves a caller pointing at a helper that's gone.
pub fn root_pick(path: &str, policy: &str) -> String {
    format!("h={path}; policy={policy}; if [ ! -x \"$h\" ] && [ -x {ROOT_HELPER} ]; then h={ROOT_HELPER}; policy={ROOT_POLICY}; fi; ")
}

/// sh: the login screen's folder as `$d`: Tessera's, else the old one while
/// only it is installed
pub fn login_share_pick() -> String {
    format!("d=/usr/share/tessera; [ -d \"$d\" ] || d={LOGIN_SHARE}; ")
}

/// A helper's message without its name in front (either name)
pub fn without_helper_prefix(text: &str) -> String {
    let mut t = text.to_string();
    for p in ["tessera-root: ", ROOT_PREFIX] {
        t = t.replace(p, "");
    }
    t
}

/// A boot animation's id from its Plymouth theme name, under either name
pub fn plymouth_id(theme: &str) -> Option<&str> {
    theme.strip_prefix("tessera-").or_else(|| theme.strip_prefix(PLYMOUTH_PREFIX))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_helper_and_its_messages() {
        let pick = root_pick("/usr/lib/tessera/tessera-root", "/p");
        assert!(pick.starts_with("h=/usr/lib/tessera/tessera-root; policy=/p; "));
        assert!(pick.contains(ROOT_HELPER) && pick.contains(ROOT_POLICY));
        assert_eq!(without_helper_prefix("irice-root: no"), "no");
        assert_eq!(without_helper_prefix("tessera-root: no"), "no");
        assert_eq!(plymouth_id("irice-breathe"), Some("breathe"));
        assert_eq!(plymouth_id("tessera-orbit"), Some("orbit"));
        assert_eq!(plymouth_id("bgrt"), None);
    }

    #[test]
    fn old_environment_names_count_until_the_new_one_is_set() {
        std::env::set_var("IRICE_OLDNAMES_TEST", "old");
        assert_eq!(env_var("OLDNAMES_TEST").as_deref(), Some("old"));
        assert_eq!(crate::env::var("OLDNAMES_TEST").as_deref(), Some("old"));
        std::env::set_var("TESSERA_OLDNAMES_TEST", "new");
        assert_eq!(crate::env::var("OLDNAMES_TEST").as_deref(), Some("new"));
    }

    /// The removal date: past it, this module (and the others it names) go
    #[test]
    fn not_past_its_removal_date() {
        let today = std::process::Command::new("date").arg("+%F").output().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
        assert!(today.is_empty() || today.as_str() <= REMOVE_AFTER, "past {REMOVE_AFTER}: remove Tessera's old name (docs/rename-to-tessera.md)");
    }
}

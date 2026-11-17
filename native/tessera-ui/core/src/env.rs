//! Tessera's environment names (`TESSERA_DEBUG`, `TESSERA_ROOT`, …): the name
//! from before the rename (`IRICE_…`) still works, the new one wins when both
//! are set (docs/rename-to-tessera.md).

/// `TESSERA_<name>`, else `IRICE_<name>`
pub fn var(name: &str) -> Option<String> {
    std::env::var(format!("TESSERA_{name}")).ok().or_else(|| std::env::var(format!("IRICE_{name}")).ok())
}

/// Set, under either name
pub fn is_set(name: &str) -> bool {
    var(name).is_some()
}

/// `--demo` (or TESSERA_UI_DEMO=1): the security surfaces (lock, session,
/// polkit) draw without their real roles (their top comments)
pub fn demo_mode() -> bool {
    std::env::args().any(|a| a == "--demo") || var("UI_DEMO").as_deref() == Some("1")
}

/// The shell's sandbox (TESSERA_SANDBOX=1, as System.qml's `sandboxed`)
pub fn sandboxed() -> bool {
    var("SANDBOX").as_deref() == Some("1")
}

/// The tree this was built from (CARGO_MANIFEST_DIR is core's: three up)
pub const BUILT_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../..");

/// The Tessera checkout whose files the shell uses (shell/assets/scripts,
/// fonts, sounds, pictures; material/assets): $TESSERA_ROOT; else the one
/// launch-shell.sh started this shell from (TESSERA_QML_FALLBACK names that
/// shell folder: a checkout's shell/, its parent the checkout); else the tree
/// this was built from, when it's there; else the installer's checkout
/// ($XDG_DATA_HOME/tessera/source: a copied install's shell folder is
/// ~/.config/quickshell/tessera, beside no checkout). The QML finds its files
/// beside itself (Qt.resolvedUrl); tessera-ui had only the build path compiled
/// in, so a ready-made binary (the release builder's, built on server1) or one
/// built in a worktree since removed found none of its scripts, fonts or
/// pictures standalone (17 Nov)
pub fn root() -> std::path::PathBuf {
    static ROOT: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    ROOT.get_or_init(|| {
        let data = crate::util::xdg("XDG_DATA_HOME", ".local/share");
        let home = std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/share");
        let sources: Vec<std::path::PathBuf> = [&data, &home]
            .iter()
            .flat_map(|d| ["tessera/source", "irice/source"].map(|f| d.join(f)))
            .filter_map(|f| std::fs::read_to_string(f).ok())
            .map(|s| std::path::PathBuf::from(s.trim()))
            .collect();
        let launched = std::env::var_os("TESSERA_QML_FALLBACK").map(std::path::PathBuf::from);
        pick_root(var("ROOT").filter(|r| !r.is_empty()).map(Into::into), launched, BUILT_ROOT.into(), &sources)
    })
    .clone()
}

/// root()'s choice (a checkout counts when it has shell/shell.qml)
fn pick_root(env: Option<std::path::PathBuf>, launched: Option<std::path::PathBuf>, built: std::path::PathBuf, sources: &[std::path::PathBuf]) -> std::path::PathBuf {
    use std::path::Path;
    let checkout = |p: &Path| p.join("shell/shell.qml").is_file();
    if let Some(r) = env {
        return r;
    }
    // (launched from <checkout>/shell, or from a link to it)
    if let Some(shell) = launched.and_then(|s| std::fs::canonicalize(s).ok()) {
        if let Some(parent) = shell.parent().filter(|p| checkout(p) && std::fs::canonicalize(p.join("shell")).ok().as_ref() == Some(&shell)) {
            return parent.to_path_buf();
        }
    }
    if checkout(&built) {
        return built;
    }
    sources.iter().find(|s| checkout(s)).cloned().unwrap_or(built)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_name_wins_and_old_name_still_works() {
        // (names of their own: tests run on threads, the environment is shared)
        std::env::set_var("IRICE_ENVTEST_A", "old");
        assert_eq!(var("ENVTEST_A").as_deref(), Some("old"));
        std::env::set_var("TESSERA_ENVTEST_A", "new");
        assert_eq!(var("ENVTEST_A").as_deref(), Some("new"));
        assert!(is_set("ENVTEST_A"));
        assert!(!is_set("ENVTEST_NEVER_SET"));
    }

    #[test]
    fn the_root_is_the_checkout_the_shell_runs_from() {
        let t = std::env::temp_dir().join(format!("tessera-root-test-{}", std::process::id()));
        let mk = |d: &str| {
            let p = t.join(d);
            std::fs::create_dir_all(p.join("shell")).unwrap();
            std::fs::write(p.join("shell/shell.qml"), "").unwrap();
            p
        };
        let (installed, built, source) = (mk("installed"), mk("built"), mk("source"));
        let gone = t.join("server1-build"); // (a release's compiled-in path: not here)
        let copy = t.join("quickshell/tessera"); // (the installer's copy of shell/)
        std::fs::create_dir_all(&copy).unwrap();
        let canon = |p: &std::path::Path| std::fs::canonicalize(p).unwrap();
        // TESSERA_ROOT always wins
        assert_eq!(pick_root(Some("/x".into()), Some(installed.join("shell")), built.clone(), &[]), std::path::PathBuf::from("/x"));
        // standalone from a checkout's shell/: that checkout, not the build tree
        assert_eq!(pick_root(None, Some(installed.join("shell")), built.clone(), &[]), canon(&installed));
        // a ready-made binary (its build path isn't here), from the installer's copy: the checkout it came from
        assert_eq!(pick_root(None, Some(copy.clone()), gone.clone(), &[source.clone()]), source);
        // nothing launched it (a hand-run one): the build tree while it's there
        assert_eq!(pick_root(None, None, built.clone(), &[source.clone()]), built);
        // nothing found at all: the build path, as before
        assert_eq!(pick_root(None, None, gone.clone(), &[]), gone);
        let _ = std::fs::remove_dir_all(&t);
    }
}

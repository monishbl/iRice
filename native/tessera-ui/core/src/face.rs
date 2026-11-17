//! Face unlock's names, shared by the lock (surfaces/lock/auth.rs), Settings'
//! "Try it" (host/system accounts/face.rs) and `tessera-ui --face-live`.
//!
//! Two PAM files in shell/assets/pam, never the system's:
//!   tessera-face      Howdy with an infrared camera (a photo doesn't show
//!                     up in infrared)
//!   tessera-face-rgb  Howdy with a regular camera, then a blink
//!                     (face_live.py through pam_exec: `--face-live`)
//! Which one: config lock.faceKind ("ir" | "rgb"), written when Settings
//! sets the camera up; anything else counts as infrared, the only kind
//! before regular cameras came.

/// The PAM service for a camera kind
pub fn service(kind: &str) -> &'static str {
    if kind == "rgb" {
        "tessera-face-rgb"
    } else {
        "tessera-face"
    }
}

/// The blink check's lines start with this (face_live.py's `say`); Howdy's
/// own messages don't, and aren't shown
pub const HINT: &str = "Tessera: ";

/// A PAM info line from the face conversation, as text to show (None:
/// Howdy's, not ours)
pub fn hint(info: &str) -> Option<&str> {
    info.strip_prefix(HINT).map(str::trim).filter(|s| !s.is_empty())
}

/// What the PAM helper hands pam_exec's program: pam_exec runs it with
/// PAM's own environment only, so these are put there (pam_putenv)
pub const PAM_ENV: [&str; 7] = ["TESSERA_ROOT", "TESSERA_FACE_DEVICE", "HOME", "PATH", "XDG_DATA_HOME", "XDG_RUNTIME_DIR", "LANG"];

/// `tessera-ui --face-live`: pam_exec's program in tessera-face-rgb (as
/// you: the lock's PAM helper runs as you). It runs face_live.py check,
/// and goes when its parent does (the lock stopped the conversation), so
/// the camera is never left on.
pub fn live_main() -> i32 {
    use std::os::unix::process::CommandExt;
    unsafe {
        libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
    }
    let Some(script) = crate::util::asset("shell/assets/scripts/face_live.py") else {
        eprintln!("tessera-ui: --face-live: face_live.py isn't there (TESSERA_ROOT)");
        return 2;
    };
    let python = if std::env::var_os("PATH").is_some() { "python3" } else { "/usr/bin/python3" };
    let err = std::process::Command::new(python).arg("-I").arg(script).arg("check").exec();
    eprintln!("tessera-ui: --face-live: {err}");
    2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn services_by_kind() {
        assert_eq!(service("rgb"), "tessera-face-rgb");
        assert_eq!(service("ir"), "tessera-face");
        // (unknown or missing: infrared, as before)
        assert_eq!(service(""), "tessera-face");
    }

    #[test]
    fn only_our_lines_are_hints() {
        assert_eq!(hint("Tessera: Blink to unlock"), Some("Blink to unlock"));
        assert_eq!(hint("Tessera: "), None);
        assert_eq!(hint("Identified face as sam"), None);
        assert_eq!(hint("Attempting facial authentication"), None);
    }
}

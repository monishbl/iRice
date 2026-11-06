//! shell/services/RootHelper.qml: Tessera's root helper, as the shell runs
//! it, through pkexec (which asks for the password or not as its polkit rule
//! says). The helper is /usr/lib/tessera/tessera-root; until the root setup
//! has installed it, the iRice-era /usr/lib/irice/irice-root with its own
//! polkit actions. Which one is decided when the command runs.
//!
//! This module only BUILDS commands (as the QML's `command`, `knows`,
//! `message`); nothing here runs pkexec by itself. The one Service call,
//! `roothelper.run [args…]`, runs it detached on a user action (the same
//! thing a QML Process with RootHelper.command(args) does).

use serde_json::Value;
use tessera_core::host::{Host, Service};

pub const PATH: &str = "/usr/lib/tessera/tessera-root";
pub const OLD_PATH: &str = "/usr/lib/irice/irice-root";

/// sh: the helper that's there (Tessera's, else iRice's) as $h, and its
/// polkit actions file as $policy
pub fn pick() -> String {
    format!(
        "h={PATH}; policy=/usr/share/polkit-1/actions/org.tessera.root.policy; if [ ! -x \"$h\" ] && [ -x {OLD_PATH} ]; then h={OLD_PATH}; policy=/usr/share/polkit-1/actions/org.irice.root.policy; fi; "
    )
}

/// ["sh", "-c", …, "sh", args…]: pkexec <helper> args…, as one command
pub fn command(args: &[String]) -> Vec<String> {
    let mut v = vec!["sh".to_string(), "-c".into(), format!("{}exec pkexec \"$h\" \"$@\"", pick()), "sh".into()];
    v.extend(args.iter().cloned());
    v
}

/// Is the helper installed, and does its policy have this subcommand's
/// action (an older helper may not)? As a shell test, for `sh -c`
pub fn knows(subcommand: &str) -> String {
    format!("{}[ -x \"$h\" ] && grep -q \"\\.root\\.{subcommand}\\\"\" \"$policy\" 2>/dev/null", pick())
}

/// A message from the helper, without its "tessera-root: " (or the older
/// "irice-root: ") prefix
pub fn message(text: &str) -> String {
    for p in ["tessera-root: ", "irice-root: "] {
        if let Some(r) = text.strip_prefix(p) {
            return r.to_string();
        }
    }
    text.to_string()
}

#[derive(Default)]
pub struct RootHelper;

impl Service for RootHelper {
    fn name(&self) -> &'static str {
        "RootHelper"
    }
    fn calls(&self) -> &'static [&'static str] {
        &["roothelper"]
    }
    /// roothelper.run [args…]: pkexec <helper> args…, detached (its caller
    /// has asked the user; polkit asks for the password)
    fn call(&mut self, _h: &mut Host, call: &str, args: &Value) {
        if call == "roothelper.run" {
            let a: Vec<String> = args.as_array().map(|a| a.iter().map(|x| x.as_str().map(str::to_string).unwrap_or_else(|| x.to_string())).collect()).unwrap_or_default();
            let c = command(&a);
            let c: Vec<&str> = c.iter().map(String::as_str).collect();
            tessera_core::services::bridge::run(&c);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn message_prefixes() {
        assert_eq!(message("tessera-root: no"), "no");
        assert_eq!(message("irice-root: no"), "no");
        assert_eq!(message("plain"), "plain");
    }
    #[test]
    fn knows_quotes_the_action() {
        assert!(knows("setup").contains("\\.root\\.setup\\\""));
        assert_eq!(command(&["a".into()]).last().unwrap(), "a");
    }
}

//! Which build of the shell this is, the same everywhere: About's Version
//! row and Software Update's This build row. What the running build says
//! about itself (tessera_core::build_info: `tessera-ui --version`), what
//! the installer wrote down (tools/native-install.py: tessera-ui.json beside
//! the installed program), and the channel Software Update follows
//! (update.channel, else the branch the checkout follows): when the build
//! and the channel differ, it says so plainly.

use super::super::super::model::{btn, Ctx, Row};
use super::{feed, s};
use serde_json::Value;
use tessera_core::build_info::{self, BuildInfo};

/// Where native-install puts the programs ($TESSERA_NATIVE_DEST, else
/// $XDG_DATA_HOME/tessera/bin)
fn dest_dir() -> std::path::PathBuf {
    if let Some(d) = std::env::var_os("TESSERA_NATIVE_DEST").filter(|d| !d.is_empty()) {
        return d.into();
    }
    let data = std::env::var_os("XDG_DATA_HOME").filter(|d| !d.is_empty()).map(std::path::PathBuf::from).unwrap_or_else(|| std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".local/share"));
    data.join("tessera/bin")
}

/// What the installer wrote about the installed tessera-ui, and whether
/// that file is a newer one than this process runs (replaced on disk:
/// /proc/self/exe reads "… (deleted)")
pub fn installed() -> Value {
    let dest = dest_dir();
    let rec = std::fs::read(dest.join("tessera-ui.json")).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()).unwrap_or(Value::Null);
    let exe = std::fs::read_link("/proc/self/exe").map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
    let replaced = exe.ends_with(" (deleted)") && std::path::Path::new(exe.trim_end_matches(" (deleted)")) == dest.join("tessera-ui");
    serde_json::json!({"record": rec, "replaced": replaced})
}

/// The channel Software Update follows: update.channel, else what the
/// updater says this copy follows (stable, beta; "custom": its own branch)
pub fn followed(cx: &Ctx) -> String {
    let chosen = cx.s("update.channel");
    if chosen == "stable" || chosen == "beta" {
        return chosen;
    }
    s(&feed(cx)["tu"]["info"]["channel"])
}

/// The plain words when this build isn't the channel Software Update
/// follows (None when they agree, or when there's nothing to compare).
/// `published`: the release the installed build came from (the manifest's
/// channel), for a build promoted from Beta to Stable unchanged
pub fn channel_note(b: &BuildInfo, followed: &str, published: &str) -> Option<String> {
    if followed != "stable" && followed != "beta" {
        return None;
    }
    let f = build_info::channel_name(followed);
    match b.channel {
        c if c == followed => None,
        // (a tagged release is Stable's)
        "release" if followed == "stable" => None,
        "stable" | "beta" | "release" if published == followed => Some(format!("Built on {} and published to {f} unchanged.", b.channel_name())),
        "local build" | "" => Some(format!(
            "Built locally{}, not from a channel. Software Update follows {f}: its next update replaces this build.",
            if b.branch.is_empty() { String::new() } else { format!(" from the branch {}", b.branch) }
        )),
        _ => Some(format!("This build is from {}, but Software Update follows {f}. Check for updates to get {f}'s version.", b.channel_name())),
    }
}

/// The row's detail: the summary, the variant and kind, then what differs
pub fn detail(b: &BuildInfo, followed: &str, inst: &Value) -> String {
    let mut lines = vec![b.summary(), b.details()];
    if let Some(n) = channel_note(b, followed, &s(&inst["record"]["channel"])) {
        lines.push(n);
    }
    if inst["replaced"] == true {
        let said = s(&inst["record"]["build"]);
        let what = said.split_once(" (").map(|x| x.1.trim_end_matches(')').to_string()).unwrap_or_default();
        lines.push(format!("A newer build is installed{}. It starts when the shell restarts.", if what.is_empty() { String::new() } else { format!(" ({what})") }));
    }
    lines.join("\n")
}

/// Software Update's line under the build, once it has checked: what the
/// channel has now (`report`, `info`: tessera-update.py's check and info,
/// as the feed carries them), else "" (not checked yet, or no report)
pub fn latest(checked: bool, available: bool, report: &Value, info: &Value) -> String {
    if !checked || report.is_null() {
        return String::new();
    }
    let ch = s(&report["channelName"]);
    let ch = if ch.is_empty() { s(&info["channelName"]) } else { ch };
    if !available {
        return if ch.is_empty() { "The latest version.".into() } else { format!("The latest on {ch}.") };
    }
    let ui = s(&report["ui"]);
    let ui = ui.strip_prefix("tessera-ui: ").unwrap_or(&ui).to_string();
    format!(
        "{}: version {}{}",
        if ch.is_empty() { "Available".to_string() } else { format!("Available on {ch}") },
        s(&report["latest"]),
        if ui.is_empty() { String::new() } else { format!(", {ui}") }
    )
}

/// The row (About's Version, Software Update's This build): the build in
/// two lines, what differs, and `more` (Software Update: what's available);
/// Copy puts the --version line on the clipboard
pub fn row(cx: &mut Ctx, label: &str, more: &str) -> Row {
    let b = build_info::current();
    let inst = cx.once("about.version.installed", installed);
    let followed = followed(cx);
    let copy = cx.on(|cx, _| {
        crate::pages::devices::copy(&build_info::current().line());
        cx.toast("Version copied", None);
    });
    let mut d = detail(b, &followed, &inst);
    if !more.is_empty() {
        d.push('\n');
        d.push_str(more);
    }
    Row::new(label).detail(d).buttons(vec![btn("Copy").icon("copy")], copy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn beta() -> BuildInfo {
        BuildInfo { version: "0.1.0", commit: "7baa48d0123456789abcdef0123456789abcdef0", date: "2026-11-17", channel: "beta", variant: "x86-64-v3", kind: "release", profile: "release", ..Default::default() }
    }

    #[test]
    fn says_so_when_the_channels_differ() {
        assert_eq!(channel_note(&beta(), "beta", ""), None);
        assert_eq!(channel_note(&beta(), "custom", ""), None);
        assert_eq!(channel_note(&beta(), "", ""), None);
        assert_eq!(channel_note(&beta(), "stable", "").as_deref(), Some("This build is from Beta, but Software Update follows Stable. Check for updates to get Stable's version."));
        // (promoted from beta to main: the same build, published to Stable)
        assert_eq!(channel_note(&beta(), "stable", "stable").as_deref(), Some("Built on Beta and published to Stable unchanged."));
        let tag = BuildInfo { channel: "release", ..beta() };
        assert_eq!(channel_note(&tag, "stable", ""), None);
        let local = BuildInfo { channel: "local build", branch: "version", kind: "local", ..beta() };
        assert_eq!(channel_note(&local, "beta", "").as_deref(), Some("Built locally from the branch version, not from a channel. Software Update follows Beta: its next update replaces this build."));
    }

    #[test]
    fn what_the_channel_has() {
        let info = json!({"channelName": "Beta"});
        assert_eq!(latest(false, false, &json!({}), &info), "");
        assert_eq!(latest(true, false, &Value::Null, &info), "");
        assert_eq!(latest(true, false, &json!({"channelName": ""}), &info), "The latest on Beta.");
        let r = json!({"channelName": "Beta", "latest": "5e6f7a8b", "ui": "tessera-ui: ready-made (v3)"});
        assert_eq!(latest(true, true, &r, &info), "Available on Beta: version 5e6f7a8b, ready-made (v3)");
        let r = json!({"channelName": "Stable", "latest": "5e6f7a8b", "ui": ""});
        assert_eq!(latest(true, true, &r, &info), "Available on Stable: version 5e6f7a8b");
    }

    #[test]
    fn the_detail() {
        let none = json!({"record": null, "replaced": false});
        assert_eq!(detail(&beta(), "beta", &none), "Beta · 7baa48d · 17 November 2026\nx86-64-v3 · signed release");
        let newer = json!({"record": {"channel": "beta", "build": "tessera-ui 0.1.0 (beta, 1a2b3c4, 2026-11-18, x86-64-v3, signed release)"}, "replaced": true});
        assert_eq!(
            detail(&beta(), "stable", &newer),
            "Beta · 7baa48d · 17 November 2026\nx86-64-v3 · signed release\nThis build is from Beta, but Software Update follows Stable. Check for updates to get Stable's version.\nA newer build is installed (beta, 1a2b3c4, 2026-11-18, x86-64-v3, signed release). It starts when the shell restarts."
        );
    }
}

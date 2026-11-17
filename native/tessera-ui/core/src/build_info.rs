//! Which build this is: the one source for `tessera-ui --version`
//! (`--version --json`), Settings → About's Version row and Software
//! Update's "This build" row. The binary's build.rs works it out
//! (build_resolve.rs: the release builder's variables, else git) and
//! src/main.rs hands it here first thing (`set`); tessera-core itself has no
//! build script, so a new commit recompiles only the binary's crate.
//!
//!   tessera-ui 0.1.0 (beta, 7baa48d, 2026-11-17, x86-64-v3, signed release)
//!   tessera-ui 0.1.0 (local build, 7baa48d-dirty, 2026-11-17, generic, fast profile, built locally)

use serde_json::{json, Value};
use std::sync::OnceLock;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BuildInfo {
    /// Cargo's version (the workspace's)
    pub version: &'static str,
    /// the full commit, "" when unknown
    pub commit: &'static str,
    /// the commit's date, YYYY-MM-DD, "" when unknown
    pub date: &'static str,
    /// native/tessera-ui had edits that weren't committed
    pub dirty: bool,
    /// the branch it was built on ("" detached or unknown)
    pub branch: &'static str,
    /// stable, beta, release (a v* tag), local build
    pub channel: &'static str,
    /// x86-64-v3, generic, native (tuned for the computer that built it)
    pub variant: &'static str,
    /// release (the release builder's, signed when published) or local
    pub kind: &'static str,
    /// cargo's profile: release, fast, debug
    pub profile: &'static str,
}

static CURRENT: OnceLock<BuildInfo> = OnceLock::new();

/// The binary's build, once, before anything asks (src/main.rs)
pub fn set(b: BuildInfo) {
    let _ = CURRENT.set(b);
}

/// This build; unknown everywhere but the version when `set` wasn't called
/// (a crate's own tests)
pub fn current() -> &'static BuildInfo {
    CURRENT.get_or_init(|| BuildInfo { version: env!("CARGO_PKG_VERSION"), channel: "local build", variant: "generic", kind: "local", ..Default::default() })
}

const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

impl BuildInfo {
    /// The commit as 7 characters, or "unknown"
    pub fn short(&self) -> &str {
        if self.commit.is_empty() {
            "unknown"
        } else {
            self.commit.get(..7).unwrap_or(self.commit)
        }
    }

    /// The short commit, "-dirty" when it had edits
    pub fn commit_label(&self) -> String {
        format!("{}{}", self.short(), if self.dirty { "-dirty" } else { "" })
    }

    pub fn is_release(&self) -> bool {
        self.kind == "release"
    }

    /// "signed release" or "built locally"
    pub fn kind_text(&self) -> &'static str {
        if self.is_release() {
            "signed release"
        } else {
            "built locally"
        }
    }

    /// The profile when it isn't the release one ("fast profile", "dev
    /// profile"), else ""
    pub fn profile_text(&self) -> String {
        match self.profile {
            "" | "release" => String::new(),
            "debug" => "dev profile".into(),
            p => format!("{p} profile"),
        }
    }

    /// The channel as a person reads it: Stable, Beta, Release, Local build
    pub fn channel_name(&self) -> String {
        channel_name(self.channel)
    }

    /// The variant as a person reads it ("native": tuned for this processor)
    pub fn variant_text(&self) -> String {
        match self.variant {
            "native" => "tuned for this processor".into(),
            "" => "generic".into(),
            v => v.into(),
        }
    }

    /// The --version line: name, version, then (channel, commit, date,
    /// variant, [profile,] kind)
    pub fn line(&self) -> String {
        let mut parts = vec![
            if self.channel.is_empty() { "local build".to_string() } else { self.channel.to_string() },
            self.commit_label(),
            if self.date.is_empty() { "unknown date".to_string() } else { self.date.to_string() },
            if self.variant.is_empty() { "generic".to_string() } else { self.variant.to_string() },
        ];
        let p = self.profile_text();
        if !p.is_empty() {
            parts.push(p);
        }
        parts.push(self.kind_text().to_string());
        format!("tessera-ui {} ({})", self.version, parts.join(", "))
    }

    /// --version --json
    pub fn json(&self) -> Value {
        json!({
            "name": "tessera-ui", "version": self.version, "commit": self.commit, "short": self.short(),
            "date": self.date, "dirty": self.dirty, "branch": self.branch, "channel": self.channel,
            "variant": self.variant, "profile": self.profile, "kind": self.kind, "line": self.line(),
        })
    }

    /// The commit's date as "17 November 2026" (Software Update's dates), or ""
    pub fn long_date(&self) -> String {
        long_date(self.date)
    }

    /// Settings' first line: "Beta · 7baa48d · 17 November 2026"
    /// (a local build off a branch names it)
    pub fn summary(&self) -> String {
        let ch = if self.channel == "local build" && !self.branch.is_empty() { format!("Local build of {}", self.branch) } else { self.channel_name() };
        let d = self.long_date();
        let mut s = format!("{ch} · {}", self.commit_label());
        if !d.is_empty() {
            s.push_str(" · ");
            s.push_str(&d);
        }
        s
    }

    /// Settings' second line: "x86-64-v3 · signed release" (· fast profile)
    pub fn details(&self) -> String {
        let mut v = vec![self.variant_text(), self.kind_text().to_string()];
        let p = self.profile_text();
        if !p.is_empty() {
            v.push(p);
        }
        v.join(" · ")
    }
}

/// stable → Stable, beta → Beta, release → Release, local build → Local build
pub fn channel_name(c: &str) -> String {
    match c {
        "stable" => "Stable".into(),
        "beta" => "Beta".into(),
        "release" => "Release".into(),
        "" | "local build" => "Local build".into(),
        c => {
            let mut ch = c.chars();
            ch.next().map(|f| f.to_uppercase().collect::<String>() + ch.as_str()).unwrap_or_default()
        }
    }
}

/// "2026-11-17" as "17 November 2026", or ""
pub fn long_date(d: &str) -> String {
    let num = |a: usize, b: usize| d.get(a..b).and_then(|x| x.parse::<u32>().ok());
    match (num(0, 4), num(5, 7), num(8, 10)) {
        (Some(y), Some(m @ 1..=12), Some(day)) if d.len() == 10 => format!("{day} {} {y}", MONTHS[m as usize - 1]),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build_resolve as r;

    fn beta() -> BuildInfo {
        BuildInfo {
            version: "0.1.0",
            commit: "7baa48d0123456789abcdef0123456789abcdef0",
            date: "2026-11-17",
            dirty: false,
            branch: "",
            channel: "beta",
            variant: "x86-64-v3",
            kind: "release",
            profile: "release",
        }
    }

    #[test]
    fn the_version_line() {
        assert_eq!(beta().line(), "tessera-ui 0.1.0 (beta, 7baa48d, 2026-11-17, x86-64-v3, signed release)");
        let local = BuildInfo { channel: "local build", dirty: true, variant: "generic", kind: "local", profile: "fast", branch: "version", ..beta() };
        assert_eq!(local.line(), "tessera-ui 0.1.0 (local build, 7baa48d-dirty, 2026-11-17, generic, fast profile, built locally)");
        let dev = BuildInfo { profile: "debug", kind: "local", ..beta() };
        assert_eq!(dev.line(), "tessera-ui 0.1.0 (beta, 7baa48d, 2026-11-17, x86-64-v3, dev profile, built locally)");
        // (a tarball build: nothing known but the version)
        let none = BuildInfo { version: "0.1.0", ..Default::default() };
        assert_eq!(none.line(), "tessera-ui 0.1.0 (local build, unknown, unknown date, generic, built locally)");
        // (native-install checks that the line starts with the name)
        assert!(none.line().starts_with("tessera-ui "));
    }

    #[test]
    fn the_json() {
        let j = beta().json();
        assert_eq!(j["short"], "7baa48d");
        assert_eq!(j["channel"], "beta");
        assert_eq!(j["kind"], "release");
        assert_eq!(j["dirty"], false);
        assert_eq!(j["line"], beta().line());
    }

    #[test]
    fn what_settings_shows() {
        assert_eq!(beta().summary(), "Beta · 7baa48d · 17 November 2026");
        assert_eq!(beta().details(), "x86-64-v3 · signed release");
        let tuned = BuildInfo { channel: "local build", branch: "rust-rewrite", variant: "native", kind: "local", profile: "fast", dirty: true, ..beta() };
        assert_eq!(tuned.summary(), "Local build of rust-rewrite · 7baa48d-dirty · 17 November 2026");
        assert_eq!(tuned.details(), "tuned for this processor · built locally · fast profile");
        assert_eq!(BuildInfo { date: "", ..beta() }.summary(), "Beta · 7baa48d");
        assert_eq!(long_date("2026-01-02"), "2 January 2026");
        assert_eq!(long_date("2026-13-02"), "");
        assert_eq!(channel_name("stable"), "Stable");
        assert_eq!(channel_name("nightly"), "Nightly");
    }

    #[test]
    fn the_environment_first_then_git() {
        // channel: the release builder's word, else the branch
        assert_eq!(r::channel(Some("beta"), "main"), "beta");
        assert_eq!(r::channel(Some("  "), "main"), "stable");
        assert_eq!(r::channel(None, "beta"), "beta");
        assert_eq!(r::channel(None, "rust-rewrite"), "local build");
        assert_eq!(r::channel(None, ""), "local build");
        // kind: release only when said
        assert_eq!(r::kind(Some("release")), "release");
        assert_eq!(r::kind(Some("local")), "local");
        assert_eq!(r::kind(Some("whatever")), "local");
        assert_eq!(r::kind(None), "local");
        // variant: said, else RUSTFLAGS' target-cpu, else generic
        assert_eq!(r::variant(Some("x86-64-v3"), ""), "x86-64-v3");
        assert_eq!(r::variant(None, "-C\x1ftarget-cpu=x86-64-v3"), "x86-64-v3");
        assert_eq!(r::variant(None, "-Ctarget-cpu=native"), "native");
        assert_eq!(r::variant(None, "-C\x1ftarget-cpu=x86-64-v3\x1f-C\x1ftarget-cpu=native"), "native");
        assert_eq!(r::variant(None, "-C\x1fopt-level=3"), "generic");
        assert_eq!(r::variant(None, ""), "generic");
        // profile: the target folder's
        assert_eq!(r::profile("/t/target/fast/build/tessera-ui-1a2b/out", "release"), "fast");
        assert_eq!(r::profile("/t/target/x86_64-unknown-linux-gnu/release/build/x/out", "release"), "release");
        assert_eq!(r::profile("/t/target/debug/build/x/out", "debug"), "debug");
        assert_eq!(r::profile("/elsewhere/out", "release"), "release");
        // commit and date: only well-formed ones
        assert_eq!(r::commit(" 7BAA48D0123456789ABCDEF0123456789ABCDEF0\n"), "7baa48d0123456789abcdef0123456789abcdef0");
        assert_eq!(r::commit("7baa48d"), "");
        assert_eq!(r::commit("not a commit at all, forty characters ok"), "");
        assert_eq!(r::date("2026-11-17T08:00:00+01:00"), "2026-11-17");
        assert_eq!(r::date("2026-11-17"), "2026-11-17");
        assert_eq!(r::date("17 Nov"), "");
    }
}

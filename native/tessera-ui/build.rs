//! What this build is, for `tessera-ui --version` and Settings (About,
//! Software Update): src/main.rs hands it to core/src/build_info.rs.
//!
//! From the environment first (the release builder,
//! tools/release-builder/tessera-release; tools/native-install.py's local
//! builds):
//!   TESSERA_BUILD_CHANNEL  stable | beta | release (a v* tag) | …
//!   TESSERA_BUILD_VARIANT  x86-64-v3 | generic | native
//!   TESSERA_BUILD_KIND     release (the release builder's, signed when published) | local
//!   TESSERA_BUILD_COMMIT   the full commit (for a build without git: a tarball)
//!   TESSERA_BUILD_DATE     its date, YYYY-MM-DD
//! else from git (the commit, its date, the branch: main is stable, beta is
//! beta, anything else a local build; edits in native/tessera-ui: dirty),
//! else unknown. The variant falls back to RUSTFLAGS' -C target-cpu (none:
//! generic); the profile is the target folder's (release, fast, debug).
//! The decisions themselves: core/src/build_resolve.rs (tested there).
//!
//! It never reruns on every build: only when one of those variables
//! changes, when HEAD moves (HEAD, the branch's ref, packed-refs), or when a
//! source here changes (and then the binary recompiles anyway). Without git
//! it watches only the variables.

#[path = "core/src/build_resolve.rs"]
mod build_resolve;

use std::path::{Path, PathBuf};
use std::process::Command;

const VARS: [&str; 5] = ["TESSERA_BUILD_CHANNEL", "TESSERA_BUILD_VARIANT", "TESSERA_BUILD_KIND", "TESSERA_BUILD_COMMIT", "TESSERA_BUILD_DATE"];

fn env(k: &str) -> Option<String> {
    std::env::var(k).ok()
}

fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let o = Command::new("git").arg("-C").arg(dir).args(args).output().ok()?;
    o.status.success().then(|| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// Rerun when P changes, only when it's there (a path that isn't there
/// would rerun this on every build)
fn watch(p: &Path) {
    if p.exists() {
        println!("cargo:rerun-if-changed={}", p.display());
    }
}

fn main() {
    let dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    for v in VARS {
        println!("cargo:rerun-if-env-changed={v}");
    }
    // (with no rerun-if-changed at all cargo would rerun this on any change
    // in the package: build.rs itself always counts)
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=core/src/build_resolve.rs");

    let mut commit = build_resolve::commit(&env("TESSERA_BUILD_COMMIT").unwrap_or_default());
    let mut date = build_resolve::date(&env("TESSERA_BUILD_DATE").unwrap_or_default());
    let mut branch = String::new();
    let mut dirty = false;
    if git(&dir, &["rev-parse", "--is-inside-work-tree"]).as_deref() == Some("true") {
        // What moves HEAD: HEAD itself, the branch's ref, packed-refs (in a
        // worktree HEAD is its own, the refs are the common folder's)
        let abs = |p: String| if Path::new(&p).is_absolute() { PathBuf::from(p) } else { dir.join(p) };
        if let Some(g) = git(&dir, &["rev-parse", "--git-dir"]).map(abs) {
            watch(&g.join("HEAD"));
        }
        let head_ref = git(&dir, &["symbolic-ref", "--quiet", "HEAD"]).unwrap_or_default();
        if let Some(c) = git(&dir, &["rev-parse", "--git-common-dir"]).map(abs) {
            if !head_ref.is_empty() {
                watch(&c.join(&head_ref));
            }
            watch(&c.join("packed-refs"));
        }
        branch = head_ref.strip_prefix("refs/heads/").unwrap_or("").to_string();
        if commit.is_empty() {
            commit = build_resolve::commit(&git(&dir, &["rev-parse", "HEAD"]).unwrap_or_default());
        }
        if date.is_empty() && !commit.is_empty() {
            date = build_resolve::date(&git(&dir, &["log", "-1", "--format=%cs", &commit]).unwrap_or_default());
        }
        // Edits of yours (as native-install's `edited`): the sources watched,
        // so an edit reruns this, and then the binary recompiles anyway
        for p in ["src", "core", "surfaces", "host", "vendor", "Cargo.toml", "Cargo.lock"] {
            watch(&dir.join(p));
        }
        dirty = git(&dir, &["status", "--porcelain", "--untracked-files=normal", "--", "."]).is_some_and(|s| !s.is_empty());
    }

    let channel = build_resolve::channel(env("TESSERA_BUILD_CHANNEL").as_deref(), &branch);
    let variant = build_resolve::variant(env("TESSERA_BUILD_VARIANT").as_deref(), &env("CARGO_ENCODED_RUSTFLAGS").unwrap_or_default());
    let kind = build_resolve::kind(env("TESSERA_BUILD_KIND").as_deref());
    let profile = build_resolve::profile(&env("OUT_DIR").unwrap_or_default(), &env("PROFILE").unwrap_or_default());

    let set = |k: &str, v: &str| println!("cargo:rustc-env={k}={}", v.replace(['\n', '\r'], " "));
    set("TESSERA_BI_COMMIT", &commit);
    set("TESSERA_BI_DATE", &date);
    set("TESSERA_BI_DIRTY", if dirty { "1" } else { "" });
    set("TESSERA_BI_BRANCH", &branch);
    set("TESSERA_BI_CHANNEL", &channel);
    set("TESSERA_BI_VARIANT", &variant);
    set("TESSERA_BI_KIND", kind);
    set("TESSERA_BI_PROFILE", &profile);
}

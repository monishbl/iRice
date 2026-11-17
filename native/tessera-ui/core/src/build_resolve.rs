//! How a build decides what it is (the binary's build.rs includes this file
//! with #[path]; tessera-core compiles it too, for its tests): the release
//! builder's variables first, then git, then RUSTFLAGS and the target folder.
//! Plain std only: build.rs has no dependencies.

/// The channel: TESSERA_BUILD_CHANNEL, else the branch (main: stable, beta:
/// beta), else "local build" (another branch, a detached HEAD, no git)
pub fn channel(env: Option<&str>, branch: &str) -> String {
    if let Some(c) = env.map(str::trim).filter(|c| !c.is_empty()) {
        return c.to_string();
    }
    match branch {
        "main" => "stable".into(),
        "beta" => "beta".into(),
        _ => "local build".into(),
    }
}

/// The kind: "release" only when the release builder says so; anything
/// else was built somewhere else, so "local"
pub fn kind(env: Option<&str>) -> &'static str {
    match env.map(str::trim) {
        Some("release") => "release",
        _ => "local",
    }
}

/// The variant: TESSERA_BUILD_VARIANT, else RUSTFLAGS' -C target-cpu
/// (CARGO_ENCODED_RUSTFLAGS: flags apart by 0x1f), else generic
pub fn variant(env: Option<&str>, encoded_rustflags: &str) -> String {
    if let Some(v) = env.map(str::trim).filter(|v| !v.is_empty()) {
        return v.to_string();
    }
    target_cpu(encoded_rustflags).unwrap_or_else(|| "generic".into())
}

/// -C target-cpu=… (the last one wins, as rustc takes it), or -Ctarget-cpu=…
pub fn target_cpu(encoded: &str) -> Option<String> {
    let parts: Vec<&str> = encoded.split('\x1f').collect();
    let mut found = None;
    for (i, f) in parts.iter().enumerate() {
        if let Some(v) = f.strip_prefix("-Ctarget-cpu=") {
            found = Some(v.to_string());
        } else if let Some(v) = f.strip_prefix("target-cpu=") {
            if i > 0 && parts[i - 1] == "-C" {
                found = Some(v.to_string());
            }
        }
    }
    found.filter(|v| !v.is_empty())
}

/// The profile from OUT_DIR: the folder above `build` (target/<profile>/build/…,
/// target/<triple>/<profile>/build/…); `fallback` (cargo's PROFILE) when
/// that isn't the shape
pub fn profile(out_dir: &str, fallback: &str) -> String {
    let comps: Vec<&str> = out_dir.split('/').filter(|c| !c.is_empty()).collect();
    comps.iter().rposition(|c| *c == "build").and_then(|i| i.checked_sub(1)).map(|i| comps[i].to_string()).unwrap_or_else(|| fallback.to_string())
}

/// A commit's date as YYYY-MM-DD, or "" (TESSERA_BUILD_DATE may be a full
/// ISO time or a date)
pub fn date(s: &str) -> String {
    let d = s.trim().get(..10).unwrap_or("");
    let b = d.as_bytes();
    if b.len() == 10 && b[4] == b'-' && b[7] == b'-' && d.chars().enumerate().all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit()) {
        d.to_string()
    } else {
        String::new()
    }
}

/// A full commit (40 or 64 hex digits), or "" (anything else is ignored)
pub fn commit(s: &str) -> String {
    let s = s.trim().to_ascii_lowercase();
    if (s.len() == 40 || s.len() == 64) && s.chars().all(|c| c.is_ascii_hexdigit()) {
        s
    } else {
        String::new()
    }
}

//! Every icon the desktop draws, looked up and drawn in the installed icon
//! themes (the user's Colloid, and Adwaita): a name that resolves to nothing,
//! or to a file that draws nothing, is an icon that doesn't load.
//!   cargo test --release -p tessera-desktop --test icon_themes -- --ignored --nocapture

use std::collections::HashMap;
use std::sync::Arc;
use tessera_core::appicon::{self, Style};
use tessera_core::services::icons::{self, Icons};

/// The names the desktop asks for (files::icon_name and its fallback)
const NAMES: &[&str] = &["folder", "text-x-generic", "image-x-generic", "video-x-generic", "audio-x-generic", "application-pdf", "package-x-generic", "user-trash", "folder-download"];

fn check(theme: &str) -> Vec<String> {
    let roots = icons::roots();
    if !roots.iter().any(|r| r.join(theme).join("index.theme").is_file()) {
        eprintln!("{theme}: not installed, skipped");
        return Vec::new();
    }
    let index: HashMap<_, _> = icons::build(&roots, theme);
    let icons = Icons { theme: theme.into(), index: Arc::new(index) };
    let plain = Style { mode: 0, tiled: false, dark: true, contrast: 60, tint: [0; 3], tile: None, tile_from: "auto".into(), accent: [0; 3] };
    let mut bad = Vec::new();
    for n in NAMES {
        let p = icons.path(&[n], &["text-x-generic"]);
        let drawn = p.as_ref().and_then(|p| appicon::render(p, 96, &plain, None));
        let ink = drawn.as_ref().map_or(0, |b| b.as_bytes().chunks(4).filter(|c| c[3] > 0).count());
        eprintln!("{theme:>12} {n:<20} {p:?} drawn px {ink}");
        if ink < 50 {
            bad.push(format!("{theme}:{n}"));
        }
    }
    bad
}

#[test]
#[ignore]
fn desktop_icons_draw_in_colloid_and_adwaita() {
    let mut bad = check("Colloid");
    bad.extend(check("Colloid-Dark"));
    bad.extend(check("Adwaita"));
    assert!(bad.is_empty(), "icons that don't draw: {bad:?}");
}

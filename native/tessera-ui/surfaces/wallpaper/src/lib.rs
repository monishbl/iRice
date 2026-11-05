//! The wallpaper: the background behind the windows (background/, the
//! "background" surface) and the wallpaper picker (wallpaper/, "wallpaper").
//! One crate, as they share a page of Slint and their pictures; what the
//! look and other surfaces read too (which picture shows, decoding pictures)
//! is tessera-core's (wallpaper.rs, picture.rs).

pub mod ui {
    slint::include_modules!();
}
tessera_core::theme_global!(ui);

// tessera-core's modules (shell, wayland, theme, services, util…) under the
// names they had when everything was one crate: `crate::shell::Part`
#[allow(unused_imports)]
use tessera_core::*;

pub mod background;
pub mod wallpaper;

pub use background::Background;
pub use wallpaper::WallpaperPicker;

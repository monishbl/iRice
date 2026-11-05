//! tessera-core: what every surface of tessera-ui shares (docs/rust-ui-surfaces.md,
//! "The workspace"). The platform (Wayland layer surfaces, EGL, accessibility,
//! frame statistics), the shell's common state and the `Part` trait every
//! surface implements (shell.rs), the look's tokens (theme.rs, ui/theme.slint),
//! the services that read the system (services/), and helpers more than one
//! surface uses. Each surface is a crate of its own (native/tessera-ui/surfaces/),
//! and the tessera-ui binary (src/main.rs) names them all.
//!
//! A surface crate's lib.rs brings these modules in under the names they had
//! when everything was one crate (`crate::shell`, `crate::wayland`…), so a
//! surface's code reads the same as before the split.

pub mod a11y;
pub mod appicon;
pub mod dnd;
pub mod egl;
pub mod env;
pub mod icon_theme;
pub mod icons;
pub mod picture;
pub mod runloop;
pub mod seed;
pub mod services;
pub mod settings;
pub mod shell;
pub mod stats;
pub mod sysstat; // (the last minute of CPU, memory, network: desktop, resources, overlay)
pub mod surfaces;
pub mod theme;
pub mod util;
pub mod wallpaper;
pub mod wayland;

pub enum Msg {
    Run(Box<dyn FnOnce() + Send>),
    Quit,
    /// A service's new reading (any type; the shell keeps the newest of each)
    Data(Box<dyn std::any::Any + Send>),
    /// A message for one surface's part, by name
    Part(&'static str, Box<dyn std::any::Any + Send>),
    GlassPlugin(bool),
    ConfigChanged,
    A11yActivate(u64),
    A11yAction(u64, accesskit::ActionRequest),
}

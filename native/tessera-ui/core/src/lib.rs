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
pub mod build_info; // (which build this is: --version, Settings → About and Software Update)
pub mod build_resolve; // (how build.rs decides it; tested in build_info)
pub mod build_progress; // (tools/build-progress.py's file: Software Update, the update screen)
pub mod cfgfile; // (config.json and state.json read and changed safely)
pub mod contain; // (a panic in one part never takes the shell down: contain.rs)
pub mod crash; // (crash reports: crash.rs)
pub mod damage;
pub mod dnd;
#[cfg(feature = "skia")]
pub mod skia;
pub mod egl;
pub mod gpu;
pub mod inhibit;
pub mod outside; // (popovers close on a press on any screen: outside.rs)
pub mod memory; // (malloc's arenas and threshold, freed memory given back when quiet: memory.rs)
pub mod env;
pub mod face; // (face unlock's PAM services, hints and `--face-live`)
pub mod health; // (the health record Settings → Health shows: health.rs)
pub mod global_shortcuts; // (Hyprland's global shortcuts, as Quickshell's GlobalShortcut: standalone)
pub mod host; // (wave 4: the QML shell's side, in Rust; standalone only)
pub mod icon_theme;
pub mod idle; // (ext-idle-notify-v1 readings, wayland.rs State::idle_set)
pub mod icons;
pub mod picture;
pub mod power_supply; // (/sys/class/power_supply: own batteries, adapters of any name)
pub mod rank;
pub mod pacing; // (the display's refresh, frames on screen, frame timers, the idle timer grid)
pub mod perf; // (debug: loop stalls and list/window reloads, TESSERA_DEBUG: perf.rs)
pub mod rows; // (lists changed in place, not handed a new model: rows.rs)
pub mod saver; // (battery saver: 60 Hz, reduced motion, slower background refreshes)
pub mod runloop;
pub mod seed;
pub mod services;
pub mod settings;
pub mod shell;
pub mod spacemap;
pub mod stats;
pub mod sysstat; // (the last minute of CPU, memory, network: desktop, resources, overlay)
pub mod surfaces;
pub mod textflex; // (Material's text face drawn as Qt draws it: textflex.rs)
pub mod theme;
pub mod touch; // (wl_touch as the pointer: taps, drags, long presses; every surface)
pub mod util;
pub mod wallpaper;
pub mod wayland;
pub mod wm;
pub mod work; // (blocking work off the UI thread: a fixed pool and ordered lanes)

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
    /// A line as the shell writes it to stdin, said by the host (standalone:
    /// host.rs, Host::say); the binary handles it as if it came on stdin
    Line(String),
}

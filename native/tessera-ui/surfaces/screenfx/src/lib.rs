//! The screen-wide overlays that had no Rust version (rust.surfaces:
//! screencorners, findpointer, spaceswitch, displayconnect, displayoverlay,
//! shelfprobes), one Part each in this crate:
//!
//! - `ScreenCorners` (corners.rs): the rounded bottom screen corners
//!   (shell/modules/background/ScreenCorners.qml).
//! - `FindPointer` (ring.rs): the find-pointer ring (shell/services/FindPointer.qml).
//! - `SpaceSwitch` (cue.rs): the cue on the display you moved to
//!   (shell/modules/osd/SpaceSwitch.qml).
//! - `DisplayConnect` (connect.rs): the effect when a display is plugged in or
//!   leaves (shell/modules/osd/DisplayConnect.qml, displayfx.frag).
//! - `DisplayOverlay` (display.rs): Identify Displays and "Keep these display
//!   settings?" (DisplayOverlay.qml, MaterialDisplayIdentify.qml,
//!   MaterialDisplayConfirm.qml).
//! - `ShelfProbes` (probe.rs): the edge and shake drop strips that open the
//!   shelf (shell/modules/shelf/ShelfProbes.qml).
//!
//! All of them are standalone only (`tessera-ui --standalone`): with the QML
//! shell running, its own copies draw these, and a second set here would
//! show twice. Standalone the host feeds them (`{"to":"<surface>",…}` lines,
//! see PORT-NOTES-wave4b.md); none polls.

pub mod ui {
    slint::include_modules!();
}
tessera_core::theme_global!(ui);

// tessera-core's modules (shell, wayland, theme, services, util…) under the
// names they had when everything was one crate: `crate::shell::Part`
#[allow(unused_imports)]
use tessera_core::*;

pub mod connect;
pub mod corners;
pub mod cue;
pub mod display;
pub mod probe;
pub mod ring;

pub use connect::DisplayConnect;
pub use corners::ScreenCorners;
pub use cue::SpaceSwitch;
pub use display::DisplayOverlay;
pub use probe::ShelfProbes;
pub use ring::FindPointer;

use tessera_core::wayland::{Spec, State, Surface};
use smithay_client_toolkit::reexports::client::protocol::wl_output::WlOutput;
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use std::any::{Any, TypeId};
use std::rc::Rc;

/// These parts draw only when tessera-ui is the whole shell (see above)
pub(crate) fn on() -> bool {
    tessera_core::host::standalone()
}

/// A layer surface over a whole output, on the overlay layer
pub(crate) fn overlay(st: &mut State, output: &WlOutput, name: &str, ns: &str, keyboard: KeyboardInteractivity) -> Rc<Surface> {
    st.prepare_surface(Spec {
        layer: Layer::Overlay,
        namespace: ns,
        output: Some((output, name)),
        anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
        size: (0, 0),
        exclusive: -1,
        keyboard,
        margin_left: 0,
    })
}

/// Take a surface off the screen and out of the compositor's lists
pub(crate) fn retire(st: &mut State, s: &Rc<Surface>) {
    s.hide(); // (unmapped now: dropping the window alone leaves it on screen)
    st.drop_surface(s);
}

// ── Easing (the QML's NumberAnimation curves), t in 0..1 ────────────────────

pub(crate) fn clamp01(t: f32) -> f32 {
    t.clamp(0.0, 1.0)
}
pub(crate) fn out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - clamp01(t)).powi(3)
}
pub(crate) fn in_cubic(t: f32) -> f32 {
    clamp01(t).powi(3)
}
pub(crate) fn in_out_cubic(t: f32) -> f32 {
    let t = clamp01(t);
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}
pub(crate) fn in_out_sine(t: f32) -> f32 {
    -((std::f32::consts::PI * clamp01(t)).cos() - 1.0) / 2.0
}

/// A line of the shell's JSON for this part as a Value, if it is one
pub(crate) fn said(msg: Box<dyn Any + Send>) -> Result<serde_json::Value, Box<dyn Any + Send>> {
    msg.downcast::<serde_json::Value>().map(|v| *v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn easings_run_from_0_to_1() {
        for f in [out_cubic, in_cubic, in_out_cubic, in_out_sine] {
            assert!(f(0.0).abs() < 1e-6);
            assert!((f(1.0) - 1.0).abs() < 1e-6);
            assert!((f(2.0) - 1.0).abs() < 1e-6);
        }
        assert!(out_cubic(0.5) > 0.5 && in_cubic(0.5) < 0.5);
    }
}

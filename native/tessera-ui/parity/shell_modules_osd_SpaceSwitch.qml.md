# shell/modules/osd/SpaceSwitch.qml

Parity (12 Nov, written by the lead; not built or tested yet).

Rust: surfaces/screenfx/src/cue.rs + ui/cue.slint (rust.surfaces: spaceswitch); the decision in host/desktop wm/spaceswitch.rs.
- Glow 150 in / 250 held / 450 out, dim 18%, reduce-motion timings, only on the display moved to, clicks pass through: ported earlier, unchanged.

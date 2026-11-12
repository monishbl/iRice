# shell/modules/osd/DisplayOverlay.qml

Parity (12 Nov, written by the lead; not built or tested yet).

Rust: surfaces/screenfx/src/display.rs + ui/display.slint.
- Identify (2.5 s, number and name top left), Keep these display settings? with countdown, Go back / Keep settings: same.
- Fixed this round: Glass buttons are SettingsButton's size (28 high, 24 padding; were 32/28), weight 600 primary / 500 other, press 0.97, Tab focus with Space/Enter and keyboard-only ring.

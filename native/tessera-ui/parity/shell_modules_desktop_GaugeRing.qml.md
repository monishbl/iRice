# shell/modules/desktop/GaugeRing.qml → ui/desktop.slint `Ring`

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Track + arc from the top, round cap; Material secondary container track | 20-37 | two Paths | same |
| Device picture in the middle (78 %), else the icon (36 %) | 38-55 | `has-picture`, SymIcon | same |
| Picture loaded once, kept across refreshes | — | images cached by key; the gauges list kept per widget | fixed |

## Test plan

Geometry (1920x1080 IRICE-TEST, the user's 4 widgets): grid origin 32,54; a cell 164 px, gap 16; a cell (c,r) starts at x = 32 + 180c, y = 54 + 180r; s = 164x164, m = 344x164, l = 344x344. The user's layout: clock m at (0,0) → 32..376 x 54..218; stack m at (0,1) → 32..376 x 234..398; batteries m at (0,2) → 32..376 x 414..578; shortcuts m at (2,0) → 392..736 x 54..218.
Both looks: run once with appearance.look "glass" (the user's) and once "material" (dark).

1. Look at the batteries widget (32..376 x 414..578) with a Bluetooth device connected if the arena has none: at least "This computer" when the arena reports a battery. PASS: ring arcs from 12 o'clock clockwise, glyph/picture centred. Then run the RELOAD step of DesktopWidgets: the ring's picture never blinks.

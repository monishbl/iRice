# shell/modules/desktop/WorldClockWidget.qml → ui/desktop.slint `WorldW`; lib.rs `fill` "worldClock", `refresh_zones`

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Small: one face and name; medium: four faces, names, "Tomorrow, +9h" | all | `WorldW` | same |
| Zone times each minute | 18-30 | `refresh_zones` each minute | same |
| Faces turn instead of being made again | — | `Lists.clocks` | fixed |

## Test plan

Geometry (1920x1080 TESSERA-TEST, the user's 4 widgets): grid origin 32,54; a cell 164 px, gap 16; a cell (c,r) starts at x = 32 + 180c, y = 54 + 180r; s = 164x164, m = 344x164, l = 344x344. The user's layout: clock m at (0,0) → 32..376 x 54..218; stack m at (0,1) → 32..376 x 234..398; batteries m at (0,2) → 32..376 x 414..578; shortcuts m at (2,0) → 392..736 x 54..218.
Both looks: run once with appearance.look "glass" (the user's) and once "material" (dark).

1. Add World Clock Medium. Across a minute change (record 3 s from :59): all four minute hands turn ≈400 ms with overshoot; no face blinks.

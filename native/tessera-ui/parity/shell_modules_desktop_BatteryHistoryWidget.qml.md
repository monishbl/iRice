# shell/modules/desktop/BatteryHistoryWidget.qml → ui/desktop.slint `BatteryHistoryW`; lib.rs `fill` "batteryHistory"

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Level, charging glyph, 24 h graph | all | `runs` | same |
| Read every 5 minutes | 33-37 | `slow` 300 s | same |
| Runs in place | — | `Lists.runs` | fixed |

## Test plan

Geometry (1920x1080 TESSERA-TEST, the user's 4 widgets): grid origin 32,54; a cell 164 px, gap 16; a cell (c,r) starts at x = 32 + 180c, y = 54 + 180r; s = 164x164, m = 344x164, l = 344x344. The user's layout: clock m at (0,0) → 32..376 x 54..218; stack m at (0,1) → 32..376 x 234..398; batteries m at (0,2) → 32..376 x 414..578; shortcuts m at (2,0) → 392..736 x 54..218.
Both looks: run once with appearance.look "glass" (the user's) and once "material" (dark).

1. Add Battery History Medium (arena with a battery reading). PASS: level and graph like the QML; RELOAD step: no redraw on window events.

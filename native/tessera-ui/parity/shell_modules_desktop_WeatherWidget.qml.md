# shell/modules/desktop/WeatherWidget.qml → ui/desktop.slint `WeatherW`; lib.rs `fill` "weather"

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Small: place, temperature, condition, H/L; Material small a tertiary circle | all | `WeatherW` | same |
| Medium: hourly row (the next five 3-hour slots from the hour before now) | 145-168 | `hours` from core's Weather `hourly` | **tester: blank in Rust, 00…12 in QML** — the desktop draws whatever core's Weather gives; core filters `hourly` once when it fetches (after 21:00 that leaves none) and doesn't re-read for the new day: core owner |
| Material weather glyph filled | 33 | SymIcon fill | different: core's symbols font (FILL axis), core owner |
| Loading / error line | 61-80 | t5 | same |
| Live from Weather; hours change in place | — | feed → `Lists.hours` | fixed |

## Test plan

Geometry (1920x1080 TESSERA-TEST, the user's 4 widgets): grid origin 32,54; a cell 164 px, gap 16; a cell (c,r) starts at x = 32 + 180c, y = 54 + 180r; s = 164x164, m = 344x164, l = 344x344. The user's layout: clock m at (0,0) → 32..376 x 54..218; stack m at (0,1) → 32..376 x 234..398; batteries m at (0,2) → 32..376 x 414..578; shortcuts m at (2,0) → 392..736 x 54..218.
Both looks: run once with appearance.look "glass" (the user's) and once "material" (dark).

1. Scroll the stack to Weather (or add Weather Medium). PASS: matches the QML shot; RELOAD step: no redraw.

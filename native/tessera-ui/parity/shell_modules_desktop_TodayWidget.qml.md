# shell/modules/desktop/TodayWidget.qml → ui/desktop.slint `TodayW`; lib.rs `fill` "today"

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Greeting (click opens Today), date | 40-60 | `act("today")` | same |
| Weather glyph and temperature; large High/Low/rain | 66-92 | g1, t3, t4 | same |
| Today's coming events; large: open to-dos with ticks | 95-185 | `lines`, `lines2` | same |
| Re-read every minute | 25-30 | `M::Tick` | same |
| Rows in place | — | `Lists.lines`, `Lists.lines2` (keyed) | fixed |

## Test plan

Geometry (1920x1080 TESSERA-TEST, the user's 4 widgets): grid origin 32,54; a cell 164 px, gap 16; a cell (c,r) starts at x = 32 + 180c, y = 54 + 180r; s = 164x164, m = 344x164, l = 344x344. The user's layout: clock m at (0,0) → 32..376 x 54..218; stack m at (0,1) → 32..376 x 234..398; batteries m at (0,2) → 32..376 x 414..578; shortcuts m at (2,0) → 392..736 x 54..218.
Both looks: run once with appearance.look "glass" (the user's) and once "material" (dark).

1. Add Today Medium. Click the greeting: Today opens. Esc. RELOAD step: no redraw.

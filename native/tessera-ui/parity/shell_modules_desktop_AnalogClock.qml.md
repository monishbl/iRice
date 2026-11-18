# shell/modules/desktop/AnalogClock.qml → ui/desktop.slint `Analog`

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Glass: round face, light by day, dark from 18 to 6 | 60-69 | `face`, `night` | same |
| Material: cookie12 in primary container (day) / tertiary (night) | 70-77 | Path `cookie` | same |
| Ticks: 12 (Glass), quarter dots (Material) | 78-96 | `for i in 12` | same |
| Hands turn clockwise 400 ms OutBack each minute | 98-123 | `ah`/`am` animate 400 ms cubic-bezier(0.34,1.56,0.64,1); angles only grow | fixed: they now animate (the face used to be made again every minute, so the hands jumped) |
| Pivot | 124-132 | last Rectangle | same |

## Test plan

Geometry (1920x1080 TESSERA-TEST, the user's 4 widgets): grid origin 32,54; a cell 164 px, gap 16; a cell (c,r) starts at x = 32 + 180c, y = 54 + 180r; s = 164x164, m = 344x164, l = 344x344. The user's layout: clock m at (0,0) → 32..376 x 54..218; stack m at (0,1) → 32..376 x 234..398; batteries m at (0,2) → 32..376 x 414..578; shortcuts m at (2,0) → 392..736 x 54..218.
Both looks: run once with appearance.look "glass" (the user's) and once "material" (dark).

1. Resize the clock to Small (right-click 200 130 → Small). Record 3 s across a minute change (start at :59). PASS: the minute hand turns 6° clockwise with a small overshoot over ≈400 ms (`anim-timing.py` on the face region ≈ 400 ms); the hour hand 0.5°. FAIL if it jumps. Set it back to Medium.

# shell/modules/share/ProgressRing.qml → ui/share.slint `Ring`

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | Track: a full circle in the fill colour, `thickness` wide, radius w/2 − inset | 14-20 | first Path | same |
| 2 | Done part: accent, round caps, clockwise from 12 o'clock, 2π·value | 21-24 | second Path (ArcTo, large-arc past half) | same (value kept to 0.0001..0.9999 so a full ring still draws; at 0 nothing is drawn where the QML draws a cap-sized dot) |
| 3 | Repaints on each value, no animation between values | 10-11 | binding | same |
| 4 | Used by the sheet's devices 3/2 (Glass, while sending) and the incoming card 4/3 (Glass, receiving) | ShareSheet 291-300, IncomingCard 169-178 | `DeviceItem`, `ShareCard` | same |

## Test plan (tester)
Glass, arena 1920×1080, sandbox.
1. Sheet: `inbox 'ipc sharingdemo peers'`, open the sheet (ShareSheet plan), `inbox 'ipc sharingdemo sending 25'` → zoomed shot of device 1 (centre ≈ (812, 486)): accent arc from top to 3 o'clock, round ends, grey track. `sending 75` → to 9 o'clock. QML vs Rust shots (± 1 px stroke).
2. Card: `inbox 'ipc sharingdemo ask; ipc sharingdemo progress 50'` → 4 px ring half way round the 84 px avatar.
3. `inbox 'ipc sharingdemo sending 100'` → a full ring, no gap wider than 1 px.

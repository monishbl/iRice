# shell/modules/today/TodayButton.qml (Glass's buttons)

Rust: ui/today.slint `DayButton` (Glass branch).

| Behaviour | QML | Rust | Status |
|---|---|---|---|
| 36 high, text + 32, radius control | 356-358 | DayButton | same |
| Primary: accent, darker 1.1 hover / 1.2 pressed; secondary: fill / fillHover; colour on Theme.motion.hover | 359, 361 | background + animate 150 ms | same |
| Scale 0.97 pressed, NumberAnimation Theme.motion.press (linear) | 360, 362 | transform-scale | easing was ease-out → **fixed** linear |
| Enter / Space activate; Tab reaches it | 364-367 | key-pressed, FocusScope | same |
| Focus ring: 2 px accent, 4 px out | 369-377 | Ring | same |
| Icon 16 + text 14 medium, 6 apart | 379-396 | row | same |
| Click activates, pointing hand | 397-403 | TouchArea | same |

## Test plan (Glass)

1. Today open; hover Close (primary, bottom right): darker accent fading
   150 ms; press 300 ms: scales to 0.97 over 100 ms, linear.
2. Hover Options: fill → fillHover.
3. Tab to each button: a 2 px accent ring 4 px outside; Enter / Space
   activate.

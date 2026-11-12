# shell/modules/screentools/RecordCountdown.qml → surfaces/screentools (src/cards.rs `Countdown`, ui/screentools.slint `RecordCountdown`)

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | Shown while Recorder.countdown > 0 on the focused screen: a 120 disc with 24 margin (168 × 168) centred; clicks go through; no keyboard | 11-25 | `Countdown::set` (no anchors, empty input region) | same |
| 2 | Glass: a glass disc r 60 | 37-44 | glass shape r 60 / panel fill | same |
| 3 | Material: a 120 shape in primaryContainer, cookie9, sunny, cookie12, clover8, softBurst by second (count mod 5) | 45-56 | `SHAPES`, `shapes::outline` | known difference: the shapes switch without morphing, and "clover8" is drawn as core's "clover4" (core/src/surfaces/shapes.rs has no clover8) |
| 4 | The number: 56 px 600, label (Material onPrimaryContainer) | 57-63 | Text | same (no tabular figures) |
| 5 | Each second pops 0.82 → 1 over 220 ms, OutBack (overshoot 1.70158) | 31-36 | `pop_at` on a 16 ms timer | same |

## Test plan (tester)
Arena 1920×1080, both shells, both looks. Countdown to 3 s: `inbox 'ipc recorder options'`, click "3 s" in the Countdown row, Esc.
1. `rec cd 4 "inbox 'ipc recorder screen'"`. Without a recorder in the arena the countdown may not start: then mark this file **not testable here**.
2. Where it runs: the disc centred at (960, 540), 120 px; 3, 2, 1 a second apart; each number pops (0.82 → ≈ 1.03 → 1 within 220 ms): compare the number's height per frame, QML vs Rust (± 1 frame). Material: the shape changes each second (3 → clover, 2 → cookie12, 1 → sunny).
3. A click at (960, 540) during the countdown reaches the window under it.

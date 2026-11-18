# shell/modules/network/SpeedTestCard.qml → ui/network.slint `SpeedTestCard`, src/lib.rs `push_speed`

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | Card in the Wi-Fi menu: title, stage line, live figure, wavy/plain progress bar (width on 250 ms OutCubic) | 20-60, 130-145 | `SpeedTestCard`, `WaveBar` | same |
| 2 | Action button: "Test speed" / "Cancel" / "Test again"; Material 32 high, filled primary → tonal while running, 8 % / 10 % state layers; Glass 28 high, accent (darker 1.1 / 1.2) or fill while running | 62-95 | action | same |
| 3 | Material: corners round → 8 while pressed on springFast; press swell (ElasticControl) | 71-78 | `animate border-radius`, `transform-scale` | **was** 200 ms spring(0.3), fixed 1.06 on a 100 ms linear; now 222 ms spring(0.4), 1 + min(6 %, 10 px / width) on 290 ms spring(0.57) |
| 4 | Glass: shrinks to 97 % while pressed over `press` (100 ms) | 83-84 | `transform-scale` | same |
| 5 | Colour changes over `hover` | 85 | `animate background` | **was** hard-coded 150 ms; now `Theme.hover` |
| 6 | Ping / download / upload figures, the active one highlighted, "—" when none | 100-128 | `stats` | **was** a new model on every tick of the test (the three figures made again ~10×/s); now one `Rows`, set in place |
| 7 | Response and verdict lines wrap under the figures | 146-170 | `resp_h`, `verdict_h` | same |

## Test plan (tester)
Both looks. Open the Wi-Fi menu: `inbox 'ipc panels open wifiMenu TESSERA-TEST'` (the menu bar's Wi-Fi item). The card is at the bottom of the menu; the button is at its right edge.
1. Shot: button "Test speed" (Material filled primary 32 px pill; Glass accent 28 px).
2. Material: `P glide` onto the button → 8 % layer over 150 ms; `rec net-press 0.6 "P down"` → corners to 8 in ≈ 0.22 s with a slight overshoot, button swells ≈ 6 %; `P up`.
3. Glass: `P down` → 97 % in 100 ms, no overshoot.
4. If the arena has network: click it → "Cancel" (tonal), figures update live without flicker; record 2 s and check the stats row doesn't blink. Without network, mark the run "not testable here" (the error state "Test again" can still be checked).

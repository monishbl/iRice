# shell/modules/desktop/ClockWidget.qml → ui/desktop.slint `ClockW`; lib.rs `fill` "clock"

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Small: the analog face | 12-19 | `Analog` with `clocks[0]` | same |
| Medium: DAY (accent ink, bold), time 64 px DemiBold −2 tracking, date | 21-41 | `ClockW` | same |
| Material: the time's weight as Qt draws Tessera Sans Flex DemiBold (the tester saw light digits in QML) | 34 | **bold in Rust** → 400 in Material, 600 in Glass | fixed (batch 3; check against the QML shot) |
| Tabular figures | 34 | **Slint can't turn on the font's tnum** → the medium time is drawn a character a slot (`chars`, kept per widget), each digit centred in a slot as wide as a 0, so the time no longer shifts as digits change | fixed (batch 2) |
| Updates each minute | 10 | `M::Tick` each new minute | same; the faces now keep their element (they were made again each minute) |

## Test plan

Geometry (1920x1080 IRICE-TEST, the user's 4 widgets): grid origin 32,54; a cell 164 px, gap 16; a cell (c,r) starts at x = 32 + 180c, y = 54 + 180r; s = 164x164, m = 344x164, l = 344x344. The user's layout: clock m at (0,0) → 32..376 x 54..218; stack m at (0,1) → 32..376 x 234..398; batteries m at (0,2) → 32..376 x 414..578; shortcuts m at (2,0) → 392..736 x 54..218.
Both looks: run once with appearance.look "glass" (the user's) and once "material" (dark).

1. **Tabular digits**: record the clock widget (32..376 x 54..218) across a minute change where a 1 becomes another digit (e.g. 10:11 → 10:12). PASS: the other digits and the colon don't move sideways (compare frames before and after: x of the colon identical). QML: same.
2. Leave the desktop idle across a minute change, recording 3 s around it (start at :58). PASS: only the minute digits change; nothing else in the clock widget or other widgets redraws (`region-changes.py` over the other three widgets reports no change).
3. **No reload on window events**: `pointer move 1500 600`; start a 12 s recording; at +1 s `foot &`; at +3 s a second `foot &`; at +5 s focus the first (click it); at +7 s close one (`hyprctl dispatch 'hl.dsp.window.close({ window = "class:foot" })'`); at +9 s close the other. Then `python3 tools/region-changes.py rec.mp4 32 54 704 524` over the widgets (crop the windows' area out if they cover it: open the windows with `hyprctl dispatch movewindowpixel` to x ≥ 800, or use `tiling` off and check only the part of the region the windows don't cover). PASS: the widgets' pixels don't change when windows open, focus or close (only the clock's minute may change). FAIL if any widget fades/grows in again (the appear: opacity 0→1 260 ms, scale 0.9→1 420 ms) or blinks. Same in the QML pass for reference.

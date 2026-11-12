# shell/modules/osk/Keyboard.qml → surfaces/osk (src/lib.rs, keys.rs, vkbd.rs, ui/osk.slint)

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | Keyboard along the bottom; key unit min(58, (screen w − 120) / 15.6), keys unit × width − 6 | 66, 112-114 | `unit`, `Key` | same |
| 2 | Keys type through a virtual keyboard; Shift / Ctrl / Alt / Super latch; Return is the action key | 100-112 | `vkbd`, `keys` | same |
| 3 | Material colours: latched primary, Return primaryContainer, modifiers / wide keys secondaryContainer, else surfaceContainerHighest; 8 % / 10 % state layers | 109-123 | `m-base`, `m-ink` | same |
| 4 | Material corners 12 → round while pressed or latched, on springFast | 118-122 | `animate border-radius` | **was** 250 ms spring(0.6); now 222 ms spring(0.4) |
| 5 | Material press swell (ElasticControl); Glass 95 % while pressed over `press` (100 ms) | 115-126 | `transform-scale` | **was** fixed 1.04 on 250 ms spring(0.35), Glass ease-out; now 1 + min(6 %, 10 px / size) on 290 ms spring(0.57), Glass linear |
| 6 | Glass: accent when latched, fill pressed / hover, toggle-off for modifiers and wide keys | 123-124 | background | same |

## Test plan (tester)
Both looks. Open: `inbox 'ipc panels open keyboard IRICE-TEST'` (Search "On-Screen Keyboard" does the same). Focus a text field first (e.g. a foot window).
1. Shot: rows of keys at the bottom; with a 1920 px screen unit = 58 → letter keys 52 px.
2. Click `a` → "a" typed in the focused window. Click Shift (latches: Material primary, fully round; Glass accent), then `a` → "A", Shift unlatches.
3. Material: `rec osk-press 0.6 "P down"` on a letter → corners round in ≈ 0.22 s, ≈ 6 % swell with an overshoot; compare with QML ± 1 frame. Glass: 95 % in 100 ms.

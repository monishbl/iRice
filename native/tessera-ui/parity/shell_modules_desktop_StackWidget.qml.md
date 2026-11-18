# shell/modules/desktop/StackWidget.qml → ui/desktop.slint `Frame` (page, dots, scroll); lib.rs `stack_kinds`, `go`, `rotate`

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Kinds chosen for the stack that fit its size | 20 | `stack_kinds` | same |
| The next slides 24 px from below/above (300 ms OutCubic) and fades in (200 ms) | 84-108 | `changed page` → `page-y`, `page-o` | same |
| Dots: 5 px, current 12 px (200 ms), hover tint, click jumps | 117-150 | dots in `Frame` | same |
| Wheel/swipe: one flip per 120 angleDelta; a touchpad's small steps add up | 152-163 | **flipped on every scroll event (a swipe ran through every page)** → `wheel-acc`, one flip per 15 px | fixed |
| Smart Rotate (music → Now Playing; event < 30 min → Calendar; battery < 20 % → Batteries; not within 15 min of a manual flip), on change and every 5 min | 37-62 | `relevant`, `rotate` | same |
| Right-click: top widget's items, ✓ kinds, Smart Rotate | 64-82 | `widget_menu` | same |

## Test plan

Geometry (1920x1080 TESSERA-TEST, the user's 4 widgets): grid origin 32,54; a cell 164 px, gap 16; a cell (c,r) starts at x = 32 + 180c, y = 54 + 180r; s = 164x164, m = 344x164, l = 344x344. The user's layout: clock m at (0,0) → 32..376 x 54..218; stack m at (0,1) → 32..376 x 234..398; batteries m at (0,2) → 32..376 x 414..578; shortcuts m at (2,0) → 392..736 x 54..218.
Both looks: run once with appearance.look "glass" (the user's) and once "material" (dark).

1. Stack at 32..376 x 234..398; dots at x ≈ 366, centred vertically (≈316). `pointer move 200 316`, `scroll` down one notch. PASS: exactly one flip: the next widget slides up 24 px and fades in (300/200 ms); the current dot grows to 12 px. One notch up: back. Then 5 small scroll steps of 3 px (touchpad-like, if `dev pointer scroll` takes an amount) → one flip, not five.
2. Hover a dot (glide to 366, 300): it turns label/on-surface-variant. Click it: jumps there with the slide.
3. **No reload on window events**: `pointer move 1500 600`; start a 12 s recording; at +1 s `foot &`; at +3 s a second `foot &`; at +5 s focus the first (click it); at +7 s close one (`hyprctl dispatch 'hl.dsp.window.close({ window = "class:foot" })'`); at +9 s close the other. Then `python3 tools/region-changes.py rec.mp4 32 54 704 524` over the widgets (crop the windows' area out if they cover it: open the windows with `hyprctl dispatch movewindowpixel` to x ≥ 800, or use `tiling` off and check only the part of the region the windows don't cover). PASS: the widgets' pixels don't change when windows open, focus or close (only the clock's minute may change). FAIL if any widget fades/grows in again (the appear: opacity 0→1 260 ms, scale 0.9→1 420 ms) or blinks. Same in the QML pass for reference.

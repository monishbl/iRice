# shell/modules/today/TodayScreen.qml

Rust: surfaces/today/src/lib.rs (window, open/close, data), src/motion.rs
(the entrances and the exit as timelines, tested), ui/today.slint.

| Behaviour | QML | Rust | Status |
|---|---|---|---|
| Opens on the focused display (stays there) | 31-37 | output_for() (the shell names it; Hyprland's focused; the first) | same |
| Shell tells open/close, effect, options, motion scale, the accounts' items | 49-63 | message(): open / close / options / accounts | same |
| Window made while open, kept through the exit | 26-27, 78-80 | window kept (hidden) after the first open; `gone()` hides it after the exit | same (kept: reopening is instant) |
| Overlay layer, exclusive keyboard while open | 86-90 | Spec Overlay; keyboard Exclusive on open, None on close | same |
| Target: min(860, w−64) × min(620, h−96), centred, radius panel + 8 | 96-101 | motion::target | same (tested) |
| bloom: frost 380 OutCubic; after 140: opacity 320, rect 520, blur 520 OutCubic; rows after 120 more, 620 linear | 153-171 | motion::enter | same (tested) |
| shrink: frost 260; rect + radius 640 InOutCubic; rows after 420 | 172-184 | motion::enter | same |
| drop: 44 px drop falls 420 InQuad, ripple 700, rect 560 OutCubic, rows after 300 | 185-205 | motion::enter | same |
| Reduce motion: 200 ms linear fades | 148-152 | `still` | same |
| × CoolDown.motionScale | 94 | `motion` from the shell | same |
| Exit: rows 140, opacity/rect 240 (to 97 %, centred), frost 280, InCubic | 220-232 | motion::exit (from wherever it is) | same |
| Frost: your wallpaper's blurred backdrop, under a veil (Material: surface at 0.5/0.4; Glass: black 0.42 / white 0.3) | 235-249 | backdrop (cache's backdrop-<screen>.png, read off the UI thread) | same |
| Clicks and wheel outside the panel do nothing | 250-255 | frost TouchArea / scroll swallowed | same |
| The drop's ring: 44 + ripple × 0.9 min(w,h), border 1.5 + 10(1−r), blurred | 259-272 | ring | **different**: no blur on the ring (Slint has no blur); same size, width, colour |
| Panel glass (Glass) following the rectangle; a soft surface + hairline over it (not Material) | 275-301 | glass shapes in after_frame (sent only when the frame changes); overlay Rectangle | same; corners round, not squircles (**different**, at a glance the same) |
| Content laid out at the final size, clipped, centred in the growing panel | 303-330 | content Rectangle at tw×th | same |
| Content blur while condensing (MultiEffect blur) | 331-336 | blur → a fade of the content | **different** (Slint has no blur; the fade runs on the same curve) |
| Glass / Material content (TodayContent / MaterialTodayContent) by the look | 309-330 | GlassDay / MaterialDay | same |

## Test plan (both looks; both shells; 1920x1080)

Panel target: x 530, y 230, 860×620.
1. Record 1.6 s while `ipc today preview bloom`: frost fades in 380 ms,
   the panel condenses from 106 % from 140 ms (520 ms OutCubic), the rows
   rise one after another; settles ≈ 880 ms. Rust within ±40 ms.
2. Same for `preview shrink` (full screen glass draws in, 640 ms
   InOutCubic) and `preview drop` (44 px drop falls 420 ms, ring, opens).
3. Record 3 s with Today open and the pointer still: region-changes over
   the panel must show 0 changed frames (nothing reloads).
4. Escape: record the exit (rows 140 ms, panel 240 ms to 97 %, frost
   280 ms, InCubic). Clicking outside the panel while open does nothing.

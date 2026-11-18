# shell/modules/settings/pages/trackpad.qml ↔ surfaces/settings (src/pages/input/mouse.rs `trackpad`)

_Re-checked 17 Nov (B5, parity-settings) against main 18628a5 + the B5 ports: statuses below say what's there now; [12 Nov: …] is the old status._

The Trackpad part of Mouse & Trackpad (page `mouse`, PagePart "Trackpad").
No popups, timers or animations of its own.

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | Group "Point and click", footer "No trackpad found. These settings apply when one is connected." only with no touchpad (live Mouse.touchpads) (16–19) | mouse.rs 120, 135–136 (feed `input.touchpads`) | same |
| 2 | "Tracking speed" WordSlider 240, −1…1, Slow/Fast, round 0.05 (20–30) | 121–122, 137 | same |
| 3 | "Tap to click": Segmented 260 Off / Click / Click and drag; current from tapToClick/tapAndDrag; pick: i>0 → tapAndDrag = (i==2); tapToClick = i>0; detail by state (3 texts) (32–45) | 123–131, 138–147 | same |
| 4 | "Secondary click": Segmented 240 "Two fingers"/"Bottom right" ↔ twoFingers/corner; detail by value (46–56) | 132–134, 148–151 | same |
| 5 | Group "Scrolling": "Natural scrolling" + "Content follows your fingers"; "Scroll speed" WordSlider 0.2–3 round 0.05 (59–78) | 153–159 | same |
| 6 | "Three-finger drag" switch ↔ Keymap.threeFingerDrag (live), toggled → Keymap.setThreeFingerDrag(!checked); detail by state (80–90) | 161–165 (feed `keymap.threeFingerDrag`, ask `threeFingerDrag` → RustBridgeInput 67) | same |
| 7 | SubpageLink icon hand-swipe-right "Gestures", detail "Swipe and pinch with several fingers: spaces, All Windows, apps" → mouse/gestures (92–100) | 167–168 `Row::link` | same |
| 8 | More › "Point and click": "Drag lock" switch, **row disabled and the whole row (label, detail, switch) at opacity 0.5** unless tapToClick && tapAndDrag (107–113) | 180 `.switch(..).enabled(tap && drag)` | **same (fixed before 17 Nov: faded(): the whole row at 0.5)** [12 Nov: **different**: only the switch dims (0.45 Glass / M3 disabled colours); the label and detail stay at full strength (settings.slint RowView doesn't dim a disabled row)] |
| 9 | "Middle click" switch, detail by rightClick (corner → "Click both bottom corners together", else "Click or tap with three fingers") (114–118) | 181 | same |
| 10 | "Ignore the trackpad while typing" switch + "Stops a resting palm from moving the pointer" (119–123) | 182 | same |

## Test plan

Setups as in pages_mouse (both shells, both looks). `open mouse`, scroll to
the "Trackpad" heading (`run 'P move 1100 600; P scroll 0 15'`), `shot tp`.
Read y from the shot.

T1 Part: PASS: heading "Trackpad", groups Point and click (with the "No
trackpad found…" footer in the arena), Scrolling, Three-finger drag, the
Gestures link with its icon; the same in both shells.

T2 Tap to click: click "Off": detail "Press the trackpad down to click";
"Click and drag": detail "Tap with one finger to click; tap, then keep the
finger down…", config tapToClick true, tapAndDrag true.

T3 Drag lock dimming: with Tap to click "Click", open More options, shot the
Drag lock row. PASS (QML): label, detail and switch all at half strength, the
switch can't be clicked. Rust must dim the label/detail too. Switch to "Click
and drag": the row is at full strength and the switch works.

T4 Secondary click "Bottom right": detail "Click in the bottom-right corner";
More › Middle click detail "Click both bottom corners together".

T5 Three-finger drag: toggle it. PASS: switch and detail follow; the shell log
(`log`) shows Keymap.setThreeFingerDrag; toggling it back restores.

T6 Gestures link: click it → the Gestures page (title "Gestures").

## Fixed in batch 3 (fixer)

- Drag lock: the whole row at half when it can't be used (Row::faded).

## Test results

(tester)

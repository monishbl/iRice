# shell/modules/settings/Toast.qml ↔ settings.slint `toast` (1250–1303), keys (671); lib.rs show_toast (806–815), M::Undo / M::ToastOver (940–954), close (540–544); model.rs Ctx::toast

_Re-checked 17 Nov (B5, parity-settings) against main 18628a5 + the B5 ports: statuses below say what's there now; [12 Nov: …] is the old status._

A toast at the bottom of the Settings window: what changed, Undo for 8 s (5 s without).

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | show(text, undo): 8000 ms with Undo, 5000 without; a new one replaces the last, timer restarts (20–26) | lib.rs show_toast `toast_timer` SingleShot | same |
| 2 | Undo: clears it, hides, then runs it (31–37, 115) | M::Undo | same |
| 3 | Ctrl+Z undoes while it's up with an Undo (43–47) | settings.slint:671 keys `control && "z" && toast-shown && toast-undo` | same (as in the QML, a focused text field keeps Ctrl+Z) |
| 4 | Centred, 24 px above the window's bottom, non-modal, no focus, stays until the timer (51–58) | toast Rectangle x / y | same |
| 5 | Enter: opacity 0→1 200 ms **OutCubic**; Material rises 8→0 px with it (59–62) | settings.slint:1253 `animate p { 200ms; ease-out }`, y + (1−p)·8 | **same (fixed before 17 Nov: cubic-bezier(0.33, 1, 0.68, 1))** [12 Nov: **different** (easing: `ease-out` ≠ OutCubic)] |
| 6 | Exit: opacity →0 140 ms **InCubic**, no movement (63–65) | same `p` animation 140 ms `ease-in`; y uses (1−p)·8 so it also sinks 8 px | **same (fixed before 17 Nov: 140 ms cubic-bezier(0.32, 0, 0.67, 0), Material moves only on entry)** [12 Nov: **different** (easing `ease-in` ≠ InCubic; Material sinks 8 px on exit, QML doesn't)] |
| 7 | Background: Material inverseSurface radius 4; Glass #3A3A40 (dark, 1 px 16 % white ring) / #18181B, radius 14 (68–73) | toast background / border | same (QML Squircle; round rect) |
| 8 | Width: Material max(288, min(560, text + undo + 12 + 32)); Glass min(520, …) (75) | settings.slint:1256 `toast-row.preferred-width + 32px`, but toast-row already holds the 16 + 8/16 padding and 12 spacing | **same (fixed before 17 Nov: width from the row's own preferred width)** [12 Nov: **different** (32 px wider than the QML)] |
| 9 | Height 48 / 44 (76) | `height` | same |
| 10 | Message: left 16, right 12 to Undo (or 16), elided, inverseOnSurface / #EDEDEF / #FAFAFA, 14 px 0.25 tracking / body (79–86) | toast-row Text | same |
| 11 | Undo button: label+24 / +20 wide, 40 / 28 high, radius 20 / control, 8 px from the right (87–93) | undo Rectangle | same |
| 12 | Undo colour: inversePrimary 8 % hover / 10 % press (Material); white 12 % / 20 % (Glass), ColorAnimation 150 ms (94–99) | undo `background` | **same (fixed before 17 Nov: animate background 150 ms)** [12 Nov: **different** (no `animate background`)] |
| 13 | Undo press: Material ElasticControl swell, Glass scale 0.97 (96–98) | none | **same (fixed before 17 Nov: Undo: Material 1.06 / Glass 0.97 while pressed, spring back)** [12 Nov: **missing**] |
| 14 | Undo label 14 px 500 inversePrimary / small 600 #EDEDEF/#FAFAFA (103–109) | `ul` Text | same |
| 15 | Accessible AlertMessage with the text; Undo a button (77–78, 100–102) | role text + label; Undo role button + default action | same at a glance |
| 16 | Pointing-hand cursor on Undo (114) | `uta` pointer | same |
| 17 | Window closes → toast gone | lib.rs close stops timer, hides | same |

## Test plan

T1 Make a toast with Undo: `open about` → Login items (`open about/loginitems`, or the link on About), toggle one login item's switch (harmless; it's undone in T3). `shot t1-…` within 1 s. PASS (both looks, both shells): toast centred 24 px above the bottom, same width as the QML's for the same text (measure in the shots), Undo at the right.
T2 Timing: record `REGION="<toast area>"` from the toggle to its end. PASS: fades in 200 ms OutCubic (Material: rises 8 px), stays ~8 s, fades out 140 ms InCubic without moving.
T3 Undo: toggle again, then `run 'key -M ctrl z -m ctrl'` (`key` is wtype). PASS: toast goes, the switch flips back. Repeat clicking Undo, recording: hover fades ~150 ms; the button swells (Material) / shrinks to 0.97 (Glass) while pressed.
T4 Without Undo: a toast with no Undo (Restore defaults is NOT to be used; skip if none is safe). PASS: 5 s.

## Fixed in batch 3 (fixer)

- OutCubic in, InCubic out; Material rises 8 px only as it comes; its width counts its padding once; Undo's hover animates and it presses (Material swells, Glass 0.97).

## Test results

(tester)

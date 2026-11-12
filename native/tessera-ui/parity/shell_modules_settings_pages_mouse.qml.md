# shell/modules/settings/pages/mouse.qml ↔ surfaces/settings (src/pages/input/mouse.rs `mouse`)

Mouse & Trackpad (page `mouse`). Trackpad is a PagePart (trackpad.qml, its
own parity file). One popup: the "Pointer boundaries" Dropdown (Dropdown.qml →
Rust `Row::choice`, the Dropdown list of the Choice/notifications parity).
No timers, no animations of its own; every control is the kit's.

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | Group footer, live from Mouse.mice: none → "No mouse is connected. These settings apply when one is."; one → "Applies to <name>."; n → "Applies to n mice. Trackpads have their own settings." (23–25) | mouse.rs 11, 31–36 (feed `input.mice`) | same |
| 2 | "Tracking speed": WordSlider 240 wide, −1…1, words Slow / Fast, onMoved → speed = round(v·20)/20 (26–36) | 37 `.slider(-1,1,0).words().width(240)`, `cx.num(…, 0.05)` | same |
| 3 | "Shake to find the pointer" + detail; Segmented 280 Off / Bigger / Ring; now = off when findPointer.enabled is false or style "off", grow, else ring ("both" shows as Ring); pick: enabled = v ≠ off, style = v unless off (37–53) | 15–30, 38–41 | same |
| 4 | Group "Scrolling": "Natural scrolling" switch + "Content moves the way the wheel turns" (59–63) | 43–48 | same |
| 5 | "Scroll speed" WordSlider 0.2–3, Slow/Fast, round to 0.1 (64–74) | 45–49 `num(…, 0.1)` | same |
| 6 | Group "Buttons": "Primary button" + detail, Segmented 160 Left/Right ↔ leftHanded (77–90) | 51–53 | same |
| 7 | Group "Windows", footer "Also counts for trackpads.": "Focus", detail by value (click → "A window takes the keyboard…", else "Workspace changes focus…"), Segmented 260 "Follows pointer"/"On click" ↔ hover/click (92–101) | 55–61 | same |
| 8 | "Restore defaults" row, button "Restore Defaults": speed 0, accelProfile adaptive, naturalScroll off, scrollSpeed 1, leftHanded off, middleEmulation off, focus hover, boundaryMode corners, cornerSize 24 (103–122) | 63–74 | same |
| 9 | More options › "Pointer acceleration", detail by value (flat → "The pointer moves the same distance…", else "Faster movements carry the pointer further"), Segmented 160 On/Off ↔ adaptive/flat (125–133) | 77–85 | same |
| 10 | More › "Buttons": "Middle click with both buttons" switch + detail (134–142) | 87–89 | same |
| 11 | More › "Between displays", footer by Mouse.boundaryAvailable (two texts); "Pointer boundaries" Dropdown 260: Protect shared corners / Edge resistance / push again / Only during screen selection / Off ↔ corners/resistance/capture/off (143–155) | 91–105 `.choice(..).width(260)` | same (list = Dropdown parity) |
| 12 | "Corner reach" only while boundaryMode is corners; detail "N logical pixels from each corner"; WordSlider 4–100 Small/Large, round (156–161) | 106–111 | same |
| 13 | PagePart "Trackpad" (trackpad.qml) under the page (166–169) | 115 `p.part(cx, "Trackpad", trackpad)` | same |

Nothing missing or different here.

## Test plan

Setups: `tools/settings-scenarios.sh setup qml material`, then `setup rust
material <bin>`; the same with `glass`. Each: `open mouse`, `shot mouse-top`.
Material window 1040×700 at 440,205; the page column starts at x≈772. Read
each control's y from the shot.

M1 Page: shot the top, scroll (`run 'P move 1100 600; P scroll 0 10'`) and shot
again; click "More options" and shot to the end. PASS: same groups, titles,
texts, footers and order in both shells; Trackpad heading after More options.

M2 Footer: no mouse in the arena → "No mouse is connected. These settings apply
when one is." in both.

M3 Shake: `clicks shake "<Bigger x> <y>" "<Ring x> <y>" "<Off x> <y>"`. PASS:
config findPointer.enabled true/style grow, then ring, then enabled false
(style stays ring). Same in both shells.

M4 Focus: click "On click": detail changes to "A window takes the keyboard when
you click it, as on a Mac"; config mouse.focus "click".

M5 Sliders: drag Tracking speed to the right end: config mouse.speed 1 (steps
of 0.05); Scroll speed: steps of 0.1.

M6 Boundaries: More options, click the Pointer boundaries control, shot the
open list, pick "Off": the Corner reach row disappears; pick "Protect shared
corners": it is back with "24 logical pixels from each corner".

M7 Restore defaults: change speed, focus, boundary; click Restore Defaults.
PASS: every value of #8 back, both shells.

## Test results

(tester)

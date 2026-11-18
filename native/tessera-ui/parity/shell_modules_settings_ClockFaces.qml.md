# shell/modules/settings/ClockFaces.qml ↔ surfaces/settings (src/pages/look/lock.rs 57–89, ui/settings-look.slint `ClockFaces` 557–631, `Face` 401–553)

_Re-checked 17 Nov (B5, parity-settings) against main 18628a5 + the B5 ports: statuses below say what's there now; [12 Nov: …] is the old status._

Lock & Login Screen → Clock, Material look only: the lock clock's style
(lock.clockFace) from a horizontal carousel (a QML ListView), each face drawn
live over the wallpaper.

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | Material only (lock.qml loads it only there) | lock.rs 57 `if material` | same |
| 2 | Six faces: dynamic, stacked, wide, overlap, analog, weather with names (12–19) | lock.rs 10 `FACES` (tested) | same |
| 3 | Height 212; list 12 px in from top and bottom; spacing 8; item 176 wide; clipped (11, 22–30, 32) | slint 557–577: height 212, cards 176 × (212 − 24) at y 12, step 184 (176 + 8), clip | same |
| 4 | Chosen centred (StrictlyEnforceRange, preferredHighlightBegin/End around the middle); currentIndex = chosen (Math.max(0, findIndex)) (33–38) | x = (w − 176)/2 + (i − index)·184; `r.index` (lock.rs 71) | same |
| 5 | Moving to the chosen: highlightMoveDuration Theme.motion.panel (320 ms × pace) (36) | `animate x { 320ms; cubic-bezier(0.2,0,0,1) }` (577) | same at a glance (QML ListView's own highlight easing) |
| 6 | **Swipe/drag/flick the list (ListView, SnapToItem): the item it settles on becomes the face (onMovementEnded)** (37, 40) | — | **same (ported 17 Nov, B5: drag follows and settles on the nearest face (was there); a sideways scroll (trackpad, tilt wheel) now steps a face per 15 px, at most one per 120 ms. An up/down wheel scrolls the page, not the carousel: checked against the QML in a sandbox (arena 97, scroll 1 over the faces moved the page). Flick inertia isn't modelled: a fast drag settles on the face nearest where it was let go)** [12 Nov: **missing**: no drag, flick or wheel on the carousel; only click and ← →] |
| 7 | Keys: ← / → pick the previous / next face; activeFocusOnTab (41–43) | FocusScope key-pressed (565–571) | same |
| 8 | Accessible list "Clock style: <name>"; items list-items with name, selected (44–45, 114–116) | accessible-label "Clock style" (561); items 579–582 | same at a glance (list label lacks the face's name) |
| 9 | Neighbours: off = distance of the item's centre from the view's centre in item widths, **continuous while the list moves**; **the whole card (frame, face, name, ring) scaled** max(0.84, 1 − off·0.16); opacity max(0.55, 1 − off·0.45) (52–57) | off = abs(i − index) (575): jumps at once while x animates over 320 ms; only the frame rectangle shrinks (594–597): the Face and its name keep their full size | **same (fixed before 17 Nov: off follows the animated `shown`, the whole card scales)** [12 Nov: **different**: (a) scale and opacity jump instead of following the slide; (b) the face drawing and the name aren't scaled with the frame] |
| 10 | Frame: 6 px in (room for the ring), radius 28, surfaceContainerHighest, clipped; wallpaper cover (sourceSize 352×376) at 0.85; black 28 % veil (59–77) | slint 594–608; lock.rs 82–84 `cx.picture(&wall, 352, 376)` | same |
| 11 | The face: LockClock centred, −12 px; size analog 64, overlap 54, dynamic 50, else 58; weight clockWeight, width/round axes; colours primaryFixed, primaryFixedDim, **tertiaryFixed** (color3); time = now (78–92) | slint Face (size table, weight = r.step); lock.rs 67–81 (hours, minutes, AM/PM or date, analog minutes, weather) | **different (small, remaining): the third colour is m3 tertiary, not tertiaryFixed (no such theme token yet; adding one rebuilds every crate)** [12 Nov: different (documented): Slint can't set the font's width/roundness axes (plain text); **c3 is Theme.m3-tertiary, not tertiary-fixed** (408; the theme has no tertiary-fixed)] |
| 12 | Name at the bottom (14 px up), white, 13 Medium (93–98) | slint 614–620 | same (x centred by default) |
| 13 | Chosen ring: frame −4 → 3 px m3 primary, radius 32 (100–108) | slint 584–592 (x 2, radius 32, 3 px) | same |
| 14 | Click a card → that face (pointing hand) (109–113) | TouchArea 621–627 (also focuses the carousel) | same |

## Test plan

Setup: `tools/settings-scenarios.sh setup qml material`, then `setup rust
material <bin>` (Glass has no carousel: check it's absent with `setup … glass`).
`open lock`, `shot faces`. Material window 1040×700 at 440,205; the carousel is
the first row of the page column (from x≈772); read its y from the shot.

C1 Look: PASS: the chosen face centred and ringed, neighbours smaller and
fainter on both sides, the wallpaper behind each, names at the bottom. Compare
a neighbour's clock digits: QML scales the drawing to 84 %; Rust must too.

C2 Click: `clicks faces "<right neighbour x> <y>"` (recorded). PASS: the
carousel slides 184 px in ~320 ms; the new face is ringed; config
lock.clockFace changes. In the recording the old face shrinks/fades **during**
the slide in QML; Rust must match (not jump at the start).

C3 Keys: click a face, `run 'key -k Left'`: previous face chosen and centred;
Right: next. At the ends nothing moves.

C4 Drag: `run 'P move <centre x> <y> down; P move <centre x − 200> <y> glide 300; P up'`.
PASS (QML): the list follows the pointer, snaps to a card and that face becomes
the choice. Rust must do the same.

C5 Glass: `setup … glass`, `open lock`: no Clock group / carousel in either shell.

## Fixed in batch 3 (fixer)

- Not done: dragging/flicking the carousel, the neighbours' smooth shrink and fade, scaling the whole card, tertiary-fixed.

## Fixed in batch 4 (fixer)

- Batch 4: drag or flick the carousel: it follows, settles on the nearest face and chooses it; a click chooses the face under it; the neighbours shrink and fade continuously as it slides (from its position, 320 ms); the whole card scales (clock and name too); its accessible name says the face.
- Not done: tertiary-fixed for the third clock colour (the theme has no such colour).

## Test results

(tester)

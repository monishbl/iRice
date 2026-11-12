# shell/modules/launchpad/AppDrawer.qml (Material's app drawer)

Rust: surfaces/launchpad/src/lib.rs (state, keys, glide), ui/launchpad.slint
(drawing), src/model.rs (AppLibraryModel).

| Behaviour | QML | Rust | Status |
|---|---|---|---|
| Opens on springDefault (6.08/0.5, ε 0.004) | 52-60 | slint `animate progress`: 230 ms spring(0.01) (Qt's 16 ms steps fitted) | was 560 ms spring(0.4): slower, bouncier → **fixed** |
| Closes: panel × 0.6 = 192 ms, exit curve (0.3,0,0.8,0.15) | 61-69 | 192 ms × pace, same bezier | same |
| Layer stays through the close, input passes through | 71-77 | close(): input region empty, keyboard None; hide on `closed()` | same |
| Scrim m3.scrim × 0.32 × progress, click closes | 130-138 | scrim Rectangle, `dismissed` | same |
| Sheet: 1120×780 max, radius 28, veil(surface), opacity progress×1.6, scale 0.94→1 from the bottom, rises 96 px | 141-162 | sheet Rectangle | same |
| Clicks on the sheet don't reach the scrim | 165 | sheet TouchArea (also closes the FAB menu) | same |
| Search pill 56 high, radius 28, surfaceContainerHigh; magnifier 22 px; back button in a folder | 177-205 | searchbox | same |
| Back / Clear IconButtons: radius 20→8 pressed (150 ms OutCubic), state layer colour (hover duration), ElasticControl swell ×1.06 on springPress | IconButton.qml 19-24, 52 | mb / cb Rectangles | radius and colour snapped, no swell → **fixed** (animated; swell 235 ms spring(0.53)) |
| Tooltips "Back to categories" / "Clear search" | 203, 245 | accessible-label | same (IconButton's tooltip is only its accessible name; nothing is drawn) |
| Field: typing searches, selection reset on text change | 206-217 | `edited` → lib.set_query; push | same |
| Esc: clears search, then leaves folder, then closes | 218 | key(): back() | same |
| Enter: opens the selection | 219-220 | key(): activate | same |
| Tab: toggles the view | 221 | key(): toggle_view | same |
| ←/→ move (only with an empty field), ↑/↓ by a row; selection kept in view | 222-225, 124-127 | handles() + key(): step + reveal | same |
| Placeholder "Search N apps" | 226-234 | Text | same |
| Clear (×) only with text, refocuses | 236-250 | cb | same |
| Categories / All apps: connected button group 260×48; corners springFast | 254-268, Segmented.qml 124-134 | LpSegmented | corner spring was 200 ms spring(0.4) → **fixed** (springFast fitted: 150 ms spring(0.28)); the pressed segment widening (squeeze) is **not drawn** (segments keep their widths) |
| Folder title (name 24 px + "N apps") in a folder | 272-291 | HorizontalLayout | same |
| Grid: 116/212 px columns, cells 120/244 | 305-307 | cols / cell-w / cell-h | same |
| Scroll to selection on a spring (springDefault spring, damping 0.62, ε 0.5); a wheel/drag stops it | 311-340 | glide_step: Qt's discrete SpringAnimation (qt_spring) | was a 320 ms cubic → **fixed**; drag-stop: the glide timer is replaced on the next key, a wheel during a glide is overridden until it ends (≤ 0.46 s) |
| Touchpad momentum (Momentum.qml): each step moves the grid at once; speed smoothed v·0.4 + step/dt·0.6 (dt 4–100 ms); no step for 50 ms ends the run; faster than 120 px/s → Flickable.flick (deceleration 1500 px/s², at most 2500 px/s), stopping at the ends | 296, Momentum.qml 30-70 | wheel-area TouchArea scroll-event → wheel() / wheel_end() / glide_step's fling (fling_at, tested) | was Slint's own wheel handling (each step a 180 ms animation, no glide after lifting: this backend gives no touchpad phases) → **fixed**; a single step (a mouse wheel notch) doesn't glide; a scroll stops a keyboard glide (onMovementStarted) |
| Cell intro: each comes in on springDefault 16 + 14·min(i,24) ms after the page changes (first 41) | 352-369 | `intro` from animation-tick, critically damped ω = 2π/230 ms | was linear 260 ms, cut off at 500 ms → **fixed** (spring curve, runs to the end) |
| App tile 108², corner 20 → 36 when selected on springFast | 372-382 | tile Rectangle | was 250 ms spring(0.5) → **fixed** 150 ms spring(0.28) |
| Tile colour secondaryContainer when selected/lit, 150 ms; dimmed 0.4 when another letter is picked | 383-386 | same | same |
| State layer 0.08 hover / 0.1 pressed | 392-398 | state layer | same |
| Icon 56 px, ElasticControl swell 1.06 on springPress | 399-408 | Image transform-scale | was 300 ms spring(0.5) → **fixed** 235 ms spring(0.53) |
| Name 13 px, hidden with dock.appNames off; weight 500 selected | 409-419 | Text | same |
| Hover selects (onEntered) | 426 | has-hover → hovered(i) | was a full rebuild of every cell on each hover (the grid's model replaced: hover and press states, the corner/colour springs and the pictures were lost) → **fixed**: only `selected`/`picked` change; cells are one `Rows` |
| Left click opens, right click or press-and-hold: the app's menu at the pointer | 428-429 | at.pointer-event / clicked / Hold timer 800 ms | same (Qt's pressAndHold is 800 ms) |
| Folder card 188², corner 28 → 64 selected (springFast), colour 150 ms, swell 1 + min(0.06, 10/188) | 439-453 | folder Rectangle | swell was 1.04 → **fixed** 1.053; corner spring **fixed** as above |
| Folder 2×2 previews (60 px) | 466-478 | previews | each folder's previews now a kept `Rows` (were a new model each refresh) → **fixed** |
| Folder name 16 px (600 when selected) + count | 488-503 | HorizontalLayout | same |
| Letter scrubber (A–Z, long list), 28 wide, clear of the FAB (64) | 510-516 | scrub | same |
| Marked letter: primary circle, opacity hover-duration, scale 0.5 → 1 on springFast | 541-551 | lt Rectangle | scale missing → **fixed** |
| Click / drag along it jumps (picks the letter, leads its row) | 521-528, 561-569 | sc TouchArea → jump | same |
| No results: cookie9, 32 px magnifier, two lines | 573-606 | no-results | same |
| More: FabMenu (Get apps, Show A to Z / folders, settings) | 611-620 | FAB + fab items | same (items a kept `Rows`) |
| App menu (shortcuts, Open, Keep in / Remove from Dock), keys ↑↓ Enter Esc | 106-123 | WMenu, menu_for, key() | same (rows a kept `Rows`) |

## Test plan (Material look, user's settings; both shells; 1920x1080 IRICE-TEST)

Geometry (both shells should match): sheet x 400, y 150, 1120×780; search
pill x 424–1220, y 174–230; Categories/All apps group x 1236–1496, y 178–226.
Categories grid: 5 folder cards, card i centred at (531 + 214·i, 346).
A–Z grid: 8 columns, tile i centred at (488 + 129·(i mod 8), 308 + 120·row);
letter scrubber x 1468–1496.

1. Pointer at (1700, 900). Record 1.2 s while `ipc panels open launchpad
   IRICE-TEST`. anim-timing on it: QML springDefault rise (sheet 0.94→1
   from its bottom, 96 px up, opacity ×1.6), settles ≈ 270 ms after the
   first moving frame with no visible overshoot. Rust must start no later
   and settle within ±40 ms of QML.
2. Cells come in one after another (each fades in and rises 20 px, 14 ms
   apart): visible in the same recording; the last of the first 25 settles
   ≈ 600 ms after open in both.
3. Glide (vptr glide) from (531, 346) to (1387, 346) over 600 ms: each card
   in turn turns secondaryContainer and its corner rounds 28 → 64 on a quick
   spring (≈ 150–220 ms, a small overshoot); the card left behind goes back.
   Record 1 s; no other card flickers or reappears (region-changes on a
   card not hovered: 0 changed frames other than its own selection).
4. Press and hold (down, 300 ms, up elsewhere) on a card: it swells ≈ 5 %
   and springs back past its size on release (≈ 0.5 s bounce).
5. Key Tab: A–Z grid with the scrubber; the first tile is selected
   (corner 36). Key Down ×4 quickly (100 ms apart): the selection moves a
   row each time and the grid scrolls on a spring with no overshoot,
   settling ≈ 460 ms after the last key (record 1 s; match QML ±60 ms).
6. Type `fire`: results show, first selected; Escape clears the search
   (back to A–Z); Escape again closes (record: close 192 ms exit curve,
   both shells within ±30 ms).
7. Reopen, Tab back to Categories, Enter on the first folder: the folder
   title (name 24 px + "N apps") appears, back arrow in the pill, cells come
   in again. Hover the back arrow (452, 202): a state layer appears; press:
   corners square to 8 and it swells ≈ 6 %; click: back to the folders.
8. In a folder or A–Z, right-click a tile: the menu opens at the pointer
   ("Shortcuts" if any, "Actions": Open, Keep in Dock / Remove from Dock);
   Down/Up move, Escape closes the menu only.
9. A–Z: press on the scrubber (1482, 400) and drag down to (1482, 600): the
   letter under the pointer is picked (filled primary circle growing in
   from half on a quick spring), its apps lit, the rest dimmed to 40 %.
10. Hover a tile and wait: Rust must not redraw the other tiles (the
    pictures stay; no flash) when an icon finishes drawing.
11. Key Super+Ctrl+Space (arena: Alt+Ctrl+Space) toggles it open.
12. Touchpad fling: Tab to A–Z, pointer at (900, 500). Send a run of 8
    scroll steps of 30 px down, 10 ms apart (vptr scroll, or `wtype`-less:
    the pointer tool's scroll with small steps), then nothing; record 2 s.
    During the run the grid follows each step at once (no lag); after the
    last step it keeps gliding, slowing steadily (constant deceleration
    1500 px/s²: a run at ~2000 px/s glides ≈ 1.3 s and ≈ 1300 px, or stops
    at the bottom). QML (Momentum + Flickable.flick) and Rust: glide
    lengths within ±15 %, stop time within ±150 ms.
13. One isolated step (a single scroll of 30 px, then none for 1 s): the
    grid moves 30 px and stops dead in both shells.
14. During a keyboard glide (Down pressed then a scroll step within
    100 ms) the glide stops and the step applies.
Must also be checked in Glass: see AppLibraryGlass.qml's plan.

## Tester round 1 (desktop-batch1.md): failures and what was done

| Failure | Cause | Status |
|---|---|---|
| App icons in the Categories view are monochrome glyphs (Glass: Clear; Material: Themed circles) where QML shows each app's coloured Colloid logo on its squircle tile | The user's `appearance.iconStyle` is `"default"`. AppIcon.qml reads it as mode 0 (the app's own logo, tiled with iconShape squircle). core's `appicon::Style::from_settings` read `"default"` as the old name of `"look"` (Clear / Themed). The launchpad already draws every icon through the shared lookup and style (`wm::AppIcons`: Apps.iconFor → IconLookup, styled as AppIcon's default `styled: true`, in both views and in folder previews), so it follows core | **needs core's fix**: it is in ef9fd43 on `parity-int` ("iconStyle default is each app's own logo again"), which is **not** in this branch yet (3bbaaa3 doesn't carry it; core/src/appicon.rs here still maps default → look). Nothing to change in the launchpad; merge ef9fd43's appicon.rs and the tiles show the coloured logos |
| Category tiles "lay out differently" | Measured on the tester's shots (rust-/qml-{material,glass}-launchpad.png, crop 380,140–1300,500): card positions, sizes, corners, preview grid and labels match to the pixel in both looks; what differs is only the icons' style (Material's themed circles inside the card read as a different layout) and the accent (core's) | same once the icon style is fixed. One small difference seen: Material's selected folder name draws heavier in Rust (Slint's 600) than in QML (DemiBold rendered as the family's 500): left as the QML asks |

Motion tokens: `motion-kind` binds to core's Theme.snappy / Theme.expressive,
`pace` to Theme.snappy (0.85), and Material's press swells (icon, folder
card, back, clear) follow Theme.press-bounce (off with Reduce motion), as
ElasticControl's `pressBounce`.

### Retest
- With core's iconStyle fix merged: open Categories in both looks; every
  preview icon is the app's coloured logo on a squircle tile, tile by tile
  as QML (shot both, crop 380,140 → 1300,500, compare).
- Settings → Appearance → Reduce motion on (Material): pressing a tile or
  folder no longer swells (QML: no swell either).

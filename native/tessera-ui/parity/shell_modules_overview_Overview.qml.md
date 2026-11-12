# shell/modules/overview/Overview.qml → surfaces/overview

Rust: `native/tessera-ui/surfaces/overview/src/lib.rs` (open/close, layout via
`src/layout.rs`, one pointer tracker deciding hover/press/drag, menus, search),
`ui/overview.slint` (drawing). The user's settings: **overview.style = macos**,
displays = each, previews = snapshot, wallpapers = off, Glass expressive.
Test both styles (macos first: it's the user's).

## Behaviours

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | Opens through Panels ("overview"), on the focused screen, or on every screen with displays = each (keyboard on the one it opened on) | 37-63 | `open` (outs, key_out) | same |
| 2 | Open: refresh windows/monitors, picked = active space, field cleared + focused, results cleared | 228-241 | `open` | same |
| 3 | Open spring: SpringAnimation spring+0.6 / damping+0.08 (Glass expressive 3.6/0.30 → ω 15 rad/s ζ 0.63, ~8 % overshoot, settles ≈ 0.59 s; Material 6.68/0.58 → ω 20.4 ζ 0.89, settles ≈ 0.3 s) | 242 | `animate openness` | **was** 520 ms spring(0.4) both looks; now Material 307 ms spring(0.11), Glass 419 ms spring(0.37) (same ω, ζ) |
| 4 | Close: popover×0.7 (154 ms) InCubic; reopening mid-close retakes snapshots | 243, 222-227 | `animate openness`, `thumbs.retake` | same |
| 5 | Input only while shown; the closing fade lets clicks through | 245-249 | `close` → input region `[]` | same |
| 6 | Scrim: Material scrim × (mac 0.24 / grid 0.4), Glass black × (0.18 / 0.35), × progress; click on it closes | 403-411 | first Rectangle, `Press::Background` | same |
| 7 | Content opacity = progress, scale 0.97 + 0.03·progress; the field's and results' glass follow the zoom | 413-424 | `content`, `glass()` scales shapes by `content-scale` | same |
| 8 | Search field 560×52 (Glass glass r 26 / Material pill surfaceContainerHigh), magnifier 20 px bold, 20 px text, placeholder "Search" (mac) / "Search apps, or pick a space" | 426-489 | search Rectangle | same |
| 9 | Field y: mac round(h·0.36), grid 64 | 433 | `sy` | same |
| 10 | Typing: results = Search.query(text) minus AI rows, selection 0 | 470-473 | `edited` → `rank::query` | different: rows only QML Search has (files, conversions, calendar, contacts, App Store) and later-arriving answers (Search.revision, 286-297) aren't asked for |
| 11 | Escape: clears text, else closes | 474 | `key` | same |
| 12 | Return/Enter: runs the selected result (closing) or goes to the picked space | 475-476 | `key` → `run_result` / `go` | same |
| 13 | Left/Right with empty text: pick previous/next space (clamped, not wrapping) | 477-478 | `layout::step` | same |
| 14 | Up/Down: with text move the result selection; empty: grid ± columns, mac nothing | 479-480 | `key` | same |
| 15 | Results panel under the field (y = field bottom − 30), ≤ 420 px of rows, glass r 22 (Material panel radius); click a row runs it; selected row kept in view | 492-529 | results Rectangle, `select` | same |
| 16 | Grid hidden while results show (QML `visible`, delegates kept) | 34 (part 2) | **was** `if !mac && results.length == 0` (every cell made again when typing / clearing); now `visible:` | **fixed** |
| 17 | mac: glass bar at the top (Glass rgba .12/.12/.14/.6 dark or white .5, 1 px separator; Material surfaceContainer, bottom corners panel radius), slides in from −40 px | 538-557 | mac Rectangle | same (now kept, `visible: mac`) |
| 18 | mac: tile per space, tileW = min(stripH/aspect, 170, (w−220−(n−1)·24)/n), stripH 120 (rows: 48..96), 24 apart, centred; title under (small, 600 current / 400) | 597-660, 820-836 | `layout::mac`, tiles | same |
| 19 | mac tile hover scale 1.03, press 0.97 (not while dragged), hover duration (150 ms) | 700-701 | tile `transform-scale` 150 ms | same |
| 20 | mac tile: radius 16 M / 10 G, colours (current secondaryContainer), 1 px border Glass; wallpaper when overview.wallpapers | 702-716 | tile Rectangle | same |
| 21 | mac tile miniatures: each window at its place (min 4 px), r 6 M / 3 G, icon min(18, h·0.6); the dragged one at 40 % | 717-741 | `OvMini` | same |
| 22 | mac ring (3 px accent, −5/−3 margin) on the current space, the drop target, or a window drop under it | 743-751 | `t.current \|\| target \|\| hot` | same |
| 23 | mac: press on a miniature then move > 8 px: app icon (30 px on a 48 px chip, 2 px accent border) follows the pointer; release drops it on the space under the pointer | 763-803 | `Press::Tile` with window, proxy | same |
| 24 | mac: click a miniature → focus that window; click elsewhere on a tile → go to the space | 804-808 | `release` | same |
| 25 | mac: drag a tile (or its name) sideways > 8 px: it follows, others' nearest becomes the drop target, release reorders (Spaces.move); with zones it can leave the bar toward one | 658-693, 826-836 | `Press::Tile` without window | same |
| 26 | mac: right-click a tile → space menu at the pointer | 779-780 | `press` button 1 | same |
| 27 | mac: + per row (36 G / 40 M, right 28): hover fill / M state layer 8 %, press fillHover / 10 %, Material squares to r 12 while pressed (press 100 ms); click: close + new space | 842-875 (part 2 1-24) | pluses | same |
| 28 | grid: cells cellW = min(300, (w−160−side−(c−1)·24)/c), c = min(5, spaces) (rows mode: 6 and fit), aspect of the display, 24 apart, rows 32 apart, label per display in rows mode | 145-162, 861-901 | `layout::cell_w`, `layout::grid` | same |
| 29 | grid cell: Material surfaceContainer (current secondaryContainer) r tile; Glass #1A1A1D / #F4F4F5 r 14, border 2 px accent when picked / drop / target / edge-hot, current: softer (rows mode accent 55 %) | 930-946 | cell Rectangle | same |
| 30 | grid cell hover 1.02 / press 0.97 on a spring (spring+1, damping+0.04) | 947-949 | `animate transform-scale` | **was** 300 ms spring(0.36) both; now Material 299 ms spring(0.2) (ω 21 ζ 0.80), Glass 397 ms spring(0.49) (ω 15.8 ζ 0.51) |
| 31 | grid: hovering a cell (or its title, or a window in it) picks it | 1001, 1137, 1195 | `pointer` Press::None → `picked` | same |
| 32 | grid: "No windows" in an empty cell; guest badge (monitor icon, 24 px) for a space from an unplugged display | 952-993 | `c.guest`, Text | same |
| 33 | grid: window miniatures at their place (min 16×12), fullscreen fills inside the 1 px border, stacking tiled < floating (recent on top) < fullscreen | 1024-1046 | `place`, `layout::stack`, `is_full` | same |
| 34 | grid: a window's new place glides there (x/y/w/h 200 ms OutCubic) once settled (400 ms after opening); not while dragged | 1059-1079 | **was missing** (placed again at once); now `animate x, y, width, height` with `settled` Timer | **fixed** |
| 35 | grid: hover on a window: its outline in accent (WindowPreview `hovered`) | 1089 | `hover-win` | same |
| 36 | grid: click a window → focus it (its space first if not shown) | 1107-1113 | `focus_window` | same |
| 37 | grid: right-click a window → window menu ("Move to <display>", its other spaces up to 8, separators; "Only one display is connected") | 1110, 359-377 | `window_menu` on release | same |
| 38 | grid: drag a window > (DragHandler threshold ~10 px): it lifts (scale 1.04, 150 ms), stays whole on screen, drops on the space under the pointer (or an edge zone / chip / pushed edge); on its own space it springs back; hidden until it lands (1.5 s fallback) | 1115-1157, 1040-1101 | `Press::Window`, `drag_copy`, `gone_win`, `arm_ungone` | same (the lift is instant 1.04, QML animates 150 ms: see note) |
| 39 | grid: drag a space by its empty area or title > 10 px: follows, nearest cell in its row is the target; release reorders, or over a zone/edge moves the space to that display | 893-920, 1188-1210 | `Press::Space` | same |
| 40 | grid: right-click a space (or its title) → space menu | 1004-1006, 1198-1200 | `press` button 1 | same |
| 41 | Title "Space N · k windows", Glass white 95 % (picked/current) / 70 %, Material on a chip (secondaryContainer current, highest when picked) | 1173-1186 | title / Material chip | same |
| 42 | Material: the picked cell outlined 3 px primary over its windows, current (rows mode) 2 px secondary | 1163-1171 | Material outline | same |
| 43 | each: other displays as zones on the facing edge (glass pill, monitor icon + label); hover fill / M 8 %; click goes to that display's space | 1218-1300 | zones | same |
| 44 | zone held over by a dragged window 600 ms (with > 1 space): its space chips open; closed 350 ms after leaving; chip under the window turns accent (Material round, 100 ms) | 1260-1395 | `track_zone`, timers, chips | same |
| 45 | Context menu: opens at the pointer (upward when below 60 % of the height), keys Up/Down/Return/Escape, dismiss refocuses the field | 380-397 | `Menu`, `WMenu` | same (no ElasticGrab pull) |
| 46 | A space moved to another display stays hidden until it's gone (1.5 s) | 172-177, 338-344 | `space_gone`, `arm_ungone` | same |
| 47 | Lists keep their elements through every refresh (ScriptModel; "the zones' Repeater keeps its delegates") | 74, 120-124, 225 | **was** every refresh (each picture, each live frame, each Hyprland event) replacing every model: all cells, windows, tiles, zones made again; now `Rows` per list, keyed (cells/tiles by id, windows by address, zones by name, chips by id) | **fixed** |
| 48 | Pictures: snapshot (once per open), live (≤ 30/s), hybrid (live on current spaces); nothing captured while closed; dropped after the fade | 199-216 | `thumbs`, `set_live`, `closed` | same |

Note on 38: the QML's ScaleAnimator (150 ms) on the dragged miniature's 1.04 — Rust's drag copy is
a separate element made at the drag's start at 1.04 (no grow). Small; flag if the tester sees it.

## Test plan (tester)

IRICE-TEST is 1920×1080. Run QML then Rust (`--only qml|rust`), Glass (`--look glass`, the user's)
and Material. Prepare: `inbox 'ipc lock unlock; ipc panels close'`; open 3 foot windows on space 1
(`foot -T A`, `-T B`, `-T C`, 1 s apart), 1 on space 2 (`hyprctl dispatch 'hl.dsp.workspace(2)'`,
`foot -T D`, back to 1). Pointer parked at (100, 900). Open with `inbox 'ipc panels open overview IRICE-TEST'`;
close with `ipc panels close`.

### macos style (the user's: config overview.style = "macos", the default in the script's copy)

Geometry with 5 spaces (count 5): tiles 170×96, x0 = 487, 194 apart → tile centres
**(572, 74) (766, 74) (960, 74) (1154, 74) (1348, 74)**; titles at y ≈ 128; **+ at (1874, 74)**;
field 560×52 at (680, 389); bar height 166. (If fewer spaces show, recompute: x0 = (1920 − n·170 − (n−1)·24)/2.)

1. **Open timing** (T3): `rec ov-open 1.5 "ipc panels open overview IRICE-TEST"`, `anim-timing.py`:
   first moving frame after the command (Rust not later than QML), duration to settle,
   and look for the overshoot of the content scale (Glass ~8 %: the bar slides past and back a little).
   Compare frame-by-frame the bar's y over the first 400 ms: within 2 frames of QML.
2. **Close** (T4, T5): `rec ov-close 1 "ipc panels close"`: ≈ 154 ms fade; during the fade `P click 960 540`
   reaches the window below.
3. **Hover/press a tile** (T19): `P glide 766 74 300` → tile 2 grows to 1.03 over 150 ms (shot);
   `P down` → 0.97; `P up` → goes to space 2, overview closes.
4. **Click a miniature** (T24): reopen; space 1's tile has 3 miniatures; `P glide` to the centre of one
   (inside tile 1: x 487..657, y 26..122), click → that window focused, overview closed.
5. **Drag a window to another space** (T23): reopen; `P move` onto a miniature in tile 1, `P down`,
   `P glide 1154 74 600` (the 48 px icon chip follows from > 8 px), the target tile shows the 3 px ring
   while under it; `P up` → the window is on space 4 (hyprctl clients). Repeat with release over tile 1
   itself: nothing moves.
6. **Reorder a space** (T25): `P move 1154 74`, `P down`, `P glide 766 74 600` (tile 4 follows at 85 %
   opacity, tile 2 ringed), `P up` → order changes (QML and Rust same result). Also by the title: down at (1154, 128).
7. **Right-click a tile** (T26): `P move 960 74`, right click (`dev pointer … click right` or `down/up` with
   the right button as vptr allows) → menu at the pointer ("Only one display is connected" with one display).
   Keys: `wtype -k Down`, `-k Escape` → menu goes, typing works again in the field.
8. **+** (T27): `P glide 1874 74 300` → hover fill (Glass fill / Material 8 % layer); press → Material squares
   to r 12 over 100 ms; click → overview closes, a new empty space is made.
9. **Search** (T8–T15): reopen, `wtype fo` → results panel under the field; `-k Down` moves the selection;
   `-k Return` launches/focuses foot; reopen, type, `-k Escape` → text clears; `-k Escape` again closes.
   Rust: no file/conversion rows (T10, known).
10. **Background click** (T6): reopen, `P click 960 800` → closes; `P click 960 150` (on the bar) → stays.

### grid style (`use_config`: overview.style = "grid")

Geometry, 5 spaces: cells 300×168.75, x0 = 162, 324 apart → cell centres
**(312, 528) (636, 528) (960, 528) (1284, 528) (1608, 528)**, cell top ≈ 444; titles at y ≈ 621;
field at (680, 84) 560×52.

11. **Hover picks** (T30, T31): `P glide 636 528 400` → cell 2 ringed (accent 2 px Glass / primary 3 px Material)
    and grows to 1.02 on the spring: record (`rec grid-hover 1`), compare the scale curve QML vs Rust
    (Glass overshoots slightly, settles ≈ 0.4 s; Material ≈ 0.3 s).
12. **Arrows** (T13, T14): `wtype -k Right -k Right` → cell 3 picked; `-k Left` ×5 → stops at cell 1 (no wrap);
    `-k Return` → space 1, closed.
13. **Window hover/click** (T35, T36): reopen, glide onto a miniature in cell 1 → its outline turns accent
    (2 px Glass / 3 px Material); click → focused.
14. **Window drag** (T38): down on a miniature in cell 1, `P glide 1284 528 700` → it lifts (1.04), cell 4
    ringed; `P up` → the window moves to space 4 and appears there (it is hidden in between).
15. **Re-tile glide** (T34, the fix): reopen, wait 1 s, `inbox 'foot -T E &'` (a fourth window on space 1
    re-tiles the others) → record 1 s: the existing miniatures in cell 1 **glide** to their new places
    over ~200 ms (QML and Rust both); right after opening (< 400 ms) a change must jump, not glide.
16. **No rebuild on refresh** (T47, the fix): with previews = live (`use_config` overview.previews = "live"),
    open, hover cell 2 and keep the pointer still; record 2 s: the hover ring and scale stay steady
    (before the fix live frames rebuilt every cell: hover flickered and pictures blanked).
    `region-changes.py` over an empty cell (e.g. 1458..1758 × 444..612): no change frames.
17. **Typing hides the grid, clearing brings it back at once** (T16): type `f` → grid gone, results shown;
    Backspace → grid back with its pictures already there (no icon-then-picture flash).
18. **Space drag** (T39): down on cell 4's empty area (or its title at (1284, 621)), glide to (636, 528),
    up → reordered.

### each / zones (needs two displays: `hyprctl output create headless IRICE-B` in the arena, if the
arena allows; otherwise mark 43–46 "not testable here")

19. A zone pill shows on the edge facing the other display; drag a window onto it and hold 600 ms →
    its space chips open; over a chip → chip turns accent; release → window on that space.


## Round 2 (after panels-batch1 results)
| # | Finding | Cause | Fix |
|---|---------|-------|-----|
| R1 | Tile row 11 px lower (Glass; ~6 px Material), ring over "Desktop N"; grid cells ~8 px lower | the wallpaper Rectangle had no x/y, so Slint centred it in a parent that also holds the 6 px gap + 16 px title ((6 + 16) / 2 = 11 px) | wallpaper / cell pinned at x 0, y 0 (`overview.slint`, tiles and grid cells) |
| R2 | Search field without a caret on open | the field was focused before the window had the keyboard | once the keyboard is on the window, the field is focused out and in again (`refocus`, after_frame) |
| R3 | "fo": QML Foot, Foot Client; Rust Foot, Foot Server | equal scores kept the app list's order (QML: DesktopEntries' hash order) | ties broken by name in `core/src/rank.rs` (Spotlight too) |

Re-test: tile top y 26 in Glass macos with the title clear of the ring (shot); the caret right after open (shot); `wtype fo` ordering.

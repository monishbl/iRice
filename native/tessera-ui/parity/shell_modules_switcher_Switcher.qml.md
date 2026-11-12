# shell/modules/switcher/Switcher.qml → surfaces/switcher

Rust: `native/tessera-ui/surfaces/switcher/src/lib.rs` (phases, selection,
keys, layout), `ui/switcher.slint` (drawing, fade, tiles).

## Behaviours

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | Alt+Tab (tessera:switcher) begins: windows on regular spaces + minimized/aside, most recent first; scratchpads out | 51-76, 172-183 | `candidates`, `begin` | same |
| 2 | First selection: forward = the window behind the focused one (index 1 if list[0] is active), backward = last | 66 | `first_selected` | same |
| 3 | A popover open closes on begin | 72-73 | `begin` → `panels.closeOpen` | same |
| 4 | Layer mapped at once, transparent, no input region, exclusive keyboard (so Alt's release arrives) | 237-255 | `begin` (`set_input_region([])`, Exclusive) | same |
| 5 | Strip appears after 150 ms held (holdDelay) or at once on a second Tab | 29, 88-98, 146-150 | `hold` timer, `step` → `reveal` | same |
| 6 | Quick tap (Alt up before 150 ms): switches to the previous window, nothing drawn | 100-113 | `Released` → `commit`; `Hold` with Alt already up → `commit` | same |
| 7 | Tab / Right: next; Shift+Tab (Backtab) / Left: previous; wraps round | 277-284, 87 | `key`, `step` (rem_euclid) | same |
| 8 | Down / Up: ± one row (perRow) | 285-290 | `key` | same |
| 9 | Escape cancels (no switch) | 270-271 | `key` → `finish` | same |
| 10 | Return / Enter focuses the selection | 273-275 | `key` → `commit` | same |
| 11 | Release of Alt, AltGr, Super (either) commits; autorepeat ignored | 260-267 | `is_hold_key` (Alt, AltGr, Meta, MetaR) | **was different** (AltGr, right Super missing): fixed |
| 12 | Commit focuses 50 ms after the keyboard is given back; minimized/aside windows are shown (Apps.show) instead | 105-144 | `commit`, `later` 50 ms, `focus` | same |
| 13 | Click on a tile: select it and commit | 467-470 | TouchArea `picked(i)` → `Picked` | same |
| 14 | Tile press: scale 0.97, press duration (100 ms) OutCubic | 355-356 | `transform-scale`, 100 ms cubic-bezier(.33,1,.68,1) | same |
| 15 | Hover: Glass fillHover / Material onSurface 8 %, pressed 10 %; ColorAnimation press (100 ms) | 360-368 | tile background `ta.has-hover` | same |
| 16 | Selected (Glass): accent tint 22 % dark / 14 % light, 2 px accent ring, radius 14 | 363-366 | same | same |
| 17 | Selected (Material): 3 px primary outline 5 px off the card (radius card+5), chip turns secondaryContainer (animated 100 ms), title onSecondaryContainer | 406-417, 428-436, 449 | `if Theme.material && tile.current` | same |
| 18 | Title weight 600 when selected, 500 otherwise; fallback title → app name → class → "Window" | 443-451 | `push` title chain, Text weight | same |
| 19 | "Minimized" or the space's title at the right when not on this space | 452-459 | `place` | same (tnum font feature not available in Slint; figures look the same in the UI font) |
| 20 | Card: window aspect fitted in 192×120, min 32×24, radius 20 Material / 8 Glass, app icon 48 px until the picture arrives | 372-405 | `card()`, `has-picture` / `has-icon` | same |
| 21 | Pictures captured once when the strip shows, never live | 39, 95, 395-397 | `thumbs.ask` only in `Shown` | same |
| 22 | Grid: 208-wide cells, gap 8, pad 12, as many per row as the screen fits (−96), rows past the screen height scroll; selection kept in view (GridView.Contain) | 220-232, 323-341 | `grid`, `body`, `push` scroll | same |
| 23 | Fade in: opacity + scale 0.97→1, popover (220 ms) OutCubic; out: 0.65× (143 ms) InCubic | 243-250, 304-305 | `progress` animate, uses `Theme.popover` | same (was hard-coded 220/143; now follows Theme.popover) |
| 24 | No input while waiting or fading out (clicks go through) | 240 | input region `[]` except while shown | same |
| 25 | Empty: "No open windows", 240×72, secondary, body size, weight 500 | 314-321 | `body(0)`, Text | same |
| 26 | Another panel opening, Escape-all, or lock cancels | 152-168 | `close` command, `locked`, `keyboard_left` | same |
| 27 | Tiles keep their elements across Tab steps and picture arrivals (QML GridView delegates persist) | 323-342 | **was** a new VecModel on every `push` (every Tab and every picture rebuilt every tile: hover/press lost, pictures reloaded); now one `Rows` keyed by address | **fixed** |
| 28 | Window ready before the first Alt+Tab (QML keeps it 60 s after use; first use builds it) | 193-209 | made 1.5 s after start (`M::Prepare`), kept | better than QML |
| 29 | GridView Momentum flick when rows overflow | 326 | Slint Flickable's own deceleration | different (Slint can't take Momentum's curve; only with 30+ windows) |

## Test plan (tester)

Arena output IRICE-TEST is 1920×1080. Run both shells (`--only qml`, `--only rust`),
both looks (`--look glass`, `--look material`). Prepare: `inbox 'ipc lock unlock; ipc panels close'`,
then open three windows on space 1, one second apart, so focus order is C (newest, focused), B, A:
`inbox 'foot -T A >/dev/null 2>&1 & sleep 1; foot -T B >/dev/null 2>&1 & sleep 1; foot -T C >/dev/null 2>&1 &'`, wait 2 s.
Open a fourth `foot -T D` on space 2 (`hyprctl dispatch 'hl.dsp.workspace(2)'`, open, then back to 1), wait 2 s.
Move the pointer away first: `P move 100 900`.

Geometry with 4 windows (both looks): body is 4·216−8+24 = 880 wide, centred:
body left x = 520; tiles' centres x = 520+12+104+216·i → **636, 852, 1068, 1284**, centre y = **540**.

1. **Quick tap** (T1): `dev run wtype -M alt -k Tab -m alt` (no pause). Expect: nothing drawn
   (record 1 s: `rec tap 1 "wtype -M alt -k Tab -m alt"`, every frame free of the strip), focus moves to B
   (`hyprctl activewindow` title B). Tap again → back to C.
2. **Hold** (T5, T23): `rec hold 1.5 "wtype -M alt -k Tab -s 1200 -m alt"`. Expect the strip to start appearing
   ~150 ms after the Tab (no frame before 120 ms); `python3 tools/anim-timing.py` on the recording: fade-in
   ≈ 220 ms (both shells within 2 frames), scale 0.97→1 visible; selection on tile 2 (B: accent ring Glass /
   primary outline Material). After release: fade-out ≈ 143 ms, B focused. Compare QML vs Rust first-moving-frame
   latency after the key: Rust must not be later.
3. **Second Tab shows at once** (T5): `wtype -M alt -k Tab -s 40 -k Tab -s 800 -m alt`. Strip visible
   within ~1 frame of the second Tab (well before 150 ms), selection on tile 3; release focuses that window (A).
4. **Steps and wrap** (T7, T8): `wtype -M alt -k Tab -s 400 -k Tab -s 200 -k Tab -s 200 -k Tab -s 300` then
   `dev shot` → selection wrapped to tile 1. Then `-k Left` ×2 → tile 3; `-M shift -k Tab -m shift` → tile 2;
   `-k Right` → tile 3. Down/Up with one row: selection stays put (step ±4 wraps to itself). Finish with `-m alt`.
5. **Escape** (T9): hold Alt, Tab, wait 400 ms, `-k Escape`, wait 300 ms, `-m alt`. Strip fades, focus unchanged.
6. **Enter** (T10): hold Alt, Tab, wait 400 ms, `-k Return`. Strip goes and B is focused while Alt is still held.
7. **Hover / press / click** (T13–T15): hold the strip open (`wtype -M alt -k Tab -s 4000 -m alt &`), then within
   the 4 s: `P glide 1068 540 300` → tile 3 shows the hover fill (Glass: fillHover; Material: 8 % state layer);
   `shot hover`; `P down`, 50 ms, `shot press` → tile 3 at 97 % scale (Material: 10 % layer); `P up` → tile 3's
   window (B or A, whichever is 3rd) focused, strip fades out.
8. **Hover survives a Tab** (T27, the fix): strip open as in 7, `P glide 852 540 300`, then `wtype -k Tab`
   (Alt still held). Expect tile 2's hover fill to stay visible after the step (before the fix it vanished until
   the pointer moved). Also `region-changes.py` over a non-selected tile (x 1214, y 457, w 140, h 166) across the
   Tab: only the selection ring area changes; the picture must not blank and redraw.
9. **Elsewhere / minimized label** (T19): with D on space 2, the D tile shows its space title at the right of the
   title row ("2" or the space's name, as QML). Minimize A (`ipc apps minimize` or the decorations' minimize) and
   open the strip: A's tile says "Minimized"; selecting it brings A back to space 1.
10. **Pictures** (T20, T21): in shot 2 every tile has the window's picture; a window that has no capture shows its
    48 px app icon centred in a fill-coloured card (Colloid icon for foot). Pictures don't update live (type in a
    foot while the strip is held: its tile doesn't change).
11. **Empty** (T25): close all windows, hold Alt+Tab 600 ms: 240×72 panel, "No open windows" centred.
12. **Clicks pass through while pending** (T24): `wtype -M alt -k Tab -s 100 -m alt` while `P click` at 960 540
    lands on the window under it (not on the strip).
13. **Cancelled by another panel** (T26): hold the strip open, `inbox 'ipc panels open controlCenter IRICE-TEST'`:
    strip goes, no switch on release.
14. **First press after start** (T28): right after `dev reload` + 3 s, do step 2: the strip's first frame must not be
    later than on the second try.


## Round 2 (after panels-batch1 results)
| # | Finding | Status |
|---|---------|--------|
| R1 | Under wtype, a second Tab and Escape didn't reach the strip (QML did) | **open**: the code path is right (Tab / Escape → `key` → `step` / `finish`; the strip has the keyboard exclusively from the first Tab). Suspect: each `wtype` run is a new virtual keyboard with its own keymap. Check with a real keyboard, or with `TESSERA_DEBUG=1` see whether key / `debug: modifiers` lines arrive from the second wtype. |

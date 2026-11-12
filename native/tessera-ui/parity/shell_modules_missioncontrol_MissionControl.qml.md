# shell/modules/missioncontrol/MissionControl.qml → surfaces/missioncontrol

Rust: `native/tessera-ui/surfaces/missioncontrol/src/lib.rs` (open/close, layout `src/layout.rs`,
pointer tracker), `ui/missioncontrol.slint`.

## Behaviours

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | Opens through Panels ("missionControl") on its screen; exclusive keyboard while shown | 18-31, 74 | `open` | same |
| 2 | App Exposé: Panels.exposeApp when it opens (kept while it animates out); switching app while open | 40-54, 76-79 | `told` / `expose`, `message` | same |
| 3 | Windows: one app's from every space + minimized/aside (by class or entry id), or the current space's | 42-47 | `windows_for`, `of_app` | same |
| 4 | Progress: Theme.motion.panel (320 ms), emphasized bezier in / exit bezier out (Glass vs Material curves) | 56-64 | `animate progress` | same |
| 5 | Input only while shown | 66-71 | input region `[]` on close | same |
| 6 | Full-screen borderless glass (no bevel/refraction/rim) at progress; Glass: black veil 18 % × progress | 94-109 | `glass()` shape (rim 0), veil Rectangle | same |
| 7 | Escape closes; click on the background closes | 111-119 | `on_key` Escape, `Press::Background` | same |
| 8 | Exposé header: 40 px icon + name (20 px 600) + "N window(s)", y 56 sliding from −30, opacity progress | 121-146 | `if expose` header | same |
| 9 | Spaces bar: 150 px cards (screen aspect), 16 apart, centred, y 44 sliding from −30; radius 16 M / 8 G; colours; wallpaper if overview.wallpapers | 148-215 | `layout::cards`, cards | same |
| 10 | Card marked (current or drop target while dragging): Glass 2 px accent border (else 1 px), Material 3 px primary outline over its windows | 198, 212-213, 238-247 | `card.marked` | same |
| 11 | Card miniatures at their places, r 4 M / 3 G, icon min(20, h × 0.6) | 216-236 | `McMini` | same |
| 12 | Click a card: switch to that space (Mission Control stays open, showing it) | 266-275, 90-92 | `release` → `focus_workspace` (no close) | same |
| 13 | Drag a card > 8 px sideways: it follows (85 %), the nearest is the target, release reorders (Spaces.move) | 152-167, 249-277 | `Press::Space`, `drag-dx`, `drop_on` | same |
| 14 | Only the card takes the press; a click on a card's title falls through to the background (closes) | 249 (MouseArea in spaceCard) | `card_at` | **was** card + title (clicked title went to the space); now the card only, as QML |
| 15 | Title under each card: small, current white/600 (Material onSurface), others 70 % / 500 | 279-284 | Text | same |
| 16 | Stage: 48 px margins, top below the bar + 40; the grid that keeps the smallest window largest (gap 32, 28 under each), scale ≤ 0.9, short last row centred | 289-343 | `layout::stage`, `layout::fit`, `layout::tiles` | same |
| 17 | Stage opacity progress, scale 0.96 + 0.04·progress | 293-294 | stage Rectangle | same |
| 18 | Empty: "No windows on this desktop" / Exposé "No open windows", title size | 316-322 | Text | same |
| 19 | Window card: r tile M / 12 G, colour; hover scale 1.02 (hover 150 ms) and 3 px accent border | 347-354 | tile Rectangle | same |
| 20 | Pictures: once on open (snapshot), live/hybrid ≤ 30/s while open | 356-370 | `thumbs`, `set_live` | same |
| 21 | Glass: 40 px app icon centred under the card (16 px overlapping); hovered: title pill (24 px, r 12, dark 90 %) 30 px below | 373-397 | icon Image, title pill | same |
| 22 | Material: recents chip 10 px under the card (32 px, icon 20 + title), secondaryContainer when hovered (hover 150 ms) | 399-430 | chip | same |
| 23 | Click a window: close; minimized/aside → shown; on another space (Exposé) → its space then it | 81-89, 432-438 | `focus_window` | same |
| 24 | Lists keep their elements across refreshes (ScriptModel "keeps each window's live preview") | 217, 325 | **was** new models on every rebuild (every picture, live frame and Hyprland event made every card/window again); now `Rows` (cards by id, windows by address, miniatures by position) | **fixed** |

## Test plan (tester)

1920×1080, both shells, both looks. Prepare 3 foot windows on space 1 (A, B, C), one on space 2 (D).
Open: `inbox 'ipc panels open missionControl IRICE-TEST'`; close `ipc panels close`.

Geometry: cards 150×84, 166 apart, centred: with 5 spaces x0 = (1920 − 5·150 − 4·16)/2 = 553 →
card centres **(628, 86) (794, 86) (960, 86) (1126, 86) (1292, 86)**, titles at y ≈ 134.
Stage starts at y = 44 + 84 + 6 + 15 + 40 = 189, x 48..1872, to y 1032. With 3 windows the tiles are side by side
(read their centres off a shot; the middle one is near (960, 560)).

1. **Open/close timing** (T4): `rec mc-open 1.2 "ipc panels open missionControl IRICE-TEST"` and `rec mc-close`:
   320 ms each (emphasized in, exit out); bar slides 30 px down while fading; stage 0.96 → 1.
2. **Hover a window** (T19, T21, T22): `P glide 960 560 400` → card grows 1.02 over 150 ms with a 3 px accent border;
   Glass: title pill appears under it; Material: the chip turns secondaryContainer (150 ms). Glide off: back.
3. **Click a window** (T23): click it → focused, closed.
4. **Click a card** (T12): reopen, `P click 794 86` → space 2 shown, Mission Control **stays open** with D only.
5. **Click a title** (T14, the fix): reopen, `P click 794 134` (title under card 2) → Mission Control **closes**
   (as the QML: the title isn't part of the card's MouseArea).
6. **Drag a card** (T13): `P move 1126 86`, `P down`, `P glide 794 86 600` → card follows at 85 %, card 2 marked;
   `P up` → reordered (titles swap order). Small move (< 8 px) then up → just a click.
7. **Escape / background** (T7): reopen, `wtype -k Escape` → closes; reopen, `P click 960 1060` → closes.
8. **Exposé** (T2, T3, T8): `inbox 'ipc panels exposeApp foot'` (or however the QML pass opens it; dock right-click →
   Show All Windows) → header "foot" + "4 windows", all four foot windows (D from space 2 too); click D → space 2, D focused.
9. **Empty** (T18): switch to an empty space 3, open → "No windows on this desktop".
10. **No rebuild** (T24, the fix): previews = live; open, hover a window and hold still 2 s, record: no flicker of the
    hover border / title pill, the picture never blanks; `region-changes.py` over a card in the bar: no changes.


## Round 2 (after panels-batch1 results)
| # | Finding | Cause | Fix |
|---|---------|-------|-----|
| R1 | Card bar ~6 px lower | the card's picture had no y, so it was centred in a parent that also holds the title | pinned at y 0 (`missioncontrol.slint`) |
| R2 | Glass open 0.50–1.15 s / close 0.55–0.79 s vs QML 0.34 / 0.20 (Material equal) | not found by reading: the glass goes out async (one Hyprland batch per frame, as GlassPlugin.qml), same namespace and curve as the QML | **open**: re-measure with `TESSERA_DEBUG=1` (the log shows each `glass set`) to see whether the delay is before the first glass shape (map / first frame) or in the plugin's drawing |

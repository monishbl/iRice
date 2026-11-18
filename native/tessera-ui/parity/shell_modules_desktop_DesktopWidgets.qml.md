# shell/modules/desktop/DesktopWidgets.qml → surfaces/desktop/src/lib.rs (`update`, `read_layout`, `save`, `drag`, menus), grid.rs, ui/desktop.slint `Desktop`

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Grid from the screen: origin 32,54; cols/rows from cell 164 + gap 16, 110 px kept for the dock | 18-21 | grid.rs `Grid::new` | same |
| Unknown saved kinds kept when saving | 24, 45 | `unknown`, `grid::entries` | same |
| `placed`: a saved cell that doesn't fit moves to the nearest free one without rewriting the layout | 28-39 | `Grid::placed` | same |
| Editing only on the screen Edit Widgets was opened on | 40 | `feed.edit_screen` | same |
| Keyboard (OnDemand) while a note / large reminders / large calendar is placed | 42 | `want_keys` | same |
| add / remove / resize / moveTo (nearest free, rows weigh 1.1; full: island "No room on the desktop") | 77-112 | `add`, `grid::remove`, `Grid::resize`, `Grid::move_to`, host `noRoom` | same |
| Faint cells while editing or dragging, fading over Theme.motion.popover (220 ms) | 144-165 | `grid-shown` animate 220 ms | same |
| Snap ghost springs to its cell (Glass 5/0.4, Material springDefault 6.08/0.5) | 167-181 | `ghost-on`, animate x,y 400 ms spring(0.4 / 0.5) | same by eye (Slint's spring isn't Qt's SpringAnimation) |
| Right-click: the widget's menu (sizes with ✓, its own items, Widget Options…, Edit Widgets…, Remove Widget) or the desktop's (Edit Widgets…, Change Wallpaper…, ✓ Use Stacks, ✓ Show Files on Desktop, Desktop Settings…) | 185-233 | `widget_menu`, `desktop_menu`, `DeskMenu` | same |
| A click on the bare desktop ends editing | 189-193 | `M::Bare` → `endEdit` | same |
| The menu opens at the pointer, downward | 243-253 | `open_menu` | same |
| One slot per widget, changed in place: a move/resize keeps every delegate; adding/removing one inserts/removes just that one | 262-296 | `Rows<DeskWidget>` `set_keyed` by `kind:id#n`. **Each widget's own lists (lines, gauges, days, hours, clocks, stats, stocks, buttons, bars, apps, runs) were new models on every refresh, so every widget's insides were made again each minute, each feed line, each loaded picture** → `Lists` per widget key, a `Rows` each: a refresh changes only what changed | fixed |
| Nothing else closing touches the desktop | — | **`surface_closed` made the desktop window and every widget again when ANY part's surface closed (a panel, a popup, a window's decoration): "closing a window reloads all the widgets"** → only its own window or menu | fixed |
| "Pick widgets from the gallery below" on an empty desktop while editing | 299-308 | `hint` | same |
| The gallery's picks are added (Panels.addWidget) | 114-121 | `{"add":[kind,size]}` → `add` | same |

## Test plan

Geometry (1920x1080 TESSERA-TEST, the user's 4 widgets): grid origin 32,54; a cell 164 px, gap 16; a cell (c,r) starts at x = 32 + 180c, y = 54 + 180r; s = 164x164, m = 344x164, l = 344x344. The user's layout: clock m at (0,0) → 32..376 x 54..218; stack m at (0,1) → 32..376 x 234..398; batteries m at (0,2) → 32..376 x 414..578; shortcuts m at (2,0) → 392..736 x 54..218.
Both looks: run once with appearance.look "glass" (the user's) and once "material" (dark).

1. **No reload on window events**: `pointer move 1500 600`; start a 12 s recording; at +1 s `foot &`; at +3 s a second `foot &`; at +5 s focus the first (click it); at +7 s close one (`hyprctl dispatch 'hl.dsp.window.close({ window = "class:foot" })'`); at +9 s close the other. Then `python3 tools/region-changes.py rec.mp4 32 54 704 524` over the widgets (crop the windows' area out if they cover it: open the windows with `hyprctl dispatch movewindowpixel` to x ≥ 800, or use `tiling` off and check only the part of the region the windows don't cover). PASS: the widgets' pixels don't change when windows open, focus or close (only the clock's minute may change). FAIL if any widget fades/grows in again (the appear: opacity 0→1 260 ms, scale 0.9→1 420 ms) or blinks. Same in the QML pass for reference.
2. **Right-click a widget**: `pointer glide 200 300 300`, `pointer click right` (or down/up with the right button) on the stack widget. PASS: a menu opens at the pointer, downward, listing Small ✓/Medium/…, the stack's kinds with ✓, Smart Rotate, Widget Options…, Edit Widgets…, Remove Widget (red). Escape closes it (≈120 ms fade). Compare to the QML pass.
3. **Right-click the bare desktop** at 1200 400. PASS: Edit Widgets…, Change Wallpaper…, ✓ Use Stacks, Show Files on Desktop (✓), Desktop Settings…; opens at the pointer.
4. **Edit Widgets**: choose Edit Widgets… from that menu. PASS: faint cells fade in over ≈220 ms across the free grid; each widget gets a round − badge at its top-left that springs in (overshoots slightly); the gallery opens at the bottom. Click the bare desktop at 1200 400: cells fade out, badges vanish at once.
5. **Drag a widget**: with or without editing, `pointer move 600 130` (shortcuts), `down`, `glide 600 330 400`, wait 0.5 s, `glide 1000 330 400`, `up`. PASS: after 6 px the widget follows the pointer, grows to 1.04 (150 ms), a ghost (Glass white 10 %, Material primary container) springs from cell to cell; on release it springs into the ghost's cell (≈450 ms, small overshoot), the layout is saved (config.json desktop.widgets has shortcuts at its new cell). Drag it back the same way.
6. **Resize**: right-click the batteries widget (200 500) → Small. PASS: it shrinks to 164x164 in place; the others don't fade/grow in again. Set it back to Medium.

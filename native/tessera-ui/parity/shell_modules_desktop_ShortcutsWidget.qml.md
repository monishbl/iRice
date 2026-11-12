# shell/modules/desktop/ShortcutsWidget.qml → ui/desktop.slint `ShortcutsW`; lib.rs `fill` "shortcuts"

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| 4×2 (medium) buttons from Spotlight's actions, chosen per widget | 33-39 | `buttons` | same |
| Glass: tints at 0.18 / hover 0.26 / pressed 0.32; press 0.95 over 100 ms | 57-60 | `b.tint.with-alpha`, scale 0.95 100 ms | same |
| Material: tonal containers, state layer 0.08/0.10, corners 20→12 pressed (springFast) | 48-57 | `bg.mix(ink)`, radius 200 ms spring | same |
| Material press: swells by pressGrow (≤ 6 %, ≤ 10 px) and springs back past its size (ζ 0.3); none with Reduce motion (pressBounce) | 54-55 | **was 1.04 over 100 ms, no spring** → 1 + min(0.06, 10px/size), 420 ms spring(0.3), only while core's `Theme.press-bounce` | fixed |
| Glyphs filled (Material Symbols FILL 1 / Phosphor-Fill) | 64-69 | SymIcon `fill: true`: Glass Phosphor-Fill; Material outlined (the FILL axis) | different (Material): core's font; needs a static filled instance of the symbols font (core owner) |
| Click runs the action (App Drawer opens the launchpad) | 82-89 | **tester (batch 1): a click did nothing** → tiles are `DragTap`s: the click is taken on release over the tile (pointer-event, not `clicked`) and logged (`tessera-ui: desktop shortcut "App Drawer"`) → `search.action` → `panels.show launchpad` | fixed (batch 3) |
| Press on a tile and drag moves the whole widget (the frame's DragHandler takes over after 6 px); right-click on a tile is the widget's menu | DragHandler / MouseArea left only | **tester: dragging from a tile did nothing** → `DragTap` hands the drag to the frame (`drag-by` → Frame, the same drag as its own) and a right-click to the frame's menu; the same for Photos, Calendar (small and its days), Today's greeting and Now Playing's buttons | fixed (batch 3) |
| Right-click: ✓ each action | menuItems | `widget_menu` | same |

## Test plan

Geometry (1920x1080 IRICE-TEST, the user's 4 widgets): grid origin 32,54; a cell 164 px, gap 16; a cell (c,r) starts at x = 32 + 180c, y = 54 + 180r; s = 164x164, m = 344x164, l = 344x344. The user's layout: clock m at (0,0) → 32..376 x 54..218; stack m at (0,1) → 32..376 x 234..398; batteries m at (0,2) → 32..376 x 414..578; shortcuts m at (2,0) → 392..736 x 54..218.
Both looks: run once with appearance.look "glass" (the user's) and once "material" (dark).

1. **Click** (tester's failure): click the App Drawer tile (the user's: 604,175; find it by its label). PASS: the launchpad opens (as QML); the log has `tessera-ui: desktop shortcut "App Drawer"`. Esc.
2. **Drag from a tile** (tester's failure): Material, `pointer move 600 130`, `down`, `glide 600 330 400`, `glide 1400 330 400`, `up`. PASS: the whole Shortcuts widget follows from the first 6 px (grows 1.04), a ghost shows the cell, on release it springs into it and config desktop.widgets has shortcuts at its new cell (QML: shortcuts:m:6:1). No action runs on that release. Drag it back.
3. **Right-click a tile** (600,130): the Shortcuts widget's menu opens (sizes, ✓ actions, Edit Widgets…, Remove Widget).
4. Hover each button of the shortcuts widget: `pointer glide 440 100 300`, then 520 100, 600 100, 680 100, 440 175 … PASS: the hovered one brightens (Glass 0.18→0.26 tint, 150 ms; Material state layer). Press and hold 440 100 for 0.5 s then release: Glass shrinks to 0.95 in 100 ms; Material grows ≈4 % with its corners tightening 20→12 and bounces back past 1 on release. Record and compare with the QML pass.
5. Click one with a harmless action (e.g. one that opens a panel), check it ran; Escape.
6. **No reload on window events**: `pointer move 1500 600`; start a 12 s recording; at +1 s `foot &`; at +3 s a second `foot &`; at +5 s focus the first (click it); at +7 s close one (`hyprctl dispatch 'hl.dsp.window.close({ window = "class:foot" })'`); at +9 s close the other. Then `python3 tools/region-changes.py rec.mp4 32 54 704 524` over the widgets (crop the windows' area out if they cover it: open the windows with `hyprctl dispatch movewindowpixel` to x ≥ 800, or use `tiling` off and check only the part of the region the windows don't cover). PASS: the widgets' pixels don't change when windows open, focus or close (only the clock's minute may change). FAIL if any widget fades/grows in again (the appear: opacity 0→1 260 ms, scale 0.9→1 420 ms) or blinks. Same in the QML pass for reference.

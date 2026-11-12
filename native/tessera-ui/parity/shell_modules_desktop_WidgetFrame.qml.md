# shell/modules/desktop/WidgetFrame.qml → ui/desktop.slint `Frame`; lib.rs `drag`, `after_frame` (glass)

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Appear: opacity 0→1 260 ms OutCubic, content 0.9→1 420 ms OutBack, once when made | 64-71 | `shown`, `grow` in `init` | same (now only when the widget is really new) |
| Glass under it (radius tile 36, liquid) | 74-84 | `after_frame` glass shapes | same |
| Material: tonal card per kind, pill where the def says | 85-98 | `material_card`, `has-card`, `pill` | same |
| Padding 16 (Glass) / the card's pad and padX (Material) / none for bleeding kinds | 106-120 | `pad`, `pad-x` | same |
| Drag after 6 px; above the others; live snap preview | 123-136 | TouchArea `moved`, `z`, `drag(…, true)` | same |
| Dragged: scale 1.04 over Theme.motion.hover | 48-49 | `lift` animate Theme.hover (was one 150 ms animation over the whole transform: the appear and the lift fought) | fixed |
| Springs back / into the new cell (Glass 5/0.38, Material springDefault) | 35-37 | animate x,y 450 ms spring(0.38 / 0.5) | same by eye |
| Lock/unlock: zooms toward/away from the screen's centre (≤ 1.22×), fades, outer widgets later (LockFx) | 51-62 | **was missing** → `fx`, `fx-d`, `fx-p`, `fx-s` in `Frame`; the host's lock bridge now tells the desktop its `settle` steps; the glass under each widget zooms and fades with it | fixed |
| Remove badge at −8,−8, 24 px, springs in (5/0.35) when editing; click removes | 138-164 | **popped in with no spring** → `badge` animate spring(0.35) | fixed |
| Glass's glass scales with the content while it grows in | 97 | the glass shape is the cell's | different: the plugin's glass stays cell-sized during the 0.9→1 grow (hard to see) |

## Test plan

Geometry (1920x1080 IRICE-TEST, the user's 4 widgets): grid origin 32,54; a cell 164 px, gap 16; a cell (c,r) starts at x = 32 + 180c, y = 54 + 180r; s = 164x164, m = 344x164, l = 344x344. The user's layout: clock m at (0,0) → 32..376 x 54..218; stack m at (0,1) → 32..376 x 234..398; batteries m at (0,2) → 32..376 x 414..578; shortcuts m at (2,0) → 392..736 x 54..218.
Both looks: run once with appearance.look "glass" (the user's) and once "material" (dark).

1. **No reload on window events**: `pointer move 1500 600`; start a 12 s recording; at +1 s `foot &`; at +3 s a second `foot &`; at +5 s focus the first (click it); at +7 s close one (`hyprctl dispatch 'hl.dsp.window.close({ window = "class:foot" })'`); at +9 s close the other. Then `python3 tools/region-changes.py rec.mp4 32 54 704 524` over the widgets (crop the windows' area out if they cover it: open the windows with `hyprctl dispatch movewindowpixel` to x ≥ 800, or use `tiling` off and check only the part of the region the windows don't cover). PASS: the widgets' pixels don't change when windows open, focus or close (only the clock's minute may change). FAIL if any widget fades/grows in again (the appear: opacity 0→1 260 ms, scale 0.9→1 420 ms) or blinks. Same in the QML pass for reference.
2. **Lock and unlock** (Settings → Lock Screen's transition on, zoom): `ipc lock lock`, wait 2 s, `ipc lock unlock`; record 2 s from the unlock. PASS: the widgets start ≈1.2× larger and see-through, zoom toward their cells and fade in; widgets further from the screen centre start later (the batteries widget after the clock); on Glass the glass under each widget zooms with it; whole thing ≈900 ms (Glass) / 800 ms (Material), ease-out. Compare timings with `tools/anim-timing.py` against the QML pass.
3. **Badge spring**: right-click the desktop → Edit Widgets…; record 1 s. PASS: each badge grows from 0 with a small overshoot (≈400 ms). Click a badge on the batteries widget (32-8+12, 414-8+12 = 36,418): that widget leaves; the others stay put without fading in again. Undo by re-adding Batteries Medium from the gallery (or restore config).
4. **Drag lift**: press on the clock (200 130), glide 40 px right in 200 ms, hold. PASS: grows to 1.04 in ≈150 ms; release springs back.

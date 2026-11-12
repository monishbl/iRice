# shell/modules/launchpad/AppLibraryGlass.qml (Glass's App Library)

Rust: surfaces/launchpad (lib.rs, ui/launchpad.slint, model.rs). The dialog
itself is glass/Dialog.qml.

| Behaviour | QML | Rust | Status |
|---|---|---|---|
| Dialog opens on Theme.motion.spring + 0.4 / damping + 0.08 (ε 0.004) | Dialog.qml 25-33 | `animate progress`, per motion setting: expressive 345 ms spring(0.25), standard 285 ms spring(0.02), snappy 240 ms spring(0.0) (Qt's steps fitted) | was 480 ms spring(0.4) for every setting → **fixed** |
| Closes: panel × 0.6, exit (0.4,0,1,1) | Dialog.qml 34-41 | 192 ms × pace | same |
| Scrim black 0.25 × progress; click closes | Dialog.qml 58-66 | scrim | same |
| Body opacity min(1,p), scale 0.96 + 0.04p, 40 px above centre | Dialog.qml 69-77 | gx/gy/gscale | same; glass shapes follow the scale (glass()) |
| Elastic pull of the dialog (ElasticGrab) | Dialog.qml 43-46 | — | **not drawn** (the dialog doesn't follow a press-drag) |
| Esc closes / Tab toggles view | Dialog.qml 77-78, 43 | key() | same |
| Size min(1120, w−160) × min(720, h−200), padding 24 | 27-29 | gw/gh | same |
| Reset on open (no search, no folder, first selected, field focused) | 36-42 | open(): lib.reset, invoke_open | same |
| Back button + folder name in a folder | 76-93 | header | back button had no press squish nor colour fade → **fixed** (ElasticControl squish by motion setting, spring fitted; Glass IconButton colour Theme.motion.hover); rubber band follow not drawn |
| Search 320×36, fill, accent border when focused, magnifier 16 | 95-110 | searchbox | same |
| Field keys: Esc, Enter, ←/→ when empty, ↑/↓ by columns, Menu / Shift+F10 menu | 111-132 | handles()/key() | same |
| Placeholder "Search N apps" | 133-139 | Text | same |
| Segmented "Categories / All Apps" 200 wide, sliding pill 220 ms emphasized | 143-153 | LpSegmented | same |
| Grid: 120/210 columns, cells 116/236 | 161-163 | cols/cell-h | same |
| Index jump: contentY on OutCubic, Theme.motion.panel | 172-184 | glide_step (320 ms × pace, OutCubic) | same (pace for snappy added) |
| Touchpad momentum (Momentum) | 159 | wheel() / wheel_end() fling (see AppDrawer) | was no glide after lifting → **fixed** |
| App cell 104², accent tint selected/lit, fill on hover (no animation) | 197-205 | tile | colour faded 150 ms → **fixed** (instant, as the Squircle has no Behavior) |
| Dim 0.4 when another letter picked (hover duration) | 204-205 | opacity | same |
| Icon 56, scale 0.94 pressed (ScaleAnimator, press 100 ms, linear) | 206-213 | Image | was ease-out → **fixed** linear, × pace |
| Name small, elided | 214-222 | Text | same |
| Hover selects; click opens; right-click / press-and-hold menu | 223-232 | at | hover rebuilt every cell → **fixed** (Rows; only `selected` changes) |
| Folder card 188², platter / fillHover, accent 2 px border selected, scale 0.97 pressed | 240-249 | folder Rectangle | colour faded → **fixed** (instant); press linear |
| Folder icons: three 64 px + a cluster of four 27 px, or up to four 64 px | 252-281 | previews | same; previews kept per folder (Rows) → **fixed** |
| Folder label "Name  N" | 290-295 | Text | same |
| Letter index: visible when A–Z and longer than the view; letter in view bold accentInk; hovered letter gets a fill (hover duration); click/drag jumps | 303-368 | scrub | same |
| No results: magnifier 32, title, hint | 371-393 | no-results | same |
| App menu at the pointer (ContextMenu, glass) | 47-68 | WMenu + its glass shape | same |

## Test plan (Glass look, user's motion setting; both shells; 1920x1080)

Geometry: dialog box x 376, y 116, 1168×768 (content x 400–1520, y 140);
search x 800–1120, y 140–176; segmented x 1320–1520, y 143–173.
Categories: 5 cards, card i centred at (512 + 224·i, 294). A–Z: 9 columns,
tile i centred at (460 + 120·(i mod 9), 252 + 116·row); scrubber x 1496–1520.

1. Record 1.2 s while `ipc panels open launchpad IRICE-TEST`: scale 0.96→1
   and fade on Dialog's spring. With motion "expressive" QML settles
   ≈ 450 ms with a slight overshoot; "standard" ≈ 290 ms; "snappy"
   ≈ 380 ms (no overshoot). Rust within ±50 ms, starting no later.
2. Glide (512, 294) → (1408, 294) over 600 ms: each card gets fillHover and
   a 2 px accent border at once (no fade), the previous one loses it.
3. Press a card 300 ms: scales to 0.97 (100 ms linear) and back.
4. Tab → A–Z; glide along the first row: the tile under the pointer gets
   the accent tint at once; press: the icon scales to 0.94 (100 ms).
5. Click the scrubber letter "F" area (1508, ~300): the grid scrolls on
   OutCubic over 320 ms (record; match ±40 ms), F's apps lit, others 40 %.
6. Type `fire`, Escape, Escape: search clears, then the dialog closes
   (192 ms, exit curve (0.4,0,1,1)).
7. Tab back, click a folder card, then hover/press the back button
   (418, 158): fill on hover, squish to 0.9 (expressive) with a springy
   return; click: back to categories, that folder selected.
8. Right-click an app: glass menu at the pointer, as QML.
9. Press-drag inside the dialog: QML's dialog gives elastically (known
   difference: Rust's doesn't move).
10. Touchpad fling as AppDrawer's plan steps 12–13, A–Z view, pointer at
    (900, 500): same glide in Glass.

## Tester round 1 (desktop-batch1.md): failures and what was done

| Failure | Cause | Status |
|---|---|---|
| App icons in the Categories view are monochrome glyphs (Glass: Clear; Material: Themed circles) where QML shows each app's coloured Colloid logo on its squircle tile | The user's `appearance.iconStyle` is `"default"`. AppIcon.qml reads it as mode 0 (the app's own logo, tiled with iconShape squircle). core's `appicon::Style::from_settings` read `"default"` as the old name of `"look"` (Clear / Themed). The launchpad already draws every icon through the shared lookup and style (`wm::AppIcons`: Apps.iconFor → IconLookup, styled as AppIcon's default `styled: true`, in both views and in folder previews), so it follows core | **needs core's fix**: it is in ef9fd43 on `parity-int` ("iconStyle default is each app's own logo again"), which is **not** in this branch yet (3bbaaa3 doesn't carry it; core/src/appicon.rs here still maps default → look). Nothing to change in the launchpad; merge ef9fd43's appicon.rs and the tiles show the coloured logos |
| Category tiles "lay out differently" | Measured on the tester's shots (rust-/qml-{material,glass}-launchpad.png, crop 380,140–1300,500): card positions, sizes, corners, preview grid and labels match to the pixel in both looks; what differs is only the icons' style (Material's themed circles inside the card read as a different layout) and the accent (core's) | same once the icon style is fixed. One small difference seen: Material's selected folder name draws heavier in Rust (Slint's 600) than in QML (DemiBold rendered as the family's 500): left as the QML asks |

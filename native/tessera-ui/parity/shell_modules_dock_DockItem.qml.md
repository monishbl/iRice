# shell/modules/dock/DockItem.qml → ui/dock.slint (slot, Glass branch), surfaces/dock/src/{layout.rs, lib.rs}

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Magnification: size = base·(1 + (maxScale−1)·cos(min(1, |p − restCentre| / 2.5·base)·π/2)), straight from the pointer (no animation) | 157-158 | `layout::proximity`, `place` (the pointer's every move) | same |
| Hot: hovered, no menu, pointer within the icon ± gap/2 | 160-161 | `layout::hit` → `hot` | same |
| Grows upward from the pill's edge | 165 | slot `y: bar − pad − size` | same |
| Divider: 1 px, 0.8·base tall, 0.1·base above the bottom | 167-176 | `if s.divider` | same |
| Hover lift 0.14·base (only without magnification) on a spring 4.5 / 0.28 (ω 16.8, ζ 0.52) | 184-185 | `animate lift { 380ms; spring(0.48) }` (ω 16.5, ζ 0.52) | same |
| Press squish 0.9 on a spring 6 / 0.35 (ω 19.4, ζ 0.56) | 187-188 | `animate transform-scale { 320ms; spring(0.44) }` | same |
| Apps button: the theme's app grid (not generic, **not symbolic**), else the 3×3 dot grid | 192-227 | `find_icons`: the lookup took a `-symbolic` match as themed (Adwaita: a black glyph where the QML draws the dot grid). **Fixed**: `icons.path(names, [])` and `launcher_themed` excludes generic and symbolic, as launcherThemed | fixed |
| Minimized: its last frame (ScreencopyView, live: false) on a tile, the app's icon at half size bottom right | 229-254 | **was a blank tile**: now core's window pictures (`wm::Thumbs`, hyprland-toplevel-export) taken once while minimized (again if minimized again), drawn `image-fit: contain`, clipped to the tile | fixed |
| Icons rendered once at the largest magnified size (no re-render while magnifying) | 197-200, 250 | `icon_px` = base·maxScale·scale, one render per icon/size/style | same |
| Icon names: Downloads, Trash full/empty, apps (Apps.iconFor), with IconLookup's fallbacks | 252-253 | `Item::icon_names` + core `Icons::path` (same variants and fallbacks). Checked with Colloid (the user's) and Adwaita: every dock name resolves (see DockLogic notes / report) | same |
| Badge: unread count (99+), or a dot when it wants attention; red #FF3B30, 1 px border, pops in 200 ms OutBack | 256-280 | **was missing**: `Badge` in dock.slint, counts from the host's `badges` state (Badges.qml's), dot from `Windows.urgent` | fixed |
| Bounce: up 0.35·base 260 ms OutQuad, down 260 ms InQuad, twice | 283-294 | `hop` from `animation-tick()` | same |
| Running dot: 4 px, label colour 80 %, 0.35·pad under the icon | 296-306 | `if !Theme.material && s.running` | same |
| Name label above the hovered icon on frosted glass (26 px, radius 13), 10 px above the lifted icon | 308-324 | the label Rectangle (a translucent fill + top highlight; the dock's layer is blurred by the compositor as in QML) | same |
| Left click activates, right click the menu (at the icon's centre) | 326-335 | `press`/`release` in Rust from the window tracker | same |
| **Tester batch 1 (FAIL): Glass icons**: Apps a white mono grid on a dark tile (QML Colloid's coloured grid on its white tile), Downloads a grey flat folder on a dark tile (QML the blue Colloid folder-download, no tile), Trash a blank dark tile (QML Colloid's grey can) | AppIcon.qml 20-28 | The lookups were right (Colloid's files); the **style** was wrong: the user's `iconStyle` is "default" with `defaultsVersion` 3, which AppIcon reads as mode 0 (each logo as it is, on the squircle shape); core's `Style::from_settings` maps "default" to "look" (Clear in Glass: white mono on a dark glass tile; Themed in Material). `style_cfg()` in the dock hands core "original" (mode 0) for a migrated "default". Core should do the same for every surface (desktop, launchpad: the same report there) | fixed (dock); core fix needed for the rest |

## Test plan

### Geometry (the user's settings, 1920x1080 TESSERA-TEST, scale 1, no windows open)
Items: Apps button | Downloads, Trash (pinned is empty; the divider before Downloads).
- Glass (icon 48, gap 9, pad 12, bar 72, bottom gap 6, magnification ×1.6): pill x 857..1063, y 1002..1074; icon centres x Apps 894, Downloads 970, Trash 1027; y 1038. Hover band y 961..1074.
- Material (icon 48, pad 8, gap 9, bar 64, bottom gap 16): pill x 861..1059, y 1000..1064; icon centres x 894, 970, 1027; y 1032.
Each running app adds 57 px (icon + gap) before the divider and shifts the pill left by 28.5 px: re-measure from a shot.
Use `P glide` (not jumps) so hover enters/leaves. Record with wf-recorder at 60 fps; time with tools/anim-timing.py; widget reloads with tools/region-changes.py.

Glass look.
1. **Magnification**: record `P move 760 1000; P glide 760 1040 200; P glide 1160 1040 1600` (left to right across the dock). Each icon grows to 1.6× (76.8 px) as the pointer passes its centre with a cosine falloff over 2.5 icons (120 px); growth follows the pointer instantly (no lag/animation of its own), the pill widens symmetrically. Compare the QML recording frame by frame at the same pointer x: icon sizes equal ±1 px.
2. Name label: hovering Downloads (970,1038) shows "Downloads" on a frosted 26 px pill 10 px above the icon top; moving to Trash switches instantly.
3. With magnification off (`dock.magnification` false): hovering lifts the icon 6.7 px (0.14·48) on a springy curve (~380 ms, slight overshoot); leaving drops it back.
4. Press squish: press and hold on Downloads (`P down`) → icon scales to 0.9 with a spring (~320 ms); release → springs back.
5. Apps button: with Colloid, the Colloid view-app-grid icon (styled as the other icons); with Adwaita (`appearance.iconTheme` "Adwaita", restart) the **3×3 colour dot grid** (Adwaita's grid is only symbolic — the fixed case).
6. Icons with Colloid: Downloads = Colloid folder-download, Trash = user-trash (empty) / user-trash-full; foot/firefox app icons all drawn (none blank) — compare with the QML shot.
7. Minimised window: minimise a foot window → the item shows the window's last frame on a rounded tile with foot's icon at half size bottom right (was a blank tile).
8. Running dot: 4 px dot under foot's icon while it runs.
9. Badge: send an unread count (if a LauncherEntry app is available, or `ipc badges update` per host) → red #FF3B30 badge with the number pops in (200 ms OutBack) at the icon's top right.
10. **Icons (batch-1 failure)**: user's settings (iconStyle "default", defaultsVersion 3, iconShape squircle). Glass dock: Apps = Colloid's coloured grid on its white tile, Downloads = blue Colloid folder-download with no dark tile, Trash = grey can. Same as the QML shot (c-qml-glass-dock.png). Then set iconStyle "look": both shells turn to the Clear style (white logos on glass tiles).

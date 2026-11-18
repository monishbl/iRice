# shell/modules/dock/MaterialDockItem.qml → ui/dock.slint (slot, Material branch)

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| focused: running and the app in front | 153 | `Item.focused` | same |
| Hot: hovered, no menu, within the slot ± gap/2 | 157 | `layout::hit` | same |
| Divider: 1 px, half the slot, outlineVariant, centred | 165-173 | `if s.divider` (Material) | same |
| State layer circle (slot + 4) onSurface 8 % hover / 10 % pressed, 150 ms | 182-192 | the circle Rectangle, `animate background 150ms` | same |
| Glyph swells while pressed (ElasticControl: 1 + min(.06, 10/size) on the press spring 7.5 / 0.3: ω 21.7, ζ 0.43) | 194-209 | `animate transform-scale { 290ms; spring(0.57) }` | same |
| Apps button: tonal circle squaring off to 16 when pressed (hover duration, OutCubic), tint 8 / 10 %, squares-four filled | 211-229 | the launcher Rectangle; its own second ElasticControl (it swells twice) **was missing**: now its own transform-scale too | fixed |
| Minimized: its last frame, radius.item, surfaceContainerHighest, the app's icon at half size | 231-245 | **was a blank tile**: now the window picture (`wm::Thumbs`), clipped, `Theme.radius-item` (was 8 px) | fixed |
| Icon names as Glass | 247-256 | `Item::icon_names` | same |
| Badge: Material's large badge (error / onError, 16 px, 11 px text, 999+) or the 8 px dot; pops on the fast spring | 258-285 | **was missing**: `Badge` (Material branch), spring 222 ms / bounce 0.4 | fixed |
| Bounce: up 0.3·slot 220 ms on emphasized decelerate (0.05, 0.7, 0.1, 1), down 260 ms OutBounce, twice | 289-300 | the up phase was out-quint (up to 23 % off the curve); **now the exact bezier** (`emphasized()`, solved by halving in Slint); down: out-bounce | fixed |
| Running dash: 12 px, 16 px and primary when focused; width on the fast spring, colour 150 ms | 302-320 | `animate width { 220ms; spring(0.4) }`, `animate background 150ms` | same |
| Plain tooltip: inverseSurface, 24 px, radius 4, 12 px / 0.4 spacing, ≤ 280 px; fades 150 in / 100 out, rises 4 px on emphasized 150 ms; hidden while a menu or the stack is up | 322-345 | the tooltip Rectangle (`popup-open`) | same |
| Clicks: left activates, right the menu | 347-354 | Rust press/release | same |
| **Tester batch 1 (FAIL): Material icons**: Downloads a pale pink flat folder, Trash a blank brown disc (QML the blue Colloid folder and grey can) | AppIcon.qml 20-28 | same cause as Glass: "default" taken as Themed (mode 4); now mode 0 via `style_cfg()` | fixed (dock) |
| Press swell only with Material's press bounce (Reduce motion turns it off: Theme.motion.pressBounce) | ElasticControl / Theme.qml 307 | `Theme.press-bounce` gates both swells; the Downloads stack's Quick Look swell too | fixed |

## Test plan

### Geometry (the user's settings, 1920x1080 TESSERA-TEST, scale 1, no windows open)
Items: Apps button | Downloads, Trash (pinned is empty; the divider before Downloads).
- Glass (icon 48, gap 9, pad 12, bar 72, bottom gap 6, magnification ×1.6): pill x 857..1063, y 1002..1074; icon centres x Apps 894, Downloads 970, Trash 1027; y 1038. Hover band y 961..1074.
- Material (icon 48, pad 8, gap 9, bar 64, bottom gap 16): pill x 861..1059, y 1000..1064; icon centres x 894, 970, 1027; y 1032.
Each running app adds 57 px (icon + gap) before the divider and shifts the pill left by 28.5 px: re-measure from a shot.
Use `P glide` (not jumps) so hover enters/leaves. Record with wf-recorder at 60 fps; time with tools/anim-timing.py; widget reloads with tools/region-changes.py.

Material look.
1. State layer: hover Downloads (970,1032) → a 52 px circle of onSurface 8 % fades in over 150 ms; press (`P down`) → 10 %; the glyph swells ~6 % on a springy press (290 ms, overshoot on release).
2. Apps button (894,1032): a secondaryContainer circle with the apps (squares-four) symbol; press → corners square to 16 px (150 ms) and it swells (twice the swell of other icons: its own ElasticControl inside the icon's).
3. Bounce: pin foot, click it while not running → hops 14.4 px (0.3·48) twice: up 220 ms on emphasized decelerate (fast start, long ease), down 260 ms with OutBounce. Record at 60 fps and compare the hop curve with the QML pass frame by frame (peak height and time to peak equal ±1 frame).
4. Running dash: 12 px dash under foot while it runs, 16 px primary while foot is in front; switching focus animates the width springily (~220 ms).
5. Minimised foot: its last frame on a surfaceContainerHighest tile (radius item), foot's icon half-size bottom right.
6. Badge (if a count can be sent): 16 px error-coloured badge, 11 px number; urgent: 8 px dot.
7. **Icons (batch-1 failure)**: Material dock with the user's settings: Downloads = blue Colloid folder, Trash = grey can (not pink/brown), as in c-qml-material-dock.png.
8. `appearance.reduceMotion` true: pressing an icon doesn't swell (both shells).

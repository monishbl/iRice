# shell/modules/dock/MaterialDock.qml → surfaces/dock/src/{layout.rs `Metrics`, lib.rs}, ui/dock.slint `Dock` (Material branch)

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Icon 36..56, glyph icon − 4, pad 8, gap from Settings, bar icon + 16, bottom gap 16, no magnification | 31-38 | `Metrics::new` (Material) | same |
| Pill width = rest + 2·pad; hover against the resting rect only | 42-53 | `layout::hit` (Material) | same |
| Tooltips: wait 500 ms the first time, then follow the pointer; reset on leaving | 60-75 | `tip_timer` → `M::TipsWarm`, `tips_warm` | same |
| Slide on Material's default spatial spring (k 380, ζ 0.8) | 79-83 | `animate slide { 320ms; spring(0.2) }` (ω 19.6, ζ 0.8) | same |
| Exclusive zone only when auto-hide is off; window = bar + gap + 48 | 85-88 | `spec`, `window_height` | same |
| Pill: surfaceContainer, fully round, no shadow; slides out bar + gap + 8 | 100-108 | dock.slint `pill` | same |
| …login intro settle (LockFx) | 104 | — | missing (LockFx isn't in the Rust shell) |
| Menu and Material stack in their own windows | 127-136 | `open_menu`, `downloadsstack` | same |

## Test plan

### Geometry (the user's settings, 1920x1080 TESSERA-TEST, scale 1, no windows open)
Items: Apps button | Downloads, Trash (pinned is empty; the divider before Downloads).
- Glass (icon 48, gap 9, pad 12, bar 72, bottom gap 6, magnification ×1.6): pill x 857..1063, y 1002..1074; icon centres x Apps 894, Downloads 970, Trash 1027; y 1038. Hover band y 961..1074.
- Material (icon 48, pad 8, gap 9, bar 64, bottom gap 16): pill x 861..1059, y 1000..1064; icon centres x 894, 970, 1027; y 1032.
Each running app adds 57 px (icon + gap) before the divider and shifts the pill left by 28.5 px: re-measure from a shot.
Use `P glide` (not jumps) so hover enters/leaves. Record with wf-recorder at 60 fps; time with tools/anim-timing.py; widget reloads with tools/region-changes.py.

Material look (appearance.look "material").
1. Idle: surfaceContainer pill, fully round, 64 px tall, 16 px above the bottom, no shadow.
2. Tooltips: glide onto Downloads (970,1032) and stay: the tooltip "Downloads" appears after ~500 ms (fade 150 ms, rising 4 px). Glide to Trash (1027,1032) without leaving the pill: "Trash" appears immediately (no second wait). Glide off to (960,900) and back: the 500 ms wait again.
3. Slide (autoHide "always"): spring ~320 ms, slight overshoot coming up; compare with QML.

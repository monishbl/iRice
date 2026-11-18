# shell/modules/dock/DockGlass.qml → surfaces/dock/src/{layout.rs `Metrics`, lib.rs}, ui/dock.slint `Dock` (Glass branch)

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Sizes: pad = 0.26·icon, corner = 0.32·icon, gap = icon·clamp(gap, .05, .4), bar = icon + 2·pad, bottom gap 6, divider 1 + 2·gap | 26-35 | `Metrics::new` (Glass) | same |
| Magnification only when on: maxScale = magnifiedScale | 27 | `max_scale` | same |
| Rest width; the largest pill width (rest + 2·pad + extra·2.6) for the zones | 39, 56 | `rest_width`, `max_width` | same |
| Hovered: settled, on the (magnified) pill or the icons above it | 45 | `layout::hit` (Glass) | same |
| pointerX relative to the centre, only while hovered (else no magnification) | 47 | `hit`: `place(.., None)` when not on it | same |
| Zone and pill rectangles (fixed) | 57-58 | `layout::zones` | same |
| Slide: up 350 ms OutBack (overshoot 1.2), down 250 ms InCubic | 62-69 | `animate slide`: down cubic-bezier(0.32,0,0.67,0) (InCubic); up was a bezier 1.9 % off OutBack; now cubic-bezier(0.35,1.45,0.7,0.975), within 0.5 % of OutBack(1.2) (fitted) | fixed (as close as a bezier gets) |
| Top layer; exclusive zone bar + gap only when auto-hide is off | 71-74 | `spec`, `Metrics::exclusive` | same |
| Window height: ceil(icon·maxScale) + 2·pad + gap + 44 | 75 | `window_height` | same |
| Pill slides all the way out (lifted icons too): bottomMargin = gap − (bar + extra + gap + 24)·slide | 92 | dock.slint `pill.y` | same |
| …and the login intro settles it 20 px (LockFx.settle) | 92 | — | missing: LockFx (the lock screen's zoom) isn't in the Rust shell; the dock is simply there |
| Glass slab: the plugin's glass, radius = corner, bevel 14 | 98-104 | `after_frame`: `theme::glass_shape(pill)` sent when it changes; without the plugin a tint + hairline | same |
| Row of items, each at its rest centre | 106-123 | `layout::place` | same |
| Menu and Downloads stack in their own overlay windows | 127-139 | `open_menu` (DockMenu window), `downloadsstack` part | same |

## Test plan

### Geometry (the user's settings, 1920x1080 TESSERA-TEST, scale 1, no windows open)
Items: Apps button | Downloads, Trash (pinned is empty; the divider before Downloads).
- Glass (icon 48, gap 9, pad 12, bar 72, bottom gap 6, magnification ×1.6): pill x 857..1063, y 1002..1074; icon centres x Apps 894, Downloads 970, Trash 1027; y 1038. Hover band y 961..1074.
- Material (icon 48, pad 8, gap 9, bar 64, bottom gap 16): pill x 861..1059, y 1000..1064; icon centres x 894, 970, 1027; y 1032.
Each running app adds 57 px (icon + gap) before the divider and shifts the pill left by 28.5 px: re-measure from a shot.
Use `P glide` (not jumps) so hover enters/leaves. Record with wf-recorder at 60 fps; time with tools/anim-timing.py; widget reloads with tools/region-changes.py.

Glass look only.
1. Slide timings (autoHide "always"): record a reveal and a hide; reveal 350 ms with an overshoot of a few px (OutBack 1.2), hide 250 ms InCubic. Compare frame by frame with the QML pass (anim-timing.py on both: same length ±1 frame, overshoot peak at ~55 % of the way).
2. Hover band: glide from (960,950) down to (960,1040): magnification starts only once y ≥ 961 (the band includes the magnified height + 12).
3. The slab: plugin glass under the pill following its width while magnifying (no stale glass left at the old width: shot at rest after a glide off).
4. Exclusive zone with autoHide "off": a maximized foot window ends 78 px (72 + 6) above the bottom.

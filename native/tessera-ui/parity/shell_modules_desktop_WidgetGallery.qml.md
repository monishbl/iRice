# shell/modules/desktop/WidgetGallery.qml → surfaces/desktop/src/gallery.rs; ui/desktop.slint `Gallery`

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Opens with Edit Widgets, springs up (4/0.32) and fades | 36-45 | `progress` 520 ms spring(0.4) / 192 ms out | same by eye |
| Escape ends editing; Up/Down pick a kind | 47-52 | `key-pressed` | same |
| Kinds list hover/picked | 70-104 | `rowt` | same |
| Done (primary) | 130-145 | `dt` | same |
| Size previews: hover accent border, 1.04, + badge; pressed 0.95; spring 5/0.35; click adds | 150-215 | `at` | same |
| Previews keep their elements while open | — | lists kept per `gallery:kind:size` | fixed |

## Test plan

Geometry (1920x1080 TESSERA-TEST, the user's 4 widgets): grid origin 32,54; a cell 164 px, gap 16; a cell (c,r) starts at x = 32 + 180c, y = 54 + 180r; s = 164x164, m = 344x164, l = 344x344. The user's layout: clock m at (0,0) → 32..376 x 54..218; stack m at (0,1) → 32..376 x 234..398; batteries m at (0,2) → 32..376 x 414..578; shortcuts m at (2,0) → 392..736 x 54..218.
Both looks: run once with appearance.look "glass" (the user's) and once "material" (dark).

1. Right-click desktop → Edit Widgets…; record the gallery opening (compare rise and settle with QML). Press Down 3×, Up 1×: picks follow. Hover a size preview: accent border, grows 1.04 with a spring; click: the widget is added on the desktop (grows in 0.9→1, 420 ms); the other widgets don't blink. Escape: gallery and editing end.

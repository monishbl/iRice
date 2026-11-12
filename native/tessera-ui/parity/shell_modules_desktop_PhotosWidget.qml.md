# shell/modules/desktop/PhotosWidget.qml → ui/desktop.slint `PhotosW`; lib.rs `fill` "photos", `next_photo`

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| A random picture; the next every 15 minutes | 29-41 | `pick`, `slow` | same |
| Cross-fade a ↔ b over 800 ms | 47-66 | `front`, opacity 800 ms | same (now actually plays: the images are no longer made again) |
| Cropped to fill, tile corners | 49-75 | `files::cover` | same |
| Material small: cut to clover4 on the surface container | 76-93 | **was a circle** → pictures cut to clover4 in their pixels; the clover drawn in surfaceContainer under them | fixed |
| Empty state; click → next (2+ pictures) | 96-119 | t1, TouchArea | same |

## Test plan

Geometry (1920x1080 IRICE-TEST, the user's 4 widgets): grid origin 32,54; a cell 164 px, gap 16; a cell (c,r) starts at x = 32 + 180c, y = 54 + 180r; s = 164x164, m = 344x164, l = 344x344. The user's layout: clock m at (0,0) → 32..376 x 54..218; stack m at (0,1) → 32..376 x 234..398; batteries m at (0,2) → 32..376 x 414..578; shortcuts m at (2,0) → 392..736 x 54..218.
Both looks: run once with appearance.look "glass" (the user's) and once "material" (dark).

1. Put 3 jpgs in the arena's ~/Pictures. Add Photos Medium. Click it: the next picture cross-fades over ≈800 ms (anim-timing on its region ≈ 0.8 s). Material: resize to Small → the picture is a four-leaf clover on the surface container colour.

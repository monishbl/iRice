# shell/modules/desktop/NowPlayingWidget.qml → ui/desktop.slint `NowPlayingW`; lib.rs `fill` "nowPlaying", `art`

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Art 64 px (small) / full height (medium), cropped, loaded async | 13-27 | `art()` off the UI thread, cached | same |
| Material: art in a cookie9 while playing, a square while paused, morphing | 28-38 | **was a circle clip** → art cut to cookie9 in its pixels (core polar::cut); square and cookie covers cross over 260 ms with a 0.92→1 settle | fixed (a cross-fade, not a vertex morph) |
| No art: music-notes glyph; title ("Not Playing") and artist | 39-68 | SymIcon, t1, t2 | same |
| Material play/pause: primary, round paused / squarer playing (springFast) | 70-87 | border-radius 250 ms | same by eye |
| Previous / play-pause / next; disabled without a player | 88-110 | `bt.enabled: w.flag` | same |

## Test plan

Geometry (1920x1080 TESSERA-TEST, the user's 4 widgets): grid origin 32,54; a cell 164 px, gap 16; a cell (c,r) starts at x = 32 + 180c, y = 54 + 180r; s = 164x164, m = 344x164, l = 344x344. The user's layout: clock m at (0,0) → 32..376 x 54..218; stack m at (0,1) → 32..376 x 234..398; batteries m at (0,2) → 32..376 x 414..578; shortcuts m at (2,0) → 392..736 x 54..218.
Both looks: run once with appearance.look "glass" (the user's) and once "material" (dark).

1. Material look. Put Now Playing in the stack (scroll to it) or add it Medium. Play something with art in the arena (`mpv --no-video` on a file with a cover, or a player the arena has). PASS: playing → the cover becomes a 9-scallop cookie (≈260 ms); pause (click the middle button) → back to a rounded square; the button squares off while playing. Next/previous work. Glass: cover stays a 12 px rounded square.

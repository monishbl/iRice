# shell/modules/menubar/ShapedArt.qml

Material: a picture (album art, an avatar) cut into one of Material's shapes
(MaterialShape: cookie9, square, circle…), the shape's colour showing while
there's none. Used in IslandEvent (card avatar), IslandFocusCard (side art),
IslandLive (pill art), NowPlaying. Rust: no shape cutting; Slint clips to
rounded rectangles / circles.

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| Image: crop, async, retain while loading, sourceSize | 16-24 | lib.rs art loaded off-thread (`follow_art` 319-334) | same |
| Cut into `shape` (cookie9 / square / circle), `animated` morph | 25-29 | island.slint:981 (pill: circle), 1332 (card: radius 16), 643 (alert art: radius 12) | different: circle in the pill matches; cookie9 and the playing↔paused morph missing |
| Shape's own colour while there's no picture | 14, 28 | island.slint:983, 1334 (ink 0.15 / 0.1) | different: ink tint, not surfaceContainerHighest / tertiaryContainer |

## Summary

1. Material cookie shapes (focus card art while playing, card avatars, alert art) render as rounded squares / circles; no morph between playing and paused.

## Fixed in batch 5 (fixer, 8 Nov)

- Material's cookie: Now Playing's art is cut into a nine-sided cookie (r = R(0.92 + 0.08 cos 9θ), anti-aliased) in Rust while it plays, and crossfades (300 ms) to the 16 px-cornered square when paused. (The island's art is its background in the user's overlay layout; the side layout's thumbnail isn't drawn.) Not the QML's spring morph between the shapes: a crossfade.

## Test plan (batch 5)

- Material, Now Playing open with the fake player and an art file (`fakeplayer.py /path/cover.jpg`): playing, the art is a scalloped cookie; pause: it becomes a rounded square (fade ~300 ms).

## Test results (batch 5)

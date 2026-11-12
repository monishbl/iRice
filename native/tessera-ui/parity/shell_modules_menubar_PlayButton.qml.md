# shell/modules/menubar/PlayButton.qml

Material's play / pause button: a filled shape that morphs on a spring
(round paused, rounded square playing), squeezes when pressed, with a state
layer and keyboard focus. Used by IslandFocusCard (Material, 44 px) and
NowPlaying. Rust: none; the open card's play is a plain `IconButton`
(`surfaces/menubar/ui/island.slint` 511-532, used at 1369-1379).

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| Size 48 (44 in the card), primary fill, onPrimary icon at 0.42 | 99-102, 135-141 | island.slint:1369-1379 (36×32 transparent, icon 20 in ink) | different |
| Opacity 0.38 when disabled | 108 | island.slint:1376 (0.4) | same |
| Keyboard: Tab focus, Space / Return / Enter clicks; FocusRing | 109-121 | — | missing |
| a11y: Button "Play" / "Pause", press action | 116-118 | island.slint:1377 (`label: c.icon`) | different: glyph code point as name |
| Shape morph circle ↔ square on Material's spring | 123-134 | — | missing |
| State layer: icon colour 8 % hover, 10 % press | 129 | island.slint:519 (ink 0.12 / 0.2 on transparent) | different |
| Press squeeze (ElasticControl) + scale spring (springFast) | 130-133 | — | missing |
| Click, pointer cursor | 142-148 | island.slint:529-531 | same (no pointer cursor) |

## Summary

1. Material play button is a plain glyph: no filled morphing shape, no press squeeze.
2. Not keyboard-focusable; accessible name is a glyph code point.

## Batch 3

WavyProgress-style seek bar and the morphing play button: see IslandFocusCard.qml.md batch 3.

# shell/modules/menubar/PlayButton.qml

Material's play / pause button: a filled shape that morphs on a spring
(round paused, rounded square playing), squeezes when pressed, with a state
layer and keyboard focus. Used by IslandFocusCard (Material, 44 px) and
NowPlaying. Rust: none; the open card's play is a plain `IconButton`
(`surfaces/menubar/ui/island.slint` 511-532, used at 1369-1379).

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| Size 48 (44 in the card), primary fill, onPrimary icon at 0.42 | 99-102, 135-141 | island.slint `PlayMorph` (44, primary, on-primary 20 ≈ 0.42·44) | same (fixed after batch 2) |
| Opacity 0.38 when disabled | 108 | island.slint:1376 (0.4) | same |
| Keyboard: Tab focus, Space / Return / Enter clicks; FocusRing | 109-121 | — (accessible default action only) | different: the island takes no Tab focus (it has the keyboard only after a click on an alert, for Esc), so its buttons aren't reachable by Tab in either look |
| a11y: Button "Play" / "Pause", press action | 116-118 | island.slint `PlayMorph` accessible-label Play / Pause, default action | same (fixed after batch 2) |
| Shape morph circle ↔ square on Material's spring | 123-134 | island.slint `PlayMorph` radius 22 ↔ 14, spring(0.35) 350 ms | same (fixed after batch 2) |
| State layer: icon colour 8 % hover, 10 % press | 129 | island.slint `PlayMorph` (primary mixed with on-primary 8 % / 10 %) | same (fixed after batch 2) |
| Press squeeze (ElasticControl) + scale spring (springFast) | 130-133 | island.slint `PlayMorph` 94 % pressed (100 ms OutCubic) | same (fixed after batch 2; one squeeze curve for both) |
| Click, pointer cursor | 142-148 | island.slint:529-531 | same (no pointer cursor) |

## Summary

1. Material play button is a plain glyph: no filled morphing shape, no press squeeze.
2. Not keyboard-focusable; accessible name is a glyph code point.

## Batch 3

WavyProgress-style seek bar and the morphing play button: see IslandFocusCard.qml.md batch 3.

## Re-check 17 Nov (B4)

`PlayMorph` had been built after batch 2; the rows still said missing. Before: 3 missing, 3 different; after: 7 same, 1 different (no Tab focus in the island).

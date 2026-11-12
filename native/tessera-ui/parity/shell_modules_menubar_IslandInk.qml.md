# shell/modules/menubar/IslandInk.qml

Singleton: the island's colours. Glass: white on black. Material: a tonal
surface (surfaceContainerHigh, Highest while an alert shows) with
on-colours; red (urgent) kinds become error. Rust: spread over
`surfaces/menubar/ui/island.slint` (Island `ink` / `ink2` 1100-1101, surface
1162-1173, Pill 480-509, IconButton 511-532) and `src/island.rs` `is_red`
(398) / `src/lib.rs` tints (350-361, 431-436).

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| surface(inverse): Highest with an alert, else High | 165-167 | island.slint:1167 | same |
| ink(): Glass #F5F5F7, Material onSurface | 168-172 | island.slint:1100 | same |
| dim(a): Glass white a; Material onSurfaceVariant for 0.45–0.9, else onSurface at a | 173-179 | island.slint:1101 (`ink2`), per-use alphas | same for ink2; tracks / hovers use ink alphas (close enough) |
| accent / onAccent: Glass Theme.accent / onAccent; Material primary / onPrimary | 181-191 | Pill 490, 499 | same |
| tint(c): Material error if red, else primary | 194-198 | lib.rs:350-361, 432; ActivityRow 899, 908 | same where applied; LiveInfo lead (983) keys on kind instead; focus card (1334) always primary |
| onTint(c): onError if red, else onAccent | 199-201 | ActivityRow 913 | different elsewhere: alert glyph icon (663), LiveInfo icon (1002), focus-card icon (1345) always on-primary |
| isRed: r > 0.8, g < 0.45, b < 0.45, or equal to m3.error | 202-207 | src/island.rs:398-400; ActivityRow 899 | same (no m3.error equality) |
| layer(base, content, m): content at 8 % hover / 10 % press | 210-212 | Pill filled (490 `mix` 0.92 / 0.9) | same for the filled pill; tonal pills (490), IconButtons (519) have no layer on their base |
| tonal / onTonal: secondaryContainer / onSecondaryContainer | 214-219 | Pill 490, 499 | same |

## Summary

1. Material: icons on red discs (alert glyph, pill lead, focus badge) use on-primary instead of on-error; focus badge never turns error.
2. Pill lead's red decided by kind, not colour.
3. Tonal buttons lack the 8 % / 10 % state layer.

## Fixed in batch 5 (fixer, 8 Nov)

- Material, red by colour (IslandInk.tint): the pill's lead circle and its icon (error / on-error) follow the activity's colour, not its kind; alert glyphs on red discs on-error (batch 4).
- Not done: the tonal buttons' 8 % / 10 % state layer.

## Test plan (batch 5)

- Material: a paused recording (grey) has a primary lead circle; a dictation (red) an error one.

## Test results (batch 5)

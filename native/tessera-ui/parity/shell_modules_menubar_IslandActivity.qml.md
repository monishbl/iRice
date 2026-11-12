# shell/modules/menubar/IslandActivity.qml

One live activity as a row in the open island when Settings → island →
activityMode is "all" (IslandView 720-729: every activity but music, which
keeps the full player above them). Fades and rises in after `index × 45 ms`.
Material inline. Rust: `ActivityRow` in `surfaces/menubar/ui/island.slint`
(885-966), `ActRow` struct (421-430), placed at 1394-1405 under `all-mode`;
data `detail` / `progress` / `all` from the host (`host/shell/src/island.rs`
`push_live` 543-555) parsed in `src/island.rs` 64-89. In progress at audit
time: lib.rs doesn't yet set `all-mode` / `act-rows` (settings.rs reads
`activityMode`).

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| Height: progress 42, else 36 (music 52/60, captions its lines) | 21 | island.slint:895 | same (music isn't a row: IslandView 721); captions not special |
| Position poll Timer (music rows) | 24-29 | — | n/a (no music row) |
| Appear: after `delay`, 280 ms OutCubic; rises 8 px; no delay going out | 31-39 | island.slint:896-901 (cubic-bezier(.33,1,.68,1)) | same |
| Lead: 30 px circle, activity colour 0.22; Material IslandInk.tint (error if red) | 42-49 | island.slint:902-908 | same |
| Icon 15 px filled, Glass activity colour, Material onTint | 59-66 | island.slint:909-914 | same |
| Music lead 48 px radius 10 with art | 44-58 | — | n/a |
| Title body, Medium (Material) / DemiBold; captions "Live captions" | 73-79 | island.slint:939-945 | same (captions' title is whatever the owner pushed) |
| Captions: last 3 lines (CaptionLines) | 81-88 | — | missing |
| Detail: small, dim 0.6, tabular | 89-96 | island.slint:946-951 | same (no tabular figures) |
| Music seek line 3→5 px on hover, click seeks; Material wavy | 97-142 | — | n/a |
| Progress: 3 px, white 0.16, fill activity colour, width 900 ms | 144-158 | island.slint:952-963 | same |
| Material progress: WavyProgress in IslandInk.tint | 159-167 | island.slint:955-960 | different: flat bar (secondary-container / primary or error) |
| Controls: every control but "reset", 32 px, icon 16, danger red, 0.4 when off, click runs `act` | 170-189 | island.slint:916-933, `control(kind, j, true)` → host island.rs:926-934 (`all`) | same once lib.rs wires it |
| Controls' a11y name | IconButton | island.slint:930 (`label: c.icon`, a glyph code point) | different: screen readers hear the glyph |

## Summary

1. "All at once" mode only works once lib.rs sets `all-mode` / `act-rows` (struct and view are in).
2. Captions row lacks its caption lines.
3. Material progress is flat, not wavy; buttons' accessible names are glyph code points.

## Fixed in batch 1 (fixer, 7 Nov)

- lib.rs now feeds the rows: `act_rows()` (every activity but music, title, detail or time, progress, every control but reset, a kept controls model per row), `card` = the music activity in "all" mode, `all-mode`, `control(kind, i, true)` with the index mapped back past "reset" → host `island.live <kind> <j> all`.

## Test plan (Sonnet tester)

See shell_modules_menubar_IslandView.qml.md steps 3-4 and shell_services_Island.qml.md "Test plan: live activities". Check the rows stagger in 45 ms apart (record the island opening with a timer and a stopwatch running: each row fades and rises 8 px over 280 ms).

## Test results

## Fixed in batch 4 (fixer, 7 Nov)

- The rows' entrance runs on their own clock (fade + 8 px rise, 280 ms OutCubic after index × 45 ms), as the status line's (an animated property could settle while hidden).

## Test plan (batch 4)

- Record the island opening with a timer and a stopwatch: rows fade/rise in 45 ms apart.

## Test results (batch 4)

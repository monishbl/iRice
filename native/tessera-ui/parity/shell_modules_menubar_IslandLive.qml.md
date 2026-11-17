# shell/modules/menubar/IslandLive.qml

One side of the collapsed island's live activity: `info` (art or icon,
title, time, dancing bars) or `controls`. Each piece switchable
(Island.shows). Material inline. Rust: `LiveInfo` in
`surfaces/menubar/ui/island.slint` (968-1034) and the `trail` controls in
`Island` (1251-1268); data `live_struct` in `src/lib.rs` (346-420); bar
clock in `ui/menubar.slint` (367-373).

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| Pieces on/off per kind (Island.shows: art/icon, title/label/name, time, bars, controls) | 27-29 | host island.rs:435-441 `shows`, push_live 556-559; lib.rs:371-394 (music, settings.rs `shows`) | same |
| Lead 18 px (Material 20) | 43-49 | island.slint:976-981 | same |
| Glass art: radius 5, ink 0.15 behind | 50-64 | island.slint:981-989 | same |
| Material: tinted circle IslandInk.tint (error when the colour is red) | 66-71 | island.slint LiveInfo (`live.tint == m3-error`; lib.rs `tint_of`: red colours → error) | same (fixed after batch 1: by colour) |
| Material: art in a circle (ShapedArt) | 72-83 | island.slint:981-989 | same |
| Recording dot 9 (Material 8), red (Material onTint), opacity 0.675 + 0.325·cos(t·π/0.7), 15 fps Ticker | 84-94 | island.slint:991-997, menubar.slint:369-373 (83 ms tick) | same (12 fps) |
| Call: the app's icon 16 px | 95-102 | island.slint LiveInfo `live.kind == "call" && live.has-app-icon` Image 16; host island_live.rs call `appIcon` | same (17 Nov; a call in progress is an activity standalone now) |
| Icon 14 (Material 12), Glass activity colour, Material onTint | 103-110 | island.slint LiveInfo SymIcon (on-error on error) | same (fixed after batch 1) |
| Title: Marquee, music = track title or identity, screenShare = the app, others LiveInfo.title | 113-118 | island.slint:1006-1014, lib.rs:386 | same text; no marquee |
| Title max 132 (music) / 90, minus the other pieces and IslandView's maxWidth | 123 | island.slint LiveInfo Marquee `max-w` (minus lead, the measured time, the bars; `room` = area − trail − 4 beside the spaces) | same (17 Nov) |
| Title scrolls when long (`music.scroll`) | 119 | island.slint `Marquee` (`scroll: live.show-scroll`) | same (fixed after batch 2; see Marquee.qml.md) |
| Time: not for music; Material recording error else ink, Glass recording red else activity colour; small DemiBold tabular | 125-132 | island.slint:1015-1021 | same (no tabular figures) |
| Bars: music, playing, `bars` piece; fade 200 ms and width → 0 when off | 135-143 | island.slint LiveInfo bars (width and opacity animate 200 ms) | same (17 Nov) |
| Bar clock 12 fps; held while unseen and on battery under 30 % | 144-146 | menubar.slint `tick` Timer (only while the compact pill shows music; `bars-hold` from lib.rs: on battery under 30 %) | same (17 Nov: the battery hold; unseen = not compact, already held) |
| Bars: 2.5 px, lo/hi 5–11, 4–10, 4–13, periods 0.70 / 0.66 / 0.74, phase 1.9 rad each, green (Material primary) | 147-162 | island.slint:1024-1032 | same |
| Controls: LiveInfo.controls filtered by pieces, 24×22, icon 13, danger red, 0.4 and disabled when off | 167-187 | island.slint:1258-1267, host push_live 541; lib.rs:371-380 (music) | same |
| Control click runs `act` | 184 | island.slint:1266 → lib.rs `island_control` 1122-1139 (MPRIS / `rustui act live`) | same (only while compact) |
| Hovering stop/pause doesn't open the island (IslandView.overControls) | IslandView | island.slint:1126-1130 | same |
| Controls' a11y name | IconButton | island.slint `label: c.name` (host island.rs `ctl_name`; music Previous / Play / Pause / Next) | same (17 Nov) |

## Summary

1. Call shows a generic icon, not its app's icon.
2. Long song titles don't scroll; title width not shared with the other pieces.
3. Material colours keyed on kind (dictation not error, paused recording still error); Material icon always on-primary.
4. Bars pop instead of fading; no hold on low battery.
5. Controls' accessible names are glyph code points.

## Batch 2

The pill time keeps a stable width (every digit measured as 0): MenuBar.qml.md test plan step 7.

## Re-check 17 Nov (B4)

Before: 2 missing, 6 different; after: all same. Two had been fixed after batch 1 without the rows changing (colour-keyed tint, Material on-error, the Marquee); ported now: a call's app icon, the title's room, the bars' fade, the battery hold, the controls' names.

# shell/modules/osd/MaterialVolumePanel.qml

Material's OSD: Android's volume panel at the right edge. Rust: `OsdPanel`
in `surfaces/osd/ui/osd.slint` (447-790), driven by `surfaces/osd/src/lib.rs`
(`open_hud`, `refresh`, `after_frame` for the input region and the pointer).

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| Sizes: track 44×200, pad 8, buttons 44, 16 from the edge, card 296 | 24-36 | slint properties | same |
| Layer: overlay, top+bottom+right, width edge+panel+12+card+24 | 38-44 | `Spec` 408 wide | same |
| Enter: springDefault (overshoot) from the edge; exit 200 ms on the exit curve | 46-77 | `animate progress` spring(0.2) / cubic-bezier | different engine; tester compares settle |
| Input only on the panel (and the card when open) | 79-93 | after_frame `set_input_region` | same |
| Hover anywhere on it → pointerOn (keeps it up) | 94-96 | `hover` TouchArea → `pointer-on` | same |
| Panel slides in by its width + 16, opacity progress·2 | 99-105 | slint `panel` | same |
| Shadow (none when blurred), surfaceContainer pill | 107-121 | drop-shadow, `panel-alpha` | same |
| Mute toggle: round, squarer (12) tertiary when muted, spring radius, hover/press layer; click toggles quietly | 129-164 | slint toggle, `toggle-mute` | same |
| Slider: inactive top with stop dot, handle 4 (2 pressed, 100 ms), active primary below; shownRatio glides 150 ms (standard) except pressed | 166-235 | slint `slider` | same |
| Icon inside the active part while it fits, else above the handle | 236-244 | slint Icon | same |
| Press / drag (area −pad all round) sets the level; wheel ±5% | 246-267 | `area` | same; **fixed**: the level shown is the one just set (held 600 ms), so the handle no longer snaps back to the server's older readings mid-drag |
| Sliders button: secondaryContainer and squarer when open, toggles the card | 270-303 | slint `more-ta` | same |
| Value pill beside the handle while pressed (inverseSurface, 100 ms) | 306-324 | slint | same (tabular figures: font default) |
| Toast pill: device name or mic state, slides 8 px, hidden while the card is open or dragging | 326-359 | `toast-pill` | same |
| Card: opens beside, opacity 150 ms, scale 0.92→1 spring from bottom-right, "Outputs/Inputs/Displays", IslandLevels; closing resets to the list | 362-411 | slint `card`, `Levels` | same |
| Card rows: pick output/input, App volumes, per-app level and mute, display level, settings | IslandLevels | lib.rs `picked`, `row_level` | same; **fixed** levels follow at once; "Sound settings" opens Settings standalone (was shell-ipc.sh) |

## Tester plan (Material)

See Osd.qml's plan steps 10–15 and 17; additionally:
1. Hover the panel, then move the pointer onto the slider, the mute toggle
   and the card in turn (glides): it must stay up the whole time; leave: it
   hides 1.6 s later (time it in a recording).
2. Drag the slider fast from bottom to top in 200 ms: the final level is
   100% (wpctl 1.00) and the handle ends at the top.
3. Click the panel's empty part between the toggle and the slider: nothing
   happens and clicks don't reach the window behind (input region).
4. Click outside the panel (on the desktop to its left, outside the card):
   the click reaches the desktop (the window doesn't block it).

## Test results

(tester)

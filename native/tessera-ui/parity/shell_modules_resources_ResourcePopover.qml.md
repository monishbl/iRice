# shell/modules/resources/ResourcePopover.qml (+ ResourceRow.qml, ResourceGraph.qml, ResourceTones.qml)

The small glass popover under a hovered figure: every stat group on a row
(Processor, Graphics, Memory, Network) with a badge, figures and the last
minute's graph; the pointed-at group highlighted. Rust:
`surfaces/resources/src/lib.rs` (window, sampling, rows), `stats.rs`,
`ui/resources.slint` (`ResourcePopover`, `Row`), graphs from `core sysstat::graph`.

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| Overlay layer, no keyboard, no input (mask empty) | Popover 267-276 | `Layer::Overlay`, `KeyboardInteractivity::None`, `set_input_region(&[])` | same |
| Placed centred under anchorX, top = anchorY − shadow margin 24, kept on screen | 270-273 | `place()` (tested) | same |
| Size: 300 content + 2×14 pad + 2×24 margin; rows 48 high, 2 apart | 260-275, Row 365-366 | `size()` (tested) | same |
| Fades in and drops 6 px over Theme.motion.popover OutCubic | 279-286 | `progress` animate `Theme.popover`, cubic-bezier(.33,1,.68,1); y `(p−1)×6` | same |
| Closing: gone at once (LazyLoader inactive) | ResourceHover 231-232 | `close()` hides and drops the surface | same |
| Glass: lens 0.35, popover radius | 288-293 | glass shape lens 0.35, radius 18 | same |
| Group highlighted: cpu/temperature → processor, gpu → graphics, memory/swap → memory, net* → network | 264 | `stats::group_of` | same |
| Row: badge circle 28 (Material 32) in the tone at 18 % (Material role container), icon 15/16 filled | Row 376-391 | `Row` badge | same |
| Title 12/600 (Material 14/500), detail caption secondary; value body/500, network caption with ↓/↑ | Row 392-428 | `Row` texts | same (no tabular figures) |
| Highlight: radius 10 (Material 16), white 8 % dark / 35 % light (Material surfaceContainerHighest) | Row 371-375 | `Row` background | same |
| Graph 76×26, area/bars/line per `resources.graphStyle`, line 1.25; network one scale for both lines, upload as a line | Row 430-452, Graph | `sysstat::graph`, `net_top` | same |
| Material tones: blue/indigo/pink → primary, green/teal → tertiary, purple/orange → secondary | ResourceTones 536 | `stats::role` | same |
| Live: values and graphs update once a second while shown | Resources | 1 s Tick → `refresh()` | same; fixed: rows now change in place (`Rows`), not a new model each second (it rebuilt every row every second) |
| Graphs already full when it opens (sampling while an island is open or the bar shows the figures) | Resources.desktopUsers / menubarActive | `warm` from the menu bar + `bar_figures` | same (fixed) |
| Network extra (speed test card) | 309-316 | — | missing (nothing passes networkExtra in the QML today either) |

Not possible: tabular figures.

## Test plan (Sonnet tester)

Use the steps of ResourceHover.qml.md / IslandStatus.qml.md to open it (island open, hover the CPU figure 0.7 s). Then:
1. `shot pop`: 3–4 rows (Graphics only with an AMD/Intel GPU reading), each with a round tinted badge, title, detail ("NN°C" under Processor, "N.N of N GB" under Memory), the value right-aligned and a filled-area graph that already spans most of its width (the island was open for a few seconds first: hover the island ≥ 10 s before the figure to check).
2. Stay 3 s and take two shots 1.5 s apart: the figures and the rightmost graph point change; nothing flickers/blinks between (no row rebuilt; `tools/region-changes.py` on a 3 s recording of the popover region shows only the figure/graph pixels changing).
3. Hover memory: the Memory row is the highlighted one. Hover the thermometer: Processor.
4. Material: badges in primary/secondary/tertiary containers, highlighted row surface-container-highest with 16 px corners. Glass: glass panel, white highlight.
5. Record opening (`rec pop-in 1`): fade + 6 px drop over ~Theme.motion.popover (expressive: compare with QML recording).

## Test results

## Fixes after testing (fixer, 7 Nov, batch 3 build)

- Appeared in one frame: the window's `progress` was first read already shown; it is now read hidden first, so it fades in and drops 6 px over Theme.motion.popover.

Retest the FAIL steps above.

# shell/modules/resources/ResourceHover.qml

When to show the resources popover: half a second's rest on a figure opens
it under the figure, 120 ms after leaving it closes. Used by the island's
CPU / temperature / memory figures (IslandStatus) and the menu bar's
resources item (MenuBar.qml 453, not while its menu is open).

Rust: the timers are in `surfaces/menubar/ui/menubar.slint` (Bar: `res-want`,
`res-open`, `res-open-timer` 500 ms, close timer 120 ms, `res-hover()`),
fired from the island (`stat-hover`) and the bar's items (`hovered` of the
item with id `resources`); `res-popover(key, open, x, y)` → lib.rs
`M::ResPopover` → the resources part (`surfaces/resources/src/lib.rs`,
`{"open":true,"key","x","y","screen"}` / `{"open":false,…}`).

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| Open 500 ms after the hover starts (restarted for each figure) | 204-213 | `res-open-timer` 500 ms, `restart()` when the key changes | same (fixed; was missing everywhere) |
| Close 120 ms after the hover ends | 204, 214-218 | 120 ms Timer `res-open != "" && res-want != res-open` | same (fixed) |
| Anchored at the target's bottom centre + gap (island 14, bar item 8) | 198, 209-210 | island: figure bottom + 14; bar: x = item centre, y = 30 + 8 | same (fixed) |
| Bar item: only while its menu isn't open (`!parent.active`) | MenuBar 456 | `res-hover("cpu", on && !e.active, …)`; a click closes it | same (fixed; it showed a plain "CPU · memory" tip instead) |
| Takes no room / no input; the popover is its own window | 199-200 | the resources part's overlay layer with an empty input region | same |
| Moving to another figure: the old one closes at 120 ms, the new opens at 500 ms | per-figure timers | close timer then open timer (both run) | same |
| The island closing under the pointer closes it | IslandStatus 61 (`root.shown`) | `changed expanded` → `stat-hover("*")` clears `res-want` | same (fixed) |

## Test plan (Sonnet tester)

Arena with the user's settings, `--bin` the fixer's binary, both looks; screen 1920×1080 scale 1.

1. Island: covered by IslandStatus.qml.md steps 3–5 (hover each figure; open at ~500 ms, close at ~120 ms). Record `rec hover-cpu 1.5` while gliding onto the CPU figure and staying: measure with `tools/anim-timing.py` that the popover's first frame comes 500 ± 50 ms after the pointer arrives (QML the same).
2. Menu bar item: set `menubar.resources: true` in the arena's config (Settings → Menu bar → Resources, or edit config.json and reload). `shot bar` to find the "NN% · NN%" item with a CPU icon on the right of the bar. Glide onto it (`P glide <x> 15 200`), wait 0.7 s, `shot bar-pop`: the resources popover under it (top ≈ 38 px), Processor row highlighted. No small text tip "CPU · memory".
3. Click the item: the Resources status menu opens and the popover goes. Hovering the item while its menu is open shows no popover.
4. Glide away: the popover closes within ~120 ms.

## Test results

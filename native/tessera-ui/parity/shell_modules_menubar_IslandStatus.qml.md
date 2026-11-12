# shell/modules/menubar/IslandStatus.qml

The open island's last line: battery, CPU, temperature, memory — each an
icon (or the drawn battery) and its figure. Rust: `surfaces/menubar/ui/island.slint`
(`IslandStatus`, `StatFigure`), data from `surfaces/menubar/src/lib.rs`
(`island_stats`), popover and tip timers in `ui/menubar.slint` (`res-hover`,
`tip-*`), the popover itself in `surfaces/resources`.

Before this round the Rust line was one text string ("Battery 64% · CPU 12%
· Memory 40%"): no icons, no tint, no stagger, no hover. It's now a port.

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| Shows battery (only with a laptop battery), CPU, temperature (only with a reading), memory, in that order | 45 | island.slint `IslandStatus` (`if s.battery`, `if s.temp != ""`) | same (fixed) |
| Centred across the island, 14 px between figures, 4 px icon→text | 41-42, 54 | `HorizontalLayout { spacing: 14px }`, `StatFigure` row spacing 4px | same (fixed) |
| Battery: the drawn BatteryGlyph at size 10, charging bolt below 100 %, plug at 100 % on power | 73-81 | `BatteryGlyph { size: 10px }` (moved from menubar.slint to island.slint, `size` now an input) | same (fixed) |
| Icons: Phosphor `cpu`, `thermometer-simple`, `memory`, 13 px (Material: the Material Symbol, 1.2×) | 82-88 | `SymIcon { font-size: 13px }`, glyphs from lib.rs `island_stats` | same (fixed) |
| Figures: `NN%`, temperature `NN°`, small font | 89-94 | `Text { font-size: Theme.small }` | different: no tabular figures (Slint has no font features) |
| Colour: IslandInk.dim(0.6) (Glass white 60 %, Material onSurfaceVariant) | 53 | `dim: Theme.material ? m3-on-surface-variant : #ffffff99` | same (fixed) |
| Temperature turns red while running hot (Island.hot): Material error, Glass rgba(1,.55,.5,.85) | 52-53 | `tint: s.hot ? hot-tint` (`#ff8c80d9` / m3-error), `hot` from state `island.hot` | same (fixed) |
| Staggered entry: after `Island.activities.length × 45 ms`, fades in and rises 8 px over 280 ms OutCubic; no delay going out | 29-37, IslandView 734 | `appear` animate `delay: shown ? delay : 0`, 280 ms cubic-bezier(.33,1,.68,1); `y: (1-appear)*8px`; `act-n: live-count` | same (fixed) |
| Sampling CPU/memory while shown (Resources.desktopUsers) | 19-27 | lib.rs asks `resources.use desktop` while any island is expanded | same |
| Hover CPU / temperature / memory 500 ms → the resources popover under the figure (gap 14), the matching row highlighted; leave → closes 120 ms later | 55-64, ResourceHover | figure `TouchArea.has-hover` → island `stat-hover` → Bar `res-hover` (500 ms / 120 ms timers, restart on a new figure) → `res-popover` → lib.rs `M::ResPopover` → resources part `{"open":true,"key",x,y,screen}` | same (fixed; was missing) |
| Popover only while the island is open (`root.shown`) | 61 | `stat-hover(k, on && expanded…)`, and `changed expanded` sends `"*"` off | same (fixed) |
| Hover the battery 500 ms → a tip with BatteryHealth.summary() ("64% · 1 h 30 min left · Balanced") under it, gap 14 | 66-72 | Bar tip with `tip-top = y + 14`, text `battery-tip` (state `battery.summary`, else items::battery_tip) | same (fixed; was missing) |
| The popover's graphs already hold the last minute when it opens | Resources.qml desktopUsers | resources part samples while `warm` (an island is open) | same (fixed; was empty for a minute) |

Not possible in Slint: tabular figures (the digits are a little narrower).

## Test plan (Sonnet tester)

Setup: `tools/live-scenarios.sh --bin <worktree>/native/tessera-ui/target/release/tessera-ui --arena N --only rust` with the user's settings, once `--look material`, once `--look glass`. Test screen IRICE-TEST 1920×1080 at scale 1; the island is centred at x = 960, the bar 30 px tall. Do the same steps with `--only qml` to record the QML timings.

1. Rest: `P move 1500 600`, wait 1 s. Then `P glide 960 14 400`, wait 1.5 s, `shot status-open`.
   Must see: the island open (392 px wide), its last line centred: a CPU icon and `NN%`, a thermometer icon and `NN°` (if the sandbox has a reading), a memory icon and `NN%` (and a drawn battery + `NN%` first if the machine has a battery). Icons, not words. Dim colour (Material: on-surface-variant grey; Glass: 60 % white).
2. Record the opening: `rec status-in 1.2` while gliding from (1500,600) to (960,14) in 400 ms. In the recording the status line fades in and rises ~8 px over ~280 ms, starting when the island opens (no activities: no delay). Compare with the QML recording (`tools/anim-timing.py`).
3. Find the CPU figure's x from `status-open.png` (≈ 920–940 at y ≈ 50). Glide onto it: `P glide <cpuX> 50 200`, wait 0.3 s → `shot pop-none` (no popover yet), wait 0.5 s more → `shot pop-cpu`: a popover (300 px content, rows Processor/Graphics?/Memory/Network, each with a badge icon, figures and a graph) hangs centred under the CPU figure, 14 px below it; the Processor row highlighted. Its graphs are already drawn (not empty) because sampling ran while the island was open.
4. Glide to the memory figure (same y): ~120 ms later the popover closes, ~500 ms after arriving it reopens under memory with the Memory row highlighted (`shot pop-mem`). Then the thermometer: Processor row highlighted.
5. Glide down out of the island `P glide 960 600 300`: the popover is gone within ~120 ms and the island closes (260 ms after leaving). Must not stay on screen.
6. Battery (only on a machine with one): hover the battery figure 0.7 s → a small tip "NN% · … left" (Material: dark inverse-surface tooltip with 4 px corners; Glass: glass tip) 14 px under it.
7. Running hot: `ipc island hot true` with the island hovered open: the temperature figure turns red (Material error colour / Glass light red). `ipc island hot false` restores.
8. Repeat 1–5 with the Glass look.

## Test results

## Fixes after testing (fixer, 7 Nov, batch 3 build)

- Temperature: host/system's Mirror now publishes `resources.temperature` (it published only CPU and memory), so the thermometer figure shows, and turns red while hot.
- Too tall / status low: the open island's body filled the island and spread its rows (the status line sat ~11 px low); it's now as tall as its content (`height: preferred`, `alignment: start`).
- No fade/rise: the status line's entrance now runs on its own clock from the moment it's shown (fade + 8 px rise, 280 ms OutCubic after the activities' delay); the animated property had settled while the body was hidden.
- Opening speed: the island's size springs are shorter (Glass 320 ms bounce 0.22, Material 300 ms bounce 0.12; were 480 ms) to match the QML's ~200 ms to settle: tester to re-time.

Retest the FAIL steps above.

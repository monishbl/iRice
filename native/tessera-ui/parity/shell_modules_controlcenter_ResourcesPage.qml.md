# shell/modules/controlcenter/ResourcesPage.qml

CPU & Memory page. Rust: host/system/src/accounts/cc.rs resources_page (one `graph` block per reading), surfaces/controlcenter/ui/controlcenter.slint Block kind 10, bridge.rs graph_path.

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| one row per reading: CPU, memory, swap, temperature, network, GPU | 18-40 | resources_page | fixed (two text rows before) |
| label, value, detail | ResourceRow | graph block | same |
| the last minute as a filled line, scaled to its max | ResourceRow | graph_path (60 × 100 box) | same |
| sampled once a second while shown | 18-40 | host/system/src/accounts/cc.rs resources.use "graph" on page show/hide/close | fixed |
| speed test slot (networkExtra) | 16, 32-37 | — | missing |
| footer note | 46-54 | note | same |

## Fixed in batch 2

- graphs per reading
- sampling while shown

## Still different / missing

- speed test slot

## Tester plan

Arena with the user's settings: copy `tools/live-scenarios.sh`, run with
`--arena N --only rust --bin <this worktree>/native/tessera-ui/target/release/tessera-ui`,
both `--look material` and `--look glass`; the QML for comparison with `--only qml`.
Open the Control Center: `ipc panels open controlCenter IRICE-TEST`; read
rectangles from a `dev shot` first. Pointer: `dev pointer -o IRICE-TEST move|glide|click|down|up|scroll`;
keys: `dev run wtype …`.

1. Open CC → right-click CPU & Memory (or its tile's button in Material): rows CPU, Memory, Swap, Temperature (when read), Download/Upload, GPU (when reported), each with a graph.
2. Watch 10 s: the graphs move left once a second; run `dev run stress --cpu 2 -t 5` (or a busy loop): the CPU line climbs.
3. Leave the page: sampling stops (Resources bus `active` false in the log unless the bar shows them).

## Test results

(tester)

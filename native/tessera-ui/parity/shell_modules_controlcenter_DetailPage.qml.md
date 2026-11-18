# shell/modules/controlcenter/DetailPage.qml

A page's header and body. Rust: `PageHeader`/`PageView` (surfaces/controlcenter/ui/controlcenter.slint ~1690-1960).

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| Glass refresh spin 500 ms | 58-84 | PageHeader | same |
| Glass title elides | 52-57 | PageHeader | fixed |
| M3 refresh spin | 119-134 | GhostButton turn | fixed |
| M3 switch bar | 140-172 | PageHeader | same |
| hovered → lists hold still | 20-22 | PageView.hovered → cc.hover | fixed (see WifiList) |

## Fixed in batch 2

- title elide
- M3 refresh spin
- hover tells the host

## Tester plan

Arena with the user's settings: copy `tools/live-scenarios.sh`, run with
`--arena N --only rust --bin <this worktree>/native/tessera-ui/target/release/tessera-ui`,
both `--look material` and `--look glass`; the QML for comparison with `--only qml`.
Open the Control Center: `ipc panels open controlCenter TESSERA-TEST`; read
rectangles from a `dev shot` first. Pointer: `dev pointer -o TESSERA-TEST move|glide|click|down|up|scroll`;
keys: `dev run wtype …`.

1. Material: open Wi-Fi page, click refresh: the arrow turns 360° in 500 ms.
2. Glass: a page with a long title elides with …

## Test results

(tester)

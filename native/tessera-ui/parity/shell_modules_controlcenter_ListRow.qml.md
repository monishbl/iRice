# shell/modules/controlcenter/ListRow.qml

A page row. Rust: `Block` kind 1 (surfaces/controlcenter/ui/controlcenter.slint).

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| group corners, tint, inner radius spring | 35-124 | Block row | same |
| Space/Enter acts | 79-80 | surfaces/controlcenter/src/lib.rs page_key | same |
| Menu key / Shift+F10 → menu | 81-82 | surfaces/controlcenter/src/lib.rs page_key | fixed Shift+F10; menu at panel+100 (different place) |
| accessible list item | 93-97 | row accessible-* | fixed |
| right-click → menu | 213-215 | rt | same |
| press and hold 550 ms → menu, no click | 210-224 | row-hold Timer | fixed |
| busy spinner | 186-203 | Spin | same (M3 spinner look differs) |
| ElasticControl press | 113-114 | — | missing |

## Fixed in batch 2

- a11y
- hold for menu
- Shift+F10

## Still different / missing

- menu at the row from the keyboard
- elastic press

## Tester plan

Arena with the user's settings: copy `tools/live-scenarios.sh`, run with
`--arena N --only rust --bin <this worktree>/native/tessera-ui/target/release/tessera-ui`,
both `--look material` and `--look glass`; the QML for comparison with `--only qml`.
Open the Control Center: `ipc panels open controlCenter IRICE-TEST`; read
rectangles from a `dev shot` first. Pointer: `dev pointer -o IRICE-TEST move|glide|click|down|up|scroll`;
keys: `dev run wtype …`.

1. Wi-Fi page: hold a network row 1 s: its menu opens, the row does not connect on release.
2. Right-click a row: its menu. Tab to a row, Shift+F10: its menu.

## Test results

(tester)

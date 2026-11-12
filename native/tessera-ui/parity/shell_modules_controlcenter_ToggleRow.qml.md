# shell/modules/controlcenter/ToggleRow.qml

A grid tile helper (circle + label opening its page); not part of pages. Rust: the tile paths.

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| label opens page, circle toggles | all | surfaces/controlcenter/src/lib.rs secondary/act | same |

## Tester plan

Arena with the user's settings: copy `tools/live-scenarios.sh`, run with
`--arena N --only rust --bin <this worktree>/native/tessera-ui/target/release/tessera-ui`,
both `--look material` and `--look glass`; the QML for comparison with `--only qml`.
Open the Control Center: `ipc panels open controlCenter IRICE-TEST`; read
rectangles from a `dev shot` first. Pointer: `dev pointer -o IRICE-TEST move|glide|click|down|up|scroll`;
keys: `dev run wtype …`.

1. Covered by ModuleView/QSTile plans.

## Test results

(tester)

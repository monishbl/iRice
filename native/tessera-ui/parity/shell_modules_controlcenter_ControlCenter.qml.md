# shell/modules/controlcenter/ControlCenter.qml

Scope: RustBridge always, windows lazy. Rust: the host service host/system/src/accounts/cc.rs + the surface.

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| bridge alive, windows lazy | 13-20 | host service | same role |

## Tester plan

Arena with the user's settings: copy `tools/live-scenarios.sh`, run with
`--arena N --only rust --bin <this worktree>/native/tessera-ui/target/release/tessera-ui`,
both `--look material` and `--look glass`; the QML for comparison with `--only qml`.
Open the Control Center: `ipc panels open controlCenter IRICE-TEST`; read
rectangles from a `dev shot` first. Pointer: `dev pointer -o IRICE-TEST move|glide|click|down|up|scroll`;
keys: `dev run wtype …`.

1. Covered by the other files' plans.

## Test results

(tester)

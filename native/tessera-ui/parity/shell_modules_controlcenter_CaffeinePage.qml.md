# shell/modules/controlcenter/CaffeinePage.qml

Caffeine. Rust: host/system/src/accounts/cc.rs.

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| switch, rows, note | all | host | same |

## Tester plan

Arena with the user's settings: copy `tools/live-scenarios.sh`, run with
`--arena N --only rust --bin <this worktree>/native/tessera-ui/target/release/tessera-ui`,
both `--look material` and `--look glass`; the QML for comparison with `--only qml`.
Open the Control Center: `ipc panels open controlCenter IRICE-TEST`; read
rectangles from a `dev shot` first. Pointer: `dev pointer -o IRICE-TEST move|glide|click|down|up|scroll`;
keys: `dev run wtype …`.

1. Caffeine page: switch and durations work.

## Test results

(tester)

# shell/modules/controlcenter/BluetoothPage.qml

Bluetooth header. Rust: host/system/src/accounts/cc.rs bt_page.

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| switch/refresh when ok | 16-20 | host | same (also when state unknown and an adapter exists) |

## Tester plan

Arena with the user's settings: copy `tools/live-scenarios.sh`, run with
`--arena N --only rust --bin <this worktree>/native/tessera-ui/target/release/tessera-ui`,
both `--look material` and `--look glass`; the QML for comparison with `--only qml`.
Open the Control Center: `ipc panels open controlCenter TESSERA-TEST`; read
rectangles from a `dev shot` first. Pointer: `dev pointer -o TESSERA-TEST move|glide|click|down|up|scroll`;
keys: `dev run wtype …`.

1. Bluetooth page: switch powers off/on.

## Test results

(tester)

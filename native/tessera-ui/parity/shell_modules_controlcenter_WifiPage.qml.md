# shell/modules/controlcenter/WifiPage.qml

Wi-Fi page header. Rust: host/system/src/accounts/cc.rs wifi_page.

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| switch when managed, toggles | 15-17 | host | same |
| refresh → rescan | 18-19 | host | same |
| cap 360 | 24 | surfaces/controlcenter/src/lib.rs | same |

## Tester plan

Arena with the user's settings: copy `tools/live-scenarios.sh`, run with
`--arena N --only rust --bin <this worktree>/native/tessera-ui/target/release/tessera-ui`,
both `--look material` and `--look glass`; the QML for comparison with `--only qml`.
Open the Control Center: `ipc panels open controlCenter TESSERA-TEST`; read
rectangles from a `dev shot` first. Pointer: `dev pointer -o TESSERA-TEST move|glide|click|down|up|scroll`;
keys: `dev run wtype …`.

1. Wi-Fi page: switch turns Wi-Fi off/on; refresh rescans.

## Test results

(tester)

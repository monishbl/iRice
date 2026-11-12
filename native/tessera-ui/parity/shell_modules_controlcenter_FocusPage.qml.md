# shell/modules/controlcenter/FocusPage.qml

Focus modes. Rust: host/system/src/accounts/cc.rs.

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| mode rows, click, durations, settings | 12-52 | host | same; schedule times raw HH:MM (different) |

## Still different / missing

- locale clock for schedule times

## Tester plan

Arena with the user's settings: copy `tools/live-scenarios.sh`, run with
`--arena N --only rust --bin <this worktree>/native/tessera-ui/target/release/tessera-ui`,
both `--look material` and `--look glass`; the QML for comparison with `--only qml`.
Open the Control Center: `ipc panels open controlCenter IRICE-TEST`; read
rectangles from a `dev shot` first. Pointer: `dev pointer -o IRICE-TEST move|glide|click|down|up|scroll`;
keys: `dev run wtype …`.

1. Focus page: click a mode: on; the "Turn on" segmented sets its duration.

## Test results

(tester)

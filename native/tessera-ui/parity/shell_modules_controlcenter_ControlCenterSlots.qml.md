# shell/modules/controlcenter/ControlCenterSlots.qml

One window per screen, look-specific. Rust: one CC surface, recreated on another output (surfaces/controlcenter/src/lib.rs window()).

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| window per screen / per look | 8-35 | surfaces/controlcenter/src/lib.rs window | different by design (one surface moved to the screen asked) |

## Tester plan

Arena with the user's settings: copy `tools/live-scenarios.sh`, run with
`--arena N --only rust --bin <this worktree>/native/tessera-ui/target/release/tessera-ui`,
both `--look material` and `--look glass`; the QML for comparison with `--only qml`.
Open the Control Center: `ipc panels open controlCenter TESSERA-TEST`; read
rectangles from a `dev shot` first. Pointer: `dev pointer -o TESSERA-TEST move|glide|click|down|up|scroll`;
keys: `dev run wtype …`.

1. Open CC on each output (`ipc panels open controlCenter <name>`): it shows on that screen.

## Test results

(tester)

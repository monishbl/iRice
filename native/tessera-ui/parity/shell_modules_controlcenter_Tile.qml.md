# shell/modules/controlcenter/Tile.qml

Not instantiated anywhere in the shell (only its own file). Nothing to port; GlassModule covers its role.

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| 2×1 toggle + label opening a page | all | GlassModule | n/a |

## Tester plan

Arena with the user's settings: copy `tools/live-scenarios.sh`, run with
`--arena N --only rust --bin <this worktree>/native/tessera-ui/target/release/tessera-ui`,
both `--look material` and `--look glass`; the QML for comparison with `--only qml`.
Open the Control Center: `ipc panels open controlCenter IRICE-TEST`; read
rectangles from a `dev shot` first. Pointer: `dev pointer -o IRICE-TEST move|glide|click|down|up|scroll`;
keys: `dev run wtype …`.

1. None (unused).

## Test results

(tester)

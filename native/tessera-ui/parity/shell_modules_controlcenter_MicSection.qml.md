# shell/modules/controlcenter/MicSection.qml

Mic section (also under Sound). Rust: bridge.rs.

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| slider + inputs | all | bridge.rs | same |

## Tester plan

Arena with the user's settings: copy `tools/live-scenarios.sh`, run with
`--arena N --only rust --bin <this worktree>/native/tessera-ui/target/release/tessera-ui`,
both `--look material` and `--look glass`; the QML for comparison with `--only qml`.
Open the Control Center: `ipc panels open controlCenter TESSERA-TEST`; read
rectangles from a `dev shot` first. Pointer: `dev pointer -o TESSERA-TEST move|glide|click|down|up|scroll`;
keys: `dev run wtype …`.

1. As MicPage.

## Test results

(tester)

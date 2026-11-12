# shell/modules/controlcenter/MicPage.qml

Microphone page. Rust: surfaces/controlcenter/src/bridge.rs mic_page.

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| title, slider (unmutes), inputs | all | bridge.rs | same; slider rows kept (batch 1) |

## Tester plan

Arena with the user's settings: copy `tools/live-scenarios.sh`, run with
`--arena N --only rust --bin <this worktree>/native/tessera-ui/target/release/tessera-ui`,
both `--look material` and `--look glass`; the QML for comparison with `--only qml`.
Open the Control Center: `ipc panels open controlCenter IRICE-TEST`; read
rectangles from a `dev shot` first. Pointer: `dev pointer -o IRICE-TEST move|glide|click|down|up|scroll`;
keys: `dev run wtype …`.

1. Mic page: drag the slider end to end; click an input: it becomes default.

## Test results

(tester)

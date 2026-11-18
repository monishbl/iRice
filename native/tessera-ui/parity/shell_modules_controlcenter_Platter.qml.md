# shell/modules/controlcenter/Platter.qml

Glass module background. Rust: GlassModule background, GlassFooterButton (surfaces/controlcenter/ui/controlcenter.slint).

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| radius clamp | 13 | shape-radius | same |
| chip colours white 0.07/0.28 else platter | 12 | idle | same |
| chip glass from the compositor | 18-26 | surfaces/controlcenter/src/lib.rs after_frame | same idea |
| squircle corners | 27-34 | Rectangle | different (Slint) |
| chip border | 32-33 | border | same |

## Still different / missing

- squircle

## Tester plan

Arena with the user's settings: copy `tools/live-scenarios.sh`, run with
`--arena N --only rust --bin <this worktree>/native/tessera-ui/target/release/tessera-ui`,
both `--look material` and `--look glass`; the QML for comparison with `--only qml`.
Open the Control Center: `ipc panels open controlCenter TESSERA-TEST`; read
rectangles from a `dev shot` first. Pointer: `dev pointer -o TESSERA-TEST move|glide|click|down|up|scroll`;
keys: `dev run wtype …`.

1. Glass with bare chips: shot matches QML (borders, colours).

## Test results

(tester)

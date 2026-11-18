# shell/modules/controlcenter/CircleToggle.qml

Glass 2×1 pill's round button. Rust: GlassModule pill button (surfaces/controlcenter/ui/controlcenter.slint ~410-435).

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| size, position, colours, 150 ms | 18-21 | pill button | same |
| ElasticControl squish (motion setting) + follow | 22, 56-59 | scale 0.95 linear | different: no follow, motion setting ignored |
| click acts | 47-54 | pill TouchArea | same |
| own a11y node | 30-34 | — | missing (module node only) |

## Still different / missing

- elastic follow, motion setting

## Tester plan

Arena with the user's settings: copy `tools/live-scenarios.sh`, run with
`--arena N --only rust --bin <this worktree>/native/tessera-ui/target/release/tessera-ui`,
both `--look material` and `--look glass`; the QML for comparison with `--only qml`.
Open the Control Center: `ipc panels open controlCenter TESSERA-TEST`; read
rectangles from a `dev shot` first. Pointer: `dev pointer -o TESSERA-TEST move|glide|click|down|up|scroll`;
keys: `dev run wtype …`.

1. Glass: click the round part of a 2×1 pill (Wi-Fi): toggles; press shows 0.95 squish.

## Test results

(tester)

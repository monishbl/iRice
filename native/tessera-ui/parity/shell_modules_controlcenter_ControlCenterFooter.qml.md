# shell/modules/controlcenter/ControlCenterFooter.qml

Footer (you, battery, Settings, Power, Edit). Rust: Glass/Material footer (surfaces/controlcenter/ui/controlcenter.slint), surfaces/controlcenter/src/lib.rs key().

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| button colours/width/hover | 36-39 | GlassFooterButton / TonalButton | same |
| squish | 40, 76-79 | scale | different |
| Tab stops, Space/Enter, FocusRing | 41-53 | surfaces/controlcenter/src/lib.rs key (extras after the modules), kbd-focus ring | fixed |
| avatar, battery glyph | 83-124 | footer | same; bolt at 100% while plugged (different) |
| Settings / Power / Edit | 132-145 | surfaces/controlcenter/src/lib.rs | same |

## Fixed in batch 2

- keyboard stops

## Still different / missing

- elastic squish

## Tester plan

Arena with the user's settings: copy `tools/live-scenarios.sh`, run with
`--arena N --only rust --bin <this worktree>/native/tessera-ui/target/release/tessera-ui`,
both `--look material` and `--look glass`; the QML for comparison with `--only qml`.
Open the Control Center: `ipc panels open controlCenter IRICE-TEST`; read
rectangles from a `dev shot` first. Pointer: `dev pointer -o IRICE-TEST move|glide|click|down|up|scroll`;
keys: `dev run wtype …`.

1. Open CC, Tab past the last module: the ring lands on Settings, then Power, then Edit; Enter on each acts. Left/Right move between them.

## Test results

(tester)

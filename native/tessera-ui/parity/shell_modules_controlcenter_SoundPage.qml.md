# shell/modules/controlcenter/SoundPage.qml

Sound page. Rust: bridge.rs sound_page, lib.rs block/block_slide.

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| outputs, profiles, applications, mic | 24-249 | bridge.rs | same |
| rows kept while dragging | 33, 69 | live::set_blocks | fixed in batch 1 |
| app icon | 116-121 | generic | different |
| per-app output chooser | 75-240 | — | missing |
| AirPlay status | 36-40 | — | missing |

## Still different / missing

- per-app output
- app icons
- AirPlay

## Tester plan

Arena with the user's settings: copy `tools/live-scenarios.sh`, run with
`--arena N --only rust --bin <this worktree>/native/tessera-ui/target/release/tessera-ui`,
both `--look material` and `--look glass`; the QML for comparison with `--only qml`.
Open the Control Center: `ipc panels open controlCenter IRICE-TEST`; read
rectangles from a `dev shot` first. Pointer: `dev pointer -o IRICE-TEST move|glide|click|down|up|scroll`;
keys: `dev run wtype …`.

1. See CCSlider plan.

## Test results

(tester)

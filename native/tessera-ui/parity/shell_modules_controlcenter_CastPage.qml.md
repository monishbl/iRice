# shell/modules/controlcenter/CastPage.qml

Cast. Rust: host/system/src/accounts/cc.rs.

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| header text, rows, avahi, none-found | 14-67 | host | same |
| row icon Cast.iconFor | 34 | host/system/src/accounts/cc.rs | fixed: speaker by model, else TV |

## Fixed in batch 2

- icons

## Tester plan

Arena with the user's settings: copy `tools/live-scenarios.sh`, run with
`--arena N --only rust --bin <this worktree>/native/tessera-ui/target/release/tessera-ui`,
both `--look material` and `--look glass`; the QML for comparison with `--only qml`.
Open the Control Center: `ipc panels open controlCenter IRICE-TEST`; read
rectangles from a `dev shot` first. Pointer: `dev pointer -o IRICE-TEST move|glide|click|down|up|scroll`;
keys: `dev run wtype …`.

1. Cast page with a discovered device: icon TV or speaker.

## Test results

(tester)

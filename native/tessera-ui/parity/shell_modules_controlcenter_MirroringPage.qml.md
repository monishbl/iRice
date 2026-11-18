# shell/modules/controlcenter/MirroringPage.qml

Screen mirroring. Rust: host/system/src/accounts/cc.rs.

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| look while visible / stop on close | 13-22 | host/system/src/accounts/cc.rs | fixed (stops on CC close) |
| protocol name | 24-37 | host/system/src/accounts/cc.rs | fixed: "Miracast (network)" etc. |

## Fixed in batch 2

- stop on close
- protocol names

## Tester plan

Arena with the user's settings: copy `tools/live-scenarios.sh`, run with
`--arena N --only rust --bin <this worktree>/native/tessera-ui/target/release/tessera-ui`,
both `--look material` and `--look glass`; the QML for comparison with `--only qml`.
Open the Control Center: `ipc panels open controlCenter TESSERA-TEST`; read
rectangles from a `dev shot` first. Pointer: `dev pointer -o TESSERA-TEST move|glide|click|down|up|scroll`;
keys: `dev run wtype …`.

1. Mirroring page rows show "Chromecast"/"Miracast (network)".

## Test results

(tester)

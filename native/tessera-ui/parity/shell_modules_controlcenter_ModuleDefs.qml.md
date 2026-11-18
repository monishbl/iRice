# shell/modules/controlcenter/ModuleDefs.qml

The module table and layout rules. Rust: surfaces/controlcenter/src/defs.rs, host/system/src/accounts/cc.rs.

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| all defs, labels, kinds, sizes, pages | 21-72 | defs.rs / host defs | same |
| live label/icon/on/status | 23-71 | host/system/src/accounts/cc.rs 1 s tick + after a click | different: changes from elsewhere up to 1 s late |
| act per module | 23-63 | host/system/src/accounts/cc.rs module_act, surfaces/controlcenter/src/lib.rs mic | same |
| layout parse/save/remove/cycle/add/move | 74-112, 188-202 | defs.rs | same |
| nearestSlot/Toward, pack | 121-233 | defs.rs | same |
| slider values / setSlider quiet | 142-152 | surfaces/controlcenter/src/lib.rs slide | same (batch 1 hold) |

## Still different / missing

- state polled 1 s

## Tester plan

Arena with the user's settings: copy `tools/live-scenarios.sh`, run with
`--arena N --only rust --bin <this worktree>/native/tessera-ui/target/release/tessera-ui`,
both `--look material` and `--look glass`; the QML for comparison with `--only qml`.
Open the Control Center: `ipc panels open controlCenter TESSERA-TEST`; read
rectangles from a `dev shot` first. Pointer: `dev pointer -o TESSERA-TEST move|glide|click|down|up|scroll`;
keys: `dev run wtype …`.

1. Toggle Wi-Fi from a terminal (`nmcli radio wifi off`) with CC open: the tile follows within ~1 s (QML: at once).

## Test results

(tester)

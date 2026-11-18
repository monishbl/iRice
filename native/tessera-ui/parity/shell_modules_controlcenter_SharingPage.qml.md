# shell/modules/controlcenter/SharingPage.qml

Nearby sharing. Rust: host/system/src/accounts/cc.rs.

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| switch, segmented, name field, rows | 13-108 | host | same |
| look(false) when the page goes (incl. closing CC) | 19-28 | host/system/src/accounts/cc.rs open_edge | fixed: stops looking when CC closes |

## Fixed in batch 2

- stop looking on close

## Tester plan

Arena with the user's settings: copy `tools/live-scenarios.sh`, run with
`--arena N --only rust --bin <this worktree>/native/tessera-ui/target/release/tessera-ui`,
both `--look material` and `--look glass`; the QML for comparison with `--only qml`.
Open the Control Center: `ipc panels open controlCenter TESSERA-TEST`; read
rectangles from a `dev shot` first. Pointer: `dev pointer -o TESSERA-TEST move|glide|click|down|up|scroll`;
keys: `dev run wtype …`.

1. Open Sharing page, close CC: the sharing service's "looking" stops (`ipc` state or log).

## Test results

(tester)

# shell/modules/controlcenter/ServiceNotice.qml

"X isn't working" notice. Rust: `Block` kind 6 (surfaces/controlcenter/ui/controlcenter.slint).

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| icon, title, text, button | 24-79 | Block 6 | same |
| click disabled while busy | 85 | nt enabled | fixed |
| keyboard focus | 63-72 | — | missing |
| error line | 89-97 | — | missing |

## Fixed in batch 2

- no repeat clicks while busy

## Still different / missing

- keyboard
- error line

## Tester plan

Arena with the user's settings: copy `tools/live-scenarios.sh`, run with
`--arena N --only rust --bin <this worktree>/native/tessera-ui/target/release/tessera-ui`,
both `--look material` and `--look glass`; the QML for comparison with `--only qml`.
Open the Control Center: `ipc panels open controlCenter TESSERA-TEST`; read
rectangles from a `dev shot` first. Pointer: `dev pointer -o TESSERA-TEST move|glide|click|down|up|scroll`;
keys: `dev run wtype …`.

1. Stop bluetoothd in the arena; Bluetooth page shows the notice; click the fix: "Working…", further clicks do nothing until done.

## Test results

(tester)

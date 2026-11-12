# shell/modules/controlcenter/BluetoothList.qml

Bluetooth devices. Rust: host/system/src/accounts/cc.rs bt_page/bt_menu.

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| filter, sort, dedupe, top 40 | 28-44 | host | same |
| snapshot every 4 s unless hovered | 45-83 | host/system/src/accounts/cc.rs hold_order | fixed |
| discovery on while the list shows | 85-91 | host/system/src/accounts/cc.rs tick: scan renewed every 8 s | fixed |
| busy from device state | 209 | own commands only | different |
| status Pairing only when unpaired | 136-144 | host/system/src/accounts/cc.rs | fixed ("Connecting…" for paired) |
| notice rechecks every 3 s | 160-165 | host/system/src/accounts/cc.rs tick | fixed |
| trusted toggle state in menu | 118-121 | host menu | different |
| notice error line | 148-157 | — | missing |

## Fixed in batch 2

- hold still
- discovery while shown
- status wording
- notice recheck

## Still different / missing

- busy from BlueZ state
- trusted state
- error line

## Tester plan

Arena with the user's settings: copy `tools/live-scenarios.sh`, run with
`--arena N --only rust --bin <this worktree>/native/tessera-ui/target/release/tessera-ui`,
both `--look material` and `--look glass`; the QML for comparison with `--only qml`.
Open the Control Center: `ipc panels open controlCenter IRICE-TEST`; read
rectangles from a `dev shot` first. Pointer: `dev pointer -o IRICE-TEST move|glide|click|down|up|scroll`;
keys: `dev run wtype …`.

1. Bluetooth page open 20 s: `dev run bluetoothctl show | grep Discovering` says yes while it shows; close CC: discovery stops within ~9 s.
2. Pointer resting on the list: rows hold still.
3. Connect a paired device from the page: status "Connecting…" (not "Pairing…").

## Test results

(tester)

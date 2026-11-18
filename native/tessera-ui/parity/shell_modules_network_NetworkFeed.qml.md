# shell/modules/network/NetworkFeed.qml → surfaces/network/src/lib.rs (the shell side of the Wi-Fi menu)

NetworkFeed.qml is the shell's half: it tells tessera-ui the Wi-Fi state and does what the menu asks.

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | Networks as a snapshot, sorted connected → known → signal, at most 30 | 36-46 | `rows` (one VecModel kept for the window) | same |
| 2 | Refreshed every 4 s while open, but not while hovered, a row menu is open, or a password is typed | 39-62 | `hold <hovered> <asking>`, `hold_sent` | same |
| 3 | A failed join marks that network ("failed") | 80-90 | row status | same |
| 4 | Speed test state and strings forwarded | 100-148 | `push_speed` | same (stats now in place, see SpeedTestCard) |
| 5 | Menu x follows Panels.anchorX | 170-180 | anchor | same |
| 6 | Asks: speed test, switch, rescan, join, forget, Settings | 189-230 | `call` | same |

## Test plan (tester)
Both looks; open `inbox 'ipc panels open wifiMenu TESSERA-TEST'`.
1. With Wi-Fi in the arena (else "not testable here"): rows sorted connected first. Keep the pointer on the menu 10 s while networks change: rows hold still; move off → they refresh within 4 s.
2. Toggle the Wi-Fi switch → the list empties / comes back; no crash.
3. The menu opens under the menu bar's Wi-Fi item.

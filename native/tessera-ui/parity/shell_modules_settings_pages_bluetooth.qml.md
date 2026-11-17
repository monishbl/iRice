# shell/modules/settings/pages/bluetooth.qml ↔ surfaces/settings (pages/devices/network.rs `bluetooth`, `bt_list`, `bt_menu`; ui/settings-devices.slint `btn-switch` / `list`)

_Re-checked 17 Nov (B5, parity-settings) against main 18628a5 + the B5 ports: statuses below say what's there now; [12 Nov: …] is the old status._

A part of the Network page (PagePart; `open bluetooth` maps to network › Bluetooth,
map.rs 51/69). Embeds controlcenter/BluetoothList.qml twice (paired / nearby)
with ListRow.qml and PointerMenu.qml. Devices: tessera-ui's own BlueZ reading
(services/bluez.rs, `cx.want("bluez")`); extras (trusted, pairing, busy, picture,
adapter name) and every action: RustBridgeDevices.qml (`feed.bt`, `feed.backends`;
btEnable, btRescan, btFix, btConnect/Disconnect/Pair/CancelPair/Trust/Forget).
No QQC.Popup (the menu is PointerMenu → glass/ContextMenu).

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | `broken` = btState not "ok"/"" ; recheck every 3 s while shown and broken (22–28) | bridge Timer (page network, broken) | same |
| 2 | Row label: btTitle when broken, else "Bluetooth"; detail: reason / "Not available" / "On · this computer appears as “name”" / "Off" (34–35) | network.rs 784–802 | same |
| 3 | Fix button (primary) when broken and a fix exists: btFixLabel / "Working…", busy (38–45) | `btn-switch` style 1, busy | same |
| 4 | "Look again" (arrow-clockwise) when on → discovering off/on + refresh (46–52; BluetoothList 55–61) | `btn-switch` "button" → btRescan | same |
| 5 | Switch (hidden when broken; disabled without an adapter) (53–59) | r.from, r.enabled | same |
| 6 | Group footer = btFixError (32) | network.rs 814 | same |
| 7 | My devices / Nearby devices groups only when !broken && on, with their footers (64–103) | network.rs 816–825 | same |
| 8 | Discovery while the page shows and Bluetooth is on (BluetoothList 85–91) | bridge Binding discovering (page network) | same |
| 9 | Lists: paired / nearby (named), connected → paired → name, unpaired once per name, max 40 (BluetoothList 28–44) | `device_list` (unit-tested) | same |
| 10 | Rows are a snapshot, re-sorted every 4 s unless the pointer is over the list (or its menu is open); a vanished device dropped at once (BluetoothList 45–83) | rows follow bluez live | **same (fixed before 17 Nov: network.rs sorted_snapshot: re-sorted at most every 4 s, not while hovered or a menu is open)** [12 Nov: **missing**: rows reorder under the pointer] |
| 11 | Placeholders: "No devices yet. Pair one from Nearby devices." / "Looking for devices…" (184–197) | `bt_list` empty text | same |
| 12 | Row: device icon by BlueZ icon, or its picture (Material 72 / Glass 56 px source) with an accent dot when connected; name ‖ address; status Connected · N % / Pairing… / Not connected; busy spinner (202–212; ListRow 128–203) | `bt_list` 746–759, ListItem badge | same |
| 13 | Click: disconnect / connect / pair (97–104) | `bt_list` 761–765 | same |
| 14 | Menu: Disconnect/Connect, Stop pairing/Pair, (Don't) connect automatically, **separator**, Copy address, Forget device (danger) (108–134) | `bt_menu` 699–719 | **same (fixed before 17 Nov: separator above Copy address)** [12 Nov: **different**: no separator] |
| 15 | Right-click or press-and-hold 550 ms; menu with its left edge at the pointer (ListRow 205–225; PointerMenu 294) | ListItem right-click only, anchor centred on the pointer | **same (fixed before 17 Nov: ListItem hold 550 ms opens the menu (no click after); pointer menus open with their left edge at the pointer)** [12 Nov: **missing** hold; **different** position (see network rows 15, 17)] |
| 16 | Keyboard: Tab/↑/↓ between rows, Space/Enter, Menu key / Shift+F10 (ListRow 74–100) | — | **same (fixed before 17 Nov: DevList keys: Tab/↑/↓, Space/Enter, Menu/Shift+F10, ring on the keyed row)** [12 Nov: **missing** (same fix as network row 16)] |
| 17 | Material segmented rows, press swell (ListRow 107–126) | ListItem | **same (ported 17 Nov, B5: ListItem Material press swell (1+min(6 %, 10 px), spring back))** [12 Nov: different: no press swell (network row 12)] |
| 18 | Fast Pair: "Show earbuds nearby" switch (fastPair.enabled) + footer (106–113) | network.rs 827–831 | same |

## Test plan (Material window at 440,205; page column from x≈772)

Never click the Bluetooth switch, Look again, the fix button, or any device
(they act on the host's adapter). Look, hover, right-click + Esc only.

B1 Look: `setup qml material`, `open bluetooth`, shot; rust the same; both
looks. PASS: the page lands on Network scrolled to Bluetooth in both; the row
says the same (likely "Not available" or a broken-state title/reason in the
arena) with the same buttons visible (fix button primary, no switch when broken).

B2 Hover the fix / Look again button (read x,y), shot. PASS: hover state in both.
Do not click.

B3 Fast Pair: click the "Show earbuds nearby" switch twice (config only, safe).
PASS: it toggles and returns in both.

B4 If the arena shows devices (unlikely): hover a row (Material tint, inner
corners), right-click it, shot (PASS QML: menu at the pointer with a separator
before "Copy address"; Rust: report), Esc. Never press-and-hold in Rust.

## Fixed in batch 2 (fixer)

- The right-click menu opens at the pointer, with the separator before "Copy address".
- Not fixed yet: snapshot order, press-and-hold, row keys, press swell (see network.qml's notes).

## Fixed in batch 3 (fixer)

- Its lists keep their order as the Wi-Fi list does (`bt.paired`, `bt.nearby`); press-and-hold, row keys (see network.qml).

## Test results

(tester)

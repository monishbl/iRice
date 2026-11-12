# shell/modules/controlcenter/WifiShareCard.qml

Share-network card, inside the Wi-Fi page. Rust: host/system/src/accounts/cc.rs `WifiView::Share`, `share_page`; block `picture` (surfaces/controlcenter/ui/controlcenter.slint kind 11).

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| opens from the connected row's QR button and "Share network…" | WifiList 257-259 | host/system/src/accounts/cc.rs open_share | fixed (opened Settings) |
| title "Share “ssid”" | 27-45 | head | same (no back arrow: Done) |
| QR on white; spinner while loading | 47-99 | picture block | same |
| error; "Install qrencode" | 47-99 | note + buttons | same |
| shared again once qrencode is installed | 18-25 | host/system/src/accounts/cc.rs tick share_retry | same |
| password hidden, eye toggle | 111-136 | row s:password | same |
| Copy with a 1.5 s "Copied" | 137-147 | buttons, share_copied | same |
| closing removes the code (wifishare.close) | service | close_wifi_view | same |

## Fixed in batch 2

- the whole card

## Tester plan

Arena with the user's settings: copy `tools/live-scenarios.sh`, run with
`--arena N --only rust --bin <this worktree>/native/tessera-ui/target/release/tessera-ui`,
both `--look material` and `--look glass`; the QML for comparison with `--only qml`.
Open the Control Center: `ipc panels open controlCenter IRICE-TEST`; read
rectangles from a `dev shot` first. Pointer: `dev pointer -o IRICE-TEST move|glide|click|down|up|scroll`;
keys: `dev run wtype …`.

1. Connected Wi-Fi (or the arena's fake): click the connected row's QR button: the card with a QR on white (spinner first).
2. Click the Password row: the password shows; again: dots. Click "Copy password": the button says "Copied" for 1.5 s; `dev run wl-paste` gives the password.
3. Done: back to the list (the runtime QR file is removed).

## Test results

(tester)

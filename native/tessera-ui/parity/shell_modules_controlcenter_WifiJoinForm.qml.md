# shell/modules/controlcenter/WifiJoinForm.qml

Hidden / enterprise network form, inside the Wi-Fi page. Rust: host/system/src/accounts/cc.rs `WifiView::Form`, `form_page`, `submit_form`; blocks `field` (masked when secret), `seg`, `buttons` (surfaces/controlcenter/ui/controlcenter.slint Block kinds 9, 3, 12).

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| opens from: Join other network…, an enterprise row, "Connect with sign-in…", "Other options…" | WifiList | host/system/src/accounts/cc.rs open_form | fixed (opened Settings → Network) |
| hidden: name field, security segmented (None / WPA/WPA2 / Enterprise) | 129-163 | form_page | same |
| PSK field (masked) | 150-163 | field flag | same |
| EAP method, phase 2 (PEAP/TTLS), identity, password / TLS cert+key+key password, CA (system/none/file), domain, anonymous | 166-290 | form_page | same; certificate and key are typed paths (no FilePicker button): different |
| saved network pre-filled (never secrets) | 46-62 | host/system/src/accounts/cc.rs wifijoin.saved → form_filled | same |
| validation messages in order | 78-105 | submit_form | same wording |
| Join busy "Saving…/Joining…", failure text | 64-70, 301-320 | buttons busy, note | same |
| connected → back to the list | 64-70 | host/system/src/accounts/cc.rs wifi_page | same |
| secrets dropped after sending | 101-104 | submit_form | same |
| Cancel; leaving the page or closing CC drops the form | n/a | close_wifi_view | same |
| Enter in a field joins | Field onAccepted | "enter" | same |

## Fixed in batch 2

- the whole form, in the page

## Still different / missing

- file picker buttons (paths are typed)

## Tester plan

Arena with the user's settings: copy `tools/live-scenarios.sh`, run with
`--arena N --only rust --bin <this worktree>/native/tessera-ui/target/release/tessera-ui`,
both `--look material` and `--look glass`; the QML for comparison with `--only qml`.
Open the Control Center: `ipc panels open controlCenter IRICE-TEST`; read
rectangles from a `dev shot` first. Pointer: `dev pointer -o IRICE-TEST move|glide|click|down|up|scroll`;
keys: `dev run wtype …`.

1. Wi-Fi page → scroll to "Join other network…" → click: the form shows (title, name field, security segmented).
2. Click Join with nothing typed: "Type the network name." in red. Type a name, choose WPA/WPA2, type a 5-char password, Enter: "Wi-Fi passwords are 8 to 63 characters."
3. Choose Enterprise: EAP segmented; PEAP shows phase 2 and password; TLS shows certificate/key path fields instead.
4. Cancel: back to the list. Open the form again, close CC, reopen: the list (no form).
5. With an arena NM test network (or check the ask in the log with TESSERA_DEBUG): a valid join shows "Joining…" on the button.

## Test results

(tester)

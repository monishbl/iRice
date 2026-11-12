# shell/modules/settings/pages/sharing.qml ↔ surfaces/settings (src/pages/devices/phone.rs `sharing`)

Sharing, the PagePart of Phone & Sharing (page `phone`). Its processes
(file-manager-share.py status/install/remove, sharing-check.py status/fix)
live in RustBridgeDevices.qml for Rust. No popups, timers or animations.

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | On load: fm(["status"]) and checkSystem() (64–67) | RustBridgeDevices 35–38 (page "phone") | same |
| 2 | Group "Nearby sharing", footer: no LocalSend → "Sharing needs Python 3 and openssl…", else the LocalSend text + " " + LocalSend.error (83–86) | phone.rs 220–228 | same |
| 3 | "Send files" + detail, button "Send files…" (paper-plane-tilt), enabled when LocalSend available and FilePicker not busy → Panels.pickShare() (87–96) | 229–231 ask `sendFiles` | same |
| 4 | "Receive files from": Segmented 320 No one / Everyone for 10 min / Everyone ↔ sharing.receiving; detail by value (97–107) | 232–235 | same |
| 5 | "Firewall" only with SharingPorts.error: detail the error, "Show" → Settings security, row "Firewall" (110–118) | 236 `cx.go("security", "Firewall")` | same |
| 6 | "Your name": TextField 220, placeholder "<USER>'s computer" → sharing.name = trim (119–128) | 237 | same |
| 7 | Group "Android phones (Quick Share)", footer by QuickShare.available (name in quotes) + error (132–137) | 256–265 | same |
| 8 | "Visible to phones": Segmented 320 ↔ sharing.quickShare; detail by value (138–148) | 266–269 | same |
| 9 | "Bluetooth": detail by adapter/on (3 texts); "Turn on Bluetooth" (bluetooth) only when available and off → QuickShare.turnOnBluetooth() (149–160) | 270–283 | same |
| 10 | "Phones ask to pair" (system.pairing.risk): detail with askNote (passwordless → ""); "Stop pairing prompts" (bluetooth), busy while fixing "pairing", disabled while any fix runs → fixSystem("pairing") (161–172) | 284–287 | different (extra): Rust appends the fix's error to the detail; the QML keeps `fixError` but never shows it. Harmless; keep or drop |
| 11 | "Fast transfers without a shared Wi-Fi": switch on unless quickShareHotspot is false; detail by state (173–182) | 249–250, 288–294 | same |
| 12 | "Save files to": PathField folder, placeholder "~/Downloads", start "~" (or the folder) → quickShareFolder (183–192) | 251–252, 295 `.path(..)` + `path_setting` | same |
| 13 | "iPhone and other phones" + detail, "Receive from iPhone" (qr-code) → LinkShare.receive() (194–202) | 296–298 ask `receiveLink` | same |
| 14 | Group "Share from anywhere", footer fmError / some found / none found (206–211) | 319–326 | same |
| 15 | A row per manager found or installed: name; detail "“Send to a nearby device…” is in its right-click menu[. note]" / "Not added"; "Remove menu" (minus) / "Add menu" (plus), busy while the fm process runs → install/remove (212–225) | 301–314 (feed `managers`, `fmBusy`) | same |
| 16 | "Share the selection" + detail, "Share selection" (share-network) → Sharing.shareSelection() (226–234) | 315–318 | same |

Nothing to fix (#10 is Rust showing more than the QML).

## Test plan

Setups as in pages_mouse (both shells, both looks). `open phone`, scroll to the
"Sharing" heading (`run 'P move 1100 600; P scroll 0 25'`), `shot sh`; scroll
to the end, `shot sh-end`. Read y from shots. Never send or receive files for
real; set Receive/Visible back to "No one" after each test.

S1 Part: PASS: groups Nearby sharing, Android phones (Quick Share), Share from
anywhere with the same rows, buttons (with icons), footers; both shells.

S2 Receive: click "Everyone for 10 min": detail "You're asked before anything
is saved", config sharing.receiving "10min". Back to "No one".

S3 Name: click the name field, type "Arena", Enter: config sharing.name
"Arena". Clear it and Enter: "" (placeholder "<user>'s computer").

S4 Save files to: click Choose…: a folder chooser opens at ~ (or the folder);
cancel it. Type "~/Tmp" + Enter: config quickShareFolder "$HOME/Tmp" (restore).

S5 Hotspot switch: toggle; detail flips between the two texts; config
quickShareHotspot false/true.

S6 File managers: whatever the arena has: the same rows in both shells. Do not
click Add/Remove.

## Test results

(tester)

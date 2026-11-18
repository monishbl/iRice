# shell/modules/settings/pages/security.qml ↔ surfaces/settings (src/pages/system/security.rs `security`)

_Re-checked 17 Nov (B5, parity-settings) against main 18628a5 + the B5 ports: statuses below say what's there now; [12 Nov: …] is the old status._

Privacy & Security (page `security`). Its untitled part is biometrics.qml
(own parity file); the add-a-finger sheet is FingerprintSheet.qml (own file).
State from feed.system (RustBridgeSystem.qml: `check` = sharing-check.py
status, `quickShare`, `ports`).

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | On open: sharing-check.py `status` read into `system` (firewall, passwordless) (19–35) | RustBridgeSystem.qml 25–30 (`page === "security"` → checkSystem) | same |
| 2 | Re-read when SharingPorts stops being busy (37–43) | RustBridgeSystem.qml 57–60 (only while page is security) | same |
| 3 | Biometrics part first, untitled (46–48) | security.rs 431 `p.part(cx, "", biometrics)` | same |
| 4 | Group "Privacy", row "Screen sharing shows": detail by island.enabled / privacy.screenShareIndicator (4 texts); Segmented 240 wide Island/Dot/Both, current = index (min 0) → sets screenShareIndicator (50–64) | 433–450 | same |
| 5 | Group "Without your password", footer text (66–69) | 481–482 | same |
| 6 | Row "Tessera's helper": detail Installed… / Not installed yet… by `passwordless`; no control (70–74) | 483–487 | same |
| 7 | Row "Fast transfers without asking": hidden when sharing.quickShareHotspot === false; detail trustError / trusted / off texts; label red on trustError; Switch checked = trusted, disabled while trustBusy → QuickShare.setTrusted(!checked) (75–87) | 488–501 (`tone 2`, `quickShareTrust`) | same |
| 8 | Row "Firewall": label red on SharingPorts.error; detail: error / no firewall / open now "while you're <why or sharing>" / only while visible (+ " Closed now." when asked) (90–98) | 461–469, `firewall_detail` 45–55 | same |
| 9 | Firewall button only with an error: failedWith 64 → "Open Software Update" (arrow-circle-up) → Panels.openSettings("update"); else "Try again" (shield-check) → SharingPorts.retry(); busy while SharingPorts.busy (99–105) | 470–479 (`cx.go("update")`, `portsRetry`, `.on(pbusy)`) | same |
| 10 | While the add-a-finger sheet is open the page is still shown under it (FingerprintSheet is its own overlay window) | 422–426: the sheet **replaces** the page | **intentional: the sheet takes the page area (in-window)** [12 Nov: different (see FingerprintSheet parity)] |

## Test plan

Setups as in pages_menubar (`setup qml material`, `setup rust material <bin>`,
then glass). Material window 1040x700 at 440,205, page column from x≈772;
read every y from a shot. **Never** flip "Fast transfers without asking" or
press the Firewall buttons for real (they change system security); look only.

S1 `open security`, `shot sec-top`; scroll to the end (`run 'P move 1100 600 scroll 5'`),
`shot sec-end`. PASS: same groups, texts, footers, order as QML; on a computer
without reader/IR camera no Fingerprint/Face groups.

S2 Screen sharing: `clicks sec-seg "<Dot x> <y>" "<Both x> <y>" "<Island x> <y>"`
(read x/y from the shot; the Segmented is 240 wide at the row's right). PASS:
detail changes to the three texts in turn; config privacy.screenShareIndicator
follows. With island.enabled false (Notifications › Island off first, then
back): detail "The island is off, so a share shows as the purple dot."

S3 Hotspot hidden: note whether "Fast transfers without asking" shows; it must
match QML for the same config (sharing.quickShareHotspot).

S4 Firewall: read the detail in both shells; must be the same sentence.

## Test results

(tester)

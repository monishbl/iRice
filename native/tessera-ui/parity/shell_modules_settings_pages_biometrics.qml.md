# shell/modules/settings/pages/biometrics.qml ↔ surfaces/settings (src/pages/system/security.rs `biometrics`, ui/settings-system.slint `pick-add`)

_Re-checked 17 Nov (B5, parity-settings) against main 18628a5 + the B5 ports: statuses below say what's there now; [12 Nov: …] is the old status._

Fingerprint and Face, the untitled part at the top of Privacy & Security.
Both groups exist only with a reader / an IR camera. One popup: the finger
**Dropdown** (Dropdown.qml, its own list). State: feed.system.fingerprint /
face (RustBridgeSystem.qml).

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | On open Fingerprint.refresh(), Face.refresh() (22) | RustBridgeSystem.qml 26–28 | same |
| 2 | Group "Fingerprint" only with a reader; footer (ready only) "Password prompts, sudo…" + " " + pamError (23–28) | security.rs 175–185, 308–309 | same |
| 3 | Reader row (not ready): label reader name or "Fingerprint reader"; detail chain installError / unsupported / missing (+AUR note) / fixing / fixError / slowStart / restart; label red on install or fix error (31–41) | 189–215 | same |
| 4 | "Set Up Reader"/"Setting up…" (fingerprint, primary, busy fixing) only slowStart && nothing missing → Fingerprint.fix() (42–49) | 216–218 | same |
| 5 | "Install"/"Installing…" (download-simple, primary, busy installing, disabled while Updates.running) when packages missing → Fingerprint.install() (50–58) | 219–222 | same |
| 6 | A row per enrolled finger (icon fingerprint, name); Remove (danger) → asks: detail "It's deleted from the reader…", buttons Keep + "Remove <name>"; second press removes (62–92) | 229–251 | same (name fallback: key instead of "fingerprint" when unnamed — negligible) |
| 7 | "Add a fingerprint"/"Add another" + detail; **Dropdown** 190 wide of free fingers (fingerNames order), value = picked while free else first; Add (fingerprint, primary, disabled while enrolling/waiting or none) → sheet.open(finger) (93–120) | 252–281, settings-system.slint 196–221 | **same (fixed before 17 Nov: pick-add uses SChoice)** [12 Nov: **different**: the Dropdown is drawn as SChoice without `dropdown: true` and the row's control is "custom", so `model::dropdowns()` misses it: its list opens as an ActionMenu (44/32 px items, ≥ 240/220 wide) not Dropdown's (36/28 px, ≥ max(control,200)); no Up/Down stepping while closed; Material caret 20 px not 14] |
| 8 | "Unlock the lock screen" switch lock.fingerprint (default true) (123–128) | 284–286 | same |
| 9 | Password prompts / sudo in terminals / Login screen (GDM text when info.gdm): switch = pam[key], disabled while pamBusy, detail "Changing…" for the busy one → setPam(key, !checked) (129–145) | 287–306 | same |
| 10 | Sheet's addAnother: next free finger ≠ current → open(next) (149–156) | 126–138 (sheet's Add Another Finger) | same |
| 11 | Group "Face" only with IR camera; footer error / not-installed (with camera name) / installed text (160–166) | 316–330 | same |
| 12 | "Face unlock" (not installed): detail installError or Howdy text, red on error; Install (not primary, busy, disabled while updates run) (167–179) | 332–342 | same |
| 13 | "Set up the camera" (installed, not set up): "Set Up"/"Setting up…" primary, busy setup, **enabled only when Face.busy === ""** (180–191) | 343–351 `.off(!busy.is_empty() && busy != "setup")` | same (busy blocks clicks) |
| 14 | Faces row: label Look at the camera… / Add your face / 1 face added / N faces added; detail by state; Keep (confirming), Remove All → "Remove your face"/"Remove all N faces" (danger, disabled while busy), Add Face (primary at 0) / Add Another (smiley, busy add) (192–226) | 352–404 | same |
| 15 | "Unlock the lock screen" lock.face when set up and faces > 0 (227–232) | 405–409 | same |

## Test plan

Setups as in pages_menubar. `open security`. **Only on a host with a
fingerprint reader** (else B1–B3 show nothing: write "no reader"). **Never**
press Add, Remove, Install, Set Up, the PAM switches or the Face buttons:
open the Dropdown and Esc only.

B1 Shot the Fingerprint group in both shells. PASS: same rows/texts/order.

B2 Finger Dropdown: click it (read x,y from the shot), `shot bio-list`, Esc.
PASS (QML): item height 36 (Material) / 28 (Glass), list ≥ 200 wide, its right
edge on the control's, 4 px under it. Write Rust's sizes.

B3 Dropdown keys: click the Dropdown, Esc, `run 'key -k Down'`. PASS (QML):
the next finger is chosen without the list opening; Rust must do the same.

B4 Remove confirm (safe: the first press only asks): click "Remove" on a
finger row: detail "It's deleted from the reader…", Keep + "Remove <name>"
appear; click **Keep**. PASS: back to Remove; nothing removed.

## Fixed in batch 3 (fixer)

- The finger list is a Dropdown (36/28 px list, Up/Down step it closed, 14 px caret).

## Test results

(tester)

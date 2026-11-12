# shell/modules/settings/DriveGroup.qml ↔ surfaces/settings (src/pages/accounts/accounts.rs `drive_group` 413–468)

A cloud drive as a folder (rclone), the last group of Google (`google.qml`
192, drive gdrive) and Microsoft 365 (`microsoft.qml` 105, onedrive). State:
the accounts feed's `drives`.

Note: src/pages/about/drive.rs `group` is a second, unused copy (nothing
calls it; RustBridgeAbout.qml's `drives` feed and `drives.*` acts serve only
it). The live one is accounts.rs `drive_group`.

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | Title "<name> folder" (name from CloudDrives.names) (127–135) | accounts.rs 417, 465 (name fixed "Google Drive"/"OneDrive") | same (as long as CloudDrives.names says the same) |
| 2 | Footer: error / "Sign in in your browser…" while connecting / connected text / "<name> as a folder in Files…" (136–139) | 426–434 | same |
| 3 | No rclone: "<name> as a folder", detail installError (red) or "Needs rclone…"; Install (download-simple, busy, disabled while updates run) (141–153) | 435–445 | same |
| 4 | With rclone: label "~/<folder's last part>" when connected else "<name> as a folder"; detail Working… / Connected / Connected · not mounted / Not connected (154–157) | 459–461 | same |
| 5 | Buttons 8 apart: Open (folder-open, connected+mounted); Mount (connected, not mounted, busy working, enabled when nothing busy); Disconnect (danger, enabled when nothing busy); Connect <name> / "Waiting for the browser…" (busy); Cancel while connecting (158–192) | 446–462 | same |

## Test plan

Setups as in pages_menubar. `open google` (and `open microsoft`), scroll to
the end; read y from a shot. **Never press Install, Connect, Mount or
Disconnect**; Open (only when mounted) is fine.

D1 `shot drive-google`, `shot drive-ms`. PASS: title, row label/detail,
buttons and footer match QML for the same state.

D2 If connected and mounted: click Open → the folder opens in Files; close it.

## Test results

(tester)

# shell/modules/settings/RustBridge.qml ↔ host/shell/src/panels.rs (sync settings block 417–470, settings_call 669–717), bridges.rs settings_act/look_feed (85–200), picker reply (bridges.rs 1036)

The router between Panels and the Rust Settings window. In the QML shell it forwards
Panels.settingsOpen / .settingsPage / .settingsTarget as `open` / `page` / `close`
commands, answers the window's `close`, `page`, `pick` and `act` calls, and hands `act`
to the seven page-group bridges (each sends its own `feed`). In the standalone shell this is
Panels::sync (commands, the groups' `live` / `page`, the look and notify feeds) and
Panels::settings_call (the window's calls), with host-system's five bridge services doing the
other groups.

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | `on` = Rust draws Settings; `live` = on && Panels.settingsOpen (23–25) | panels.rs 418 `drawing(h,"settings")`; groups' live from `settings_open` (438–446) | same (the groups' `<g>.live` is sent from `settings_open` alone, not gated on `drawing`; fine in standalone) |
| 2 | Becoming live: `open` {screen: focused monitor, page, target}, target cleared (30–38) | panels.rs 422–425 (screen = focused, else Panels.screen) | same |
| 3 | No longer live: `close` (39–40) | panels.rs 427 | same |
| 4 | Page changed (not an old alias id): `page` {page, target}, target cleared (44–50) | panels.rs 428–431; aliases resolved in `set_settings_page` (127–135) | same |
| 5 | Target set while open: `page` with the target (51–59) | panels.rs 428 (`!settings_target.is_empty()`) | same |
| 6 | Call `close` → settingsOpen = false (68–70) | panels.rs 672 | same |
| 7 | Call `page` [id] → settingsPage follows the window (71–74) | panels.rs 673–681 (also marks it sent so it isn't echoed back) | same |
| 8 | Call `pick` [row id, folder, title, start, filter] → FilePicker, answer `{picked:{id,path}}` (75–78) | panels.rs 683 → `picker.pick` reply "settings"; bridges.rs 1036 tells `{"picked":{id,path}}` | same |
| 9 | Call `act` [group, what, …] → the group's `act` (80–86); unknown group warns (85) | panels.rs 694–713: look/notify → bridges::settings_act; devices/input → `<g>.act`; accounts → `accounts`; about/system → `<g>.<what>` | same (routing); each group's own gaps are in its parity file |
| 10 | Unknown call warns (88–89) | panels.rs 715 | same |
| 11 | Each group bridge tells its own `feed` when it changes (95–101, the groups' onFeedChanged) | look/notify: panels.rs 455–470 (diffed, every sync); devices/input/accounts/system/about: their services (polled 250 ms / 300 ms / 1 s / 400 ms / 400 ms) | **different**: polled instead of live signals (acceptable); and see 12 |
| 12 | Every page-group field that only the shell knows is in the feed (the groups' `feed`) | panels.rs 180 `feeds` map + `settings.feed` [group, patch] handler (604–611, 685–692) | **missing**: no crate ever asks `settings.feed`, so `feeds` is always empty and the look/notify feeds carry only bridges.rs/panels.rs's own fields (see RustBridgeLook/Notify parity files) |
| 13 | After the surfaces restart (`ready`) Settings is re-opened where it was | panels.rs 506–511 `sent_settings = None` | same |
| 14 | The page goes to the groups that react to it (devices, system, about read Panels.settingsPage) | panels.rs 448–453 `<g>.page` for devices, system, about | same |

## Test plan

Setup: `tools/settings-scenarios.sh setup rust material <bin>` (1920x1080; Material Settings window 1040x700 at 440,205: take `shot r0` first to read positions). Compare with the same steps after `setup qml material`.

R1 Open/page/close: `open network` → `shot r1a` (Network page). `run 'ipc panels settings sound'` → `shot r1b` (Sound page, same window, no reopen flash). Press Esc (`run 'key Escape'`) → `shot r1c` (window gone). PASS: as QML.
R2 Old page id: `open bluetooth` → `shot r2` . PASS: Network page, scrolled to Bluetooth (alias resolved).
R3 Target: with Settings open on Appearance, `run 'ipc panels settings sound Output'` → `shot r3`. PASS: Sound page at the Output group.
R4 Window → Panels: click a sidebar entry (e.g. Displays) with `clicks r4 "x y"`, then close (Esc) and `open` with no page → `shot r4`. PASS: reopens on Displays.
R5 Pick: `open wallpaper`, click a path row's Choose button (`clicks r5 "x y"`), cancel the file chooser. `log`: no "no call"/"no page group" lines. PASS: the chooser opened; cancel leaves the row as it was.
R6 Act routing: on each group's page do one safe act (Notifications: play a sound preview; Appearance: nothing destructive, e.g. Quick Settings "Edit Controls"; Network: Rescan; Mouse: none needed; Accounts: none; Battery: none; About → System health "Check again"). `log` after each. PASS: no "no act"/"no page group" warnings.
R7 Feeds arrive: on Focus, Notifications, Wallpaper and Lock & Login Screen, compare `shot`s with the QML pass. PASS: the same live values (Focus modes listed, notification count, login screen installed state, boot animations). Expected to FAIL until row 12 is fixed.

## Fixed in batch 4 (fixer)

- Settings merges each group's feed by field (a host-system bridge's whole feed and the services' `settings.feed [group, patch]` add up).


## Test results

(tester)

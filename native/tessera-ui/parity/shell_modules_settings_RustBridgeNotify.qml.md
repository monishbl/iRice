# shell/modules/settings/RustBridgeNotify.qml ↔ host/shell/src/panels.rs 461 (notify feed), bridges.rs settings_act 89–115; host/desktop wm/focusmodes.rs, wm/mod.rs 274–286, today/notif.rs 266–295

_Re-checked 17 Nov (B5, parity-settings) against main 18628a5 + the B5 ports: statuses below say what's there now; [12 Nov: …] is the old status._

The "notify" page group (Notifications & Island, Focus, Windows & Spaces). The standalone
feed is only `{glassPlugin, pickerBusy}` (panels.rs 461) merged with `Panels.feeds["notify"]`,
which nothing fills; the acts go out as asks to sound, focus, notify and decorations.

## Behaviours

Feed (QML 120–139):

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | `focus` {on, active, statusText, modes, added} (121–127) | — (desktop focusmodes.rs 125–131 publishes only focus.on/icon/name/status) | **same (fixed before 17 Nov: desktop wm focusmodes settings_feed → settings.feed notify.focus)** [12 Nov: **missing**: Focus page lists no modes (notify/focus.rs 56–63), Notifications' Focus switch always off (notifications.rs 106)] |
| 2 | `added`: the id Focus.addMode returned, so the page opens the new mode (117, 166–169) | focusmodes.rs 231–236 makes the id but nobody reports it | **same (fixed before 17 Nov: focus.added in the same feed; focus.rs selects it once)** [12 Nov: **missing**] |
| 3 | `notificationCount` (128) | — (desktop notif.rs keeps `list`, publishes only `unread`) | **same (fixed before 17 Nov: today/notif.rs asks settings.feed notify.notificationCount)** [12 Nov: **missing**: "Clear All" row shows 0 (notifications.rs 186)] |
| 4 | `temperature` only on notifications/island (130) | — (host-system Resources samples it but nothing feeds it) | **same (ported 17 Nov, B5: panels.rs notify feed `temperature` = h.state(resources).temperature on notifications/island, 0 elsewhere)** [12 Nov: **missing** (notifications.rs 492 shows 0)] |
| 5 | `networkCanWatch`, `networkReason` (131–132) | — (host-system network_alerts.rs 175–177 on private bus "NetworkAlerts") | **same (fixed before 17 Nov: network_alerts.rs settings.feed networkCanWatch/networkReason)** [12 Nov: **missing** (page defaults canWatch true: fine with NetworkManager, wrong without)] |
| 6 | `decorationsPlugin`, `pluginControls` Decorations (133–134) | — (desktop wm/decorations.rs knows plugin loaded) | **same (fixed before 17 Nov: wm decorations settings_feed decorationsPlugin/pluginControls)** [12 Nov: **missing**: Windows page always offers to install hyprbars (windowrules.rs 505–506)] |
| 7 | `glassPlugin` (135) | panels.rs 461 `h.st.shell.plugin` | same |
| 8 | `rulesError` WindowRules.error (136) | — | **same (fixed before 17 Nov: rulesError in the wm's settings.feed)** [12 Nov: **missing** (a broken rule never shows its error, windowrules.rs 278)] |
| 9 | `pickerBusy` (137) | panels.rs 461 | same |
| 10 | Feed told on every change while live (139) | panels.rs 455–470 | same (diffed each sync) |

Acts (QML 141–186):

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 11 | `play` [sound] even when off (143–145) | bridges.rs 94 → sound.play [s, true] | same |
| 12 | `playStartup` [id] (only `[a-z-]*`), at startupVolume (146–152) | bridges.rs 96 → sounds.rs 246–255 (only known ids) | same |
| 13 | `focusOn` [mode id, default dnd] (153–155) | bridges.rs 97 → focus.activate (wm/mod.rs 274) | same |
| 14 | `focusOff` (156–158) | bridges.rs 98 → focus.deactivate | same |
| 15 | `clearAll` (159–161) | bridges.rs 99 → notify.clearAll (notif.rs 275) | same |
| 16 | `setMode` [id, patch] (162–165) | bridges.rs 100–104 → focus.setMode | same |
| 17 | `addMode` [name] → `added` (166–169) | bridges.rs 105–109 → focus.addMode | **same (fixed before 17 Nov: added in the wm's settings.feed)** [12 Nov: **different**: the new id isn't fed back (row 2)] |
| 18 | `removeMode` [id] (170–173) | bridges.rs 110 | same |
| 19 | `restoreMode` [mode] (Undo) (174–179) | bridges.rs 111 → focusmodes.rs 238 | same |
| 20 | `installHyprbars` (180–182) | bridges.rs 112 → decorations.installPlugin (wm/mod.rs 286: terminal running install-hyprbars.sh) | same |

## Test plan

Setup: `tools/settings-scenarios.sh setup rust material <bin>`; window 1040x700 at 440,205 (`shot` to read positions). Compare each `shot` with the `setup qml material` pass.

N1 Notifications: post two test notifications (`run 'notify-send A a; notify-send B b'`), `open notifications` → `shot n1`. PASS: Clear All row says 2; Focus switch state matches QML; network alert row enabled as QML.
N2 Sound preview: click a sound's play button (`clicks n2 "x y"`). PASS: the sound plays (log shows no "no act").
N3 Island page: `open notifications/island` → `shot n3`. PASS: the CPU temperature shows (non-zero) as QML.
N4 Focus: `open focus` → `shot n4`. PASS: the same modes listed as QML, the active one marked, status text.
N5 Focus on/off: on Notifications toggle Do Not Disturb on then off (`clicks n5 "x y" "x y"`). PASS: the menu bar moon shows then hides; switch follows.
N6 Add/remove mode: on Focus add a mode named "Test" (type it, Add), `shot n6a` (PASS: the new mode opens/is selected as in QML), then remove it and click Undo in the toast, `shot n6b` (PASS: it is back), then remove it again.
N7 Windows page: `open windowrules` → `shot n7`. PASS: title-bar plugin row as QML (loaded → no Install button), no rules error unless QML shows one.
Never: installHyprbars, clearAll on real notifications you need (use the test ones).

## Fixed in batch 4 (fixer)

- Settings merges each group's feed by field (a host-system bridge's whole feed and the services' `settings.feed [group, patch]` add up).
- focus (modes, active, status, added), decorationsPlugin/pluginControls, rulesError: host/desktop wm `settings_feed`, diffed; notificationCount (today/notif.rs); networkCanWatch/networkReason (network_alerts.rs); temperature from Resources (sampled while Settings is open).

## Test results

(tester)

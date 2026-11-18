# shell/modules/settings/RustBridgeAbout.qml ↔ host/system/src/sysinfo/bridge_about.rs (About service: page_changed 131–154, feed 156–238, act 381–490, call 512–575), software_update.rs, tessera_update.rs, updates.rs; host/shell probes.rs (health, conflicts, bugreport, needs); input_bridge.rs (keymap.* replies)

_Re-checked 17 Nov (B5, parity-settings) against main 18628a5 + the B5 ports: statuses below say what's there now; [12 Nov: …] is the old status._

The "about" page group (Software Update, System health, Report a Bug, cloud drive folders,
shortcut/gesture import–export on About's backup). The About service owns the three update
models itself (fine) and reads the rest from host-system's private bus: "clouddrives",
"bootsnapshots", "keymap" exist; "health", "conflicts", "bugreport", "needs" (host-shell
state) and "today" (host-desktop) are never put, so System health, conflicts and Report a
Bug are empty.

## Behaviours

Page on show (QML 25–39):

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | update: SoftwareUpdate.checkUndo, BootSnapshots.refresh (27–29) | bridge_about.rs 136–138 | same |
| 2 | about/health: Health.check (30–31) | 139–140 → health.check (probes.rs 736) | same |
| 3 | about/bugreport: gather when no log and not gathering (32–34) | 141–145 (reads bus "bugreport" → never put → always asks) | **same (fixed before 17 Nov: reads h.state("bugreport") before gathering)** [12 Nov: **different**: re-gathers every time the page opens; read `h.state("bugreport")`] |
| 4 | Update output read only while shown (logUsers ±1) (37–39) | 147–152 | same |

Feed (QML 171–294):

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 5 | `su` {running … includeSkipped, plan, steps, undo…} (172–210) | 186–202 | same |
| 6 | `tu` {allowed, available, error, applying, report, result, conflicts, choices, missing, dropped, reasons} (211–223) | 203–208 | same |
| 7 | `up` {job, list, aurList, fake, interrupted, cancellable, running, progress, showDetails, log} (224–235) | 209–213 | same |
| 8 | `bs` {booted, assistant, canMenu, grubBtrfs, menuOn, error, busy} (236–244) | 214 (bus "bootsnapshots", boot_snapshots.rs 88–93) | same |
| 9 | `needsInstalling` (245) | 215 bus "needs" never put | **same (fixed before 17 Nov: h.state("needs").installing)** [12 Nov: **missing**: `h.state("needs")["installing"]`] |
| 10 | `health` {checks[…], groups, checking, fixing, error, checkedAt hh:mm} (246–253) | 175–177, 216–219 (bus "health" never put) | **same (fixed before 17 Nov: h.state("health") + Health.groups order)** [12 Nov: **missing**: System health shows no checks. Source: `h.state("health")` (probes.rs 406: checks, checking, checkedAt, fixing, error — no `groups`)] |
| 11 | `conflicts` [id, name, what, fix, muted] (254) | 178–180 (bus "conflicts" never put) | **same (fixed before 17 Nov: h.state("conflicts") found/muted)** [12 Nov: **missing**: `h.state("conflicts")` (probes.rs 402: found, muted)] |
| 12 | `bug` {title … issueUrl, info from sysinfo} (255–273) | 181–182, 220–224 (bus "bugreport" never put) | **same (fixed before 17 Nov: h.state("bugreport") with sysinfo as info)** [12 Nov: **missing**: Report a Bug form never shows its log/info/state; `h.state("bugreport")` (probes.rs 443)] |
| 13 | `welcomeName` Today.accountName (274) | 228 (bus "today" never put) | **same (bus "account" when there; the page falls back to the account's full name from /etc/passwd, Account.name)** [12 Nov: **missing** (host-desktop)] |
| 14 | `healthChecks`, `healthAttention` counts (275–276) | 229–230 (bus "health") | **same (fixed before 17 Nov: counted from health.checks (fail/warn))** [12 Nov: **missing** (About's health row shows nothing; `attention` isn't in probes' state either)] |
| 15 | `drives` {hasRclone, installing, installError, busy, errors, names, gdrive, onedrive, updatesRunning} (277–287) | 231–235 (bus "clouddrives") | same |
| 16 | `keymap` {ready, dialog = zenity\|kdialog} (288–291) | 236 (bus "keymap" has `ready` but no `hasZenity`/`hasKdialog`) | **same (fixed before 17 Nov: keymap bus has hasZenity/hasKdialog)** [12 Nov: **different**: `dialog` always false → always the typed path field, never a file dialog; ie_pick (279–300) likewise never runs zenity/kdialog. Put `hasZenity`, `hasKdialog`, `home` in input_bridge.rs `publish` (241–243)] |
| 17 | `ie` per section set (292) | 237 | same |
| 18 | Feed told on change (294) | 241–250, polled 400 ms (t_feed) | same (polled) |

Acts (QML 296–425, ieAct 118–169):

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 19 | `ie.export/import/end4/path/pathCancel/apply/close/dismiss/undo` (118–169, 298–301) | 386–388 → ie_act 328–380; Keymap via keymap.exportTo/previewFile/previewEnd4/applyPreview/restore with replies (input_bridge.rs 595–642, about.ieExported/iePreview/ieApplied 451–470) | same (dialog: row 16) |
| 20 | `su.check`, `su.update`, `su.cancel`, `su.goBack` (304–315) | 391–394 | same |
| 21 | `su.hold`, `su.includeAur`, `su.removeDropped`, `su.includeSkip` (316–327) | 395–398 | same |
| 22 | `tu.choose`, `tu.dismissResult` (328–333) | 399–400 | same |
| 23 | `up.dismiss`, `up.copyLog`, `up.showDetails` (334–342) | 401–407 | same |
| 24 | `reboot` (343–345) | 408 | same |
| 25 | `bs.openAssistant`, `bs.setMenu` (346–351) | 409–410 | same |
| 26 | `needs.install` [btrfs-assistant\|grub-btrfs] (352–355) | 412–416 | same |
| 27 | `health.check`, `health.copy`, `health.fix` [id] (357–365) | 418–420 → probes.rs 736–754 | same |
| 28 | `conflicts.mute` [[id]], `conflicts.unmute` [id] (366–371) | 421–422 | same |
| 29 | `bug.set`, `bug.include`, `bug.editShot`, `bug.copy`, `bug.openGithub`, `bug.send` (373–392) | 424–438 → bugreport.* | same |
| 30 | `bug.openIssue` (https only) (393–396) | 439–444 (reads bus "bugreport" → never put) | **same (fixed before 17 Nov: bug.openIssue reads h.state("bugreport").issueUrl, https only)** [12 Nov: **missing**: never opens; read `h.state("bugreport")["issueUrl"]`] |
| 31 | `welcome` → Panels.welcomeRequested (398–400) | 446 → welcome.request (bridges.rs 546) | same |
| 32 | `focusReload` (401–403) | 447 → focus.reload | same |
| 33 | `openFile` [/abs path] → Launch.open("file://…") (404–407) | 448–453 → `launch.open ["file://…"]` | **same (fixed before 17 Nov: openFile asks `open` [path])** [12 Nov: **missing**: bridges.rs 865–868 `launch.open` only opens `https://` links, so nothing opens. Ask `open` [path] (bridges.rs 954–959) instead] |
| 34 | `drives.install/open/act/cancel` (409–421) | 455–463 → clouddrives.* | same |

## Test plan

Setup: `tools/settings-scenarios.sh setup rust material <bin>`; window 1040x700 at 440,205 (`shot` first). Compare with the QML pass.

B1 Software Update: `open update` → `shot b1`. PASS: same "up to date"/pending list, hold/AUR switches, boot snapshot rows as QML.
B2 Check: click Check for Updates (`clicks b2 "x y"`), `shot b2` after it finishes. PASS: same result as QML; `log` clean. Do NOT click Update.
B3 Details: if a previous job is shown, toggle Show Details (`clicks b3 …`), `shot b3`. PASS: log text appears/disappears.
B4 System health: `open about/health` → wait 5 s → `shot b4`. PASS: the same checks, groups and "checked at" time as QML.
B5 Copy report: click Copy Report, `run 'wl-paste | head -5'`. PASS: the report text.
B6 Report a Bug: `open about/bugreport` → `shot b6`. PASS: log and system info filled as QML. Type a title (`clicks` into the field, `run 'key T e s t'`), switch tab away and back: title kept. Do NOT send.
B7 About: `open about` → `shot b7`. PASS: health summary row and welcome name as QML.
B8 Open a file: on About → Back up (or any row whose button opens a folder/file of yours), click it. PASS: the folder opens in Files (`run 'hyprctl clients | grep -i class'`).
B9 Export (About's backup of shortcuts): Export… → `shot b9`. PASS: a file dialog opens when zenity/kdialog is installed (as QML), else the path field.
Never: su.update, su.goBack, reboot, tu.choose on conflicts, bs.setMenu, needs.install, health.fix, bug.send, drives.*.

## Fixed in batch 4 (fixer)

- Settings merges each group's feed by field (a host-system bridge's whole feed and the services' `settings.feed [group, patch]` add up).
- health (groups, attention), conflicts, bugreport (issueUrl, log), needs from host state; welcomeName from the account; openFile asks `open` with the path; keymap dialog fields (input bridge).

## Test results

(tester)

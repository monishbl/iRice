# shell/modules/settings/RustBridgeAccounts.qml ↔ host/system/src/accounts/accounts_bridge.rs (AccountsBridge), google.rs, cloud_accounts.rs, cloud_drives.rs; host/shell probes.rs (needs); host/desktop ai/mod.rs, today/calendar.rs

_Re-checked 17 Nov (B5, parity-settings) against main 18628a5 + the B5 ports: statuses below say what's there now; [12 Nov: …] is the old status._

The "accounts" page group (Accounts, Calendar, AI & Sidebar, Default Apps). AccountsBridge
sends the feed (polled every 1 s; it also follows `panels.settingsOpen`) and forwards the acts
as asks. Google, iCloud/Microsoft and the drives are host-system services on its bus ("google",
"cloudaccounts", "clouddrives": fine). "needs", "calendar" and "ai" are read from that bus too
but are owned by host-shell (`h.state("needs")`) and host-desktop (nothing published), so they
are always empty.

## Behaviours

Feed (QML 47–110):

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | `signingIn` (48) | accounts_bridge.rs 103 | same |
| 2 | `google` {connected, configured, busy, operation, pendingAccount, lastError, outbox, signin, goaSettings, goaAccounts (unused), accounts, calendars, taskLists} (49–63) | 104–118 (bus "google", google.rs 247) | same |
| 3 | `cloud` {busy, signingIn, signingInto, lastError, errorFrom, accounts+contacts count, calendars, taskLists} (64–73) | 119–129 (bus "cloudaccounts") | same |
| 4 | `needs` {checked, secretTool, keyring, goa, goaMicrosoft, goaSignin, installing, building, buildError, updatesRunning} (74–85) | 130–141 (bus "needs" never put) | **same (fixed before 17 Nov: h.state("needs"); unchecked flags left out)** [12 Nov: **different**: every flag sent as false; because it is sent (not null) the page's defaults (accounts.rs 269–271: missing → true) don't apply, so it asks to install secret-tool/keyring/GOA even when present. Read `h.state("needs")` (probes.rs 517)] |
| 5 | `drives` {hasRclone, busy, installing, installError, errors, gdrive, onedrive} (86–94) | 142–150 (bus "clouddrives") | same |
| 6 | `calendar` {sources[name,url,color,enabled], lastError, defaultAlert} (95–99) | 151–155 (bus "calendar" never put) | **same (fixed before 17 Nov: accounts_bridge.rs feed `calendar` from calendar_doc())** [12 Nov: **missing**: Calendar page always "No subscriptions yet"; desktop today/calendar.rs owns the sources and publishes nothing for Settings] |
| 7 | `ai` {providers, hasKey{model: bool}, envKeys, sessions, pinned, undo} (100–107) | 156–163 (bus "ai" never put; envKeys and undo own) | **same (fixed before 17 Nov: accounts_bridge.rs feed `ai`: hasKey from the shell's Keys + cfg ai.models, sessions, pinned, undo, envKeys; providers come from the page's own list)** [12 Nov: **missing**: no key ever shown as set (ai.rs 256), no chat counts; desktop ai/mod.rs publishes nothing for Settings] |
| 8 | `tools`: the apps on PATH from the fixed list (108) | 164 | same |
| 9 | Feed told on change; AI undo finished when Settings closes (110–111) | 264–275, live 297–304, poll 322–335 | same (polled 1 s) |

Acts (QML 113–237):

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 10 | `googleRun` [{cmd ∈ sync, disconnect, connectDesktop, connect, configure, forget}] (117–120) | 181–185 → google.run | same |
| 11 | `googleConnect` (121–123) | 186 | same |
| 12 | `googleEvent` [account, text] (124–127) | 187–192 → google.write | same |
| 13 | `googleTasks` → open tasks.google.com (128–130) | 193 → launch.open | same |
| 14 | `googleKeep` (131–133) | 194 → google.openKeep | same |
| 15 | `onlineAccounts`: GNOME Online Accounts settings, or an install hint (134–142) | 195–203 → launch.run / google.setError | same |
| 16 | `cloudSync`, `cloudDisconnect` [id] (144–149) | 205–206 | same |
| 17 | `icloudSignIn` [id, password] (150–153) | 208–212 | same |
| 18 | `microsoftSignIn`, `appleAccount` (154–159) | 213–214 | same |
| 19 | `needsInstall` [[pkgs]], `needsBuild` (160–166) | 215–221 | same |
| 20 | `driveInstall`, `driveOpen`, `driveAct` [mount\|disconnect\|connect, drive], `driveCancel` (168–180) | 223–231 | same |
| 21 | `calSet` [url, enabled], `calRemove`, `calAdd` [name, url], `calAlert` [n] (182–194) | 233–236 → calendar.* (today/calendar.rs 85–101) | same (results not fed back: row 6) |
| 22 | `aiSetModel` [key] (196–198) | 238 | same |
| 23 | `aiSave` [draft, old key, credential] (199–211) | 240 → ai.accountsSave | same |
| 24 | `aiRemove` [key]: remove, "<name> removed" undo for 8 s, credential forgotten after (212–223, 22–31) | 241–251: looks the model up in bus "ai".models → never put → returns early | **same (fixed before 17 Nov: aiRemove looks the model up in cfg ai.models)** [12 Nov: **missing**: Remove does nothing. Look the model up in `h.cfg()["ai"]["models"]`] |
| 25 | `aiUndo` (224–230) | 252–258 | same (depends on 24) |
| 26 | `aiDeleteAll` (231–233) | 259 | same |

## Test plan

Setup: `tools/settings-scenarios.sh setup rust material <bin>`; window 1040x700 at 440,205 (`shot` first). Compare with the QML pass.

A1 Accounts: `open accounts` → `shot a1`. PASS: Google/iCloud/Microsoft rows, synced times, drive rows as QML; no "Install secret-tool / keyring" prompt unless QML shows it.
A2 Calendar: `open calendar` → `shot a2`. PASS: the same subscriptions and default alert as QML.
A3 Calendar toggle: switch one subscription off then on (`clicks a3 "x y" "x y"`). PASS: switch follows, `run 'cat ~/.config/tessera/calendars.json'` (or wherever the sources live) back to the original.
A4 AI: `open ai` → `shot a4`. PASS: models, "key set" marks, chat count as QML.
A5 AI remove + Undo: add a throwaway model "Test" (provider Ollama, no key), save, Remove it (`shot a5a`: toast "Test removed"), click Undo (`shot a5b`: back), then Remove again and let it expire.
A6 Default Apps: `open defaultapps` → `shot a6`. PASS: same terminal/browser/editor choices.
Never: sign in/out, cloudDisconnect, googleRun disconnect/forget, needsInstall, driveInstall/driveAct, aiDeleteAll.

## Fixed in batch 4 (fixer)

- Settings merges each group's feed by field (a host-system bridge's whole feed and the services' `settings.feed [group, patch]` add up).
- needs from host state (unchecked flags left out); calendar subscriptions from calendars.json; AI hasKey per model, sessions, pinned; aiRemove finds the model in cfg.
- Not done: needs.installing/building/buildError and calendar lastError (nothing tracks them).

## Test results

(tester)

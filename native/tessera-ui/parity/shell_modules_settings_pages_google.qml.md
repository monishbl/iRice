# shell/modules/settings/pages/google.qml ↔ surfaces/settings (src/pages/accounts/accounts.rs `google`, `drive_group`)

_Re-checked 17 Nov (B5, parity-settings) against main 18628a5 + the B5 ports: statuses below say what's there now; [12 Nov: …] is the old status._

The Google part of Accounts (page `accounts`, heading "Google"; old link
`google`). Data: RustBridgeAccounts.qml feed `google`, `drives`, `needs`;
actions `googleRun`, `googleConnect`, `googleEvent`, `googleTasks`,
`googleKeep`, `onlineAccounts`, `drive*`. Menus: two Dropdowns (Row::choice).
DriveGroup.qml (Google Drive folder) is listed here too.

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | "now" ticks every 30 s while visible; ago(t): not yet / just now / N min ago / N h ago / date (48–60) | mod.rs 32 `ago`; page rebuilt on every service update (lib.rs 1076 `data`) | same at a glance (no own 30 s timer; refreshed by any data change) |
| 2 | Footer by state: lastError › busy → working words (by operation; signin: "Connecting…" if GOA accounts else "Sign in … browser") › outbox "1 change is"/"N changes are … waiting" › connected text › signin/GOA text › "needs native helpers" text (62–79) | accounts.rs 16–35, 71–83 | same |
| 3 | Group title "Google accounts" (>1) / "Google account" (73) | 160 | same |
| 4 | One row per connected account: label identity or "Google account"; detail "Syncing…" while syncing else "Last synced …"; buttons "Sync now" (busy for that account's sync, disabled while busy) and "Disconnect" (danger, disabled while busy) → Google.run sync/disconnect (82–105) | 86–102 | same |
| 5 | Connect row: label "Add another account" (connected) / "Google account"; detail by GOA count/connected (108–113) | 104–112, 137–143 | same |
| 6 | Button "Connect Google account" (primary when not connected) / "Add account"; shown with GOA accounts or signin; busy on signin/connectDesktop; disabled while busy; >1 GOA → connectDesktop(chosen), else Google.connect() (114–121) | 113–132 | same |
| 7 | "Open Online Accounts" (no GOA, no signin): opens gnome-control-center online-accounts google, or the GTK app; none → lastError "Install gnome-online-accounts …" (122–133) | 133–141 → bridge `onlineAccounts` | same |
| 8 | "Account" row (>1 GOA accounts): Dropdown 260 of their names → chosen (135–144) | 144–154 `.choice(..).width(260)` | same |
| 9 | "Not signed in yet" row + detail (not connected, no GOA, signin) (145–149) | 155–159 | same |
| 10 | Group "Calendars" (connected): footer by calendar count; SubpageLink calendar-blank "Shown calendars" → calendar / "From your accounts" (155–166) | 167–194 | same |
| 11 | "Add to primary calendar": TextField 200 "Event and time" (no commit on blur, Enter does nothing); "Add event" disabled while empty → Google.write eventAdd(quickAccount, primary, text), field cleared (167–182) | 169–195 (field `i` 0 typing, `i` 10 button) → bridge `googleEvent` | same |
| 12 | Group "Tasks" (connected): link list-checks "Where new tasks go" → calendar / "To-dos"; "Open Google Tasks" → tasks.google.com (185–201) | 197–202 | same |
| 13 | Group "Keep": "Open Google Keep" → Google.openKeep() (203–211) | 205–208 | same |
| 14 | DriveGroup gdrive: title "Google Drive folder"; footer error › connecting › connected › plain (DriveGroup 15–19) | 414–434 | same |
| 15 | Drive "as a folder" row without rclone: detail installError (label danger) or "Needs rclone…"; Install (busy installing, disabled while updates run) (DriveGroup 21–33) | 436–445 (tone 2) | same |
| 16 | Drive row with rclone: label "~/<folder>" / "Google Drive as a folder"; detail Working… / Connected / Connected · not mounted / Not connected; buttons Open (folder-open, mounted), Mount (busy working), Disconnect (danger), "Connect Google Drive"/"Waiting for the browser…", Cancel while connecting; disabled while drives busy (DriveGroup 34–73) | 446–467 | same |
| 17 | More options › "Calendars": "Quick add uses" Dropdown 260 of identities (>1 account) → quickAccount (218–232); group folds away when hidden | 214–227 | same |
| 18 | More › "Advanced": switch "Use your own OAuth client" (page-local, starts off) (235–243) | 230–234, 250 | same (but see #22) |
| 19 | "Client file" (advanced): detail Imported / "The JSON file…"; PathField: field placeholder "client_secret_….json" commits on Enter/blur (~ expanded) → configure; "Choose…" → chooser "Choose a file" in ~/Downloads, filter OAuth client|*.json; **"Choosing…" and disabled while the picker is open** (244–254; PathField.qml 21–45) | 236–251 `.path(..)` | **same (fixed before 17 Nov: busy(pickerBusy) on Client file)** [12 Nov: **different**: no `.busy(..)` on the row, so the button never says "Choosing…"/disables (the feed has no picker-busy flag)] |
| 20 | "Connect with your client" (advanced): "Connect Google account", disabled while busy or not configured → run connect (255–263) | 248, 252 | same |
| 21 | One "Remove Tessera's copy[ of identity]" row per account, "Remove local data" disabled while busy → forget (264–273); footer only while advanced (238) | 254–261 | same |
| 22 | Page-local state (advanced, chosen, quickAccount, typed event) starts over each time the page loads (Settings' page Loader) | lib.rs 511/579 keep `acct.google.*` | **same (fixed before 17 Nov: acct.* in PAGE_TRANSIENT)** [12 Nov: **different**: Advanced stays on, typed text kept after leaving the page] |

## Test plan

Setups: `tools/settings-scenarios.sh setup qml|rust material|glass <bin>`.
Material window 1040×700 at 440,205, page column from x≈772. `open google`
(scrolls to the Google heading). **Read y from a shot.** The arena has no
Google account: never sign in, never click Connect/Add account, never pick a
real client file. Look, hover, open menus, Esc only.

G1 Page: `shot g-top`, scroll until "Google Drive folder" and More options.
PASS: groups "Google account" (footer one of the not-connected texts), "Keep",
"Google Drive folder" with the same rows, buttons, details as the QML.

G2 Hover: glide over "Open Google Keep" and the Drive button
(`clicks g-hover "x y"` with points read from the shot, then Esc). PASS: same
hover fill and press look as the QML, both looks. Do NOT click Connect.

G3 More options: click "More options". PASS: "Advanced" with the switch off
and no footer; "Calendars" (quick add) absent (no accounts). Switch "Use your
own OAuth client" on: "Client file" and "Connect with your client" (disabled)
appear, footer shows. Hover "Choose…": hover look.

G4 Choosing: click "Choose…" once, shot while the chooser is open, then Esc
the chooser. PASS (QML): the button reads "Choosing…", greyed, while it's open.
Rust must match.

G5 State reset: with Advanced on, go to another page and back. PASS (QML):
More options closed, Advanced off. Rust must match.

## Fixed in batch 3 (fixer)

- "Choosing…" while the file chooser is open; typed state starts over with the page.

## Test results

(tester)

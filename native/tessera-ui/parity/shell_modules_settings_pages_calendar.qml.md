# shell/modules/settings/pages/calendar.qml ↔ surfaces/settings (src/pages/accounts/calendar.rs, ui/settings-accounts.slint "source", "removed", "field")

Calendar (page `calendar`). Data: feed `google`, `cloud`, `calendar`
(sources, lastError, defaultAlert); config `google.hiddenCalendars`,
`cloud.hiddenCalendars`, `google.taskListId`; actions `calSet`, `calRemove`,
`calAdd`, `calAlert`. Menus: two Dropdowns (Row::choice).

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | Group "From your accounts" (Google connected or any cloud account) + footer (25–29) | calendar.rs 66, 107–108 | same |
| 2 | "No calendars yet" (no calendars at all), label secondary, detail "Syncing…" while either syncs / "They appear after the first sync" (30–35) | 70–71 (tone 1) | same |
| 3 | Per Google calendar: "title (primary)"; detail Google · identity (>1 account) · View only; switch shown (hidden list holds "acc|id" or a bare id); toggling rewrites the list without either, adding "acc|id" when hiding (36–54) | 72–88, `google_shown`/`google_toggle` 12–25 | same |
| 4 | Per iCloud/Outlook calendar: title; detail iCloud/Outlook · identity (>1) · View only; switch → cloud.hiddenCalendars add/remove "acc|id" (55–71) | 89–106 | same |
| 5 | Group "To-dos" (Google connected or a Microsoft account) + footer; "New to-dos go to" Dropdown 260: "This computer only", Google lists ("title · identity" with >1 account), "title · To Do" cloud lists; value: "cloud|…" as is, else the Google list found by "acc|id" or bare id, else "" → google.taskListId (75–92) | 115–133, `task_value` 30–42 | same |
| 6 | Group "Subscribed calendars": footer lastError or refresh text; "No subscriptions yet" row when none (95–103) | 135–169 | same |
| 7 | Per source: name; detail URL without http(s)://, first 60 chars + "…" when > 68 (106–110) | 139–146 | same |
| 8 | Source controls, 10 apart: colour dot 12 (in a 12×28 box); Switch enabled → setSource(url, {enabled: !checked}); IconButton trash → removed = it, removeSource (111–141) | 147–162, slint "source" 156–184 | same (but see #9) |
| 9 | IconButton (IconButton.qml): **28×28 and icon 16 in both looks**; Material round, squares to radius 8 while pressed (150 ms OutCubic); hover/press fills (hover 150 ms); Tab-focusable, Space/Enter click, FocusRing; ElasticControl press (Glass squish, Material grow + spring) | settings-accounts.slint 74–92 `IconBtn` | **different**: Material 40 px / icon 20; no pressed-radius morph; not focusable (no keys, no ring); no press scale |
| 10 | "Removed “name”" row (outside groups) with accent "Undo" (DemiBold, pointer) → addSource(name, url), row gone; Timer 8 s while removed ≠ null clears it (stops when Undo clears; a new removal later starts a fresh 8 s) (146–167) | 152–158, 171–180; slint "removed" 98–125 | **different (small)**: the 8 s sleeper is `cx.spawn("cal.removed")` guarded by `cal.removed.busy`: after Undo, a new removal within the old 8 s gets no timer of its own and vanishes when the first one ends |
| 11 | Group "Add a calendar" + footer; "Name" TextField 260 "Work"; "Link" TextField 260 "https://… .ics", detail = error text; Enter in Link adds (no commit on blur) (169–192) | 183–197 `field` | same |
| 12 | "Add calendar" (row with empty label), primary while the link has text; add: link must start https?:, webcal:, / or file: else error "Paste a link that starts with https:// or webcal://, or a file path."; else clear error, addSource(name.trim, url), clear both fields (193–213) | 186–197, `add_calendar` 213–224, `link_ok` 45 | same |
| 13 | Group "Alerts": "Remind me before events" Dropdown Never/At start time/5/10/15/30 minutes/1 hour → defaultAlert + saveSources (216–231) | 199–209 → `calAlert` | same |
| 14 | TextField look (TextField.qml): 220 default, 40/28 tall, Material filled pill outlined while typing, Glass fill + hairline, accent border on focus, placeholder hidden while focused | slint `SyncField` 20–71 | same |
| 15 | Page state (removed, typed name/link, error) starts over when the page loads again | lib.rs 511/579 keep `cal.*` | **different**: typed text, error and the Removed row survive leaving the page |

## Test plan

Setups: `tools/settings-scenarios.sh setup qml|rust material|glass <bin>`;
Material window 1040×700 at 440,205, page column from x≈772. `open calendar`.
**Read y from a shot.** No accounts in the arena (groups 1 and 5 hidden).
Use only a local file as a subscription: first `run 'printf "BEGIN:VCALENDAR\nEND:VCALENDAR\n" > /tmp/t.ics'`.

C1 Page: shot top to bottom. PASS: "Subscribed calendars" (No subscriptions
yet or the arena's ones), "Add a calendar", "Alerts", same texts, both looks.

C2 Add error: click the Link field, `run 'key -t x.ics'`, click "Add calendar".
PASS: Link row detail shows the error text; Add button primary while text is there.

C3 Add: select all in Link (`run 'key -M ctrl -k a -m ctrl'`), type `/tmp/t.ics`,
name `Test`, press Enter in Link. PASS: a "Test" row with a colour dot, switch
on and a trash button; fields cleared; error gone.

C4 Trash button: shot zoomed on it, hover it, Tab to it (`run 'key -k Tab'`
from the switch). PASS (QML): 28×28 with a 16 px icon in BOTH looks; Material
press squares the corners; a focus ring shows on Tab and Space removes. Write
the Rust size, ring, and press feel.

C5 Remove + Undo: click trash. PASS: "Removed “Test”" with Undo appears below
the group; after 8 s it goes (time it from the click, both shells). Again:
remove, click Undo within 8 s → row back. Then within the first 8 s remove
it again: PASS (QML) the new "Removed" row stays a full 8 s.

C6 Switch: toggle the Test source off → config `calendar` source enabled false
(`run 'jq . ~/.config/tessera/calendars.json'` or the shell's file), on again.

C7 Alerts dropdown: click it, shot the open list (rows 36 Material / 28 Glass),
Esc; pick "30 minutes": closed shows "30 minutes".

C8 Reset: type `x` in Name, go to another page and back. PASS (QML): empty.

Clean up: remove the Test subscription.

## Fixed in batch 3 (fixer)

- The trash: 28 px, 16 px icon, a Tab stop (Space/Enter), Material's corners square off when pressed, a press scale.
- The Removed row: each removal gets its own 8 s (a later one isn't cut short by an earlier timer); typed state starts over with the page.
- Not done: "Last synced N min ago" has no 30 s timer of its own.

## Fixed in batch 4 (fixer)

- Batch 4: every 30 s while Settings is open the page is built again ("Last synced N min ago" moves on; the look previews' clocks too).

## Test results

(tester)

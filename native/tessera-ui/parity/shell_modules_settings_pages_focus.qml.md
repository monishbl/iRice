# shell/modules/settings/pages/focus.qml ↔ surfaces/settings (src/pages/notify/focus.rs, ui/settings-notify.slint `ItemRow`, `EntryField`, `icons`, `days`)

_Re-checked 17 Nov (B5, parity-settings) against main 18628a5 + the B5 ports: statuses below say what's there now; [12 Nov: …] is the old status._

Focus (page `focus`). Modes list (each row picks the mode to edit), "Add a
mode" entry, an 8 s Undo after deleting, then the chosen mode's editor. No
popups. One Timer (undo 8000 ms).

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | selectedId starts at the first mode; mode = modeById(selectedId) ?? first (14–15) | focus.rs 68–73 (local `focus.selected`, falls back to the first) | same, but the choice survives page switches (QML page is made anew: back to the first) — **different (small)** |
| 2 | Mode rows (Group, footer "Click a mode to edit it…"): label name; detail "<statusText> · " when active + "Scheduled <from>–<to>" (Focus.fmt, TimeFormat.hm) or "Manual"; icon on the right 18, filled + accent when selected, else secondary (32–44) | 76–100 (`item` custom row; `fmt_time` with appearance.clock24); slint ItemRow 179–262 | same |
| 3 | The whole row is a MouseArea, pointing-hand cursor → selectedId; **Glass: no hover fill** (Row.qml), Material: the Group's hover tile (45–50) | ItemRow TouchArea; Glass `ta.has-hover ? Theme.fill` | **same (fixed before 17 Nov: Glass hover fill only on chevron rows (window rules' apps); Focus mode rows have none)** [12 Nov: **different (small)**: Glass rows get a hover/press fill the QML rows don't have (windowrules' AppRow has it; the mode rows don't)] |
| 4 | "Add a mode": TextField 200, placeholder "Name, then Enter", commitOnBlur off; Enter with text → selectedId = Focus.addMode(trim), field empties (53–66) | 101–107 (`entry` → ask addMode; the bridge says `added`, 63–67 selects it); slint EntryField 133–176 | same |
| 5 | Undo row (a bare Row, **outside any Group**) "Deleted “name”" with an **"Undo" text link** (accentInk, body, DemiBold, pointing hand) → modes.concat(removed), save, select it; Timer 8000 ms hides it (69–93) | 114–125: a group row with `.button("Undo").minor()`, ask restoreMode, 8 s by `focus.removedAt` + a spawned 8.1 s tick | **same (fixed before 17 Nov: text_link Undo)** [12 Nov: **different**: Rust shows a minor (tonal-less) button in a group box; QML a plain accent text link on the page background. Use `.text_link()` (style 4)] |
| 6 | Mode group (only with a mode), title = its name (95–98) | 127, 227 | same |
| 7 | "Name" TextField 200 → setMode name (trimmed, not empty) on commit (99–106) | 131–139, 228 `.field` | same |
| 8 | "Icon": Flow **180 wide, spacing 2, 12 IconButtons (28×28 in both looks)**, filled + accent when chosen → setMode icon (107–123) | 140–149, 223; slint `icons` 297–330 (two rows of 6, `Ghost`) | **same (fixed before 17 Nov: Ghost is 28×28 in both looks)** [12 Nov: **different**: Ghost is 40×40 in Material (IconButton is 28 in both looks), so Material's picker is 250 wide instead of 178] |
| 9 | "Allowed apps" TextField 220, placeholder "None", text = allowApps joined ", " → split on commas, trimmed, empties dropped (124–133) | 150–157, 230 (`app_list` tested) | same |
| 10 | "Wallpaper" TextField 220, placeholder "No change" → trim, leading ~ → $HOME (134–143) | 158–171, 231 | same |
| 11 | "Appearance" Segmented 240 No change / Light / Dark ↔ "", light, dark (144–153) | 172–180, 232 | same |
| 12 | "Schedule" switch + detail → schedule.enabled (154–161) | 181–185, 233 | same |
| 13 | "From" (+ "24-hour time, e.g. 22:30") and "To": TextField 90, shown while scheduled, commit only /^\d{1,2}:\d{2}$/ (162–180) | 186–198, 234–235 (`is_time` tested) | same |
| 14 | "Days": 7 Chips S M T W T F S, solid, no tick, 40 (M) / 30 (G) wide, spacing 4; toggles the day, sorted (181–203) | 199–204, 236; slint DayChip 54–84 | same |
| 15 | "Delete this mode" (not for dnd): **"Delete" text link in danger** (body, DemiBold) → removed = mode, Focus.removeMode, select the first (204–221) | 205–219, 237 `.button("Delete").danger()` | **same (fixed before 17 Nov: danger_link Delete)** [12 Nov: **different**: a filled danger button instead of a red text link (SButton has no danger link style)] |

## Test plan

Setups as in pages_mouse (both shells, both looks). `open focus`, `shot f`.
Material window 1040×700 at 440,205; page column from x≈772; read y from shots.

F1 Page: PASS: mode rows with icons on the right (first one accent), "Add a
mode" field, the first mode's group; same texts in both shells.

F2 Hover (Glass): move the pointer over a mode row and shot. PASS (QML): no
fill. Rust must match. Material: the tile lightens in both.

F3 Select: click the second mode. PASS: its icon fills in accent, the group
title and fields show it.

F4 Add: click the field, `run 'key -t Reading; key -k Return'`. PASS: a mode
"Reading" appears, is selected, the field is empty.

F5 Icon picker: shot the Icon row in Material. PASS (QML): 2 rows of 6 icons,
28 px each, about 178 px wide. Rust must match. Click "book-open": the mode's
icon changes in the list.

F6 Schedule: switch on: From, To, Days rows appear; type "7:5" in From + Enter:
nothing changes; "07:30" + Enter: the list detail "Scheduled 7:30 AM–…" (or
07:30 with 24-hour clock). Click M and W: chips fill.

F7 Delete + Undo: on "Reading", shot the Delete control (QML: red text "Delete",
no box). Click it. PASS: mode gone, first selected, "Deleted “Reading”" with an
accent text "Undo" (no button box, no group box in Glass). Click Undo within
8 s: mode back and selected. Again, wait 9 s: Undo row gone.

F8 Page switch: select the second mode, open another page, back to Focus. PASS
(QML): the first mode is selected again.

## Fixed in batch 3 (fixer)

- Undo: an accent text link; Delete: a red text link; Glass mode rows have no hover fill; the icon picker's buttons 28 px with their names as tooltips; typed state starts over with the page.

## Test results

(tester)

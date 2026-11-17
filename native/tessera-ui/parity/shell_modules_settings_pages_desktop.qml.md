# shell/modules/settings/pages/desktop.qml ↔ surfaces/settings (pages/look/desktop.rs)

_Re-checked 17 Nov (B5, parity-settings) against main 18628a5 + the B5 ports: statuses below say what's there now; [12 Nov: …] is the old status._

Page id `desktop`. Feed `cx.feed["look"]`: `files`, `weather`, `shelfCount`,
`shelfOpened` (RustBridgeLook.qml under QML; host/shell/src/bridges.rs
`look_feed` in the standalone shell).

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | Widgets group footer "Drag a widget to move it…" (12–15) | desktop.rs 43–44 | same |
| 2 | Edit Widgets: detail "N widget(s) on the desktop"; accent pill "Edit…" (Material 40, Glass 28): closes Settings, picks the first screen, starts widget editing (16–35) | 27–45, host `editWidgets` | same |
| 3 | Files app: detail "No file manager installed (e.g. sudo pacman -S nautilus)" / "Opens folders, Downloads and the Trash"; Dropdown 220 "Automatic (<first>)" + installed managers → desktop.fileManager (36–45) | 29–49 | **same (page fallback: file managers on PATH)** [12 Nov: **different (standalone)**: `look_feed` has no `files`, so the list is only "Automatic" and the detail always says no file manager is installed] |
| 4 | Weather footer: "Showing <place>: <temp>°, <condition lowercased>." when ready, else the error or "Loading…" (48–51) | 51–57 | **same (fixed before 17 Nov: look_feed weather)** [12 Nov: **different (standalone)**: no `weather` in `look_feed`: always "Loading…"] |
| 5 | Location TextField 220, placeholder "City, e.g. Bengaluru", committed trimmed → weather.location; detail (52–61) | 58–70 | same |
| 6 | Units Segmented 200 Celsius/Fahrenheit → metric/imperial (62–70) | 66–71 | same |
| 7 | Hot corners: 4 Dropdowns 220 (Top left/right, Bottom left/right) with None/Overview/All Windows/Show desktop/Notification Center/App Drawer/Lock screen/Start Bedside Mode/Quick note; footer (73–114) | CORNER_ACTIONS, 73–88 | same |
| 8 | More options → Drop shelf: footer; Shake while dragging / Drag to the side of the screen / Menu bar icon switches with details (117–137) | 90–106 | same |
| 9 | Open the shelf: detail "N item(s) on it" / "Empty"; button "Hide Shelf"/"Show Shelf" toggles the shelf (138–145) | 99–109, host `shelf` → shelf.toggle | **same (fixed before 17 Nov: look_feed shelfCount/shelfOpened)** [12 Nov: **different (standalone)**: no `shelfCount`/`shelfOpened` in `look_feed`: always "Empty" and "Show Shelf"] |

## Test plan (both looks)

`setup qml|rust material|glass <bin>`, `open desktop`, `shot`.

D1 Widgets: detail counts the widgets in the arena's config ("1 widget"/"N
widgets"). Files app dropdown: open it (read x≈1340 and y from the shot), `shot`.
PASS: "Automatic (Nautilus)" or similar, then each installed manager; detail
"Opens folders, Downloads and the Trash" when one is installed.

D2 Weather: after the weather loads (wait 10 s), footer "Showing <place>: NN°,
<condition>." in both shells. Type "Paris" in Location, Return: footer changes
to Paris once loaded. Units Fahrenheit: the temp changes. Restore afterwards.

D3 Hot corners: open "Top left": 9 actions in the order of #7; choose "Overview";
the dropdown shows it. Set it back.

D4 More options: the 3 switches toggle; "Open the shelf": click "Show Shelf".
PASS: the shelf opens and the button reads "Hide Shelf"; drop or have an item:
detail "1 item on it". Click "Hide Shelf": it closes.

D5 Edit…: click it (last; it closes Settings). PASS: Settings closes and the
desktop's widget editing starts on the first screen.

## Fixed in batch 2 (fixer)

- Files app: the file managers on the PATH (Files.qml's list), when the shell doesn't send `files`.
- Weather footer: from the weather service itself.
- Drop shelf: the standalone shell now sends `shelfCount` / `shelfOpened`.

## Test results

(tester)

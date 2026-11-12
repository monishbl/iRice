# shell/modules/settings/pages/menubar.qml ↔ surfaces/settings (src/pages/look/menubar.rs `menubar`)

Menu Bar & Dock (page id `menubar`). Plain rows only (Row/Group/MoreOptions/
PagePart); the Dock and Quick Settings parts have their own parity files
(pages_dock, pages_controlcenter). No Popup, no Timer, no animation of its own.

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | Group "Behaviour": switches autoHide "Automatically hide the menu bar" (+ "Move the pointer to the top edge to show it."), background "Show menu bar background", scrollGestures "Scroll to change brightness and volume" (+ "Left half: brightness. Right half: volume.") → `menubar.<key>` (27–33) | menubar.rs 30–38 `toggle` | same |
| 2 | Group "Left and centre": windowTitle "Show window title"; "Spaces" Segmented 240 Off/Dots/App icons ↔ off/dots/icons, unknown → 0 (35–49) | 40–46 | same |
| 3 | "Battery" Segmented 300 Off/Icon/% inside/% beside; current: battery off → 0, else max(1, index of batteryPercent with "true"→outside, "false"→off); choosing i>0 writes batteryPercent (1→"off"), always writes battery = i>0; detail only while battery on (56–71) | 50–67 | same |
| 4 | Status switches wifi, bluetooth, sound, resources "CPU and memory", weather, keyboardLayout (+hint), collapseTray "Tuck away app icons" (+hint), search/ai/controlCenter/clock with the live key combo in the hint (KeyNames.combo of SUPER Space / SUPER A / SUPER C / SUPER N), appMenu, windowMenu (+ SUPER CTRL F) (72–84) | 68–89, `keys::combo` | same |
| 5 | Group "Clock": showDate, showSeconds (87–92) | 91–95 | same |
| 6 | More options folded (95) | `p.more` / `p.end_more` (MoreOptions parity) | same |
| 7 | More › Behaviour › "Screens" (+ "Show the menu bar on these screens."): a chip per screen, solid (Glass: on = accent fill), checked when the list is empty or holds it; click toggles, all chosen → []. Flow 260 wide, **RightToLeft** (first screen at the right edge), wraps (99–122) | 100–113 `.chips(..).width(260)` | **different**: chips are one HorizontalLayout (alignment end), first screen at the left of the run, no wrap; `width` ignored by the chips control |
| 8 | More › Left and centre: "Spaces always shown" slider 1–10 step 1 (200), detail "N space(s); more appear when used"; "When there are more spaces" Segmented 220 Arrows/Dynamic dots + its two details; "Spaces on other displays" switch + detail; all three hidden while workspaces == "off" (124–151) | 115–136 | same |
| 9 | More › Status items › "Unread notifications" Segmented 320, 5 options ↔ bellDot/dot/count/bellCount/off + detail (152–166) | 138–144 | same |
| 10 | More › Buttons: 7 switches (screenshot "Screenshot tools", colorPicker "Colour picker", mic, keyboard, darkMode, powerProfile "Energy mode", record) toggling the id in `menubar.utilButtons` (167–185) | 146–168 | same |
| 11 | PagePart "Dock" (dock.qml), PagePart "Quick Settings" (controlcenter.qml) after More options (189–196) | 173–174 `p.part` | same |
| 12 | Every value live from Config (another window changing it updates the page) | page rebuilt after each change / config reload | same |

## Test plan

Setup per look: `setup qml material`, `setup rust material <bin>`, then the same
with `glass`. Material window 1040×700 at 440,205, page column from x≈772;
Glass: read the window's place from setup's output. Positions of rows depend on
scrolling: **read the y from a shot** every time.

M1 Page: `open menubar`, `shot mb-top`; scroll (`run 'P move 1100 600 scroll 5'`)
and shot until the Dock heading shows. PASS: same groups, labels, details and
order as the QML in both looks; the key combos in the Search/AI/Quick
Settings/Clock hints read the same as the QML (e.g. "Super Space").

M2 Battery: click "Off", then "% inside", then "Icon" on the Battery segmented
(`clicks mb-battery "x y" …`, x/y read from a shot). PASS: Off hides the detail;
"% inside" shows it; after "Icon" the config has battery true, batteryPercent
"off" (`run 'jq .menubar.battery,.menubar.batteryPercent ~/.config/tessera/config.json'`).
Both shells write the same values.

M3 Spaces: set Spaces to "Off", open "More options". PASS: the three spaces
rows under "Left and centre" are hidden; "Dots" brings them back. Move the
"Spaces always shown" slider: detail "1 space; …" at the left end, "N spaces" else.

M4 Screens chips (needs 2 outputs; in the arena add one with
`dev hyprctl output create headless` if the plan allows, else note "1 screen"):
open More options, shot the Screens row. PASS: chip order and colours as the
QML (QML: first screen at the RIGHT, Glass on = solid accent); click one off,
then on again: the list goes [other] → [] .

M5 Buttons: switch "Colour picker" on. PASS: `menubar.utilButtons` gains
"colorPicker" (and the menu bar shows the button in both shells).

M6 Parts: scroll to the bottom. PASS: "Dock" and "Quick Settings" headings and
their groups follow More options, as the QML.

## Fixed in batch 2 (fixer)

- Screens chips: solid, right to left (the first screen at the right edge), as the QML.

## Test results

(tester)

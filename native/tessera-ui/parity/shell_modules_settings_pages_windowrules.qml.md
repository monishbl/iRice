# shell/modules/settings/pages/windowrules.qml ↔ surfaces/settings (src/pages/notify/windowrules.rs, ui/settings-notify.slint `ItemRow`, `listhead`, `empty`, `search`, `edithead`, `stepper`, `size`)

_Re-checked 17 Nov (B5, parity-settings) against main 18628a5 + the B5 ports: statuses below say what's there now; [12 Nov: …] is the old status._

Windows & Spaces (page `windowrules`). The rule editor in three views (list,
pick, edit), an 8 s Undo after removing a rule, then the PagePart "Windows,
Stages and Overview" (appearance-windows.qml, its own parity file). No
popups. One Timer (undo 8000 ms).

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | view list/pick/edit; edit(cls, name) refreshes toplevels, sets class/name, view edit (16–34) | windowrules.rs 200–205 (local `wr.*`), `cx.want("clients")` | same |
| 2 | The page is made anew on every visit: view back to "list", undo gone (implicit: page Loader) | `wr.view`, `wr.removed` survive page switches (lib.rs 581) | **same (fixed before 17 Nov: wr.* in PAGE_TRANSIENT)** [12 Nov: **different**: leaving in the picker or an app's rules and coming back shows that view again] |
| 3 | List: "New windows open" Segmented 220 Tiled/Floating ↔ windowRules.defaultLayout, detail + footer by layout; only when WindowRules.available (112–127) | 260–272 | same (no `available` check, see #4) |
| 4 | Errors group: !available → "Window rules aren't set up" + "The windowRules section is missing…"; else WindowRules.error → "Rules weren't saved" + error (130–137) | 274–278 (feed `notify.rulesError`) | **same (ported 17 Nov, B5: windowrules.rs: "Window rules aren't set up" when the config has no windowRules)** [12 Nov: **missing (small)**: the "aren't set up" case (no `available` in the feed)] |
| 5 | Undo group (list view): "Removed the rule for <name>", TextButton "Undo" (arrow-counter-clockwise) → WindowRules.restore(rule, index), timer stops; 8 s (140–155) | 294–308 | same |
| 6 | Empty state (no rules): 64 px taller than its column; app-window 40 tertiary, 4 gap, "No window rules" title DemiBold, body text centred ≤ 360 wide, 24 gap, primary "Choose app" (plus) → pick (158–197) | 315–318 `empty`; slint 462–499 | same |
| 7 | Head line (rules exist): "Apps open the way you set here. Rules match the window class." small secondary, left 16/4; primary "Add app" (plus) → pick; 28 tall (199–218) | 319 `listhead`; slint 440–461 | same |
| 8 | Rules group + footer "Changes apply to windows opened after the change.": AppRow per rule (summary detail) → edit (220–234) | 320–326 `app_row` | same |
| 9 | AppRow: 64 (M) / 48 (G) tall; AppIcon 40/28 at 20/14 (**Apps.iconFor falls back to "application-x-executable"**), name 16/body, detail 14/caption, elided; chevron caret-right 24/12 (bold in Glass) tertiary; Glass fill on hover/press, Material the group tile; pointing hand (55–108) | 215–232; slint ItemRow (`pic`, `chevron`) | **same (fixed before 17 Nov: icon lookup falls back to application-x-executable (notify/mod.rs))** [12 Nov: **different**: `app_icon` (notify/mod.rs 77–90) has no fallback icon, so an app without one has no icon and its name starts at the left edge] |
| 10 | Picker head: IconButton caret-left (label colour) **tooltip "Back to all rules"** → list; TextField (placeholder "Search apps, or type a window class", commitOnBlur off) cleared and focused as the picker shows; Enter → first open/installed match, else the text as a class (237–313) | 327–368 `search` block; slint `SearchBox` (focus on init) + `Ghost` | **same (fixed before 17 Nov: Ghost tooltip "Back to all rules"; the search field takes focus as it shows)** [12 Nov: **different**: Ghost is **40×40 in Material** (IconButton 28 in both looks); **no tooltip**] |
| 11 | Open apps (toplevels, one per class, not org.quickshell, by name) and installed apps (startupClass ‖ id, not NoDisplay, not open, by name); filter by name or class; installed capped at 80 (248–280) | 329–352 (`open_apps`, `matches` tested) | same |
| 12 | "Loading apps…" while no entries and no open apps (315–321) | 370–372 `note` num 3 | same |
| 13 | No results: "No apps match “q”" + "Check the spelling, or press Enter…" (323–330) | 373–375 | same |
| 14 | Groups "Open now" / "Applications" (footer "Showing 80 of N. Type to narrow the list." over 80); detail = the rule's summary or the class (332–363) | 376–389 | same |
| 15 | Edit head (44 tall): back IconButton (tooltip "Back to all rules"), AppIcon 36, name title DemiBold + "Window class <cls>" caption, elided; danger TextButton "Remove rule" (trash) only when a rule exists → remove, back to list, undo (366–412, 38–45) | 395–421 `edithead`; slint 518–568 | **same (fixed before 17 Nov: back Ghost with tooltip; Remove rule only with a rule)** [12 Nov: **different**: back button 40×40 in Material, no tooltip; no icon at all when the app has none (QML: the fallback)] |
| 16 | "When it opens": "Float" (tiled default) switch on for float or pin, toggling → pin ? {float:false,pin:false} : {float:!float, tile:false}; "Tile" (floating default) → {tile:!tile, float:false, pin:false} (414–436) | 423–427, 445–446 | same |
| 17 | "Open on space": detail by workspace; stepper: minus IconButton (tooltip "Previous space", tertiary at 0), "Space N"/"Any" 64 wide tabular, plus (tooltip "Next space", tertiary at 10); 28+4+64+4+28 wide (437–470) | 428–441 `stepper`; slint 341–363 | **same (fixed before 17 Nov: Ghost 28×28 with tooltips Previous/Next space)** [12 Nov: **different**: Ghost 40×40 in Material (stepper 152 wide vs 128), no tooltips, no tabular figures] |
| 18 | "Start as" Segmented 280 Normal/Maximized/Fullscreen; "Let the app maximize itself" switch; "Show on all spaces" switch (471–496) | 442–451 | same |
| 19 | "Look": "Always opaque" switch (499–511) | 453–455 | same |
| 20 | "Size" + footer: "Default size" (detail "Width × height, in pixels" unless an error), two TextFields 72 ("Width"/"Height") and "×"; each commits on Enter/blur: both empty → 0×0; one empty → "Enter both…"; not whole / out of 200–7680 × 150–4320 → "Enter whole numbers…" (513–581) | 457–473 `size` (SizeField commits on Enter and on focus loss), `size_of` (tested) | same |
| 21 | Fields refilled from the rule (and error cleared) when the view or class changes (543–552) | SizeField text bound to the rule; `edit` clears `wr.sizeError` | same at a glance (a typed field keeps its text until the row is made again) |
| 22 | "Current window" (an open window of that class): "W × H", "Use current size" → fields = max(200,w), max(150,h), then commit (validated) (582–595) | 474–488: change() directly, no validation | **same (ported 17 Nov, B5: Use current size goes through size_of's check)** [12 Nov: **different (small)**: no validation (a window over 7680×4320 would save); the fields may keep stale typed text (test)] |
| 23 | Size error under the group: caption danger, left 16/4 (598–606); "Changes apply to <name> windows opened from now on." caption tertiary (608–616) | 489–492 `note` num 2 / 0 | same |
| 24 | PagePart "Windows, Stages and Overview" (619–622) | 284 `p.part` | same |

## Test plan

Setups as in pages_mouse (both shells, both looks). `open windowrules`,
`shot wr`. Material window 1040×700 at 440,205; page column from x≈772; read y
from shots. Rules are config only: remove any rule you add (or restore the
config afterwards).

W1 List: with no rules: empty state (icon, title, text, Choose app). Shot.
PASS: same sizes and gaps in both shells.

W2 Picker: click Choose app. PASS: back button, focused empty search, "Open
now" and "Applications" with icons. Shot in Material: back button 28×28 in QML;
Rust must match. Hover it 1.5 s: tooltip "Back to all rules" (QML). Type
`run 'key -t zzzz'`: "No apps match “zzzz”". Clear and type "foot", Enter:
edit view for the first match.

W3 Icons: in the picker, find an app with no icon (or a typed class
`run 'key -t my.fake.class; key -k Return'`). PASS (QML): a generic
application icon left of the name in the edit head; Rust must show the same.

W4 Edit: Float on, Open on space + twice ("Space 2", detail "Opens on space 2
and takes you there"); shot the stepper in Material: 28 px buttons, 128 wide
(QML). Hover minus 1.5 s: tooltip "Previous space". Start as Maximized, Always
opaque on. Back: the list shows the app with its summary.

W5 Size: type 300 in Width, Tab: "Enter both a width and a height, or leave both
empty." under the group; 300 × 100 → "Enter whole numbers: …"; 800 × 600 → no
error, saved. With the app open: "Current window W × H" → Use current size;
PASS: both fields show the window's size, saved.

W6 Remove + Undo: Remove rule. PASS: back to the list, "Removed the rule for X"
+ Undo; Undo restores it in the same place; wait 9 s after another remove: the
Undo is gone.

W7 Page switch: open the picker, switch to another page and back. PASS (QML):
the list view. Rust must match.

## Fixed in batch 3 (fixer)

- Back and the space stepper's buttons 28 px with tooltips; an app with no icon gets the generic one; typed state (wr.*) starts over with the page.

## Test results

(tester)

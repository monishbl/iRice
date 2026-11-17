# shell/modules/settings/pages/shortcuts.qml ↔ surfaces/settings (src/pages/input/shortcuts.rs, keymap.rs, input/mod.rs; ui/settings-input.slint)

_Re-checked 17 Nov (B5, parity-settings) against main 18628a5 + the B5 ports: statuses below say what's there now; [12 Nov: …] is the old status._

The Keyboard page: Tap Super, the shortcuts list (search, per-bind menu, undo),
the form for one shortcut (recorder: see shell_modules_settings_ShortcutRecorder.qml.md),
More options (Screenshots, Text in screenshots) and the Dictation part (own file).
Binds come from `feed.input.keymap` (RustBridgeInput.qml / host input_bridge.rs);
changes go back with `ask input kbDisable|kbReset|kbRemove|kbSave|undo` (the bridge
keeps one snapshot for undo). Keymap.qml's logic is keymap.rs (unit-tested lists,
parseKeys/formatKeys/canonical/keyLabel/keyText/matches/conflicts).
QQC.Popup: `ActionMenu { id: pageMenu }` (660) opens for each bind's "…" menu and the
form's two Choices (Default app role, Tessera action). Rust: `cx.menu` → settings menu
as an xdg_popup.

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | Tap Super: Overview/Search/Nothing (width 260) keyboard.superTap (152–165) | shortcuts.rs:224–227 | same |
| 2 | Problems: one group per defaultsError/userError/saveError: "Something's wrong with the shortcuts files", detail = error, warning-circle 20 px warning colour (168–180) | shortcuts.rs:230–232; slint `icon` style 4 | same |
| 3 | List: search field (placeholder "Search by name or keys, like “super s” or “ctrl print”", live) + primary "Add shortcut" (plus), disabled while userError (183–202) | shortcuts.rs:313–323; slint `searchbar` | same |
| 4 | "Loading shortcuts…" while !Keymap.ready (204–210) | shortcuts.rs:325–327 `note` | same |
| 5 | Undo group (list view only): label = the undo text, "Undo" (arrow-counter-clockwise) → Keymap.restore(snap); gone after 8 s (22–32, 212–227) | input/mod.rs `offer_undo` (8 s thread), shortcuts.rs:329–335 → `ask undo` (bridge restores its last snapshot) | same |
| 6 | Search: groups filtered by Keymap.matches (keys query matches keys only; words match label/group/actionText); "Your shortcuts" kept (empty) when no query (34–39) | shortcuts.rs:337–341, keymap.rs `matches`/`parse_query` (tested) | same |
| 7 | No results: "No shortcuts match “q”", detail, "Clear search" empties the field (229–237) | shortcuts.rs:342–345 (`set_field kb.query ""` bumps the searchbar's gen = `b.num`, a block, so it works) | same |
| 8 | Groups in data order, Your shortcuts first; footer "Your shortcuts win when they share keys with a default." when it has any (239–263) | `groups` (tested), shortcuts.rs:401–402 | same |
| 9 | Empty Your shortcuts: "No shortcuts of your own yet" + "Add shortcut" (plus) (249–254) | shortcuts.rs:350–353 | same |
| 10 | BindRow: label (secondary when off), detail "Off" / "Changed · was <keys>" / actionText (custom); keycaps (opacity 0.4 when off); "…" button; 56/44 px min (82–150) | settings-input.slint BindRow 129–184; shortcuts.rs:354–399 | same |
| 11 | Clicking anywhere on the row (left OR right button) opens the menu under the "…" (134–141) | BindRow TouchArea `clicked → dots.open()` | **same (fixed before 17 Nov: right button up opens the menu too)** [12 Nov: **different**: right-click does nothing (TouchArea clicked is left only)] |
| 12 | Row keyboard: Tab focuses each bind row (activeFocusOnTab), focus ring (Material 3 px secondary r4 / Glass 2 px accent), Return/Enter/Space/Menu open its menu (89–105) | BindRow has no FocusScope / ring / keys | **same (fixed before 17 Nov: BindRow FocusScope (Return/Space/Menu open), SFocusRing)** [12 Nov: **missing**] |
| 13 | Glass hover/pressed fill on the row; also lit while focused (88) | BindRow `if !Theme.material` Rectangle (hover/pressed only) | same except focus (see 12) |
| 14 | Material: the Group's tile shows an 8 % on-surface hover layer on the row (87; Group.qml:73–83) | custom rows draw `Tile` (settings-input.slint:19–28) with no hover | **same (fixed before 17 Nov: BindRow/AppRow Material on-surface 8 %/10 % hover/press layer)** [12 Nov: **missing** (Material bind rows and app rows don't light on hover)] |
| 15 | "…" icon lit (label colour) while the row is hovered OR its menu is open (146) | Dots `lit: ta.has-hover` | **same (fixed before 17 Nov: Dots `shown` while its menu is open)** [12 Nov: **different**: not lit while its menu is open] |
| 16 | Accessible name "label, keys[, off]" (90–91) | `accessible-label: r.label`, description = detail | **different (a11y name only, remaining): the keys are in the description, not the name** [12 Nov: different (minor: no keys in the name)] |
| 17 | Menu items: Edit (pencil-simple) / Change shortcut (keyboard); Turn on/off (power); Remove (trash, danger) / Reset to default (arrow-counter-clockwise, enabled only when changed) (46–79) | shortcuts.rs:375–396 | same |
| 18 | Each menu action offers undo: "Turned off X" / "Turned on X" / "Removed X" / "X is back to <default keys>" (55–76) | shortcuts.rs:377–388 | same |
| 19 | Import and export (keybinds, shortcuts, gestures) when list && ready (265–269) | importexport::block (own parity file) | same |
| 20 | Form head: back (caret-left, tooltip "Back to all shortcuts") → list; title New / Edit / Change shortcut; subtitle "label · group" for a default (366–392) | shortcuts.rs:416–420; slint `formhead` | same (no tooltip on the back button) |
| 21 | open form loads name, app search "", appId, role (default terminal), shell, command, kind (app/role/shell/command, default app), repeating, locked, recorder.load(keys), error "" (41–45, 290–306) | `open_form` shortcuts.rs:94–124 | same |
| 22 | Name group (custom only): "Shown in Settings and the cheatsheet", field 260, placeholder "Open Firefox" (395–408) | shortcuts.rs:423–427 (`field` ctl, gen in r.num: not a custom row, so kept) | same |
| 23 | Keys group: the recorder, footer "Most combinations go to the window manager …" (410–426) | shortcuts.rs:430–478 | same (see the recorder's file) |
| 24 | "Default" row (changing a default whose keys differ): detail = default keys, "Use default keys" → recorder.load(default) (427–435) | shortcuts.rs:463–473 | same (Rust also clears the error; QML's load doesn't emit edited: harmless) |
| 25 | Conflicts group: "Used by A, B", "Only one shortcut can use <keys>.", "Use anyway (turns that one off)" / "(turns those off)" → save(true) (288, 438–449) | shortcuts.rs:481–488, keymap.rs `conflicts` (tested) | same |
| 26 | Action group (custom): Segmented App / Default app / Tessera action / Command, full width minus 20/14, 64/54 tall; choosing clears the error (452–470) | shortcuts.rs:492–499; slint `segmented` | same |
| 27 | Default app: "Opens your" Choice (width 180) of Keymap.roles, detail (471–482) | shortcuts.rs:502–512, `ROLES` (tested against Keymap.qml) | same |
| 28 | Tessera action: Choice width 260, placeholder "Choose an action", clears error (483–497) | shortcuts.rs:513–524, `SHELL_ACTIONS` (tested) | same |
| 29 | Command: field 300, placeholder "kitty -e btop", detail "Runs with sh, …" (498–508) | shortcuts.rs:525–528 | same |
| 30 | App: search field (56/48 tall, "Search apps"); list of !noDisplay apps sorted by name, chosen first then up to 30; matched by name/genericName/id (509–533) | shortcuts.rs:529–562; slint `search`, AppRow | same (sort: lowercase compare vs localeCompare) |
| 31 | The app search field is emptied each time the form opens (296) | `set_field kb.appSearch ""` bumps a gen put in `r.num`, but `place_all` (shortcuts.rs:308 → input/mod.rs:46–58) overwrites `num` of every custom row with its tile place; the `search` view uses `gen: r.num` (settings-input.slint:433) | **same (fixed before 17 Nov: places() skips search/recorder rows' num)** [12 Nov: **different**: the field can't be reset by the page; a reused field keeps old text while Rust's query is ""] |
| 32 | App row: 24 px icon, name (600 when chosen), accent tint + check when chosen; click → choose (fills Name "Open <app>" if empty) (534–593) | AppRow settings-input.slint:350–397; shortcuts.rs:547–561 | same |
| 33 | App row keyboard: Tab focus, focus ring, Space/Return choose (542–562) | AppRow: no FocusScope | **same (fixed before 17 Nov: AppRow FocusScope + ring, Space/Return choose)** [12 Nov: **missing**] |
| 34 | "Loading apps…" / "No apps match “q”" + detail / "Showing 30 of N. Type to narrow the list." (>31) (595–607) | shortcuts.rs:563–569 | same |
| 35 | Options (custom): Repeat while held ("Like volume keys"), Works on the lock screen (610–629) | shortcuts.rs:577–589 | same |
| 36 | Error text under the form: danger, small, left 16/4 (631–639) | `note` num 2 | same |
| 37 | Validation messages (key / app / action / command) and the conflict message (318–338) | `validate`, `save` shortcuts.rs:153–188 | same |
| 38 | Cancel + primary "Add shortcut"/"Save shortcut" (disabled while userError), right-aligned (641–658) | slint `actions` | same |
| 39 | Save: snapshot; turn off the taken-over binds; default → setKeys; custom → add/update {keys, label (name or describe), action, opts}; back to list; undo "<label>: <keys>[ (turned off …)]" (331–364) | `save` → `ask kbSave` (bridge does the snapshot, disable, setKeys/add/update, appAction, describe) | same |
| 40 | More options (also shown, empty, in the form view) (665) | `p.more` always | same |
| 41 | Screenshots (list only): Markup editor Tessera/Swappy (Swappy off without swappy, width 200); thumbnail switch (not Swappy); Swappy markup switch (disabled without swappy); footer "Install swappy …" (666–700) | shortcuts.rs:242–259 | same |
| 42 | Text in screenshots (list only): install row when no Tesseract (Installing…/busy, disabled while Updates.running); Find screenshots by their text (disabled without Tesseract); a row per language with Install; footer = install error or the text (704–751) | shortcuts.rs:261–303 | same |
| 43 | Dictation part (755–758) | `p.part("Dictation", dictation)` | same (own file) |

## Test plan (both looks; Material window at 440,205, page column from x≈772; arena mod key = ALT)

Read every y from a shot; the list is long. Never run the shortcuts you record
(no power/session/lock actions). Restore with Undo or "Reset to default" after each change.

K1 List: `setup qml material`, `open shortcuts`, `shot kb-qml-m`; same for rust.
PASS: Tap Super row, search + Add shortcut, "Your shortcuts" (empty state with
Add shortcut) then the groups in the same order, same keycaps (ALT where Super is).

K2 Search: click the search field, `run 'key screenshot'` (wtype text). PASS: only
matching binds, same rows in both. Clear it (`run 'for i in $(seq 12); do key -k BackSpace; done'`),
then type `alt s` (a keys query: matches keys only, same rows in both). Clear, type
`zzzz`: "No shortcuts match “zzzz”" + Clear search; click Clear search → full list, field empty.

K3 Menu: click a default bind's row (not the "…"). PASS: popup under its "…" with
Change shortcut / Turn off / Reset to default (greyed when unchanged). Right-click the row
instead: QML opens the menu; report Rust (expected nothing = #11).

K4 Turn off + undo: menu → Turn off. PASS: label secondary, "Off" detail, keycaps
faded, an undo row "Turned off X" at the top; click Undo → back on; another Turn off,
wait 9 s → the undo row disappears by itself (both).

K5 Keyboard on rows: click the search field, then `run 'key -k Tab'` several times
with a shot after each. PASS (QML): focus reaches the bind rows with a ring; `run 'key -k Return'`
opens that row's menu, Esc closes. Report Rust (expected: rows not reachable = #12).

K6 Material hover: P move over a bind row's middle, shot. PASS (QML): the row's tile
lightens. Report Rust (#14). Also: open a row's menu and shot: QML's "…" stays dark (lit); Rust (#15).

K7 Change a default: menu → Change shortcut. PASS: head "Change shortcut", subtitle
"<label> · <group>", Keys group with the recorder, no Name/Action/Options. Toggle the
SHIFT chip: a "Default" row "Use default keys" appears; if the new keys clash, the
"Used by …" group shows. Click "Use default keys": the row goes. Cancel.

K8 New shortcut, command: Add shortcut → kind "Command", type `true` in the Command
field, click the recorder box, `run 'key -k F9'`, Save. PASS: back to the list,
"Your shortcuts" has it (label "Runs true", keys F9), undo "Runs true: F9". Click Undo: gone.

K9 Validation: Add shortcut, Save at once. PASS: red "Choose a key: click the box and
press one, or use Edit as text." Pick kind "Tessera action", record a key, Save:
"Choose a Tessera action." Kind "App" with a key and no app: "Choose an app from the list."

K10 App kind: Add shortcut → App. Type `fire` in "Search apps": list narrows; click an
app: tinted with a check, moves first, Name fills "Open <app>". Cancel; Add shortcut
again → App. PASS (QML): "Search apps" is empty and the full list shows. Report Rust
(#31: the field may keep "fire" while the list is unfiltered). Tab into the app list,
Space (QML chooses; Rust #33).

K11 Conflict: Add shortcut → Command `true`, toggle the chips to match an existing bind
(read one from the list, e.g. ALT + Q style) and record its key. PASS: "Used by X",
"Only one shortcut can use …", "Use anyway (turns that one off)"; plain Save shows
"These keys are used by X. …". Cancel (don't take over).

K12 More options (list view): Screenshots and Text in screenshots rows identical
(swappy / tesseract presence as the arena has them).

K13 Glass: K1, K3, K6, K10 with `setup … glass`.

## Fixed in batch 2 (fixer)

- The app search and the key recorder keep their reset count (places() no longer overwrites it): the search empties when the form reopens; "Use default keys" reaches the text field.
- Bind rows: a Tab stop with a focus ring; Return/Space/Menu open the menu; right-click opens it; Material's 8 % hover layer (Glass: fill); the "…" stays lit while its menu is open (SBus.menu-shown).
- The form's Default-app and Tessera-action choices are Choice.qml (`menu_choice`), not Dropdowns.
- Not fixed yet: app rows' keyboard; Print Screen / code:N keys in the recorder; focus after recording; caret into "Edit as text".

## Fixed in batch 3 (fixer)

- App rows: a Tab stop each with a ring, Space/Return choose; Material's hover layer.

## Test results

(tester)

# shell/modules/settings/ImportExport.qml ↔ surfaces/settings (src/pages/input/importexport.rs `block`, ui/settings-input.slint fieldbtns/line/buttons/skip)

Import and export of shortcuts and gestures, a part of Keyboard Shortcuts
(`shortcuts`: all three sections, "Import and export") and Gestures
(`mouse/gestures`: gestures only). The work is the shell's Keymap through
RustBridgeInput.qml (`ie` acts, feed `ie.<scope>`).

Note: src/pages/about/importexport.rs (and RustBridgeAbout.qml's `ie.*`
state 48–169) is an older copy that nothing calls; the live one is
src/pages/input/importexport.rs.

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | Shown only when Keymap.ready (shortcuts.qml 265–269, mouse-gestures.qml 284–289) | shortcuts.rs 408–410 (`if ready`), gestures.rs 210 | same |
| 2 | Group title (default "Import and export"); footer "Install zenity or kdialog…" without a dialog (138–141) | importexport.rs 58–59 | same |
| 3 | Row per section: names/details; Import… / Export… disabled while busy (143–157) | 37–41 | same |
| 4 | "Everything" (more than one section): Import… / Export all (158–168) | 42–46 | same |
| 5 | "end4 (illogical-impulse)": detail by keybinds; "Import from end4" busy/disabled while busy (169–179) | 47–57 | same |
| 6 | File dialog: zenity (save with confirm-overwrite / open, JSON filter) or kdialog; Cancel does nothing; a dialog that fails → the path field (53–80) | RustBridgeInput.qml | same |
| 7 | Path field group (no dialog): "Save as"/"Open file", field 260 with the default path, **focused at once**; Export/Import primary (disabled empty or busy), Enter = go; Cancel (182–208) | 64–96 (`fieldbtns`, gen refills the text) | **missing**: the field isn't focused when it appears (InField has `focus-input()` but fieldbtns never calls it) |
| 8 | Export: "Exported to ~/…" or "Couldn't write … Choose a folder you can write to…" (86–96) | bridge | same |
| 9 | Import: read → preview, or "Couldn't read … Check the path…"; preview errors (97–116) | bridge | same |
| 10 | Error group "Couldn't import or export" + detail; **IconButton x, tooltip "Dismiss"** (211–223) | 99–104 `btn("").icon("x")` | different (small): no tooltip |
| 11 | Status group (status, no preview): label status; Undo (arrow-counter-clockwise) while undoSnap → restore, "Import undone."; 8 s timer clears (37–41, 224–241) | 105–115 | same |
| 12 | Preview group: title, footer note; summary row "Nothing changes until you apply." with Apply changes (primary) + Cancel, or Close alone (244–278) | 118–143 | same |
| 13 | Lines: first 8 (all when shown), icon plus/power/pencil-simple 12 tertiary at 20/14, text small wrapped, +14 px (279–302) | 144–151 | same |
| 14 | "Show all N"/"Show fewer" and "See skipped (N)"/"Hide skipped" buttons; skipped lines inset 42/36, secondary; both fold on a new preview (251–261, 303–338) | 119–177 | same |

## Test plan

Setups as in pages_menubar. `open shortcuts`, scroll to the end (read y from
a shot). **Never press "Apply changes"**; Export writes a file — only into
/tmp of the arena, else skip.

IE1 `shot ie`. PASS: same rows, footer (zenity/kdialog present or not).

IE2 Import from end4: click it. PASS: a preview group (or an error) in both;
"Show all N"/"See skipped" when there are more; press **Cancel**/Close.

IE3 Path field (only when the footer says to install zenity or kdialog):
click "Import…" on "Your shortcuts" and type at once `run 'key -t x'`.
PASS (QML): the text lands in the field. Rust: write whether it does. Cancel.

IE4 Error: in the path field type /nonexistent.json, Enter. PASS: "Couldn't
import or export" with "Couldn't read /nonexistent.json …"; hover the x
1 s (QML tooltip "Dismiss"), click it → gone.

Gestures page (`open mouse/gestures`): IE1 again; only one section, no
"Everything" row.

## Fixed in batch 3 (fixer)

- The path field takes the caret as it appears.

## Test results

(tester)

# shell/modules/settings/PathField.qml ↔ settings.slint Control `r.control == "path"` (settings.slint:121); model.rs Row::path, Ctx::path_setting / choose_file; lib.rs:762 → host panels.rs:683 `picker.pick`

A path in a text field, with Choose… opening a file chooser.

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | Field 200 wide (fieldWidth), spacing 8 to the button, both vertically centred (16–23) | path branch: SField `r.width > 0 ? r.width : 200px`, spacing 8 | same |
| 2 | Commit (Enter / focus out): trim, `~` → $HOME, only when different → picked(p) (26–30) | SField `committed` → act i 1 → model.rs `path_setting` (trim, `~`, compare) | same |
| 3 | Choose…: FilePicker.pick with folder, title "Choose a folder"/"Choose a file", start = value (folder) or its directory, else `start` ("~"), filter (36–41) | act i 2 → `path_setting` → `choose_file` → lib.rs:762 `["pick", …]` → panels.rs:683 `picker.pick` (reply to settings, the row's action gets i 1) | same |
| 4 | While any file chooser is open (FilePicker.busy, global): button reads "Choosing…" and is disabled (34–35) | path branch uses `r.busy`; only phone.rs:295 sets `.busy(b("pickerBusy"))`. wallpaper.rs:139–141, 150, 205, network.rs:342–353, accounts.rs:251 don't | **different** (most path rows never show "Choosing…" / stay enabled) |
| 5 | Field placeholder (25) | SField placeholder | same |

## Test plan

F1 `open wallpaper` (Material and Glass, both shells), set the wallpaper mode so a path row ("Folder" / "Light picture") shows; `shot f1-…`. PASS: field width, button and gap as the QML.
F2 Click Choose… on that row. PASS: a file chooser opens in the arena at the current value's folder, and the button reads "Choosing…", disabled, until it's closed. Close the chooser with Esc (choose nothing).
F3 Type `~/Pictures` in the field, `run 'key -k Return'`. PASS: the setting becomes /home/…/Pictures (the field shows the expanded path after a re-open, as the QML). Put the old value back the same way.

## Fixed in batch 3 (fixer)

- "Choosing…" (and disabled) on every path row while the shell's file chooser is open (SBus.picker-busy from the look/devices feeds).

## Test results

(tester)

# shell/modules/dock/DockMenu.qml (+ glass/ContextMenu.qml) → surfaces/dock/src/{model.rs `menu`, lib.rs `open_menu` / `menu_chosen`}, ui/dock.slint `DockMenu`

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Points at the icon; its bottom barHeight + bottomGap + 8 above the screen's | 258-260 | `set_point_x`, `set_point_bottom` | same |
| Apps button: Open App Drawer / — / Remove from Dock | 265-270 | `model::menu` | same |
| Minimized: Restore / — / Close Window | 271-276 | `model::menu` | same |
| Downloads: Open Downloads, Show Recent Files / — / Remove from Dock | 277-283 | `model::menu` | same |
| Trash: Open Trash / — / Empty Trash… (disabled when empty, red, asks once: "Delete N items for good") | 284-293 | `confirm` row; `armed` in DockMenu | same |
| Empty Trash plays the trash sound | 291 | **was missing**: `shell::ask("sound.play", ["trash"])` | fixed |
| App: Open or New Window / — / Keep in Dock or Remove from Dock / (running) — / Quit | 294-300 | `model::menu`, `Action::{Launch, Pin, Unpin, Quit}` | same |
| Dismissed by a click outside or Escape | ContextMenu | full-screen overlay TouchArea; FocusScope Escape | same |
| Keys: arrows skip separators and disabled rows; Return/Space choose; hover picks | ContextMenu | `step`, `choose`, rows' `has-hover` | same |
| Opens on a spring (5 / 0.36: ω 17.7, ζ 0.64), closes in 120 ms; ≈7.5 px a character, at least 200 | ContextMenu | `animate progress` 350 ms spring(0.36) / 120 ms ease-in; `menu_width` | same |
| Its rows | — | a `Rows<DockMenuRow>` handed once per menu window (was a new model at each open) | fixed |
| Opening a panel closes it | DockLogic 148-155 | `data(ShellState)` | fixed |
| **Tester batch 1 (FAIL): Trash state**: same arena home, the QML shows the Trash full with "Empty Trash…" red, Rust empty and greyed | DockSlot 190-201 | Both read `$XDG_DATA_HOME/Trash/files` (else ~/.local/share). The arena home has **no Trash folder**, and Qt's FolderListModel, given a folder that doesn't exist, lists the **working folder** instead: checked offscreen with qml6 (a missing …/Trash/files gave `count 27`, `folder` = the process's cwd). So the QML's "full" there is that fallback (it would offer "Delete 27 items for good"); Rust's empty is the truth, kept, not copied | different (QML's fallback is a bug; Rust right). With a real Trash/files both agree |

## Test plan

### Geometry (the user's settings, 1920x1080 IRICE-TEST, scale 1, no windows open)
Items: Apps button | Downloads, Trash (pinned is empty; the divider before Downloads).
- Glass (icon 48, gap 9, pad 12, bar 72, bottom gap 6, magnification ×1.6): pill x 857..1063, y 1002..1074; icon centres x Apps 894, Downloads 970, Trash 1027; y 1038. Hover band y 961..1074.
- Material (icon 48, pad 8, gap 9, bar 64, bottom gap 16): pill x 861..1059, y 1000..1064; icon centres x 894, 970, 1027; y 1032.
Each running app adds 57 px (icon + gap) before the divider and shifts the pill left by 28.5 px: re-measure from a shot.
Use `P glide` (not jumps) so hover enters/leaves. Record with wf-recorder at 60 fps; time with tools/anim-timing.py; widget reloads with tools/region-changes.py.

1. Right-click Apps (894,y): menu "Open App Drawer / — / Remove from Dock", centred over the icon, bottom 8 px above the pill top. Record the open: Glass/Material spring 350 ms (overshoot, scale 0.94→1 from the bottom centre); Escape closes in ~120 ms.
2. Right-click Downloads (970,y): "Open Downloads, Show Recent Files / — / Remove from Dock". Choose Show Recent Files → the stack opens.
3. Right-click Trash (1027,y) with the trash empty: "Empty Trash…" greyed. Put a file in the trash (`gio trash ~/Downloads/notes.txt`), right-click again: enabled, red. Click it once → turns into "Delete 1 item for good" (red fill). Click again → trash emptied, the trash sound plays (check the host log for sound.play trash), the icon turns to the empty trash within 2 s.
4. Keys: open the Trash menu, Down/Down/Up moves the highlight skipping the separator; Return chooses.
5. App menu: with foot running, right-click foot → "New Window / — / Keep in Dock / — / Quit". Keep in Dock → config dock.pinned gains "foot". Quit → foot windows close.
6. A left click anywhere outside closes the menu; a right click outside also closes it.
7. Opening a panel while the menu is open (`ipc panels open controlCenter IRICE-TEST`) closes the menu.
8. **Trash state (batch-1 failure)**: in the arena home `mkdir -p ~/.local/share/Trash/files && echo x > ~/.local/share/Trash/files/a.txt` (both shells read this). Both: the full-trash icon and "Empty Trash…" red/enabled, confirm reads "Delete 1 item for good". Remove the folder entirely: Rust empty/greyed; the QML may show "full" counting its working folder (Qt's FolderListModel fallback): note it, not a Rust failure.

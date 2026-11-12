# shell/modules/screentools/editor/ScreenshotEditor.qml → surfaces/screentools (session in src/session.rs, flow in src/shots.rs `Core`/`Shots::command`)

With rust.surfaces: screentools the QML forwards `shot` / `edit` / `editorClose` to tessera-ui (`toRust`, lines 13-16)
and Rust owns the session. session.rs belongs to the other fixer; the rows below were read against it.

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | Session: path, screen, serial (bumped when the file changes), thumb "" / shot / deleted, editorOpen | 18-35 | `Session` | same |
| 2 | Marks in native pixels; arrays replaced so history keeps references; crop 0×0 = whole | 37-43 | `Session.items`, `crop` | same |
| 3 | Tool, colour (default Vermilion, index 1), width (medium) kept across screenshots for the run; fixed chart palette (not per theme) | 45-53 | `Session.tool/color/width`, `PALETTE` | same |
| 4 | History: commit pushes a snapshot and clears redo; add, setCrop; undo/redo swap snapshots | 65-99 | `commit`, `add`, `set_crop`, `undo`, `redo` | same |
| 5 | A new screenshot: thumbnails off → the notification with Edit / Show in Folder (QML side, before forwarding); else the thumbnail (path only replaced when the editor is closed) | 101-120, 209-234 | QML `notify` stays on the QML side; `Session::shot` + `thumb_enter` | same |
| 6 | Open the editor: from the thumbnail / ipc; a different picture while there are changes asks first (banner) and opens after | 122-142, 150-159 | `Session::open_editor` → `Opened::AskFirst` banner | same |
| 7 | Close discards (the editor asked first); then a pending open | 144-149 | `close_editor` on `After::Close` | same |
| 8 | Saved: the editor goes, the result floats in as a thumbnail (unless thumbnails are off), serial bumps | 161-171 | `Session::saved`, `thumb_enter` | same |
| 9 | Delete acts at once: the thumbnail turns into the Undo toast (8 s); Undo reopens the editor with its edits; the file goes to the Trash (gio, else rm) when the toast ends | 173-207 | `delete_shot`, `undo_delete`, `finish_delete` + `trash` | same |
| 10 | ipc `shotEditor edit FILE`, `thumbnail FILE`, `close` | 246-269 | `Shots::command` "edit" / "thumbnail" / "editorClose" | same |
| 11 | Windows made only while needed (LazyLoaders) | 271-278 | made on first need, then hidden and kept | same idea (Rust keeps the made window hidden: reopening is faster than the QML's) |
| 12 | On quit a pending deletion is final | 238-241 | `Shots::quit` | same |

## Test plan (tester)
Arena 1920×1080, both shells, both looks; the 1600×900 `/tmp/t.png` of the Annotation plan (copy it before each
destructive step: `inbox 'cp /tmp/t.png /tmp/t2.png'`).

1. **Thumbnail → editor**: `inbox 'ipc shotEditor thumbnail /tmp/t.png'` → thumbnail slides in bottom right (card ≈ x 1684..1896, y 931..1056); `P click 1790 993` → the editor opens centred.
2. **Ask before switching**: draw a pen stroke, then `inbox 'ipc shotEditor thumbnail /tmp/t2.png'`, click the new thumbnail → banner "Save changes before opening the new screenshot?" with Keep editing / Discard changes / Save. Discard → the editor reopens on t2 with no marks.
3. **History**: draw 3 marks; Ctrl+Z ×2, Ctrl+Shift+Z ×1 → 2 marks; Undo/Redo buttons enable/disable to match.
4. **Tool memory**: pick Arrow (A) and colour 4 (Orchid), close, open another picture: Arrow and Orchid still chosen.
5. **Save**: draw, Ctrl+S → editor fades out (200 ms), thumbnail of the saved file slides in; the file on disk has the marks.
6. **Delete + Undo**: in the editor click the trash (bottom bar, first action button) → editor goes, "Screenshot deleted" toast with Undo; click Undo → editor back with the marks. Delete again and wait 8 s: `/tmp/t2.png` is gone (in the Trash).
7. **ipc close**: `inbox 'ipc shotEditor close'` with the editor open → it closes without asking.

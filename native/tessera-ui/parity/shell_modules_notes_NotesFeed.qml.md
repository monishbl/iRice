# shell/modules/notes/NotesFeed.qml → surfaces/notes/src/lib.rs (`message`, the shell side of the quick note)

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | Sends the notes folder (with ~ for home) when it changes, when tessera-ui starts or draws | 12-26 | `folder` | same |
| 2 | Asks `save <text>` → Notes.create; empty text saves nothing | 27-33 | `save` | same |
| 3 | Asks `openFolder` → closes and opens the folder | 34-37 | `openFolder` | same |
| 4 | Tells saved / failed paths | 40-50 | `saved` / `failed` | same |

## Test plan (tester)
1. Open the quick note (`inbox 'ipc panels open quickNote IRICE-TEST'`): the footer shows the notes folder with `~`.
2. Type text, Ctrl+Enter → a new file in that folder; the dialog closes.
3. Click the folder line → the folder opens in the file manager, the dialog closes.

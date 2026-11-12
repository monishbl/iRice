# shell/modules/dock/DownloadsFiles.qml → surfaces/dock/src/stack.rs

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| ~/Downloads, newest first, files only, no hidden ones, up to 12 | 15-22 | `stack::newest` | same |
| The list stays live (FolderListModel watches the folder) | 16 | **was listed once at opening**; now listed again every 2 s while open and changed in place (`Rows`, keyed by path) | fixed (≤ 2 s late) |
| Open a file (closes the stack); right-click shares it | 27-34 | `S::Open` → `xdg-open` / `panels.share` | same |
| Open the folder (closes the stack) | 35-38 | `S::Folder` → `open_folder` | same |

## Test plan

### Setup
Put report.pdf, photos.zip, shot.png, notes.txt, clip.mp4 in ~/Downloads (arena home), plus a hidden `.x` and a folder `dir`.

1. Click Downloads (970,1038 Glass / 970,1032 Material): 5 files, newest first, no `.x`, no `dir`.
2. **Live list (fixed)**: with the stack open, `echo y > ~/Downloads/new.txt` → within 2 s new.txt appears first; the other cells don't flash (they move, keeping their icons).
3. Left-click report.pdf → the stack closes, the file opens (xdg-open). Right-click shot.png → the stack closes, the Share panel opens with it.
4. "Open in Files" → the stack closes, the file manager opens ~/Downloads.

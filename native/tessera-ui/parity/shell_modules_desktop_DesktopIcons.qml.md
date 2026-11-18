# shell/modules/desktop/DesktopIcons.qml → lib.rs `place_icons`, `icon_rows`, `icon`, `file_menu`, `stack_files`; grid.rs; files.rs; ui/desktop.slint `IconCell`, stack panel

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Desktop folder, re-read when it changes | 22-52 | `desktop_dir`, `refresh_folder` every 3 s (applied only when different) | same |
| Stacks by kind, newest on top | 54-77 | `grid::items` | same |
| Spots saved per key, else from the top right around the widgets | 80-147 | `place_icons`, `move_icon` | same |
| Icon: theme icon for the file / folder, fallback text-x-generic; images as thumbnails | 226-258 | `file_icon`, `icon_name`, `thumbnail` | same; Colloid and Adwaita checked (tests/icon_themes.rs) |
| A stack's three newest fanned (−3 px, ±6°) | 227-240 | `fan`: a kept `Rows` per stack (was a new model each refresh) | fixed |
| Newest first by the time to the millisecond (FolderListModel's fileModified) | 76 | **seconds: files made in the same second fell back to name order, so the Documents stack showed a text file on top where QML shows the PDF** → `modified` in ms | fixed (batch 3) |
| Selection tile; name white with raised shadow, 2 lines | 211-281 | `IconCell` | same |
| Name: two lines, wrapped anywhere, elided in the middle | 270-275 | **Slint elides at the end and wrapped at words, any number of lines** → `elide_middle` (26 characters on the desktop, 14 in an open stack) before it's drawn, `char-wrap`, two lines at most | fixed (batch 2) |
| Click selects / toggles a stack; double-click opens; right-click its menu; drag (6 px) selects and drops on the nearest free spot; springs home | 284-311 | `IconCell`, `icon()` | same |
| Bare click clears selection, closes a stack | 177-183 | `M::Bare` | same |
| File menu (Open, Quick Preview, Share…, Show in Files, Copy Path, Move to Trash + Undo); stack menu | 153-174 | `file_menu` | same |
| Lock/unlock zoom with the widgets | 193-206 | **was missing** → `IconCell` `fx` | fixed |
| Open stack: a glass panel, 4 a row, ≤ 16; hover layer; click opens; right-click menu | 314-394 | stack panel; files a kept `Rows` (was new each refresh) | fixed |
| Icons follow their file | — | `Win.icons` keyed by path/stack key (was by position: a new file shifted every icon's data) | fixed |

## Test plan

Geometry (1920x1080 TESSERA-TEST, the user's 4 widgets): grid origin 32,54; a cell 164 px, gap 16; a cell (c,r) starts at x = 32 + 180c, y = 54 + 180r; s = 164x164, m = 344x164, l = 344x344. The user's layout: clock m at (0,0) → 32..376 x 54..218; stack m at (0,1) → 32..376 x 234..398; batteries m at (0,2) → 32..376 x 414..578; shortcuts m at (2,0) → 392..736 x 54..218.
Both looks: run once with appearance.look "glass" (the user's) and once "material" (dark).

1. Stack order: `touch ~/Desktop/notes.txt; sleep 0.3; touch ~/Desktop/report.pdf` (same second). PASS: the Documents stack's top (front) icon is the PDF's (Colloid red), as QML.
2. Put in the arena's ~/Desktop: a folder `Work`, `notes.txt`, `report.pdf`, `photo.jpg`, `song.mp3`, `archive.zip`. The user has Use Stacks on: stacks appear top-right (x ≈ 1840 down). PASS (Colloid, the user's theme): every stack's fanned icons are Colloid's (folder blue-grey Colloid folder, document, pdf, image thumbnail, audio, archive) — none blank, none a generic fallback where the QML shows a real icon. Compare the QML shot. Repeat with Use Stacks off (right-click desktop → Use Stacks): each file's own icon.
3. Click a stack: the panel opens beside it with its files; hover a file (state layer); click outside: closes.
4. Drag `notes.txt` (Use Stacks off) 200 px left: it follows after 6 px, drops on the nearest free spot, springs there; `config.json` iconPositions has it.
5. Long names: `touch ~/Desktop/a-very-long-file-name-for-testing-middle-elide.txt` with Use Stacks off. PASS: its label is at most two lines, cut in the middle (starts "a-very-long-", ends "-elide.txt" with … between), like the QML's. In an open stack (Use Stacks on, open Documents) its name is one line cut in the middle.
6. `touch ~/Desktop/new.txt` while recording: only the new icon appears; existing icons don't blink.
7. **No reload on window events**: `pointer move 1500 600`; start a 12 s recording; at +1 s `foot &`; at +3 s a second `foot &`; at +5 s focus the first (click it); at +7 s close one (`hyprctl dispatch 'hl.dsp.window.close({ window = "class:foot" })'`); at +9 s close the other. Then `python3 tools/region-changes.py rec.mp4 32 54 704 524` over the widgets (crop the windows' area out if they cover it: open the windows with `hyprctl dispatch movewindowpixel` to x ≥ 800, or use `tiling` off and check only the part of the region the windows don't cover). PASS: the widgets' pixels don't change when windows open, focus or close (only the clock's minute may change). FAIL if any widget fades/grows in again (the appear: opacity 0→1 260 ms, scale 0.9→1 420 ms) or blinks. Same in the QML pass for reference.

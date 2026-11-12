# shell/modules/quicklook/QuickLook.qml → surfaces/screentools (src/quicklook.rs, ui/quicklook.slint)

Springs (Qt steps 16 ms: ω = √(62.5·spring), ζ = 62.5·damping / 2ω; Slint d = 2π/ω, bounce = 1 − ζ): MaterialSpring "fast" (springFast 12.8/0.54) → 222 ms spring(0.4); ElasticControl's swell (springPress 7.5/0.3) → 290 ms spring(0.57).
Qt.darker(c, f) divides the HSV value by f; Slint `darker(x)` divides by 1 + x.

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | Opens on Panels.quickLook(path) on the focused screen (the Search query forwarded); an empty path closes | 25-35, 63 | `command("open" / "close")` | same |
| 2 | Window ready when it opens | 37-58 (LazyLoader) | `start` → `Q::Prepare` 1.5 s after start | **was** made on the first Space (the first preview late); now made ahead |
| 3 | Scrim: Glass black 30 %, Material scrim 32 %, × progress; a click closes it; clicks on the card don't | 159-166, 226 | scrim TouchArea | same |
| 4 | progress 200 ms OutCubic in, 140 ms InCubic out; card opacity = progress, scale 0.94 → 1 | 139-149, 218-219 | `animate progress` | same |
| 5 | Kind from `file --mime-type` (a directory first), else the extension; subtitle size · date ("1.5 MB · 2026-11-07 10:00", folders "N items") | 94-137, 250 | `kind_of`, `size_text`, probe thread | same |
| 6 | Card: the preview's shape within 80 % of the screen (picture size; video 1280×720; PDF page or 850×1100; text and folders 760×900; else 420×260), width ≥ 360, header 52 | 204-216 | `nat-w/h`, `k` | same |
| 7 | Glass r 20 (Material Theme.radius.panel) | 221-225 | `card` | same (Material drawn as surfaceContainer with a shadow, not the shell's Glass) |
| 8 | Header: close 12 in, name (body 600, middle elide) and subtitle (small, secondary); Text (pictures), Share (not folders), Open | 229-313 | header | same (Slint elides at the end; no tabular figures) |
| 9 | Open: Glass accent 28 high, hover Qt.darker 1.1, pressed 1.2; Material primary pill 36 high with state layers, r 8 pressed on MaterialSpring fast, ElasticControl swell; "Open File" / "Open file", "Open Folder" / "Open folder" | 283-312 | open button | **was** darker(0.09) / darker(0.17), radius on 250 ms spring(0.55), swell 1.04 on 300 ms spring(0.35); now darker(0.1) / darker(0.2), 222 ms spring(0.4), 1 + min(6 %, 10 px / size) on 290 ms spring(0.57) |
| 10 | Keys: T Live Text (pictures); in Live Text Ctrl+C copy, Ctrl+A all, Esc clears a selection; Space / Esc close; Return opens it in its app | 171-203 | `on_key` → `key` | same |
| 11 | Picture decoded off the UI thread at ≤ 1.5 × the fitting box, fitted (PreserveAspectFit); Live Text exactly over it | 330-351 | "image" thread, `pic-w/h` | same (Rust's box is the screen × 0.8 × 1.5, without taking off the header) |
| 12 | Live Text bar 12 above the card's bottom, room made under the picture | 207-208, 316-322 | `LiveTextBar`, `footer` | same |
| 13 | PDF: first page via pdftoppm into the cache, off the UI thread | 353-368 | "pdf" thread | same |
| 14 | Video plays on open; a click pauses; progress (Glass 3 px fill / accent, Material wavy) | 371-383, 469-497 | ffmpeg frames + ffplay, `wave` | known difference: Slint has no media player (frames from ffmpeg, sound from ffplay, pause = SIGSTOP) |
| 15 | Audio / other / empty folder: 96 icon (Material on a 144 shape: sunny for audio, else cookie12) and "No preview for this kind of file" / "Empty folder" | 384-407, 461-467 | icon column | same |
| 16 | Audio play: Glass a 44 IconButton (play / pause, filled 22); Material a 64 primaryContainer shape, square paused, circle playing, cookie9 while pressed, ElasticControl swell | 408-460 | play button | **was** no cookie9 while pressed, swell on 300 ms spring(0.35); now `press-shape` (cookie9) while pressed, swell on 290 ms spring(0.57). Shapes switch without morphing (known) |
| 17 | Text: the first 200 KB, mono 12, wrapped, selectable (selection accent 40 %) | 499-517 | Flickable + TextInput | same (no scroll bar) |
| 18 | Folder: entries folders first, no hidden ones; rows 32 high, icon 20, 10, the name (body) | 519-550 | `entries`, rows | same (Rust sorts names ignoring case; FolderListModel minds case) |
| 19 | Folder loading | 520-524 (FolderListModel: async, rows at once, icons per row) | "folder" thread → `Q::Folder`, then `Q::FolderIcons`; `rows: Rows` | **was** the folder read on the UI thread for the count, nothing listed until every row's icon was drawn, then a new model; now the names come at once into one Rows, each kind of icon is drawn once and filled into the same rows in place |
| 20 | Share closes and opens the share sheet with the file | 269-280 | `Q::Share` | same |
| 21 | Input only while shown; keyboard Exclusive while shown | 66, 147-149 | `after_frame`, `open` / `close` | same |
| 22 | Icons follow the look | (Icon per look) | `icons()` on a look change | **was** the download icon not refreshed on a look change; now all of them |

## Test plan (tester)
Arena 1920×1080, both shells, both looks. Files: `inbox 'ffmpeg -loglevel quiet -f lavfi -i testsrc=size=1600x900 -frames:v 1 -y /tmp/ql.png; ffmpeg -loglevel quiet -f lavfi -i sine=d=6 -y /tmp/ql.ogg; seq 1 3000 > /tmp/ql.txt; mkdir -p /tmp/qldir/sub /tmp/qlempty; touch /tmp/qldir/a.txt /tmp/qldir/b.png'`.
Picture 1600×900: card 1443 × 864 at (238, 108); header centre y 134; Open's right edge x 1669 (Glass) / 1665 (Material).
1. **First open is immediate** (row 2): fresh shell, wait 3 s, `rec ql-first 1 "inbox 'ipc panels quickLook /tmp/ql.png'"` → the first card frame no later in Rust than in QML (± 1 frame). `wtype -k space` closes.
2. **Curves** (row 4): `rec ql-open 1 "inbox 'ipc panels quickLook /tmp/ql.png'"` → 200 ms OutCubic, scale 0.94 → 1; `rec ql-close 0.5 "wtype -k Escape"` → 140 ms. Compare frame strips.
3. **Geometry** (rows 6, 11): `shot ql-png` → card edges x 238 / 1681, y 108 / 972 in both shells (± 1).
4. **Open button** (row 9): `P glide 1630 134 200`, `shot ql-open-hover` → colour equals QML's (Glass: accent ÷ 1.1). Material: `rec ql-open-press 0.7 "P down; sleep 0.25; P glide 960 600 1; P up"` (released away, nothing opens) → corners 18 → 8 and back on a quick spring, the pill swells ≈ 6 % and settles ≈ 0.3 s; curves within ± 1 frame.
5. **Live Text** (rows 10, 12): picture open, `wtype -k t` → Text selected, the bar appears; `wtype -M ctrl -k a -m ctrl` selects all; Esc clears the selection; Esc again closes. Reading needs tesseract: if missing, the bar's "Install" shows (mark reading **not testable here**).
6. **Folder** (rows 18, 19): `inbox 'ipc panels quickLook /usr/share/applications'` → names at once, icons fill in after; scroll down a few rows (`P glide 960 540 100`, wheel) before they land: the list stays where it is (Rust: rows not rebuilt). `/tmp/qldir` → "sub" first, then a.txt, b.png; subtitle "3 items · date". `/tmp/qlempty` → folder icon + "Empty folder".
7. **Text** (row 17): `/tmp/ql.txt` → mono 12 lines; card 686 × 864 at (617, 108); wheel scrolls.
8. **Audio** (rows 15, 16): `/tmp/ql.ogg` plays (sound **not checkable** in the arena; the progress line moves). Glass: the 44 px button under the icon pauses and plays. Material: `P down` on the 64 px shape (centre ≈ (960, 620)) → `shot ql-play-press` shows the cookie9 shape swelled; release → square (paused) / circle (playing).
9. **Return** opens it (Rust log `files.open /tmp/ql.png`) and closes the card; **Share** (left of Open) closes and opens the share sheet.

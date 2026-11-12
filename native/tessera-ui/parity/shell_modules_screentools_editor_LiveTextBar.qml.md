# shell/modules/screentools/editor/LiveTextBar.qml → surfaces/screentools (`LiveTextBar` in ui/shoteditor.slint; state in src/livetext.rs `bar_message`/`chip_*`, src/ltui.rs `bar_in`; the editor's copy/said in src/shots.rs)

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | "said" (what was just copied) for 2 s, cleared when the picture changes | 18-33 | `live_say` 2000 ms timer; `Live::start` clears it | same |
| 2 | Message per state: Finding text… / No text found in this picture / Couldn't install it: … / Text recognition needs Tesseract / Couldn't read the text in this picture / N word(s) selected / Drag across text to select it | 35-42 | `bar_message` | same |
| 3 | Glyph: check after a copy, circle-notch reading, text-aa ready/none, warning-circle otherwise; danger colour for error/missing | 43, 96-111 | `glyph`, `danger` | same |
| 4 | Reading: Material its Spinner 20; Glass the glyph spinning 900 ms | 88-110 | `spinning` → `StSpin` 20 / 16 | same |
| 5 | Chips: links, emails, phones (≤ 4), then Wi-Fi codes; labels ≤ 22 chars; "Copy …" for a phone without a tel: handler; Wi-Fi "Join …" / "Copy Wi-Fi password"; tooltips; tap acts and says | 45-70, 122-134 | `livetext::actions`, `chip_label`, `chip_tooltip`, `live_chip` | same |
| 6 | Only the chips that fit beside the status and the buttons (FontMetrics widths) | 46-59 | `chips_that_fit` with ~7 px per char, room = canvas − 380 | known difference: Slint gives Rust no text metrics, widths are estimated (a chip more or less at the edge) |
| 7 | Chips list kept while the bar's message changes | 122-134 (Repeater over a slice) | `bar_in` → `lt_chips: Rows<LtChip>` | **was** a new chips model on every `push_live` (every drag step over the words, every "said"), so a hovered chip lost its hover and tooltip; now one model, changed in place |
| 8 | Copy text (secondary, Ctrl+C, only with a selection); Copy all text (ghost, icon only without a selection); Try again (error); Install Tesseract / Installing… busy, disabled while Software Update runs | 137-176 | buttons | same |
| 9 | Height 40 / 36; hidden while status is "" | 72-74 | `height`, `visible` | same |
| 10 | Width capped at the room left of the actions | 72 | preferred width | known difference: not capped (chips are already limited by row 6) |

## Test plan (tester)
Arena 1920×1080, both shells, both looks; needs tesseract installed in the arena (else row 8's Install path: mark
"missing" states only). Test picture from the Annotation plan (has "Invoice total 42.00" and a link).
`inbox 'ipc shotEditor edit /tmp/t.png'`, then `wtype -k s` (Select text). Bottom bar y: Glass 953, Material 947; bar starts x Glass 280, Material 301.

1. **Reading → ready**: `rec lt-read 3 "wtype -k s"`: "Finding text…" with the spinner (Material 20 px primary indicator, Glass 16 px) then "Drag across text to select it" with chips "example.com/pay"; compare the first ready frame time QML vs Rust.
2. **Chip hover survives a drag** (row 7): `P glide <chip x> 953 200` (the first chip right after the message, ≈ x 560 Glass), wait 800 ms (tooltip "Open in your browser"), then `wtype -M ctrl -k a -m ctrl` (select all: the message changes). Rust before the fix: tooltip and hover vanished; now they stay, as QML.
3. **Copy**: with words selected (`-M ctrl -k a`), click "Copy text" → message "Text copied" with a check for 2 s, then back to "N words selected". `wl-paste` in the arena shows the text.
4. **Copy all**: `-k Escape` (clears the selection), click "Copy all text" → "All text copied".
5. **No text**: open a plain picture (`magick -size 800x600 xc:gray /tmp/blank.png`, `ipc shotEditor edit /tmp/blank.png`, `wtype -k s`) → "No text found in this picture", no chips.
6. **Missing** (mark not testable here if tesseract can't be hidden in the arena): "Text recognition needs Tesseract" + Install Tesseract.

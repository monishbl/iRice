# shell/modules/emoji/EmojiStore.qml → surfaces/emoji (src/data.rs; placement and pick in src/lib.rs)

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | emoji.json read off the UI thread; groups, emoji (name, keywords, shortcode or snake_case name, group, tones), kaomoji and symbol sections | 24-80 | `Data::parse`, `load` (thread) | same; Rust reads it at start, the QML on the first open (known difference: earlier, so the first open needn't wait) |
| 2 | Load failure → `failed` → "Emoji couldn't load" + Try again reloads | 74-79, 37-41 | `M::Loaded(Err)`, `state_act("dataError")` | same |
| 3 | Usage, tones and KLIPY customer id in $XDG_STATE_HOME/tessera/emoji.json (same file and format) | 84-128 | `Remembered` | same (Rust writes at once; the QML 400 ms later — not visible) |
| 4 | Frequently used: count × ½^(age / week), most first; ≤ 200 kept, oldest dropped; count ≤ 999 | 130-145 | `frequent`, `record` | same |
| 5 | Skin tones: modifier after the first character (replacing U+FE0F) or the five spelled forms; per-emoji tone remembered; names Default…Dark | 149-194 | `with_tone`, `tone`, `set_tone`, `TONE_NAMES` | same |
| 6 | Search: every query word starts a word of name/keywords/shortcode; score exact 0 < prefix 1+len/100 < whole word 1.5 < contains 2+ < other 3+; ties in list order; punctuation = literal characters first; `:shortcode:` and `thumbs_up` read as words | 198-262 | `score`, `rank`, `literal`, `merge`, `search` | same |
| 7 | Placement: pointer and active window via hyprctl (one process) | 266-299 | two `hypr::request`s on a thread | same |
| 8 | Pick: record use (emoji without tone as key), close, wl-copy; unless copy-only: 140 ms later focus the old window by address, 60 ms later wtype the text | 303-337 | `pick` | same |

## Test plan (tester)
Arena 1920×1080, panel centred x 750..1170, y 326..754; search field y 338..374; list from y 418; first search-result cell (col 0) centre (784, 468).
1. Open, type `wtype smile` → the first results are the same emoji in the same order as the QML (compare frames of the grid y 418..694).
2. Type `:heart:` and `<3` → same results and order as QML (literal matches first for `<3`).
3. Pick 😀 with Enter in a text field app (e.g. `foot` in the arena; mark not testable here without one): the text arrives ≈ 200 ms after close; reopen → it is first under "Frequently used".
4. Right-click a toned emoji (search `wave`, `P click 784 468` with the right button: `P down`/`P up` on button 3 if the harness supports it, else hold 0.5 s), choose a tone; reopen and search `wave` → it shows in that tone; footer says "Waving hand, … skin tone".

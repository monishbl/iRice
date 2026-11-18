# shell/modules/desktop/NotesWidget.qml → ui/desktop.slint `NotesW`; lib.rs `act` "note"

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Note glyph and header | all | `NotesW` | same |
| Wrapping text field in a clipped box | 40-63 | TextInput | same |
| Saved 500 ms after typing stops | 55, 65-69 | `note_timer` 500 ms | same |
| Escape leaves the field | 56 | `key-pressed` Escape | same |
| Placeholder "Click to write a note" | 57-62 | T | same |

## Test plan

Geometry (1920x1080 TESSERA-TEST, the user's 4 widgets): grid origin 32,54; a cell 164 px, gap 16; a cell (c,r) starts at x = 32 + 180c, y = 54 + 180r; s = 164x164, m = 344x164, l = 344x344. The user's layout: clock m at (0,0) → 32..376 x 54..218; stack m at (0,1) → 32..376 x 234..398; batteries m at (0,2) → 32..376 x 414..578; shortcuts m at (2,0) → 392..736 x 54..218.
Both looks: run once with appearance.look "glass" (the user's) and once "material" (dark).

1. Add Note Small. Click inside it, type `hello world` (`wtype`). PASS: text appears as typed; after ≈0.5 s idle the state file has it (`desktopNotes`). Press Escape: the caret leaves. Open and close a foot window: the text stays and the field doesn't redraw empty.

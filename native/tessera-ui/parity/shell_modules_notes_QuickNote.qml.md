# shell/modules/notes/QuickNote.qml (+ common FabMenu.qml) → surfaces/notes (src/lib.rs, ui/notes.slint)

Dialog.qml's spring is Theme.motion.spring + 0.4 / damping + 0.08: Material 6.48/0.58 → ω 20.1, ζ 0.90 → 312 ms spring(0.1); Glass (subtle) 4.2/0.40 → ω 16.2, ζ 0.77 → 388 ms spring(0.23). It closes in panel × 0.6 (192 ms) on the look's exit curve.

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | Dialog 440 wide (padding 24 / 16), opens on Dialog's spring, closes in 192 ms exit | 24-30, glass/Dialog.qml 21-41 | `animate openness` | **was** 420 ms spring(0.15) in both looks, Glass exit for both; now per look as above |
| 2 | Opening puts the kept draft back, cursor at the end, field focused; closing keeps the draft | 32-44 | `start` | same |
| 3 | Editor: wraps, scrolls (stops at bounds); Ctrl+Enter saves; Esc closes | 120-160 | editor, keys | same |
| 4 | Footer: folder (elided middle) + Save button "Save note ⌃↵" / "Saving…", disabled while empty or saving | 175-225 | footer | same |
| 5 | Save (Material): filled pill 40 high, 8 % / 10 % state layers, corners 8 while pressed on springFast, press swell; Glass 32 high, accent (darker 1.15 pressed / lighter 1.08 hover), 97 % while pressed (100 ms) | 185-210 | save button | **was** 200 ms spring(0.3) corners, Material 1.04 on 100 ms; now 222 ms spring(0.4), 1 + min(6 %, 10 px / width) on 290 ms spring(0.57) |
| 6 | Saved → field emptied, draft dropped, closes; failed → button back | 58-70 | `saved`/`failed` | same |
| 7 | Material FAB (FabMenu) 56 px: corners 16 → 20 pressed → round open on springPress; colour on `hover`; press swell; icon + → × turning 90° on springPress | FabMenu 120-150 | FAB | **was** 300 ms spring(0.3) corners and turn, 250 ms spring(0.3) swell, 150 ms colour; now 290 ms spring(0.57), `Theme.hover` |
| 8 | FAB items come out nearest first, 30 ms apart (only when opening), each on springPress with damping + 0.1 | FabMenu 55-75 | menu pills `t` | **was** 250 ms emphasized bezier with the stagger both ways; now 290 ms spring(0.42), stagger only when opening |

## Test plan (tester)
Both looks. Open: `inbox 'ipc panels open quickNote TESSERA-TEST'`.
1. `rec qn-open 1 "ipc panels open quickNote TESSERA-TEST"`: Material settles in ≈ 0.3 s with ≤ 1 % overshoot; Glass ≈ 0.4 s, a little more overshoot; `-k Escape` closes in 192 ms. Compare with QML ± 1 frame.
2. Type `hello`, close, reopen → "hello" back, caret at the end.
3. Save button: empty → disabled (Material 12 % / 38 %). With text, Material `P down` on it (bottom right of the card) → corners 8 and ≈ 6 % swell in ≈ 0.22–0.29 s; Glass → 97 %.
4. `-M ctrl -k Return -m ctrl` → saves (a file in the notes folder), closes; reopen → empty.
5. Material: click the FAB (bottom right of the editor) → it rounds and the + turns to × with a small overshoot; the items spring out nearest first; click again → they go back together.

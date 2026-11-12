# shell/modules/emoji/EmojiSlots.qml → surfaces/emoji (src/lib.rs: `window`, `closed`, `retire`, `M::Prewarm`)

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | One picker window per screen, made on its first open there | 9-21 | one window, moved to the screen it opens on (`window`) | same (visible behaviour: it opens on the focused screen) |
| 2 | Kept 5 minutes after closing (linger 300000), so reopening within them doesn't draw every emoji glyph again | 21-24 | `RETIRE_SECS` | **was** 10 s (the next open after 10 s made a new window and waited for its unseen warm-up frame); now 300 s |
| 3 | The first open is quick | (QML warms glyphs while open, PickerWindow 168-225) | `M::Prewarm`: 3 s after the data is read the window is made and drawn once unseen | same intent; Rust's is ahead of the first open (the QML's first open pays for it) |

## Test plan (tester)
Arena 1920×1080, panel centred x 750..1170, y 326..754.
1. Open (`inbox 'ipc panels open emoji IRICE-TEST'`), `wtype -k Escape`, wait 60 s, `rec em-reopen 0.6 "ipc emoji open"` → the first frame with the panel comes within ≈ 1 frame of the command and the spring starts at once (before: after 10 s closed, the reopen waited for a warm-up frame, ≥ 1 extra frame and sometimes a visible hitch). Compare with QML (also instant within 5 min).
2. Wait 6 minutes closed, reopen: both QML and Rust make the window again (a short wait is expected in both).

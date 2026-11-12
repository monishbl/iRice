# shell/modules/today/TodayOptions.qml

Rust: ui/today.slint `OptionsSheet` (+ Segmented, Toggle, NameField);
src/lib.rs `set()` (today.set to the shell, shown at once).

| Behaviour | QML | Rust | Status |
|---|---|---|---|
| Shown over the panel (Options, or the widget's menu); focus to the first choice | 15 | begin_options / options_open | same |
| Esc goes back to the day | 16-19 | OptionsSheet key-pressed | same |
| Sheet in the panel's colour; clicks stay on it | 22-29 | Rectangle + TouchArea | same |
| Scrolls (StopAtBounds) | 45-49 | Flickable | same |
| "Open Today": mode Segmented + hint | ~60-80 | Segmented (mode) | same; Material corner spring was 250 ms spring(0.54) → **fixed** springFast fitted (150 ms spring(0.28)); the pressed segment's widening (squeeze) is **not drawn** |
| Entrance Segmented + hint | ~80-100 | Segmented | same |
| Name field (placeholder from the account, committed on Enter / focus out) | ~100-120 | NameField | same |
| Chime switch | ~120-130 | Toggle | Material handle spring was 250 ms spring(0.54) → **fixed** springFast (150 ms spring(0.28)); Glass knob 220 ms emphasized: same |
| Done: commits a name being typed, back to the day; Enter/Space; focus ring | 135-173 | done-button | colour faded 150 ms where the QML changes at once → **fixed** |

## Test plan (both looks)

1. Today open, click Options: the sheet covers the panel, focus on the
   first choice. Arrow keys change "Open Today"; in Material the chosen
   segment turns fully round on a quick spring (≈ 150–220 ms).
2. Click the chime switch: Material's handle grows and moves on the quick
   spring; Glass's knob slides 220 ms.
3. Hover/press Done: colour changes at once (no fade), Enter or click
   goes back to the day; Esc too.
4. Type a name, click Done: the greeting uses it.

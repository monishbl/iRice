# shell/modules/screentools/editor/EditorButton.qml → surfaces/screentools (`EditorButton` in ui/screentools.slint)

`ui/screentools.slint` is owned by the other fixer of this crate; the differences below are left for them (exact
changes in the report). Spring conversion as in the spotlight files: springFast 12.8/0.54 → 222 ms spring(0.4);
springPress 7.5/0.3 → ω 21.65, ζ 0.43 → 290 ms spring(0.57).

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | Variants ghost / secondary (1) / primary (2) / danger (3); `selected`, `busy`, `vibrant`, `tipBelow` | 13-23 | properties | same |
| 2 | Material content colour (disabled 38 % onSurface, primary onPrimary, danger error, selected onPrimary/onSecondaryContainer, vibrant onPrimaryContainer, tonal onSecondaryContainer, text button primary, icon button onSurfaceVariant) | 29-47 | `mfg` | same |
| 3 | Material container + state layer 8 % hover / 10 % press (filled: tinted; else the layer alone) | 48-60 | `mbg` | same |
| 4 | Size 40 (Material) / 36 (Glass); width = height without a label, else row + 32/28 | 62-63 | height/width | same |
| 5 | Material corner: round, 12 selected, 8 pressed, on MaterialSpring "fast" (springFast) | 66-71 | `animate border-radius { 250ms; spring(0.55) }` | **known difference (not my file)**: should be `222ms spring(0.4)` |
| 6 | Glass colours: primary accent, hover Qt.darker 1.1, press Qt.darker 1.25; selected accent 18 % / 28 % pressed; secondary fill; hover fillHover, press fillPressed; colour animates Theme.motion.hover | 72-87 | `gbg`, `animate background { Theme.hover }` | **known difference (not my file)**: Slint `darker(f)` divides HSV value by 1+f, so it should be `darker(0.1)` / `darker(0.25)` (now 0.09 / 0.2: a slightly lighter primary hover/press) |
| 7 | Glass press: scale 0.97, ScaleAnimator Theme.motion.press (100 ms) OutCubic | 88-89 | `cubic-bezier(0, 0, 0.58, 1)` 100 ms | **known difference (not my file)**: OutCubic is `cubic-bezier(0.33, 1, 0.68, 1)` |
| 8 | Material press: ElasticControl swell 1 + min(0.06, 10/max(w, h)) on springPress | 65, ElasticControl | `1 + min(0.06, 10px / width)` on `300ms spring(0.35)` | **known difference (not my file)**: should be `290ms spring(0.57)` and use max(width, height) |
| 9 | Focus ring: Material 3 px secondary at −5, Glass 2 px accent at −4 | 91-101 | `if keys.has-focus` | same |
| 10 | Icon 16 box (Material glyph 18, filled when selected); busy: Glass circle-notch spinning 900 ms, Material Spinner 20 | 103-138 | `StIcon` / `StSpin` | same (StSpin draws the arc for both) |
| 11 | Label: Material 14 px, Glass Theme.font.body, weight Medium | 139-145 | Text | same |
| 12 | Click, Space, Return; pointing-hand cursor; disabled/busy take nothing | 147-160 | `touch`, `keys` | same |
| 13 | Tooltip after 600 ms hover (not while pressed), fades Theme.motion.hover; above or below 8 px; Material inverseSurface r 4 h 24, Glass #3A3A3F/#1C1C1E r 8 h 26; shortcut at 70 % / 55 % | 162-198 | Timer + `tip` | same |
| 14 | Width buttons (editor): "A" in 11/14/18, dot 4/7/11 (selected: in the ink), block for redact | ShotEditorWindow 1110-1139 | `glyph`, `glyph-size`, `dot-color` | same |

## Test plan (tester)
Arena 1920×1080, both shells, both looks. `inbox 'ipc shotEditor edit /tmp/t.png'` (the 1600×900 test picture of the
Annotation plan). Tool buttons (top bar, y Glass 127 / Material 133): pen at x Glass 391 / Material 424.

1. **Hover + tooltip**: `P glide 470 127 200` (Glass; Material 508 133) → `rec tip 1.2 "sleep 0.1"`: hover fill at once, tooltip "Arrow A" fading in ≈ 600 ms after; both shells within 2 frames.
2. **Press**: `P down`, `dev shot press`, `P up`. Glass: 97 % scale; Material: swells ~6 % and its corner squares to 8 then springs back (record `rec press 0.6 "…"` with `P down; sleep 0.15; P up`; Material corner/scale curve differs as listed in rows 5 and 8 until fixed).
3. **Primary hover (Glass)**: hover the Save button (bottom right, left of the copy button: ≈ 1570, 953) after drawing one mark; colour compare QML vs Rust (row 6: Rust slightly lighter until fixed).
4. **Keyboard**: `wtype -k Tab` until a button shows the focus ring; Space activates it.
5. **Busy**: click Copy (≈ Glass 1410, 953) → spinner in the button until the toast.

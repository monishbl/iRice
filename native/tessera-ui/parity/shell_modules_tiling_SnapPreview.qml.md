# shell/modules/tiling/SnapPreview.qml → surfaces/tiling (src/lib.rs, ui/tiling.slint)

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | Full-screen overlay layer, no keyboard, click-through | 40-44 | layer surface, empty input region | same |
| 2 | The preview moves and resizes to the target over 200 ms OutCubic | 46-56 | `animate px, py, pw, ph` | same |
| 3 | Fades in 150 ms / out 100 ms; scale 0.96 → 1 over 150 ms OutCubic | 52-58 | `animate o`, `animate s` | same |
| 4 | Glass: a glass rectangle; Material: primaryContainer tone outlined in primary | 60-80 | look branches | same |
| 5 | Gone once faded out | — | `faded` | same |

## Test plan (tester)
Both looks. Drag a foot window by its title bar to the left screen edge (`P down` on the title, `P glide 2 540 400`, hold) → the preview shows the left half; glide to the top-left corner → it moves to the quarter over 200 ms; `P up` → it fades out in 100 ms. Or `inbox 'qs ipc call tiling snap left'`. Record and compare the 200 ms move with QML ± 1 frame.

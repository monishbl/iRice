# shell/modules/decorations/WindowControls.qml → surfaces/decorations (src/lib.rs, layout.rs, ui/decorations.slint)

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | Close / minimise / maximise controls over a window's title bar (plugin or overlay mode) | 20-55 | `zones`, buttons | same |
| 2 | Hovering maximise asks for the tile grid; the grid opens at the pointer if it's on this monitor | 56-67 | `tiling`, `linger` | same |
| 3 | The grid closes if the pointer never comes over in 1.5 s, or once it leaves | 69-82 | `invoke_linger`, `tiling_off` | same |
| 4 | Size changes (controls ↔ grid) over `hover` | 75-76 | `animate qw, qh { Theme.hover }` | same |
| 5 | Material icon buttons (IconButton.qml) 24 px: corners round → 8 while pressed, 150 ms OutCubic; state layer 8 % / 10 % on `hover` | IconButton.qml 19-23 | `MButton` | same (colour duration now the `Theme.hover` token) |
| 6 | Click a grid zone → the window snaps there (`tiling.snap`) | 90-200 | `tiling.snap` | same |

## Test plan (tester)
Both looks, decorations on. A foot window.
1. Glide onto its maximise control and wait → the tile grid opens; glide away → it closes; don't come over for 1.5 s → it closes by itself.
2. Click the left-half zone → the window takes the left half.
3. Material: `P down` on close (don't release over it) → corners to 8 in 150 ms; glide off, `P up`.

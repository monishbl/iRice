# shell/modules/dock/MaterialDownloadsStack.qml → surfaces/dock/src/stack.rs, ui/stack.slint `StackMaterial`

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Overlay window, exclusive keyboard | 22-28 | `overlay_surface` | same |
| Grows up out of the dock on the default spatial spring (k 380, ζ 0.8); opacity = min(1, 1.5·p), scale 0.9 → 1 about the bottom | 30-42, 63-65 | was 520 ms spring(0.4) (ω 12.1, ζ 0.6: slower and bouncier). **Now 322 ms spring(0.2)** | fixed |
| Click outside / Escape closes | 51-55, 67 | overlay TouchArea, FocusScope | same |
| Card: surfaceContainer, radius popover, elevation 2's shadow (offset 2, blur 12, shadow 40 % / 16 %) | 69-81 | `body` drop-shadow | same |
| Title "Downloads" 16 / medium; "Open in Files" text button (primary 8 / 10 % state layer, 150 ms) | 83-117 | title Text, button Rectangle | same |
| Cells: state layer (radius 16) 8 % hover / 10 % pressed, 48 px icon from the shell's theme, 12 px name | 119-165 | the cell; icons now from core `Icons::path` (was the GTK theme's) | fixed |
| Quick Look: tonal 28 px button, corners square to 8 when pressed, swells (ElasticControl) | 166-192 | radius morph was there; **press swell added** (290 ms spring(0.57)) | fixed |
| Glyphs: Material Symbols (eye, download-simple) | 179-183, 209-214 | **were Phosphor glyphs**; now `SymIcon` with the `Symbols` global (Material Symbols, outlined: Slint can't set the FILL axis; these two aren't filled in the QML either) | fixed |
| Empty: download symbol on a cookie9 shape, "Nothing downloaded yet" | 197-222 | `if n == 0` | same |
| The list | — | `Rows<StackFile>`, refreshed live every 2 s | fixed |

## Test plan

### Setup
Material look, files as in DownloadsFiles' plan.

1. Record a click on Downloads (970,1032): grows from the bottom on Material's default spring (~320 ms, ζ 0.8, a slight overshoot; it was 520 ms), opacity reaching 1 at 2/3 of the way. Compare with QML: same length ±1 frame.
2. Title "Downloads" at top left, "Open in Files" primary text button top right (hover 8 % primary).
3. Cells: hover → rounded (16) state layer 8 %, pressed 10 %; Colloid mimetype icons (fixed: were from another theme).
4. Quick Look button (28 px tonal, top right of a hovered cell): **Material Symbols eye** (fixed: was Phosphor); press → corners to 8 and a small swell.
5. Empty folder: cookie-shaped secondaryContainer blob with the Material download symbol (fixed: was Phosphor) and "Nothing downloaded yet".
6. Escape / outside click closes.

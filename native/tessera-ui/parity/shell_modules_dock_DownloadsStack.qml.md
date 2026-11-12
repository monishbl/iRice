# shell/modules/dock/DownloadsStack.qml → surfaces/dock/src/stack.rs, ui/stack.slint `StackGlass`

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Overlay window over the screen, exclusive keyboard | 57-61 | `overlay_surface` | same |
| Springs up: progress on SpringAnimation 4.5 / 0.34 (ω 16.8/s, ζ 0.63); opacity = progress, scale 0.92 → 1 about the bottom | 63-68, 89-91 | was 520 ms spring(0.35) (ω 12.1: about 40 % slower). **Now 375 ms spring(0.37)** | fixed |
| A click outside (any button) or Escape closes | 77-81, 94 | overlay TouchArea, FocusScope | same |
| Card: 4 × 96 cells, row gap 8, + 24 + 44, centred on the icon, 8 px above the dock; glass, radius popover | 83-105 | `body`; glass shape sent from `after_frame` following its scale | same |
| Cells: hover fill, 48 px icon (unstyled) from the shell's icon theme: IconLookup.path(Search.iconForFile(path), "text-x-generic") | 110-127 | **was `icon_theme::find` (the GTK theme, else Papirus/breeze/Adwaita), not the shell's theme (the user's Colloid)**. Now core `Icons::path` (the shell's theme index, IconLookup's variants and fallbacks) | fixed |
| Name elided in the middle | 128-136 | `stack::middle` (cut in Rust: Slint elides only the end) | different (same look at a glance) |
| Quick Look eye on hover (cell or button) | 148-161 | eye Rectangle (`ft`/`eye` hover) | same |
| "Nothing downloaded yet" | 166-173 | `if n == 0` | same |
| "Open in Files" pill (fill / hover / pressed) | 176-199 | the pill Rectangle | same |
| The list | — | `Rows<StackFile>` handed once (was a new model) | fixed |

## Test plan

### Setup
Glass look, files in ~/Downloads as in DownloadsFiles' plan. The stack opens centred on the Downloads icon x (970), its card bottom 8 px above the dock pill (y ≈ 994).

1. Record a click on Downloads (970,1038): the card fades in and scales 0.92 → 1 from its bottom centre on a spring (QML 4.5 / 0.34: settles in ~375 ms with a small overshoot; it was ~520 ms). anim-timing on both shells: same length ±1 frame.
2. **Icons (fixed)**: with Colloid each file shows Colloid's mimetype icon (pdf, zip, image, text, video) — not blank, not another theme's. Compare with the QML shot cell by cell.
3. Hovering a cell: fill behind it and the eye button (24 px, top right); clicking the eye opens Quick Look for that file (the stack stays).
4. Escape closes; a click outside closes; a right-click outside closes.
5. Empty ~/Downloads: "Nothing downloaded yet" centred.

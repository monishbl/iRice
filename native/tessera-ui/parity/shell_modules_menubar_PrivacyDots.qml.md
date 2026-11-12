# Parity: shell/modules/menubar/PrivacyDots.qml

The bar's privacy indicator: Glass coloured dots (camera green, mic orange,
location blue, screen share purple); Material an Android privacy chip with
the icons. Click opens the "In use" menu.
Rust: `surfaces/menubar/src/items.rs:332-334` (entry, `Glyph::Privacy`),
`ui/menubar.slint:53` (width), 182-206 (dots / chip), click `src/lib.rs:1075`.

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Visible while `Privacy.any` | 13 | items.rs:332 (`privacy.any` from the shell state) | same |
| Opens `privacyMenu` under it (toggle) | 14 + MenuBar 657 | lib.rs:1075 | same |
| Highlight while its menu is open | MenuBar 656 | items.rs:180 (active from panel) | same |
| Width: dots/chip + 16 px | 16 | menubar.slint:53 | same |
| Glass dots 7 px, spacing 4, colours #30D158 / #FF9F0A / #0A84FF / #BF5AF2, in that order | 18-35 | menubar.slint:182-191 | same |
| Each dot pops in: scale 0→1, NumberAnimation 200 ms OutBack | 36-41 | — | missing |
| Material chip: tertiaryContainer, h 20, radius 10, icons 12 px filled onTertiaryContainer, spacing 2, +12 px | 46-74 | menubar.slint:192-206 | same |
| No hover tip (PrivacyDots has no HoverTip) | — | items.rs:333 (`tip: "In use"`) → menubar.slint:557-561 | different: Rust shows a tip "In use" after 500 ms |
| Live: dots follow Privacy.camera / mic / location / shareDot | 24-29, 59-64 | items.rs:333 (shell state privacy.*) | same |

## Summary (missing / different, by priority)

1. Dots' pop-in (scale 0→1, 200 ms OutBack) missing.
2. Rust adds a hover tip ("In use") the QML item doesn't have.

## Batch 2

The "In use" tip removed (MenuBar.qml.md test plan step 4). The dots pop-in is still missing.

## Batch 3

Dots pop in (MenuBar.qml.md batch 3 step 8).

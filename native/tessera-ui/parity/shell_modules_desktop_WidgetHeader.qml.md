# shell/modules/desktop/WidgetHeader.qml → ui/desktop.slint `Header`

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| 26 px disc with a filled 15 px icon, title 8 px after, body DemiBold | 68-89 | `Header` | same |
| Material: primary disc, error when alert | 81 | `Header` | same |
| Glass: icon dark on light tints (luminance > 0.7), white otherwise | 82 | dark on yellow, on-accent on the accent, white otherwise | same for every tint the widgets use |
| Material icons filled (FILL axis) | 82 | SymIcon `fill: true` | different: Slint can't set the Material Symbols FILL axis, outlined glyphs |

## Test plan

Geometry (1920x1080 IRICE-TEST, the user's 4 widgets): grid origin 32,54; a cell 164 px, gap 16; a cell (c,r) starts at x = 32 + 180c, y = 54 + 180r; s = 164x164, m = 344x164, l = 344x344. The user's layout: clock m at (0,0) → 32..376 x 54..218; stack m at (0,1) → 32..376 x 234..398; batteries m at (0,2) → 32..376 x 414..578; shortcuts m at (2,0) → 392..736 x 54..218.
Both looks: run once with appearance.look "glass" (the user's) and once "material" (dark).

1. Show a Reminders or Screen Time widget (add Reminders Medium from Edit Widgets). PASS: a 26 px disc with the glyph, the title beside it, same as the QML shot side by side.

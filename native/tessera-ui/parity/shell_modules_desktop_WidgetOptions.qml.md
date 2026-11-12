# shell/modules/desktop/WidgetOptions.qml → surfaces/desktop/src/options.rs; ui/options.slint

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Per kind: Photos folder, cities, symbols, stack kinds, System graph | all | `WidgetOptions` | same |
| Primary button states; Return/Enter/Space | 170-200 | options.slint | same |
| Writes config.json; the desktop follows (in place) | — | `write_cfg` → `config_changed` | same |

## Test plan

Geometry (1920x1080 IRICE-TEST, the user's 4 widgets): grid origin 32,54; a cell 164 px, gap 16; a cell (c,r) starts at x = 32 + 180c, y = 54 + 180r; s = 164x164, m = 344x164, l = 344x344. The user's layout: clock m at (0,0) → 32..376 x 54..218; stack m at (0,1) → 32..376 x 234..398; batteries m at (0,2) → 32..376 x 414..578; shortcuts m at (2,0) → 392..736 x 54..218.
Both looks: run once with appearance.look "glass" (the user's) and once "material" (dark).

1. Right-click the stack → Widget Options…; toggle a kind; Done. PASS: the stack gains/loses that page; the other widgets don't blink.

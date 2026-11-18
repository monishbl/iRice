# shell/modules/desktop/StocksWidget.qml → ui/desktop.slint `StocksW`; lib.rs `fill` "stocks"

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Rows: symbol, name, price, change chip, sparkline | all | `stocks` | same |
| Every 15 minutes, and when the symbols change | 34-38 | `slow`, `config_changed` | same |
| Rows in place | — | `Lists.stocks` | fixed |

## Test plan

Geometry (1920x1080 TESSERA-TEST, the user's 4 widgets): grid origin 32,54; a cell 164 px, gap 16; a cell (c,r) starts at x = 32 + 180c, y = 54 + 180r; s = 164x164, m = 344x164, l = 344x344. The user's layout: clock m at (0,0) → 32..376 x 54..218; stack m at (0,1) → 32..376 x 234..398; batteries m at (0,2) → 32..376 x 414..578; shortcuts m at (2,0) → 392..736 x 54..218.
Both looks: run once with appearance.look "glass" (the user's) and once "material" (dark).

1. Add Stocks Medium (needs network; else the error line, same in both shells). RELOAD step: no redraw.

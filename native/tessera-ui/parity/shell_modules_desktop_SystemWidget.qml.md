# shell/modules/desktop/SystemWidget.qml → ui/desktop.slint `SystemW`; lib.rs `fill` "system", sysstat.rs

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Small cpu+memory; medium + temperature (or swap) + down; large all | 14-21 | `sysstat::keys_for` | same |
| Sampled once a second while shown | header | `sample_timer` 1 s while a System widget is shown | same |
| Stats don't rebuild each sample | 12-13 | `Lists.stats` in place: only paths and figures change | fixed |
| GPU where the driver reports it | header | — | missing: sysstat has no GPU reading |

## Test plan

Geometry (1920x1080 IRICE-TEST, the user's 4 widgets): grid origin 32,54; a cell 164 px, gap 16; a cell (c,r) starts at x = 32 + 180c, y = 54 + 180r; s = 164x164, m = 344x164, l = 344x344. The user's layout: clock m at (0,0) → 32..376 x 54..218; stack m at (0,1) → 32..376 x 234..398; batteries m at (0,2) → 32..376 x 414..578; shortcuts m at (2,0) → 392..736 x 54..218.
Both looks: run once with appearance.look "glass" (the user's) and once "material" (dark).

1. Add System Medium (Edit Widgets → System → Medium) or put it in the stack. Record 5 s. PASS: graphs scroll left each second; labels never blink; the widget never fades/grows in again.

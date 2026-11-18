# shell/modules/desktop/RemindersWidget.qml → ui/desktop.slint `RemindersW`; lib.rs `fill` "reminders"

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Count, open to-dos, due label, overdue red, "N more" | all | `lines`, t1, t4 | same |
| Tick (ring / Material checkbox with hover layer) ticks the to-do | 70-88 | `Tick`, `act("tick")` | same |
| A ticked row leaves; the others keep their elements | — | `Lists.lines` keyed by to-do key | fixed |

## Test plan

Geometry (1920x1080 TESSERA-TEST, the user's 4 widgets): grid origin 32,54; a cell 164 px, gap 16; a cell (c,r) starts at x = 32 + 180c, y = 54 + 180r; s = 164x164, m = 344x164, l = 344x344. The user's layout: clock m at (0,0) → 32..376 x 54..218; stack m at (0,1) → 32..376 x 234..398; batteries m at (0,2) → 32..376 x 414..578; shortcuts m at (2,0) → 392..736 x 54..218.
Both looks: run once with appearance.look "glass" (the user's) and once "material" (dark).

1. Add two to-dos (`ipc` or Today), add Reminders Medium. Hover a tick (Material: round state layer). Click the first tick: that row leaves, the second moves up without the widget redrawing whole.

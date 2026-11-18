# shell/modules/desktop/CalendarWidget.qml → ui/desktop.slint `CalendarW`; lib.rs `fill` "calendar", `act` "day"

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Small: click opens Notification Center's calendar | 56-67 | TouchArea → `calendar` | same |
| Day header, date on a primary cookie9 (Material) | 69-110 | t1, t2, cookie9 | same |
| Events of the picked day | 112-190 | `lines` | same |
| Medium month: today accent, picked secondary container, hover layer, busy dots; click picks, second click back to today | 195-247 | `days`, `act("day")` | same |
| Large: Notification Center's calendar (Month/Week/To-do) | 40-52 | medium layout at large size | missing: the Rust NC calendar (surfaces/notifications CalendarTab/TodoTab and its host `calendar` asks) is another crate's; the faithful route is to move those components into core/ui as a shared library component and reuse it here (needs the notifications and core owners) |
| Days/events in place | — | `Lists.days`, `Lists.lines` | fixed |

## Test plan

Geometry (1920x1080 TESSERA-TEST, the user's 4 widgets): grid origin 32,54; a cell 164 px, gap 16; a cell (c,r) starts at x = 32 + 180c, y = 54 + 180r; s = 164x164, m = 344x164, l = 344x344. The user's layout: clock m at (0,0) → 32..376 x 54..218; stack m at (0,1) → 32..376 x 234..398; batteries m at (0,2) → 32..376 x 414..578; shortcuts m at (2,0) → 392..736 x 54..218.
Both looks: run once with appearance.look "glass" (the user's) and once "material" (dark).

1. Scroll the stack to Calendar (Medium). Glide over the month's days: each hovered day gets a faint round layer. Click a day: it's picked (secondary container), the left side shows its events; click it again: back to today.

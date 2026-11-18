# shell/modules/desktop/DesktopFeed.qml → host/desktop/src/wm/feed.rs (standalone); the QML file under the QML shell

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Weather, events, busy days, upcoming, to-dos, player, power, actions, notes, stacks, buttons, greeting, Edit Widgets | all | `Feed::build` | same |
| Batteries incl. Bluetooth devices | 32-55 | **only this computer** → + bluez devices | fixed |
| Screen Time | — | **sent empty** → screentime.rs counts and the feed sends it | fixed (batch 2) |
| Sent again when it changes | — | every 2 s if the JSON differs | same |
| The desktop's asks | — | `Feed::call` | same |

## Test plan

Geometry (1920x1080 TESSERA-TEST, the user's 4 widgets): grid origin 32,54; a cell 164 px, gap 16; a cell (c,r) starts at x = 32 + 180c, y = 54 + 180r; s = 164x164, m = 344x164, l = 344x344. The user's layout: clock m at (0,0) → 32..376 x 54..218; stack m at (0,1) → 32..376 x 234..398; batteries m at (0,2) → 32..376 x 414..578; shortcuts m at (2,0) → 392..736 x 54..218.
Both looks: run once with appearance.look "glass" (the user's) and once "material" (dark).

1. Covered by BatteriesWidget and DesktopWidgets plans (connect a Bluetooth device in the arena if it can; otherwise check the feed line in the log has the device list empty, not missing).

# shell/modules/desktop/BatteriesWidget.qml → ui/desktop.slint `BatteriesW`, `Ring`; lib.rs `fill` "batteries"; host/desktop wm/feed.rs

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| This computer (lightning while charging) | 15-16 | feed `batteries` | same |
| Connected Bluetooth devices with a level, Devices' glyph and drawing | 17-28 | **standalone listed only this computer** → host feed adds connected bluez devices with a level (`bt_icon`, `picture_for`); the host now wants the `bluez` service | fixed |
| Earbuds' left/right/case apart (Fast Pair, heard in the last 2 minutes) | 21-24 | **one ring** → host/sharing publishes its levels as state `fastLevels.<address>`; the feed splits them into Left / Right / Case rings | fixed (batch 2) |
| Green / primary; red / error at ≤ 20 % not charging | 31-36 | tint | same |
| Layouts: small one / small many / medium with names | 38-126 | `BatteriesW` | same |
| Rings kept across refreshes | — | `Lists.gauges` | fixed |

## Test plan

Geometry (1920x1080 IRICE-TEST, the user's 4 widgets): grid origin 32,54; a cell 164 px, gap 16; a cell (c,r) starts at x = 32 + 180c, y = 54 + 180r; s = 164x164, m = 344x164, l = 344x344. The user's layout: clock m at (0,0) → 32..376 x 54..218; stack m at (0,1) → 32..376 x 234..398; batteries m at (0,2) → 32..376 x 414..578; shortcuts m at (2,0) → 392..736 x 54..218.
Both looks: run once with appearance.look "glass" (the user's) and once "material" (dark).

1. Fast Pair (only if real earbuds with an open case can reach the arena's Bluetooth; otherwise skip and say so): their Left / Right / Case show as three rings named so; close the case and wait 2 minutes: back to one ring.
2. The user's batteries widget (32..376 x 414..578). PASS: same rings as the QML pass (this computer if the arena has a battery; a connected Bluetooth device with its drawing if any). RELOAD step: rings and pictures never blink.

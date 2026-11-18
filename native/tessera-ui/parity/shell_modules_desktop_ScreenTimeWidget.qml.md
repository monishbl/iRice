# shell/modules/desktop/ScreenTimeWidget.qml → ui/desktop.slint `ScreenTimeW`; lib.rs `fill` "screenTime"

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Total, the week's bars, top apps with icons | all | `bars`, `apps` | same |
| Data in standalone: seconds per front app per day while not idle (2 min, inhibitors count as use), on app change and each minute, a week kept in state.json `screenTime` | ScreenTime.qml | **was sent empty** → host/desktop wm/screentime.rs (idle watch idle::SCREEN_TIME 120 s, the front app's entry, flush ≤ 600 s, a week), the feed's total / week / top 3 with names and icons | fixed (batch 2) |
| Rows in place | — | `Lists.bars`, `Lists.apps` | fixed |

## Test plan

Geometry (1920x1080 TESSERA-TEST, the user's 4 widgets): grid origin 32,54; a cell 164 px, gap 16; a cell (c,r) starts at x = 32 + 180c, y = 54 + 180r; s = 164x164, m = 344x164, l = 344x344. The user's layout: clock m at (0,0) → 32..376 x 54..218; stack m at (0,1) → 32..376 x 234..398; batteries m at (0,2) → 32..376 x 414..578; shortcuts m at (2,0) → 392..736 x 54..218.
Both looks: run once with appearance.look "glass" (the user's) and once "material" (dark).

1. Add Screen Time Medium (Edit Widgets → Screen Time → Medium). Open foot, keep it focused 2+ minutes, moving the pointer over it every 30 s (`pointer glide`) so you're not idle; then focus another app 1+ minute. Wait for the minute flush. PASS: the total shows ≥ 2m, the week's last bar (today) is filled, foot is in the app rows with its Colloid icon, bar lengths in proportion. `state.json` has `screenTime.<today>.<foot's entry id>`. Leave it idle 3 minutes: the total stops growing.

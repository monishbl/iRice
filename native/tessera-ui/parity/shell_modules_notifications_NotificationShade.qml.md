# shell/modules/notifications/NotificationShade.qml

Material shade. Rust: surfaces/notifications/src/panel.rs, surfaces/notifications/ui/notification-center.slint (seg/swipe).

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| time/date, empty state | 24-80 | nc | same |
| segmented blocks, radius 150 ms | 114-179 | seg | same |
| swipe past 1/3 dismisses | 203 | swipe | same |
| fling over 900 px/s dismisses | 203 | seg.vel | fixed |
| spring back on the default spring | 212-220 | animate dx | fixed (was an ease-in curve) |
| fly off 150 ms InCubic, then dismiss | 221-228 | fly Timer 150 ms | fixed timing |
| a reused row isn't left off-screen | n/a | changed rid | fixed |
| Manage / Clear all keyboard | 317-386 | — | missing |

## Fixed in batch 2

- fling
- spring back
- fly timing
- off reset

## Still different / missing

- keyboard on Manage/Clear all

## Tester plan

Arena as above; open Notification Center with `ipc panels open notificationCenter IRICE-TEST`.
Notifications: `dev run notify-send -a App -i firefox 'Title' 'Body'` (actions:
`notify-send -A yes=Yes -A no=No …`; inline reply: a test app or `gdbus` call with the
`inline-reply` action). Both looks.

1. Material NC with 3 notifications. Drag a card 1/4 right and release: springs back (record: overshoot-free settle ~300 ms).
2. Flick a card fast (glide 120 px in 80 ms, release): it dismisses (fling).
3. Drag past 1/3: flies off in 150 ms and is gone.

## Test results

(tester)

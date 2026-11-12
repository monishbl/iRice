# shell/modules/notifications/NotificationPopups.qml

Banners. Rust: surfaces/notifications/src/lib.rs.

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| stack maths, pills, timers | 42-226 | surfaces/notifications/src/lib.rs | same |
| collapse when hidden | 43 | surfaces/notifications/src/lib.rs refresh | fixed: an expanded stack folds when NC/CC covers it (its timers ran never again) |
| hidden only when NC is on that screen | 80 | surfaces/notifications/src/lib.rs nc_here | fixed |
| pills set only when they change | 70 | surfaces/notifications/src/lib.rs pills_sent | fixed (hover on pills was lost every refresh) |
| cards updated only when changed | 113 | surfaces/notifications/src/lib.rs stable buttons | fixed |
| Glass shadow | 228-239 | — | missing (documented) |

## Fixed in batch 2

- collapse when hidden
- per screen
- pills
- buttons

## Still different / missing

- Glass shadow

## Tester plan

Arena as above; open Notification Center with `ipc panels open notificationCenter IRICE-TEST`.
Notifications: `dev run notify-send -a App -i firefox 'Title' 'Body'` (actions:
`notify-send -A yes=Yes -A no=No …`; inline reply: a test app or `gdbus` call with the
`inline-reply` action). Both looks.

1. Send 3 banners, click "2 more" (expand), open NC, close NC: the banners expire on their timers.
2. Two outputs: NC open on one, a banner arriving shows on the other.
3. Hover the "Clear all" pill for 30 s: hover stays.

## Test results

(tester)

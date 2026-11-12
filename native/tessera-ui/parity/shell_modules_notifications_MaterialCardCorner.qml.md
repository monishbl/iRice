# shell/modules/notifications/MaterialCardCorner.qml

Card corner. Rust: surfaces/notifications/ui/notifications.slint.

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| close on hover/focus, expander | 18-33 | corner | focus case missing |

## Still different / missing

- focus

## Tester plan

Arena as above; open Notification Center with `ipc panels open notificationCenter IRICE-TEST`.
Notifications: `dev run notify-send -a App -i firefox 'Title' 'Body'` (actions:
`notify-send -A yes=Yes -A no=No …`; inline reply: a test app or `gdbus` call with the
`inline-reply` action). Both looks.

1. See MaterialNotificationCard.

## Test results

(tester)

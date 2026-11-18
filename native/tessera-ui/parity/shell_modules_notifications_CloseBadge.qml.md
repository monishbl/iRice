# shell/modules/notifications/CloseBadge.qml

Close badge. Rust: surfaces/notifications/ui/notifications.slint CloseBadge.

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| 22 px badge, hit reach | 9-32 | CloseBadge | same (reach 4 fixed) |

## Tester plan

Arena as above; open Notification Center with `ipc panels open notificationCenter TESSERA-TEST`.
Notifications: `dev run notify-send -a App -i firefox 'Title' 'Body'` (actions:
`notify-send -A yes=Yes -A no=No …`; inline reply: a test app or `gdbus` call with the
`inline-reply` action). Both looks.

1. Hover a Glass card: badge at its corner; click closes.

## Test results

(tester)

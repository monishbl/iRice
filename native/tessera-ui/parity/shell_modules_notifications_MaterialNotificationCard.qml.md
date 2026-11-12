# shell/modules/notifications/MaterialNotificationCard.qml

Material card. Rust: surfaces/notifications/ui/notifications.slint MaterialCard.

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| tap expands / default | 59-71 | MaterialCard | same |
| close on hover or focus | 106-115 | corner | different: hover only |
| body line height 20 | 123-159 | MaterialCard | different (16.8) |
| wavy progress | 161-169 | flat | different |
| pills, composer | 198-324 | MaterialCard | same |

## Still different / missing

- focus shows close
- line height
- wavy progress

## Tester plan

Arena as above; open Notification Center with `ipc panels open notificationCenter IRICE-TEST`.
Notifications: `dev run notify-send -a App -i firefox 'Title' 'Body'` (actions:
`notify-send -A yes=Yes -A no=No …`; inline reply: a test app or `gdbus` call with the
`inline-reply` action). Both looks.

1. Material banner: hover shows close; click expands when it has more.

## Test results

(tester)

# shell/modules/notifications/MaterialAlertCard.qml

Material alert. Rust: surfaces/notifications/ui/notifications.slint.

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| badge, expand, act | 30-148 | MaterialCard | same; keyboard missing |

## Still different / missing

- keyboard

## Tester plan

Arena as above; open Notification Center with `ipc panels open notificationCenter TESSERA-TEST`.
Notifications: `dev run notify-send -a App -i firefox 'Title' 'Body'` (actions:
`notify-send -A yes=Yes -A no=No …`; inline reply: a test app or `gdbus` call with the
`inline-reply` action). Both looks.

1. Material NC alerts show and act.

## Test results

(tester)

# shell/modules/notifications/MaterialCheck.qml

To-do check. Rust: surfaces/notifications/ui/notification-center.slint Tick.

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| square, fill | 40-46 | Tick | same |
| state layer, animations, keyboard | 17-63 | — | missing |

## Still different / missing

- state layer, tick spring, keyboard

## Tester plan

Arena as above; open Notification Center with `ipc panels open notificationCenter TESSERA-TEST`.
Notifications: `dev run notify-send -a App -i firefox 'Title' 'Body'` (actions:
`notify-send -A yes=Yes -A no=No …`; inline reply: a test app or `gdbus` call with the
`inline-reply` action). Both looks.

1. To-do tab (Material): click a check: fills.

## Test results

(tester)

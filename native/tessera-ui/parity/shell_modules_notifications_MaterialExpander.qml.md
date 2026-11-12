# shell/modules/notifications/MaterialExpander.qml

Count + chevron. Rust: surfaces/notifications/ui/notifications.slint Expander.

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| pill, chevron rotation, hover | 17-45 | Expander | same |
| keyboard | 19-24 | — | missing |

## Still different / missing

- keyboard

## Tester plan

Arena as above; open Notification Center with `ipc panels open notificationCenter IRICE-TEST`.
Notifications: `dev run notify-send -a App -i firefox 'Title' 'Body'` (actions:
`notify-send -A yes=Yes -A no=No …`; inline reply: a test app or `gdbus` call with the
`inline-reply` action). Both looks.

1. Material NC group: click the expander: chevron turns, group opens.

## Test results

(tester)

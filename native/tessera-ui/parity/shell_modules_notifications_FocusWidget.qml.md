# shell/modules/notifications/FocusWidget.qml

Focus / stopwatch tab. Rust: surfaces/notifications/ui/notification-center.slint FocusTab, surfaces/notifications/src/panel.rs.

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| switch, ring, buttons, stopwatch, laps | 22-159 | FocusTab | same; laps in place (fixed) |
| each opening: Focus mode | new instance | surfaces/notifications/src/panel.rs open | fixed |
| tabular digits | 75-89 | — | different |

## Fixed in batch 2

- laps in place
- reset sub-mode

## Still different / missing

- tabular digits

## Tester plan

Arena as above; open Notification Center with `ipc panels open notificationCenter TESSERA-TEST`.
Notifications: `dev run notify-send -a App -i firefox 'Title' 'Body'` (actions:
`notify-send -A yes=Yes -A no=No …`; inline reply: a test app or `gdbus` call with the
`inline-reply` action). Both looks.

1. Focus tab: start, pause, reset; Stopwatch: laps add rows without flicker.

## Test results

(tester)

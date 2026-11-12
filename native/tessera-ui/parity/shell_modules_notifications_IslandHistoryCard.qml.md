# shell/modules/notifications/IslandHistoryCard.qml

Island alert in NC. Rust: surfaces/notifications/src/panel.rs nc_banner.

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| badge/picture, act, close | 24-130 | GlassCard | same |
| "Nh ago" up to 6 h | 59-77 | time_label | different |
| 2-line detail | 79-96 | 4-line | different |

## Still different / missing

- ago format
- line cap

## Tester plan

Arena as above; open Notification Center with `ipc panels open notificationCenter IRICE-TEST`.
Notifications: `dev run notify-send -a App -i firefox 'Title' 'Body'` (actions:
`notify-send -A yes=Yes -A no=No …`; inline reply: a test app or `gdbus` call with the
`inline-reply` action). Both looks.

1. Island alerts in NC show and close.

## Test results

(tester)

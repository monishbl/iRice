# shell/modules/notifications/NotificationReply.qml

Inline reply state. Rust: surfaces/notifications/src/lib.rs (banners), surfaces/notifications/src/panel.rs (NC).

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| open/cancel/send | 20-40 | both | same |
| "Sent" clears after 2.5 s | 41-46 | surfaces/notifications/src/panel.rs → M::SentDone | fixed in NC (it stayed forever and Reply never came back) |
| keys back to the panel after reply | n/a | surfaces/notifications/src/panel.rs take_keys | fixed: Escape closes NC again after sending/cancelling |
| island Reply opens the composer | 47-57 | — | missing (needs a shell message) |

## Fixed in batch 2

- Sent clears
- keys back

## Still different / missing

- island replyTo

## Tester plan

Arena as above; open Notification Center with `ipc panels open notificationCenter IRICE-TEST`.
Notifications: `dev run notify-send -a App -i firefox 'Title' 'Body'` (actions:
`notify-send -A yes=Yes -A no=No …`; inline reply: a test app or `gdbus` call with the
`inline-reply` action). Both looks.

1. Notification with inline reply in NC: Reply, type, Enter: "Sent" shows, and after 2.5 s the Reply button is back. Press Escape: NC closes.

## Test results

(tester)

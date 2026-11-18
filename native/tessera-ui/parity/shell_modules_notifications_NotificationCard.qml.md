# shell/modules/notifications/NotificationCard.qml

Glass card. Rust: surfaces/notifications/ui/notifications.slint GlassCard, surfaces/notifications/src/lib.rs card().

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| hover close badge, click default+dismiss | 38-48, 235-239 | GlassCard | same |
| reply button, actions | 112-180 | GlassCard | same; buttons keep their model (fixed) |
| composer Enter/Esc, draft kept | 184-222 | GlassCard | same |
| keyboard on buttons | 118-164 | — | missing |

## Fixed in batch 2

- stable buttons

## Still different / missing

- button keyboard

## Tester plan

Arena as above; open Notification Center with `ipc panels open notificationCenter TESSERA-TEST`.
Notifications: `dev run notify-send -a App -i firefox 'Title' 'Body'` (actions:
`notify-send -A yes=Yes -A no=No …`; inline reply: a test app or `gdbus` call with the
`inline-reply` action). Both looks.

1. Banner with actions: hover a button, wait 30 s (clock refresh): hover remains.

## Test results

(tester)

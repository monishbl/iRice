# shell/modules/notifications/NotificationCenter.qml

Glass Notification Center. Rust: surfaces/notifications/src/panel.rs, surfaces/notifications/ui/notification-center.slint.

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| clear all, empty state, flickable | 69-128 | nc | same |
| groups by app; island alerts a group | 138 | center.rs | same |
| collapsed stack edges, click expands | 219-275 | nc | same |
| group toggle/clear by group (not by name) | 151-203 | surfaces/notifications/src/panel.rs gkey | fixed: an app named like another group no longer toggles both |
| live updates keep cards | 138-140 | surfaces/notifications/src/panel.rs Rows keyed | fixed: cards are matched by note/alert; buttons keep their model (hover/press/composer survive ticks and arrivals) |
| widgets card | 287-327 | nc | same; lists now in place |

## Fixed in batch 2

- rows keyed
- group key
- stable button models

## Still different / missing

- kinetic scroll

## Tester plan

Arena as above; open Notification Center with `ipc panels open notificationCenter IRICE-TEST`.
Notifications: `dev run notify-send -a App -i firefox 'Title' 'Body'` (actions:
`notify-send -A yes=Yes -A no=No …`; inline reply: a test app or `gdbus` call with the
`inline-reply` action). Both looks.

1. Send 3 notifications from one app and 1 from another; open NC. Expand a group, hover a card (close badge shows), send another notification: the hovered card keeps its badge (no flicker).
2. Start the Focus timer (widget) and hover a card's action button for 5 s: its hover stays (before it reset every second).

## Test results

(tester)

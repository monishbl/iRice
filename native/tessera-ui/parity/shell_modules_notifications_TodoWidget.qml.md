# shell/modules/notifications/TodoWidget.qml

To-do tab. Rust: core/ui/calendar.slint `TodoTab` (moved there unchanged so the desktop's Calendar widget can share it; Notification Center imports it from `@tessera/calendar.slint`), surfaces/notifications/src/panel.rs.

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| add field, Enter adds, blanks ignored | 36-68 | surfaces/notifications/src/panel.rs TodoAdd trims | fixed (blanks sent) |
| tick, strike, due | 104-183 | TodoTab | same |
| rename survives updates | 145-174 | surfaces/notifications/src/panel.rs Rows keyed | fixed: the field is no longer remade every second while a timer runs |
| × on hover, undo 8 s | 185-229 | TodoTab | same; Undo uses the readable accent (fixed) |

| shared component (desktop widget's To-do tab can use it) | CalendarWidget 698-708 | core/ui/calendar.slint TodoTab | fixed (moved, same behaviour) |

## Fixed in batch 2

- rename survives
- blank add
- undo colour

## Tester plan

Arena as above; open Notification Center with `ipc panels open notificationCenter IRICE-TEST`.
Notifications: `dev run notify-send -a App -i firefox 'Title' 'Body'` (actions:
`notify-send -A yes=Yes -A no=No …`; inline reply: a test app or `gdbus` call with the
`inline-reply` action). Both looks.

1. Start the Focus timer, then in To-do double-click a to-do and type slowly for 10 s: text is kept (before it reset each second). Enter keeps it.
2. Type spaces only + Enter: nothing added.
3. (The move) everything as before: type "milk" + Enter in the add field: it
   appears; hover it: the × shows at its right; click ×: "Deleted “milk” Undo"
   for 8 s; Undo brings it back; the tick strikes it through. Both looks.

## Test results

(tester)

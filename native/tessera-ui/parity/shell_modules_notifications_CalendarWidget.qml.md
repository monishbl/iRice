# shell/modules/notifications/CalendarWidget.qml

Calendar tab. Rust: core/ui/calendar.slint `CalendarTab` (shared: Notification
Center imports it through `@tessera/calendar.slint`; the desktop's large
Calendar widget can too), surfaces/notifications/src/panel.rs (checks a save,
"this or all?", sends the acts), host/desktop/src/today/mod.rs `nc_act`
(cal.save / cal.remove / cal.restore / cal.move), today/calendar.rs
`nc_save` / `nc_remove` / `nc_restore` / `nc_move`, today/notif.rs
`calendar_payload` (each item's fields for the editor, the calendars a new
event can go to, the default alert, the Undo title).

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| header, month/week, arrows | 186-231 | CalendarTab | same |
| wheel adds up, one step per notch, months and weeks | 273-283 | wheel() | fixed (batch 2) |
| grid, dots, select | 285-731 | CalendarTab | same |
| lists in place | n/a | panel.rs Rows (days, items, targets) | fixed |
| done to-do struck through, pointer cursor | 557, 571 | items | fixed (batch 2) |
| each opening: month view | new instance | panel.rs open | fixed (batch 2) |
| "+" beside the day title: editor for that day ("Add to this day") | 453-462 | CalendarTab day row IconButton → create(day-key, day-short) | fixed |
| empty text "Nothing on this day. Double-click a date to add something.", hidden while Undo shows | 464-472 | CalendarTab | fixed |
| double-click a date (month grid and week rows) → new event editor on it, that date selected | 425, 724 | day cell / week row `double-clicked` → create() (pick + editor) | fixed |
| click an item → edit it; read-only calendars: "Event" with the info line and Close only | 596, 793-814 | item TouchArea clicked → edit(it); NcItem.editable / info | fixed |
| editor title: "New on ddd d MMM" / "Edit Event" / "Edit To-do" / "Event" | 922-927 | form header | fixed |
| Event / To-do switch (new only); switching sets the start ("" / 9:00 AM or 09:00) | 928-937 | Segmented in form | fixed |
| target calendar (new event, more than one) | 938-944 | CalChoice + list (menu 1); the host offers "This computer" only (see below) | fixed (list shows only with >1) |
| title field (placeholder "What to do"/"Title"), Enter saves, focused on new | 945-952, 831 | CalField title-f, committed → save-now(); create() focuses it | fixed |
| All day switch | 954-969 | CalSwitch | fixed |
| starts – ends (event) / optional time (to-do); Enter saves | 970-1003 | CalField ×2 / ×1 | fixed |
| location (account calendars only), notes (events), Enter saves | 1004-1020 | CalField (ed-google), CalField notes | fixed |
| alert dropdown: None … 1 day before (events, not all-day, not accounts) | 1021-1037 | CalChoice + list (menu 2), Up/Down step, list keys Up/Down/Home/End/Enter/Esc | fixed |
| "this or all" for repeating events (save, delete, drag) | 761-766, 896-899, 1057-1093 | ask(what, day) → three buttons → save/remove/move with every 0/1 | fixed (host has no editable repeating events: see below) |
| Delete (left, editing, not read-only) / Cancel or Close / Save ("Save Changes", "Add To-do", "Add Event") | 1094-1146 | button row | fixed |
| errors: "Give it a title.", "Use a time like 9:30 or 5pm, or leave it empty.", "Use times like 9:30 or 5pm.", a failed write's message | 862-905, 1050-1056 | panel.rs nc_cal_save (center::minutes = editor.minutes); host error → calendarError → fail() | fixed |
| Escape closes the editor (closes an open list first) | 919 | ed-keys FocusScope | fixed |
| click on the scrim (outside the card) closes; clicks in the card stay | 908, 918 | editor TouchArea / card TouchArea | fixed |
| 150 ms opacity fade in and out | 750-753 | editor `animate opacity { duration: 150ms; }` | fixed |
| after save: the saved day selected | 910 | panel.rs nc_cal_save sets selected (and the month shown) | fixed |
| drag an item (movable: your events, to-dos) to another date: a ghost (180 × 30/36, accent border) follows the pointer, the row fades to 50 %, the date under it lights (accent 35 %/30 %; week row 25 %), drop → moveTo keeping the time, that date selected | 606-648, 125-154, 336, 692-698 | item TouchArea moved/pointer-event (10 px threshold), drop-key from each cell's `target`, ghost in CalendarTab; host calendar::nc_move | fixed |
| cursor: open hand on movable items, pointing hand otherwise | 600 | `grab` / `pointer` (`grabbing` while dragging) | fixed |
| delete with an 8 s "Deleted “…” Undo" row | 156-169, 678-713 | CalendarTab undo row; host notif.cal_undo (8 s, undo_expired), cal.restore | fixed |
| week row styling | 347-412 | week view | different |

## What still differs

- The editor's card is centred over the tab as in the QML; when it is taller
  than the tab, the tab grows to hold it (the QML lets it overflow the widget).
- The calendars a new event can go to: "This computer" only (so the list is
  hidden). The accounts' calendars (Google, iCloud, Microsoft 365) live in the
  system host and their events aren't in the Rust calendar at all; no
  account event can be opened, so no location field or "this or all?" shows
  in practice (the code paths are there, driven by NcItem.account /
  recurring).
- Subscribed (file) calendars' events open read-only with
  "<time>\nFrom <name>, a subscribed calendar (read-only)."; the QML's
  "repeats in a way Tessera can't show yet" line isn't there (the reader
  doesn't flag it).
- Tab doesn't move focus between the editor's fields (Slint focus chain).
- The QML-bridge path (shell/services/Notifications.qml ncAct) doesn't know
  cal.* acts; only the standalone host does.
- Week row styling (unchanged from batch 2).

## Tester plan

Arena as usual, Rust only for the behaviour, QML for timing references. Open
Notification Center with `ipc panels open notificationCenter IRICE-TEST`,
Calendar tab. Below, (tx, ty) is the tab's top-left: the widgets card's top
edge + the tabs row (Glass 12 + 28, Material 12 + 36 + 4) and the card's left
edge; the month grid starts at tx + 12, ty + 12 + 26 (Material 32) + 8 + 16
(the weekday letters), cells are (cardwidth − 24)/7 wide and 32 tall. Do each
in Glass and in Material.

1. "+": glide to the + at the right end of the day title row, click. Within
   one frame the scrim starts fading in; it's fully in at ~150 ms (record and
   time with tools/anim-timing.py). Card title "New on <ddd d MMM>" of the
   selected day; Event/To-do switch on Event; title field focused (type
   "abc": it appears); start "9:00 AM" (or "09:00" with clock24), end
   "10:00 AM"; All day off; Notes; Alert "10 min before". Buttons: Cancel,
   Add Event (no Delete).
2. Escape: the editor fades out in ~150 ms; Notification Center stays open.
   Escape again: Notification Center closes.
3. Double-click a date in the grid (two clicks < 400 ms at its centre): that
   date is selected and the editor opens "New on …" for it. Same in Week view
   on a row.
4. New event: type "Dentist", click in Starts, select all, type "2pm", Ends
   "3pm", press Enter (in the field): the editor closes, the date shows a dot,
   the list under the grid shows "Dentist" with "2:00 PM – 3:00 PM ·
   Reminders" within ~1 s.
5. Errors: open "+", leave the title empty, click Add Event: red "Give it a
   title." Type "x", Starts "soon", Enter: "Use times like 9:30 or 5pm."
   Switch to To-do, time "soon", Enter: "Use a time like 9:30 or 5pm, or leave
   it empty." Clear the time, Enter: a to-do "x" for that day (orange tick,
   "To-do"), an orange square under the date.
6. Click "Dentist" in the list: "Edit Event", fields filled (title, 2:00 PM,
   3:00 PM, alert), no Event/To-do switch; buttons Delete Event, Cancel, Save
   Changes. Change the title to "Dentist 2", click Save Changes: the list says
   "Dentist 2".
7. Alert: in that editor click the Alert button: a list opens under it (right
   edges lined up, 200 px wide), "10 min before" highlighted/chosen. Down,
   Down, Enter: "1 hour before". Click the button, click outside the list
   (inside the card): the list closes, the editor stays.
8. All day: click the switch (knob slides 220 ms Glass / spring Material):
   the times row and the Alert row disappear.
9. Delete: open "Dentist 2", click Delete Event: the editor closes, the item
   is gone, a row "Deleted “Dentist 2” Undo" shows. Click Undo within 8 s: it
   comes back (within ~1 s). Delete again and wait: the Undo row goes after
   8 s. Delete a to-do the same way from its editor ("Delete To-do").
10. Drag: press on the "x" to-do's text (not its tick), move 30 px down then
    glide over another date in the grid (pointer down, glide, up). From 10 px
    of movement a ghost card with "x" follows the pointer (its top-left 20 px
    left and 18 px above the pointer), the row underneath fades to half, the
    date under the pointer gets an accent circle. Release: the to-do moves
    there (same time), that date becomes selected. Same with an event (keeps
    its time). In Week view the row under the pointer turns accent-tinted.
    Release over the header or outside the grid: nothing moves.
11. Cursor: over an event of your own: open hand (grab); over a subscribed
    calendar's event: pointing hand; while dragging: closed hand.
12. Read-only: with a file subscription in calendars.json (a local .ics with
    one event), click its event: title "Event", fields disabled, a line
    "<time>\nFrom <name>, a subscribed calendar (read-only).", only "Close".
    It can't be dragged.
13. Click on the scrim outside the card: closes. Click inside the card on
    empty space: stays.
14. Earlier batch: one wheel notch moves one month; week view steps weeks;
    reopening shows the month view.

## Test results

(tester)

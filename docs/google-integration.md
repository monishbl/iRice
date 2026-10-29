# Google Calendar and Tasks

In **Settings → Google**. Connect one Google account or several: their
calendars and task lists show together, in one calendar, each event in its
calendar's colour. The account shows in an event's details and in Settings,
not as separate groups.

## Connecting

**Connect Google account** is the whole flow:

- A Google account already in GNOME Online Accounts (GOA) and not connected
  yet is used as it is, with no browser. With several, Settings asks which.
- **Add another account** works the same way, with an account already
  connected. Each account has its own **Sync now** and **Disconnect**.
- Otherwise the native helper `goa-signin` (`shell/native/goa-signin.c`,
  built with iRice's native helpers) asks GOA's own Google provider to add
  one. Your browser opens at Google's sign-in; when you approve, Google
  redirects to GOA's handler, which hands the result back to the helper over
  D-Bus. The account is then GOA's, like one added in GNOME Settings, and
  GNOME Settings is never opened. GOA shows a small "Sign in" window; the
  helper presses its button for you.
- Calendars switched off for the account in Online Accounts are switched on,
  since connecting it to iRice asks for them. GOA's Google sign-in already
  includes Tasks.

GOA renews the sign-in; iRice asks it for short-lived tokens over the session
bus and never sees a password or refresh token. **Disconnect account** only
unlinks iRice from that account; it stays usable by other apps. Needs
`gnome-online-accounts` and `python-gobject`. Without the native helpers,
Settings offers Online Accounts' own window instead.

**Advanced → Use your own OAuth client**: a Google Cloud project with the
Calendar and Tasks APIs and an OAuth client of type *Desktop app*, whose JSON
you import. Sign-in then happens in your browser (a random loopback port,
state check and PKCE). Apps left in Google's *Testing* state may have their
refresh token expire after seven days.

## Syncing

Incremental, so a sync where nothing changed is small:

- **Calendars:** the calendar list and each calendar's events use Google's
  sync tokens, so only changes come back. A token Google no longer accepts
  (HTTP 410) refetches that calendar in full, a year either side of now.
- **Tasks:** Google Tasks has no sync tokens, so each list asks for tasks
  updated since the last sync (a minute early, for clock skew), deletions
  included.

With 4 calendars and 3 task lists, a sync where nothing changed is 9 small
requests, about 3 ms of work in the helper plus about 100 ms to start Python.
The responses are a few hundred bytes each; this is estimated, not measured
against Google.

**When:**

- at login, and on unlock or wake (sleep locks first);
- when Notification Center opens and the last sync is over 2 minutes old;
- every 15 minutes while you use the computer;
- **Sync now**.

Never while it's locked or idle (5 minutes without input). After a failure,
automatic syncs wait 1, 4, 16 and 64 minutes, then 2 hours; **Sync now**
always goes. Each sync is one short-lived helper process, with nothing
running in between.

**What shows:**

- **Calendar:** events from the calendars switched on in Settings, in
  Calendar, Today and Spotlight.
  - New events go to this computer or to any Google calendar you can edit
    (chosen in the New event form, remembered for next time).
  - Open an event to change its title, time, all-day, location or notes, or
    drag it to another day. Deleting can be undone for 8 seconds; Google only
    hears of it afterwards.
  - Repeating events ask **This event** (that occurrence only) or **All
    events** (the series; a new time moves every occurrence by as much as
    this one moved).
  - Calendars you can only view (shared to you read-only, or free/busy) show
    the event's details with no editing.
  - **Add event** in Settings uses Google's quick-add on an account's primary
    calendar.
- **Tasks:** every task list, next to local to-dos.
  - Tick, rename, re-date or delete them as usual. Changes show at once, go
    to Google in the background, and only that list is fetched again.
  - Deleting can be undone for 8 seconds.
  - Google stores due dates but not times.
- **New tasks go to:** this computer only (the default), or a Google list.

### Changes made offline, and conflicts

Every change to an event or task is written to an outbox on disk before it's
sent, and shows as made right away, including after a restart. If Google
can't be reached, it waits: iRice tries again after 30 seconds, then 2, 8 and
30 minutes, and before every sync. Changes go out in the order you made them.

Each edit carries the version (ETag) of the item it was made against, sent as
`If-Match`. If the item changed on Google meanwhile (on your phone, say), or
was deleted there, Google's version is kept and a notification says so once:
"Your change to *title* couldn't be saved because it changed on Google.
Here's what you had: …", with **Copy**. Deleting something already deleted on
Google simply counts as done. Two quick edits of your own aren't a conflict:
the second goes against the version the first one made.

Google Keep only opens in the browser: Google doesn't offer personal Keep
notes to other apps.

## Storage

Kept in `$XDG_CONFIG_HOME/irice/google` (mode `0700`, files `0600`, written
atomically):

- `accounts/<id>/`, one folder per connected account:
  - `cache.json`: events, tasks, calendars;
  - `sync.json`: sync tokens and times;
  - `connection.json`: which account;
  - for a custom client, its tokens;
- `outbox.json`: changes not on Google yet;
- `versions.json`: the versions your own recent edits replaced;
- `client.json`: a custom client's credentials, shared by its accounts.

The single account kept before several were possible moves into
`accounts/` on first run, as it is.

Tokens never reach QML, command lines or UiState. A sync that fails part-way
keeps the last complete data and position. **Remove local data** deletes
iRice's copy of an account, and its unsent changes, without contacting
Google.

## Not done yet

Keyring storage for the custom client's tokens; alerts for Google events
(Google keeps its own reminders); a deletion still inside its 8-second Undo
when the shell quits isn't sent.

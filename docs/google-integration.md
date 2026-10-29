# Google Calendar and Tasks

In **Settings → Google**. One Google account at a time.

## Connecting

**Connect Google account** is the whole flow:

- A Google account already in GNOME Online Accounts (GOA) is used as it is,
  with no browser. With several, Settings asks which.
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
unlinks iRice; the account stays usable by other apps. Needs
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
  Calendar, Today and Spotlight. Read-only in iRice (edit them in Google
  Calendar); **Add event** uses Google's quick-add on the primary calendar.
- **Tasks:** every task list, next to local to-dos.
  - Tick, rename, re-date or delete them as usual. Changes show at once, go
    to Google in the background, and only that list is fetched again.
  - Deleting can be undone for 8 seconds.
  - Google stores due dates but not times.
- **New tasks go to:** this computer only (the default), or a Google list.

Conflicts are last-writer-wins: a task changed here and elsewhere at the same
time keeps whichever change reached Google last.

Google Keep only opens in the browser: Google doesn't offer personal Keep
notes to other apps.

## Storage

Kept in `$XDG_CONFIG_HOME/irice/google` (mode `0700`, files `0600`, written
atomically):

- `cache.json`: events, tasks, calendars;
- `sync.json`: sync tokens and times;
- `connection.json`: which account;
- for a custom client, its credentials and tokens.

Tokens never reach QML, command lines or UiState. A sync that fails part-way
keeps the last complete data and position. **Remove local data** deletes
iRice's copy without contacting Google.

## Not done yet

Several accounts at once, editing Google events in iRice, keyring storage for
the custom client's tokens.

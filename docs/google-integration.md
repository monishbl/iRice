# Google Calendar and Tasks

In **Settings → Google**. One Google account at a time.

## Connecting

The default is the desktop's account service (GNOME Online Accounts): open
**Settings → Google → Open Online Accounts**, sign in to Google there with
Calendars enabled, then pick the account in iRice and **Connect account**.
GNOME Online Accounts owns the sign-in and token renewal; iRice asks it for
short-lived tokens over the session bus and never copies its credentials.
**Disconnect account** only unlinks iRice; the account stays usable by other
apps. Needs `gnome-online-accounts`, `python-gobject` and
`gnome-control-center` (or `gnome-online-accounts-gtk`).

Optionally (**Advanced connection**), your own OAuth client: a Google Cloud
project with the Calendar and Tasks APIs, and an OAuth client of type
*Desktop app* whose JSON you import. Sign-in then happens in your browser
(a random loopback port, state check and PKCE). Apps left in Google's
*Testing* state may have their refresh token expire after seven days.

## What syncs

- **Calendar:** events from your visible Google calendars, one year either
  side of the last sync, in Calendar, Today and Spotlight. They're read-only
  in iRice (edit them in Google Calendar); **Add event** uses Google's
  quick-add on the primary calendar.
- **Tasks:** every task list, next to local to-dos. Tick, rename, re-date or
  delete them as usual. Changes show at once, queue while a sync runs, and
  reach Google in the background; only the changed list is fetched again.
  Deleting can be undone for 8 seconds: the task is only deleted at Google
  once that has passed. Google stores due dates but not times, so a timed
  to-do can't go to a Google list.
- **New tasks go to:** Local (the default) or a Google list. Local to-dos are
  never uploaded on their own.
- Refreshes every 15 minutes, and on **Sync now**.

Conflicts are last-writer-wins: a task changed here and on another device at
the same time keeps whichever change reached Google last. Writes need a
connection; one that fails shows the error, and the task goes back to what
Google has.

Google Keep only opens in the browser: Google's Keep API is for enterprise
administration, not personal notes.

## Storage and failures

Client credentials, tokens and the cached events and tasks live in
`$XDG_CONFIG_HOME/irice/google` (mode `0700`, files `0600`, written
atomically). Tokens never reach QML, command lines or UiState. A sync that
fails part-way keeps the last complete one. **Disconnect account** revokes
the token before removing it; **Forget locally** removes it without a
connection (revoke iRice in your Google account afterwards).

## Not done yet

Several accounts, choosing which calendars show, editing Google events in
iRice, keyring storage for the custom client's tokens.

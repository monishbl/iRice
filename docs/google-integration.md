# Google Calendar, Tasks and Keep

Implementation added 29 October 2026. Available in **Settings → Google**.

## Connect without a Google Cloud project

1. Open **Settings → Google → Open Online Accounts**.
2. Choose **Google**, sign in in your browser, and enable **Calendars**.
3. Back in iRice, click **Refresh accounts**, select the account and click **Connect**.

The desktop account broker (GNOME Online Accounts) owns the OAuth application and manages sign-in/token renewal. iRice uses its supported D-Bus API; it does not copy GNOME's client credentials or refresh tokens. The Google provider requests Calendar and Tasks scopes. Availability depends on the installed provider, Google consent and your organization's policies.

New installations include `gnome-online-accounts`, `gnome-control-center` and `python-gobject`. Existing installations need these packages (or a compatible `gnome-online-accounts-gtk` frontend). Account setup launches outside iRice's isolated TTY config environment, so it uses the desktop's account/keyring service.

**Disconnect** removes iRice's account link and cache only; the shared Online Accounts entry remains usable by other applications. Remove it in Online Accounts if you want to disconnect the desktop entirely. Account selection is explicit, and accounts with Calendars disabled are not offered.

## Optional advanced connection: your own OAuth client

Enable **Advanced connection → Use a custom OAuth client** first. This is optional, not the default end-user setup.


1. In [Google Cloud Console](https://console.cloud.google.com/), create/select a project and enable **Google Calendar API** and **Google Tasks API**.
2. Configure the OAuth consent screen. If your app is in Testing, add your account as a test user. Google may expire refresh tokens after seven days for external apps in Testing; reconnect when needed.
3. Create an OAuth client with application type **Desktop app** and download its JSON file.
4. Enter that file's absolute path (or `~/…`) in iRice Settings → Google and click **Import client**.
5. Click **Connect**, finish consent in your normal browser, and return to Settings. Status/errors appear there. Sign-in times out after three minutes.

This build does not ship an iRice-owned OAuth client. A fully iRice-branded direct sign-in flow would require a maintainer-owned project, consent-screen configuration and any applicable Google verification. Desktop Online Accounts avoids that requirement for iRice users.

## What works

- One Google account at a time; refresh-token authentication, manual sync and 15-minute refresh while the shell runs.
- Visible Google calendars are merged into Calendar, Today and Spotlight. Google expands recurring events; timed events retain their timezone offsets. All-day dates use local calendar dates and exclusive end dates.
- A rolling window of 366 days before and after the last successful sync. Existing Google events are read-only inside iRice; use **Open Calendar** to edit them.
- **Add event** creates an event on the primary calendar using Google's quick-add parser. Check its interpretation in Google Calendar, particularly for ambiguous dates/languages.
- Tasks from all task lists appear alongside local reminders in Notification Center, Today and desktop widgets. Complete/reopen, rename, change a date, and delete using the existing task controls.
- **New tasks go to** chooses Local (the default) or a Google task list. Existing local tasks are never uploaded automatically. Google task due times are unsupported by the API; a timed task is rejected rather than silently losing its time.
- Google deletions have no Undo. Changes require a connection; there is no offline mutation queue. While a sync/write runs, other writes are rejected with a status message. A successful write followed by failed refresh can leave the cached view stale; sync before repeating the action.
- **Keep & notes** opens Google Keep in the browser. This is an external integration, not native Keep sync. Existing Quick Note/Markdown notes stay local.

## Account data and failures

For the optional direct connection, OAuth uses a browser, random loopback port, state validation and PKCE. Desktop Online Accounts handles its own authentication; iRice keeps only the selected account object path and cached events/tasks, and obtains short-lived tokens over the session bus. The helper accepts only Desktop client JSON and uses fixed Google API endpoints. Client credentials, tokens and cached events/tasks are stored under `$XDG_CONFIG_HOME/irice/google` (the shell's private TTY config directory when applicable), with directory mode `0700` and atomic `0600` files. Tokens do not enter QML, command-line arguments or UiState. Storage is permission-protected, not keyring-encrypted; exclude this directory from shared configuration exports.

Every list is paginated. A sync failure preserves the last complete snapshot instead of publishing a partial account. The Settings error identifies missing APIs/permissions, expired authentication or connectivity failures. **Disconnect** revokes the token before removing local tokens/cache. **Forget locally** removes local tokens/cache without requiring a network connection; revoke iRice separately in your Google account afterward. The imported OAuth client remains available for reconnecting.

Live authentication and real-account API access have **not** been verified in this checkout; Online Accounts is not installed and no OAuth client/account was supplied. Broker tests verify account filtering, token delegation and disconnect behavior without revoking shared desktop access. Automated tests cover callback state/PKCE, token refresh, private storage, pagination, task writes, sync failure retention and account removal. An isolated Quickshell smoke check verifies local/cloud task merging and local editing/undo without touching live settings.

## Official API constraints

- [Desktop OAuth flow](https://developers.google.com/identity/protocols/oauth2/native-app)
- [OAuth token expiration](https://developers.google.com/identity/protocols/oauth2#expiration)
- [Calendar event listing](https://developers.google.com/workspace/calendar/api/v3/reference/events/list)
- [Google Tasks resource and date-only due dates](https://developers.google.com/workspace/tasks/reference/rest/v1/tasks)
- [Google Keep API](https://developers.google.com/workspace/keep/api/reference/rest): enterprise administration; personal-account note synchronization is not offered by this implementation.

Remaining work: multi-account selection, per-calendar visibility controls, native Google event editing/deletion, offline conflict handling, native Keep synchronization if a supported personal-account API becomes available, and keyring-backed storage for the optional direct-auth token store (Online Accounts already owns its credentials).

- [GNOME Online Accounts credential API](https://gnome.pages.gitlab.gnome.org/gnome-online-accounts/overview.html)
- [GNOME Google provider and requested scopes](https://github.com/GNOME/gnome-online-accounts/blob/master/src/goabackend/goagoogleprovider.c)

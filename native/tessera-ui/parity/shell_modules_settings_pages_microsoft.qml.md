# shell/modules/settings/pages/microsoft.qml ↔ surfaces/settings (src/pages/accounts/accounts.rs `microsoft`, `drive_group`)

The Microsoft 365 part of Accounts (heading "Microsoft 365"; old link
`microsoft`). Data: feed `cloud`, `needs`, `drives`; actions `cloudSync`,
`cloudDisconnect`, `needsInstall`, `needsBuild`, `microsoftSignIn`, `drive*`.
No menus.

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | ago() 30 s tick (128–135) | mod.rs 32 | same at a glance |
| 2 | Group "Microsoft account" footer: lastError (not busy, not from icloud) › "Sign in to Microsoft in your browser…" › signed-in text › GOA text (137–143) | accounts.rs 347–356 | same |
| 3 | Per account: identity or "Microsoft account"; "Syncing…"/"Last synced …"; Sync now, Disconnect (danger), disabled while busy (144–157) | 358–367 | same |
| 4 | "GNOME Online Accounts is needed" / "… is too old" (none, checked, !goa or !goaMicrosoft): Install/Update/Installing… (download-simple, busy, disabled while updates run) → install gnome-online-accounts (158–169) | 369–378 | same |
| 5 | "Tessera's sign-in helper isn't built yet" / "Building…" (goa ok, no signin helper): detail buildError (label danger) or text; "Build It"/"Building…" busy → Needs.buildNative() (170–176) | 379–388 (tone 2) | same |
| 6 | "Microsoft 365 account" (none and (!checked or all ready)): primary "Sign in with Microsoft"/"Waiting for the browser…", busy signing here, disabled busy → connectMicrosoft() (177–188) | 389–395 | same |
| 7 | Group "Calendars and To Do" (signed in): links "Shown calendars" (calendar-blank → calendar "From your accounts"), "Where new to-dos go" (list-checks → "To-dos") (191–212) | 401–408 | same |
| 8 | DriveGroup onedrive: "OneDrive folder" (215; DriveGroup.qml) | 410, 414–473 | same (see google #14–16) |

## Test plan

Setups as pages_accounts. `open microsoft`. **Read y from a shot.** Never
click "Sign in with Microsoft", "Install"/"Update", "Build It" or "Connect OneDrive".

MS1 Page: shot the part. PASS: same rows as the QML for the arena's state
(which of #4/#5/#6 shows depends on Needs: write which one each shell shows;
they must agree), footer text, "OneDrive folder" group, both looks.

MS2 Hover: glide over the visible primary/Install button and the OneDrive
button. PASS: same hover/press fill as QML. Esc.

## Test results

(tester)

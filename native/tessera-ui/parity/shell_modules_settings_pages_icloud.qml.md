# shell/modules/settings/pages/icloud.qml ↔ surfaces/settings (src/pages/accounts/accounts.rs `icloud`)

The iCloud part of Accounts (heading "iCloud"; old link `icloud`). Data:
RustBridgeAccounts feed `cloud` (accounts carry their contact count),
`needs`; actions `cloudSync`, `cloudDisconnect`, `needsInstall`,
`appleAccount`, `icloudSignIn`. No menus.

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | ago() with a 30 s tick while visible (17–24) | mod.rs 32; rebuilt on data changes | same at a glance |
| 2 | Group "iCloud account" footer: lastError (not busy, not from microsoft) › "Checking your Apple ID…" while signing in here › signed-in text › app-specific-password text (26–32) | accounts.rs 281–289 | same |
| 3 | Per iCloud account: label identity; detail "Syncing…" (busy) / "Last synced … · N contacts"; "Sync now" (disabled busy) → sync(); "Sign out" danger (disabled busy) → disconnect(id) (34–47) | 291–296 | same |
| 4 | "A keyring is needed first" (none signed in, checked, no secret-tool or keyring): Install/Installing… download-simple, busy installing, disabled while updates run → Needs.install(gnome-keyring, libsecret) (50–61) | 299–306 | same |
| 5 | "1. Make an app-specific password" + "Open account.apple.com" (arrow-square-out) → account.apple.com/account/manage (63–68) | 308–309 | same |
| 6 | "2. Apple ID" TextField 260 "you@icloud.com", no commit on blur (69–73) | 310 `field(.., 260, false)` | same |
| 7 | "3. App-specific password" TextField 260 secret "abcd-efgh-ijkl-mnop" (74–78) | 311 (secret) | same |
| 8 | "Sign in to iCloud": primary "Sign in"/"Signing in…" busy while signing here, enabled when not busy and both filled → connectICloud(id.trim, pw); password cleared, Apple ID kept (79–93) | 312–325 | same |
| 9 | Group "Calendars" (signed in): footer; SubpageLink "Shown calendars" → calendar "From your accounts" (96–109) | 332–337 | same |
| 10 | Typed Apple ID/password dropped when the page unloads | lib.rs 511/579 keep `acct.icloud.*` | **different** (accounts #6): typed text (incl. the password) survives leaving the page |

## Test plan

Setups as pages_accounts. `open icloud`. **Read y from a shot.** Never type
a real Apple ID or password; never click "Sign in" or "Open account.apple.com".

I1 Page: shot. PASS: "iCloud account" with rows 1.–3. and "Sign in to iCloud"
(button greyed), plus the keyring row if the arena has no keyring, same texts
and footer as QML, both looks.

I2 Fields: click the "2. Apple ID" field, type `x` (`run 'key -t x'`); PASS:
placeholder hides on focus, accent border (Glass) / primary outline (Material);
Sign in stays greyed (password empty). Clear it (`run 'key -k BackSpace'`), Esc.

I3 Hover: glide over "Open account.apple.com" and "Sign in". PASS: same hover/disabled look.

I4 Reset: type `x` in Apple ID, go to another page and back. PASS (QML): field empty. Rust must match.

## Fixed in batch 3 (fixer)

- Typed state (the Apple ID, the password) starts over with the page.

## Test results

(tester)

# shell/modules/settings/pages/accounts.qml ↔ surfaces/settings (src/pages/accounts/accounts.rs `page`)

Accounts (page `accounts`): three PageParts, Google, iCloud and Microsoft 365
(google.qml, icloud.qml, microsoft.qml, each with its own parity file). Rust
builds them as parts of one page. Older links (`google`, `icloud`,
`microsoft`) go to `accounts` and scroll to the part's heading (map.rs 55–57,
74–76).

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | Column, 22 px between parts (9) | model.rs part spacing | same |
| 2 | PagePart "Google" → google.qml (11–14); heading Material 22 px Normal, Glass title DemiBold, 16/12 px above (PagePart.qml 18–27) | accounts.rs 10 `p.part(cx, "Google", google)` | same |
| 3 | PagePart "iCloud" → icloud.qml (15–18) | accounts.rs 11 | same |
| 4 | PagePart "Microsoft 365" → microsoft.qml (19–22) | accounts.rs 12 | same |
| 5 | An older link (`ipc panels settings google` / `icloud` / `microsoft`) opens Accounts at that part's heading (PagePart.qml 4–7) | map.rs 55–57 / 74–76 | same |
| 6 | The page is a Loader item: leaving the page or closing Settings drops every part's own state (Google's Advanced switch, chosen account, typed text; iCloud's typed Apple ID/password) (Settings pageLoader) | lib.rs 511, 579: only `more:` (and Wi-Fi) keys dropped | **different**: `acct.*` local keys survive page changes and reopening |

## Test plan

Setups: `tools/settings-scenarios.sh setup qml material`, `setup rust material <bin>`,
then both with `glass`. Material window 1040×700 at 440,205, page column from
x≈772; Glass: read the window's place from setup's output. **Read every y from
a shot.** Never sign in to an account, never type a real key or password:
look, hover, open menus, Esc only.

A1 Page: `open accounts`, `shot acc-top`; scroll (`run 'P move 1100 600 scroll 5'`)
and shot until the end. PASS: headings "Google", "iCloud", "Microsoft 365"
in that order, each part's groups as the QML (both looks), same heading size/weight.

A2 Old links: `open google`, `shot acc-google`; `open icloud`; `open microsoft`.
PASS: Accounts opens scrolled so that heading is at the top, both shells.

A3 State resets: in the Google part open More options, switch "Use your own
OAuth client" on; click another page in the list, then come back to Accounts.
PASS (QML): More options closed and the switch is off again. Rust must match.

## Fixed in batch 3 (fixer)

- Its typed state (acct.*) starts over when you leave the page or open Settings again.

## Test results

(tester)

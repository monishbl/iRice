# shell/modules/settings/pages/about-health.qml ↔ surfaces/settings (src/pages/about/health.rs `page`, ui/settings-about.slint `health`)

System health (page `about/health`, title "System health"). Checks and fixes
are the shell's (Health, Conflicts): feed.about.health / conflicts, asks
`health.*`, `conflicts.*`.

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | Title "System health"; Health.check() on open (13–15) | health.rs 182; RustBridgeAbout.qml 30–31 | same |
| 2 | Top footer: error / "Checked at <h:mm>. Nothing is checked in the background…" / "" (29–33) | 208–215 (bridge formats checkedAt) | same |
| 3 | Top label: Checking… (no checks yet) / All good / 1 thing needs attention / N things…; red when any fails; detail "" / "Every service Tessera uses is answering." / attention names (34–40) | 190–206, 216 | same |
| 4 | "Copy report" (copy, disabled with no checks) → copyReport; "Check again"/"Checking…" (arrow-clockwise, busy) → check() (41–55) | 207, 216–219 | same |
| 5 | "Also running" (Conflicts.found): name and what capitalised, "\nTo stop it: <fix>"; "Don't show again" ↔ "Show alerts" (mute/unmute); footer (60–79) | 221–234 | same |
| 6 | A group per Health.groups with its checks (hidden when none) (81–90) | 239–297 | same (empty groups fold away) |
| 7 | Check row: detail + "\nRuns as administrator[, after your password]: <command>" when confirming, else "\nTo fix: <fix>" for fail/warn (93–98) | 247–257 | same |
| 8 | Buttons 10 apart: Cancel (confirming); Fix / "Run with password" / "Run fix" / "Fixing…" (primary while confirming, busy, disabled while any fix runs); root fixes ask once first (99–120) | 258–286 | same |
| 9 | Status: 8 px dot (success / warning / danger / tertiary) + word OK / Check / Not working / Off, small; secondary for ok/off, DemiBold for fail (121–138) | word/tone 6–25; slint health 603–637 | same |

## Test plan

Setups as in pages_menubar. `open about/health`; wait 5 s; read y from a
shot. **Never press "Run fix"/"Run with password"** (a first "Fix" on a root
fix only asks and may be pressed, then Cancel). A non-root Fix runs at once:
don't press Fix unless the row shows it would ask (fixRoot).

H1 `shot health` (both shells, both looks; scroll and shot to the end).
PASS: same top label/detail, groups, rows, dots and words.

H2 Check again: click it, `shot health-checking` at once. PASS: "Checking…"
busy button, then the footer time updates.

H3 Copy report: click; `run 'wl-paste | head -3'`. PASS: a report in the
clipboard in both shells.

H4 Also running (if shown): click "Don't show again" → "Show alerts"; click
again to put it back.

## Test results

(tester)

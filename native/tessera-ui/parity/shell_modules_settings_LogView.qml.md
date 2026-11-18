# shell/modules/settings/LogView.qml ↔ surfaces/settings (ui/settings-about.slint `LogBox`, the `log` custom row)

_Re-checked 17 Nov (B5, parity-settings) against main 18628a5 + the B5 ports: statuses below say what's there now; [12 Nov: …] is the old status._

Text as a terminal shows it: the update's output (follow) and the bug
report's log and system info (no follow).

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | Squircle radius Material 16 / Glass 8; colour inverseSurface / #0E0E10 dark / #18181B light (6–13) | LogBox 78–82 | same |
| 2 | Flickable, margins 10, clip, StopAtBounds, Momentum (kinetic wheel) (14–20) | LogBox 86–91 (Slint Flickable) | same at a glance |
| 3 | Text: wrap anywhere, plain, mono 11, inverseOnSurface / #D4D4D8 (24–32) | LogBox 101–109 (`char-wrap`) | same |
| 4 | follow: keeps the end in view as text comes unless scrolled up; atEnd set when a movement ends (≤ 4 px from the end) (21–23) | LogBox 92–100 (at-end on every content-y change) | same |
| 5 | **ScrollIndicator** at the right edge (2 px in) while scrolling (34–37) | — | **same (fixed before 17 Nov: LogBox: a 4 px indicator at the right edge while there's more)** [12 Nov: **missing**: no scroll indicator] |
| 6 | Size by the caller: update 240 (margins 10 → 220 box), bug log 200, info 140 (update.qml 415–427, about-bugreport.qml 173–210) | slint `log` row 420–428: r.on ? num−20 with 10 padding : num with 12 padding | same |

## Test plan

Setups as in pages_menubar. Safe place: `open about/bugreport`, Tessera log
row › "Show" (read y from a shot).

L1 `shot log-box`. PASS: dark box 200 tall, 12 px in from the group's edge,
mono 11 px text wrapping anywhere; same colours per look.

L2 Scroll inside the box (`run 'P move X Y scroll 3'`, X/Y inside it),
`shot log-scrolled` during the scroll. PASS (QML): a thin scroll indicator at
the right; Rust: write whether one shows.

L3 Follow (only with a simulated update, see update parity U4): with details
shown the end stays in view; scroll up and it stays put.

## Fixed in batch 3 (fixer)

- A thin scroll indicator while there's more.

## Test results

(tester)

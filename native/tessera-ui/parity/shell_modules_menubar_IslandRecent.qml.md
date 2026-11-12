# shell/modules/menubar/IslandRecent.qml

The open island's Recent list: the last 8 alerts (Island.history), one line
each, swipe or × to remove, Clear, a tap runs a remembered button. With live
activities it's a collapsible header (and IslandView's top-left Recent
button, 755-768). Material inline. Rust: none in the island (lib.rs header
13: "the island's Recent list" not yet). The host keeps the history and
publishes it (`host/shell/src/island.rs` `record` 329-367,
`publish_history` 369-372 → state `island.list`), with `forget`, `clear`,
`alertAct` calls (903-917) that a Rust list could use.

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| Appear: 280 ms × motionScale OutCubic, rises 8 px (none with reduce motion) | 32-35 | — | missing |
| Minute clock for "5m" while visible | 38-42 | — | missing |
| Header: "Recent" (collapsed: "Recent · N"), caret when collapsible; hover 0.1 / press 0.16, focus ring; Enter/Space/click toggles; a11y Show/Hide recent alerts | 45-90 | — | missing |
| Clear: hover / press, focus ring, Enter/Space/click → clearHistory; a11y | 91-120 | host island.rs:914-918 `clear` | missing in the UI |
| Flickable list, momentum, clipped, max height from IslandView | 124-134 | — | missing |
| Up to 8 lines of Island.history, 36 px | 139, 25, 24 | host state `island.list` | missing in the UI |
| Swipe sideways: follows, > 72 px or 500 px/s slides off (150 ms × motionScale InCubic) then forget; else back (popover ms OutCubic) | 147-184 | host island.rs:908-913 `forget` | missing |
| Tap a line with a remembered button (Open a download / drive, What's new) | 185-189 | host island.rs:903-907 `alertAct` | missing in the UI |
| Line hover background 0.08 (150 ms) | 196-201 | — | missing |
| Badge 24: picture, else icon on colour 0.22 (Material tint / onTint) | 202-225 | — | missing |
| Title small DemiBold (≤ 60 %), then subtitle · battery % · ×times | 226-242 | — | missing |
| Ago ("now", "5m", "2h", time) caption dim 0.45; × button while hovered (Remove <title>) | 243-283 | — | missing |

## Summary

1. The island has no Recent list at all (nor the top-left Recent button); the data and calls exist in the host.
2. No swipe / × to forget, no Clear, no tap-to-run in the island.

## Fixed in batch 2 (fixer, 7 Nov)

- The Recent list (`RecentList` in island.slint) under the status line: header "Recent" (a toggle when activities share the island) and "Clear"; up to 8 lines from the host's history (state `island.list`, newest first), each: a 24 px badge (the alert's icon in its colour at 22 %, Material primary/error circle; or its picture), the title (12/600, at most 60 % of the room), the detail ("subtitle · NN% · ×N"), how long ago ("now", "5m", "2h", else HH:MM), and on hover a 20 px × instead of the time. Fades in 280 ms OutCubic. More than ~7 lines scroll (300 px).
- A click on a line runs its remembered button (host `island.alertAct`; the host now marks lines whose button is remembered, `act`); dragged sideways past 72 px, or its ×, the line slides off and fades (150 ms InCubic) and is forgotten (`island.forget`); Clear → `island.clear`.
- The Recent button (clock-counter-clockwise, top left, the pin's twin) when live activities share the island: opens the list in their place (the cards/rows hide), closes with the island.
- Host fix: `publish()` replaced the whole `island` state object each call, wiping `island.list`: it now sets its fields one by one.
- Different: the island doesn't hold its height while Recent is open over the activities (recentHoldH); "Recent · N" isn't shown in the header (it shows only while open); the release velocity flick (> 500 px/s) isn't used, only the 72 px distance.

## Test plan (Sonnet tester)

1. Make history: `ipc island demo download`, `ipc island demo usb`, `ipc island demo wifi` (let each leave). Hover the island open (no activities): under the status line, "Recent" and "Clear", then three lines newest first with icon, title, detail and "now".
2. Hover a line: its time turns into a ×; click ×: the line slides right and fades (~150 ms), the others move up.
3. Drag a line left by 100 px (`P down`, glide −100, `P up`): it slides off and goes. Drag one 30 px and release: it springs back.
4. Click the download line (it has "Open" remembered): the download's Open runs.
5. Click Clear: the list empties and disappears.
6. With a timer running (activities), make history again: the top-left clock button appears; click it: the timer row/card hides and Recent shows; click again: back. Close the island and reopen: Recent is closed.
7. Both looks.

## Test results

## Fixed in batch 3 (fixer, 7 Nov)

- Call card staggered in as IslandCall on IslandEvent's clock: photo scales 0.7 → 1 (from 70 ms), name/app fade in and slide 12 px (from 105 ms), buttons fade in (from 210 ms), each 315 ms OutCubic.
- Recent over live activities keeps the island as tall as it was when opened (recentHoldH), so it doesn't shrink from under the pointer.
- Recent lines: a quick flick (> 500 px/s on release) removes a line too, not only 72 px of travel.
- "Recent · N" in the header: in the QML the header shows only while the list is open (the list is hidden while collapsed), so "· N" is never seen there either; the count is in the Recent button's tooltip label ("Recent alerts · N").

## Test plan additions (batch 3)

- Call: record the card's appearance (1 s): photo grows, then the text slides in, then the buttons fade in, ~0.5 s overall.
- Recent over activities (a timer running + history): open Recent with the clock button: the island keeps its height (no jump down) while the list shows.
- Flick a Recent line quickly (`P down`, glide −60 px in 50 ms, `P up`): it goes.

## Test results (batch 3)

## Fixes after testing (fixer, 7 Nov, batch 3 build)

- Badge mid-row (batch 2 FAIL 2): the badge had no x (Slint centres it); it's at the line's left now.

Retest the FAIL steps above.

## Fixed in batch 4 (fixer, 7 Nov)

- The list's entrance on its own clock (fade over 280 ms OutCubic).

## Test plan (batch 4)

- Open the island with history: Recent fades in (record: not one frame).

## Test results (batch 4)

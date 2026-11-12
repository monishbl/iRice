# shell/modules/menubar/IslandAlertStack.qml

Alerts that arrive while the island is already open (hovered or pinned):
the island stays as it is and they drop in as a card just under it (newest
on top, up to two older ones peeking 7 px each, "+N" waiting). Material
inline. Rust: none. `src/island.rs` `view` (352-369) sets
`event_inside = showing`, so an alert always takes over the open island
(the activities card and status line go) instead of dropping below. lib.rs
header lists "its alerts card below an open island" as not yet (13-14).

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| Shown while IslandView.alertsBelow (alert up and island hovered / pinned, not over full screen) | 23, 37; IslandView 47 | src/island.rs:352-369 | missing: alerts replace the open content |
| Newest three uids, newest first | 28-34 | — | missing |
| Own overlay layer window 392 + 2·16 wide, 640 tall, input mask = the stack, no keyboard | 39-56 | — | missing |
| Appears: `shown` 0→1 popover ms OutCubic, slides down 8 px | 58-60, 72, 75 | — | missing |
| Hover holds the alerts' time (`below:<screen>`), released on destruction | 62-66 | — (the island's own hover holds: island.slint:1142) | missing for this card |
| Older cards peek under it: 7 px each, inset 12 px per depth, Glass rgba(0.1,0.1,0.11, 0.92−0.18·depth) rim 0.1; Material surfaceContainerHighest / High | 82-104 | — | missing |
| Front card: Glass blur (Glass item, radius tile) + black 0.92→0.55 gradient + rim 0.14; Material card with elevation-2 shadow (offset 2, blur 12) in IslandInk.surface | 107-154 | — | missing |
| Height follows the alert (popover ms OutCubic) | 110-111 | — | missing |
| IslandEvent inside (18 / 16 inset); acted → Island.remove(uid, since); dismissed → remove newest | 155-165 | — | missing |
| "+N" waiting, bottom-right, dim 0.5, tabular | 167-173 | lib.rs pill text "+N" only over a full-screen app (~642) | missing here |

## Summary

1. Hovering or pinning the island and getting an alert swaps the island's content for the alert instead of showing a card under it; the live activity disappears meanwhile.
2. No stacked peeking cards / "+N" count.
3. No hold-on-hover for that card (moot until it exists).

## Fixed in batch 5 (fixer, 8 Nov)

- Alerts that start while the island is hovered or pinned open (not over a fullscreen app) go into a card under it; the island keeps what it shows (the activities, Recent). The card: 392 wide, 8 px under the island, Glass black 0.9 with a rim / Material surfaceContainerHigh 28 px with elevation; it drops in 8 px and fades in (300 ms OutCubic); the newest alert in it (its buttons work), the earlier ones as a card peeking out under it and "+N waiting". Resting on it holds them (Island.hold). Decided once when they start; gone with the last one. It takes the pointer (input region).

## Test plan (batch 5)

- Pin the island (or hover it open), then `ipc island demo download`: the island keeps its content; a card appears under it with the download alert (Open / Show in Folder work). `ipc island demo usb` too: "+1 waiting" and a peeking card. Rest on the card: they don't time out; leave: they do. Unpinned and not hovered, a new alert goes in the island itself as before.

## Test results (batch 5)

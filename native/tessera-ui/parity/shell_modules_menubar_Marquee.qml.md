# shell/modules/menubar/Marquee.qml

A one-line label that, when it overflows and `scroll` is on, slides to show
the rest, pauses, and comes back; otherwise elides. Used by IslandLive (song
title in the pill) and IslandFocusCard (title over the art). Rust: none;
`Text { overflow: elide }` in `surfaces/menubar/ui/island.slint` LiveInfo
(1006-1014) and the focus card (1351-1357). lib.rs header lists "a scrolling
song title" as not yet (15).

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| Overflow test against `maxWidth` (a hidden probe) | 15-26 | island.slint:1007 (`max-width`) | same (width only) |
| Elide when not scrolling | 36 | island.slint:1012 | same |
| Clip while scrolling | 18 | — | missing |
| Motion: wait 1.8 s, slide out over max(0.6, span·0.033) s, wait 1.4 s, back over max(0.4, span·0.012) s; cosine ease; whole pixels | 39-48 | — | missing |
| 30 fps Ticker, only while overflowing, scrolling and visible | 50-54 | — | missing |
| Restart on a new text | 55 | — | missing |
| `music.scroll` piece switches it (Island.shows) | IslandLive 119, FocusCard 333 | settings.rs:113 lists `music.scroll` | different: read, not used |

## Summary

1. Long song titles never scroll (pill and open card); the `music.scroll` setting does nothing.

## Fixed in batch 4 (fixer, 7 Nov)

- Ported (`Marquee` in island.slint): a title wider than its room rests 1.8 s, glides out (cosine, max(0.6 s, 33 ms/px)), rests 1.4 s, glides back (max(0.4 s, 12 ms/px)), repeats; with `music.scroll` off it's cut with an ellipsis. Used by the pill's music title (132 px) and the open card's title. Runs on animation-tick only while it overflows (QML: a 30 fps Ticker).

## Test plan (batch 4)

- FAKEPLAYER_TRACK="A very long song title that does not fit in the pill at all|Artist" python3 sandbox/tools/fakeplayer.py &: with music.scroll in livePieces, the pill's title rests ~1.8 s then scrolls to its end, rests, scrolls back (record 8 s); without music.scroll: an ellipsis, still.

## Test results (batch 4)

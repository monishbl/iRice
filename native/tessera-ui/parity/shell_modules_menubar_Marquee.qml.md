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
| Clip while scrolling | 18 | island.slint `Marquee` `clip: moving` | same (fixed after batch 2) |
| Motion: wait 1.8 s, slide out over max(0.6, span·0.033) s, wait 1.4 s, back over max(0.4, span·0.012) s; cosine ease; whole pixels | 39-48 | island.slint `Marquee` `t` / `k` (rounded x) | same (fixed after batch 2) |
| 30 fps Ticker, only while overflowing, scrolling and visible | 50-54 | island.slint `Marquee` (animation-tick only while `moving`) | different: frames at the display's rate while it moves, not 30 fps (Slint has no frame-rate cap per item) |
| Restart on a new text | 55 | island.slint `Marquee` `changed text => t0` | same (17 Nov) |
| `music.scroll` piece switches it (Island.shows) | IslandLive 119, FocusCard 333 | lib.rs `show_scroll: cfg.shows("music", "scroll")` → `scroll:` | same (fixed after batch 2) |

## Summary

1. Long song titles never scroll (pill and open card); the `music.scroll` setting does nothing.

## Fixed in batch 4 (fixer, 7 Nov)

- Ported (`Marquee` in island.slint): a title wider than its room rests 1.8 s, glides out (cosine, max(0.6 s, 33 ms/px)), rests 1.4 s, glides back (max(0.4 s, 12 ms/px)), repeats; with `music.scroll` off it's cut with an ellipsis. Used by the pill's music title (132 px) and the open card's title. Runs on animation-tick only while it overflows (QML: a 30 fps Ticker).

## Test plan (batch 4)

- FAKEPLAYER_TRACK="A very long song title that does not fit in the pill at all|Artist" python3 sandbox/tools/fakeplayer.py &: with music.scroll in livePieces, the pill's title rests ~1.8 s then scrolls to its end, rests, scrolls back (record 8 s); without music.scroll: an ellipsis, still.

## Test results (batch 4)

## Re-check 17 Nov (B4)

The Marquee had been built after batch 2; the rows still said missing. Ported: restarting on a new title. Before: 4 missing, 1 different; after: 6 same, 1 different (frame rate).

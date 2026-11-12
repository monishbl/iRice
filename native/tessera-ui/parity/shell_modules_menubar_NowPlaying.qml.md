# Parity: shell/modules/menubar/NowPlaying.qml

The Now Playing popover from the bar's music item (island off): art, title,
seek bar, transport, other players. Material: art cut into a cookie while
playing, morphing play button, tonal skip buttons with tooltips.
Rust: `surfaces/statusmenus/` panel "nowPlaying" — `src/lib.rs` `player`
569-580, `position_now` 583-590, `read_position` 593-611, `push_np` 613-653,
`np_do` 655-666, `seek` 668-678, `pick` 680-689, Tick 958-966;
`ui/statusmenus.slint` 708-850 (layout), `Transport` 140-169, `PlayButton`
171-199, `GlassSeek` 201-235. Popover open/close behaviour: see StatusMenus.qml.md.

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Popover 320 wide at anchorX − 120, lens 0.35, autoClose 5000 | 23-31 | model.rs:23; lib.rs:85 | same |
| Closes: outside click / Escape / 5 s without pointer / bar item again | Popover | see StatusMenus.qml.md | same (bar-strip empty click differs) |
| Player shown: `Media.player` (pinned, else the active one) | 33 | lib.rs:569-580 (picked here, else shell `media.bus`, else chosen) | same |
| Position re-read once a second while shown and playing; on show; on player change | 34-42 | lib.rs:358-361, 377-379, 958-966, 593-611 (busctl Position + elapsed interpolation) | same |
| Glass art 72×72 radius 12 (fill bg; music-notes 28 when none) | 55-77 | statusmenus.slint:713-731 | same |
| Material art: ShapedArt cookie9 while playing, square when paused (morph) | 79-101 | statusmenus.slint:716-717 (circle 36 px / radius 16, spring(0.5) 300 ms) | different: circle instead of cookie; rounded square not square (lib.rs:37-38) |
| Title (title font DemiBold, "Nothing playing"), artist — album, identity (caption tertiary) | 102-127 | statusmenus.slint:733-755; lib.rs:633-635 | same |
| Seek bar visible when length supported and > 0 | 131-134 | statusmenus.slint:758 (`np.timed`) | same |
| Seek: Slider `onMoved` while dragging → absolute position (only if canSeek) | 135-146 | statusmenus.slint:229-233 (Glass: only on release, no drag), 763-769 (Material QsSlider); lib.rs:668-678 (relative Seek, no CanSeek check) | different: Glass seek has no drag, jumps on release; seeks without canSeek (lib.rs:44-45) |
| Elapsed / "-remaining" labels, caption tertiary, tnum | 147-152 | statusmenus.slint:770-786 | same except tnum |
| Glass transport: prev 22 / play-pause 30 / next 22, spacing 18, 0.35 when disabled, hit area +8 px | 155-175 | statusmenus.slint:788-821, 140-169 (size+16 box, spacing 2) | same |
| Material: tonal IconButtons 40×40 (prev/next, iconSize 20, 0.38 disabled) around a 56 px morphing PlayButton | 177-216 | statusmenus.slint:140-169 (m3: 40×40, hover 0.08 / pressed 0.1, radius → 8 when pressed), 171-199, 806-811 | same |
| Material IconButton tooltips "Previous" / "Next" | 191, 210 | — | missing |
| Other players list (when > 1): "identity · title", 30 px rows, hover fill, click pins (`Media.pinned`) | 218-244 | statusmenus.slint:823-847; lib.rs:680-689 (+ `shell::ask pin`) | same |
| Other players list rebuilt on each push | — | lib.rs:651-652 (`model(rows)`) | different (lists should change in place) |

## Summary (missing / different, by priority)

1. Glass seek bar can't be dragged (jumps on release only); seeks without checking canSeek.
2. Material skip buttons have no tooltips.
3. Material art: circle instead of cookie9 (Slint can't clip to the shape).
4. Other players' list replaced on every refresh.

## Fixed in batch 5 (fixer, 8 Nov)

- Glass seek bar: the knob follows a drag; it seeks once on release (it only jumped on release).
- The other players' list changes in place (Rows), not a new model each refresh.
- Material art cookie: see ShapedArt.qml.md.
- Not done: skip buttons' tooltips (Material), canSeek (core's MPRIS reading has none).

## Test plan (batch 5)

- Glass Now Playing: press on the seek bar at 25 %, drag to 75 %: the knob follows; release: the time jumps there once.

## Test results (batch 5)

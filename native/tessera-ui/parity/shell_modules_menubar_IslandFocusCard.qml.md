# shell/modules/menubar/IslandFocusCard.qml

The open island's view of one live activity: a mini player for music
(overlay or side layout), captions, or timer / stopwatch / recording /
transfer / call with their controls. Material inline (no twin). Rust: the
"IslandFocusCard, simplified" block in `surfaces/menubar/ui/island.slint`
(1324-1393), fed by `surfaces/menubar/src/lib.rs` `live_struct` (346-420) and
`island_control` (1122-1139). In progress at audit time: `card` / `card-on`
and `control(kind, i, true)` (every control, not the pill's) in island.slint;
lib.rs wiring not yet there.

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| Height: music 200 (overlay) / 132 (side), captions its lines, others 80 | 36 | island.slint `cardl`: music overlay (art band 64 + title + seek + transport), side row 72 + seek + transport, captions its lines, others a fixed 80 px Rectangle | same (17 Nov: others 80, captions by lines, the side player; overlay measured by its parts, not a fixed 200) |
| Timer: asks MPRIS for the position each second while visible and playing | 39-44 | lib.rs `follow_position` (1 s `pos_timer` while open with music) → `read_position` | same (fixed after batch 1) |
| fineClock: asks Timers for the hundredths tick while a running stopwatch shows | 32-35 | host island.rs stopwatch `fine` {ms, running} → Live `fine-ms`/`fine-running`; island.slint `sw-text` counts on `animation-tick()` from each push while open | same (17 Nov; m:ss.cc as Timers.formatMs) |
| openCast(): Control Center on the focused screen, Cast page | 53-56 | lib.rs `M::OpenCast` → `rustui act page "cast <output>"` | same (17 Nov) |
| openPlayer(): raise, else focus the app's window, close panels | 57-70 | lib.rs `open_player` (mpris raise, focus by class, `panels close`) | same (fixed earlier) |
| Volume model: the player's own volume if supported, else system; mute remembers the level | 71-100 | lib.rs `volume_now` / `muted_now` / `toggle_mute` (`saved_volume`) | same (fixed earlier) |
| SeekWave (Material wavy progress, click seeks) | 103-117 | island.slint `WavySeek` + TouchArea → `seek` | same (fixed earlier) |
| Ctl: 36×36 round, icon 18 | 119-126 | island.slint card controls (music 36×36 / 18; others 36×36 / 18) | same (17 Nov: the other kinds' were 36×32 / 20) |
| Volume: speaker (Mute/Unmute tooltip); hover widens 36→104 (150 ms OutCubic) to a slider (drag); wheel ±0.05 | 131-185 | island.slint `vol` | same (fixed earlier; a11y name instead of a tooltip) |
| OpenApp: hover widens left to the app's name, tap opens the player, tooltip "Open …" | 188-230 | island.slint `oa` | same (fixed earlier; a11y name "Open …") |
| Transport: prev (dim when !canGoPrevious) / play 44 px icon 24 / next, spacing 10 | 233-256 | island.slint music controls (`card.controls` = every control, `all`) | same (fixed earlier; Glass play icon 18 in a 44 px button) |
| Material PlayButton (morphing shape) | 245-254 | island.slint `PlayMorph` | same (fixed earlier) |
| Cast button (when devices exist or casting; filled while casting; tooltip) | 261 | island.slint `card.cast-shown` IconButton left of OpenApp; lib.rs reads state `cast` (devices / active / device) | same (17 Nov; a11y name "Cast to a TV" / "Casting to …" for the tooltip) |
| Overlay "faded": art behind everything with edges faded (edgefade shader) | 272-292 | lib.rs `background_art(_, 2)`: cropped to the card, alpha by edgefade.frag's smoothsteps (0.28/0.22/0.28/0.6); island.slint `card.faded` Image | same (17 Nov; the fade baked into the picture, no shader) |
| Material overlay: art under a 72 % surfaceContainerHigh wash, radius tile−12 | 295-314 | island.slint `Theme.material && card.has-bg-art` block | same (fixed earlier; wash on surfaceContainerHighest, the island's own surface when open) |
| No art: music-notes 40 px | 315-322 | island.slint music `!card.has-art` SymIcon 40 | same (fixed earlier) |
| Title: Marquee 16 DemiBold, "Not Playing" fallback, scrolls with the `music.scroll` piece | 330-336 | island.slint `Marquee` 16/600, `scroll: card.show-scroll` | same (fixed earlier) |
| Artist · album, dim 0.75 | 337-344 | lib.rs artist = "artist · album"; island.slint 0.75 white / on-surface-variant | same (fixed earlier) |
| Seek row: position and −remaining (caption, tabular), track 4 px → 6 on hover (150 ms), click seeks if canSeek | 346-397 | island.slint seek row (`card.seekable`, 4→6 px hover, press/drag/release seeks) | same (fixed earlier; no tabular figures: Slint) |
| Side layout: 72 px art in a squircle mask (radius tile−12); Material cookie9 while playing, square paused | 405-463 | island.slint `card.side` row: 72 px, radius tile−12 (Glass); Material round while playing, square paused (spring) | different: rounded rectangle, not a squircle; Material a circle for the cookie (Slint can't clip a picture to a path) |
| Side layout: title 16, artist · album, player identity (dim 0.4, caption) | 464-490 | island.slint `card.side` column (title 16/600, artist · album, `app-name` at 0.4) | same (17 Nov) |
| Captions: "Live captions" + last 4 lines + Off button (turns captions off) | 553-580 | island.slint `card.kind == "captions"`; host island_live.rs captions (state `livecaptions`, lines from captions.rs, Off → `captions off`) | same (17 Nov; captions in the island were never an activity standalone before) |
| Badge 44 px circle, activity colour 0.22; Material cookie9 in IslandInk.tint (error when red) | 587-601 | island.slint `badge` 44, tint 0.22; Material `Path` cookie9 (shapes.rs) in primary / error | same (17 Nov) |
| Call: the app's icon (28 px) in the badge | 603-609 | island.slint badge `card.kind == "call" && card.has-app-icon` Image 28 | same (17 Nov) |
| Badge icon 22 (Material 20), onTint | 610-617 | island.slint badge SymIcon 22 / 20, on-primary / on-error | same (17 Nov) |
| Text: transfer / call / screenShare / sysupdate: title (body DemiBold) over detail; timer "Focus · paused", stopwatch "· N laps", recording "· mic off" over a 24 px tabular number (recording red, grey paused) | 619-639 | island.slint `wordy`; host island_live.rs `caption` (timer paused, laps from timers.rs `sw.laps`, mic off) | same (17 Nov; no tabular figures: Slint). Mirroring / casting / dictation say their own title over the time; the QML's fall-through said "Recording" (RUST-ONLY) |
| Buttons: every LiveInfo control (reset, discard, mic, pause, stop…), danger red, 0.4 when off | 640-655 | island.slint `card-ctls` (`card.controls` = `all`) | same |
| Recording mic meter (√level, grey muted, 80 ms width; counts meter users) | 656-677 | lib.rs `follow_meter` (parec 8 kHz mono, peak each 80 ms, only while the recording card shows and the mic isn't muted) → `mic-level`; island.slint meter | same (17 Nov; parec in place of PwNodePeakMonitor) |
| Progress bar 4 px (white 0.16, activity colour, width 900 ms); Material wavy | 678-700 | island.slint card progress (Glass bar, Material `WavySeek` in primary / error) | same (17 Nov) |
| Page dots (IslandView 676-697): clickable, focus that one | IslandView 690-693 | island.slint dots TouchArea (−4 px) → `focus-at(i)` → lib.rs `M::IslandFocusAt` → island.rs `focus_at` | same (17 Nov) |
| Swap animation when cycling (opacity + 16 px slide by direction) | IslandView 667-668 | island.slint `swap` / `swap-dir` (on the shown kind changing): card opacity + 16 px side padding, pill opacity + 10 px | different: only the swap's in-half (260 ms OutBack 1.6; OutCubic with reduce motion) — the old one doesn't fade out first (the data has already changed); the pill doesn't shrink to 85 % (a layout can't scale) |
| Wheel over the card cycles activities | IslandView 669-673 | island.slint `over` scroll-event | same |

## Summary

1. Music: no seek bar / times, no volume control, no Cast, no open-the-player; play/prev/next follow the pill's piece settings.
2. Non-music: no big number, no detail line, no progress bar, no mic meter; every control appears only once lib.rs passes `all`.
3. Captions card (lines + Off) missing; call app icon missing.
4. Art styles (faded / Material wash / island background, cookie shapes) missing; layout always the small one; no title marquee.
5. Page dots not clickable; no swap animation; sizes (36×32 / 40 px badge / 64 px art) differ.

## Fixed in batch 2 (fixer, 7 Nov)

The open music card (playerLayout overlay/side both use this simplified layout) now has:
- **Seek bar** (when the track has a length): 3 px (5 px hovered, 150 ms; Material 6 px) track with the played part, elapsed on the left, `-remaining` on the right (caption). A click or a drag seeks (MPRIS `Seek` by offset via busctl, the position then extrapolated). The position is read each second while an island is open with music (busctl `Position`), as IslandFocusCard's positionChanged timer.
- **Transport**: the volume pill on the left (speaker 36 px; pointed at it widens to 104 px over 150 ms OutCubic with a slider; click on the speaker mutes / unmutes back to the saved level (0.6 if none); the wheel over it ±5 %; the player's own Volume when it has one, else the system output via wpctl); prev / play (44 px) / next in the middle (every one, not the pill's pieces); "open the player" on the right (36 px; pointed at it widens to show the app's name, up to 140 px; a click raises the player (MPRIS Raise), focuses its window by class and closes panels).
- Not done: the Cast button (no Cast service state in standalone yet), art as the card background / edge fade (shader), Material's wavy seek bar (a straight bar), Material's morphing play button (plain icon), the larger "big number" layouts of timers/recording (they show title, detail and every control), captions card, the call's app icon.

## Test plan (Sonnet tester)

Setup: `python3 sandbox/tools/fakeplayer.py &` in the arena (3:30 track, answers Seek/SetPosition/Volume). One-at-a-time or all mode both show the music card.
1. Hover the island open: under the title/artist, a seek bar with "0:1x" left and "-3:1x" right; it advances each second (two shots 2 s apart).
2. Click at 50 % of the bar: within 1 s the elapsed time jumps to ≈1:45 (`dev run playerctl position` agrees). Drag from 25 % to 75 %: the time follows the drag end.
3. Point at the speaker (left of the transport): the pill widens to ~104 px over ~150 ms showing a slider. Click at its middle: `playerctl volume` ≈ 0.5. Scroll up one notch over it: +0.05. Click the speaker: muted (icon speaker-x, slider empty); click again: back to the previous level.
4. Prev / play / next: each click works first time; play is the wider middle button.
5. Point at the open-player button (right): it widens to show the player's name ("Fake Player"/its identity); click: the fake player's Raise is called (it prints/logs) and panels close.
6. Both looks.

## Test results

## Fixed in batch 3 (fixer, 7 Nov)

- **Material wavy seek bar** (WavySeek, WavyProgress's look): the played part a sine (wavelength 40, amplitude 3, stroke 4) moving right once a second while playing; flat and still when paused or under 10 % / over 95 % (amplitude eases over 300 ms); the rest a 4 px secondary-container track 4 px after it with a primary stop dot at its end. Click / drag seeks.
- **Material morphing play button** (PlayMorph): primary 44 px, a circle (radius 22) paused, a rounded square (radius 14) playing, on a spring (350 ms, bounce 0.35); 94 % pressed; state layer on hover/press.
- **Art behind the card** (Material): the album art, reduced to 24×24 in Rust (Slint's upscaling blurs it), fills the card's area under a 72 % wash of surfaceContainerHighest.
- **Glass, playerArt "island"** (the user's setting): the album art is the open island itself (0.82, fading in over Theme.popover), its lower 70 % faded out in Rust (edgefade's (0, 0, 0, 0.7)), under the black gradient and rim. Shown for music (one-at-a-time: when music is the shown activity; all: when there is music), not while an alert or Recent is in the island.
- Not done: the art's edge fade relative to the island's live height (it's baked into the picture's rows), Cast.

## Test plan additions

7. Material: with the fake player playing at ~30 %, the played part of the seek bar is a moving wave (record 2 s: it travels right, period ~1 s); pause: it lies flat within ~300 ms. At the start (< 10 %) and end (> 95 %) it's flat.
8. Material: the play button is a filled primary circle when paused and a rounded square when playing; clicking it morphs between them with a slight spring.
9. Material: the card shows the album art blurred under a tinted wash (the fake player needs an art file: `python3 sandbox/tools/fakeplayer.py /path/to/cover.jpg`).
10. Glass: open the island with music playing: the art fills the island, solid near the top, fading out towards the bottom rim; an alert replaces it with plain black.

## Test results (batch 3)

## Fixes after testing (fixer, 7 Nov, batch 3 build)

- Seek drag overshoot (batch 2 FAIL 4): a drag previews the position and sends one Seek on release (relative Seeks per move piled up).
- Glass overlay layout (batch 1 FAIL 8): the music card is now the overlay layout in both looks: a 40 px glyph centred above when there's no art (the art is the island / the card's background), the title 16/600, "artist · album", the seek row (elapsed | bar | -remaining, bar 4 px, 6 px hovered), then the transport.
- sandbox/tools/fakeplayer.py now answers Seek (relative, past the end: next track), SetPosition, Raise (logs `fakeplayer: Raise` to stderr) and emits Seeked; CanRaise true.

Retest the FAIL steps above.

## Fixed in batch 4 (fixer, 7 Nov)

- Glass, the art as the island: the fade now follows the island's live height (ten bands, solid over the top 30 %, to nothing at the rim), not baked into the picture's rows; the picture is no longer pre-faded.
- Long song titles scroll in the open card (Marquee) when `music.scroll` is on.

## Test plan (batch 4)

- Glass: as the island opens and closes, the art's fade stays relative to its height (no hard edge mid-way).

## Test results (batch 4)

## Re-check 17 Nov (B4)

Re-read against 18628a5: the music player had been finished after batch 1 (seek, volume, open-the-player, PlayMorph, WavySeek, Marquee, the Material wash) but the rows still said "missing". The other kinds' card was still the batch-1 stand-in (a small icon, title, the time, controls centred under it). Ported: QML's card for timer / stopwatch / recording / transfer / call / sharing / update (badge 44 on a cookie, caption over a 24 px number or title over detail, controls on the right, the progress bar or wavy line, the mic meter), the captions card, the stopwatch's hundredths, Cast, the side player, the faded art, clickable page dots and the swap. Before: 18 missing, 12 different, 2 same; after: 30 same, 2 different.

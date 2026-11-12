# shell/modules/menubar/IslandGlyph.qml

An alert's animated leading icon: springs in, then a short motion per kind
(ripples, ring fill, shake, waves, flip, heat, Wi-Fi arcs, bolt). Material
inline. Rust: the `glyph` in `AlertView`, `surfaces/menubar/ui/island.slint`
629-686 (the file's header says: "the alerts' glyphs pop in without their
ripples, shakes and flips"); tint and ring from `src/lib.rs` `alert_rows`
(431-436), arcs from `src/island.rs` `arc` (373).

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| play(): restart enter, ring, shake, bolt, heat, Wi-Fi per kind | 41-57 | island.slint:635-637 | different: one scale-in only |
| Tint: Glass the event's colour, Material IslandInk.tint (error if red, else primary) | 26 | lib.rs:431-432 | different: colour names aren't parsed (recorder.rs:259/365 "danger"/"orange", host island.rs charger "rgba(255,255,255,0.6)") and fall back to the accent |
| Ringed kinds (charging, battery, timer, temperature with a value) | 30 | lib.rs:434 | same |
| Art and device picture flags | 27-29 | lib.rs:446-447 | same |
| Ripples: 2 rings, 1.5 px tint, scale 1→1.9, opacity 0.7→0, 900 ms OutCubic, after 140 + 260·i ms; device / usb / display / custom / notification / VPN-hotspot-Ethernet | 39-40, 69-96 | — | missing |
| Disc: inset 6 px inside the ring (Material), radius 12 with art else round; colours by art / picture / Material / Glass 0.24 | 98-105 | island.slint:638-645 | same (circle, not squircle) |
| Art image (crop) | 106-114 | island.slint:646-651 | same |
| Glow on the disc (low battery: 0.35 260 ms / 0 380 ms, twice) | 115-128 | — | missing |
| Material: art cut in a cookie9 | 130-140 | island.slint:643 (radius 12) | different |
| Device picture (fit) | 141-148 | island.slint:653-658 | same |
| Icon: 0.34 / 0.42 / 0.48 of the size, filled, Material onTint | 149-159 | island.slint:659-664 | different: Material always on-primary (on-error expected on red alerts) |
| Wi-Fi icon steps none→low→medium→high | 154 | — | missing |
| Ring track: 3 px dim 0.14, r = size/2 − 2 | 175-181 | island.slint:665-674 | same |
| Ring arc: tint, 3 px round cap | 182-189 | island.slint:675-685 | same (static at its value) |
| Ring fill: 180 ms wait, 900 ms OutCubic; a timer runs down from full | 171-173, 254-259 | — | missing |
| Temperature ring warms orange → red as it fills (Glass) | 183-184 | island.slint:681 (tint) | different: static tint |
| Audio: three sound-wave arcs pulse beside the icon (×3, 180 / 360 ms, staggered 200 + 120·i) | 192-232 | — (island.slint:688 still leaves the 26 px gap) | missing (an empty gap) |
| Enter: pop 0.35→1 SpringAnimation spring 4 damping 0.22; icon turns in −90° (timer, focus) or −18° (OutBack overshoot 2, 520 ms); art flips 90°→0 (520 ms OutBack); USB drops 14 px and bounces (620 ms OutBounce) | 236-253 | island.slint:635-637 (scale 0.6→1, 420 ms spring(0.35), after `a.delay`) | different: a softer pop, no turn / flip / drop |
| Low battery shake: wait 250, ±3 px ×3 (50 / 90 / 50 ms) | 260-269 | — | missing |
| Heat: wait 900, glow 0.4 + icon 1.18 (380 OutSine), back (520 InOutSine), twice | 270-285 | — | missing |
| Wi-Fi joined: arcs fill 260 / 170 / 170 ms, then icon 1.15 and back | 287-299 | — | missing |
| Charging bolt pulses 1.25 twice (160 / 260 ms) after 400 ms | 301-310 | — | missing |

Not possible in Slint as is: none of these need shaders (all are transforms,
opacity and arcs); the header's "no shaders" doesn't cover them.

## Summary

1. Glyph colour wrong for alerts posted with colour names (recording alerts, charger removed): accent instead of red / grey.
2. Material icon on a red disc uses on-primary instead of on-error.
3. Ring doesn't fill / run down; temperature ring doesn't warm.
4. Audio alerts show an empty gap where the waves go.
5. No per-kind motion: ripples, shake + glow, heat swell, Wi-Fi arcs, bolt pulse, art flip, icon turn, USB drop.

## Fixed in batch 4 (fixer, 7 Nov)

- Colours named by services ("danger", "orange", rgba(…)) are read (batch 1).
- Material: an icon on a red (error) disc takes on-error (IslandInk.onTint).

## Test plan (batch 4)

- `ipc island demo battery` in Material: the red disc's icon is on-error (light on the error colour).

## Test results (batch 4)

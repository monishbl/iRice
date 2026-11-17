# shell/modules/menubar/IslandGlyph.qml

An alert's animated leading icon: springs in, then a short motion per kind
(ripples, ring fill, shake, waves, flip, heat, Wi-Fi arcs, bolt). Material
inline. Rust: the `glyph` in `AlertView`, `surfaces/menubar/ui/island.slint`
629-686 (the file's header says: "the alerts' glyphs pop in without their
ripples, shakes and flips"); tint and ring from `src/lib.rs` `alert_rows`
(431-436), arcs from `src/island.rs` `arc` (373).

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| play(): restart enter, ring, shake, bolt, heat, Wi-Fi per kind | 41-57 | island.slint AlertView `glyph`: its own clock (`t0` at init, `t` = ms after its stagger `a.delay`, stops at +3.2 s) drives every motion below | same (17 Nov) |
| Tint: Glass the event's colour, Material IslandInk.tint (error if red, else primary) | 26 | lib.rs `alert_rows` (`island::color` reads names and rgba(); `is_red` → error) | same (fixed batch 1: colour names and rgba() parsed) |
| Ringed kinds (charging, battery, timer, temperature with a value) | 30 | lib.rs `ringed` | same |
| Art and device picture flags | 27-29 | lib.rs `has_art` / `has_picture` | same |
| Ripples: 2 rings, 1.5 px tint, scale 1→1.9, opacity 0.7→0, 900 ms OutCubic, after 140 + 260·i ms; device / usb / display / custom / notification / VPN-hotspot-Ethernet | 39-40, 69-96 | island.slint glyph `for i in a.ripple ? 2 : 0`; lib.rs `ripple` (those kinds; network with `ripple` and not Wi-Fi) | same (17 Nov) |
| Disc: inset 6 px inside the ring (Material), radius 12 with art else round; colours by art / picture / Material / Glass 0.24 | 98-105 | island.slint glyph `disc` | same (circle, not squircle) |
| Art image (crop) | 106-114 | island.slint `disc` Image cover | same |
| Glow on the disc (low battery: 0.35 260 ms / 0 380 ms, twice) | 115-128 | island.slint `glyph.glow` (GlyphMath.blink) on a disc overlay | same (17 Nov) |
| Material: art cut in a cookie9 | 130-140 | island.slint `disc` (radius 12) | different: a rounded square (Slint can't clip a picture to a path) |
| Device picture (fit) | 141-148 | island.slint glyph picture Image contain | same |
| Icon: 0.34 / 0.42 / 0.48 of the size, filled, Material onTint | 149-159 | island.slint glyph SymIcon (on-error when the tint is error, else on-primary) | same (fixed after batch 1) |
| Wi-Fi icon steps none→low→medium→high | 154 | island.slint `wifi-step` (glyphs wifi-none/low/medium/high); lib.rs `wifi` (network + wifi-high) | same (17 Nov) |
| Ring track: 3 px dim 0.14, r = size/2 − 2 | 175-181 | island.slint glyph track Path | same |
| Ring arc: tint, 3 px round cap | 182-189 | island.slint glyph arc Path, `GlyphMath.arc(sweep)` | same |
| Ring fill: 180 ms wait, 900 ms OutCubic; a timer runs down from full | 171-173, 254-259 | island.slint `ring-t`, `sweep` (timer: 1 − (1 − v)·t) | same (17 Nov; the arc built in Slint each frame, was Rust's static arc) |
| Temperature ring warms orange → red as it fills (Glass) | 183-184 | island.slint arc stroke rgb(255, 255·(0.62 − 0.35·t), 26) | same (17 Nov) |
| Audio: three sound-wave arcs pulse beside the icon (×3, 180 / 360 ms, staggered 200 + 120·i) | 192-232 | island.slint `a.kind == "audio"` waves (arcs ±40°, GlyphMath.wave) in the 26 px gap | same (17 Nov) |
| Enter: pop 0.35→1 SpringAnimation spring 4 damping 0.22; icon turns in −90° (timer, focus) or −18° (OutBack overshoot 2, 520 ms); art flips 90°→0 (520 ms OutBack); USB drops 14 px and bounces (620 ms OutBounce) | 236-253 | island.slint `pop` (damped cosine 0.35→1, ~380 ms period), `turn`, `flip` (transform-scale-x = the flip's cosine), `drop` (OutBounce) | same (17 Nov; the spring as a damped curve, the flip as a horizontal squeeze) |
| Low battery shake: wait 250, ±3 px ×3 (50 / 90 / 50 ms) | 260-269 | island.slint `shake-x` (GlyphMath.shake) | same (17 Nov) |
| Heat: wait 900, glow 0.4 + icon 1.18 (380 OutSine), back (520 InOutSine), twice | 270-285 | island.slint `glow` / `grow` (GlyphMath.swell) | same (17 Nov) |
| Wi-Fi joined: arcs fill 260 / 170 / 170 ms, then icon 1.15 and back | 287-299 | island.slint `wifi-step`, `grow` (pulse 140 / 220) | same (17 Nov) |
| Charging bolt pulses 1.25 twice (160 / 260 ms) after 400 ms | 301-310 | island.slint `grow` (GlyphMath.pulse 160 / 260 ×2) | same (17 Nov) |

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

## Re-check 17 Nov (B4)

The glyph popped in and nothing else moved. Ported every motion as a function of the glyph's own clock (a `GlyphMath` global: OutCubic, OutBack, OutBounce, the shake, blink, pulse, swell and wave curves, the ring's arc), so stacked alerts still start after their stagger and everything holds still once done (the clock stops at +3.2 s: no idle frames). Reduce motion: no pop, turn, flip, drop, ripples, shake or swell; the ring still fills. Before: 9 missing, 6 different, 7 same; after: 21 same, 1 different (Material's cookie-cut art).

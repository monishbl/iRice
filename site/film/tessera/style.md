# Style and motion

## Look
- **Canvas:** near-black #0B0B0E behind everything, so footage, photos and type carry the colour. The photos are the project's own wallpapers for the film: Unsplash nature photography, credited in capture/seed/photos/CREDITS.md.
- **One accent:** warm amber #FFB45C, the Badge's colour. It's used on the accent word of each title, the seam's glow (A) and the lit tiles (B). Every other colour comes from the footage itself: Glass's translucency and Material's tonal palettes.
- **Two colour worlds, each with one meaning:**
  - **Glass:** frosted white over the photo, a rim of light.
  - **Material:** tonal surfaces from the alpine lake photo (deep green #1F3B2C, sage #A8CFB2, cream #E9F0E6).

  In A they're the two halves; in B they're the two kinds of tile.
- **Type:** Google Sans Flex (OFL, from the project), one family.
  - titles: 92 px weight 500, tracking −1.5%;
  - the name: 150 px weight 600, tracking −3%;
  - chapter cards (B): 120 px weight 600.

  Nothing to be read is under 48 px. Sentence case, one line per screen.
- **The Badge:** Phosphor `circle-wavy`, filled, amber on dark, from the font the shell uses (Phosphor-Fill.ttf, U+E604).
- **Footage:** always on its own look's real desktop, a 2× still of the same state underneath (the plate), with the moving region recorded live on top at the exact place it came from.

## Motion

| | A "Two halves" | B "Tiles" |
|---|---|---|
| Family | calm and precise: no overshoot except the Badge's one pop | snappy: a 120 bpm grid (0.5 s), cuts on beats |
| Entrances | `o` (out-cubic), 0.4–0.6 s | `expo`, 0.25–0.35 s |
| Exits | `i`, a third faster | `i3`, 0.2 s |
| Camera | hold-drift 2–3 % on every hold; pushes 0.5–2 s, `io5` | punch-ins on the beat (`expo`, 0.3 s), whips 0.3 s with motion blur |
| Full-frame moves | seam sweeps and halves sliding, 1.0–1.2 s (`io5`) | tile shatters, 1.0 s (0.5 s in, 0.5 s out, staggered by distance from the centre) |
| Type | titleIn: words rise out of blur, accent word last | slam-in: scale 1.3 → 1 with blur 12 → 0 on a beat, out on the next |
| The product's own motion | from the footage at 1×: springs, morphs, the island's alerts | the same; only the dull waits are sped up (`remap`) |

**Continuity agents:**
- A: the seam, a 2 px vertical line of amber-white light with a soft glow, carrying the Badge.
- B: tesserae, 64 px tiles with 14 px corners, half frosted glass and half tonal.

**Loops:** A begins and ends on the two halves drifting apart in the dark; B begins and ends on drifting tiles.

**Playback:** the HTML plays the clips as `<video>` kept in step with the film's clock (re-synced when drift exceeds 0.1 s, seeked exactly when scrubbing).

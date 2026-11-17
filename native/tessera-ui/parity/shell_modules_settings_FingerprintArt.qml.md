# shell/modules/settings/FingerprintArt.qml ↔ surfaces/settings (src/pages/system/art.rs, ui/settings-system.slint `FpSheet` art)

_Re-checked 17 Nov (B5, parity-settings) against main 18628a5 + the B5 ports: statuses below say what's there now; [12 Nov: …] is the old status._

The print in the add-a-finger sheet: ridges as dashes, cut into 11 sections,
coloured taken / next / faint / all green when done; animations on each
touch. Rust: the same geometry as SVG path commands per section (made once),
each section a Path stroked in its colour; misses/takes counters drive the
shake/bump.

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | Geometry: 6 tilted hairpins (gap 13, tilt 24°, legs) + 3 outer ridges, fitted into the box with stroke margin 7; section by radius (0.27 / 0.6) and angle; dashes [62,30,48,22,70,36,54], gap 11, core two unbroken (312–389) | art.rs 17–128 | same (section index clamped ≤ 10) |
| 2 | Round caps, stroke 7 (399–400) | FpSheet 124–125 | same |
| 3 | Colours: done → success; taken → accent; target → mix(faint, soft=accent·0.42, glow), flash → mix toward danger; else faint (dark white 16 % / light black 13 %) (401–415) | FpSheet 63, 126 | same |
| 4 | Glow-in on a new target: glow 0.2→1, 420 ms OutCubic (260–262, 279; Sheet 104–110) | `animate stroke 420ms ease-out` (faint → soft) | same at a glance |
| 5 | Fill-in of the section just taken: soft→accent 380 ms OutCubic (251–255, 280) | stroke animate 420 ms ease-out | **same (ported 17 Nov, B5: a section taken fills in over 380 ms OutCubic (the stroke's timing followed the wrong state))** [12 Nov: different (small): 420 not 380 ms] |
| 6 | Miss flash: flash →1 in 60 ms, hold 450, →0 over **700 ms InCubic** (256–259, 281–286) | bridge flash true 510 ms (RustBridgeSystem.qml 71); stroke animate 60 ms to red, back over **420 ms ease-out** | **same (ported 17 Nov, B5: the miss's red fades back over 700 ms InCubic)** [12 Nov: **different**: fade back 420 ms ease-out instead of 700 ms ease-in] |
| 7 | Bump on a take: canvas scale →0.96 80 ms OutCubic, →1 260 ms OutBack (287–291) | FpSheet 46–60 | same |
| 8 | Shake on a miss: x −8 (50) → 7 (70) → −5 (60) → 3 (50) → 0 (40) ms (292–299) | FpSheet 29–45: 55 ms steps, each animated 50 ms | same at a glance |
| 9 | Done badge: 44 px success circle at (w−52, h−58), 3 px border in the sheet colour, white bold check 22; scale 0.4→1.1 (260 OutBack) →1 (140) (300–304, 437–455) | FpSheet 131–146: 0.4→1 260 ms ease-out-back, opacity 0→1 at once | same at a glance |
| 10 | Durations × CoolDown.motionScale (279–303) | fixed ms | **intentional: no Cool-down motion scale anywhere in tessera-ui (a global, not a Settings gap)** [12 Nov: different (small): ignores Cool-down's motion scale] |
| 11 | Repaint on dark/light change (274–277) | bindings | same |

## Test plan

Needs a fingerprint reader and touches on it — testers must **not** enrol.
Code review only; if the user runs an enrolment himself, record it in both
shells (`clicks` not needed: a 60 fps recording) and compare the miss flash:
QML fades back over ~700 ms slowly then fast; Rust over ~420 ms. Also
watch the badge pop at the end (QML overshoots to 1.1 then settles).

## Fixed in batch 3 (fixer)

- A section taken: 380 ms; a miss flashes, then fades back over 700 ms (ease-in).

## Test results

(tester)

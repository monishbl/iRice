# Parity: shell/modules/lock/MaterialClock.qml

Android 16's big clock: hours over minutes, two tabular digits each, placed
by their ink; the minute tick.
Rust: `surfaces/lock/ui/lock.slint` `StackClock` (used by `MClock` stacked,
Bedside's Material clock and the Material morph).

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Hours / minutes padded to two digits (24 h setting) | 335-336 | `LockData.hh2`, `mm` | same |
| Lines by their digits: capTop 0.25 em, digits 0.716 em, gap 0.085 em; size = ink | 338-346 | `digits`, `gap`, `Ink` | same |
| align left / centre between the lines | 330, 365, 377 | `align-left` | same |
| Weight in steps of 40 (an animated weight makes few instances) | 325-327, 348-351 | `round(wght / 40) * 40` | same |
| Width / roundness / opsz axes | 348-351 | — | different: Slint sets weight only |
| Minute tick (ticks on, visible): minutes' weight −240 over 180 ms OutCubic, back over 420 ms OutBack | 353-359, 383 | `dip` from `animation-tick()`: out-cubic 0–180 ms, 1 − out-back 180–600 ms | **fixed** (was: thin for 600 ms, then back in 180 ms, no overshoot) |
| Colours: hours color, minutes color2 | 367, 379 | `ink`, `ink2` | same |

## Test plan (arena N, Material, lock.clockFace "stacked")

1. Lock; wait for a minute change; record 1.5 s around it (`wf-recorder` started a few seconds before :00).
   PASS: the minutes go visibly thinner within ~180 ms, then swell back over ~420 ms with a slight over-bold before settling (OutBack); the hours never change weight. Compare frame by frame with the QML recording (`python3 tools/region-changes.py rec.mp4 x y w h` over the minutes: change spans ≈ 600 ms in both).
2. In StandBy (`demo standby`) at a minute change: no tick (ticks off).

## Test results

(testers fill in)

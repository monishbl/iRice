# Parity: shell/modules/lock/LockClock.qml

The Material lock clock in Android 17's faces (stacked, dynamic, wide,
overlap, analog, weather), large ↔ small cross-over (`small` 0..1).
Rust: `surfaces/lock/ui/lock.slint` `MClock`, `Ink`, `StackClock`, `Dial`.

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Size from face: implicit sizes per face; cross-over w/h mix by k | 46-51 | `lw`, `lh`, `sw`, `sh`, width/height | same |
| Large form: fades out (1 − 1.7k), scales toward the small size, top-left origin | 53-63 | `large` | same |
| Small form: fades in from k 0.35, scales from 1/ratio, top-left origin | 64-75 | small `Rectangle` | same |
| Ink: digits placed by their ink (0.716 em tall, 0.25 em down) | 84-104 | `Ink` | same |
| Weight in steps of 40; width and roundness axes, opsz | 39-44 | `round(wght / 40) * 40` | different: Slint sets weight only (no wdth / ROND / opsz) |
| stacked → MaterialClock | 106-121 | `StackClock` | same (see MaterialClock) |
| dynamic: hours 1.6× tall, minutes 0.95× beside their top, AM/PM (or "d MMM" with 24 h) under the minutes | 123-158 | dynamic block, `LockData.half` | same |
| wide: one line 0.8×, as wide as the font goes | 160-167 | `wide` | different: no width axis (as wide as the default width) |
| overlap: heavy (≥760) pairs, minutes over the hours' corner, colour3 | 169-193 | overlap block | same |
| analog: 12-scallop cookie dial (colour3 20 %), hour hand thick 0.085 d × 0.27 d, minute hand 0.05 d × 0.38 d, centre dot (darker 1.6) | 195-241 | `Dial` | same (centre dot now darker(0.6) = Qt.darker 1.6) |
| Hands turn **clockwise** with OutBack 450 ms (354° → 0° is 6° on) | 254-260 | `Dial` `animate transform-rotation` 450 ms ease-out-back; Rust `forward()` keeps the angles growing | **fixed** (at :59 → :00 the minute hand spun 354° backwards) |
| Small analog: the same dial **with its hands**, 1.1 × small size | 201-206 | small form `Dial` | **fixed** (was the dial without hands) |
| weather: time 0.78× over the weather icon (0.42×) and temperature (0.5×) | 264-296 | weather block, `WxIcon` | same |
| small line: one line hh:mm at small size (overlap ≥760) | 298-305 | `small-line` | same |
| 24-hour setting (TimeFormat.hour) | 37 | `logic::hour(h, h24)` | same |

## Test plan (arena N, Material)

For each `lock.clockFace`: `ipc lock lock`, shot, compare with the QML shot at the same minute (`--only qml`).
1. **analog**: hands at the right time; at a minute change record 1 s: the minute hand steps **forward** 6° with a small overshoot (450 ms). At :59 → :00 (wait for it, or set the arena's clock if possible): forward, not a full backward spin.
2. **small analog**: `lock.clockSize: "small"` with analog → the small dial in the corner **has both hands**.
3. **dynamic** with `appearance.clock24` true/false: "AM"/"PM" vs "7 Nov" under the minutes; the date line above hides for 24 h while large.
4. **Cross-over**: switch `lock.clockSize` small ↔ large while locked (edit the config): the large face shrinks and fades while the line grows in, both pinned top-left; no jump at the end.

## Test results

(testers fill in)

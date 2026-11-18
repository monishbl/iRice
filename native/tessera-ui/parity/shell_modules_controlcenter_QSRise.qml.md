# shell/modules/controlcenter/QSRise.qml

How far a Quick Settings piece has risen as the panel opens (with
QuickSettings.qml's `stagger`, 60-88). Rust: `stagger` / `rising` on the
`ControlCenter` window and each tile's `due` / `rise`
(`surfaces/controlcenter/ui/controlcenter.slint`, ~2034 and ~2440, footer ~2560).

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| stagger 0→1 over 360 ms when the panel opens (QuickSettings 70-82) | QS 70-82 | `rising` → `stagger`, `animate 360ms` | same |
| A piece is due once stagger·24 ≥ order+1 (or stagger 1) | 16 | `due` | same |
| Due: rises on springFast with damping 0.38 (overshoot) | 28-35 | `animate rise { spring(0.58) 222ms }` | different engine; tester compares the overshoot |
| Not due: back to 0 at once | 20-26 | `animate rise { duration: due ? 222ms : 0ms }` | **fixed** (before, it sank on the spring) |
| Closing: stagger is NOT reset (only on the next open); the pieces stay in place while the panel fades | QS 79-87 | `rising` stays true until `openness` reaches 0 | **fixed**: before, `stagger: shown ? 1 : 0` dropped to 0 on close, so every tile and the footer faded and shrank on its own spring during the close (one layer per tile per frame: the close stuttered) |
| Made after the panel opened (a tile added): already risen | 19 | stagger is 1: `due` true at creation | same |
| The tiles are kept between opens (Loader kept) | QS | lib.rs `open()` no longer empties the model | **fixed**: before every open made all tiles again |
| scale 0.86+0.14·rise, opacity min(1, rise) | 9-10 | tile `transform-scale`, `opacity` | same |

## Tester plan (Material)

1. Open the Control Center with `ipc panels open controlCenter TESSERA-TEST`,
   recording at 60 fps. In frames: tiles appear in order (top-left first),
   each overshooting slightly. Compare to the QML recording: first tile
   starts, last tile settles (`tools/anim-timing.py`), within ±1 frame of
   the QML's.
2. Close it (`ipc panels close`), recording. **Every tile stays at full size
   while the panel fades out (154 ms)**; no tile shrinks on its own. Longest
   frame gap during the close (`anim-timing.py`: "longest gap") ≤ 20 ms
   (60 fps) — record it for both shells; Rust must not be worse than QML.
3. Open again: the tiles rise in again in order (not all at once, not
   already in place).
4. Open, then close and re-open within 100 ms: no stuck invisible tiles.
5. Open, change the layout (Edit controls → add a control) → the new tile is
   there at once (no rise).

## Test results

(tester)

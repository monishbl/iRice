# shell/modules/controlcenter/QSSlider.qml

Material Quick Settings' brightness and volume (and CCSlider's medium size
in pages): the Expressive slider. Rust: `QsSlider` in
`surfaces/controlcenter/ui/controlcenter.slint` (650-780), used by the
Material grid (`if !plain && m.kind == 2`, with its page button) and by
`Block` kind 4 (page sliders); `moved` → `ControlCenter::slide` /
`block_slide` (`src/lib.rs`).

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| Track 52 (medium 40), handle stands out | 122, 129 | `track` | same |
| `shown` follows the value, OutCubic 150 ms, straight while pressed | 160-166 | `animate shown` | same |
| Corners square off to 8 while pressed, spring back (springPress) | 171-175 | `animate corner { spring(0.5) 290ms }` | different curve engine (Slint spring ≠ Qt SpringAnimation); tester to compare the release spring |
| Handle 4 px, 2 px while pressed/focused, 100 ms | 177-178 | `handle` | same (focus thinning missing: Rust focus is the grid's ring) |
| Done part primary / rest secondaryContainer, 2 px corners at the gap, gap 6 | 182-214 | slint Rectangles | same |
| Stop dot 4 px onSecondaryContainer at the far end | 206-213 | slint | same |
| Icon inside the done part while it fits, else past the handle; colour glides | 225-241 | slint `Ic` | same |
| Standing (1×2): rises from the bottom | (QuickSettings rotates it) | `vertical` | same |
| Press: value from x; drag follows; >6 px cancels hold; hold 550 ms → level back + page | 243-288 | `touch` + `hold` | same |
| Right-click → page | 256-259 | `pointer-event` right | same |
| Wheel ±5% | 278 | `scroll-event` | same |
| Keys: arrows ±5%, PgUp/PgDn ±20%, Home/End, Enter → page | 133-149 | lib.rs `key()` | same; **fixed**: steps accumulate |
| The level under the pointer shows at once | (Audio sets at once) | `held` (live.rs) | **fixed** |
| Page slider rows survive refreshes (mic, app volumes) | SoundPage ScriptModel | `live::set_blocks` | **fixed** (a new model per reading ended the drag) |

## Tester plan

Material (the user's look): `--look material`. Open the Control Center
(Quick Settings). Sliders are the 52 px tall bars (Display, Sound).

1. Click at 30% → the done part reaches 30% at once; `wpctl get-volume` ≈ 0.30.
2. Press, glide 20%→90% over 600 ms, release; record. The handle follows the
   pointer with no backward jump; corners square to 8 px while held and
   spring back round on release (compare to QML's recording: settle time).
3. Scroll ±3 notches: ±15%.
4. Press and hold 1 s: level restored, the Sound page slides in.
5. Right-click: the Sound page.
6. Arrow keys: Tab to it, Right ×5 in one wtype: +25%.
7. Sound page (Material: inside the panel): drag the microphone slider and an
   app's slider end to end: the drag holds to the end, the value follows.
8. Standing slider: Edit controls → resize Sound to 1×2 (click it while
   editing until 1×2), Done; drag bottom→top: it rises with the pointer.

## Test results

(tester)

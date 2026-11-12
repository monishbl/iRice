# shell/modules/controlcenter/CCSlider.qml

The slider row inside pages (Sound page: app volumes; Microphone page and
section: input level) and the status Sound menu's volume. Glass: a 26 px
track filling white with the icon at the left; Material: QSSlider at the
medium size (track 40). Rust: `Block` kind 4 in
`surfaces/controlcenter/ui/controlcenter.slint` (Glass `sl` TouchArea,
Material `QsSlider`), callbacks `block-slide` → `block_slide`
(`src/lib.rs`) and, in the status menus, `volume` / `sound-slide`
(`surfaces/statusmenus/src/lib.rs`).

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| Caret-right (hasMore) opens more | 19-31 | — | not used by any Rust page (no hasMore slider in pages) |
| Optional label above (DemiBold body) | 33-39 | — | not used by the ported pages |
| Glass: track 26 high, toggleOff, fill ≥ height, white, icon 14 at x 8 (black 60%) | 42-73 | Block kind 4 Glass | same |
| Fill width glides 150 ms except while pressed | 60-63 | `animate width` | same |
| Press: value = x / width; drag follows | 79-84 | `sl` pointer-event / moved | same |
| Wheel ±5% | 85 | `sl` scroll-event | same |
| Material: QSSlider, track 40, height 48 | 91-104 | `QsSlider { track: 40px }` | same |
| A drag survives the sound server's readings | (ScriptModel keeps rows) | `live::set_blocks` (Rows, keyed) | **fixed**: before, each reading handed the page a new model; the slider was remade and the drag ended after one step |
| The value follows the pointer at once | (PwNode volume set at once) | `held` "block:<id>" | **fixed** |

## Tester plan

Both looks. Something playing (see CapsuleSlider's plan, step 8).

1. Control Center → Sound page (right-click the Sound slider). Drag the
   app's slider from 10% to 90% over 800 ms (`down`, `glide`, `up`),
   recording. Expect: the fill follows the whole way; `pactl list
   sink-inputs | grep -i volume` ≈ 90% after.
2. The microphone slider (in the Sound page, "micInSound" on by default):
   same drag; `wpctl get-volume @DEFAULT_AUDIO_SOURCE@`.
3. Wheel on each: ±5% per notch.
4. Status menu: click the menu bar's sound icon (Sound menu); drag its volume
   slider 10%→90%, and an app's slider inside it: both follow to the end.
5. While dragging (step 1), start a second player: the dragged row keeps its
   pointer (it may move down a row but the drag continues).

## Test results

(tester)

# shell/modules/controlcenter/CapsuleSlider.qml

Glass Control Center's brightness and volume modules (Display 4×1 / 1×2,
Sound 4×1 / 1×2, Mic level): the whole module is the slider, a capsule
filling with white. Rust: `CapsuleSlider` in
`surfaces/controlcenter/ui/controlcenter.slint` (190-280), used by
`GlassModule` (`m.kind == 2`); its `moved` reaches
`ControlCenter::slide` (`surfaces/controlcenter/src/lib.rs`), which sets the
level (core `services::audio` / `services::brightness`) and hushes the OSD.

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| Vertical when taller than wide; thick = the short side | 15, 49 | slint `vertical`, `thick` | same |
| Track: Squircle toggleOff, round ends (thick/2) | 55-60 | slint first Rectangle | same (rounded rect, not a squircle) |
| Fill: white (dark: 92% white), never shorter than thick, from left / bottom | 61-70 | slint `fill` | same |
| Fill length glides 150 ms (Theme.motion.hover) except while pressed | 71-74 | `animate len { duration: touch.pressed ? 0 : 150ms }` | same |
| Icon: inset (thick−size)/2, size min(22, thick·0.36), filled, accentOnGlyph | 78-86 | slint `Ic` | same |
| Left press: value from the pointer (x/w, or 1−y/h standing) | 96-112 | `pointer-event` down → `moved(at())` | same |
| Drag: follows the pointer while left held | 116-122 | `moved =>` | same |
| Drag > 6 px stops the hold timer | 119-120 | `moved =>` | same |
| Press and hold 550 ms (has a page): put the level back, open the page | 124-132 | `hold` Timer | same |
| Right-click: its page (has a page) | 101-105 | `pointer-event` right | same |
| Wheel: ±5% per notch | 123 | `scroll-event` | same |
| Keyboard: arrows ±5%, PgUp/PgDn ±20%, Home/End; Enter → page | 20-38 | lib.rs `key()` (a slider takes the arrows) | same; **fixed**: steps go on from the level just set (a held key no longer repeats the same step while the server answers) |
| preventStealing; Panels.holdPull (the panel holds still) | 99, 114 | n/a (no pull gesture in Rust) | n/a |
| The level shown follows the pointer at once | (Audio.volume set at once) | lib.rs `slide` → `held` + `show_level` | **fixed**: before, the capsule waited for the sound server's answer and jumped back a step under the pointer during a drag |
| Accessible slider, increase/decrease ±5% | 39-43 | slint accessible-* | same |
| FocusRing | 44-46 | GlassModule kbd-focus ring | same |

## Fixed in this batch

- The level you set shows at once and is kept 600 ms over the readings that
  follow (`live::Held`), so a drag doesn't flicker back to older levels.
- Keyboard steps accumulate from the level just set.
- The Sound page's sliders (app volumes, microphone) no longer lose the drag
  after one step: the page's rows are changed in place (`live::set_blocks`,
  Rows keyed by id) instead of handing the page a new model on every reading.

## Tester plan

Arena with the user's settings, Glass: `tools/live-scenarios.sh --arena N --look glass --only rust --bin <worktree>/native/tessera-ui/target/release/tessera-ui`
(copy it; add the steps). Open the Control Center: `ipc panels open controlCenter TESSERA-TEST`.
Find the Sound slider (default layout: the row under the toggles; take a
`dev shot` and read its rectangle). Volume readings: `dev run wpctl get-volume @DEFAULT_AUDIO_SINK@`.

1. Click at 25% of its width (`pointer move X Y`, `click`). Expect the fill
   at 25% within one frame and `wpctl` ≈ 0.25. PASS if both.
2. Press at 20%, glide to 80% over 600 ms (`down`, `glide X2 Y 600`, `up`),
   recording (`wf-recorder`). Expect the fill to follow the pointer with no
   backward jumps (step through frames: the fill edge never moves left while
   the pointer moves right); `wpctl` ≈ 0.80 after.
3. Scroll up 3 notches over it (`scroll` ×3 up): +15%. Down 3: back.
4. Press and hold 1 s without moving: the level returns to what it was
   before the press and the Sound page opens beside the panel (Glass popover).
5. Right-click: the Sound page opens.
6. Tab to the slider, press Right 5 times quickly (`wtype -k Right` ×5 in one
   command): +25% (not +5%).
7. Same for Display (brightness: the arena's backlight is in memory only;
   read the slider) and Mic level when present.
8. On the Sound page (Glass popover): drag an app's volume slider and the
   microphone slider 20%→80%: the drag must continue to the end (before:
   it stopped after the first step). Needs an app playing: `dev run pw-play
   --volume 0.1 /usr/share/sounds/freedesktop/stereo/complete.oga` in a loop, or
   `paplay` of a long file.
9. QML: run the same steps with `--only qml` and compare the recordings
   (`tools/anim-timing.py`): the fill's glide after a click is 150 ms.

## Test results

(tester)

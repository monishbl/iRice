# shell/modules/settings/SettingsButton.qml ↔ settings-kit.slint SButton (691–781), SFocusRing (192–206); settings.slint Control "button" / "buttons"

Text button for Settings pages: neutral, primary (accent), danger, minor (Material text button), busy.

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | Size: label+32 (minor +24) × 40 Material; label+24 × 28 Glass (25–26) | SButton width/height | same |
| 2 | Material colours: primary / errorContainer / transparent (minor) / secondaryContainer, content to match; disabled onSurface 12 % / 38 % (22–23) | `m3-container`, `m3-content` | same |
| 3 | Material state layer 8 % hover / 10 % press over the container (minor: content on nothing) (24, 38) | `bg.background` Material, `layer` | same |
| 4 | Glass colours: primary accent darker 1.1 hover / 1.2 press; others fill / fillHover / fillPressed; ColorAnimation 150 ms (39, 42) | `bg.background` Glass, `animate background SM.hover` | same |
| 5 | Glass press: scale 0.97, Theme.motion.press 100 ms (40–41) | bg width/height ×0.97, `animate width, height { SM.press }` | same |
| 6 | Material press: corners square off to 8 while held, SpringAnimation springFast (12.8 / 0.54) (30–34) | settings-kit.slint:729 `animate border-radius { 300ms; spring(0.6) }` | same at a glance (tester: time against the QML) |
| 7 | Material press: ElasticControl swell (~6 %, ≤10 px, springPress 7.5 / 0.3: overshoots on release) (28–29; ElasticControl.qml 31–37; Theme.motion.pressGrow) | none (Material bg stays 1.0) | **missing** |
| 8 | Disabled: Glass 0.45 opacity, Material colours only; no hover, no cursor (35, 113) | `opacity`, `ta.enabled: active` | same |
| 9 | Keys Space / Return / Enter click (not auto-repeat), Tab-focusable (36, 44–49) | `fs` key-pressed (`!e.repeat`) | same |
| 10 | Accessible: button, its text, its row's label as description, press (50–60) | accessible-* (`label`) | same |
| 11 | Click: takes active focus, then clicked() (115–118) | `fs.focus(); fs.clear-focus(); clicked()` | different (focus is dropped; harmless for the ring, which shows for keyboard focus only, but Tab then starts from the window, not this button) |
| 12 | Focus ring: keyboard focus only; Glass 2 px accent 4 px out (2 px gap), Material 3 px secondary 5 px out; radius control+4 (62–64; FocusRing.qml 22, 44) | SFocusRing: always 3 px out, 2 px wide, radius +3 | **different** (gap 1 px, Material ring 2 px not 3) |
| 13 | Icon before label: 18 Material / 12 bold Glass, gap 8 / 6 (70, 74–81) | `row` SIcon, spacing | same |
| 14 | Busy Glass: circle-notch spinning 900 ms linear, label stays (76–89) | `SG.notch`, `animation-tick` 900 ms | same |
| 15 | Busy Material: an 18 px Spinner in the icon's place (91–99) | `Busy { transform-scale: 0.45 }` in an 18 px box | same at a glance (tester: compare size) |
| 16 | Label: 14 px 500 0.1 tracking Material; small, 600 primary / 500 others Glass; onAccent / danger / label (100–106) | `row` Text | same |
| 17 | Pointing-hand cursor (114) | `mouse-cursor: pointer` | same |

## Test plan

B1 `open about` (Material), find a neutral and a primary button in `shot b1`; Glass the same.
B2 Press and hold (Material): `run 'P move <x> <y>; sleep 0.3; P down; sleep 0.6; P up'` while recording a REGION round the button (QML and Rust). PASS: the button swells ~6 % and squares to 8 px while held, springs back past its size on release (QML ElasticControl); time with `tools/anim-timing.py`.
B3 Glass press: same. PASS: shrinks to 0.97 in ~100 ms, back on release.
B4 Hover: glide on/off. PASS: colour fades ~150 ms.
B5 Keyboard: Tab to a button. PASS: focus ring 2 px clear of the button (Glass 2 px accent; Material 3 px secondary), as the QML; Space presses it (only a harmless button, e.g. "Check for updates").
B6 Busy: a button that runs something (Update → Check for updates). PASS: spinner (Material) / spinning notch (Glass) beside the label.

## Fixed in batch 3 (fixer)

- Material buttons swell as they're pressed (up to 6 %) and spring back past their size.
- Not done: keeping focus after a click (the ring would then show after a mouse click: Slint has no focus reason).

## Test results

(tester)

# shell/modules/settings/WordSlider.qml ↔ settings-kit.slint SWordSlider (429–457); settings.slint Control slider with `r.low`/`r.high`; model.rs Row::words

A Settings slider with a word under each end ("Slow" … "Fast").

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | Slider (common Slider.qml) on top, from / to / value, moved(v) passed on (9–22) | SWordSlider → SSlider, `moved` → act i 0 (and `released` → i 1, extra) | same |
| 2 | Width 240 by default (16) | Row::slider defaults to 220; every WordSlider row sets `.width(240.0)` (mouse.rs:37, 49, 111, 137, 159) | same where used (the other `.words()` rows, menubar.rs:290 and notifications.rs:246, are page-specified 220: check their QML) |
| 3 | 2 px between the slider and the words (17) | `spacing: 2px` | same |
| 4 | Low word left, high word right, tertiary caption (23–28) | HorizontalLayout with stretch | same |

## Test plan

W1 `open mouse` (Material, Glass, both shells), `shot w1-…`. PASS: the Tracking/Scroll speed sliders are 240 wide, words "Slow"/"Fast" 2 px under the ends, as the QML.
W2 Drag the Tracking speed knob a little and back (`run 'P move <knob>; P down; P move <knob+30> glide …; P up'`). PASS: the value follows; restore it to where it was.

## Test results

(tester)

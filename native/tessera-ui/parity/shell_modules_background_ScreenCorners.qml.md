# shell/modules/background/ScreenCorners.qml → surfaces/screenfx/src/corners.rs (+ ui/corners.slint)

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Only when `appearance.screenCorners` (default on) | 9 | `setting()`; `config_changed` rebuilds on change | same |
| One strip per screen, overlay layer, bottom/left/right anchored, height = `screenCornerRadius` (12) | 12-21 | `make`: Layer::Overlay, Anchor BOTTOM\|LEFT\|RIGHT, size (0, r) | same |
| Never takes input (empty mask) | 23 | `set_input_region(Some(&[]))` | same |
| Black concave corner at each bottom end (Corner.qml arc, flipped) | 25-26, common/Corner.qml | corners.slint two Paths: arc centred (r,0) / (0,0) of the strip, i.e. the same quarter-circle cut | same (checked the geometry: identical shape) |
| Radius change / toggle: windows made again | Variants model | `config_changed` → `rebuild` only when (on, r) changed | same |
| surface_closed scoped to its own windows | — | `Rc::ptr_eq` check | same |
| Only while the Rust shell is standalone (QML draws them otherwise) | — | `super::on()` = host::standalone() | same |

No animation, hover, keys or live data in this file. Nothing to fix.

## Test plan (Sonnet tester, arena 5, 1920x1080 scale 1; standalone Rust vs QML)

1. `./dev.sh shot`; crop the bottom-left and bottom-right 24x24 px: a black concave corner of radius 12 at each (black outside a quarter circle centred 12 px in from the corner), identical in both shells (pixel diff of the crops ≈ 0).
2. Set `appearance.screenCornerRadius` to 24 in config.json: within a second the corners are 24 px; set `appearance.screenCorners` false: they go; true: back.
3. Move the pointer to 2,1078 and click: the click reaches what is under (the strip takes no input; e.g. the dock/desktop right-click menu at 5,1075 still opens on right-click).

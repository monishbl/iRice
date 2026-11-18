# shell/modules/controlcenter/ModuleView.qml

Glass module. Rust: `GlassModule` (surfaces/controlcenter/ui/controlcenter.slint ~281-650), surfaces/controlcenter/src/lib.rs.

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| dim when a page is open, 220 ms | 25-36 | dim | different: deeper fade, no blur (Slint) |
| focus lift 1.04, press squish | 37-38, 422-426 | transform-scale | different: fixed 0.95, no follow |
| Space/Enter/arrows | 73-99 | surfaces/controlcenter/src/lib.rs key | same |
| Menu key / Shift+F10 → page | 85-87 | surfaces/controlcenter/src/lib.rs key | fixed: opens the page level with the module (menu at 0,0 before); Shift+F10 added |
| left click: menu / page / act; editing: resize | 377-395 | bg TouchArea | same |
| right click → menu else page | 378-381 | surfaces/controlcenter/src/lib.rs secondary | fixed: the page opens level with the module top (it followed the pointer) |
| right click while editing → resize | 380-383 | bg pointer-event | fixed |
| press-and-hold 800 ms | 376 | hold | same |
| drag to move (edit) | Glass 345-362 | bg pointer-event | fixed: springs home/new slot after release (stayed offset) |
| cast button opens cast page at the module | 291 | surfaces/controlcenter/src/lib.rs Media cast | fixed |
| chip glass follows squish/drag/dim | 430-438 | surfaces/controlcenter/src/lib.rs after_frame | missing: chips use resting rects |
| remove badge | 398-420 | badge | same |

## Fixed in batch 2

- Menu key / Shift+F10
- page level with module
- right-click resize in edit
- drag offset
- cast page position

## Still different / missing

- bare chip glass during drag/press/dim
- elastic follow
- blur

## Tester plan

Arena with the user's settings: copy `tools/live-scenarios.sh`, run with
`--arena N --only rust --bin <this worktree>/native/tessera-ui/target/release/tessera-ui`,
both `--look material` and `--look glass`; the QML for comparison with `--only qml`.
Open the Control Center: `ipc panels open controlCenter TESSERA-TEST`; read
rectangles from a `dev shot` first. Pointer: `dev pointer -o TESSERA-TEST move|glide|click|down|up|scroll`;
keys: `dev run wtype …`.

1. Glass: right-click Bluetooth: its page opens beside, **top level with the Bluetooth module** (not the pointer).
2. Edit controls: drag a module and release: it springs into place (no offset left). Right-click a module in edit mode: it resizes.
3. Tab to Wi-Fi, press Menu (or Shift+F10): the Wi-Fi page opens beside it.

## Test results

(tester)

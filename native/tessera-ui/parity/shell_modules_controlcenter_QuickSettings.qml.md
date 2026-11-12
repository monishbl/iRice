# shell/modules/controlcenter/QuickSettings.qml

Material Quick Settings panel. Rust: Material part of `ControlCenter` (surfaces/controlcenter/ui/controlcenter.slint ~2030-2720), surfaces/controlcenter/src/lib.rs.

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| page ↔ Panels.ccPage | 35-48 | surfaces/controlcenter/src/lib.rs set_page / host/system/src/accounts/cc.rs | same |
| shownPage kept while sliding out | 52, 57 | surfaces/controlcenter/src/lib.rs push_page | fixed: rows stay while the page slides out (it went blank at once) |
| pageT default spring | 53-56 | page-shown | same |
| Escape order | 61-67 | surfaces/controlcenter/src/lib.rs key | same |
| stagger / QSRise | 70-88 | rising/stagger | fixed in batch 1 |
| press squeeze | 116-146 | squeeze | same on tiles; not applied to slider/media neighbours (different) |
| body height spring, clip | 163-170 | m-h | same |
| home disabled while a page shows | 183-186 | home TouchArea blocker | fixed |
| grid height glides (default spring) | 243-246 | grid-h | fixed (gallery/footer jumped) |
| cell size/position springs, drag z/scale | 258-305 | mc | same |
| slider row: QSSlider full cell height, button 52 | 336-365 | slider row | fixed: band is the full 72 px cell |
| MediaCarousel (several players) | 368-377 | QsMedia (one) | missing (documented) |
| DragHandler drop + settle | 379-400 | tile drag | fixed (offset after drop) |
| gallery Momentum, keyboard on rows | 407-453 | m-gal Flickable | different (no momentum, no keys) |
| footer buttons keyboard / ElasticControl | 529-550, 611-632 | TonalButton | missing |
| edit: Done and the gallery rows by keyboard | 189-235, 407-453 | surfaces/controlcenter/src/lib.rs key extras, ring | fixed |

## Fixed in batch 2

- page rows kept while sliding out
- home disabled during page
- grid height spring
- slider row full height
- drag offset
- Done / gallery keyboard

## Still different / missing

- carousel
- TonalButton elastic swell
- squeeze on slider/media neighbours

## Tester plan

Arena with the user's settings: copy `tools/live-scenarios.sh`, run with
`--arena N --only rust --bin <this worktree>/native/tessera-ui/target/release/tessera-ui`,
both `--look material` and `--look glass`; the QML for comparison with `--only qml`.
Open the Control Center: `ipc panels open controlCenter IRICE-TEST`; read
rectangles from a `dev shot` first. Pointer: `dev pointer -o IRICE-TEST move|glide|click|down|up|scroll`;
keys: `dev run wtype …`.

1. Open CC (Material). Right-click Wi-Fi: page slides in. Press Escape: the page slides out **with its rows visible** (record; QML same).
2. While the page slides in (first 300 ms) click where a tile was: nothing toggles.
3. Edit controls: remove a tile (−): the gallery below glides up on a spring (record; not a jump).
4. Sound slider row: the slider occupies the whole row height (shot: track 52 px centred in the 72 px row).
5. Edit controls, Tab past the tiles: Done (ring), then each control to add; Enter on one adds it.

## Test results

(tester)

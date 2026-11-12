# shell/modules/desktop/WidgetDefs.qml → surfaces/desktop/src/grid.rs (`KINDS`, `pixel_size`, `material_card`, `read`/`entries`)

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Kinds, sizes, labels, own ids, bleed | all | `grid::KINDS` | same |
| Cell 164, gap 16; s 1×1, m 2×1, l 2×2 | all | `CELL`, `GAP`, `pixel_size` | same |
| Entries `kind:size:col:row[:id]` | all | `grid::read`, `grid::entries` | same |
| Material cards per kind and size | all | `material_card` | same |

## Test plan

Geometry (1920x1080 IRICE-TEST, the user's 4 widgets): grid origin 32,54; a cell 164 px, gap 16; a cell (c,r) starts at x = 32 + 180c, y = 54 + 180r; s = 164x164, m = 344x164, l = 344x344. The user's layout: clock m at (0,0) → 32..376 x 54..218; stack m at (0,1) → 32..376 x 234..398; batteries m at (0,2) → 32..376 x 414..578; shortcuts m at (2,0) → 392..736 x 54..218.
Both looks: run once with appearance.look "glass" (the user's) and once "material" (dark).

1. Covered by the unit tests (`cargo test -p tessera-desktop`) and by DesktopWidgets' plan: the user's 4 widgets sit at the cells listed above in both looks, Material cards in their containers (clock primary container, stack its top widget's, batteries, shortcuts edge to edge).

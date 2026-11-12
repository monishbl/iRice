# shell/modules/cheatsheet/Cheatsheet.qml → surfaces/cheatsheet (src/lib.rs, binds.rs, elements.rs, ui/cheatsheet.slint)

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | Dialog: opens on Dialog's spring (Material 312 ms spring(0.1), Glass 388 ms spring(0.23)), closes 192 ms on the look's exit curve | 19, glass/Dialog.qml | `animate progress` | **was** 520 ms spring(0.4) in both looks, Glass exit in both; now per look |
| 2 | Two pages in a segmented control: "Keyboard shortcuts" / "Periodic table" | 37-50 | segments | same |
| 3 | Segments (Material): chosen primary, others surfaceContainerHighest, inner corners 8 → 4 pressed → round chosen on springFast; colour on `hover` | Segmented.qml 98-134 | segment `for i in 2` | **was** corners jumped, colour 150 ms literal; now 222 ms spring(0.4), `Theme.hover`. Known difference: no press squeeze (Segmented's springPress widening) |
| 4 | Glass: the white pill slides on popover, emphasized | Segmented.qml 98-99 | pill `animate x, width` | same |
| 5 | Shortcuts: the user's binds grouped, three columns, keys as caps; scroll stops at bounds | 53-100 | `set_groups`, `in_rows` | same |
| 6 | Periodic table: 18 × 9.4 cells, tinted by category (18 % / hovered 45 %); hovered element rounds (Material) on springFast, outline 2 px (Glass 1 px) | 110-150 | `elements` | **was** 250 ms spring(0.6); now 222 ms spring(0.4) |
| 7 | Esc / click outside closes | — | keys, `dismissed` | same |

## Test plan (tester)
Both looks. `inbox 'ipc panels open cheatsheet IRICE-TEST'`.
1. Record the open (1 s) and compare the curve with QML (Material ≈ 0.3 s, Glass ≈ 0.4 s); `-k Escape` → 192 ms.
2. Click "Periodic table" → Material: the segment's inner corners round with a small overshoot in ≈ 0.22 s; Glass: the pill slides 220 ms.
3. Glide over an element → tint 45 % + outline; Material: rounds in ≈ 0.22 s.
4. Shortcuts page: binds in three columns, same groups and keys as QML (shot).

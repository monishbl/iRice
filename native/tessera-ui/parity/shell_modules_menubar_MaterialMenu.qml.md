# Parity: shell/modules/menubar/MaterialMenu.qml (+ its Glass twin glass/MenuPopover.qml and their base glass/Popover.qml)

The bar menu in each look: Material 3 Expressive vertical menu (groups on
surfaceContainerLow, 40 px rows, checked rows a tertiaryContainer pill) and
Glass MenuPopover (26 px rows, highlight). Both sit on Popover (full-screen
layer, spring open, fast close, outside click / Escape / 5 s autoClose).
Rust: `surfaces/menubar/ui/menubar.slint` `Menu` (706-895), `src/lib.rs`
(`menu_window` 859-899, `open_menu` 901-936, `menu_hold` 939-949, `close_menu`
951-974, `after_frame` 1690-1705 / 1738-1755), `src/menus.rs` (`width`,
`material_width`, `groups`, `step`, `panel_x`).
Q = MaterialMenu.qml, G = MenuPopover.qml, P = Popover.qml.

## Popover base (both looks)

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Full-screen Overlay layer; keyboard Exclusive while shown | P 77-79 | lib.rs:871-881, 929 | same (concurrent change) |
| Outside click closes; a click on the bar strip goes to the bar item there (switch / toggle) | P 93-107 | menubar.slint:747-750; lib.rs:1695-1705 (strip left to the bar) | same for clicks on items; a click on an empty part of the bar strip: QML closes (barItemAt → null → close), Rust per concurrent change ("any outside click") — verify the strip |
| Hovering other bar items does not switch menus (only clicks) | P 90-92 | bar strip still hoverable in Rust | different: bar items light up / show tips under an open menu |
| autoClose 5000 ms while pointer not over the body and `holdMenus === 0` | P 108-115; G 11; Q 18 | lib.rs:74, 939-949, 1185-1195; menubar.slint:780-781 | same (holdMenus ignored: not needed for bar menus) |
| Escape closes (side panel first) | P 127; G 70; Q 92 | menubar.slint:756 | same |
| Open: SpringAnimation progress 0→1, spring `Theme.motion.spring + 0.8`, damping `+0.06`, epsilon 0.004 (Glass 4.6/0.38, expressive 3.8/0.28, Material & snappy 6.88/0.56) | P 50-61 | menubar.slint:732-735 (`spring(0.3)`, 420 ms) | different: one curve for every look, not matched to Qt's spring per look |
| Close: 0.7 × popover (154 ms), bezier `exit` (Glass 0.4,0,1,1; Material 0.3,0,0.8,0.15) | P 62-70 | menubar.slint:733-734 (140 ms ease-in) | different: 140 ms ease-in for both looks |
| Body y = 34 − 8·(1−progress); opacity = min(1, progress) | P 122-124 | menubar.slint:763, 768 (opacity = min(1, 1.6·openness)) | different: Rust fades in 1.6× faster |
| No scale on open | P 117-127 | menubar.slint:766-767 (scale 0.92→1 about (18,0)) | different: Rust adds a scale-in |
| Input passes through while closing (mask 0); window hidden after ≤700 ms (CloseGuard) | P 73-76 | lib.rs:961-969, 976-983 | same |
| ElasticGrab: press-and-drag anywhere pulls the panel elastically; a drag doesn't count as an outside click | P 82-87, 97-100, 125 | — | missing |
| Glass: Glass body, lens 0.35, shadow margin; position clamp to screen | P 117-145; G 10 | lib.rs:1738-1755 (glass_shape lens 0.35, radius 14·scale); menus.rs:286-289 | same (glass radius 14 vs `Theme.radius.menu + 2`: verify) |
| targetX: Glass `Panels.anchorX`, Material `anchorX - 4` | G 35; Q 60 | menus.rs:286-289 | same |

## Rows and keys

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Width: Glass max(220, len·7.5 + (shortcut 60) + 48); Material max(232, len·7.4 + shortcut·7+28 + checks 28 + 40) | G 36, 39; Q 61 | menus.rs:251-268 | same |
| Padding Glass 5 / Material 4; radius Glass menu+2 / Material 16 | G 37-38; Q 19-20 | menubar.slint:719, 769 | same |
| `current = -1` on show | G 41; Q 63 | menubar.slint:740-743 | same |
| Up/Down step over enabled, non-separator, non-header, non-tile rows (wraps) | G 52-61, 65-66; Q 74-88 | menubar.slint:754-755 → menus.rs:270-282 | same; Rust also steps on Tab (extra) |
| Return / Enter / Space activate `current` | G 67-69; Q 89-91 | menubar.slint:757 | same (keypad Enter: verify) |
| Activate: ignore separator/disabled; close unless keepOpen; run action | G 43-50; Q 65-72 | lib.rs:1004-1013 | same |
| Row hover sets `current`; leaving clears it if it was this row | G 171-177; Q 224-233 | menubar.slint:881-888 | same |
| Disabled rows take no hover/click | G 174; Q 228 | menubar.slint:882 | same |
| Glass row: 26 px, highlight when hot (no colour animation), check 12 px at x 6, text x 22, shortcut right 10 | G 86, 143-170 | menubar.slint:789, 839-880 (animate background 100 ms) | different: Rust animates the highlight 100 ms (QML instant) |
| Glass separator 11 px, line width −18; header 22 px caption DemiBold tertiary | G 86, 93-109 | menubar.slint:789, 804-816 | same |
| Glass tiles: 44×32, highlight on hover (no anim), TileGlyph hot; click closes then acts | G 111-141 | menubar.slint:817-838; lib.rs:1170-1176 | same |
| Material groups: surfaceContainerLow blocks split at separators, 2 px apart, corners 12 outside / 4 inside, 4 px padding | Q 41-57, 95-115 | menubar.slint:789-795; menus.rs:235-249 | same (emulated per row) |
| Material row 40 px; header 32 px labelMedium (12 px Medium, letterSpacing 0.5) | Q 133, 141-149 | menubar.slint:789, 809-816 | same except letterSpacing 0.5 not set |
| Material hover state layer `Theme.color.highlight`, pressed 0.1 onSurface; Behavior colour `Theme.motion.hover` (150 ms) | Q 194-196 | menubar.slint:845-848 (hover 0.08, no pressed, 100 ms) | different: no pressed layer; 100 ms vs 150 ms |
| Material checked row: tertiaryContainer pill (radius h/2), tint 0.08/0.1 when hot/pressed, tick 16 px at x 12, text x 40 when any checks | Q 190-216 | menubar.slint:844-858 | same except pressed tint |
| Material disabled text onSurface 0.38; shortcut 13 px onSurfaceVariant | Q 197, 217-223 | menubar.slint:862-878 | same |
| Material tiles: radius 8, hover highlight, pressed 0.1, Behavior colour hover; pointing-hand cursor | Q 159-184 | menubar.slint:823-838 | different: no pressed state, no colour animation, no hand cursor |
| Pointing-hand cursor on Material rows | Q 229 | — | missing |
| Accessibility roles (MenuItem, checkable, press action) | G 87-91; Q 134-138 | menubar.slint:796-802 | same |

## Summary (missing / different, by priority)

1. Open/close motion not matched: Rust spring(0.3)/420 ms open and 140 ms ease-in close for both looks, plus an extra 0.92→1 scale and 1.6× faster fade; QML uses Qt springs per look (Glass 4.6/0.38, Material 6.88/0.56) and 154 ms `exit` bezier.
2. ElasticGrab (press-and-drag elastic pull; drags not treated as outside clicks) missing.
3. Bar items stay hoverable (chips, tips) under an open menu; QML's full-screen catcher blocks that.
4. Material: no pressed state on rows/tiles, 100 ms vs 150 ms hover colour, no tile colour animation, no hand cursor, header letter-spacing missing.
5. Glass: row highlight animates 100 ms in Rust (QML instant).
6. Extra Tab-to-step key in Rust.

## Fixed in batch 4 (fixer, 7 Nov)

- (The space menu is a ContextMenu: it no longer uses MaterialMenu's rows in Material; the system/app/Window/tray menus still do.)

## Test plan (batch 4)

- Material: system menu = grouped Material rows (40 px); space menu = plain 28 px rows.

## Test results (batch 4)

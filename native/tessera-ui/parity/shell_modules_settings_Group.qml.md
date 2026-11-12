# shell/modules/settings/Group.qml ↔ settings.slint GroupView / RowView (tile, hairline), model.rs Page::group / Page::shown

A grouped form section: caption, platter, footer. Material: Expressive segmented
list (a tile per row, 2 px apart, outer corners 24, inner 4). Glass: one white /
5 % platter with hairlines between rows.

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | No row on show → the whole group folds (no caption, platter, footer, gap) (8–25, 30, 40, 116) | model.rs `Page::shown` skips groups with no rows (`Block::row` leaves out invisible rows) | same |
| 2 | Spacing caption/platter/footer 8 / 6 (27) | GroupView `spacing` | same |
| 3 | Caption: x 16 / 4, primary 14 px 500 0.1 tracking / secondary small 600 (29–35) | GroupView title Text | same |
| 4 | Platter: radius 12, Material transparent, Glass white (dark 5 % white), 1 px separator border Glass only (37–44) | GroupView Rectangle | same at a glance (QML Squircle, Rust round rect) |
| 5 | Material tile per visible row, surfaceContainerHigh, first row's top corners 24, last row's bottom 24, others 4 (47–82) | RowView Material Rectangle (`first`/`last`) | same |
| 6 | Material hover: the row under the pointer gets an 8 % onSurface state layer, ColorAnimation Theme.motion.hover (150 ms) (73–84, 94–97) | RowView `hover.has-hover` → mix 0.92, `animate background SM.hover` | same (tester: check the tile stays lit while the pointer is over the row's control, e.g. a switch) |
| 7 | Material rows 2 px apart (93) | GroupView inner VerticalLayout spacing | same |
| 8 | Glass hairline above each visible row but the first, x 14, width −28, separator (100–113) | RowView `!Theme.material && !first` | same |
| 9 | Footer: padding 16 / 4, onSurfaceVariant 12 px 0.4 tracking / tertiary caption, wraps (115–125) | GroupView footer | same |

## Test plan

Material, then Glass (`tools/settings-scenarios.sh setup rust material <bin>` / `… glass`; the QML the same with `setup qml …`). Material window 1040x700 at 440,205.

G1 `open appearance`, `shot g1-<shell>-<look>`. PASS: captions, tiles (24/4 corners, 2 px gaps), footer text match the QML shot.
G2 Hover: `clicks g2` is click-only, so use `run 'P move 900 <y1>; P move 900 <y1> glide 900 <y3> 900'` while recording (`REGION` ≈ the group) — read the y's of three rows from G1. PASS: each tile lights under the pointer in ~150 ms and un-lights after; it stays lit while over the row's switch.
G3 Folding: Wallpaper page, flip the "Dynamic" choice so a conditional group appears/disappears, `shot` each. PASS: a group with no rows leaves no caption and no gap, as in the QML.

## Test results

(tester)

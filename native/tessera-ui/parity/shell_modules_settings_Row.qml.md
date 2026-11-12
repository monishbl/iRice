# shell/modules/settings/Row.qml ↔ settings.slint RowView (kind "row"/"text"), Control; model.rs Row

A form row: optional icon, label (+ detail) on the left, a control on the right.

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | Height: Material max(56, labels+24, control+16); Glass max(44, labels+20, control+16) (18) | RowView min-height; label padding 12/10 top+bottom; control padding 8 | same |
| 2 | Side padding 20 / 14 (17, 23, 62) | RowView `pad` | same |
| 3 | Leading icon 24 onSurfaceVariant / 20 tertiary, gap to label 16 / 12 (20–30) | RowView `has-icon` SIcon | same |
| 4 | Label 16 px / body, wraps; labelColor (a status line: secondary or danger) (32–38) | Text, `r.tone` 0–3 | same |
| 5 | Detail 14 px / caption, secondary, wraps, hidden when empty (39–46) | `if r.detail != ""` | same |
| 6 | Label to control gap 16 / 12 (30) | label VerticalLayout padding-right | same |
| 7 | Controls named after the row's label for screen readers (49–56) | Control passes `label: r.label` to each control | same |
| 8 | Control slot sized by visible controls only, vertically centred (58–72) | Control HorizontalLayout `alignment: end`, VerticalLayout `alignment: center` | same |
| 9 | A hidden row takes no space | model.rs `Block::row` drops `!visible` rows | same |

## Test plan

R1 `open mouse` (Material and Glass, both shells), `shot r1-<shell>-<look>`. PASS: row heights, paddings, icon sizes, label/detail fonts and the word sliders' places match the QML shot (same window position: compare crops).
R2 A status row: `open network`, `shot r2-…`. PASS: secondary / danger status colours as in the QML.

## Test results

(tester)

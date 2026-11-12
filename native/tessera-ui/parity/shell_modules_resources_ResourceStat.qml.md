# shell/modules/resources/ResourceStat.qml

One of Resources' stats with its last minute: name and value above the
graph, or `compact` on one line with a small graph between. Used only by the
desktop's System widget (shell/modules/desktop/SystemWidget.qml 37), whose
Rust is the desktop crate (`surfaces/desktop/src/lib.rs` "system", 831; its
`desktop.slint`): **owned by the desktop agent**, so behaviours are listed
here and the fixes belong to that crate.

| behaviour | QML line | Rust place | status |
|---|---|---|---|
| Keeps Resources sampling while shown (graphUsers ±1 on visible) | 33-43 | desktop lib.rs (system widget asks `resources.use graph`) | check (desktop agent) |
| Head: label (small; compact: secondary, normal; else label, DemiBold) left, value right (small, tnum; compact label colour, else secondary) | 49-69 | desktop.slint system widget | check |
| `showDetail`: value + "  ·  " + detail | 65 | — | check |
| Compact: graph between name (≤ 84 px) and value, left 88, right 72, 14 px high, line 1.25 | 70-79 | — | check |
| Full: graph under the head, `graphHeight`, tone colour, style per resources.graphStyle | 81-88 | desktop.slint | check |
| Tone: Material ResourceTones role, else the accent named | 11 | — | check |
| Repaints only when values change (once a second) | ResourceGraph 121-127 | — | check |

## Test plan (Sonnet tester)

Desktop with a System widget (Settings → Desktop → add "System", size m): each stat shows its name and value above a graph that fills over a minute; values change once a second without the widget flickering (`tools/region-changes.py` on the widget only shows the graph/figures changing). Material: graphs in primary/secondary/tertiary. Resource graph style (area / bars / line) changes all graphs at once.

## Test results

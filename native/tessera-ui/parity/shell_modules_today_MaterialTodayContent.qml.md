# shell/modules/today/MaterialTodayContent.qml (Material)

Rust: ui/today.slint `MaterialDay`, `DayButton` (Material branch),
`MaterialCheck`, `Segmented`, `Toggle`; data in src/lib.rs.

| Behaviour | QML | Rust | Status |
|---|---|---|---|
| `now` each minute; events / to-dos / hours live | 23-31 | minute timer + data → refresh() | lists were new models every refresh → **fixed** (kept `Rows`; to-dos keyed) |
| Esc / Enter close | 33-35 | keys | same |
| Stagger (Rise: 40 ms apart, 300 ms, 8 px) | 40-53 | Rise | same |
| MButton: 40 high, radius 20 → 12 pressed (press, OutCubic), colour hover duration, ElasticControl swell (springPress) | 57-104 | DayButton | swell was 290 ms spring(0.3) (ζ 0.7: no bounce) → **fixed** to springPress fitted from Qt's steps: 235 ms spring(0.53) |
| Focus ring radius + 5 | 79 | Ring | same |
| Date cookie (cookie12), greeting, weather on verySunny / cookie4 | 110-290 | Shape, cookie | same; shapes are outlines, not the shader (same at a glance) |
| Events card: as many as fit | 296-330 | events (fits) | same |
| To-dos card: MaterialCheck (state layer 0.08/0.1), text, due (error when overdue) | 375-412 | MaterialCheck | same |
| Hours card | 445-480 | hours | same |
| Buttons Options / Pin / Close; Tab | 500-520 | DayButton | same |
| Options sheet over it | 525-530 | OptionsSheet | same |
| Icons: Material Symbols filled | throughout | Material Symbols outlined | **different** (Slint can't set the FILL axis) |

## Test plan (Material; both shells)

1. `ipc today preview bloom`; press and hold the Close button (bottom
   right of the panel: padding 24, Close's right edge x 1366, centre ≈ (1321, 806)) 300 ms: it swells ≈ 6 % with corners
   20 → 12, and on release springs back past its size (a visible bounce,
   ≈ 0.5 s) — record and compare to QML.
2. Hover a button: tint 8 %, press 10 %, fading 150 ms.
3. Tick a to-do's check: it leaves; the other cards don't redraw (no
   fade-in again).
4. Across a minute boundary: nothing reloads (as TodayContent's plan).

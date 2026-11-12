# shell/modules/today/TodayContent.qml (Glass)

Rust: ui/today.slint `GlassDay`; data in src/lib.rs `refresh()`, src/model.rs.

| Behaviour | QML | Rust | Status |
|---|---|---|---|
| `now` ticks every minute; events and to-dos follow | 18-26 | `minute` timer → refresh(); Calendar/Todos/Weather data → refresh() | same; the lists were **replaced on every refresh** (every row element made again: the stagger's rows and the tick hovers rebuilt each minute) → **fixed**: events, the first five, to-dos (keyed by the to-do's key) and the hours are kept `Rows` |
| Toggle a to-do (Todos.update) | 28-30 | toggle_todo → todos.toggle, ticked at once | same |
| Stagger: row n, 40 ms apart, 300 ms fade, rising 8 px (cubic) | 33-43 | Day.appear / Rise | same (motion::appear tested) |
| Esc / Enter close | 46-48 | keys FocusScope | same |
| Greeting + date, weather now (icon filled, temp, condition) | 51-100 | header | same; weather glyph outlined (Phosphor fill axis) |
| "Today" title; empty "Nothing on your calendar today"; five events with colour bar, time, dimmed when past | 103-190 | events-five | same |
| To-dos: ring tick (accent on hover, hit area +8), text, due (danger when overdue), "No to-dos…" / "All done" | 192-240 | todos | same |
| Hours ahead: time, icon, rain ≥ 20 %, temp | 280-330 | hours | same |
| Buttons Options / Pin for today (hidden when the widget is always on) / Close (primary, focused); Tab moves | 336-367 | DayButton ×3 | same |
| TodayOptions over the panel | 370-375 | OptionsSheet | same |

## Test plan (Glass; both shells)

1. `ipc today preview bloom`; wait 2 s. Hover each to-do's ring (left
   column, x ≈ 571): its border turns accent; click: the to-do leaves the
   list, the others don't fade in again (record 1 s; only the list below
   the ticked one moves).
2. Leave it open across a minute boundary (watch the clock), recording 3 s
   around it: no row fades in again (region-changes: changes only where
   text changed).
3. Tab cycles Options → Pin for today → Close (focus ring 2 px accent);
   Enter on Close closes. Esc closes.

Geometry (Glass, panel x 530, y 230, 860×620, padding 32): greeting at
(562, 262); weather top right ending x 1358; to-do rings at x 571, rows
36 px apart below the events; buttons bottom right, Close's right edge at
x 1358, centre y ≈ 800 (Close ≈ (1318, 800), Pin and Options to its left,
12 px apart). The stagger: row n fades in 40 ms after row n−1 over 300 ms
rising 8 px; all rows done ≈ 880 ms after the bloom starts.

# shell/modules/overlay/OverlayWidget.qml → ui/overlay.slint `Frame` + `BarButton`, src/model.rs

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | A widget: glass card at its saved x/y/w/h, dragged by its title bar, saved on release | 20-60 | widget frame, `model::save` | same |
| 2 | Shown while open, or pinned; click-through ones at 55 % while closed; opacity on the hover duration | 25-29 | `opacity`, `animate opacity { Theme.hover }` | same |
| 3 | Title-bar buttons (click-through, pin, close, resize) 24 px; Material: on → corners 8 on springFast, hover state layer 8 %; Glass: fill hover | 79-110 | `BarButton` | **was** 250 ms spring(0.6); now 222 ms spring(0.4) |

## Test plan (tester)
Both looks. Open the overlay (`inbox 'ipc panels open overlay TESSERA-TEST'`), toggle a widget on (e.g. Resources).
1. Drag its title bar (`P down` on it, `P glide` 200 px, `P up`); close and reopen: it stays where dropped.
2. Material: click the pin button → it turns accent and its corners go from 12 to 8 in ≈ 0.22 s (record 0.5 s, compare with QML).
3. Click-through on, close the overlay → the widget at 55 % (fade over 150 ms).

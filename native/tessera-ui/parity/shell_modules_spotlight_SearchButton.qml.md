# shell/modules/spotlight/SearchButton.qml → ui/spotlight.slint `MButton`

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | Pill 40 high (Get on rows 32), width label + 2·round(h·0.6); filled primary / tonal secondaryContainer; colour 150 ms | 13-24, 34-35 | MButton | same |
| 2 | Pressed: corners → min(12, h/2) on springFast; press swell | 25-33 | `animate border-radius` | **was** 250 ms spring(0.4); now 222 ms spring(0.4) |
| 3 | State layer content colour 8 % hover / 10 % pressed, 150 ms | 41-48 | inner Rectangle | same |
| 4 | Label 14 px (13 under 40 high), 500 | 49-54 | Text | same |
| 5 | Click | 55-61 | TouchArea | same |

## Test plan (tester)
Material. Needs the AI answer buttons (AI set up) or a store row's "Get"; if the arena has neither, mark "not testable here".
1. Hover each button → 8 % layer over 150 ms; `P down` → corners square to 12 in ≈ 0.22 s; `P up` → it acts ("Copy answer" copies and closes).

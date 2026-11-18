# shell/modules/spotlight/SearchView.qml → surfaces/spotlight (Material drawing in ui/spotlight.slint)

Spring conversion used here (Qt SpringAnimation steps 16 ms): ω = √(62.5·spring), ζ = 62.5·damping / (2ω);
Slint `spring(bounce)` with duration d has ω = 2π/d, ζ = 1 − bounce.
springDefault 6.08/0.5 → ω 19.5, ζ 0.80 → 322 ms, spring(0.2). springFast 12.8/0.54 → ω 28.3, ζ 0.60 → 222 ms, spring(0.4).

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | Opens on springDefault (~1.5 % overshoot, settles ≈ 0.3 s); closes popover×0.7 (154 ms) exit bezier | 39-74 | `animate openness` | **was** 450 ms spring(0.5) (ζ 0.5, ω 14: slower and bouncier); now 322 ms spring(0.2) |
| 2 | Scrim 32 % × progress; click on it closes | 111-120 | scrim + TouchArea | same |
| 3 | Column 720 (≤ w−64) centred, y round(h·0.2); opacity progress×1.6; scale 0.92→1 from top centre; drops from −32 px | 122-140 | `col` | same |
| 4 | View height 56 + body (answer+28, or list ≤ 480 + 20) on springDefault; colour surfaceContainer open / surfaceContainerHigh (150 ms) | 143-152 | `m-view-h` | **was** 450 ms spring(0.5); now 322 ms spring(0.2) |
| 5 | Bar 56 r 28 surfaceContainerHigh; magnifier 22 at x 18 (caret-left back button while answering); field 18 px; placeholder "Search apps, files and the web" | 157-219 | bar | same |
| 6 | Trailing: Clear (x, while text), Ask (sparkle, filled + primary while answering), 40 px round buttons | 221-249 | trailing | same (no tooltips) |
| 7 | Keys: Esc close, Enter accept, Down/Up, Tab next, Shift+Space / Ctrl+Y Quick Look | 202-208 | `key` | same |
| 8 | Groups per section (12 between groups, 2 between rows), label primary 14 px 500 tracking 0.1 ("Top hit"); first row a hero for Top Hit/Calculator/Answer/Conversion/Dictionary/Do | 262-356 | `layout::material` | same |
| 9 | A list that just appeared: rows ≤ index 10 rise 14 px and fade in, each on springDefault, starting 16 + 28·index ms | 33-35, 94-109, 292-309 | intro | **was** a cubic ease over 380 ms; now the spring's closed form 1 − e^(−15.6t)(cos 11.7t + 1.33 sin 11.7t), same stagger |
| 10 | Inline AI answer: 36 px sunny shape (spinner while thinking), model name, answer ≤ 360, "Continue in chat" (filled) / "Copy answer" (tonal) + "Enter continues in chat" | 359-451 | answer body | same (Busy arc spinner; StyledText Markdown) |

## Test plan (tester)
Material (the user's look). Geometry 1920×1080: view x 600..1320, top y 216; bar centre (960, 244); Clear ≈ (1236, 244), Ask ≈ (1280, 244); results start y ≈ 280.
1. **Open**: `rec sp-m-open 1 "ipc panels open spotlight TESSERA-TEST"`; track the view's top edge / width over frames: QML and Rust settle in ≈ 0.3 s, overshoot ≤ 2 % (before, Rust bounced visibly ~0.45 s). `-k Escape` → 154 ms.
2. **View grows**: type `f` → view springs to the list's height in ≈ 0.3 s; compare the height curve QML vs Rust (± 1 frame).
3. **Row entrance**: clear the field, type `fi` → rows rise and fade one after another (28 ms apart); compare frame strips.
4. **Scrim click** at (960, 900) closes; a click inside the view doesn't.
5. Clear/Ask: hover → 8 % layer; Clear empties and refocuses.

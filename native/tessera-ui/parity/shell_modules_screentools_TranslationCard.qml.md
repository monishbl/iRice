# shell/modules/screentools/TranslationCard.qml → surfaces/screentools (src/cards.rs `Translation`, ui/screentools.slint `TranslationCard`)

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | Shown while ScreenTools.translation is set, on its screen; Overlay, keyboard OnDemand | 10-21 | `Translation::set` | same |
| 2 | Width max(280, min(460, the area's width)) | 15 | `card.width` | same |
| 3 | x: the area's left, kept within [24, screen − card − 24] | 23 | `card.x` | **was** max(0, …) (could touch the screen's left edge); now max(24, …), the QML window's margin |
| 4 | y: 8 below the area, kept within [54, screen − card − 24] | 24 | `card.y` | same |
| 5 | Glass r 20 (Material r 24 in surfaceContainerHigh); content 14 in (Material 20 × 16), spacing 8; height content + 28 | 27-58 | `card` | same |
| 6 | Heading "TRANSLATION" caption 600 secondary (Material "Translation", small, primary), 22 high (Material 32) | 60-70 | heading | same |
| 7 | Copy and close: 22 round, hover fillHover (Material 32 tonal with state layers, r 8 pressed, ElasticControl swell on springPress) | 71-89 | buttons | **was** no Material press swell; now 1.06 on 290 ms spring(0.57) |
| 8 | The text at title size, "Translating…" while busy, an error in danger | 91-97 | Text | same |
| 9 | Source: small, tertiary, at most 4 lines, elided | 98-107 | Text, max-height | same (4 line heights; Slint elides by height) |
| 10 | Esc closes; Copy → wl-copy | 56, 76 | `on_key`, `copy` | same |
| 11 | Icons follow the look | 87 | `look()` | **was** set once when the window was made; now set again on a look change |

## Test plan (tester)
The translation needs network and translate-shell: the translated text is **not testable here** without them, but the card and its error show. Arena 1920×1080, both shells, both looks. Open a foot window with some text at the top left.
1. `inbox 'ipc screen region translate'`, `P glide 100 200 100`, `P down`, `P glide 500 260 300`, `P up` → card at x 100, y 268, 400 wide ("Translating…", then the result or "Couldn't translate…" in red).
2. **Left edge** (row 3): repeat with the drag starting at (4, 200) → the card's left edge at x 24 in both shells (Rust was at 4).
3. **Buttons** (row 7): hover Copy / Close at the heading's right end → Glass fillHover, Material state layer. Material: `P down` on Close, `shot tr-press` → swelled (≈ 34 px) with r 8; `P up` closes the card.
4. Click into the card, `wtype -k Escape` → closes.

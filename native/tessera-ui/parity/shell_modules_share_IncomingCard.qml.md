# shell/modules/share/IncomingCard.qml → surfaces/share (src/lib.rs `update_card`, `card_key`; ui/share.slint `ShareCard`, `CardButton`)

springFast 12.8/0.54 → ω 28.3, ζ 0.60 → 222 ms spring(0.4); springPress 7.5/0.3 → ω 21.7, ζ 0.43 → 290 ms spring(0.57).

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | Shown while Sharing.request; on the focused screen; overlay, keyboard on demand, no exclusive zone | 47-60 | `update_card`, `card_output`, `card_surface` | same |
| 2 | Enter: 320 ms × motionScale, OutBack overshoot 1.2 (≈ 5.3 % over); opacity + scale 0.9 → 1 (glass, Material surface and content); gone at once when the request goes | 62-91 | `animate shown`, `card_glass` | **was** cubic-bezier(0.34, 1.45, 0.64, 1) (≈ 6.6 % over); now (0.34, 1.4, 0.64, 1), ≈ 5.3 % |
| 3 | Card 300 (Material 312), padding 20/22 (Material 24/24), height body + 2·padX | 87-99 | `card` | same |
| 4 | Enter accepts while asking; Esc declines (asking) or drops | 92-93 | `card_key` | same (Tab/Space on the buttons stand in for Qt's focus chain) |
| 5 | Avatar 84: Material shape 76 by state (cookie9 / circle / sunny / square in errorContainer); Glass 68 accent gradient; a computer glyph 32, or a phone outline with the link glyph (wifi / broadcast / bluetooth) | 103-167 | avatar | same (shape switches, no morph) |
| 6 | Glass ring 4/3 while receiving; Material wavy bar 160 (indeterminate while waiting) | 168-208 | `Ring`, `WavyBar` | same |
| 7 | Done/failed badge 26 r 13, inset 6: primary/success ✓, error/danger × | 179-196 | badge | same |
| 8 | Name (title 600); message by state ("would like to share … (size).", "Receiving … n %", "… are in Downloads.", failure) ≤ 3 lines; bytes line tertiary; PIN chip (Material outlined 32 r 8; Glass fill 24 r 12) | 209-259 | `card_message`, `card_bytes` | same (no tabular figures) |
| 9 | Buttons by state (Decline/Accept, Cancel/Hide, Show in Files/Open or Done/Show in Files, OK), 40 high, 10 apart, the one to do on the right; Repeater keeps them | 263-283 | `card_buttons`, `act_call`, `btns: Rows` | **was** a new model per feed (each progress tick remade the buttons, hover lost); now set in place |
| 10 | Material button: primary / secondaryContainer + 8 %/10 % layer; radius 20 → 12 pressed on springFast; ElasticControl swell 1 + min(6 %, 10 px / width) on springPress | 289-300 | `CardButton` | **was** radius 250 ms spring(0.3), swell 1.04 on 350 ms spring(0.3); now 222 ms spring(0.4), swell 1.06 (1.038 for the lone OK), 290 ms spring(0.57) |
| 11 | Glass button: accent (darker pressed) / fill with hover; scale 0.97 pressed, 100 ms **linear** | 300-304 | `CardButton` | **was** 100 ms OutCubic; now linear |
| 12 | A finished request is dropped after 9 s | 37-42 | (the shell's Timer: IncomingCard.qml stays loaded) | same |

## Test plan (tester)
Arena 1920×1080, sandbox. Glass card x 810..1110 (Material 804..1116), centred. Asking with a PIN, the buttons' centre y ≈ 645: Decline ≈ (892, 645), Accept ≈ (1027, 645) (take exact y from a first shot).
1. **Enter** (fix 2): `rec in-enter 0.6 "ipc sharingdemo ask"` → scale 0.9 → 1, ≈ 5 % over near 0.18 s, settled by 0.32 s; compare the scale curve QML vs Rust (± 1 frame).
2. **Press**, Material (fix 10): `P move 1027 645`, `rec in-press 0.4 "P down"` → corners 20 → 12 in ≈ 0.22 s with a slight overshoot, swelling to 1.06; `P glide 960 950 100`, `P up` (released off it: nothing happens).
3. **Press**, Glass (fix 11): same → 0.97, linear over 100 ms.
4. **Hover kept through progress** (fix 9): `inbox 'ipc sharingdemo progress 10'`; `P glide 892 <Cancel y> 200` (Cancel hovered); `inbox 'ipc sharingdemo progress 40'`, `progress 80` → Cancel keeps its hover fill every time; ring/wave and "Receiving 2 photos… 80%", "5 MB of 6.2 MB".
5. `inbox 'ipc sharingdemo done'` → sunny / ✓, "2 photos are in Downloads.", Done / Show in Files; 9 s later the card goes.
6. Keys: `inbox 'ipc sharingdemo ask'`, click the card's body once (keyboard on demand), `wtype -k Escape` → declined, card goes. Again, `-k Return` → receiving.
7. `inbox 'ipc sharingdemo ask; ipc sharingdemo fail'` → square in errorContainer / danger ×, "Maya's phone stopped sending.", OK.
8. A real phone's transfer: not testable here.

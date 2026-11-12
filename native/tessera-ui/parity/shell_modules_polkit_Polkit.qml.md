# shell/modules/polkit/Polkit.qml → surfaces/polkit (src/lib.rs, agent.rs, ui/polkit.slint)

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | Previews: `polkit preview` (password) and `previewFinger` (fingerprint step) | 115-135 | forwarded as told | same |
| 2 | Dialog 340 wide centred, scrim; progress over `panel` (320 ms): emphasized in, exit out, per look | 140-150 | `animate progress` | same |
| 3 | A wrong password shakes the body: +10, −10, +6, 0 px over 50/80/70/60 ms | 175-185 | `shake` from animation-tick | same |
| 4 | Material leading shape: cookie9 → softBurst on error, clover8 / softBurst while waiting for a finger; colour on `hover` | 218-230 | MShape | same (clover8 drawn as clover4: core has no clover8) |
| 5 | Field: password dots, Enter submits, Esc cancels | 240-300 | field | same |
| 6 | Buttons Cancel / Allow ("Checking…"); Allow disabled while empty; Material 40 high, corners 8 while pressed on springFast, press swell; Glass 34 high | 330-360 | button row | **was** 200 ms spring(0.3) corners, fixed 1.04 on 250 ms spring(0.4); now 222 ms spring(0.4), 1 + min(6 %, 10 px / size) on 290 ms spring(0.57) |
| 7 | The real agent runs only outside demo / sandbox | — | `real` | same (by design) |

## Test plan (tester)
Both looks. `inbox 'qs ipc call polkit preview'` (forwarded to tessera-ui), or the arena's equivalent.
1. `rec pk-open 1 "<preview command>"`: body fades/slides in over 320 ms on the emphasized curve; compare QML frames ± 1.
2. Material: glide onto Cancel (≈ x 870, bottom row of the card), `P down` → corners 8 in ≈ 0.22 s with a slight overshoot and a ≈ 6 % swell; `P up`.
3. Type `wrong` + `-k Return` (preview mode answers "wrong") → body shakes 260 ms (+10/−10/+6/0); record and compare.
4. Allow is dimmed while the field is empty (Material 38 % ink / 12 % base; Glass 50 % opacity).
5. `previewFinger` → finger glyph and Cancel only; Material shape clover (drawn 4-leaf).
6. `-k Escape` → closes on the exit curve.

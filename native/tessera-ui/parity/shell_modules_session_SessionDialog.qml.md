# shell/modules/session/SessionDialog.qml → surfaces/session (src/lib.rs, ui/session.slint)

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | Dialog opens on Dialog's spring (Material 312 ms spring(0.1), Glass 388 ms spring(0.23)), closes 192 ms exit | 44-50, glass/Dialog.qml | `animate progress` | **was** 420 ms spring(0.12) / spring(0.3); now Dialog's per look |
| 2 | Five choices: Lock, Sleep, Log Out, Restart, Shut Down (Material shapes cookie4 / cookie9 / clover4 / cookie12 / softBurst in primary / secondary / tertiary / error; labels sentence case) | 60-66, 170-180 | `options` | same |
| 3 | "What would you like to do?"; hint line "Click, or choose with the arrow keys and press Enter · Esc to cancel" | 89, 190 | texts | same |
| 4 | Arrows move (clamped), Enter / keypad Enter choose (not on repeat), Esc closes | 99-106 | keys | same |
| 5 | Glass circle 64 px: accent / danger when current, else toggle-off; 94 % while pressed over `press` (100 ms, linear); colour on `hover` | 130-148 | option circle | **was** ease-out; now linear (ScaleAnimator's default) |
| 6 | Material: 88 px shape, press swell (ElasticControl) | 136 | option | **was** fixed 1.06 on 250 ms spring(0.4); now 1 + min(6 %, 10 px / size) = 1.06 on 290 ms spring(0.57) |
| 7 | Hovering a choice makes it current | 150-160 | `M::Hover` | **was** made the five choices again on every hover (a press under the pointer restarted); now one `Rows`, set in place |

## Test plan (tester)
Both looks. Open: `inbox 'ipc panels open session TESSERA-TEST'`. Do NOT press Enter on Sleep/Log Out/Restart/Shut Down in the arena unless it's sandboxed; use Esc.
1. `rec ses-open 1 "ipc panels open session TESSERA-TEST"`: compare the open curve with QML (Material ≈ 0.3 s, Glass ≈ 0.4 s).
2. `-k Right` ×6 → stops at Shut Down; `-k Left` ×6 → Lock. The current one is accent (Glass) / its shape filled (Material).
3. Glide across the five → the current follows the pointer; then `P down` on one and hold while gliding a few px → the swell (Material) / 94 % (Glass) stays, doesn't restart.
4. `-k Escape` → closes in 192 ms.

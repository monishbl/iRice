# shell/modules/share/ShareSheet.qml → surfaces/share (src/lib.rs `push_sheet`, `sheet_key`; ui/share.slint `ShareSheet`, `DeviceItem`, `WayRow`)

Spring conversion (Qt SpringAnimation steps 16 ms): ω = √(62.5·spring), ζ = 62.5·damping / (2ω); Slint `spring(bounce)`, duration d: ω = 2π/d, ζ = 1 − bounce.
springPress 7.5/0.3 → ω 21.7, ζ 0.43 → 290 ms spring(0.57) (ElasticControl's Material swell).

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | Full-screen overlay; progress 220 ms OutCubic in, 150 ms InCubic out; unmapped after the fade | 31-37, 67-73 | `animate progress`, `closed` → `sheet_closed` | same |
| 2 | Input only while shown (mask 0 while fading out) | 74-76 | scrim TouchArea `enabled: shown`, keyboard None on close | same for the scrim; the card's own rows still take a click during the 150 ms fade (known: no input mask switch here) |
| 3 | Scrim black 25 % × progress; click closes | 78-82 | scrim | same |
| 4 | Card 420 (Material 432) centred, opacity = progress, scale 0.94 → 1; Glass glass r panel / Material surfaceContainerHigh r 28 | 84-105 | `card` + `after_frame` glass | same (Material: a Slint drop shadow for Glass.qml's shadow) |
| 5 | Esc closes; Tab/Shift+Tab/←/→ move focus × → devices → ways; Space/Enter acts | 92, 171-177, 214-227, 471-477 | `sheet_key`, `sheet_handles` | same (own focus index, not Qt's chain: a click doesn't move it) |
| 6 | Header: 52 px thumb r 16/12 fill; picture (async, 104 px, crop) or file icon 36 px; title elided in the middle, title size 600; kind line; × 40/30 round | 116-184 | header, `ask_thumb` (thread), `icon_image` (cached), `elide_middle` | same |
| 7 | ×: hover/press fill; Material radius 12 while pressed, **at once** (no Behavior); Material swell 1.06 on springPress; Glass no scale | 162-183 | `close` | **was** radius animated 250 ms spring(0.3), swell 350 ms spring(0.3); now radius instant, swell 290 ms spring(0.57) |
| 8 | Glass hairline under the header | 186 | separator | same |
| 9 | Devices row 118/128 high, items 92 wide 4 apart, flicks; a focused device scrolls to the middle | 189-228 | Flickable, `row_scroll` | same |
| 10 | Device list follows Sharing.devices: a progress tick updates one device, the others keep their delegate (hover, press, wave) | 202-203 (Repeater) | `devs: Rows` (`set_keyed` by key) | **was** a fresh model on every feed (each progress tick rebuilt every avatar, hover lost); now set in place |
| 11 | Avatar 68: Material shape 64 (cookie9/clover4/cookie4/sunny; hover cookie12; sending circle), primary/tertiaryContainer; Glass 60 round accent gradient, 88 % on hover | 234-290 | `DeviceItem` | same (shape switches, no morph) |
| 12 | Press: Glass scale 0.94, 100 ms **linear**; Material ElasticControl 1.06 on springPress | 240-244 | `avatar.transform-scale` | **was** Glass 100 ms OutCubic, Material 350 ms spring(0.3); now linear / 290 ms spring(0.57) |
| 13 | Glass ring while sending (ProgressRing 3/2); Material wavy bar 64 under it (indeterminate while waiting) | 291-330 | `Ring`, `WavyBar` | same (wave drawn as dots) |
| 14 | Ended badge 22 r 11 bottom-right: sent primary/success ✓, else surfaceContainerHighest/secondary ×; 2 px border | 301-318 | badge | same |
| 15 | Name 88 wide, 2 lines, small 500; status caption (Sending… n %, Sent, Declined, Didn't send, Cast, Quick Share, LocalSend) | 331-353 | `device_status` | same (line height 1.0, not 0.95; no tabular figures) |
| 16 | Click sends unless sending; Cast closes (shell side) | 354-362, 58-65 | `send` | same |
| 17 | Looking: Glass 3 circles pulsing at 15 fps; Material Spinner 48, or cookie9 + share icon when it can't look; texts + ports error | 368-430 | empty column | same (Busy arc for the Spinner) |
| 18 | Errors line | 432-440 | `errors` | same |
| 19 | Other ways: Copy / Show in Files / Share with a QR code (text: Copy only); Glass grouped platter, hairlines, hover fill r 12; Material segmented 52 px, outer 16 inner 4, 8 %/10 % layer; act, then close | 442-529 | `WayRow`, `way` | same; the list is now one `Rows` (was a new model per feed) |

## Test plan (tester)
Arena 1920×1080, sandbox (`TESSERA_SANDBOX=1`). Prepare: `inbox 'ipc sharingdemo peers'`. Open: `inbox 'ipc panels shareFiles /usr/share/pixmaps/archlinux-logo.png'` (any picture path in the arena).
Glass geometry (4 devices): card x 750..1170, top ≈ 352; × centre (1139, 382); avatars centre y ≈ 486, x 812 / 908 / 1004 / 1100; ways centres y ≈ 602 / 646 / 690 (x 960).
Material geometry: card x 744..1176, top ≈ 330; × (1132, 374); avatars y ≈ 460, x 814 / 910 / 1006 / 1102; ways y ≈ 592 / 646 / 700. (Take exact y from a first shot.)
1. **Open/close**: `rec sh-open 0.6 "ipc panels shareFiles /usr/share/pixmaps/archlinux-logo.png"` → fade + scale 0.94→1 in 220 ms; `rec sh-close 0.4 "wtype -k Escape"` → 150 ms. QML vs Rust ± 1 frame, both looks.
2. **Progress keeps the rows** (fix 10): reopen; `P glide 1004 486 200` (device 3 hovered: Glass 88 %, Material cookie12); then `inbox 'ipc sharingdemo sending 10'`, `sending 40`, `sending 80` → device 1 shows ring/wave and "Sending… n %"; device 3 stays hovered throughout (before: the hover dropped on each tick). `rec sh-tick 1 "ipc sharingdemo sending 60"`: no frame where every avatar blinks.
3. `inbox 'ipc sharingdemo sent'` → device 1 badge ✓, "Sent" in success (Material primary).
4. **Press** (fix 12): Glass: `P move 908 486`, `rec sh-press 0.3 "P down"` → shrinks to 0.94 linearly over 100 ms; then `P glide 960 950 100`, `P up` (released off it: no send). Material: same → swells to 1.06; on release it settles in ≈ 0.29 s with a visible overshoot.
5. **× press** (fix 7, Material): `P move 1132 374`, `rec sh-x 0.3 "P down"` → corners jump to 12 at once (no tween) while it swells 1.06; `P glide 960 950 100`, `P up`.
6. **Keys**: `wtype -k Tab` → ring on ×; `-k Tab` → device 1; `-k Right` ×3 → device 4; `-k Tab` ×2 → "Show in Files"; `-k Escape` closes.
7. **Looking**: `inbox 'ipc sharingdemo none'`, reopen → Glass three pulsing circles / Material spinner, "Looking for nearby devices…".
8. **Text**: copy some text, `inbox 'ipc panels shareSelection'` → title in quotes, "Text · sent as a text file", one way (Copy).
9. Real devices / real sends: not testable here (no peer). Afterwards `inbox 'ipc sharingdemo real'`.

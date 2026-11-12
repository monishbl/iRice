# shell/modules/shelf/ShelfDropZone.qml → surfaces/shelf (src/lib.rs `show_zone`, `hide_zone`, `drag_zone`; src/logic.rs `in_zone`, `target_at`; ui/shelf.slint `ShelfZone`, `Target`)

springFast 12.8/0.54 → ω 28.3, ζ 0.60 → 222 ms spring(0.4).

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | While Shelf.zone (+200 ms); overlay, namespace tessera:shelf-zone, top, margin 4, 560 wide; input on the zone only | 15-43 | `zone_window`, input region | same |
| 2 | In: 320 ms × motionScale OutBack overshoot 1.2 (OutCubic when motion is scaled); out 180 ms OutCubic; opacity + scale 0.72 → 1 from the top | 45-67 | `animate shown` | **was** cubic-bezier(0.34, 1.45, …) (≈ 6.6 % over); now (0.34, 1.4, 0.64, 1), ≈ 5.3 %. Cool-down's motionScale isn't in the shelf's feed (known) |
| 3 | Zone 520, r 28 (Glass 32); Material surfaceContainer / Glass black with 12 % white border | 56-64 | `zone` | same |
| 4 | A drag anywhere on it → Shelf.zoneHover; a click on it → cancelZone; a drop between the targets does nothing | 69-84 | `drag_zone`, `set_over`, TouchArea | same |
| 5 | Head: tray-arrow-down 18 filled, "Drop to keep or send" 15 px 600, "n on the shelf"; × 28 round (Material 8 % layer on hover, Glass 10 % → 20 % white), colour Theme.motion.hover, hit area 8 px bigger | 93-145 | head, `close` | **was** colour 150 ms hard-coded; now `Theme.hover` |
| 6 | Targets 236 × 116, 12 apart: Material tonal / primaryContainer + 2 px primary under the drag, corners 20 → 28 on springFast; Glass 8 % white / accent 38 %; scale 1.03 under the drag (Theme.motion.hover OutCubic); colour Theme.motion.hover | 168-212 | `Target` | **was** radius 200 ms spring(0.3), colour/scale 150 ms hard-coded; now 222 ms spring(0.4), `Theme.hover` |
| 7 | Drop on Keep → Shelf.keep(urls, text); on Send → Shelf.sendOn(urls) | 149-163, 213-221 | `drag_zone` Drop | same |

## Test plan (tester)
Arena 1920×1080, sandbox. Zone x 700..1220, y 4..188; × (1190, 31); Keep target centre (834, 114); Send (1082, 114).
1. **In/out** (fix 2): `rec zone-in 0.6 "ipc shelf zone"` → scale 0.72 → 1 from the top centre, ≈ 5 % over near 0.18 s, settled at 0.32 s; compare QML vs Rust scale curve. `P click 960 30` (on the zone) → out in 180 ms.
2. **× hover** (fix 5): `inbox 'ipc shelf zone'`, `P glide 1190 31 200` → fill fades in over 150 ms (Glass 10 → 20 % white; Material 8 % layer). `P click 1190 31` → gone.
3. **Targets under a drag** (fix 6): with an item on the shelf, `inbox 'ipc shelf zone'`, take a shelf row out (`P down` on a row, `P glide 834 114 600`) → Keep lights (Material: corners round to 28 in ≈ 0.22 s with slight overshoot, primary outline; Glass accent 38 %), grows to 1.03 over 150 ms; `P glide 1082 114 300` → Send lights, Keep returns. `P up` over Send → the share sheet opens (sendOn). If no item can be put on the shelf in the arena, use a file manager's drag or mark not testable here.
4. Drop between the targets (`P up` at (958, 114)) → nothing happens, the zone stays until the drag ends.

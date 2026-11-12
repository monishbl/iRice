# shell/modules/screentools/RegionSelector.qml → surfaces/screentools (src/region.rs, src/geometry.rs, ui/screentools.slint `RegionSelector`)

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | Shown while ScreenTools.selecting is set, on the selecting screen: full-screen Overlay, keyboard Exclusive, namespace tessera:region | 20-27, 60-71 | `select` | same |
| 2 | Window ready when a selection starts | 60-62 (LazyLoader) | `make_ahead` (M::Prepare, 1.5 s after start) | **was** made on the first selection (first ⇧⌘4 late); now made ahead on the focused output |
| 3 | Dims 35 % black everywhere but the selection (four rectangles) | 107-113 | four Rectangles `#00000059` | same |
| 4 | Box: 1 px white (Material 2 px primary) 1 px outside the selection; r 0 while dragging, else 12 (Material 16); not for a circle stroke | 114-125 | `show-box` | same (Squircle corners drawn round) |
| 5 | Circle to Search: the stroke from the first pixel, 5 px accent over a 14 px white glow at 45 %, round caps and joins; a point every ≥ 2 px | 128-145, 188-193 | Paths, `on_moved_to` | same |
| 6 | Stroke kind and its box (strokeKind; strokeBox padded 12) | 82-86 | `geometry::stroke_kind` / `stroke_box` | same |
| 7 | Size "W × H" right-below the selection (+8, kept 8 inside), 22 high pill #1a1a1f d9 (Material 28, primary / onPrimary), small 500; not while circling | 148-163 | `show-size` | same (no tabular figures in Slint) |
| 8 | Hint pill 48 from the top, centred, 32 high (Material 48 in inverseSurface, body size), text per mode | 165-178 | hint | same |
| 9 | Hovering highlights the window under the pointer (Hyprland windows on the showing workspace) | 73, 194-196 | `windows_on`, `on_moved_to` | same (where windows overlap Rust takes the most recently focused; QML the first in Hyprland's list) |
| 10 | Press starts (clamped); the right button cancels; release: a drag > 4 px (circle: kind ≠ none) selects, else the hovered window | 198-221 | `on_down` / `on_up` | same |
| 11 | Esc cancels; Return takes the selection, or the whole screen | 180-185 | `on_key` | same |
| 12 | Under 4 px either way: ignored; else hide, wait 160 ms, then capture | 46-56, 92-101 | `finish` | same |
| 13 | Crosshair cursor | 190 | `mouse-cursor: crosshair` | same |

## Test plan (tester)
Arena 1920×1080, output IRICE-TEST, both shells, both looks. Open two windows first (`inbox 'foot -T A >/dev/null 2>&1 &'`, then `foot -T B`), wait 2 s, `P move 100 900`.
1. **First selection is immediate** (row 2): fresh shell, wait 3 s, `rec rs-first 1 "inbox 'ipc screen region screenshot'"` → dim and hint no later in Rust than in QML (± 1 frame).
2. **Drag** (rows 3, 4, 7): `inbox 'ipc screen region screenshot'`, `P glide 400 300 200`, `P down`, `P glide 900 650 400`, `shot rs-drag` → dim outside 400..900 × 300..650, square outline (1 px white / 2 px primary), readout "500 × 350" at (908, 658). Compare QML vs Rust. `rec rs-up 1 "P up"` → the overlay goes at once, the screenshot thumbnail appears ≈ 160 ms later.
3. **Window click** (rows 9, 10): `inbox 'ipc screen region ocr'`, glide onto window A's centre → its rect outlined (r 12 Glass / 16 Material), the rest dimmed; hint "Drag to select an area to copy text, or click a window. Esc to cancel.". `wtype -k Escape`.
4. **Circle** (rows 5, 6): `inbox 'ipc screen circle'`, `P glide 800 500 100`, `P down`, glide through (1000, 400) (1200, 500) (1000, 650) (800, 520), 150 ms each, `shot rs-circle` (stroke + glow, no box, no size), `P up`. The search itself is **not testable here** (no network): check the Rust log asks `screen.captured ["circle", …]`.
5. **Keys** (row 11): open, `wtype -k Escape` → gone; open, `wtype -k Return` → the whole screen captured.
6. **Right button** (row 10): open, press the right button at (960, 540) (if the pointer helper has no right button, mark not testable).

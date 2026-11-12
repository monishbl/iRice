# shell/modules/emoji/PickerButton.qml → surfaces/emoji (ui/emoji.slint `PickerButton`)

springFast 12.8 / 0.54 → ω 28.3, ζ 0.60 → 222 ms spring(0.4); springPress 7.5 / 0.3 → ω 21.7, ζ 0.43 → 290 ms spring(0.57).

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | Size: label + 32 (Material + 40), 36 tall (Material 40); label body 500 | 102-103, 114-120 | `PickerButton` | same |
| 2 | Colours: Glass accent (darker 1.1 hover, 1.2 pressed) / fill (fillHover, fillPressed); Material primary or secondaryContainer with 8 %/10 % state layers of its content colour; on hover duration (150 ms) | 101, 108-112 | `background` | same (Slint `darker(0.1/0.2)` ≈ Qt.darker 1.1/1.2) |
| 3 | Material corners: pill, 12 px while pressed, on springFast | 106-107 | `animate border-radius` | **was** 200 ms spring(0.3) (core TextButton); now 222 ms spring(0.4) |
| 4 | Glass press: scales to 0.97 over Theme.motion.press (100 ms) | 110-111 | `squish` | **fixed** (was missing) |
| 5 | Material press: swells min(6 %, 10 px) while held, springs back past its size on release (ElasticControl on springPress) | 104-105, ElasticControl | `swell` 290 ms spring(0.57) | **fixed** (was missing) |
| 6 | Pointer cursor; click → clicked | 122-128 | `touch` | same |

## Test plan (tester)
Arena 1920×1080, panel centred x 750..1170, y 326..754. Without a KLIPY key, the GIF tab (1141, 398) shows two buttons centred under the text: "Open Settings" (primary) and "Get a key", ≈ y 640 (read their exact boxes from the first frame; Glass row ≈ x 849..1071).
1. **Glass**: `P glide <Open Settings centre> 200` → background darkens in 150 ms. `P down`, `rec em-btn-g 0.3 ""` → shrinks to 97 % in 100 ms; `P up` → back. Don't release over it if you don't want Settings to open (glide off first, then `P up`).
2. **Material**: `P down` on "Get a key", `rec em-btn-m 0.6 ""`, `P up` → corners 20→12 on a ≈ 0.22 s spring, the button grows ≈ 6 % with a small overshoot (≈ 0.3 s), and on release overshoots below/above 1.0 once and settles. Compare QML vs Rust strips.

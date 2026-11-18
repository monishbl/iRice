# shell/modules/overview/WindowPreview.qml → core/ui/wm.slint `WPreview` (+ surfaces/overview)

Rust: `WPreview` in `native/tessera-ui/core/ui/wm.slint` (drawing; only the Overview uses it),
pictures and the failed set from `tessera_core::wm::Thumbs` (surfaces/overview/src/lib.rs `rebuild`).

## Behaviours

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | Clipped card, radius from the Overview (4 / 13 full Glass; 12 / tile Material), colour surfaceContainerHighest M / #2C2C31 dark / #FFFFFF light | 38-42 | WPreview inner Rectangle | same |
| 2 | Picture fitted to the miniature (frames no bigger than drawn: constraintSize) | 43-49 | `thumbs.ask(.., max = size × scale)` | same |
| 3 | Picture fades in 150 ms OutCubic when it arrives | 50-51 | `animate shown` | **was** `ease-out` (CSS 0,0,.58,1); now cubic-bezier(.33,1,.68,1) (OutCubic) |
| 4 | Snapshot: once per open; live (≤ 30/s) via LiveCapture when `live` | 46, 56-59 | `Thumbs::set_live`, `LiveTick` | same |
| 5 | Reopened before unloading: a fresh picture (recapture) | 31-36 | `thumbs.retake()` on open | same |
| 6 | Loading / failed: app icon in the middle, size round(clamp(12, 32, h × (roomy ? 0.4 : 0.7))), roomy = h ≥ 56 && w ≥ 72 | 61-72 | `mid`, `roomy` | same |
| 7 | App name under it (12 px, 500) when roomy and the capture failed or there is no source | 73-81 | `if roomy && failed` | same (no source counts as failed in Thumbs) |
| 8 | Loaded: icon badge bottom-right (margins 4 G / 8 M), size round(clamp(12, 20, h × 0.22)), only if w ≥ 40 && h ≥ 32; Material on a round chip (badge + 8, surfaceContainerHigh) | 84-101 | badge Rectangle | same |
| 9 | Outline: Glass 1 px (#36363C / #E4E4E7), hovered 2 px accent; Material none, hovered 3 px accent | 103-111 | outline Rectangle | same |
| 10 | Icons: Apps.iconFor(class) (the user's icon theme: Colloid) | 71, 100 | `AppIcons::get(class, size, scale, styled)` | same (styled = Material's themed tile when the look asks) |

## Test plan (tester)

Run with the Overview in **grid** style (`use_config` overview.style = "grid"), both shells, both looks,
3 foot windows on space 1 (see the Overview plan for geometry: cell 1 spans x 162..462, y ≈ 444..612).

1. **Fade-in** (T3): `rec wp-fade 1 "ipc panels open overview TESSERA-TEST"`; crop to cell 1. Each window shows
   its icon first (if the picture is late) and the picture fades in over ~150 ms (count frames: 9 ± 1 at 60 fps).
2. **Badge** (T8): `dev shot` after 1 s: bottom-right of each miniature the foot icon (Colloid), ~20 px
   (Material: on a round chip). A tiny miniature (< 40×32) has no badge.
3. **Hover outline** (T9): glide onto a miniature: outline turns accent (Glass 2 px, Material 3 px), off: back.
4. **Failed** (T7): a window with no export (e.g. an Xwayland window if available, or kill the foot right after
   opening the Overview): its icon stays in the middle with "foot" under it when the miniature is ≥ 72×56.
5. **Live** (T4): previews = "live": type in a foot (`wtype` into it while the Overview is open — focus it first
   with `hyprctl dispatch focuswindow`, then open): its miniature updates (≤ 30/s). Snapshot: it doesn't.

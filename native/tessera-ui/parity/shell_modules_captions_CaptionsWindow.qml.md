# shell/modules/captions/CaptionsWindow.qml → surfaces/captions (src/lib.rs, ui/captions.slint)

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | Shown when enabled and not in the island; on the display in use | 20-30 | `sync`, `screen` | same |
| 2 | Bar: floating above the Dock, centred until moved, clamped to the screen; dragged anywhere, place kept | 72-160 | `bar_place`, `place` | same |
| 3 | Strip: across the bottom, windows make room | 171-215 | strip surface (exclusive zone) | same |
| 4 | Video: over the bottom of the playing / focused window, up to 3 lines, click-through | 218-265 | `video_place`, empty input region | same |
| 5 | Dims after a quiet while: bar 35 %, strip 50 %, video hidden; opacity 500 ms OutCubic | 29, 115-116, 184, 247-248 | `animate opacity { 500ms OutCubic }` | same |
| 6 | Bar and strip: what's said kept to two lines, the sentence before elided at its start | 120-140 | `tail`, `fit` | known difference: Slint elides only at the end, so the start is cut by character count |

## Test plan (tester)
For each placement (`use_config` captions placement bar / strip / video):
1. `inbox 'qs ipc call captions on'`, then `demo "Before." "Now speaking"` → shown in that place (bar centred above the Dock; strip full width at the bottom with windows shrunk; video over the focused window's bottom).
2. Wait for the quiet dim → record 1 s: bar to 35 % / strip 50 % / video gone over 500 ms OutCubic; compare with QML ± 1 frame.
3. Video: click on the captions → the click reaches the window below.
4. Bar: long text → two lines, older words cut at the start.

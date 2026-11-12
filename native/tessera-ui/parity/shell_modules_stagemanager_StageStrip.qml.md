# shell/modules/stagemanager/StageStrip.qml → surfaces/stagemanager (src/lib.rs, ui/stagemanager.slint)

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | Strip at the left, stages 140 × 104 stacked 24 apart, centred vertically; up to 6 waiting stages of the active space | 30-46 | `update`, `VerticalLayout` | same |
| 2 | A stage: its last three windows back to front (thumbnails, live when previews are live and the strip isn't covered), the front app's icon 36 px and name | 52-120 | `StCard`, icon, label | same |
| 3 | Hover 1.04, press 0.96, on `hover` (150 ms) OutCubic | 50-51 | `transform-scale` | **was** 150 ms literal; now `Theme.hover`. Known difference: Material's extra ElasticControl swell on press isn't added on top |
| 4 | Click brings the stage in; Shift+click adds its windows | 120-140 | `activate(id, shift)` | same |
| 5 | Thumbnails and live frames arrive into the stage already shown | — | `stages` + one card list per slot | **was** every thumbnail and live tick (200 ms) made every stage and card again, so a hovered stage's scale started over; now `Rows`, set in place |

## Test plan (tester)
Both looks. Turn Stage Manager on (`use_config` stageManager enabled, or its Control Center toggle), open 3 apps in turn so 2 stages wait at the left.
1. Shot: stages at x 20, 140 × 104, icons and names under the thumbnails.
2. With live previews on (`use_config` overview previews "live"): hover a stage and hold 3 s → it stays at 1.04, steady (before: it twitched back every 200 ms). Record 3 s, check no scale reset.
3. `P down` → 0.96; `P up` → that stage comes in.

# shell/modules/spotlight/Spotlight.qml → surfaces/spotlight

Spotlight.qml only picks the look's drawing: SpotlightGlass (Glass) or SearchView (Material).

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | One "spotlight" panel; Glass draws SpotlightGlass, Material SearchView, each made only in its look | 7-10 | one `SpotlightPanel`; `Theme.material` switches its drawing; `look()` on a look change | same |
| 2 | Opening draws at once | — (PanelLoader builds it on first open) | the window is now made 1.5 s after start (`M::Prepare`) and kept | better than QML (was made on the first open) |

## Test plan (tester)
1. Both looks: `inbox 'ipc panels open spotlight IRICE-TEST'` → Glass: the floating glass field; Material: the 56 px pill in the docked view. Switch look and reopen: the other drawing.
2. Right after `dev reload` + 3 s: `rec sp-first 1 "ipc panels open spotlight IRICE-TEST"` — first moving frame no later than on a second open, and no later than QML's (`anim-timing.py`).

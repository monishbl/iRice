# iRice: later

## Performance: panel animations (+25% CPU vs end4)

Measured 27 Oct 2026, plugged in, Balanced. Opening and closing six panels uses 15.9% shell CPU for iRice and 12.7% for end4. iRice already wins on idle, typing, frame stalls and memory.

Where the time goes: keeping each glass shape in sync with the compositor every frame while a panel animates (`glass/Glass.qml` `pluginShape`, `glass/GlassWindow.qml` `syncGlass`), plus building each panel when it opens.

Built on 27 Oct, not yet benchmarked: run 3 plugged-in rounds of `VARIANTS="end4 irice-after irice-qml irice-keep" bench-power.sh charging5`.

- [x] **Skip unchanged frames** (QML fallback): the per-frame sync stops after 8 frames where nothing moved, instead of always running 400 ms.
- [x] **Keep panels loaded**: Settings → Appearance → Performance (off by default). Spotlight, Control Center and Notification Center stay loaded after their first open.
- [x] **Glass sync in C++** (`shell/native`, QML module `IRice.Native`, type `GlassSync`).
  - It runs on each frame a glass window actually draws (`afterAnimating`), so it does nothing while idle and has no 400 ms polling.
  - Requests go out from a worker thread straight to Hyprland's socket.
  - The QML path stays as the fallback. `IRICE_NO_NATIVE=1` forces it.
  - The log says which path is in use ("Glass sync: native").
- [x] **C glass helper** (`shell/native/glass-ipc.c`), used whenever it's built; the Python helper is the fallback.
- Built by `shell/native/build.sh` (needs cmake and Qt 6 dev files), by the installer, and by `irice rebuild` after a Qt update.

Benchmark: the scratchpad `bench-power.sh <label>` (3 alternating rounds, end4 vs iRice), with the test screen focused first. Check `powerprofilesctl get` stays the same for the whole run.

## Battery watts after the 27 Oct optimisations

Not measured yet. Start 3 battery rounds above 40% charge. Below about 25% the system switched to power-saver, which skews everything.

## Island

- The island lives in the menu bar's 340 px window, so expanded alerts are capped by that height.
- A taller window would let 4 full cards show, but costs a larger layer on every frame. Worth measuring.

# shell/modules/hotcorners/HotCorners.qml → surfaces/hotcorners (src/lib.rs, ui/hotcorners.slint)

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | A 3 px transparent layer surface in each corner with an action; "none" corners have no window | 10-12, 75-100 | `corners`, `NS` tessera:hotcorner | same |
| 2 | Rest in the corner for `delay` ms (default 200) → run its action (overview, missionControl, desktop, notificationCenter, launchpad, lock, standby, quickNote) | 9, 22-60 | `dwell` timer → `hotcorners.run` | same (the action and the lock check stay in the shell) |
| 3 | Nothing while the monitor shows a fullscreen window, while locked, or while a button is held; a press in the corner cancels | 14-17, 101-120 | `pressed`, `allowed` | same |

## Test plan (tester)
Set top-left to "missionControl" (`use_config` hotCorners topLeft "missionControl"), delay 200.
1. `P glide 0 0 200` and rest 300 ms → Mission Control opens. Glide away and back for only 100 ms → nothing.
2. `P down` in the corner, rest → nothing.
3. A fullscreen window on that monitor → resting does nothing.

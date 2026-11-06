## net group: CORE-NEEDS

Nothing is needed in core to compile or run this group. What other crates must provide (not core edits):

1. host-shell Island: answer `island.post` with `{"event": {…}, "ms": n}`, where an event may carry `action`/`actionCall`/`actionArgs` and `action2`/`action2Call`/`action2Args`: when pressed, the island calls `ask(actionCall, actionArgs)`.
2. host-shell: set shell state `open` (Panels.open: "wifiMenu" while the Wi-Fi menu shows) and `anchorX` through `Host::set_state`; answer `sound.play`, `panels.openSettings`, `backends.fix`.
3. sysinfo group: answer `system.power ["sleep"|"lock"]`; put `keepAwake` and `gameMode` on the bus under "system".
4. Surfaces that want a resource sample or the hover's delays call `resources.use` / `resources.hover` (the Rust surfaces mostly sample themselves).

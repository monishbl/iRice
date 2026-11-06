# host/shell: CORE-NEEDS

1. **Surfaces' stdout events never reach the host** (core/src/shell.rs `tell`). Add at the top of `tell`:
   `let mut e = fields.clone(); e["kind"] = json!(kind); if crate::host::route_ask("ui", &e) { return; }`
   (and `closed()` goes through it). host-shell answers `ui` (closed, osd, session, lock events, ready). Other kinds (notifications...) need a general `Service::ui(&mut self, h, kind, &Value)` hook delivered to every service.
2. **Global shortcuts** (DONE without core changes: hypr/tessera/keybinds.lua binds `tessera-ui ipc call shortcut <name>` when `rust.standalone` is set, TESSERA_STANDALONE=1 forces it, =0 forbids it, and the binary is found; release binds are `superTapRelease` and the `release` names such as `dictateRelease`). Originally: hyprland-global-shortcuts protocol client in core, or change hypr/tessera/keybinds.lua `hl.dsp.global("tessera:x")` to `exec tessera-ui ipc call shortcut x` when standalone (release binds: `xRelease`).
3. **Idle notifications** (ext-idle-notify) for the lock's StandBy and the face/finger wake (Lock.qml IdleMonitor).
4. **Hyprland link repair**: a `hypr::refresh()`/reconnect for HyprlandLink's drift handling.
5. Glass plugin `window-frost` command in `hypr::Glass`, and plugin version.
6. `Service::data` hook (or a read API) so services see Data changes; today the island polls the battery every 3 s.

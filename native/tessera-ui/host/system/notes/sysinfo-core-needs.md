# CORE-NEEDS (sysinfo group)

Nothing is needed in core, a surface, or main.rs. What other crates must do
for this group to work standalone (host-shell / the other host crates):

1. host-shell's settings router: Settings asks `shell::ask("settings",
   ["act", group, what, …args])`. For group `system` and `about` forward as
   `h.ask("system.<what>", [args…])` / `h.ask("about.<what>", [args…])`; and
   tell them when Settings opens/closes and changes page: `h.ask("system.live",
   [bool])`, `h.ask("about.live", [bool])`, `h.ask("system.page", [id])`,
   `h.ask("about.page", [id])` (the QML RustBridge's `live` and
   Panels.settingsPage).
2. Calls this group makes that host-shell must answer (prefixes it owns):
   `island.post {event, ms}`, `panels.close`, `panels.openSettings [page]`,
   `lock.set [bool]` (System.locked), `uistate.set ["caffeine", value|null]`
   (UiState.data.caffeine; the file is $XDG_STATE_HOME/tessera/state.json,
   which System reads itself at start), `launch.open [url]`, `welcome.request`,
   `focus.reload`, `tools.recheck`.
   Island: an alert's `onAction: {call, args}` must be asked when its
   button is pressed (`about.reboot`, `panels.openSettings ["update"]`).
3. host-shell's lock service should tell System: `system.locked [bool]`
   (unlock) and `system.lockSecure [bool]` (the compositor confirmed the lock).
4. The net group's PulseEvents may forward lines as `privacy.pulseEvent
   [line]`; not required (Privacy runs its own `pactl subscribe`).
5. Other groups publish the bus keys listed in PORT-NOTES (sysinfo.md,
   "Contract") for the Settings System/About feeds.

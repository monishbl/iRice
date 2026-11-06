# host/system: CORE-NEEDS

Nothing is needed to compile. At runtime, other owners must provide the following.

## from the net group

## net group: CORE-NEEDS

Nothing is needed in core to compile or run this group. What other crates must provide (not core edits):

1. host-shell Island: answer `island.post` with `{"event": {…}, "ms": n}`, where an event may carry `action`/`actionCall`/`actionArgs` and `action2`/`action2Call`/`action2Args`: when pressed, the island calls `ask(actionCall, actionArgs)`.
2. host-shell: set shell state `open` (Panels.open: "wifiMenu" while the Wi-Fi menu shows) and `anchorX` through `Host::set_state`; answer `sound.play`, `panels.openSettings`, `backends.fix`.
3. sysinfo group: answer `system.power ["sleep"|"lock"]`; put `keepAwake` and `gameMode` on the bus under "system".
4. Surfaces that want a resource sample or the hover's delays call `resources.use` / `resources.hover` (the Rust surfaces mostly sample themselves).

## from the input group

# CORE-NEEDS for the input group

Nothing outside the crate is required to compile. For it to work standalone, other owners need to
honour the contracts in `input.md`:

- host-shell (settings RustBridge): send `input.live`/`devices.live` [bool] when Settings opens/closes,
  and `devices.page [page]` on every page change; forward `settings.act` groups "input" and "devices"
  as `input.act`/`devices.act` [what, …args]. Forward Panels' `open`/`locked` through `set_state`
  ("open", "locked"), which Mouse reads. Answer `island.post`, `sound.play`, `keybinds.refresh`.
- host-desktop: `spaces.arrive [screen]`, `dictation.*`, `screentools.installOcr`; bus keys
  `screentools`, `dictation`, `files`. Whoever draws DisplayConnect reads bus `displayfx` and calls
  `displays.finish [id]`.
- net group: bus `cooldown` is host-shell's (CoolDown); net publishes `feed.wifi`, `feed.backends`,
  `feed.speed`, `feed.speedTest`, `feed.ddc`, bus `wifijoin`/`needs`/`health`/`bluetooth`, and answers
  the `network.*`, `wifijoin.*`, `wifishare.*`, `speedtest.*`, `brightness.setDdc|rescanDdc`,
  `audioprofiles.refresh`, `backends.*`, `needs.*` asks.
- host-sharing: `feed.airplay`, `feed.mirroring`, `feed.phone`, `feed.sharing`, bus `quickshare`/`fastpair`,
  and `airplay.*`, `mirroring.*`, `phone.*`, `quickshare.turnOnBluetooth`, `linkshare.receive`,
  `sharing.shareSelection`.

## from the sysinfo group

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

## from the accounts group

## accounts group CORE-NEEDS

1. **Idle reading (core).** ScreenTime (2 min), Google/CloudAccounts (5 min) need "idle, respecting inhibitors" (IdleMonitor). Nothing in core has it. Needed: a service `services::idle` using ext-idle-notify-v1 (respect inhibitors) that publishes a bus value; until then the group reads `crate::bus::get("idle")["isIdle"]` (false when absent, so the clock always runs and syncs are never skipped for idleness). Exact code to add anywhere in host/shell (it owns the loop): on idle/resume `tessera_host_system::bus::put("idle", json!({"isIdle": bool}))`, with timeouts of 120 s and 300 s (two notifications).
2. **UiState writes (host-shell).** ScreenTime writes the `screenTime` key of state.json directly (read-modify-write, atomic). If host-shell ports UiState with its journal/merge (assets/scripts/state-merge.py), it should expose `uistate.set {key, value}` and this group will switch to it (`screen_time.rs::save`).
3. **System.locked (sysinfo).** `bus::put("system", {"locked": bool, …})` must carry `locked` (Google/CloudAccounts freshen only when unlocked). Everything else the QML read is listed in notes/accounts.md.
4. **Settings router (host-shell).** forward `["act","accounts",what,…]` to `accounts.<what>` (see notes/accounts.md); tell `accounts.live [bool]` when Settings opens/closes (optional).

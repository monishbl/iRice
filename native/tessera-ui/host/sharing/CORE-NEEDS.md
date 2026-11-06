# What host/sharing needs from outside its crate

Nothing in core, a surface or main.rs has to change for this crate to build
and for the sharing services to run. The items below are the **contracts with
the other host crates** (host-shell, host-system, host-desktop): messages this
crate sends them, and calls they should send this crate. None is a code change
in core. Where the other side is missing, the feature degrades as said.

## 1. What this crate asks of host-shell (all via `Host::ask`, answered by whoever owns the prefix)

| call | args | what the QML did | if nobody answers |
|---|---|---|---|
| `island.post` | `{"event": {...}, "ms": N}` | `Island.post(event, ms)`. The event is the QML's event object, with `onAction` / `onAction2` as `{"call": "<prefix>.<what>", "args": [...]}` instead of functions (the Island runs it with `Host::ask` when the button is pressed). Colours are `#rrggbb` / `#aarrggbb` strings (dark-mode accents). `key` replaces a card with the same key. `"unlessKind": "device"` = skip if the card with that key is already of that kind (FastPair's "Connected"). No `color` = the theme accent. | no island alerts from sharing, Phone, Shelf, Transfers, FastPair |
| `island.remove` | `{"key": "bt:..."}` | `Island.remove(Island.find(key))` | a stale "Connecting..." card stays until it times out |
| `sound.play` | `["received"]` | `Sounds.play("received")` | silent |
| `panels.share` | `[paths, screen, note]` | `Panels.share(paths, scr, note)`: sets `sharePaths`/`shareNote` and opens the sheet | the sheet doesn't open from the shelf / sent-on text |
| `panels.close` | `[]` | `Panels.close()` | the sheet stays after a cast starts / the QR card |
| `panels.openSettings` | `["phone"]` | `Panels.openSettings("phone")` | - |
| `panels.appStoreOpen` | `[false]` | `Panels.appStoreOpen = false` (the store's close button) | the store's close button does nothing |
| `panels.quickLook` | `[path]` | `Panels.quickLook(path)` | - |
| `launch.openLink` | `[url]` | `Launch.openLink(url)` (web and mail links only) | links from App Store / received links don't open |

Shell state host-shell should publish (`Host::set_state`), read here with `Host::state`:
- `open`: the open panel (`"share"` while the sheet is up). The sheet is "looking for devices" while `open == "share"`.
- `appStoreOpen` (bool): `Panels.appStoreOpen`. Alternatively call `appstore.panel [bool]` when it changes.

## 2. What host-shell should call on this crate

- `Panels.share(paths, scr, note)` -> before opening the sheet: `h.ask("share.paths", json!([paths, note]))`. (This crate also calls `panels.share` itself for the shelf and shared text, and sets the paths first, so those work either way.)
- The Hyprland raw event `tessera-drag` / `irice-drag` (a window is being moved) -> `h.ask("shelf.windowMove", json!([]))`.
- The global shortcuts `pointerDown` / `pointerUp` (left button, hypr/tessera/keybinds.lua) -> `shelf.pointerDown` / `shelf.pointerUp`.
- The menu bar's shelf icon and the keymap's `shelf` action -> `shelf.toggle` (or `shell-ipc.sh call shelf toggle`, the IPC target is answered here too).
- Probes (ShelfProbes.qml's job, which is a surface nobody has ported): read the shell state `shelf.probing` / `shelf.probe` ({x, y, screen}) and call `shelf.dragArrived ["left"|"right"|"top"|"shake", screen]` / `shelf.dragLeft [where]`, and `shelf.keep [urls, text]` for a drop on a probe. Until then the shelf opens from the shortcut, the menu bar and `shelf.offer` only.
- The Island's own "Undo" etc. run whatever `onAction` call this crate sent.

## 3. What host-system / host-desktop should know

Everything below is published as shell state (read with `Host::state("<path>")`) and driven by `share.*` calls, so their bridges need no QML singleton:

- **Control Center / Settings (host-system)**: replace `Sharing.*`, `LocalSend.*`, `QuickShare.*`, `Cast.*`, `Mirroring.*`, `Phone.*`, `AirPlay.*`, `SharingPorts.*` with the state keys `sharing`, `quickshare`, `localsend`, `cast`, `mirroring`, `phone`, `airplay` (each is one JSON object; fields named as the QML properties, e.g. `quickshare.trusted`, `cast.devices`, `mirroring.displays`, `phone.pairStage`, `sharing.ports.error`) and the calls in PORT-NOTES.md ("Calls other hosts make").
- **Island live activity (host-shell)**: `transfers` (`items`, `first`, `progress`, `speed`) = `Transfers.*`; `cast` (`state`, `title`, `time`, `duration`, `device`) = `Cast.*`; `mirroring` (`state`, `targetName`) = `Mirroring.*`.
- **Dock / Batteries widget**: `phone.phone`, `phone.mirrors` (serials), `fastpair.levels.<address>` = `FastPair.levelsFor(address)`.
- **Spotlight (host-desktop)**: `appstore.lookUp [q]`, `appstore.forget`, answer in the state `appstore.quick` ({query, apps, loading, revision}); "Get" opens the store with `Panels.appStoreApp` and a call `appstore.app [app]`.
- **Software Update (host-system's Updates)**: this crate starts `sysupdate.py run <args>` itself (exactly `Updates.update()`'s command) and follows `$XDG_RUNTIME_DIR/tessera-sysupdate/status.json` while its own job runs. host-system's Updates should also read that file when a job it didn't start is running (the QML only polls after its own `update()`), or the island/Settings won't show the progress of installs started from here (AirPlay, Mirroring, Phone, App Store, QR card).
- **Audio (host-system)**: AirPlay speakers are PipeWire sinks named `raop_sink.*`; `tessera_host_sharing::airplay::is_airplay(name)` is `AirPlay.isAirPlay`.
- **Panels / SoftwareUpdate**: `Sharing.shareText` / `shareSelection` (Panels.qml, the keymap) -> `share.text [text, screen]` / `share.selection`.

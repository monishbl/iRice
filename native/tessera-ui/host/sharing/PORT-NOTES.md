# host/sharing: port notes (wave 4)

One service, `Sharing` (`src/lib.rs`), holds every module: they lean on each
other (device list, the firewall's needs, the share sheet's feed). It answers
the call prefixes `share`, `shelf`, `appstore` and the IPC targets
`sharingdemo`, `linkshare`, `cast`, `mirroring`, `shelf`. It never opens a
firewall or runs a root tool itself: it runs the same scripts the QML runs
(`sharing-check.py ports`, `tessera-root` through pkexec for `quickshare-trust`
and `phonecam`).

How the QML idioms map:
- `Process` / `StdioCollector` / `SplitParser` -> `proc.rs` (`spawn` for lines, `run` for a whole output, `detach` for execDetached). Long-lived children are started on the loop thread with PR_SET_PDEATHSIG (what `setpriv --pdeathsig` and `Tools.watcher` do).
- `KeptProcess` -> `proc::Keep` (same back-off: firstWait doubling to 5 min, a run of a minute resets).
- `Timer` -> `proc::Sched` (named deadlines, one `slint::Timer` for the nearest).
- Property bindings / `onXChanged` -> `Sharing::settle`, called after every call and message: it re-evaluates what's needed (helpers start and stop) and publishes.
- `Island.post(e, ms)` -> `h.ask("island.post", {event, ms})`; button callbacks are `{call, args}` (CORE-NEEDS.md).
- `RustUi.tell("share", {feed})` -> `Host::tell`, only when the feed changed.

Shell state published (Host::set_state): `sharing`, `quickshare`, `localsend`, `cast`, `mirroring`, `phone`, `airplay`, `fastpair`, `shelf`, `transfers`, `appstore`.

Calls other hosts make (all `share.<what>` + an args array unless said): `paths [paths, note]`, `sheetOpen [bool]`, `look [bool]`, `setLevel [off|10min|everyone]`, `text [text, screen]`, `selection`, `pickFor [deviceKey]`, `fileOpen/fileReveal/openFolder [path]`, `openDownloads`, `openLink [url]`, `copyText [text]`, `portsRetry`, `trustSet [bool]`, `trustCheck`, `bluetoothOn`, `linkReceive`, `transferCancel [item]`, `castPlaying [device]`, `castFile [device, path]`, `castToggle`, `castStop`, `castSeek [t]`, `castVolume [v]`, `mirrorStart [uuid, name]`, `mirrorStop`, `mirrorLook [bool]`, `mirrorInstall`, `phoneMirror [serial]`, `phoneWebcam [bool]`, `phoneSetUpCam`, `phoneCheckCam`, `phonePair`, `phoneCancelPair`, `phoneInstall [withCamera]`, `fastConnect [device]`, `fastRepair [device, known]`, `airInstall`, `airCheck`; `shelf.toggle|show|open|close|undoClear|offer|dragArrived|dragLeft|pointerDown|pointerUp|windowMove`; `appstore.panel [bool]|app [app]|lookUp [q]|forget|users [+1/-1]|openApp [app]`.

Unowned prefixes: none. After integration the one `Sharing` service also answers the QML services' own names that host-system (Settings, Control Center) asks: `airplay.{check,install}`, `phone.{install,mirror,pair,cancelPair,setUpCam,checkCam,setWebcam}`, `mirroring.{look,install,start,stop}`, `linkshare.receive`, `quickshare.{turnOnBluetooth,setTrusted}`, `sharing.{look,setLevel,pickFor,shareSelection}`, `cast.{castPlaying,stop}`, `sharingports.retry`. They are routed to the `share.*` action above by `alias()` in `src/lib.rs` (no second implementation); `mirroring.start` takes a display object or its uuid, `sharing.pickFor` a device object or its key. `island.post` is asked as `{event, ms}` and `island.remove` as `{key}`: host-shell's island accepts both forms.

## Per file

### shell/services/Sharing.qml -> `src/sharing.rs` (+ `lib.rs` helpers)
Ported: level / mixed / levelText / setLevel (writes `sharing.receiving` and `sharing.quickShare`), merged `devices`, `look` counting and the "looking for real" edge (starts LocalSend scanning and Quick Share discovery), `send`, `progressFor`, `pickFor` (runs `pick-file.py --multiple`), `shareText` (private `$XDG_RUNTIME_DIR/tessera-share/Text|Link <date>.txt`, then `panels.share`), `shareSelection` (`wl-paste -p`, else clipboard), `note`, incoming requests (add/find/update/drop, helperEnded, answer, progressOf, shownRequest, cancel), send outcomes (60 s), the `sharingdemo` IPC target (sandbox only).
Differs: `Config.options` writes go straight to config.json (`set_config`; the shell's watcher re-reads it) since there is no Config singleton here. `Panels.share` is `panels.share` (host-shell) after the paths are set here.
Check: Control Center -> Sharing -> Everyone; `tessera-ui ipc call sharingdemo peers` then open the share sheet (a file's Share): four made-up devices; `sharingdemo ask` -> the request card; `progress 40`, `done`, `fail`; `sharingdemo none` / `sending 50` / `sent` / `real`. (Needs TESSERA_SANDBOX=1.)

### shell/services/SharingDevices.js -> `src/devices.rs`
Fully ported (nameKey, typeOf, merge, progressFor, combine) with tests.
Check: `cargo test -p tessera-host-sharing devices`.

### shell/services/SharingPorts.qml -> `src/ports.rs`
Ported: the needs (QuickShare visible / LocalSend receiving / looking / QR card / casting a file), open at once (300 ms), close after 5 s, once at start, error text and exit codes 64/126/127 held until Try again (`share.portsRetry`), retry after Software Update finishes. Never in the sandbox.
Check: Settings -> Security shows the firewall note; switch Sharing to Everyone: the helper `sharing-check.py ports quickshare,mdns,localsend "visible to Everyone"` runs once (watch `ps`), and `none` ~5 s after turning it off.

### shell/services/QuickShare.qml -> `src/quickshare.rs`
Ported: visibility incl. "10 minutes" (config flips to off after 10 min; 1 s after a restart with `10min` stored, like the QML), helper start/stop by need with the 3 s x 2^n (max 60 s) retry, config/discover/send/cancel commands, all events (status, devices, incoming, connected, progress, finished, declined, ended, failed, error, bluetooth, transport, link) with the same island alerts and request-card updates, `received()` (files, links, text, Wi-Fi), trust (`/etc/tessera/quickshare-trusted`, `quickshare-trust on|off` through pkexec), Bluetooth readiness from the BlueZ reading, "Turn on Bluetooth" (`share.bluetoothOn`: BlueZ `Powered = true` on hci0, only on that call).
Differs: `received` text goes to `wl-copy` on stdin (not argv); Wi-Fi password likewise.
Check: phone sends via Quick Share: the request card asks (PIN shown), Accept, ring, Open / Show in Files.

### shell/services/LocalSend.qml -> `src/localsend.rs`
Ported: receiving modes, 10 min, helper by need (not restarted by itself after it died while needed: the QML's binding broke the same way), peers, scan every 4 s while the sheet looks, send/cancel, tracking with speed, all events with the same alerts.
Check: LocalSend app on a phone: it appears in the share sheet; send a file; the ring fills; "Sent to <name>".

### shell/services/LinkShare.qml -> `src/linkshare.rs`
Ported: send/receive/close, stages, expiry, arrivals gathered 1.5 s into one alert, IPC `linkshare receive|close|demo`.
Check: share sheet -> "Share with a QR code" shows the card; `tessera-ui ipc call linkshare receive`; `linkshare close`.

### shell/services/AirPlay.qml -> `src/airplay.rs`
Ported: check (raop module present, unit running) 15 s after start (Startup.later), start/stop as a `tessera-airplay` user unit or the shell's own child (exit 100), error text with the log prefix stripped, install via Software Update, `is_airplay`.
Differs: the speaker list (PipeWire nodes) is audio's (host-system).
Check: Settings -> Sound -> AirPlay on: `systemctl --user status tessera-airplay`.

### shell/services/Cast.qml -> `src/cast.rs`
Ported: Avahi discovery (the QML's `avahi-browse` fallback path; per interface/IP-version keys, IPv4 preferred, sorted by name), castPlaying (MPRIS url) and castFile, one session at a time with the queued second cast, play/pause/seek/volume, failure alert, `servesFile` for the firewall, IPC `cast stop|toggle`.
Differs: tessera-events' D-Bus Avahi path isn't used; the player's position isn't in the Rust MPRIS reading, so "cast what's playing" starts from 0:00; the pause goes by `mpris::control`.
Check: with a Chromecast on the network: share sheet for a video -> the TV's icon; send; the island shows the cast; `tessera-ui ipc call cast toggle|stop`.

### shell/services/Mirroring.qml -> `src/mirroring.rs`
Ported: watcher while looked at or mirroring, start (late answers cleaned up), stop, error alert + stop, install gnome-network-displays, IPC `mirroring stop`.
Check: Control Center -> Screen Mirroring lists TVs (needs gnome-network-displays).

### shell/services/Phone.qml -> `src/phone.rs`
Ported: adb watcher (5 s first wait), arrival alerts, scrcpy mirror per phone (focus the existing window by pid), webcam (`/dev/video42`), virtual camera check and setup (`phonecam on` through pkexec), QR pairing, install of scrcpy / adb / qrencode / v4l2loopback, all with the same strings.
Check: connect an Android phone with USB debugging: "Phone connected" alert with Show Screen; Settings -> Phone.

### shell/services/FastPair.qml -> `src/fastpair.rs`
Ported: watcher while Bluetooth is on and `fastPair.enabled`, offers (once per 10 min per earbuds), battery cards, levels (`fastpair.levels`), connect / repair with the connecting card replaced in place.
Differs: no `picture` on the cards (Devices.pictureFor is host-system's); the island may add it from `icon`.
Check: put Fast Pair earbuds in pairing mode nearby: a card with Connect.

### shell/services/Shelf.qml + ShelfLogic.js + modules/shelf/ShelfFeed.qml -> `src/shelf.rs`, `src/shelf_logic.rs`
Ported: items kept in `$XDG_STATE_HOME/tessera/shelf.json` (300 ms debounce), drop / removeAt / clear (Undo alert) / keep / sendOn, show/hide/toggle, close-soon, the drop zone and its three timers, the pointer follower (`pointer-follow.py` while the button is held, 120 ms in), shake detection, edge holds, stale releases (150 ms) and the 4 s missing-release guard, the feed, IPC `shelf toggle|open|close|clear|zone`.
Differs: the probes (ShelfProbes.qml) are surfaces and not ported here: see CORE-NEEDS.md.
Check: Super+Alt+D (or `tessera-ui ipc call shelf toggle`) shows the shelf; drop files; `shelf zone` shows the drop zone; `shelf clear` then Undo.

### shell/services/Transfers.qml + DownloadTracker.js -> `src/transfers.rs`, `src/download_tracker.rs`
Ported: partial downloads (inotify on the Downloads folder, `find` for sizes every 1 s while one grows, 10 s while they sit), stall rule (20 s), "Downloaded <name>" alert (10 min), copies (`transfers.sh`), LocalSend/Quick Share items, progress/speed, cancel, size/label.
Differs: the QML's FolderListModel is inotify here.
Check: start a browser download: `transfers.items` has it (the island shows it); finish: "Downloaded ..." with Open / Show in Folder.

### shell/services/SseParser.qml -> `src/sse.rs`
Ported with tests. Not used by the sharing services (the Rust AI service has its own streaming); kept as a helper for the others.

### shell/services/AppStore.qml + modules/appstore/AppStoreFeed.qml -> `src/appstore.rs`
Ported: `appstore.py --serve` kept while the window is open (+2 min), queued requests, status, load (discover/category/installed/search with the `aur:` prefix stripped), enrich (`_icon`, `_source`), install/remove (Software Update for repos and AUR, flatpak for Flathub incl. installing flatpak first), done alerts, catalogue install, Spotlight's quick lookup (350 ms debounce), open (desktop entry via `gtk-launch`).
Differs: the theme icon lookup is `core::icon_theme::find`; opening an app uses `gtk-launch <desktop id>` rather than `Launch.app(entry)` (no app-scope wrapper).
Check: open the App Store, Discover loads; search "firefox"; install flows show progress in the island.

### shell/modules/share/ShareFeed.qml -> `src/lib.rs` (`share_feed`, `share_call`)
Ported: sheet (paths, note, icon, devices with sending/ended, cast devices for one media file, available, portsError, errors), request, link, screen; every call the sheet makes (send, copy, reveal, qr, answer, cancel, hide, drop, open, show, linkClose, installQr).
Differs: `motion` (CoolDown) isn't sent.
Check: Files/Shelf "Share" opens the sheet with nearby devices.

## Tests
`cargo test -p tessera-host-sharing` (run by the user): devices, download tracker, shelf logic, SSE, transfers' size text, Cast's TXT parser, AirPlay's helpers, FastPair batteries, App Store icons.

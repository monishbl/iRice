# host/system: port notes

Status: all 44 files written; NOT type-checked (the user builds and tests). Likely first-build errors: zbus calls in net/power.rs, libc::prctl in net/common.rs, is_none_or (Rust 1.82+), cross-module spawn_capture/borrow of Host in input/, mixed json! blocks in sysinfo/.

Unowned prefixes: none found in the system list. After integration the asks this crate makes reach their owners (sharing answers `airplay|phone|mirroring|linkshare|quickshare|sharing|cast|sharingports` through aliases; host-desktop answers `dictation|screentools|calendar|calls|ai.*`; host-shell answers `tools|uistate|shellpriority|backends.*|needs.turnOnAvahi|bugreport.*|welcome.request`). New arms here: `network.<what>` (dotted form, without the first word), `network.setWifi|joinWifi`, `brightness.setDdc|rescanDdc|hush`, `audio.hush`, `power.cycleProfile`, `keymap.exportTo|previewFile|previewEnd4|applyPreview|restore` (answering by asking `about.ieExported|iePreview|ieApplied`), and `system.toggleCaffeine|cyclePowerProfile|toggleMic|installUpdates|update|checkUpdates|bootRefresh|bootSet|previewLogin|setAppearance|setNightLight|installEmoji`. Cross-group contracts (bus keys and internal asks) are in each section below; the host-shell and host-desktop owners must answer the asks named there.

---

## net group (host/system/src/net/)

Shared: `common.rs` (island_post, island_wants, has_tool, Watch = a process read line by line with PDEATHSIG, capture_then, state_dir = Paths.state), `power_policy.rs`. Services talk through `crate::bus` (state, JSON under the service's name) and `Host::ask("<prefix>.<fn>", args)`. Every state described below is put on the bus under the QML singleton's name (e.g. `bus::get("SpeedTest")`, `"Power"`, `"Brightness"`, `"Audio"`, `"WifiJoin"`, `"WifiShare"`, `"AudioStreams"`, `"AudioProfiles"`, `"BatteryHealth"`, `"Resources"`, `"NetworkAlerts"`, `"pulseevents"`) with the QML's field names.

Contracts with other groups (all through `ask`, so nothing is shared at compile time):
- island alerts: `island.post` with `{"event": {...}, "ms": n}`. The event is the QML's (kind, icon, title, subtitle, color as a hex string, ripple). An action button, which in the QML is `onAction: () => ...`, is `action` (label) + `actionCall` + `actionArgs` (the call the island makes when pressed); `action2`, `action2Call`, `action2Args` likewise. host-shell's Island must implement this.
- `sound.play ["battery"]` (Power's critical-battery warning), `system.power ["sleep"|"lock"]` (Power: lid, critical battery), `panels.openSettings ["network"]`, `backends.fix ["networkmanager"]` (the Wi-Fi menu's notice button), `osd` tells `{"hush":"sound"|"brightness"}`.
- From the sysinfo group, read off the bus: `bus::field("system","keepAwake")` and `"gameMode"` (System.keepAwake / System.gameMode), used by Power.
- Shell state read: `open` ("wifiMenu" = the Wi-Fi menu is open) and `anchorX` (Panels.anchorX), set by host-shell with `Host::set_state`.
- For others to use from the net group: `power.setMode [mode]`, `power.ask [{owner,mode,why,until}]`, `power.release [owner]`, `power.energyUsers [delta]`, `power.clearEnergy`, `brightness.set [v, quiet]`, `brightness.setFor [id, v, quiet]`, `audio.setVolume [v, quiet]`, `audio.setMicVolume`, `audio.toggleMute [quiet]`, `audio.toggleMic [quiet]`, `audiostreams.move [streamIndex, sinkName]`, `audioprofiles.refresh|pick [{card,profile}]`, `batteryhealth.refresh`, `speedtest.start|cancel|probe|readLink|clearHistory`, `wifijoin.join [req]|dismiss|saved [name]|loadKnown|forget [name]` (the `saved` answer is the bus field `saved`), `wifishare.share [name]|close|scan [file]|find [file]|join [w]|copy [pw]`, `resources.use [{kind,on}]`, `resources.hover [{key,x,y,screen,hovered,gap}]`. Pure helpers: `speed_test::{format,format_ms,format_bytes,verdict,signal_words}`, `battery_health::summary`, `audio::icon`, `power::mode_name`, `pulse_events::subscribe(Box<dyn Fn(&str)+Send>)` (PulseEvents.event).

### services/NetworkAlerts.qml -> network_alerts.rs
Ported: mode (nm | ip), the two list scripts verbatim, parse/compare/announce (tested), 700 ms settle, first look only learns. Differs: always uses the QML's fallback (`nmcli monitor` / `ip monitor link address`); tessera-events' D-Bus path is host-shell's. NetworkManager "active" = `/run/NetworkManager` exists and nmcli is installed. Island colours are the dark accents. The Backends.wifiUnmanaged recheck is left out. Check: with the island on, join/leave a Wi-Fi network, toggle a VPN or a hotspot, plug Ethernet: an alert shows ("Wi-Fi connected", "VPN disconnected", "Personal Hotspot On · name").

### services/WifiJoin.qml -> wifi_join.rs
Ported fully (join, dismiss, saved, known list, forget with its errors), the same wifi-enterprise.py, request on stdin. Check: Settings -> Network, join a hidden/enterprise network; the stage goes saving, connecting, connected or an error that says how to fix it; Known networks lists and forgets.

### services/WifiShare.qml -> wifi_share.rs
Ported (share card via wifi-qr.py, stale answers dropped and their code file removed, scan with the island offer "Join" / "Copy Password", find). Differs: the offer's actions are calls (see above); the clipboard copy uses `wl-copy`; the offer's colour is blue, not the user's accent. Check: Settings -> Network -> share a network (QR appears, closing removes the file in $XDG_RUNTIME_DIR).

### services/SpeedTest.qml -> speed_test.rs
Ported fully: the state machine for speedtest.sh lines (cloudflare) and Ookla's JSON, history (10, speedtest.json in the state dir), link details (parse tested), probe, verdict and the format helpers. Check: Wi-Fi menu -> Test speed: ping, download, upload fill in live; Cancel keeps a finished download; Ookla chosen without the tool shows the install hint.

### services/Audio.qml -> audio.rs
Face over core's services::audio (volume, mute, mic, icon, hush). The QML's PwObjectTracker has no equivalent need. Check: `audio.setVolume` moves the default output; with `quiet` the OSD gets `{"hush":"sound"}`.

### services/AudioStreams.qml -> audio_streams.rs
Streams/outputs/inputs from core's Sound (event players filtered). Differs: the output a stream plays to comes from `pactl list short sink-inputs/sinks` (about once a second while streams exist), not PipeWire's link graph; moving is `pactl move-sink-input <index> <sink>` directly; no Apps.entryFor app names and no Devices.pictureFor pictures (the icons are names only). Check: play audio in two apps, bus "AudioStreams" lists them with their sink; `audiostreams.move`.

### services/AudioProfiles.qml -> audio_profiles.rs
Ported (list, pick/switch through audio-profiles.py, refresh after a switch). The refresh on card changes is the caller's (`audioprofiles.refresh`). Check: Control Center -> Sound with a monitor attached.

### services/PulseEvents.qml -> pulse_events.rs
One `pactl subscribe` with KeptProcess's back-off (5 s doubling to 5 min). Others use `pulse_events::subscribe`. No tessera-events path.

### services/Brightness.qml -> brightness.rs
Backlight: core's services::brightness (logind, not brightnessctl). DDC (ddcutil): detect (with the PROBLEM module/access probe), per-display read/write queue one at a time, coalesced writes, 10 s re-read on focus, 4 s start-up scan and rescan on outputs changing, sandbox never writes. `qs ipc call brightness rescan|status` -> `tessera-ui ipc call brightness status`. Focus comes from core's hypr Snapshot.focused (polled each 500 ms). Check: `ipc call brightness status`; with an external monitor set brightness from Control Center.

### services/Power.qml -> power.rs (+ PowerPolicy.js -> power_policy.rs)
Ported: energy-mode requests and one resolved answer, applied with `powerprofilesctl set` (the profile read over D-Bus from net.hadess.PowerProfiles or UPower's name; the first report within 10 s is put back, later outside changes become your choice), battery saver offer (Undo / Turn on actions), Game Mode request, critical stages 10/5/3 % (sound, island or notify-send, sleep through `system.power`), fully charged note, battery-guard install (`battery-guard-install.sh enable|disable`), idle listeners file + hypridle restart (800 ms debounce, own-hypridle.sh), lid inhibitor + gdbus monitor (the QML's fallback) with sleep/lock and the built-in screen off, app-energy sampler (5 min) and report (10 s while a page asks). The battery is core's status::Battery polled each second (onBattery = UPower state discharging or empty). Differs: no tessera-events; "ppd present" = the daemon answered on D-Bus. Check: `bus` shows mode; Settings -> Battery & Power mode changes `powerprofilesctl get`; set a lid action and close the lid (not in the sandbox: it never sets the mode, never holds the inhibitor).

### services/BatteryHealth.qml -> battery_health.rs
Ported (sysfs read on `batteryhealth.refresh`, condition, summary line, tested).

### services/Resources.qml -> resources.rs
Sampling through core's sysstat::Sampler plus memory GB and the GPU (amdgpu / Intel residency / nvidia-smi, same find script). Active flags set with `resources.use`; 1 s while fast, 5 s otherwise. Note the Rust surfaces sample for themselves, so these flags are only needed by bridges that read the bus.

### modules/resources/ResourceHover.qml -> resources.rs (the `hover` call)
The 500 ms open / 120 ms close delays; says `{"to":"resources","open":…}`. One shared hover (the caller passes the figure's centre-bottom x,y; the 8 px gap is added). Check: hover the CPU figure in the menu bar for half a second.

### modules/network/NetworkFeed.qml -> network_feed.rs
Says `{"to":"network","wifi":…,"speed":…,"anchorX":…}` every 250 ms tick while `open == "wifiMenu"` (only when changed) and at once on opening; answers `ask("network", [what…])`: hold, speedStart, speedCancel, wifi, rescan, connect, disconnect, join, forget, copy, settings, fix. The networks come from nmcli (list every 4 s, not while hovered or typing; sorted connected, known, signal; 30 max). Differs: connecting state is tracked here (no D-Bus ConnectionState); joining with a password uses WifiJoin's script (no password on a command line); the unmanaged notice has the generic texts. Check: open the Wi-Fi menu (`tessera-ui ipc call panels toggle wifiMenu` or the menu bar item): networks listed, Test speed runs, the switch toggles, a password join works.

---

## input group (host/system/src/input/)

Services (names for `Host::sender`): `Displays`, `Mouse`, `KeyboardLayout`, `Input`,
`DeviceAlerts`, `DevicesBridge`. Prefixes: displays + displayfx (ipc target `displays`),
mouse, keyboardlayout, input + keymap, devicealerts, devices.

### Contract the rest of the host needs to know

Calls into this group:
- host-shell's settings bridge (`settings.act` → group) must call, while Settings is open:
  `input.live [bool]`, `devices.live [bool]`, and on every page change `devices.page [page]`.
  The acts reach me as `input.act [what, …args]` / `devices.act [what, …args]` (also accepted:
  `input.<what> [args…]`). Feeds go back as `{"to":"settings","feed":{"input"|"devices": …}}`
  (every 300 / 250 ms while live, only when changed).
- `displays.set|setMany|keep|revert|reset|identify|refresh|preview|finish` (finish [id]: the connect
  overlay (DisplayConnect) reports an effect ended), `mouse.bringForward [address]`, `mouse.confine`
  (screen selection / lock changed), `keyboardlayout.next`, `keymap.setKeysTakingOver [id, keys]`,
  `devicealerts.openDrive [dev]` (drive alert's Open button).

Calls out (asks; unanswered ones are logged by the host): `island.post [event, ms]`, `sound.play [name,
preview?]`, `spaces.arrive [screen]`, `keybinds.refresh`, `audioprofiles.refresh`,
`dictation.{refresh,install,download,cancelDownload,removeModel}`, `screentools.installOcr [lang]`,
and for the Devices page: `network.{wifi,rescan,connect,disconnect,join,forget,hold}`,
`wifijoin.{loadKnown,saved,dismiss,join}`, `wifishare.{share,close}`, `speedtest.{probe,readLink,start,
cancel,clearHistory}`, `brightness.{setDdc,rescanDdc}`, `airplay.{check,install}`,
`mirroring.{look,install,start,stop}`, `phone.{checkCam,install,mirror,pair,cancelPair,setUpCam,
setWebcam}`, `needs.{install,turnOnAvahi}`, `backends.{startNm,openWifiApp,fixBluetooth,recheck}`,
`quickshare.turnOnBluetooth`, `linkshare.receive`, `sharing.shareSelection`, `panels.pickShare`.

Bus keys I publish: `displays` (monitors with lw/lh/number/scales, loaded, confirming,
confirmLeft, identifying, saved, lowered), `displayfx` (runs, hotplugAt, settling), `mouse`
(mice, touchpads, boundaryAvailable), `keyboardlayout`, `keymap` (ready, modKey, binds, gestures,
threeFingerDrag). Menu bar state: `h.set_state("keyboard", {multiple, short, name})`.

Bus keys I read (all optional, defaults when absent): `cooldown` {motionScale, savedRates{output:rate}},
`screentools` {selecting, selectingScreen, hasTesseract, ocrInstalling, ocrInstallError, ocrLanguages},
`dictation` {hasBinary, ready, installing, installError, models, downloading, downloadProgress,
downloadError, lastDownload}, `updates` {running}, `quickshare` {running}, `fastpair` {busy, levels},
`files` {manager}, `filepicker` {busy}, `needs`, `health`, `bluetooth` {devices{addr:{trusted,pairing,
busy}}, name}, `wifijoin` {saved}, and the `feed.*` segments listed in the header of
`devices_bridge.rs` (`feed.wifi`, `feed.backends`, `feed.speed`, `feed.speedTest`, `feed.ddc`,
`feed.airplay`, `feed.mirroring`, `feed.phone`, `feed.sharing`): each is the exact JSON the
RustBridgeDevices feed holds for that part, built by the owner of the data. Menu-bar state read:
`h.state("open")`, `h.state("locked")`.

Island buttons (`onAction`) are posted as `{"call":"mouse.bringForward"|"devicealerts.openDrive","args":[…]}`
(the shape island.rs runs).

### Per QML file

**services/Displays.qml, services/DisplayFx.qml** → `displays.rs` (one service). Ported: parse of
`hyprctl -j monitors all` (modes, names, mirror, built-in), saved lines in config
`displays.monitors` + `<config dir>/monitors.lua`, restore only where Hyprland differs (after start,
configreloaded, monitoraddedv2, 500 ms debounce), set/setMany with the 15 s Keep countdown and
revert, reset, identify (2.5 s), numbering, logical size; DisplayFx: arrival/removal geometry,
neighbours and edges, pending covers, the wait for a calm frame (3 calm 16 ms ticks or 600 ms, plus
1.8 s/0.9 s sync wait), connect/disconnect sound panned to the side (`sound.play`), Spaces handover
(`spaces.arrive`, 3.5 s limit), preview (`displays.preview [effect, side]` and the `displays preview`
ipc target). Differs: `runs` are published on bus `displayfx` (DisplayConnect.qml has no Rust twin
yet: whoever draws the overlay reads it and calls `displays.finish [id]` when an effect ends);
Sounds.armed / Island-style "armed" is an own 5 s timer; focused display for previews comes from
the hypr snapshot. **Check:** Settings → Displays: change a scale, see the 15 s countdown, press
Keep/Revert; `tessera-ui ipc call displays preview liquid left`; plug/unplug a display (or the
sandbox's TESSERA-* output) and look at bus `displayfx` / hear the sound.

**services/Mouse.qml (+ FocusDecision.js)** → `mouse.rs`. Ported: per-mouse `hl.device`, touchpad
config, follow_mouse, udev watcher (800 ms), 60 s poll without udevadm, config-change debounce 150 ms,
glass plugin pointer-policy/pointer-follow/pointer-confine (plugin name+version found by `hyprctl
<name> ping`), selection lease (500 ms), focus-follows-pointer after scene changes (180 ms, FocusDecision
ported as `after_events`/`on_urgent`), apps asking to come forward (urgent: 250 ms settle, input-age,
bring/notify with Island "Show"). Differs: the idle-monitor fallback (no plugin v4) isn't ported (an
urgent window is then announced, not brought forward). **Check:** change Mouse/Trackpad speed in
Settings and see `hyprctl devices`/behaviour; open an app with hover focus: pointer jumps onto it.

**services/KeyboardLayout.qml** → `keyboard_layout.rs`. Ported fully. **Check:** with two layouts, the
menu bar shows the short code; `tessera-ui` call `keyboardlayout.next` switches.

**services/Keymap.qml** → `keymap.rs` (data/editing), `keyfmt.rs` (key naming, copied from the
settings surface), `input_bridge.rs` (files, saving, helper processes). Ported: defaults/gestures/user
file reading (changes picked up by a 2 s mtime poll instead of FileView), merged binds/gestures,
all edits, three-finger drag, undo snapshot, debounced (400 ms) atomic save + `hyprctl reload
config-only`, error texts, export, file/end4 import previews (`import-end4.py`), apply. Not ported:
Keymap's search (`matches`, `parseQuery`) and `conflicts` UI helpers beyond `conflicts` itself: the
Settings surface has its own copies. **Check:** Settings → Keyboard shortcuts: rebind a key, check
keybinds.json and that Hyprland reloaded; undo; export, import.

**modules/settings/RustBridgeInput.qml** → `input_bridge.rs`. Ported: feed (mice, keymap, dictation,
tools, ocr, updatesRunning, ie), every `act`, IeState (dialogs through zenity/kdialog, path field
fallback, preview, undo expiry 8 s). Differs: `Launch.wrap` (tty env scrub) not applied to the dialogs;
dictation/OCR state and actions are other services' (bus + asks above).

**services/Devices.qml** → `device_alerts.rs`. Ported: Bluetooth connect/disconnect (+battery after
connect within 20 s, low at 20/10 %), USB add/remove and drives (1.2 s wait, per-disk 8 s de-dup,
Open button), display connect/disconnect, audio output change (`pactl subscribe` of its own + `pactl
list sinks`). Bluetooth is polled once a second from the bluez reading (the QML reacted 700 ms after a
signal). Differs: no Pipewire fallback without pactl. **Check:** connect a Bluetooth device / USB
stick: an island alert appears; unplug: "Disconnected".

**modules/settings/RustBridgeDevices.qml** → `devices_bridge.rs`. Ported: page-change side effects,
the whole feed (assembled from the bus contract above), every `act`, sharing helper scripts
(`file-manager-share.py`, `sharing-check.py`, only called). Bluetooth acts use `bluetoothctl` (as the
control centre bridge does); the page's scanners are `network.rescan` and a `bluetoothctl scan on`
child killed when the page goes away. **Check:** Settings → Wi-Fi & Bluetooth, Displays, Sound, Phone:
pages fill in and the buttons act.

---

## Group sysinfo (src/sysinfo/)

Conventions: a call's args are the QML function's positional arguments as a
JSON array (`system.setKeepAwake [true, 30]`); `Service::call` gets the full
call name (`system.setKeepAwake`). Cross-service reads use `crate::bus`
(QML property names); actions use `h.ask("<prefix>.<qml function>", [args])`.
Island alerts: `h.ask("island.post", {event, ms})`; an alert's button is
`event.onAction = {call, args}` for the island service to ask when pressed.
Everything below that lists a "contract" is what other services must publish
or answer for the Settings pages to work.

### shell/services/RootHelper.qml
- Rust: `root_helper.rs`. Ported: `pick()`, `command(args)`, `knows(sub)`,
  `message(text)` as pure functions (other modules call
  `sysinfo::root_helper::command`). Service call `roothelper.run [args…]`
  runs `pkexec <helper> args…` detached (only on a user action).
- Differs: nothing; never executes pkexec by itself.
- Check: unit tests; `tessera-ui ipc` has no target. Software Update's
  Update button (system part) is the real use (sysupdate.py asks pkexec itself).

### shell/services/SystemInfo.qml
- Rust: `system_info.rs`. Ported whole; published on the bus as "systeminfo"
  {osName, osLogo, hostname, kernel, cpu, memory, gpu}.
- Check: unit tests; Settings → About shows these (tessera-ui's own page reads
  them itself); `bus::get("systeminfo")`.

### shell/services/Privacy.qml
- Rust: `privacy.rs` (Service "Privacy", prefix `privacy`). Ported: mic list
  (pactl subscribe, 150 ms debounce, monitor/own-shell filter), camera.sh,
  location.sh, screenshare.sh via watch.sh with the 5 s retry, PipeWire graph
  -> shares (gpu-screen-recorder excluded), Hyprland screencast hint, Stop
  sharing (pw-cli destroy links, 2.5 s stopCheck), shareIndicator/shareDot
  logic. Bus "privacy" (all QML properties incl. computed); shell state
  privacy.{any,camera,mic,location,share}.
- Differs: it runs its own `pactl subscribe` (PulseEvents is the net group's);
  it also accepts `privacy.pulseEvent [line]`. The screencast target is
  resolved with hyprctl on a thread when the event arrives (the QML looked it
  up when the share appeared; same 4 s window). The events service's feeds
  can be pushed as `privacy.camera [apps]`, `privacy.location [apps]`,
  `privacy.screenshare [graph]`; the scripts run regardless (no
  Events.fallsBack gate), so both may feed the same values.
- Check: open an app that records the mic (e.g. `parecord /dev/null`): the
  orange dot appears within ~0.2 s and goes when it stops; open a camera app
  (green); share the screen in a browser call (purple dot or island);
  `privacy.stopSharing` (Stop sharing button) cuts it, "still sharing" shows
  after 2.5 s if the app holds on.

### shell/services/System.qml
- Rust: `system.rs` (Service "System", prefix `system`, ipc target
  `caffeine`). Ported: Caffeine (until/minutes, island alert, expiry timer,
  idle inhibitor child, persistence), night light (hyprsunset child), game
  mode, anti-flashbang shader, EasyEffects, WARP (status at start), power
  actions (logout/lock/sleep waits for lockSecure with 3 s guard/restart/
  shutdown, sandbox only reports), lock mark files and restoring them,
  repair shell, identify song, Settings → Appearance applied to Hyprland
  (corners+rounding_power, gaps, window opacity incl. custom 80 ms debounce,
  cursor theme, window tabs reload 800 ms), re-apply on `configreloaded`.
  Calls: `system.setKeepAwake [on, minutes]`, `power [action]`,
  `repairShell`, `toggleDarkMode`, `setEasyEffects [on]`, `setWarp [on]`,
  `set [prop, value]` (nightLight, nightLightTemp, gameMode, antiFlashbang),
  `run [cmd…]`, `hyprEval [lua]`, `identifySong`, `locked [bool]`,
  `lockSecure [bool]`, `cursorTheme [name]`. Bus "system" {keepAwake,
  keepAwakeUntil, keepAwakeMinutes, keepAwakeStatus, nightLight,
  nightLightTemp, gameMode, antiFlashbang, easyEffects, warp, locked,
  lockSecure, sandboxed}; state caffeine.{on,status}.
- Differs: the "in use" IdleMonitor of the tty session is not ported (the
  inhibitor is held only while Caffeine is on); the inhibitor is always the
  systemd-inhibit child (no tessera-events path); `Theme.dark` for
  toggleDarkMode in "auto" mode reads bus "theme".dark (default dark);
  Material cursor reads appearance.materialCursor (AppTheme's later 300 ms
  settle for the Material cursor belongs to the desktop group). `locked` is
  told to the lock screen as `lock.set [bool]`; `panels.close` and
  `uistate.set ["caffeine", value|null]` are asked of host-shell.
- Check: `tessera-ui ipc call caffeine on 1` -> island "Caffeine on", menu bar
  shows keeping awake, `systemd-inhibit --list` has a Tessera entry; `caffeine
  off`; Settings → Appearance: change window corners/gaps/transparency and
  see Hyprland follow; Control Center: night light, game mode.

### shell/modules/settings/RustBridgeSystem.qml
- Rust: `bridge_system.rs` (inside the System service). Ported: the feed
  (fingerprint, sheet, face, check, quickShare, ports, updatesRunning, power,
  cooldown, captions), every act with its argument guards, the per-page
  refreshes (security: fingerprint/face refresh + sharing-check.py status;
  accessibility: dictation refresh; power page: energy users +/-1), the
  add-a-finger sheet counters (takes, misses, retry 2.2 s, flash 0.51 s).
  Settings tells it: `system.live [bool]`, `system.page [id]`,
  `system.scanned [result]` (Fingerprint.scanned), `system.sharingBusy
  [bool]` (SharingPorts busy changed). The feed is re-checked every 400 ms
  while live and sent only when it changed.
- Contract (bus keys, QML property names): fingerprint, face, quickshare,
  sharingports, power, backends {ppd, powerShort, powerReason,
  hasPerformanceProfile}, shellpriority, cooldown, captions, dictation.
  Asks: fingerprint.{refresh,enroll,cancelEnroll,remove,fix,install,setPam},
  face.{refresh,install,setUpCamera,addFace,clearFaces},
  quickshare.setTrusted, sharingports.retry, power.{setMode,clearEnergy,
  energyUsers[±1]}, shellpriority.set, apps.quit, cooldown.toggle,
  captions.{install,download}, dictation.{refresh,cancelDownload}.
  Bus "updates".running comes from this group.
- Check: Settings → Security (fingerprint rows, add-a-finger sheet, Howdy,
  firewall status), Battery & Power (mode, caffeine row, energy apps).

### shell/services/Updates.qml
- Rust: `updates.rs` (model, owned by `bridge_about.rs`; calls
  `updates.update [args]`, `cancel`, `dismiss`, `copyLog`, `refresh`,
  `install`, `showDetails [bool]`). Ported: the sysupdate.py job (detached
  setsid, status.json poll 500 ms, lock check 5 s then 10 s, interrupted
  detection), the output (incremental, last 2000 lines, 1.5 s while shown),
  checkupdates/yay/paru parse, the finish alert (island or notify-send).
  Bus "updates" {running, count, aurCount, total, checking, available,
  interrupted, fake}. Asks `tools.recheck` when a run finishes.
- Differs: the log is read natively from the offset (same result as the sh
  snippet). No periodic check (as in the QML).
- Check: Settings → Software Update → Check for updates, then Update with
  TESSERA_FAKE_SYSUPDATE=1 (simulated): progress, finish alert, Details.

### shell/services/TesseraUpdate.qml
- Rust: `tessera_update.rs` (model; calls `tesseraupdate.check`, `choose`,
  `launch [rebuild[]]`, `dismissResult`). Ported: update-source.py repo
  discovery (TESSERA_UPDATE_REPO wins), tessera-update.py check, conflict
  decisions kept across checks, packages status, launch (the same sh -c copy
  of the updater + update-run.sh, started detached), 20 s start timeout, last
  result file (tessera-/irice-update-result.json, deleted) and the 4 s
  "Tessera updated" alert. The update screen's menu icon glyph comes from
  `tessera_core::icons`.
- Check: Check for updates (shows the report/conflicts); Update runs
  update-run.sh (only on a throwaway checkout: TESSERA_UPDATE_REPO).

### shell/services/SoftwareUpdate.qml
- Rust: `software_update.rs` (model; calls `softwareupdate.check`, `update`,
  `cancel`, `goBack`, `checkUndo`). Ported: the combined check (Tessera,
  system, compat.json of the incoming version, installed Hyprland, snapshot
  tool, helper status with --incoming), version compare/wildcards, hold /
  held / skipped logic, the plan, canUpdate, steps, update() argument
  building (--system, --snapshot, --ignore, --install, --remove, --aur,
  --incoming, --then {…}), go back, and the Tessera part after the system part.
- Differs: `compat` is read as soon as the Tessera check has finished (also
  when the source was not ready), so a check can't wait forever.
- Check: Settings → Software Update: Check for updates shows the plan;
  Hyprland hold prompt appears when compat.json's max is older than the repo's.

### shell/modules/settings/RustBridgeAbout.qml
- Rust: `bridge_about.rs` (Service "About": prefixes about, updates,
  tesseraupdate, softwareupdate). Ported: the whole feed (su, tu, up, bs,
  needsInstalling, health, conflicts, bug, welcomeName, healthChecks,
  healthAttention, drives, keymap, ie) and all acts (su.*, tu.*, up.*,
  reboot, bs.*, needs.install whitelist, health.*, conflicts.*, bug.*,
  welcome, focusReload, openFile, drives.*, ie.*). Page opening effects as
  the QML; update output reading only while the page shows it
  (`Updates.logUsers`). The import/export file dialog (zenity/kdialog on a
  thread, typed-path fallback), 8 s undo notice, messages.
  Settings tells it: `about.live [bool]`, `about.page [id]`.
- Contract (bus keys): bootsnapshots, needs, health {checkedAt in ms},
  conflicts {found, muted}, bugreport (sysinfo{}, state_), today
  {accountName}, clouddrives {…, drives:{gdrive,onedrive}}, keymap {ready,
  hasZenity, hasKdialog, home}. Asks: bootsnapshots.{refresh,openAssistant,
  setMenu}, health.{check,copyReport,fix}, conflicts.{mute [[id]],unmute [id]},
  bugreport.{gather,set [prop,val],editShot,copy,openGithub,send},
  needs.install [[pkg]], clouddrives.{install,open,act,cancel}, focus.reload,
  welcome.request, launch.open [url], panels.openSettings [page],
  keymap.exportTo {path, sections, reply}, keymap.previewFile {text,
  sections, reply}, keymap.previewEnd4 {shortcuts, gestures, reply},
  keymap.applyPreview {preview, reply}, keymap.restore {snapshot}. Keymap
  answers by asking us `about.ieExported {key,path,ok,err}`,
  `about.iePreview {key, preview|error}`, `about.ieApplied {key, snapshot}`
  (the `reply` object carries the call name and key).
- Differs: Launch.wrap is not applied to the file dialog command.
- Check: Settings → About → Import/Export shortcuts (dialog, preview, apply,
  undo within 8 s), System health, Report a bug, Software Update pages.

---

## accounts group (host/system/src/accounts/)

Written by the "accounts" fork. Not compiled by the fork (the lead's single
`cargo check` does that). Helper module: `common.rs` (Tx for thread messages,
processes with stderr/stdin/streaming, root-helper command line, local dates).

### Conventions this group follows
- State other services read: `crate::bus::put("<qml service name lowercase>", json)`, field names = the QML properties'.
- Actions between services: `h.ask("<service lowercase>.<qml function>", json!([qml args…]))`.
- Timers: `slint::Timer` posting a message to the service; threads reply with `Tx` (Host::sender).
- Root helper: only the command line is built (`common::root_command`, same `sh -c … pkexec "$h" "$@"` text as RootHelper.qml) and run on a user action, exactly as the QML.

### Per QML file

| QML file | Rust module | Prefix(es) | Ported / differs | How to check |
|---|---|---|---|---|
| services/Account.qml | account.rs | account | Whole: same sh script; bus "account" {login, fullName, name, picture, pictureUrl}. | Quick Settings footer shows your name/picture. |
| services/Fingerprint.qml | fingerprint.rs | fingerprint | Whole: status/fix/enroll(stream)/delete/setPam via fingerprint.py and the root helper; install through `updates.update`. `scanned` signal becomes bus `scanned {seq, result}`. Bus "fingerprint". | Settings → Lock → Fingerprint: reader found, add a finger (hints per touch), remove. |
| services/Face.qml | face.rs | face | Whole; faceDevice/faceCount written with `util::config_set`. Bus "face". | Settings → Lock → Face unlock: camera detected; set up/add/clear ask for the password. |
| services/LoginScreen.qml | login_screen.rs | loginscreen | Whole. Differs: the "seed" for the Glass look is the Rust `Look.seed` (the QML used Wallpaper.tone); greeter.json is re-read at the 3 s timer instead of a file watch. `loginscreen.preview` runs the same qs command. Bus "loginscreen". | Change the look, wait 3 s: `tessera-root login-look` is asked once if greeter.json differs. |
| services/BootSplash.qml | boot_splash.rs | bootsplash | Whole. Probe runs on `bootsplash.refresh` (the Settings page asks when it opens: the QML did the same by `refresh()` from the page). Bus "bootsplash". | Settings → Boot animation: choices list; picking asks for the password. |
| services/BootSnapshots.qml | boot_snapshots.rs | bootsnapshots | Whole. Probe 5 s after start (Startup.later), island alert 15 s after the check when booted from a snapshot (`island.post` ask, see below), `updatesFinished` re-probes. Bus "bootsnapshots". | Settings → Software Update → Going back: menu switch. |
| services/ScreenTime.qml | screen_time.rs | (none) | Whole logic (idle 2 min via bus "idle", 60 s flush, a week kept). Differs: front app is polled every 2 s; state.json's `screenTime` key is read at start and written directly (≤ every 30 s and on quit), see CORE-NEEDS. Bus "screentime" {today, todayTotal, todayApps, week, total, apps}. | Use two apps for a minute: the desktop Screen Time widget grows. |
| services/CloudDrives.qml | cloud_drives.rs | clouddrives | Whole (queue of jobs, remount at login 5 s after start, cancel = SIGTERM). Open folder asks `files.openFolder`. Bus "clouddrives". | Settings → Accounts → Drives. |
| services/CloudAccounts.qml | cloud_accounts.rs | cloudaccounts | Whole except iCalendar expansion: bus "cloudaccounts".events is the QML's `parsed` ({raw, cal, key, ics?, notes, hidden, patch}); the desktop group's Calendar expands/filters as `CloudAccounts.between` did. Helper request queue, etag refresh, undo-delete (8 s), syncs (20 s after start, unlock, Notification Center, 15 min, not locked/idle), Microsoft sign-in helper. Passwords go only to the helper's stdin. | Connect an iCloud account in Settings; events appear in Today; delete shows Undo for 8 s. |
| services/Google.qml | google.rs | google | Whole (queue, overrides, undo-delete, backoff, outbox resend 30 s→30 min, conflicts via notify-send + wl-copy, sign-in URL via `launch.open`). Bus "google" has override-applied `tasks` and `events` lists plus raw `account`. | Connect Google; add a task; kill the network: change stays, resends later. |
| modules/settings/RustBridgeAccounts.qml | accounts_bridge.rs | accounts | Whole: feed + every act (Google, iCloud/Microsoft, Needs, drives, Calendar sources, AI). Missing services are reached by asks (below). | Open Settings → Accounts: feed present; sign in/out buttons ask the services. |
| modules/controlcenter/RustBridge.qml (+ ModuleDefs.qml) | cc.rs | cc | State message, all eight pages (wifi, bluetooth, focus, caffeine, sharing, mirroring, cast, resources), cc.act/page/editing/layout/media/block/pick. Differs: Bluetooth device actions use `bluetoothctl` (pairing state/Trusted are not in the core bluez reading); the playing position is polled with busctl once a second; Wi-Fi rows come from bus "network" (NetworkFeed's rows). | Open Control Center: tiles show state; click a tile; open Wi-Fi and Bluetooth pages; Focus length segments; Sharing levels; media buttons. |

### Internal contract: who must publish / answer what (for the other groups and the lead)

**Asked by this group, answered elsewhere**
- `updates.update [argsArray]` (sysinfo, Updates.update). When a job ends, Updates must call `h.ask("accounts.updatesFinished", json!([job]))` (job = {state, error, text}); this group fans it out to fingerprint, face, clouddrives and bootsnapshots.
- `island.post {event, ms}` (host-shell Island). Event for BootSnapshots has `color: "accent"` (Theme.accent) and `onAction: {ask: "panels.openSettings", args: ["update"]}`.
- host-shell: `launch.open [url]`, `launch.run [argv]`, `needs.install [list]`, `needs.buildNative`, `needs.turnOnAvahi`, `panels.show [name]`, `panels.close`, `panels.openSettings [page, target]`, `panels.pickShare`, `panels.ncTab [n]`, `backends.startNm|openWifiApp|fixBluetooth`, `focus.activate [id, durationMs, fromSchedule]|deactivate`, `cooldown.toggle`.
- sysinfo: `system.set [prop, value]` (nightLight, gameMode, antiFlashbang), `system.toggleDarkMode`, `system.setKeepAwake [on, minutes]`, `system.setEasyEffects [on]`, `system.setWarp [on]`, `system.identifySong`.
- net: `network.wifi [on]`, `network.rescan`, `network.connect|disconnect|forget [name]`, `network.join [name, psk]`, `audio.toggleMic [true]`, `power.setMode [mode]`, `audioprofiles.refresh`, `audioprofiles.pick [alt]`.
- desktop: `files.openFolder [path]`, `tiling.arrange ["leftRight"]`, `captions.toggle`, `recorder.openToolbar`, `calendar.setSource [url,{enabled}]|removeSource [url]|addSource [name,url]|setDefaultAlert [n]`, `ai.setModel [key]`, `ai.accountsSave [raw, oldKey, credential]`, `ai.removeModel [key]` (keeps the removed record), `ai.restoreRemoved`, `ai.forgetRemoved`, `ai.deleteAllChats`.
- sharing: `sharing.setLevel [level]`, `sharing.look [bool]`, `sharing.pickFor [device]`, `mirroring.start [display]|stop|look [bool]`, `cast.stop`, `cast.castPlaying [device]`.

**Read from the bus (publisher)**
- sysinfo: "system" {locked, nightLight, nightLightTemp, keepAwake, keepAwakeMinutes, keepAwakeStatus, gameMode, antiFlashbang, easyEffects, warp}, "updates" {running}.
- net: "audio" {micMuted, icon, …}, "audioprofiles" {alternatives:[{name,kind,profile}], switching}, "audiostreams" {inputs}, "power" {profile}, "brightness" {available}, "resources" {cpu, memory, gpuAvailable}, "network" {wifiEnabled, wifiPresent, ssid, networks:[{name, connected, known, secured, enterprise, signal 0..1, connecting|joining, failed, busy}]}.
- input: "devices" {pictures:{address: path}}.
- host-shell: "backends" {wifiUnmanaged, wifiShort, wifiTitle, wifiReason, canStartNm, wifiApp, wifiAppName, btState, btShort, btFix, btTitle, btReason, btFixLabel, btFixing, ppd, powerShort}, "health" {fixing}, "needs" {checked, secretTool, keyring, goa, goaMicrosoft, goaSignin, installing, building, buildError, networkManager, avahi}, "focus" {on, mode:{name,icon}, until, statusText, active, modes}, "cooldown" {active, why}, "idle" {isIdle} (see CORE-NEEDS), state "open", "focus.on", "settingsOpen" (optional).
- desktop: "calendar" {sources, lastError, defaultAlert}, "ai" {providers, models, hasKey, sessions, pinned}, "captions" {enabled, ready}, "recorder" {available, recording}, "screentools" {recording}, "apps" {activeId}, "weather" {unit}.
- sharing: "sharing" {on, level, mixed, devices:[{name,type,prefer,progress|null}]}, "mirroring" {active, targetName, state, target, displays, error, p2p, available}, "cast" {devices, active, device, title, state, canCastPlaying}, "localsend" {available, error}, "quickshare" {bluetoothNeeded, bluetoothOn, error}, "filepicker" {busy}.

**Published by this group (bus)**: "account", "fingerprint", "face", "loginscreen", "bootsplash", "bootsnapshots", "screentime", "clouddrives", "cloudaccounts", "google".
**State set by this group** (`h.set_state`): "ccPage", "ccEditing".

**Calls other groups make into this one**: `google.*` (addTask, updateTask, removeTask, restoreTask, addEvent, updateEvent, removeEvent, restoreEvent, freshen [age], run), `cloudaccounts.*` (same names, plus connectICloud, connectMicrosoft, disconnect, sync), `fingerprint.*`, `face.*`, `bootsplash.refresh|set`, `bootsnapshots.refresh|setMenu|openAssistant`, `loginscreen.preview`, `clouddrives.*`, `accounts.live [bool]`.

**Settings routing (host-shell)**: the settings window sends `shell::ask("settings", ["act", "accounts", what, …args])`. host-shell's settings router should forward it as `h.ask("accounts.<what>", json!([…args]))` (the bare call `accounts` with `[what, …args]` also works). The feed goes back as `h.tell("settings", {"feed": {"accounts": …}})`, while `live` (starts true; `accounts.live [false]` or state "settingsOpen" false stops it).

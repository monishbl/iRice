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

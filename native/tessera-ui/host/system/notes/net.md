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

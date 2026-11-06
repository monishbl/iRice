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

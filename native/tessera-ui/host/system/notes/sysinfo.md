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

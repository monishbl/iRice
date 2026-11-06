# Wave 4 brief: the host (the QML shell's services, in Rust)

Read this whole file first, then `core/src/host.rs` (the seam, already
written), `core/src/services/bridge.rs`, `core/src/shell.rs` (`ask`, `Part`,
`Command`) and `src/main.rs` (`shell_line`, `--standalone`).

## What wave 4 is

Every surface of the shell is already drawn in Rust (39 surface crates under
`surfaces/`). They still lean on the QML shell for everything else, over two
seams:

- **Out:** a surface calls `shell::ask("network.connect", args)` or
  `services::bridge::call(&["panels","toggle",…])`. Today RustUi.qml
  (`shell/services/RustUi.qml`, its `call()`) and the QML modules' bridges
  (`modules/*/RustBridge*.qml`, `*Feed.qml`, handlers of `onSurfaceCall`,
  `onFeedCall`, `onSettingsCall`, `onScreenCall`, `onWmCall`,
  `onDesktopCall`, `onCcCall`) answer them.
- **In:** the QML writes JSON lines to tessera-ui's stdin: commands
  (`{"cmd":"open","surface":"launchpad","screen":"DP-1"}`), the shell's state
  (`{"src":"shell","kind":"state",…}`: services::bridge::ShellState), island
  alerts, live activities, glass, and per-surface messages
  (`{"to":"<surface>",…}`, RustUi.tell).

Standalone (`tessera-ui --standalone`) there is no QML. The **host**
(`core/src/host.rs`, a Part made first) holds every `Service` from the four host
crates. `ask` and `bridge::call` reach the service that owns that call prefix
or IPC target. A service answers by saying the **same JSON lines** the QML
writes today (`Host::say`, `command`, `tell`, `set_state`), so **no surface
changes**. Your job is to replace the QML side of those seams, file by file,
faithfully: same behaviour, same messages, same field names.

## The crates and who owns what

| crate | agent | owns |
|---|---|---|
| `host/shell` | host-shell | TesseraShell.qml, shell.qml, RustUi.qml (as the map of every message), all of `common/`, Island(+IslandCycle.js), Events, EventWatch, Startup(+StartupSounds.js), Sounds, Intents, Keybinds, Needs, Backends, ShellPriority, Conflicts, HyprlandLink, GlassPlugin, Health, BugReport; the bridges of menubar, overlay, welcome, settings (RustBridge, Look, Notify), lock (+LoginIntro), osd, polkit, session, background, hotcorners, quicklook. **Also:** the IPC socket and `tessera-ui ipc call` (`ipc.rs`), and the standalone entry (see below). |
| `host/system` | host-system | NetworkAlerts, WifiJoin, WifiShare, SpeedTest, Audio, AudioProfiles, AudioStreams, PulseEvents, Brightness, Power, PowerPolicy.js, BatteryHealth, Displays, DisplayFx, Mouse, KeyboardLayout, Keymap, Devices, Privacy, System, SystemInfo, Resources, RootHelper, SoftwareUpdate, Updates, TesseraUpdate, Fingerprint, Face, LoginScreen, BootSplash, BootSnapshots, Account, CloudAccounts, CloudDrives, Google, ScreenTime; bridges: controlcenter/RustBridge, network/NetworkFeed, settings/RustBridge{Devices,Input,Accounts,System,About}, resources/ResourceHover |
| `host/desktop` | host-desktop | Apps, AppTheme, Desktop, Windows, WindowRules, Spaces, SpaceMap.js, StageManager, Tiling, Decorations, Focus, FocusDecision.js, FocusHistory, FindPointer, Search, Files, Clipboard, ScreenTools, Recorder, LiveText, OcrLangs.js, Captions, Dictation, Tools, Today, Todos, Timers, Alarms, Weather, Calendar, Notes, Notifications, Media, Calls, Badges, Ai(+AiChats, AiKeys, AiModels, AiPrompt); bridges: desktop/DesktopFeed, overview/RustWindows, switcher/Switcher, screentools/RustBridge, screentools/editor/ScreenshotEditor, today/TodayScreen, notes/NotesFeed, captions/CaptionsFeed |
| `host/sharing` | host-sharing | Sharing, SharingDevices.js, SharingPorts, QuickShare, LocalSend, LinkShare, AirPlay, Cast, Mirroring, Phone, FastPair, Shelf, ShelfLogic.js, Transfers, DownloadTracker.js, SseParser, AppStore; bridges: share/ShareFeed, shelf/ShelfFeed, appstore/AppStoreFeed |

**Call prefixes** (`Service::calls`), one owner each:
- shell: panels, look, welcome, overlay, settings, launch, open, show, sidebar, reveal, shortcut, sound, island, lock, polkit, session, osd, quicklook, hotcorners, menubar, background, wallpaper
- system: system, about, accounts, network, cc, devices, input, resources
- desktop: notify, spaces, livetext, clipboard, ai, today, search, apps, files, todos, todo, todoAdd, recorder, desktop, wm, tiling, stacks, screen, notes, captions, overview, switcher
- sharing: shelf, share, appstore

Find every call your prefixes cover: `grep -rhoE 'ask\("[a-zA-Z0-9_.]+"' surfaces core/src`,
and the `bridge::call(&[…])` sites. A prefix nobody listed: put it in your
PORT-NOTES.md under "Unowned" and do not answer it (the host logs it).

**IPC targets** (`Service::ipc_targets`, what `qs ipc call <target>` reaches;
see each QML `IpcHandler { target: … }`): the owner of the QML file that
declares it. host-shell answers any declared in a file nobody owns.

**Services that exist already in Rust:** `core/src/services/` (apps, audio,
bluez, brightness, calendar, capture, clients, clipboard, hypr, mpris,
notify, pulse, status, todos, tray, wallpaper, weather, ai) and what the
surfaces own (e.g. the notifications surface is the D-Bus notification
server). Use them; don't write a second reader. Read their data with
`h.st.shell.get::<T>()` and `h.st.shell.want("name")`.

## Rules (strict)

1. **Edit only inside your crate**: `native/tessera-ui/host/<yours>/`.
   host-shell may also edit, guarded so the QML shell behaves exactly as
   today: `hypr/tessera/execs.lua`, `shell/assets/scripts/launch-shell.sh`,
   `keep-shell.sh`, `shell-ipc.sh` (standalone: run `tessera-ui --standalone`
   instead of `qs`, and route `shell-ipc.sh call` to `tessera-ui ipc call`;
   the switch is the setting `rust.standalone`, default false).
   Need a change anywhere else (core, a surface, main.rs)? Don't make it:
   write it in your crate's `CORE-NEEDS.md` (what, where, why, the exact
   code), and work around it meanwhile.
2. **No git**: no commit, branch, stash, checkout or reset. The lead commits.
3. **Write first, no test loops.** Port every file in your list. Then, at
   the very end, type-check your crate only, through the build gate, with
   the display stripped:
   `cd native/tessera-ui && env -u WAYLAND_DISPLAY -u WAYLAND_SOCKET -u DISPLAY ~/.local/share/tessera-port/slot.sh cargo check -p tessera-host-<yours> --tests`
   Fix what it reports and re-run it (at most about 4 times). Never
   `cargo build`, `cargo test`, `--workspace` or `-p tessera-ui`. Never run
   tessera-ui, qs, a sandbox or any binary you wrote. The user builds and
   tests everything afterwards.
4. **Safety.** Never run sudo, pkexec, irice-root, nft, ufw, systemctl
   (except reading `--user` state is not needed either), or anything that
   changes the live system. Your code may *call* such tools (as the QML
   does), but you never execute them. Never kill processes. Never read or
   print secrets (API keys in config, tokens, keyring). No company
   trademarks in new names.
5. **Faithful.** Read each QML file in full before porting it. Same
   messages, same field names, same timings, same defaults, same edge cases.
   Where the QML calls a script (`shell/assets/scripts/*.py|sh`), call the
   same script. Comments in the style of the existing Rust: say what and
   why, briefly, naming the QML file each module replaces.
6. **Threads.** Never block the loop: D-Bus, processes and files on a thread,
   results back with `h.sender("<Name>")` → `Service::message`. Timers:
   `slint::Timer`.

## Pure-UI files

Much of `common/` is widgets (Switch, Slider, Chip, Squircle, glyphs…) that
the Slint surfaces already replaced. For each, confirm the Slint equivalent
exists (grep `surfaces/*/ui`, `core/ui`), then mark it written with a
one-line note in PORT-NOTES.md ("UI: replaced by core/ui/… "). Data files
(emoji.js, phosphor.js, material-symbols.js): confirm the Rust side has them.
Logic files (Config, Theme, Panels, UiState, CoolDown, LockFx, Launch,
Wallpaper, Paths, TimeFormat, Daylight, SettingsMap, KeptProcess…) need real
ports where standalone needs them.

## Progress tracker (keep it current; the user watches it live)

`P=~/.local/share/tessera-port/prog.py`; your group is `host-<yours>`.
- At the start: `python3 $P start host-<yours> <your agent name> "host/<yours> crate"`
- Often (every file or two): `python3 $P doing host-<yours> "<2-4 words>"`
- Each file ported: `python3 $P set host-<yours> <path as listed, e.g. shell/services/Power.qml> written`
- After the final `cargo check` passes, every written file: `... set ... built`
- At the end: `python3 $P phase host-<yours> done`
See your list with `python3 $P show host-<yours>` (or read
`~/.local/share/tessera-port/progress/host-<yours>.json`).

## What to hand back

1. `host/<yours>/PORT-NOTES.md`: per QML file, its Rust module, what's
   ported, what differs or is left out, and **how to check it** (what to do
   on screen and what should happen; which `tessera-ui ipc call` or surface
   action exercises it). The lead turns these into the verification script.
2. `host/<yours>/CORE-NEEDS.md` if you need anything outside your crate.
3. A short final message: done / not done, the check result, the
   CORE-NEEDS items, anything risky.

## Open after integration

Written by the integrate pass (no cargo was run; see each crate's PORT-NOTES for what was reconciled). Still open:

1. **Surfaces' stdout events** (`shell::tell`, core/src/shell.rs): host/shell CORE-NEEDS 1 (`ui`: closed, osd, session, lock, ready) and host/desktop CORE-NEEDS "today/ 1" (`notifications`, `nc`) are two patches to the same function and must be one: route kind `notifications` | `nc` to `route_ask(kind, &fields)` (today's Hub answers those prefixes), every other kind to `route_ask("ui", &e)` with `e["kind"] = kind` (host-shell's Panels answers `ui`). Until then standalone gets no unread count, no island notification cards, no ringing calls and no panel `closed` events.
2. **host/shell CORE-NEEDS 3-6** (idle notifications for the lock's StandBy, Hyprland link repair, glass `window-frost`, a `Service::data` hook) and **host/desktop CORE-NEEDS wm 1** (the find-pointer ring surface) are unchanged.
3. **Island card buttons**: the menu bar surface only sends `1`, `2` and `more <i>` for an alert (surfaces/menubar island.slint); a notification card's own buttons (Reply, app actions) have no click wiring there yet, so only the card click (`1`) reaches `today.islandAct`. host-desktop already understands `btn <i>`.
4. **Shortcuts standalone**: keybinds.lua decides at load time (a Hyprland reload after changing `rust.standalone`). `pointerDown`/`pointerUp` (the drop shelf's left-button binds) start a `tessera-ui ipc call` per click; cheap, but worth measuring, and could be bound only while the shelf is on. A `tessera-ui` started by hand while the setting is off still gets no shortcuts.
5. **AI settings** (`ai.accountsSave|removeModel|…`, host/desktop): the credential goes to the keyring/file through core's `Keys::set` on its own copy; a sidebar chat that already runs keeps its cached key until it looks it up again (it does when a key is missing). Errors from `accountsSave` are only logged (the page checked the draft first).
6. **Behaviour differences accepted**: `apps.launch` focuses an existing window of the app before starting it (Launch.app always started it); `notify.replyLatest` opens Notification Center and shows the banner (no reply field is opened from the host); `tools.recheck` is a no-op (nothing caches tool look-ups); mirroring/airplay/phone calls are the `share.*` actions under other names (`alias()` in host/sharing/src/lib.rs).
7. **Not checked by a compiler or by running**: every edit of this pass. The likeliest first-build errors: borrow of `self` inside `call()` arms in host/shell/src/probes.rs (`self.call(h, "health.fix", …)` re-entry), `Keys::default()` use in host/desktop/src/ai/mod.rs, and the `m.get("call").map(|_| m)` lifetime in host/shell/src/island.rs `take_actions`.

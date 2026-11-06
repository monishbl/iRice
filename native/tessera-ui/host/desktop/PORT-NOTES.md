# host/desktop port notes

Not compiled: the user builds and tests. Parts: wm, screen, today, ai.

## Unowned
- `focus`, `findpointer` claimed by wm; `alarms notifications nc media badges` claimed by today.

## After integration (cross-crate calls reconciled)
Prefixes added to `calls()`: wm `decorations`; screen/service `screentools`; dictation `dictation`; today `calendar`, `calls`. The callers' names now reach an arm:
- wm: `apps.launch` (focus the app's window, else start it), `apps.mergeTabs` (= mergeIntoTabs), `spaces.moveSpace` (= moveSpaceTo), `desktop.findPointer` (= findpointer.show), `focus.restoreMode [mode]` (Undo of a removed mode), `decorations.installPlugin` (-> `launch.terminal [[install-hyprbars.sh], false]`).
- screen: `screen.select|screenshotScreen|stopRecording|recordScreen|recordToggle|cancelSelect|editShot [path]`, `recorder.openToolbar|open|showInFolder|undo`, `screentools.installOcr [lang]`, `dictation.pressed|released|refresh|install|download|cancelDownload|removeModel` (same code as the IPC ones). Island alerts of this group are posted through `island.post` (host-shell keeps their buttons), no longer as bare lines.
- today: `calendar.addSource|removeSource|setSource|setDefaultAlert` (writes calendars.json), `calls.answer|decline|open|leave|openApp`, `notify.replyLatest`, `todos.add|addText`; the ringing card's buttons "1" Answer / "2" Decline reach `today.islandAct`. Notification Center's alert rows read state `island.list` (host-shell's history array).
- ai: `ai.setModel|accountsSave|removeModel|restoreRemoved|forgetRemoved|deleteAllChats` (Settings -> AI; models written to config.json as JSON strings, the credential to the keyring through core's `Keys::set`; Undo keeps the removed record in memory until forgetRemoved).
- `tools.recheck` is answered by host-shell (a no-op: tool look-ups are never cached here).

---

# host/desktop: wm (windows, spaces, desktop feed)

One service, **Wm** (`src/wm/mod.rs`, `wm::services()`), because the QML services share Hyprland's
events and each other's state. It answers call prefixes `apps, desktop, wm, spaces, tiling, stacks,
overview, switcher` and also `focus`, `findpointer` (nobody else lists them), and the ipc targets
`tiling, focus, findpointer, switcher`. Needs `mod wm;` and `wm::services()` in lib.rs.

Plumbing: `hx.rs` (an ordered queue for dispatch / `hyprctl eval` like the QML's one-after-another
calls; `fetch()` = fresh clients, monitors, workspaces on a thread; config.json and state.json
read-modify-write; glass plugin commands). Events: one socket2 thread -> `Msg::Ev`. Timers are slint
timers -> `Msg::Tick`. Threads answer with `Msg::Run` closures.

Prerequisite for every check: `tessera-ui --standalone`, Hyprland running.

## Per file

| QML | Rust | Ported / differs | Check |
|---|---|---|---|
| Apps.qml | `apps.rs` | pin/unpin (dock.pinned), recent apps (state.json recentApps, 12), quit (close windows + quit-app.sh), minimize/restore (special:minimized), show (Show Desktop aware), activateOrLaunch, mergeIntoTabs, windowAction. Entries/icons/launch are core's (services::apps), not re-done. Calls: `wm apps.pin/unpin/show`, plus `apps.quit [id]`, `apps.minimize [addr]`, `apps.restore [addr]`, `apps.activateOrLaunch [id]`, `apps.mergeIntoTabs`, `apps.windowAction [name, {fields}]`. Differs: the active entry/`activeName`/`dockApps`/`recentFor` lists are views the dock draws itself, not kept here. | Launchpad: Keep in Dock / Remove from Dock changes `dock.pinned` in config.json. Focus a few apps, `jq .recentApps state.json`. `ipc call`-less: ask `apps.quit` from the menu bar. |
| AppTheme.qml | `apptheme.rs` | Match apps via app-theme.sh (same args, same 1.5 s settle, roles JSON in Material), appThemeOn in state.json, Material window look (border/rounding/shadow eval 250 ms after look or reload), glass-plugin controls-style, Material pointer (`hyprctl setcursor`). Differs: no CoolDown.lightBlur (host-shell state, not known here: blur is always set); Material accent = `look.accent`. | Settings -> Appearance -> Match apps: after 1.5 s the colour files regenerate. Material look: windows get the 1 px tinted border. |
| Desktop.qml | `desk.rs` | Show Desktop (stash/restore, comes back on workspace/openwindow). Geometry helpers (mostlyCovered/fullscreen/covered) are core's `Windows`. Call `desktop.showDesktop` (host-shell keybind routes the shortcut). | Press the Show Desktop shortcut twice: windows leave and return. |
| Windows.qml | (none) | The grouping is `services::clients::Windows`. | - |
| WindowRules.qml | `rules.rs` | Full: normalize, spec, window-rules.lua writer (same text), live eval with handle table, additive vs reload, hook check, configreloaded re-apply, default layout and no-self-maximize rule, rearrange on layout switch. Unit tests for the Lua. | Settings -> Window Rules: add a rule, `cat ~/.config/tessera/window-rules.lua`; open the app: it floats. Switch default layout: open windows float/tile. |
| Spaces.qml | `spaces.rs` | go, newSpace, sendWindow, sendWindowToDisplay, moveSpaceTo, move (reorder), arrive/reconcile, assign/plan, spaces.lua + TESSERA_SPACE_BASES, shown-space memory (state.json `spaces`), telling the views `{spaces:{bases,labels,count}}` and the bar's state `spaces`. `groupsFor/idsFor/title/edgeZones` are core's (spacemap.rs, wm/spaces.rs). Calls: `wm spaces.*`, `spaces.arrive [name]` (DisplayFx). | Overview: drag a space, send a window, new space. Plug a display: it gets a block (`jq .spaces state.json`). |
| SpaceMap.js | `spacemap.rs` | assign, basesByName, nextFree, plan, keyOf, labelOf with tests. | `cargo test` later. |
| StageManager.qml | `stage.rs` | Full (start/stop/reconcile/show/merge, parked on special:stage, quiet window after our own moves); tells stagemanager `{stage:{waiting}}`. Calls `wm stage.show/stage.merge`. | Settings: Stage Manager on; open two apps; the strip lists the waiting stage; click it. |
| Tiling.qml | `tiling.rs` | layouts, area (reserved + gaps), rect, snap, restore (previous kept), pip, arrange, drop on a drag zone (`tessera-drag`/`irice-drag`), closewindow cleanup; ipc `tiling snap/arrange/restore/pip`; call `tiling.snap [layout, address]`. Tests for the geometry. Differs: canRestore/isPip are not exposed. | `tessera-ui ipc call tiling snap left`; drag a window to an edge. |
| Decorations.qml | `decorations.rs` | mode, hyprbars check (30 s while wanted), glass `controls on/off dark|light`, `controls-style glass` outside Material. `installPlugin` (Launch.terminal) is not ported. | Settings -> Windows -> window controls. |
| Focus.qml | `focusmodes.rs` | Focus modes (the Do Not Disturb family): activate/deactivate/toggle/tick (30 s), schedules incl. overnight, wallpaper/appearance effects and undo, focus.json, state `focus.{on,icon,name,status}`; ipc `focus on/off/toggle`; calls `focus.activate/deactivate/toggle/reload/setMode/addMode/removeMode`. `allows(n)` is the notifications surface's (reads focus.json). | `tessera-ui ipc call focus toggle dnd`: bar shows the moon; again clears it. |
| FocusDecision.js | `focushist.rs` | `after_events`, `on_urgent` as pure fns with tests, for host/system's Mouse. | - |
| FocusHistory.qml | `focushist.rs` | order kept from activewindowv2/closewindow; `sorted`. (The Rust switcher ranks with focusHistoryID itself; nothing reads this yet.) | - |
| FindPointer.qml | `findptr.rs` | enabled flag to Lua (`TESSERA_FIND_POINTER`), grow (same script), ring position feed from pointer-follow.py as `{"to":"findpointer","show":true,"x","y"}`; ipc/call `findpointer find`. The ring itself is not drawn (see CORE-NEEDS-wm.md). | `tessera-ui ipc call findpointer find` with style grow: the pointer swells. |
| DesktopFeed.qml | `feed.rs` | The `desktop` call (all `what`s) and the feed (greeting, weather, events, busy, upcoming, to-dos, media, battery/power, notes, stacks, shortcut buttons, Edit Widgets) from core readings, every 2 s if changed. Differs: Screen Time is zeros (host/system); Bluetooth batteries are not added; `actions` comes from state `search.actions` (the Search service should set it) else []; busyDays does not skip daily series. | Desktop widgets show weather/events/to-dos; tick a to-do; edit widgets. |
| RustWindows.qml | `mod.rs` (`wm_call`) | spaces, apps, stage calls; panels.show(switcher)/closeOpen/openAppStore forwarded to host-shell with `h.ask`; locked state to switcher and stagemanager (checked each second from `h.state("locked")`); `overview.expose [app]` -> missioncontrol. | Open Overview, Mission Control, Alt+Tab, Stage Manager. |
| Switcher.qml | `mod.rs` | Only the Rust-drawn path: `switcher.step [d]` / `switcher.commit` / `switcher.cancel` (call or ipc `switcher step 1`) -> `{"cmd":"step"|"commit"|"cancel","surface":"switcher"}`. The QML window is not ported (the surface replaced it). Host-shell's keybinds must route Alt+Tab / Alt release to these. | `tessera-ui ipc call switcher step 1`, then `commit`. |

## Cross-service calls this makes (names assumed; owners must answer them)
`panels.show [name, screen]`, `panels.closeOpen`, `panels.openAppStore`, `panels.close`,
`panels.set [prop, value]` (ncTab, widgetOptionsKind), `panels.quickLook [path]`,
`panels.share [list, screen]`, `island.post [event]`, `sound.play ["trash"]`, `wallpaper.set [path]`
(host-shell); `today.show [optionsOpen]`, `todos.toggle [key]`, `todos.add [text]`, `files.open`,
`files.reveal`, `search.action [title]` (the Today/Todos/Files/Search owners). State keys read:
`locked`, `search.actions`. Written: `spaces`, `focus.*`.

## Unowned
None found in these prefixes (and see "After integration" above). `todo`/`todoAdd` come as `desktop` whats and are routed to `todos.*`.

## Risks
- state.json is read-modify-written here (recentApps, spaces, notes, stacks, appThemeOn) beside the QML UiState journal / host-shell; avoid running both at once.
- Never compiled when written: the final `cargo check` is the lead's.

---

# host/desktop: the screen group (src/screen/)

Add to lib.rs: `mod screen;` and in `services()`: `v.extend(screen::services())`.
Services (call prefix / IPC target): Search (`search`), Files (`files`), Clipboard
(`clipboard`), ScreenTools (`screen`, `recorder`, `livetext`; IPC `screen`, `recorder`,
`shotEditor`), Captions (`captions`; IPC `captions`), Dictation (IPC `dictation`).
Not compiled or run by me (rule 3): the lead's one `cargo check` is the first compile.

Guessed cross-service calls I make (`h.ask`; the owner should answer them or tell me the
real names): `panels.closeOpen`, `panels.show [name, screen?]`, `panels.showSession [kind]`,
`panels.pickShare`, `panels.openSettings`, `panels.openAppStore`, `panels.quickLook [path]`,
`launch.open`, `launch.openLink`, `launch.terminal [[argv], true]`, `sound [name]`
(screenshot, recording, recordingOff), `system.power ["lock"]`, `system.update ["--install",
csv]` (Dictation/OCR installs), `system.identifySong`, `system.checkUpdates`,
`network.joinWifi [wifi]`, `show ["health"|"bugReport"]`, `today.show [""]`,
`audio.toggleMic`. Host state I set: `recorder` ({recording, paused, file, started,
pausedAt, countdown, micMuted, canPause}) and `dictation` ({stage, ready, info, ...}) for
the island / settings readers. Island posts use uids from 7,000,000 up.
Things I tell the system host back to me over IPC: `screen installDone ok|fail [why]`,
`dictation installDone ok|fail`.

## Files

| QML | Rust | Notes / check |
|---|---|---|
| services/Tools.qml | `util::has` (PATH lookup, `tessera_core::util::on_path`) | Differs: no startup probe, no watcher reaping (`reap-watchers.sh` is the shell host's). |
| services/Files.qml | `files.rs` | `files.open/openFolder/reveal/openTrash`. Check: Spotlight "/name" row opens a folder in the file manager, a file in its app. |
| services/Clipboard.qml | `clipboard.rs` | `clipboard.refresh/pin/unpin`; stamps (clip-times.json) via a thread on the cliphist db; pins in state.json `clipPins` (read-modify-write, atomic; races with a UiState port that also writes state.json: shell host should expose one writer). Check: copy text, open the clipboard panel: heading "Just now"; pin an entry: it moves to Pinned, survives restart. |
| services/Search.qml (Rust part) | `search.rs` | `search.query/run/action/command`. Differs: no intents, reminder, App Store, contacts rows and no cast actions (other hosts own them); Ask-AI meta from config `ai.models`; calendar rows = next-year titles from core calendar; notes by name; screenshots by words. Check: Spotlight ";x", "meeting" (calendar), a note's name, a screenshot's words. |
| services/ScreenTools.qml | `screentools.rs` | grim/tesseract/trans/swappy flows as in the QML, same scripts and messages. Check: `tessera-ui ipc call screen region screenshot` then drag; `ocr`, `translate`, `ask`, `circle`; `screen ocrLanguages`. Differs: `searchWeb`/Lens (the Rust sidebar does it itself), WifiShare.scan after a shot. |
| services/OcrLangs.js | `ocr.rs` | Full port with tests. |
| services/Recorder.qml | `recorder.rs` | gsr/wf-recorder, countdown, CPU retry, pause, discard + 8 s undo, notifications when the island doesn't say. Check: `ipc call recorder toolbar`, record a region, `recorder pause`, `stop`; `discard` then `undo`. Differs: mic level meter (PipeWire peak) not read; mic mute uses `wpctl`. |
| services/LiveText.qml | `livetext.rs` | `livetext.read/act/install`, tsv parse, screenshot index (shot-index.py, low priority, not on battery < 30 %/power-saver). Differs: Wi-Fi code lookup answers `found: []`. Check: screenshot, open it in the editor, words are selectable. |
| modules/screentools/RustBridge.qml | `service.rs` (`state()`, `publish`) | Same `state` object to `screentools` and `quicklook`, sent when it changes (also at start / on outputs change). `Panels.onOpenChanged` options line is sent just before `panels.show screenTools`; toolbarRecord resets when host state `open` leaves screenTools. |
| modules/screentools/editor/ScreenshotEditor.qml | `screentools.rs` (`editor_shot`, `open_editor`), IPC `shotEditor edit|thumbnail|close` | Rust path only (the session lives in the surface). Thumbnails off -> notification with Edit / Show in Folder. |
| services/Captions.qml + modules/captions/CaptionsFeed.qml | `captions.rs` | helper `captions.py` kept via `watch.sh`; feed incl. CaptionsWindow's hint/dim/videoRect (focused window, no player match). IPC `captions toggle|on|off|demo`. Check: `ipc call captions demo "one" "two"`. Differs: video rect refreshes only on config/outputs/helper lines. |
| services/Dictation.qml | `dictation.rs` | IPC `dictation pressed|released|toggle|start|stop|cancel|status|install|download <name> [use]|cancelDownload|removeModel <name>`. Check: `ipc call dictation toggle`, speak, toggle again: typed via wtype (else copied). Differs: mic-muted check is a blocking `wpctl` call (ms); island "Open Settings"/"Unmute" actions are sent as `{call,args}` objects. |

Unowned: none found for these prefixes. `recorder.options` / `recorder.toolbar` are extra
calls I answer for Spotlight and the control center.

---

# host/desktop: today/ (Today, to-dos, timers, notifications…)

One host service, `Today` (`src/today/mod.rs`, `Hub`), holds all of these so they
share one 1 s tick and call each other directly. Written, not yet built or run
(the lead runs the one `cargo check`).

Calls it answers: `today`, `todos`, `todo`, `todoAdd`, `notes`, `notify`
(the brief's) and, added because the seams need them, `alarms`,
`notifications`, `nc`, `media`, `badges`. Ipc targets: `today`, `timer`,
`calendar`, `calls`.

State it says (`Host::set_state`): `unread`, `weather`, `daylight`
({sunrise,sunset}), `timers`, `alarms`, `calls`, `media`, `badges`, `today`.
It reads: `open` (the open panel), `locked`, `osd` (bool), `focus.on`,
`focus.name`, `privacy.micStreams`, `island.history`.

State on disk: `state.json` (todos, alarms, todayShown/Pinned/Ensured) is
read-modify-written whole per key (`common.rs`), `events.json` (your events),
`config.json` for Today's options (`today.set`) and the widget list.

## Per QML file

### Todos.qml -> `todos.rs`
Ported: keys `local/<id>`, give ids once, add with the parser (`parse.rs`),
update (a new `due` clears `notified`), remove (returns the item with its
place) / restore, `on(day)`, due label, overdue, sorted, the 30 s notify of
timed to-dos ("Reminders", "Due now", not for ones over an hour old).
Calls: `todos.toggle [key]`, `todo [key]`, `todoAdd [text]`.
Differs: Google / Microsoft To Do tasks are the system host's. Adding while
`google.taskListId` is set only tells you (notify-send "Task not added");
toggling a non-local key is logged.
Check: Today screen: tick a to-do; `tessera-ui ipc call ...` n/a. Desktop
Reminders widget: add "pay rent tomorrow 5pm": it appears due tomorrow 5 PM.

### Timers.qml + Alarms.qml -> `timers.rs`
Ported: Pomodoro (25/5/15, 4 sessions per long break, island or notify-send),
stopwatch with laps (20), alarms in state.json (`timer`/`alarm`), the alarm
sound (pw-play, else paplay), critical notification, island alert.
Ipc: `timer toggle|skip|reset|stopwatch|stopwatchReset`. Calls:
`alarms.addTimer [seconds,label]`, `alarms.addAlarm [at ms,label]`,
`alarms.cancel [id]`. Other modules parse text with `parse::parse_duration`,
`parse_clock`, `describe`.
Differs: due alarms fire on the 1 s tick (up to 0.5 s early, never late by
more than a second), not on an exact timer. The island reads Timers through
the state field `timers`.
Check: `tessera-ui ipc call timer toggle`, then Notification Center's Timer
tab counts down; `skip` posts "Time for a break" (island off) or the island.

### Calendar.qml -> `calendar.rs` (+ core `services/calendar.rs`)
Ported: your events' writes (`events.json`, mode 600), alerts every 30 s
(once, within 10 minutes of the alert time), `eventsOn`, `timeText`,
`dailyIn`/`seriesOf`, and ipc `calendar remind "<text>"` / `refresh`.
Differs: accounts' calendars (Google, iCloud, Microsoft 365) are the system
host's; subscription events have no VALARM/location/notes in the reader, so
they alert with the default alert (10 min, none all-day) and have no location
line. Subscriptions over the network are not fetched here.
Check: `tessera-ui ipc call calendar remind "call mum in 1 minute"` returns
"Call mum — Fri 9:41 AM"; a notification arrives at that time.

### Today.qml + TodayScreen.qml -> `morning.rs`
Ported: modes ask/widget/both/off, the 4 AM day, first-unlock-of-the-day
opening (900 ms; 2.8 s in the first 10 s), chime timed to the entrance,
Pin for today (the desktop adds the widget: `{"to":"desktop","add":["today","l"]}`),
`tidyWidgets` (config `desktop.widgets`), `today.set`, the open/close lines
to the Today surface (`open, effect, screen, options, motion, accounts`).
Ipc: `today preview <bloom|shrink|drop|"">`, `today close`. Calls:
`today.close`, `today.pin`, `today.set`, `today [options?]`, `today.show [fx]`
(for Spotlight's "Show Today").
Differs: `accounts` is `{events:[],todos:[]}` (system host); `Wallpaper.started`
taken as true; "locked" comes from the shell state `locked`; motion scale 1.
Check: `tessera-ui ipc call today preview drop`: the morning screen opens with
the drop entrance and the chime; Esc closes; Pin for today puts a Today widget
on the desktop.

### Notes.qml + NotesFeed.qml -> `notes.rs`
Ported: folder (`notes.folder`, else ~/Documents/Notes), list/search, file
name and content rules, save via `save-note.py`, the island "Note saved" /
"Couldn't save note", `{"to":"notes","folder"|"saving"|"saved"|"failed"}`,
`notes ["save", text]`, `notes ["openFolder"]` (closes the note, asks
`files.open [folder]`), `save_link`.
Check: Super+Alt+N, type, Ctrl+Enter: a `.md` file appears in the folder, the
island says "Note saved".

### Notifications.qml -> `notif.rs`
Ported (the part the shell keeps while the notifications surface serves):
the surface's list -> unread count (`unread` state), per new one by its
`route`: banner = sound (`sound.notify` ask with the Note), island = sound +
island card (kind notification, buttons, default action) or the banner back
(`{"do":"banner"}`), call = the call card or banner; hello at start;
covered/osd to the banners; clear all; Notification Center's widgets
(tab, focus line, to-dos, undo for 8 s, timer, stopwatch, alerts from state
`island.history`) and its calendar (marks, "every day", items, week lines);
all `nc` acts. Settings' `notify` calls: `clearAll` here; `play` ->
`sound.play [key,true]`, `playStartup` -> `sound.playStartup`, the Focus mode
ones -> `focus.<what>`, `installHyprbars` runs `install-hyprbars.sh`.
Differs: the island's duplicate checks, attaching buttons to alerts and the
fullscreen "Only important" rule are the island service's (the surface notes
the same); no Reply-composer target on opening the center from the island.
Island button table: alerts this makes have uids from 1_000_000_000;
`islandAct <uid> <1|btn i>` for them must reach `today.islandAct ["<uid> <which>"]`
(or ipc `today islandAct <uid> <which>`).
Check: `notify-send hi there`: bell count goes up, the banner shows; with the
island on and "notifications" chosen, a card shows; open Notification Center:
widgets and calendar fill in; tick a to-do, delete it and Undo.

### Calls.qml -> `calls.rs`
Ported: ringing (card kind `ringing`, ringtone every 2.6 s for ~30 s from the
style's `ringtone.ogg`, give up after 2 min, missed-call history line via
island `op:"record"`), answer/decline by the notification's own actions,
open-only apps, the app/window matching, the answered-call live window, the
call in progress (mic streams + browser meetings), ipc `calls answer|decline|
open|leave|state`.
Differs: the live call needs `privacy.micStreams` from the system host (none:
no live call). Opening the app asks `apps.show [address]` / `apps.launch [id]`.
Check: `notify-send -c call "Incoming call" Maya`: with the island's Calls on,
a ringing card shows and the tone repeats; `tessera-ui ipc call calls decline`.

### Weather.qml -> `weather.rs`; Media.qml -> `media.rs`; Badges.qml -> `badges.rs`
Weather: the reader is core's; this says `weather` and `daylight` and fetches
again when location/units change. Media: the pick (pinned/newest playing/last),
art memory, YouTube thumbnails, state `media`; calls `media [previous|next|…]`,
`media.pin [bus|""]`. Badges: LauncherEntry counts from `dbus-monitor` (same
text parser) or `badges.update [app,count,visible]`; state `badges`.
Check: `qs`-free: play music, state `media` has art places; send a
LauncherEntry update (e.g. Telegram) and the dock shows a count.

## Unowned / added prefixes
`alarms`, `notifications`, `nc`, `media`, `badges` are claimed here (the brief
lists none for them). `notifications` and `nc` need CORE-NEEDS-today.md.
Calls this group makes that others must own: `sound.notify`, `sound.play`,
`sound.playStartup`, `focus.*`, `panels.show|close|openSettings|ncTab`,
`island.forget|clear|alertAct`, `files.open`, `apps.show|launch`.

---

# host/desktop: AI (fork D)

Module: `src/ai/mod.rs` (service "Ai", call prefix `ai`, no IPC targets: none of
the Ai files declares an IpcHandler). Needs `mod ai;` and `ai::services()` in lib.rs.

The chat, models, credentials, prompt, SSE parser and history files are already
Rust in `core/src/services/ai/` and the sidebar owns the open chat, so the QML
singleton shrinks to what surfaces ask of it.

| QML file | Rust |
|---|---|
| Ai.qml | `ai/mod.rs`: `ai.quickAsk`, `ai.stopQuick`, `ai.continueQuick`, `ai.reloadHistory` (no-op: no second chat standalone). Chat, history, slash commands, attachments, model save/remove: core `ai::Chat` in the sidebar. |
| AiChats.js | core `services/ai/chats.rs` (used by the sidebar) |
| AiKeys.qml | core `services/ai/keys.rs`; the host reads it via `want("ai")` and re-looks up in the keyring when a key is missing |
| AiModels.js | core `services/ai/models.rs` |
| AiPrompt.js | core `services/ai/prompt.rs` |

Quick answers: same request as QML (system prompt "quick", effort low, 2048
tokens, no thinking), streamed on core's `stream` thread. Progress is told to
spotlight as `{"to":"spotlight","quick":{busy,answer,error}}` on each change
(what Search.qml's Connections did). Same error texts. A model change in
config stops a running quick answer (Ai.setModel/saveModel did). `continueQuick`
says `{"cmd":"continue","surface":"sidebar",question,answer}`, then asks
`sidebar.tab [0]` and `panels.show ["sidebar",""]` (host-shell's prefixes).

Helpers for the search service (Search.qml's extras): `crate::ai::meta(cfg)`
gives `{"configured","name"}` for `extras.ai`; `ai::selected(cfg)` the model.

Differences: no `Ai.notice`/chat state here (sidebar's Chat has them). Keys are
never logged; Request's Debug omits headers.

How to check: set a model and key in Settings -> AI; in Spotlight type a
question and choose "Ask ..."; the answer streams in; Esc stops it; "Continue
in chat" opens the sidebar on the chat tab with the exchange. With no key:
"Add your ... API key in Settings -> AI first."
